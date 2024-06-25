use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::time::Duration;
use std::vec;

use futures_util::StreamExt;

use crate::completion::{
    AutoCompletionTrait, CompletionItem, DBLexerCompletionBackend, DBListCompletionItem,
};
use crate::db::{
    CheckConn, DBTrait, ExportKind, Field, Index, ListTable, ListView, OffsetLimitIter, QueryResp,
    QuerySql, RedisListTable, SqlExecuteRet, TableListResp,
};
use crate::pool::{NexusPool, PoolManager};

/// https://stackoverflow.com/questions/30611215/redis-how-to-limit-the-return-number-of-keys-command

#[derive(Clone)]
pub struct RedisDB {
    server: String,
    pm: PoolManager,
    read_only: bool,
}

use redis::aio::ConnectionManager;
use redis::{AsyncCommands, AsyncIter, Value};

impl RedisDB {
    pub fn new(pm: PoolManager, server: String, read_only: bool) -> Self {
        Self {
            server,
            pm,
            read_only,
        }
    }

    fn key_value_fields() -> [Field; 2] {
        let key_field = Field {
            name: "key".to_string(),
            typ: "any".to_string(),
            nullable: false,
            default_value: None,
            is_primary: false,
            is_auto_increment: false,
        };
        let value_field = Field {
            name: "value".to_string(),
            typ: "any".to_string(),
            nullable: false,
            default_value: None,
            is_primary: false,
            is_auto_increment: false,
        };
        [key_field, value_field]
    }
}

impl RedisDB {
    async fn export_table_as_csv(
        &self, _key: &str, _include_header: bool, _delimiter: char,
        _sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        unimplemented!()
    }

    async fn export_table_as_json(
        &self, key: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let ty = Self::get_key_type(&mut conn, key).await?;
        match ty {
            RedisType::String => {
                let value: String = conn.get(key).await?;
                let mut m = HashMap::new();
                m.insert(key, value);
                let value = if pretty {
                    serde_json::to_string_pretty(&m)
                } else {
                    serde_json::to_string(&m)
                };
                let value = value?;
                sender.send(Cow::Owned(value)).await?;
            }
            RedisType::List => {
                sender.send(Cow::Borrowed("[")).await?;

                let total: isize = conn.llen(key).await?;
                let size: isize = 100;
                let iter = OffsetLimitIter::new(total, 100);
                let mut first = true;
                for x in iter {
                    let list: Vec<String> = conn.lrange(key, x, x + size).await?;
                    for l in list.iter() {
                        if !first {
                            sender.send(Cow::Borrowed(",")).await?;
                        }
                        if pretty {
                            sender.send(Cow::Borrowed("\n")).await?;
                        }
                        sender
                            .send(Cow::Owned(serde_json::to_string(l.as_str())?))
                            .await?;
                        first = false;
                    }
                }

                if pretty {
                    sender.send(Cow::Borrowed("\n")).await?;
                }
                sender.send(Cow::Borrowed("]")).await?;
            }
            RedisType::Set => {
                let mut iter = conn.sscan::<_, String>(key).await?;
                let mut first = true;
                sender.send(Cow::Borrowed("[")).await?;
                if pretty {
                    sender.send(Cow::Borrowed("\n")).await?;
                }

                while let Some(s) = iter.next().await {
                    if !first {
                        sender.send(Cow::Borrowed(",")).await?;
                        if pretty {
                            sender.send(Cow::Borrowed("\n")).await?;
                        }
                    }

                    sender
                        .send(Cow::Owned(serde_json::to_string(s.as_str())?))
                        .await?;
                    first = false;
                }
                if pretty {
                    sender.send(Cow::Borrowed("\n")).await?;
                }
                sender.send(Cow::Borrowed("]")).await?;
            }
            RedisType::Hash => {
                let mut stream: AsyncIter<'_, String> = conn.hscan(key).await?;
                let mut key = None as Option<String>;
                let mut m = HashMap::new();
                while let Some(value) = stream.next().await {
                    match &key {
                        Some(key2) => {
                            m.insert(key2.clone(), value);
                            key = None;
                        }
                        None => {
                            key = Some(value);
                        }
                    }
                }

                let value = if pretty {
                    serde_json::to_string_pretty(&m)?
                } else {
                    serde_json::to_string(&m)?
                };
                sender.send(Cow::Owned(value)).await?;
            }
            RedisType::Zset => {
                let mut stream: AsyncIter<'_, String> = conn.zscan(key).await?;
                let mut zkey = None as Option<String>;
                let mut m = HashMap::new();
                while let Some(score) = stream.next().await {
                    match zkey {
                        Some(zkey2) => {
                            zkey = None;
                            m.insert(zkey2, score);
                        }
                        None => {
                            zkey = Some(score);
                        }
                    }
                }
                let value = if pretty {
                    serde_json::to_string_pretty(&m)?
                } else {
                    serde_json::to_string(&m)?
                };
                sender.send(Cow::Owned(value)).await?;
            }
        }
        anyhow::Ok(())
    }

