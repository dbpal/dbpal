use std::borrow::Cow;
use std::collections::HashMap;

use std::future::Future;

use std::pin::Pin;
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use std::task::Poll;
use std::time::Duration;
use std::vec;

use crate::completion::{
    in_pos, match_pattern_and_sort, AutoCompletionTrait, CompletionItem, CompletionKind,
};
use crate::db::{
    create_csv, BackendName, CheckConn, DBTrait, ExportKind, Field, FormatSql, FormatSqlOpts,
    Index, ListTable, MetaValue, OffsetLimitIter, QueryLog, QueryResp, QuerySql,
    QuerySqlWithoutOpenConn, SchemaTrait, ServerMetas, SqlExecuteRet, TableListResp,
};
use dashmap::DashMap;
use futures_core::future::BoxFuture;
use futures_util::FutureExt;
use futures_util::StreamExt;
use sqlparser::tokenizer::Token;
use tokio::sync::Mutex;
use typescript_type_def::TypeDef;

use redis::aio::{ConnectionManager, MultiplexedConnection};
use redis::{AsyncCommands, AsyncIter, Value};

/// https://stackoverflow.com/questions/30611215/redis-how-to-limit-the-return-number-of-keys-command

#[derive(Clone)]
pub struct RedisDB {
    username: Option<String>,
    password: Option<String>,
    host: String,
    port: Option<u16>,
    db: Option<String>,
    m: ConnectionManager,
    read_only: bool,
    query_connections: Arc<DashMap<uuid::Uuid, Arc<Mutex<MultiplexedConnection>>>>,
}

impl RedisDB {
    fn self_make_conn_string(&self) -> String {
        Self::make_conn_string(
            &self.host,
            self.port,
            self.username.as_ref().map(|x| x.as_str()),
            self.password.as_ref().map(|x| x.as_str()),
            self.db.as_ref().map(|x| x.as_str()),
        )
    }
    fn make_conn_string(
        host: &str, port: Option<u16>, username: Option<&str>, password: Option<&str>,
        db: Option<&str>,
    ) -> String {
        let mut ret = "redis://".to_owned();
        match &username {
            Some(username) => {
                ret.push_str(&username);
                match &password {
                    Some(pass) => {
                        ret.push_str(format!(":{}", pass).as_str());
                    }
                    None => {}
                }
                ret.push('@');
            }
            None => {}
        }
        // host and port
        ret.push_str(host);
        match port {
            Some(port) => {
                ret.push_str(format!(":{}", port).as_str());
            }
            None => {}
        }
        match &db {
            Some(db) => {
                ret.push_str(format!("/{}", &db[2..]).as_str());
            }
            None => {
                ret.push_str("/");
            }
        }
        ret
    }

    pub async fn new(
        host: impl Into<String>, port: Option<u16>, username: impl Into<Option<String>>,
        password: impl Into<Option<String>>, db: impl Into<Option<String>>, read_only: bool,
    ) -> anyhow::Result<Self> {
        let host = host.into();
        let username = username.into();
        let password: Option<String> = password.into();
        let db: Option<String> = db.into();

        let conn_str = Self::make_conn_string(
            &host,
            port,
            username.as_ref().map(|x| x.as_str()),
            password.as_ref().map(|x| x.as_str()),
            db.as_ref().map(|x| x.as_str()),
        );
        log::info!("connect to redis var:{:?}", conn_str);

        let client = redis::Client::open(conn_str.as_str())?;
        let m = redis::aio::ConnectionManager::new(client).await?;
        anyhow::Ok(Self {
            db,
            host,
            username,
            password,
            port,
            m,
            read_only,
            query_connections: Default::default(),
        })
    }
}

#[derive(Clone, Debug, serde::Serialize, Default, TypeDef)]
pub struct RedisListTable {
    pub(crate) string_table: Vec<String>,
    pub(crate) list_table: Vec<String>,
    pub(crate) set_table: Vec<String>,
    pub(crate) hash_table: Vec<String>,
    pub(crate) zset_table: Vec<String>,
}