    async fn export_table_to_command(
        &self, key: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let ty = Self::get_key_type(&mut conn, key).await?;
        match ty {
            RedisType::String => {
                let x: String = conn.get(key).await?;
                sender
                    .send(Cow::Owned(format!("SET {} {}\n", key, x.as_str())))
                    .await?;
            }

            RedisType::List => {
                let total: isize = conn.llen(key).await?;
                let size: isize = 100;

                let iter = OffsetLimitIter::new(total, 100);
                for x in iter {
                    let list: Vec<String> = conn.lrange(key, x, x + size).await?;
                    for l in list.iter() {
                        sender
                            .send(Cow::Owned(format!("RPUSH {} {}\n", key, l)))
                            .await?;
                    }
                }
            }
            RedisType::Set => {
                let mut iter = conn.sscan::<_, String>(key).await?;
                while let Some(value) = iter.next().await {
                    sender
                        .send(Cow::Owned(format!("SADD {} {}\n", key, value.as_str())))
                        .await?;
                }
            }

            RedisType::Hash => {
                let mut stream: AsyncIter<'_, String> = conn.hscan(key).await?;
                let mut hkey = None as Option<String>;
                while let Some(s) = stream.next().await {
                    match hkey {
                        Some(value) => {
                            sender
                                .send(Cow::Owned(format!(
                                    "HMSET {} {} {}\n",
                                    key,
                                    value.as_str(),
                                    s.as_str()
                                )))
                                .await?;
                            hkey = None;
                        }

                        None => {
                            hkey = Some(s);
                        }
                    }
                }
            }

            RedisType::Zset => {
                let mut stream: AsyncIter<'_, String> = conn.zscan(key).await?;
                let mut zkey = None as Option<String>;
                while let Some(score) = stream.next().await {
                    match zkey {
                        Some(zkey2) => {
                            sender
                                .send(Cow::Owned(format!(
                                    "ZADD {} {} {}\n",
                                    key,
                                    score.as_str(),
                                    zkey2.as_str(),
                                )))
                                .await?;
                            zkey = None;
                        }
                        None => {
                            zkey = Some(score);
                        }
                    }
                }
            }
        }

        anyhow::Ok(())
    }
}
#[async_trait::async_trait]
impl CheckConn for RedisDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        let mut conn = self.get_conn().await?;
        let _e: bool = conn.exists("xxx").await?;

        anyhow::Ok(ins.elapsed())
    }
}

#[async_trait::async_trait]
impl DBTrait for RedisDB {
    fn read_only(&self) -> bool {
        self.read_only
    }