#[async_trait::async_trait]
impl ServerMetas for RedisDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        // TODO get server metas
        let ret = Vec::new();
        anyhow::Ok(ret)
    }
}
// export impl
impl RedisDB {
    async fn export_table_as_csv(
        &self, key: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        if include_header {
            anyhow::bail!("redis have no header")
        }
        let mut conn = self.get_conn().await?;
        let ty = Self::get_key_type(&mut conn, key).await?;
        match ty {
            RedisType::String => {
                let value: String = conn.get(key).await?;
                sender
                    .send(create_csv(&vec![vec![Some(value)]], delimiter).into())
                    .await?;
            }

            RedisType::List => {
                let total: isize = conn.llen(key).await?;
                let size: isize = 100;
                let iter = OffsetLimitIter::new(total, 100);
                for x in iter {
                    let list: Vec<String> = conn.lrange(key, x, x + size).await?;
                    let list = list.into_iter().map(|x| vec![Some(x)]).collect::<Vec<_>>();
                    sender.send(create_csv(&list, delimiter).into()).await?;
                }
            }
            RedisType::Set => {
                let mut iter = conn.sscan::<_, String>(key).await?.ready_chunks(100);
                while let Some(set) = iter.next().await {
                    let set = set.into_iter().map(|x| vec![Some(x)]).collect::<Vec<_>>();
                    sender.send(create_csv(&set, delimiter).into()).await?;
                }
            }

            RedisType::Hash => {
                let mut stream: AsyncIter<'_, String> = conn.hscan(key).await?;
                let mut key = None as Option<String>;
                while let Some(value) = stream.next().await {
                    match &key {
                        Some(key2) => {
                            sender
                                .send(
                                    create_csv(
                                        &vec![vec![Some(key2.clone()), Some(value)]],
                                        delimiter,
                                    )
                                    .into(),
                                )
                                .await?;

                            key = None;
                        }
                        None => {
                            key = Some(value);
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
                                .send(
                                    create_csv(
                                        &vec![vec![Some(zkey2.clone()), Some(score)]],
                                        delimiter,
                                    )
                                    .into(),
                                )
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
                sender.send("[".into()).await?;

                let total: isize = conn.llen(key).await?;
                let size: isize = 100;
                let iter = OffsetLimitIter::new(total, 100);
                let mut first = true;
                for x in iter {
                    let list: Vec<String> = conn.lrange(key, x, x + size).await?;
                    for l in list.iter() {
                        if !first {
                            sender.send(",".into()).await?;
                        }
                        if pretty {
                            sender.send("\n".into()).await?;
                        }
                        sender
                            .send(Cow::Owned(serde_json::to_string(l.as_str())?))
                            .await?;
                        first = false;
                    }
                }
                if pretty {
                    sender.send("\n".into()).await?;
                }
                sender.send("]".into()).await?;
            }
            RedisType::Set => {
                let mut iter = conn.sscan::<_, String>(key).await?;
                let mut first = true;
                sender.send("[".into()).await?;
                if pretty {
                    sender.send("\n".into()).await?;
                }

                while let Some(s) = iter.next().await {
                    if !first {
                        sender.send(",".into()).await?;
                        if pretty {
                            sender.send("\n".into()).await?;
                        }
                    }

                    sender
                        .send(Cow::Owned(serde_json::to_string(s.as_str())?))
                        .await?;
                    first = false;
                }
                if pretty {
                    sender.send("\n".into()).await?;
                }
                sender.send("]".into()).await?;
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

    async fn export_table_as_json_line(
        &self, key: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let ty = Self::get_key_type(&mut conn, key).await?;
        match ty {
            RedisType::String => {
                let value: String = conn.get(key).await?;
                let mut m = HashMap::new();
                m.insert(key, value);
                let value = serde_json::to_string(&m);
                let value = value?;
                sender.send(Cow::Owned(value)).await?;
            }
            RedisType::List => {
                let total: isize = conn.llen(key).await?;
                let size: isize = 100;
                let iter = OffsetLimitIter::new(total, 100);
                for x in iter {
                    let list: Vec<String> = conn.lrange(key, x, x + size).await?;
                    let list = list
                        .into_iter()
                        .map(serde_json::Value::from)
                        .collect::<Vec<_>>();
                    let value = serde_json::value::Value::from(list);
                    let json_str = serde_json::to_string(&value)?;
                    sender.send(json_str.into()).await?;
                    sender.send("\n".into()).await?;
                }
            }
            RedisType::Set => {
                let mut iter = conn.sscan::<_, String>(key).await?.ready_chunks(100);
                while let Some(s) = iter.next().await {
                    let json_value = serde_json::value::Value::from(s);
                    let json_str = serde_json::to_string(&json_value)?;
                    sender.send(json_str.into()).await?;
                    sender.send("\n".into()).await?;
                }
            }

            RedisType::Hash => {
                let mut stream: AsyncIter<'_, String> = conn.hscan(key).await?;
                let mut key = None as Option<String>;
                while let Some(value) = stream.next().await {
                    match &key {
                        Some(key2) => {
                            let mut m = HashMap::new();
                            m.insert(key2.clone(), value);
                            let json_str = serde_json::to_string(&m)?;
                            sender.send(json_str.into()).await?;
                            sender.send("\n".into()).await?;
                            key = None;
                        }
                        None => {
                            key = Some(value);
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
                            zkey = None;
                            let mut m = HashMap::new();
                            m.insert(zkey2, score);
                            let json_str = serde_json::to_string(&m)?;
                            sender.send(json_str.into()).await?;
                            sender.send("\n".into()).await?;
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
        let _: bool = conn.exists("xxx").await?;
        anyhow::Ok(ins.elapsed())
    }
}

impl FormatSql for RedisDB {
    fn format_sql(&self, _sql: &str, _ops: Option<FormatSqlOpts>) -> anyhow::Result<String> {
        anyhow::bail!(format!(
            "'{}' don't support format sql",
            self.backend_name()
        ))
    }
}

impl BackendName for RedisDB {
    fn backend_name(&self) -> &str {
        "redis"
    }
}

#[async_trait::async_trait]
impl SchemaTrait for RedisDB {
    async fn table_exists(&self, table: &str) -> anyhow::Result<bool> {
        let mut conn = self.get_conn().await?;
        let e = conn.exists(table).await?;
        anyhow::Ok(e)
    }

    async fn duplicate_table(&self, key: &str, dup: &str) -> anyhow::Result<()> {
        if self.table_exists(dup).await? {
            anyhow::bail!(format!("table '{}' already exists", dup));
        }

        let mut conn = self.get_conn().await?;
        let ty = Self::get_key_type(&mut conn, key).await?;
        let mut conn_set = self.get_conn().await?;

        match ty {
            RedisType::String => {
                let value: String = conn.get(key).await?;
                let _: () = conn_set.set(dup, value).await?;
            }
            RedisType::List => {
                let total: isize = conn.llen(key).await?;
                let size: isize = 100;
                let iter = OffsetLimitIter::new(total, 100);
                for x in iter {
                    let list: Vec<String> = conn.lrange(key, x, x + size).await?;
                    let _: () = conn_set.rpush(dup, list).await?;
                }
            }
            RedisType::Set => {
                let mut iter = conn.sscan::<_, String>(key).await?.ready_chunks(100);
                while let Some(set) = iter.next().await {
                    let _: () = conn_set.sadd(dup, set).await?;
                }
            }

            RedisType::Hash => {
                let mut stream: AsyncIter<'_, String> = conn.hscan(key).await?;
                let mut key = None as Option<String>;
                while let Some(value) = stream.next().await {
                    match &key {
                        Some(key2) => {
                            let _: () = conn_set.hset(dup, key2, value).await?;
                            key = None;
                        }
                        None => {
                            key = Some(value);
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
                            let _: () = conn_set.zadd(dup, zkey2, score).await?;
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

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.rename(old, new).await?;
        anyhow::Ok(())
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
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
            } => self.export_table_to_command(key, sender).await,
            ExportKind::JsonLine => self.export_table_as_json_line(&key, sender).await,
        }
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        anyhow::bail!("use redis list table")
    }

    async fn show_columns(&self, key: &str) -> anyhow::Result<Vec<Field>> {
        let mut conn = self.get_conn().await?;
        let typ = Self::get_key_type(&mut conn, key).await?;

        match typ {
            RedisType::String => anyhow::Ok(vec![Field::from_name_type("value", "string")]),
            RedisType::List => anyhow::Ok(vec![Field::from_name_type("list element", "string")]),
            RedisType::Set => anyhow::Ok(vec![Field::from_name_type("set element", "string")]),
            RedisType::Hash => anyhow::Ok(vec![
                Field::from_name_type("key", "string"),
                Field::from_name_type("value", "string"),
            ]),
            RedisType::Zset => anyhow::Ok(vec![
                Field::from_name_type("value", "string"),
                Field::from_name_type("score", "float64"),
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
        anyhow::bail!("don't call redis show index method")
    }

    async fn insert(
        &self, _table_name: &str, _values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        anyhow::bail!("call query sql instread")
    }

    async fn update(
        &self, _table_name: &str, _values: HashMap<String, Option<String>>,
        _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        anyhow::bail!("call query sql instread")
    }

    async fn truncate_table(&self, _table_name: &str) -> anyhow::Result<()> {
        anyhow::bail!("call query sql instread")
    }

    async fn delete(
        &self, _key: &str, _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        anyhow::bail!("not impl")
    }

    async fn drop_table(&self, table: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn: ConnectionManager = self.get_conn().await?;
        conn.del::<_, ()>(table).await?;
        Ok(())
    }
}

impl RedisDB {
    pub async fn redis_list_table(&self) -> anyhow::Result<RedisListTable> {
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
impl DBTrait for RedisDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let r: String = redis::cmd("info")
            .arg("keyspace")
            .query_async(&mut conn)
            .await?;

        extract_db_from_info_key_space(&r)
    }
    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let db = Self::new(
            self.host.clone(),
            self.port,
            self.username.clone(),
            self.password.clone(),
            Some(name.to_string()),
            self.read_only,
        )
        .await?;
        anyhow::Ok(Arc::new(db))
    }
}

#[async_trait::async_trait]
impl QuerySql for RedisDB {
    /// redis have no transaction any connection is fine
    /// so connection management for redis is dummy
    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        let client = self.get_client().await?;
        let conn = client.get_multiplexed_async_connection().await?;
        self.query_connections
            .insert(id.clone(), Arc::new(Mutex::new(conn)));
        anyhow::Ok(id)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        let client = self.get_client().await?;
        let conn = client.get_multiplexed_async_connection().await?;
        self.query_connections
            .insert(id.clone(), Arc::new(Mutex::new(conn)));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        QueryLog::remove(&id);

        self.query_connections.remove(&id);
        anyhow::Ok(())
    }

    async fn query_sql(
        &self, id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let mut query_log = QueryLog::get_guard(id, sql);
        let conn = match self.query_connections.get(&id) {
            Some(x) => x.clone(),
            None => {
                anyhow::bail!(format!("can't find connection for id '{}'", id))
            }
        };
        let mut lock = conn.lock().await;

        let ret = query_sql(&mut lock, sql).await;
        query_log.set_success(match &ret {
            Ok(x) => x.iter().all(|x| x.is_ok()),
            Err(_) => false,
        });
        ret
    }
}

impl RedisDB {
    async fn get_key_type(conn: &mut ConnectionManager, key: &str) -> anyhow::Result<RedisType> {
        let ty: String = conn.key_type(key).await?;
        log::trace!("key:{} type:{}", key, ty);
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

impl FromStr for RedisType {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "string" => Ok(Self::String),
            "list" => Ok(Self::List),
            "set" => Ok(Self::Set),
            "hash" => Ok(Self::Hash),
            "zset" => Ok(Self::Zset),
            _ => Err(anyhow::Error::msg(format!("redis type '{}' not handle", s))),
        }
    }
}

#[async_trait::async_trait]
impl QuerySqlWithoutOpenConn for RedisDB {
    async fn query_sql(&self, sql: &str) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let conn = self.get_client().await?;
        let mut conn = conn.get_multiplexed_async_connection().await?;
        query_sql(&mut conn, sql).await
    }
}

async fn query_sql(
    conn: &mut redis::aio::MultiplexedConnection, sql: &str,
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
            .map(|x| encode_response_value(&x));

        match v {
            Ok(x) => {
                ret.push(Ok(QueryResp::result("result", vec![Some(x)])));
            }

            Err(x) => ret.push(Err(x.to_string())),
        }
    }
    anyhow::Ok(ret)
}

/// some
fn encode_response_value(v: &Value) -> String {
    match v {
        Value::Nil => "(nil)".to_string(),
        Value::Int(x) => format!("integer({})", x),
        Value::Double(x) => format!("double({})", x),
        Value::Boolean(x) => format!("bool({})", x),
        Value::BigNumber(x) => format!("big_number({})", x),
        Value::ServerError(s) => {
            format!(
                "error(code:{},detail:{})",
                s.code(),
                s.details().unwrap_or("")
            )
        }
        Value::Okay => "(ok)".to_string(),
        _ => {
            log::error!("redis response not handle,resp:{:?}", v);
            "unhandle response".to_string()
        }
    }
}

impl RedisDB {
    async fn get_client(&self) -> anyhow::Result<redis::Client> {
        let client = redis::Client::open(self.self_make_conn_string().as_str())?;
        anyhow::Ok(client)
    }

    async fn get_conn(&self) -> anyhow::Result<redis::aio::ConnectionManager> {
        anyhow::Ok(self.m.clone())
    }
}

/*
extract database from those vec result.

```# Keyspace
db0:keys=12,expires=0,avg_ttl=0
db1:keys=1,expires=0,avg_ttl=0
```
*/
fn extract_db_from_info_key_space(result: &str) -> anyhow::Result<Vec<String>> {
    let mut ret = Vec::new();
    for r in result.lines().skip(1) {
        match r.find(":") {
            Some(index) => {
                ret.push(r[0..index].to_string());
            }
            None => {
                anyhow::bail!("the result of INFO KEYSPACE have changed")
            }
        }
    }
    anyhow::Ok(ret)
}

#[cfg(test)]
mod tests {

    use std::time::Duration;

    use super::{extract_db_from_info_key_space, RedisDB};
    use crate::{
        completion::AutoCompletionTrait,
        db::{ExportKind, ServerMetas},
        init_log, TEST_HOST,
    };

    async fn make_test_object() -> RedisDB {
        let db = RedisDB::new(
            TEST_HOST.to_string(),
            None,
            None,
            None,
            Some("db0".to_string()),
            false,
        )
        .await
        .unwrap();
        db
    }

    #[test]
    fn test_extract_db_from_info_key_space() {
        let r = extract_db_from_info_key_space(
            r#"# Keyspace
db0:keys=12,expires=0,avg_ttl=0
db1:keys=1,expires=0,avg_ttl=0"#,
        )
        .unwrap();
        assert_eq!(r, vec!["db0".to_string(), "db1".to_string()]);
    }

    #[tokio::test]
    async fn test_auto_completion() {
        let db = make_test_object().await;
        let items = db
            .auto_completion(
                "append str  ",
                sqlparser::tokenizer::Location { line: 1, column: 9 },
            )
            .await
            .unwrap();
        println!("{:?}", items);
    }
    #[tokio::test]
    async fn test_server_metas() {
        let db = make_test_object().await;
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }

    #[tokio::test]
    async fn test_list_redis_table() {
        init_log();
        let db = make_test_object().await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        let redis_tables = db.redis_list_table().await.unwrap();
        println!("redis tables:{:?}", redis_tables);
    }

    #[tokio::test]
    async fn test_rename() {
        let db = make_test_object().await;
        let db = &db as &dyn crate::db::SchemaTrait;
        let query_id = db.open_connection().await.unwrap();
        db.query_sql(
            query_id,
            r#"
            SET test_rename_key  your_value 
        "#,
        )
        .await
        .unwrap();

        db.rename_table("test_rename_key", "test_rename_key2")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object().await;
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }
    #[test]
    fn test_make_conn_string() {
        assert_eq!(
            RedisDB::make_conn_string("127.0.0.1", Some(111), None, None, None),
            "redis://127.0.0.1:111/"
        );
        assert_eq!(
            RedisDB::make_conn_string("127.0.0.1", None, None, None, None),
            "redis://127.0.0.1/"
        );
        assert_eq!(
            RedisDB::make_conn_string("127.0.0.1", None, Some("aaa"), Some("bbb"), None),
            "redis://aaa:bbb@127.0.0.1/"
        );
        assert_eq!(
            RedisDB::make_conn_string("127.0.0.1", None, Some("aaa"), None, None),
            "redis://aaa@127.0.0.1/"
        );

        assert_eq!(
            RedisDB::make_conn_string("127.0.0.1", None, Some("aaa"), None, Some("db1")),
            "redis://aaa@127.0.0.1/1"
        );
    }
    #[tokio::test]
    async fn test_list_schema() {
        init_log();
        let db = make_test_object().await;
        let db = &db as &dyn crate::db::DBTrait;

        let dbnames = db.list_schema().await.unwrap();
        for dbname in dbnames.iter() {
            db.open_schema(&dbname).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_check_alot_of_key() {
        let db = make_test_object().await;
        crate::db::CheckConn::check_conn(&db).await.unwrap();
        // let query_id = db.open_connection().await.unwrap();

        // for i in 0..100000 {
        //     db.query_sql(
        //         query_id,
        //         format!("set test_alot_of_key{} {}", i, i).as_str(),
        //     )
        //     .await
        //     .unwrap();

        //     println!("{}", i);
        // }
        let now = std::time::Instant::now();
        let x = db.redis_list_table().await.unwrap();
        println!("used{:?} key_len:{}", now.elapsed(), x.string_table.len());
    }

    #[tokio::test]
    async fn test_hash_key() {
        let db = make_test_object().await;
        let db = &db as &dyn crate::db::SchemaTrait;
        let query_id = db.open_connection().await.unwrap();
        let table_name = "hash_value";

        let _ = db.drop_table(table_name).await;

        let ret = db
            .query_sql(query_id.clone(), "hset hash_value key11111 value2222")
            .await
            .unwrap();
        println!("query ret :{:?}", ret);

        let ret = db
            .query_sql(query_id.clone(), "hset hash_value key2222 value33333")
            .await
            .unwrap();
        println!("query ret :{:?}", ret);
        // export
        {
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: false,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", s);
            let s = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            let _: serde_json::Value = serde_json::from_str(&s).unwrap();
            println!("json:{}", s);

            let s = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json_line:{}", s);
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: false,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", s);
        }
        let cols = db.show_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("resp:{:?}", resp);
        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(&table_name_dup).await;

        db.duplicate_table(&table_name, &table_name_dup)
            .await
            .unwrap();
        let resp2 = db
            .show_table(&table_name_dup, None, None, 1, 100)
            .await
            .unwrap();
        assert_eq!(resp, resp2);
    }

    #[tokio::test]
    async fn test_zset_key() {
        let db = make_test_object().await;
        let db = &db as &dyn crate::db::SchemaTrait;
        let query_id = db.open_connection().await.unwrap();
        let table_name = "zset_value";

        let _ = db.drop_table(table_name).await;

        let ret = db
            .query_sql(query_id.clone(), "zadd zset_value 1 haha")
            .await
            .unwrap();
        println!("query ret :{:?}", ret);

        let ret = db
            .query_sql(query_id.clone(), "zadd zset_value 2 xxx")
            .await
            .unwrap();
        println!("query ret :{:?}", ret);
        // export
        {
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: false,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", s);
            let s = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            let _: serde_json::Value = serde_json::from_str(&s).unwrap();
            println!("json:{}", s);

            let s = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json_line:{}", s);
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: false,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", s);
        }
        let cols = db.show_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("resp:{:?}", resp);
        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(&table_name_dup).await;
        db.duplicate_table(&table_name, &table_name_dup)
            .await
            .unwrap();
        let resp2 = db
            .show_table(&table_name_dup, None, None, 1, 100)
            .await
            .unwrap();
        assert_eq!(resp, resp2);
    }

    #[tokio::test]
    async fn test_set_key() {
        let db = make_test_object().await;
        let db = &db as &dyn crate::db::SchemaTrait;
        let query_id = db.open_connection().await.unwrap();
        let table_name = "set_value";

        let _ = db.drop_table(table_name).await;

        let ret = db
            .query_sql(query_id.clone(), "sadd set_value a")
            .await
            .unwrap();
        println!("query ret :{:?}", ret);

        let ret = db
            .query_sql(query_id.clone(), "sadd set_value b")
            .await
            .unwrap();
        println!("query ret :{:?}", ret);
        // export
        {
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: false,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", s);
            let s = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            let _: serde_json::Value = serde_json::from_str(&s).unwrap();
            println!("json:{}", s);

            let s = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json_line:{}", s);
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: false,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", s);
        }
        let cols = db.show_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("resp:{:?}", resp);
        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(table_name_dup.as_str()).await;
        db.duplicate_table(&table_name, &table_name_dup)
            .await
            .unwrap();
        let resp2 = db
            .show_table(&table_name_dup, None, None, 1, 100)
            .await
            .unwrap();
        assert_eq!(resp, resp2);
    }

    #[tokio::test]
    async fn test_list_key() {
        let db = make_test_object().await;
        let db = &db as &dyn crate::db::SchemaTrait;
        let query_id = db.open_connection().await.unwrap();
        let table_name = "list_value";

        let _ = db.drop_table(table_name).await;

        let ret = db
            .query_sql(query_id.clone(), "lpush list_value a")
            .await
            .unwrap();
        println!("query ret :{:?}", ret);

        let ret = db
            .query_sql(query_id.clone(), "lpush list_value b")
            .await
            .unwrap();
        println!("query ret :{:?}", ret);
        // export
        {
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: false,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", s);
            let s = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            let _: serde_json::Value = serde_json::from_str(&s).unwrap();
            println!("json:{}", s);

            let s = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json_line:{}", s);
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: false,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", s);
        }
        let cols = db.show_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("resp:{:?}", resp);
        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(table_name_dup.as_str()).await;
        db.duplicate_table(&table_name, &table_name_dup)
            .await
            .unwrap();
        let resp2 = db
            .show_table(&table_name_dup, None, None, 1, 100)
            .await
            .unwrap();
        assert_eq!(resp, resp2);
    }

    #[tokio::test]
    async fn test_string_key() {
        let db = make_test_object().await;
        let db = &db as &dyn crate::db::SchemaTrait;
        let query_id = db.open_connection().await.unwrap();
        let table_name = "string_value";

        let _ = db.drop_table(table_name).await;

        let ret = db
            .query_sql(query_id.clone(), "set string_value string_value")
            .await
            .unwrap();
        println!("query_ret:{:?}", ret);

        // export
        {
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: false,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", s);
            let s = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            let _: serde_json::Value = serde_json::from_str(&s).unwrap();
            println!("json:{}", s);

            let s = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json_line:{}", s);
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: false,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", s);
        }
        let cols = db.show_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("resp:{:?}", resp);

        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(&table_name_dup).await;
        db.duplicate_table(&table_name, &table_name_dup)
            .await
            .unwrap();
        let resp2 = db
            .show_table(&table_name_dup, None, None, 1, 100)
            .await
            .unwrap();
        assert_eq!(resp, resp2);
    }
}

#[async_trait::async_trait]

impl AutoCompletionTrait for RedisDB {
    async fn auto_completion(
        &self, sql: &str, pos: sqlparser::tokenizer::Location,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let tokens =
            sqlparser::tokenizer::Tokenizer::new(&RedisDialet {}, sql).tokenize_with_location()?;
        if tokens.len() == 0 {
            anyhow::bail!("a empty line");
        }
        let tokens = tokens
            .into_iter()
            .filter(|x| match &x.token {
                Token::Whitespace(_) => false,
                _ => true,
            })
            .collect::<Vec<_>>();
        if tokens.len() == 0 {
            anyhow::bail!("no tokens");
        }
        let command = match &tokens[0].token {
            Token::Word(word) => {
                if in_pos(pos, tokens[0].location, word.value.len() as u64) {
                    // we are auto completion on command.
                    let x = match_pattern_and_sort(
                        static_redis_command_completion_items(),
                        &word.value,
                    );
                    return anyhow::Ok(x);
                }
                word.value.clone()
            }
            _ => {
                anyhow::bail!("first token must be a command")
            }
        };

        let patterns: Vec<String> = tokens.iter().map(|x| x.to_string()).collect();
        for (token, f) in tokens
            .iter()
            .skip(
                1, // skip command
            )
            .zip(
                self.make_auto_completion_fn_by_command(&command, &patterns)?
                    .into_iter(),
            )
        {
            println!("{:?}", token);
            match &token.token {
                Token::Word(word) => {
                    if in_pos(pos, token.location, word.value.len() as u64) {
                        let item = f.await;
                        return item.map(|item| match_pattern_and_sort(item, &word.value));
                    }
                }
                _ => {
                    continue;
                }
            }
        }
        anyhow::Ok(vec![])
    }
}

impl RedisDB {
    async fn completion_keys(
        &self, pattern: &str, kind: Option<RedisType>,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let mut conn = self.get_conn().await?;
        let keys: Vec<String> = conn.keys(format!("{}*", pattern)).await?;
        let mut ret = Vec::new();
        for key in keys.into_iter() {
            let type_matched = match kind {
                Some(ty2) => {
                    let ty = Self::get_key_type(&mut conn, &key).await?;
                    ty == ty2
                }
                None => true,
            };
            if type_matched {
                ret.push(CompletionItem {
                    item: key.into(),
                    kind: CompletionKind::TableName,
                })
            }
        }
        anyhow::Ok(ret)
    }
    async fn completion_hash_keys(&self, key: &str) -> anyhow::Result<Vec<CompletionItem>> {
        let mut conn = self.get_conn().await?;
        let keys: Vec<String> = conn.hkeys(key).await?;
        anyhow::Ok(
            keys.into_iter()
                .map(|x| CompletionItem {
                    kind: CompletionKind::Column,
                    item: x.into(),
                })
                .collect(),
        )
    }

    fn make_auto_completion_fn_by_command<'a, 'b>(
        &'b self, command: &str, patterns: &'a [String],
    ) -> anyhow::Result<Vec<BoxFuture<'a, anyhow::Result<Vec<CompletionItem>>>>>
    where
        'b: 'a,
    {
        /// dummy auto completion don't return anything
        #[derive(Clone, Copy, Debug)]
        struct Dummy {}
        impl Future for Dummy {
            type Output = anyhow::Result<Vec<CompletionItem>>;
            fn poll(self: Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
                Poll::Ready(anyhow::Ok(vec![]))
            }
        }
        let dummy = Box::pin(Dummy {});

        let fns: Vec<BoxFuture<'_, anyhow::Result<Vec<CompletionItem>>>> = match command
            .to_uppercase()
            .as_str()
        {
            "APPEND" | "SET" => {
                let key = patterns.get(0);
                match key {
                    Some(key) => {
                        vec![
                        async move { self.completion_keys(&key, Some(RedisType::String)).await }
                            .boxed(),
                    ]
                    }
                    None => vec![],
                }
            }
            "BGREWRITEAOF" => {
                vec![async move {
                    anyhow::Ok(vec![
                        CompletionItem {
                            item: "SCHEDULE".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "FORCE".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "SCHEDULE FORCE".into(),
                            kind: CompletionKind::Text,
                        },
                    ])
                }
                .boxed()]
            }
            "BGSAVE" => {
                vec![async move {
                    anyhow::Ok(vec![
                        CompletionItem {
                            item: "SCHEDULE".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "FORCE".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "SCHEDULE FORCE".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "SCHEDULE CRON".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "SCHEDULE INTERVAL".into(),
                            kind: CompletionKind::Text,
                        },
                    ])
                }
                .boxed()]
            }
            "BITCOUNT" => {
                // TODO
                vec![]
            }
            "BITFIELD" => {
                let key = patterns.get(0);
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key, None).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }

            "BITOP" => {
                let mut vs = vec![async move {
                    anyhow::Ok(vec![
                        CompletionItem {
                            item: "AND".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "OR".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "XOR".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "NOT".into(),
                            kind: CompletionKind::Text,
                        },
                    ])
                }
                .boxed()];
                let x = patterns
                    .iter()
                    .skip(1)
                    .map(|x| async move { self.completion_keys(&x, None).await }.boxed())
                    .collect::<Vec<_>>();

                vs.extend(x.into_iter());
                vs
            }
            "BITPOS" => {
                let key = patterns.get(0);
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key, None).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }

            "BLPOP" | "BRPOP" => {
                let x = (&patterns[..patterns.len() - 1  // last one is timeout 
                    ])
                    .iter()
                    .map(|key| {
                        async move { self.completion_keys(&key, Some(RedisType::List)).await }
                            .boxed()
                    })
                    .collect::<Vec<_>>();
                x
            }

            "BRPOPLPUSH" => {
                let key = patterns.get(0);
                let key = match key {
                    Some(key) => {
                        let key = key.to_string();
                        async move { self.completion_keys(&key, Some(RedisType::List)).await }
                            .boxed()
                    }
                    None => dummy.clone(),
                };
                let key2 = patterns.get(1);

                let key2 = match key2 {
                    Some(key) => {
                        let key = key.to_string();
                        async move { self.completion_keys(&key, Some(RedisType::List)).await }
                            .boxed()
                    }
                    None => dummy.clone(),
                };
                vec![key, key2]
            }
            "CLIENT" => {
                vec![async move {
                    anyhow::Ok(vec![
                        // 客户端连接管理
                        CompletionItem {
                            item: "CONNECT".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "DISCONNECT".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "KILL".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "LIST".into(),
                            kind: CompletionKind::Text,
                        },
                        // 客户端信息获取
                        CompletionItem {
                            item: "GETNAME".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "ID".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "ADDR".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "PORT".into(),
                            kind: CompletionKind::Text,
                        },
                        //  客户端连接配置
                        CompletionItem {
                            item: "SETNAME".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "PAUSE".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "REPLY".into(),
                            kind: CompletionKind::Text,
                        },
                        // 客户端连接监控
                        CompletionItem {
                            item: "LIST".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "TRACKING".into(),
                            kind: CompletionKind::Text,
                        },
                    ])
                }
                .boxed()]
            }
            "CLUSTER" => {
                vec![async move {
                    anyhow::Ok(vec![
                        //集群创建和配置
                        CompletionItem {
                            item: "CREATE".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "MEET".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "ADDSLOTS".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "REPLICATE".into(),
                            kind: CompletionKind::Text,
                        },
                        // 集群状态查询
                        CompletionItem {
                            item: "NODES".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "INFO".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "KEYSLOT".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "COUNTKEYSINSLOT".into(),
                            kind: CompletionKind::Text,
                        },
                        // 集群维护和修复
                        CompletionItem {
                            item: "FAILOVER".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "FORGET".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "FLUSHSLOTS".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "RESET".into(),
                            kind: CompletionKind::Text,
                        },
                        // 集群扩展和缩容
                        CompletionItem {
                            item: "ADDSLOTS".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "DELSLOTS".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "SETSLOT".into(),
                            kind: CompletionKind::Text,
                        },
                        // 其他集群操作
                        CompletionItem {
                            item: "SAVECONFIG".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "SLOTS".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "TALK".into(),
                            kind: CompletionKind::Text,
                        },
                    ])
                }
                .boxed()]
            }
            "CONFIG" => {
                vec![async move {
                    anyhow::Ok(vec![
                        CompletionItem {
                            item: "GET".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "SET".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "RESETSTAT".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "REWRITE".into(),
                            kind: CompletionKind::Text,
                        },
                    ])
                }
                .boxed()]
            }
            "DECR" | "DECRBY" => {
                let key = patterns.get(0);
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key, Some(RedisType::String )).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }

            "DEL" => {
                let key = patterns.get(0);
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key, None).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }
            "DUMP" => {
                let key = patterns.get(0);
                match key {
                    Some(key) => {
                        let key = key.to_string();
                        vec![async move { self.completion_keys(&key, None).await }.boxed()]
                    }
                    None => vec![dummy.clone()],
                }
            }
            "EXISTS" => {
                let x = patterns
                    .iter()
                    .map(|key| {
                        async move { self.completion_keys(&key, Some(RedisType::List)).await }
                            .boxed()
                    })
                    .collect::<Vec<_>>();
                x
            }

            "EXPIRE" | "EXPIREAT" => {
                let mut count = 0;
                let x = patterns
                    .iter()
                    .map(|key| {
                        let x =
                            async move { self.completion_keys(&key, Some(RedisType::List)).await }
                                .boxed();
                        count += 1;
                        x
                    })
                    .collect::<Vec<_>>();
                x
            }
            "GEOADD" | "GEODIST" | "GEOHASH" | "GEOPOS" | "GEORADIUS" | "GEORADIUSBYMEMBER" => {
                // geo data use zset to store.
                if patterns.len() > 0 {
                    vec![async move {
                        self.completion_keys(&patterns[0], Some(RedisType::Zset))
                            .await
                    }
                    .boxed()]
                } else {
                    vec![]
                }
            }
            "GET" | "GETBIT" | "GETRANGE" | "GETSET" => {
                if patterns.len() > 0 {
                    vec![async move {
                        self.completion_keys(&patterns[0], Some(RedisType::String))
                            .await
                    }
                    .boxed()]
                } else {
                    vec![]
                }
            }
            "HDEL" | "HEXISTS" | "HGET" => patterns
                .iter()
                .enumerate()
                .map(|(index, p)| match index {
                    0 => {
                        // suggest hash key
                        async move { self.completion_keys(p, Some(RedisType::Hash)).await }.boxed()
                    }
                    1 => {
                        let key = &patterns[0];

                        // hash key store in pattern[0]
                        async move { self.completion_hash_keys(key.as_str()).await }.boxed()
                    }
                    _ => dummy.clone(),
                })
                .collect(),
            "HGETALL" => {
                if patterns.len() > 0 {
                    vec![async move {
                        self.completion_keys(&patterns[0], Some(RedisType::Hash))
                            .await
                    }
                    .boxed()]
                } else {
                    vec![]
                }
            }
            "HINCRBY" | "HINCRBYFLOAT" => {
                patterns
                    .iter()
                    .enumerate()
                    .map(|(index, p)| match index {
                        0 => {
                            async move { self.completion_keys(p, Some(RedisType::Hash)).await }
                                .boxed()

                            // suggest hash key
                        }
                        1 => {
                            let key = &patterns[0];

                            // hash key store in pattern[0]
                            async move { self.completion_hash_keys(key.as_str()).await }.boxed()
                        }
                        _ => dummy.clone(),
                    })
                    .collect()
            }
            "HKEYS" | "HLEN" => {
                if patterns.len() > 0 {
                    vec![async move {
                        self.completion_keys(&patterns[0], Some(RedisType::Hash))
                            .await
                    }
                    .boxed()]
                } else {
                    vec![]
                }
            }
            "HMGET" => {
                // get key first.
                let hash_keys: Arc<Mutex<Option<Vec<CompletionItem>>>> = Default::default();
                let hash_keys: Vec<_> = (0..patterns.len()).map(|_| hash_keys.clone()).collect();

                patterns
                    .iter()
                    .enumerate()
                    .zip(hash_keys.into_iter())
                    .map(|((index, p), hash_keys)| match index {
                        0 => {
                            async move { self.completion_keys(&p, Some(RedisType::Hash)).await }
                                .boxed()
                            // suggest hash key
                        }
                        _ => {
                            // hash key store in pattern[0]
                            async move {
                                let mut hash_keys = hash_keys.lock().await;
                                if hash_keys.is_none() {
                                    *hash_keys =
                                        Some(self.completion_hash_keys(&patterns[0]).await?);
                                }

                                anyhow::Ok(hash_keys.clone().unwrap_or(vec![]))
                                    as anyhow::Result<Vec<CompletionItem>>
                            }
                            .boxed()
                        }
                    })
                    .collect()
            }
            "HMSET" | "HSET" => {
                if patterns.len() > 0 {
                    let mut ret = vec![async move {
                        self.completion_keys(&patterns[0], Some(RedisType::Hash))
                            .await
                    }
                    .boxed()];
                    let hash_keys: Arc<Mutex<Option<Vec<CompletionItem>>>> = Default::default();
                    let hash_keys: Vec<_> =
                        (0..patterns.len() - 1).map(|_| hash_keys.clone()).collect();
                    let mut count = 0;
                    patterns.iter().skip(1).zip(hash_keys.into_iter()).for_each(
                        |(_p, hash_keys)| {
                            let x = if count % 2 == 0 {
                                async move {
                                    let mut hash_keys = hash_keys.lock().await;
                                    if hash_keys.is_none() {
                                        *hash_keys =
                                            Some(self.completion_hash_keys(&patterns[0]).await?);
                                    }

                                    anyhow::Ok(hash_keys.clone().unwrap_or(vec![]))
                                        as anyhow::Result<Vec<CompletionItem>>
                                }
                                .boxed()
                            } else {
                                dummy.clone()
                            };
                            ret.push(x);
                            count += 1;
                        },
                    );
                    ret
                } else {
                    vec![]
                }
            }
            "HSCAN" => {
                if patterns.len() > 0 {
                    let ret = vec![async move {
                        self.completion_keys(&patterns[0], Some(RedisType::Hash))
                            .await
                    }
                    .boxed()];
                    ret
                } else {
                    vec![]
                }
            }
            "HSETNX" => {
                if patterns.len() > 0 {
                    let ret = vec![async move {
                        self.completion_keys(&patterns[0], Some(RedisType::Hash))
                            .await
                    }
                    .boxed()];
                    ret
                } else {
                    vec![]
                }
            }
            "HSTRLEN" => {
                let mut ret = Vec::new();
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Hash))
                                .await
                        }
                        .boxed(),
                    );
                }
                if patterns.len() > 1 {
                    ret.push(async move { self.completion_hash_keys(&patterns[0]).await }.boxed());
                }
                ret
            }
            "HVALS" => {
                let mut ret = Vec::new();
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Hash))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "INCR" | "INCRBY" | "INCRBYFLOAT" => {
                let mut ret = Vec::new();
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::String))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "INFO" => {
                vec![async move {
                    anyhow::Ok(vec![
                        CompletionItem {
                            item: "SERVER".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "MEMORY".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "PERSISTENCE".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "STATS".into(),
                            kind: CompletionKind::Text,
                        },
                        //
                        CompletionItem {
                            item: "REPLICATION".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "CLIENTS".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "CPU".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "COMMANDSTATS".into(),
                            kind: CompletionKind::Text,
                        },
                        //
                        CompletionItem {
                            item: "CLUSTER".into(),
                            kind: CompletionKind::Text,
                        },
                        CompletionItem {
                            item: "KEYSPACE".into(),
                            kind: CompletionKind::Text,
                        },
                    ])
                }
                .boxed()]
            }
            "KEYS" => {
                vec![]
            }
            "LASTSAVE" => {
                vec![]
            }
            "LINDEX" | "LINSERT" | "LLEN" | "LPOP" | "LPUSH" | "LPUSHX" | "LRANGE" | "LREM"
            | "LSET" | "LTRIM" => {
                let mut ret = Vec::new();
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::List))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }

            "MGET" => patterns
                .iter()
                .map(|p| {
                    async move {
                        self.completion_keys(p.as_str(), Some(RedisType::String))
                            .await
                    }
                    .boxed()
                })
                .collect(),
            "MOVE" => {
                let mut ret = Vec::new();
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::List))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "MSET" => {
                let mut count = 0;
                patterns
                    .iter()
                    .map(|key| {
                        let x = if count % 2 == 0 {
                            async move { self.completion_keys(&key, Some(RedisType::String)).await }
                                .boxed()
                        } else {
                            dummy.clone()
                        };
                        count += 1;
                        x
                    })
                    .collect()
            }
            "MSETNX" => {
                //但它的特点是在所有指定的键都不存在的情况下，才会执行设置操作。
                // 所以不需要提醒数据库的key
                vec![]
            }
            "OBJECT" => {
                let mut ret = Vec::with_capacity(2);

                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            anyhow::Ok(vec![
                                CompletionItem {
                                    item: "ENCODING".into(),
                                    kind: CompletionKind::Text,
                                },
                                CompletionItem {
                                    item: "IDLETIME".into(),
                                    kind: CompletionKind::Text,
                                },
                                CompletionItem {
                                    item: "REFCOUNT".into(),
                                    kind: CompletionKind::Text,
                                },
                            ])
                        }
                        .boxed(),
                    );
                }
                if patterns.len() > 1 {
                    ret.push(async move { self.completion_keys(&patterns[1], None).await }.boxed());
                }
                ret
            }
            "PERSIST" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PEXPIRE" | "PEXPIREAT" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PFADD" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PFCOUNT" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PFMERGE" => {
                let mut ret = Vec::with_capacity(2);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                if patterns.len() > 1 {
                    ret.push(async move { self.completion_keys(&patterns[1], None).await }.boxed());
                }

                ret
            }
            "PING" => {
                //
                vec![]
            }
            "PSETEX" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PTTL" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "PUBLISH" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "RANDOMKEY" => {
                //
                vec![]
            }
            "READONLY" => {
                //
                vec![]
            }
            "READWRITE" => {
                vec![]
            }
            "RENAME" | "RENAMENX" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "RESTORE" | "RESTORE_ASKING" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "RPOP" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::List))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }

            "RPOPLPUSH" => {
                let mut ret = Vec::with_capacity(2);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::List))
                                .await
                        }
                        .boxed(),
                    );
                }
                if patterns.len() > 1 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[1], Some(RedisType::List))
                                .await
                        }
                        .boxed(),
                    );
                }

                ret
            }

            "RPUSH" | "RPUSHX" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::List))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SADD" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SCARD" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SCRIPT" => {
                let mut ret = Vec::with_capacity(2);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            anyhow::Ok(vec![
                                CompletionItem {
                                    item: "LOAD".into(),
                                    kind: CompletionKind::Text,
                                },
                                CompletionItem {
                                    item: "EXISTS".into(),
                                    kind: CompletionKind::Text,
                                },
                                CompletionItem {
                                    item: "FLUSH".into(),
                                    kind: CompletionKind::Text,
                                },
                                CompletionItem {
                                    item: "KILL".into(),
                                    kind: CompletionKind::Text,
                                },
                                CompletionItem {
                                    item: "DEBUG".into(),
                                    kind: CompletionKind::Text,
                                },
                            ])
                        }
                        .boxed(),
                    );
                }

                ret
            }
            "SDIFF" => patterns
                .iter()
                .map(|p| {
                    async move { self.completion_keys(&p, Some(RedisType::Set)).await }.boxed()
                })
                .collect(),
            "SDIFFSTORE" => patterns
                .iter()
                .map(|p| {
                    async move { self.completion_keys(&p, Some(RedisType::Set)).await }.boxed()
                })
                .collect(),

            "SETBIT" | "SETEX" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::String))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SETNX" => {
                //
                vec![]
            }
            "SETRANGE" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::String))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SINTER" => patterns
                .iter()
                .map(|p| {
                    async move { self.completion_keys(&p, Some(RedisType::Set)).await }.boxed()
                })
                .collect(),
            "SINTERSTORE" => patterns
                .iter()
                .map(|p| {
                    async move { self.completion_keys(&p, Some(RedisType::Set)).await }.boxed()
                })
                .collect(),
            "SISMEMBER" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SLOWLOG" => {
                let mut ret = Vec::with_capacity(2);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            anyhow::Ok(vec![
                                CompletionItem {
                                    item: "GET".into(),
                                    kind: CompletionKind::Text,
                                },
                                CompletionItem {
                                    item: "LEN".into(),
                                    kind: CompletionKind::Text,
                                },
                                CompletionItem {
                                    item: "RESET".into(),
                                    kind: CompletionKind::Text,
                                },
                            ])
                        }
                        .boxed(),
                    );
                }

                ret
            }
            "SMEMBERS" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SMOVE" => {
                let mut ret = Vec::with_capacity(2);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }
                if patterns.len() > 1 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[1], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }

                ret
            }
            "SORT" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "SPOP" | "SRANDMEMBER" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SREM" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::Set))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SSCAN" => {
                // TODO
                vec![]
            }
            "STRLEN" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(
                        async move {
                            self.completion_keys(&patterns[0], Some(RedisType::String))
                                .await
                        }
                        .boxed(),
                    );
                }
                ret
            }
            "SUNION" => patterns
                .iter()
                .map(|p| {
                    async move { self.completion_keys(&p, Some(RedisType::Set)).await }.boxed()
                })
                .collect(),
            "SUNIONSTORE" => patterns
                .iter()
                .map(|p| {
                    async move { self.completion_keys(&p, Some(RedisType::Set)).await }.boxed()
                })
                .collect(),
            "TOUCH" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(&p, None).await }.boxed())
                .collect(),
            "TTL" | "TYPE" => {
                let mut ret = Vec::with_capacity(1);
                if patterns.len() > 0 {
                    ret.push(async move { self.completion_keys(&patterns[0], None).await }.boxed());
                }
                ret
            }
            "UNLINK" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(&p, None).await }.boxed())
                .collect(),
            "WATCH" => patterns
                .iter()
                .map(|p| async move { self.completion_keys(&p, None).await }.boxed())
                .collect(),

            _ => {
                vec![]
            }
        };
        anyhow::Ok(fns)
    }
}