    fn backend_name(&self) -> &str {
        "redis"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone(&self) -> Box<dyn DBTrait> {
        Box::new(Clone::clone(self))
    }

    async fn show_view(
        &self, _view_name: &str, _where_str: Option<&str>, _order_by: Option<&str>, _page: usize,
        _page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        anyhow::bail!("redis have no view")
    }

    async fn export_table(
        &self, key: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::CSV {
                include_header,
                delimiter,
            } => {
                self.export_table_as_csv(key, include_header, delimiter, sender)
                    .await
            }
            ExportKind::Json { pretty } => self.export_table_as_json(key, pretty, sender).await,
            ExportKind::Sql {
                include_table_schema: _,
            } => self.export_table_to_command(key, sender.clone()).await,
        }
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let mut conn = self.get_conn().await?;
        let keys: Vec<String> = conn.keys("*").await?;
        let mut ret = Vec::with_capacity(keys.len());
        for k in keys.into_iter() {
            let typ = Self::get_key_type(&mut conn, k.as_str()).await?;
            let mut meta = HashMap::new();
            meta.insert("type".to_string(), typ.to_string());
            ret.push(ListTable { table: k });
        }
        anyhow::Ok(ret)
    }

    async fn show_columns(&self, key: &str) -> anyhow::Result<Vec<Field>> {
        let mut conn = self.get_conn().await?;
        let typ = Self::get_key_type(&mut conn, key).await?;
        let [key_field, value_field] = Self::key_value_fields();

        match typ {
            RedisType::String => anyhow::Ok(vec![value_field]),
            RedisType::List => anyhow::Ok(vec![value_field]),
            RedisType::Set => anyhow::Ok(vec![value_field]),
            RedisType::Hash => anyhow::Ok(vec![key_field, value_field]),
            RedisType::Zset => anyhow::Ok(vec![
                value_field,
                Field {
                    name: "score".to_string(),
                    typ: "float64".to_string(),
                    nullable: false,
                    default_value: None,
                    is_primary: false,
                    is_auto_increment: false,
                },
            ]),
        }
    }

    async fn show_table(
        &self, key: &str, where_str: Option<&str>, _order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let mut conn = self.get_conn().await?;
        let typ = Self::get_key_type(&mut conn, key).await?;
        use futures_util::StreamExt;
        match typ {
            RedisType::String => {
                let ret: Option<String> = conn.get(key).await?;
                anyhow::Ok(TableListResp::result(
                    "value",
                    format!("GET {}", key),
                    vec![ret],
                ))
            }
            RedisType::List => {
                let total: usize = conn.llen(key).await?;
                let start = (page - 1) * page;
                let ret: Vec<String> = conn
                    .lrange(key, start as isize, (start + page) as isize)
                    .await?;
                anyhow::Ok(TableListResp::new(
                    vec![Field::from_name_type("value", "string")],
                    total,
                    ret.into_iter().map(|x| vec![Some(x)]).collect::<Vec<_>>(),
                    format!("LRANGE {} {} {}  ", key, start, start + page_size),
                ))
            }

            RedisType::Set => {
                let total: usize = conn.scard(key).await?;
                let mut ret: AsyncIter<String> =
                    conn.sscan_match(key, where_str.unwrap_or("*")).await?;
                let mut set_values = Vec::new();
                while let Some(value) = ret.next().await {
                    set_values.push(Some(value));
                }
                anyhow::Ok(TableListResp::new(
                    vec![Field::from_name_type("value", "string")],
                    total,
                    set_values.into_iter().map(|x| vec![x]).collect(),
                    format!("SSCAN {} 0 {}", key, where_str.unwrap_or("*")),
                ))
            }
            RedisType::Hash => {
                let total: usize = conn.hlen(key).await?;
                let mut ret: AsyncIter<String> =
                    conn.hscan_match(key, where_str.unwrap_or("*")).await?;
                let mut index = 0;
                let mut key_values = Vec::new();
                let mut value_values = Vec::new();
                while let Some(value) = ret.next().await {
                    if index % 2 == 0 {
                        key_values.push(Some(value.clone()));
                    } else {
                        value_values.push(Some(value.clone()));
                    }
                    index += 1;
                }
                anyhow::Ok(TableListResp::new(
                    vec![
                        Field::from_name_type("key", "string"),
                        Field::from_name_type("value", "string"),
                    ],
                    total,
                    key_values
                        .into_iter()
                        .zip(value_values.into_iter())
                        .map(|(k, v)| vec![k, v])
                        .collect(),
                    format!("HSCAN {} 0 {}", key, where_str.unwrap_or("*")),
                ))
            }

            RedisType::Zset => {
                let total: usize = conn.zcard(key).await?;
                let mut ret: AsyncIter<String> =
                    conn.zscan_match(key, where_str.unwrap_or("*")).await?;
                let mut index = 0;
                let mut key_values = Vec::new();
                let mut value_values = Vec::new();
                while let Some(value) = ret.next().await {
                    if index % 2 == 0 {
                        key_values.push(Some(value.clone()));
                    } else {
                        value_values.push(Some(value.clone()));
                    }
                    index += 1;
                }
                anyhow::Ok(TableListResp::new(
                    vec![
                        Field::from_name_type("key", "string"),
                        Field::from_name_type("value", "string"),
                    ],
                    total,
                    key_values
                        .into_iter()
                        .zip(value_values.into_iter())
                        .map(|(k, v)| vec![k, v])
                        .collect(),
                    format!("ZSCAN {} 0 {}", key, where_str.unwrap_or("*")),
                ))
            }
        }
    }

    async fn show_indexes(&self, _table_name: &str) -> anyhow::Result<Vec<Index>> {
        anyhow::Ok(vec![])
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListView>> {
        anyhow::Ok(vec![])
    }

    async fn insert(
        &self, _table_name: &str, _values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        anyhow::bail!("insert should not called.")
    }

    async fn update(
        &self, _table_name: &str, _values: HashMap<String, Option<String>>,
        _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        anyhow::bail!("this function should not called.")
    }

    async fn truncate_table(&self, _table_name: &str) -> anyhow::Result<()> {
        anyhow::bail!("redis doesn't support this.")
    }

    async fn delete(
        &self, key: &str, _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let mut conn = self.get_conn().await?;
        conn.del(key).await?;
        anyhow::Ok(SqlExecuteRet {
            sql: format!("DEL {}", key),
            changes: 1,
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    async fn drop_view(&self, _view_name: &str) -> anyhow::Result<()> {
        anyhow::bail!("redis have no view")
    }

    async fn drop_table(&self, table: &str) -> anyhow::Result<()> {
        self.can_write()?;
        self.delete(table, Default::default()).await?;
        Ok(())
    }

    async fn redis_list_table(&self) -> anyhow::Result<RedisListTable> {
        let mut conn = self.get_conn().await?;
        let tables: Vec<String> = conn.keys("*").await?;
        let mut ret = RedisListTable::default();

        for table in tables.into_iter() {
            let typ = Self::get_key_type(&mut conn, table.as_str()).await?;
            match typ {
                RedisType::String => ret.string_table.push(table),
                RedisType::List => ret.list_table.push(table),
                RedisType::Set => ret.set_table.push(table),
                RedisType::Hash => ret.hash_table.push(table),
                RedisType::Zset => ret.zset_table.push(table),
            }
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl QuerySql for RedisDB {
    /// redis have no transaction any connection is fine
    /// so connection management for redis is dummy
    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        anyhow::Ok(id)
    }

    async fn re_connect(&self, _id: uuid::Uuid) -> anyhow::Result<()> {
        anyhow::Ok(())
    }

    async fn close_connection(&self, _id: uuid::Uuid) -> anyhow::Result<()> {
        anyhow::Ok(())
    }

    async fn query_sql(
        &self, _id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let mut conn = self.get_conn().await?;
        query_sql(&mut conn, sql).await
    }
}
impl RedisDB {
    async fn get_key_type(conn: &mut ConnectionManager, key: &str) -> anyhow::Result<RedisType> {
        let ty: String = conn.key_type(key).await?;
        RedisType::from_str(&ty)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RedisType {
    String,
    List,
    Set,
    Hash,
    Zset,
}

impl std::fmt::Display for RedisType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            RedisType::String => "string",
            RedisType::List => "list",
            RedisType::Set => "set",
            RedisType::Hash => "hash",
            RedisType::Zset => "zset",
        };
        write!(f, "{}", s)
    }
}
impl TryFrom<&str> for RedisType {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        RedisType::from_str(value)
    }
}

impl RedisType {
    fn from_str(s: &str) -> anyhow::Result<Self> {
        match s {
            "string" => Ok(Self::String),
            "list" => Ok(Self::List),
            "set" => Ok(Self::Set),
            "hash" => Ok(Self::Hash),
            "zset" => Ok(Self::Zset),
            _ => {
                anyhow::bail!(format!("'{}' not a valid type", s))
            }
        }
    }
}

async fn query_sql(
    conn: &mut ConnectionManager, sql: &str,
) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
    let mut commands = Vec::new();
    for line in sql.lines() {
        let v: Vec<_> = line.split(" ").collect();
        let mut v2 = Vec::new();
        for c in v.iter() {
            let c = (*c).trim().to_string();
            if !c.is_empty() {
                v2.push(c);
            }
        }
        if !v2.is_empty() {
            commands.push(v2);
        }
    }
    let mut ret = Vec::new();
    for c in commands.iter() {
        let args = &c[1..];
        let v = redis::cmd(c[0].as_str())
            .arg(args)
            .query_async(conn)
            .await
            .map(|x| encode_value(&x));

        match v {
            Ok(x) => {
                ret.push(Ok(QueryResp::result(
                    "result",
                    x.into_iter().map(|x| Some(x)).collect(),
                )));
            }

            Err(x) => ret.push(Err(x.to_string())),
        }
    }
    anyhow::Ok(ret)
}
fn encode_hashmap(v: &HashMap<String, String>) -> Vec<String> {
    vec![format!("{:?}", v)]
}
fn encode_hashset(v: &HashSet<String>) -> Vec<String> {
    vec![format!("{:?}", v)]
}

fn encode_value(v: &Value) -> Vec<String> {
    match v {
        Value::Nil => vec!["(nil)".to_string()],
        Value::Int(x) => vec![format!("integer({})", x)],
        Value::Data(bs) => {
            let m: Result<HashMap<String, String>, _> = redis::from_redis_value(v);
            match m {
                Ok(x) => return encode_hashmap(&x),
                Err(_) => {}
            }
            let m: Result<HashSet<String>, _> = redis::from_redis_value(v);
            match m {
                Ok(x) => return encode_hashset(&x),
                Err(_) => {}
            }
            let m: Result<Vec<String>, _> = redis::from_redis_value(v);
            match m {
                Ok(x) => return x,
                Err(_) => {}
            }
            let m: Result<String, _> = redis::from_redis_value(v);
            match m {
                Ok(x) => return vec![x],
                Err(_) => {}
            }
            return vec![format!(
                "unkownn object:0x{}",
                hex::encode_upper(bs.as_slice())
            )];
        }
        Value::Bulk(x) => {
            let data: Vec<_> = x.iter().map(|x| encode_value(&x)).collect();
            let mut ret = Vec::new();
            for d in data.into_iter() {
                for d in d.into_iter() {
                    ret.push(d);
                }
            }
            ret
        }
        Value::Status(x) => vec![format!("status({})", x)],
        Value::Okay => vec!["(ok)".to_string()],
    }
}

impl RedisDB {
    async fn get_conn(&self) -> anyhow::Result<redis::aio::ConnectionManager> {
        let m = match self
            .pm
            .get(&crate::datasource::DBPoolKey::Redis {
                server: self.server.clone(),
            })
            .await?
        {
            NexusPool::Redis(x) => x.clone(),
            _ => unreachable!(),
        };
        anyhow::Ok(m)
    }
}

#[cfg(test)]
mod tests {

    use super::RedisDB;
    use crate::TEST_HOST;

    fn make_test_object() -> RedisDB {
        let x = RedisDB {
            server: format!("redis://{}/", *TEST_HOST),
            pm: Default::default(),
            read_only: false,
        };
        x
    }
    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object();
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }
    #[tokio::test]
    async fn test() {
        let db = make_test_object();
        let db = &db as &dyn crate::db::DBTrait;
        let id = db.open_connection().await.unwrap();

        db.delete("string_value", Default::default()).await.unwrap();
        db.delete("hash_value", Default::default()).await.unwrap();
        db.delete("list_value", Default::default()).await.unwrap();
        db.delete("set_value", Default::default()).await.unwrap();
        db.delete("zset_value", Default::default()).await.unwrap();

        let ret = db
            .query_sql(id.clone(), "set string_value string_value")
            .await
            .unwrap();
        println!("ret:{:?}", ret);

        let ret = db
            .query_sql(id.clone(), "hset hash_value key11111 value2222")
            .await
            .unwrap();

        println!("ret:{:?}", ret);

        let ret = db
            .query_sql(id.clone(), "hset hash_value key2222 value33333")
            .await
            .unwrap();

        println!("ret:{:?}", ret);

        let ret = db
            .query_sql(id.clone(), "lpush list_value 1")
            .await
            .unwrap();
        db.query_sql(id.clone(), "lpush list_value 2")
            .await
            .unwrap();
        println!("ret:{:?}", ret);
        let ret = db.query_sql(id.clone(), "sadd set_value  1").await.unwrap();
        println!("{:?}", ret);
        let ret = db.query_sql(id.clone(), "sadd set_value  2").await.unwrap();
        println!("ret:{:?}", ret);
        let ret = db
            .query_sql(id.clone(), "zadd zset_value 1 haha")
            .await
            .unwrap();
        println!("ret:{:?}", ret);
        let ret = db
            .query_sql(id.clone(), "zadd zset_value 2 xxx")
            .await
            .unwrap();
        println!("ret:{:?}", ret);

        let redis_list_table = db.redis_list_table().await.unwrap();
        println!("redis list table:{:?}", redis_list_table);
        {
            let x = db
                .export_table_to_string(
                    "string_value",
                    crate::db::ExportKind::Sql {
                        include_table_schema: false,
                    },
                )
                .await
                .unwrap();
            println!("export str is :{}", x);
            let x = db
                .export_table_to_string(
                    "string_value",
                    crate::db::ExportKind::Json { pretty: true },
                )
                .await
                .unwrap();
            let _: serde_json::Value = serde_json::from_str(x.as_str()).unwrap();

            println!("export str is :{}", x);
            let x = db
                .export_table_to_string(
                    "set_value",
                    crate::db::ExportKind::Sql {
                        include_table_schema: false,
                    },
                )
                .await
                .unwrap();
            println!("export str is :{}", x);
            let x = db
                .export_table_to_string("set_value", crate::db::ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("export str is :{}", x);
            let _: serde_json::Value = serde_json::from_str(x.as_str()).unwrap();

            let x = db
                .export_table_to_string(
                    "hash_value",
                    crate::db::ExportKind::Sql {
                        include_table_schema: false,
                    },
                )
                .await
                .unwrap();
            println!("export str is :{}", x);
            let x = db
                .export_table_to_string("hash_value", crate::db::ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("export str is :{}", x);
            let _: serde_json::Value = serde_json::from_str(x.as_str()).unwrap();

            let x = db
                .export_table_to_string(
                    "zset_value",
                    crate::db::ExportKind::Sql {
                        include_table_schema: false,
                    },
                )
                .await
                .unwrap();
            println!("export str is :{}", x);
            let x = db
                .export_table_to_string("zset_value", crate::db::ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("export str is :{}", x);
            let _: serde_json::Value = serde_json::from_str(x.as_str()).unwrap();

            let x = db
                .export_table_to_string(
                    "list_value",
                    crate::db::ExportKind::Sql {
                        include_table_schema: false,
                    },
                )
                .await
                .unwrap();
            println!("export str is :{}", x);
            let x = db
                .export_table_to_string("list_value", crate::db::ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("export str is :{}", x);
            let _: serde_json::Value = serde_json::from_str(x.as_str()).unwrap();
        }

        let tables = db.list_table().await.unwrap();
        for table in tables.iter() {
            println!("table is {:?}", table);
            let cols = db.show_columns(table.table.as_str()).await.unwrap();
            println!("columns is {:?}", cols);
            let table_list = db
                .show_table(table.table.as_str(), None, None, 1, 100)
                .await
                .unwrap();
            println!("table list is {:?}", table_list);
            println!()
        }
    }

    #[tokio::test]
    async fn test2() {
        let db = make_test_object();
        let db = &db as &dyn crate::db::DBTrait;
        let id = db.open_connection().await.unwrap();
        let ret = db
            .query_sql(id.clone(), "set test_string_value string_value")
            .await
            .unwrap();
        println!("ret:{:?}", ret);

        let _ret = db
            .query_sql(id.clone(), "set test_string_value key11111")
            .await
            .unwrap();
        db.drop_table("test_string_value").await.unwrap();
    }
}

#[async_trait::async_trait]

impl AutoCompletionTrait for RedisDB {
    async fn auto_completion(
        &self, sql: &str, pos: sqlparser::tokenizer::Location,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        DBLexerCompletionBackend::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::GenericDialect,
            pos,
        )
        .await
    }
}
#[async_trait::async_trait]

impl DBListCompletionItem for RedisDB {
    fn is_multi_db(&self) -> bool {
        false
    }

    async fn list_table(&self, _db: Option<String>) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(
            crate::db::DBTrait::list_table(self)
                .await?
                .into_iter()
                .map(|x| x.table)
                .collect(),
        )
    }

    async fn list_db(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }

    async fn list_columns(
        &self, _db: Option<String>, _table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
}
#[async_trait::async_trait]
impl DBLexerCompletionBackend for RedisDB {
    type Dialect = sqlparser::dialect::GenericDialect;

    const BUILD_IN_FUNCTIONS: &'static [&'static str] = REDIS_COMMANDS;
    const KEY_WORDS: &'static [&'static str] = &[];
    const TEXTS: &'static [&'static str] = &[
        "NX",
        "XX",
        "EX",
        "PX",
        "CH",
        "INCR",
        "DECR",
        "WITHSCORES",
        "LIMIT",
        "ALPHA",
        "STORE",
        "BY",
        "RESETSTAT",
        "RESET",
        "REWRITE",
    ];
    const TYPES: &'static [&'static str] = &[];
}

const REDIS_COMMANDS: &'static [&'static str] = &[
    "APPEND",
    "ASKING",
    "AUTH",
    "BGREWRITEAOF",
    "BGSAVE",
    "BITCOUNT",
    "BITFIELD",
    "BITOP",
    "BITPOS",
    "BLPOP",
    "BRPOP",
    "BRPOPLPUSH",
    "CLIENT_GETNAME",
    "CLIENT_ID",
    "CLIENT_KILL",
    "CLIENT_LIST",
    "CLIENT_PAUSE",
    "CLIENT_REPLY",
    "CLIENT_SETNAME",
    "CLUSTER_ADDSLOTS",
    "CLUSTER_COUNT_FAILURE_REPORTS",
    "CLUSTER_COUNTKEYSINSLOT",
    "CLUSTER_DELSLOTS",
    "CLUSTER_FAILOVER",
    "CLUSTER_FORGET",
    "CLUSTER_GETKEYSINSLOT",
    "CLUSTER_INFO",
    "CLUSTER_KEYSLOT",
    "CLUSTER_MEET",
    "CLUSTER_MYID",
    "CLUSTER_NODES",
    "CLUSTER_REPLICATE",
    "CLUSTER_RESET",
    "CLUSTER_SAVECONFIG",
    "CLUSTER_SET_CONFIG_EPOCH",
    "CLUSTER_SETSLOT",
    "CLUSTER_SLAVES",
    "CLUSTER_SLOTS",
    "COMMAND",
    "CONFIG_GET",
    "CONFIG_REWRITE",
    "CONFIG_SET",
    "CONFIG_RESETSTAT",
    "DBSIZE",
    "DEBUG_OBJECT",
    "DEBUG_SEGFAULT",
    "DECR",
    "DECRBY",
    "DEL",
    "DISCARD",
    "DUMP",
    "ECHO",
    "EVAL",
    "EVALSHA",
    "EXEC",
    "EXISTS",
    "EXPIRE",
    "EXPIREAT",
    "FLUSHALL",
    "FLUSHDB",
    "GEOADD",
    "GEODIST",
    "GEOHASH",
    "GEOPOS",
    "GEORADIUS",
    "GEORADIUSBYMEMBER",
    "GET",
    "GETBIT",
    "GETRANGE",
    "GETSET",
    "HDEL",
    "HEXISTS",
    "HGET",
    "HGETALL",
    "HINCRBY",
    "HINCRBYFLOAT",
    "HKEYS",
    "HLEN",
    "HMGET",
    "HMSET",
    "HSCAN",
    "HSET",
    "HSETNX",
    "HSTRLEN",
    "HVALS",
    "INCR",
    "INCRBY",
    "INCRBYFLOAT",
    "INFO",
    "KEYS",
    "LASTSAVE",
    "LINDEX",
    "LINSERT",
    "LLEN",
    "LPOP",
    "LPUSH",
    "LPUSHX",
    "LRANGE",
    "LREM",
    "LSET",
    "LTRIM",
    "MGET",
    "MIGRATE",
    "MONITOR",
    "MOVE",
    "MSET",
    "MSETNX",
    "MULTI",
    "OBJECT",
    "PERSIST",
    "PEXPIRE",
    "PEXPIREAT",
    "PFADD",
    "PFCOUNT",
    "PFMERGE",
    "PING",
    "PSETEX",
    "PSUBSCRIBE",
    "PUBSUB_CHANNELS",
    "PUBSUB_NUMPAT",
    "PUBSUB_NUMSUB",
    "PTTL",
    "PUBLISH",
    "PUNSUBSCRIBE",
    "QUIT",
    "RANDOMKEY",
    "READONLY",
    "READWRITE",
    "RENAME",
    "RENAMENX",
    "RESTORE",
    "RESTORE_ASKING",
    "ROLE",
    "RPOP",
    "RPOPLPUSH",
    "RPUSH",
    "RPUSHX",
    "SADD",
    "SAVE",
    "SCAN",
    "SCARD",
    "SCRIPT_EXISTS",
    "SCRIPT_FLUSH",
    "SCRIPT_KILL",
    "SCRIPT_LOAD",
    "SDIFF",
    "SDIFFSTORE",
    "SELECT",
    "SET",
    "SETBIT",
    "SETEX",
    "SETNX",
    "SETRANGE",
    "SHUTDOWN",
    "SINTER",
    "SINTERSTORE",
    "SISMEMBER",
    "SLAVEOF",
    "SLOWLOG_GET",
    "SLOWLOG_LEN",
    "SLOWLOG_RESET",
    "SMEMBERS",
    "SMOVE",
    "SORT",
    "SPOP",
    "SRANDMEMBER",
    "SREM",
    "SSCAN",
    "STRLEN",
    "SUBSCRIBE",
    "SUNION",
    "SUNIONSTORE",
    "SYNC",
    "TIME",
    "TOUCH",
    "TTL",
    "TYPE",
    "UNSUBSCRIBE",
    "UNLINK",
    "UNWATCH",
    "WAIT",
    "WATCH",
    "ZADD",
    "ZCARD",
    "ZCOUNT",
    "ZINCRBY",
    "ZINTERSTORE",
    "ZLEXCOUNT",
    "ZRANGE",
    "ZRANGEBYLEX",
    "ZRANGEBYSCORE",
    "ZRANK",
    "ZREM",
    "ZREMRANGEBYLEX",
    "ZREMRANGEBYRANK",
    "ZREMRANGEBYSCORE",
    "ZREVRANGE",
    "ZREVRANGEBYLEX",
    "ZREVRANGEBYSCORE",
    "ZREVRANK",
    "ZSCAN",
    "ZSCORE",
    "ZUNIONSTORE",
];