#[derive(Debug)]
pub struct RedisDialet {}

impl sqlparser::dialect::Dialect for RedisDialet {
    /// Determine if a character is a valid start character for an unquoted identifier
    fn is_identifier_start(&self, ch: char) -> bool {
        ch.is_alphabetic() || ch == ':' || ch == '$' || ch == '/' || ch.is_numeric() || ch == '.'
    }

    /// Determine if a character is a valid unquoted identifier character
    fn is_identifier_part(&self, ch: char) -> bool {
        self.is_identifier_start(ch)
    }
}

fn static_redis_command_completion_items() -> Vec<CompletionItem> {
    static ITEMS: OnceLock<Vec<CompletionItem>> = OnceLock::new();
    ITEMS
        .get_or_init(|| {
            REDIS_COMMANDS
                .iter()
                .map(|x| CompletionItem {
                    item: (*x).into(),
                    kind: CompletionKind::KeyWord,
                })
                .collect::<Vec<_>>()
        })
        .clone()
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
    "CLIENT",
    "CLUSTER",
    "CONFIG",
    "DBSIZE",
    "DEBUG",
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
    "PTTL",
    "PUBLISH",
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
    "SCRIPT",
    "SDIFF",
    "SDIFFSTORE",
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
    "SLOWLOG",
    "SMEMBERS",
    "SMOVE",
    "SORT",
    "SPOP",
    "SRANDMEMBER",
    "SREM",
    "SSCAN",
    "STRLEN",
    "SUNION",
    "SUNIONSTORE",
    "SYNC",
    "TIME",
    "TOUCH",
    "TTL",
    "TYPE",
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
