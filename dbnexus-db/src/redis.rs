use std::collections::HashSet;
use std::ops::DerefMut;
use std::str::FromStr;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use std::vec;

use dashmap::{DashMap, DashSet};
use futures_util::StreamExt;
use redis::aio::{ConnectionManager, ConnectionManagerConfig, MultiplexedConnection};
use redis::{AsyncCommands, AsyncIter, Commands, Value};
use tokio::sync::Mutex;

use crate::datasource_config::RedisDBConfig;
use crate::db::{
    create_html_table_data, create_markdown_data, create_markdown_header, AutoCompletionQueryId,
    BackendKind, CheckConn, CreateSchemaExtra, DBTrait, EncodeXlsxDatetime, ExportFrame,
    ExportKind, ExportQueryResult, ExtractSqlValeFromJsonValue, Field, GetBackendKind,
    GetTestExportTableKinds, ListTable, MetaValue, QueryEvent, QueryEventEmitter, QueryResp,
    QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas, ShowTableParameter, SqlBuilder, XlsxEdit,
};
use crate::helper::{create_csv, OffsetLimitIter};

impl ExtractSqlValeFromJsonValue for RedisDB {
    fn extrat_sql_value_from_json_value(
        &self,
        _value: &serde_json::Value,
        _ty: &str,
    ) -> anyhow::Result<Option<String>> {
        anyhow::bail!("redis don't need impl this")
    }
}
#[derive(Clone)]
pub struct RedisDB {
    config: RedisDBConfig,
    conn_manager: ConnectionManager,
    pub(crate) query_connection: Arc<DashMap<String, Arc<Mutex<MultiplexedConnection>>>>,
    created_database: Arc<DashSet<String>>,
    pub(crate) completion_id: AutoCompletionQueryId,
}

impl RedisDB {
    pub async fn new(config: RedisDBConfig) -> anyhow::Result<Self> {
        let conn_str = config.make_conn_string();
        log::info!("connect to redis var:{:?}", conn_str);
        let client = redis::Client::open(conn_str.as_str())?;
        let mut conn_config = ConnectionManagerConfig::default();
        conn_config = conn_config.set_connection_timeout(config.conn_timeout.map(Duration::from));
        let m = redis::aio::ConnectionManager::new_with_config(client, conn_config).await?;
        anyhow::Ok(Self {
            config,
            conn_manager: m,
            query_connection: Default::default(),
            created_database: Default::default(),
            completion_id: Default::default(),
        })
    }
}

impl std::fmt::Debug for RedisDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "redis on:{:?}", self.config)
    }
}

#[derive(Clone, Debug, serde::Serialize, Default, specta::Type)]
pub struct RedisListTableRet {
    string_tables: Vec<ListTable>,
    list_tables: Vec<ListTable>,
    set_tables: Vec<ListTable>,
    hash_tables: Vec<ListTable>,
    zset_tables: Vec<ListTable>,
    has_more: bool,
}

impl RedisListTableRet {
    pub(crate) fn sort(&mut self) {
        self.string_tables.sort();
        self.list_tables.sort();
        self.set_tables.sort();
        self.hash_tables.sort();
        self.zset_tables.sort();
    }
}

impl RedisDB {
    pub async fn list_lpush(&self, key: &str, value: &[u8]) -> anyhow::Result<()> {
        self.can_write()?;

        let mut conn = self.get_conn().await?;
        let _: () = conn.lpush(key, value).await?;
        anyhow::Ok(())
    }

    pub async fn list_rpush(&self, key: &str, value: &[u8]) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let _: () = conn.rpush(key, value).await?;
        anyhow::Ok(())
    }

    pub async fn list_remove(&self, key: &str, count: isize, value: &[u8]) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let _: () = conn.lrem(key, count, value).await?;
        anyhow::Ok(())
    }

    pub async fn list_set(&self, key: &str, index: isize, value: &[u8]) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let _: () = conn.lset(key, index, value).await?;
        anyhow::Ok(())
    }

    pub async fn set_replace(
        &self,
        key: &str,
        old_member: &[u8],
        new_member: &[u8],
    ) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let _: () = conn.srem(key, old_member).await?;
        let _: () = conn.sadd(key, new_member).await?;
        anyhow::Ok(())
    }

    pub async fn set_del(&self, key: &str, member: &[u8]) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let _: () = conn.srem(key, member).await?;
        anyhow::Ok(())
    }

    pub async fn set_add(&self, key: &str, member: &[u8]) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let _: () = conn.sadd(key, member).await?;
        anyhow::Ok(())
    }

    pub async fn hash_replace(
        &self,
        key: &str,
        old_member: &[u8],
        new_member: &[u8],
        new_value: &[u8],
    ) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let _: () = conn.hdel(key, old_member).await?;
        let _: () = conn.hset(key, new_member, new_value).await?;
        anyhow::Ok(())
    }

    pub async fn hash_del(&self, key: &str, member: &[u8]) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let _: () = conn.hdel(key, member).await?;
        anyhow::Ok(())
    }

    pub async fn hash_add(&self, key: &str, member: &[u8], value: &[u8]) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let _: () = conn.hset(key, member, value).await?;
        anyhow::Ok(())
    }

    pub async fn zset_replace(
        &self,
        key: &str,
        old_member: &[u8],
        new_member: &[u8],
        new_value: f64,
    ) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let _: () = conn.zrem(key, old_member).await?;
        let _: () = conn.zadd(key, new_member, new_value).await?;

        anyhow::Ok(())
    }

    pub async fn zset_del(&self, key: &str, member: &[u8]) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let _: () = conn.zrem(key, member).await?;
        anyhow::Ok(())
    }

    pub async fn zset_add(&self, key: &str, member: &[u8], value: f64) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let _: () = conn.zadd(key, member, value).await?;
        anyhow::Ok(())
    }

    pub async fn string_set(&self, key: &str, value: &[u8]) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let _: () = conn.set(key, value).await?;
        anyhow::Result::Ok(())
    }
}

#[async_trait::async_trait]
impl ServerMetas for RedisDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let mut ret = Vec::new();
        let mut conn = self.get_conn().await?;
        let info: String = redis::cmd("info").query_async(&mut conn).await?;
        for m in info.lines() {
            if m.is_empty() {
                continue;
            }
            if m.starts_with("#") {
                // this comments.
                continue;
            }
            let (name, value) = split_info_colon(m)?;
            ret.push(MetaValue::new(name, value));
        }
        anyhow::Ok(ret)
    }
}

fn split_info_colon(info: &str) -> anyhow::Result<(String, String)> {
    match info.find(":") {
        Some(index) => anyhow::Ok((info[..index].to_string(), info[index + 1..].to_string())),
        None => anyhow::bail!("info should contains ':'"),
    }
}

fn escape_string(s: &str) -> anyhow::Result<String> {
    let r = shlex::try_quote(s)?;
    anyhow::Ok(r.to_string())
}

fn check_resp<T>(value: Vec<Result<T, redis::RedisError>>) -> Result<Vec<T>, redis::RedisError> {
    let mut kv = vec![];
    for v in value {
        let v = v?;
        kv.push(v);
    }
    Ok(kv)
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

impl GetBackendKind for RedisDB {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Redis
    }
}

#[async_trait::async_trait]
impl ExportQueryResult for RedisDB {
    async fn export_query_result(
        &self,
        query_id: Option<&str>,
        sql: &str,
        kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let commands = get_command_from_sql(sql)?;
        if commands.len() != 1 {
            anyhow::bail!("Redis does not support multiple commands");
        }
        let c = &commands[0];
        log::info!("cmd is:{:?}", c);
        let conn = match query_id {
            Some(x) => match self.query_connection.get(&x.to_string()) {
                Some(x) => x.clone(),
                None => anyhow::bail!("should be a valid query id"),
            },
            None => {
                let x = self
                    .get_client()
                    .await?
                    .get_multiplexed_async_connection()
                    .await?;
                Arc::new(Mutex::new(x))
            }
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let args = &c[1..];
        let mut redis_cmd = redis::cmd(c[0].as_str());
        redis_cmd.arg(args);
        // should all be captiable with iter_async `TYPE string_value` will return a error
        let mut stream = redis_cmd.iter_async::<String>(conn).await?;
        let mut row = 0;
        while let Some(value) = stream.next().await {
            let value = value?;
            match kind {
                ExportKind::Csv { delimiter, .. } => {
                    sender
                        .send(create_csv(&[Some(value)], delimiter)?.into())
                        .await?;
                }
                ExportKind::Json { .. } | ExportKind::JsonLine => {
                    sender.send(serde_json::Value::from(value).into()).await?;
                }
                ExportKind::Sql { .. } => {
                    anyhow::bail!("can't export query result to sql")
                }
                ExportKind::Xlsx { .. } => {
                    sender
                        .send(
                            XlsxEdit {
                                row,
                                column: 0,
                                content: value.into(),
                            }
                            .into(),
                        )
                        .await?;
                    row += 1;
                }
                ExportKind::Html { .. } => {
                    sender
                        .send(create_html_table_data(&[vec![Some(value)]]).into())
                        .await?;
                }
                ExportKind::MarkDown => {
                    sender
                        .send(create_markdown_data(&[Some(value)]).into())
                        .await?;
                }
            }
        }
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }
}

impl SqlBuilder for RedisDB {
    fn get_schema(&self) -> Option<&str> {
        None
    }

    fn put_value_in_sql(&self, _typ: &str, _value: &str) -> anyhow::Result<String> {
        anyhow::bail!("redis can't put value in sql")
    }

    fn gen_show_sql(
        &self,
        _table_name: &str,
        _fields: Option<&[Field]>,
        _para: Option<&ShowTableParameter>,
    ) -> String {
        "".to_string()
    }
}

impl EncodeXlsxDatetime for RedisDB {}

#[async_trait::async_trait]
impl SchemaTrait for RedisDB {
    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }

    async fn table_exists(&self, table: &str) -> anyhow::Result<bool> {
        let mut conn = self.get_conn().await?;
        let e = conn.exists(table).await?;
        anyhow::Ok(e)
    }

    async fn duplicate_table(&self, key: &str, dup: &str) -> anyhow::Result<()> {
        if self.table_exists(dup).await? {
            anyhow::bail!(format!("key '{}' already exists", dup));
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
                    let list: Vec<Vec<u8>> = conn.lrange(key, x, x + size).await?;
                    let _: () = conn_set.rpush(dup, list).await?;
                }
            }
            RedisType::Set => {
                let mut iter = conn.sscan::<_, Vec<u8>>(key).await?;
                while let Some(set) = iter.next().await {
                    let set = set?;
                    let _: () = conn_set.sadd(dup, set).await?;
                }
            }

            RedisType::Hash => {
                let mut stream: AsyncIter<'_, Vec<u8>> = conn.hscan(key).await?;
                let mut key = None as Option<Vec<u8>>;
                while let Some(value) = stream.next().await {
                    let value = value?;
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
                let mut stream: AsyncIter<'_, (Vec<u8>, f64)> = conn.zscan(key).await?;

                while let Some(value) = stream.next().await {
                    let (value, score) = value?;
                    let _: () = conn_set.zadd(dup, value, score).await?;
                }
            }
        }
        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<Option<String>> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.rename(old, new).await?;
        anyhow::Ok(None)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    async fn export_table_or_view(
        &self,
        key: &str,
        para: Option<ShowTableParameter>,
        kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let key_match = match &para {
            Some(x) => match x.filter.as_ref() {
                Some(x) => Some(x.as_str()),
                None => None,
            },
            None => None,
        };

        match &kind {
            ExportKind::Json { .. } => {}
            ExportKind::JsonLine => {}
            ExportKind::Sql {
                include_table_schema,
            } => {
                if *include_table_schema {
                    anyhow::bail!("redis have no table schema")
                }
            }
            ExportKind::Xlsx { include_header, .. }
            | ExportKind::Html { include_header }
            | ExportKind::Csv { include_header, .. } => {
                if *include_header {
                    anyhow::bail!("redis have no header")
                }
            }
            ExportKind::MarkDown => {}
        }
        let key_match = key_match.unwrap_or("*");
        let mut row_num = 0;
        let mut conn = self.get_conn().await?;
        let ty = Self::get_key_type(&mut conn, key).await?;

        match ty {
            RedisType::String => {
                let value: String = conn.get(key).await?;
                match &kind {
                    ExportKind::Csv { delimiter, .. } => {
                        sender
                            .send(create_csv(&[Some(value)], *delimiter)?.into())
                            .await?;
                    }
                    ExportKind::Json { .. } | ExportKind::JsonLine => {
                        let mut m = serde_json::Map::new();
                        m.insert(key.to_string(), serde_json::Value::from(value));
                        sender.send(m.into()).await?;
                    }
                    ExportKind::Sql { .. } => {
                        sender
                            .send(format!("SET {} '{}'\n", key, escape_string(&value)?).into())
                            .await?;
                    }
                    ExportKind::Xlsx { .. } => {
                        sender
                            .send(
                                XlsxEdit {
                                    row: 0,
                                    column: 0,
                                    content: value.into(),
                                }
                                .into(),
                            )
                            .await?
                    }
                    ExportKind::Html { .. } => {
                        sender
                            .send(create_html_table_data(&[vec![Some(value)]]).into())
                            .await?
                    }
                    ExportKind::MarkDown => {
                        sender
                            .send(create_markdown_header(&["value".to_string()]).into())
                            .await?;

                        let value = Some(value);
                        sender.send(create_markdown_data(&[value]).into()).await?
                    }
                }
            }

            RedisType::List => {
                let total: isize = conn.llen(key).await?;
                let size: isize = 100;
                if matches!(&kind, ExportKind::MarkDown) {
                    sender
                        .send(create_markdown_header(&["value".to_string()]).into())
                        .await?;
                }
                let iter = OffsetLimitIter::new(total, 100);
                for x in iter {
                    let list: Vec<String> = conn.lrange(key, x, x + size).await?;
                    for value in list {
                        match &kind {
                            ExportKind::Csv { delimiter, .. } => {
                                sender
                                    .send(create_csv(&[Some(value)], *delimiter)?.into())
                                    .await?
                            }
                            ExportKind::Json { .. } | ExportKind::JsonLine => {
                                sender.send(serde_json::Value::from(value).into()).await?;
                            }

                            ExportKind::Sql { .. } => {
                                sender
                                    .send(
                                        format!(
                                            "RPUSH {} '{}'\n",
                                            key,
                                            escape_string(value.as_str())?
                                        )
                                        .into(),
                                    )
                                    .await?;
                            }
                            ExportKind::Xlsx { .. } => {
                                sender
                                    .send(
                                        XlsxEdit {
                                            row: row_num,
                                            column: 0,
                                            content: value.into(),
                                        }
                                        .into(),
                                    )
                                    .await?;
                                row_num += 1;
                            }
                            ExportKind::Html { .. } => {
                                sender
                                    .send(create_html_table_data(&[vec![Some(value)]]).into())
                                    .await?
                            }
                            ExportKind::MarkDown => {
                                sender
                                    .send(create_markdown_data(&[Some(value)]).into())
                                    .await?;
                            }
                        }
                    }
                }
            }

            RedisType::Set => {
                let mut iter: AsyncIter<'_, String> = conn.sscan_match(key, key_match).await?;

                if matches!(&kind, ExportKind::MarkDown) {
                    sender
                        .send(create_markdown_header(&["value".to_string()]).into())
                        .await?;
                }
                while let Some(set) = iter.next().await {
                    let value = set?;
                    match &kind {
                        ExportKind::Csv { delimiter, .. } => {
                            sender
                                .send(create_csv(&[Some(value)], *delimiter)?.into())
                                .await?
                        }
                        ExportKind::Json { .. } | ExportKind::JsonLine => {
                            let value = serde_json::Value::from(value);
                            sender.send(value.into()).await?;
                        }
                        ExportKind::Sql { .. } => {
                            sender
                                .send(
                                    format!("SADD {} '{}'\n", key, escape_string(value.as_str())?)
                                        .into(),
                                )
                                .await?;
                        }
                        ExportKind::Xlsx { .. } => {
                            sender
                                .send(
                                    XlsxEdit {
                                        row: row_num,
                                        column: 0,
                                        content: value.into(),
                                    }
                                    .into(),
                                )
                                .await?;
                            row_num += 1;
                        }
                        ExportKind::Html { .. } => {
                            sender
                                .send(create_html_table_data(&[vec![Some(value)]]).into())
                                .await?
                        }
                        ExportKind::MarkDown => {
                            sender
                                .send(create_markdown_data(&[Some(value)]).into())
                                .await?
                        }
                    }
                }
            }

            RedisType::Hash => {
                let mut stream: futures_util::stream::Chunks<AsyncIter<'_, String>> =
                    conn.hscan_match(key, key_match).await?.chunks(2);
                if matches!(&kind, ExportKind::MarkDown) {
                    sender
                        .send(
                            create_markdown_header(&["key".to_string(), "value".to_string()])
                                .into(),
                        )
                        .await?;
                }
                while let Some(value) = stream.next().await {
                    let kv = check_resp(value)?;
                    match &kind {
                        ExportKind::Csv { delimiter, .. } => {
                            sender
                                .send(
                                    create_csv(
                                        &[Some(kv[0].clone()), Some(kv[1].clone())],
                                        *delimiter,
                                    )?
                                    .into(),
                                )
                                .await?
                        }
                        ExportKind::Json { .. } | ExportKind::JsonLine => {
                            let mut m = serde_json::Map::new();
                            let key = kv[0].clone();
                            let value = serde_json::Value::from(kv[1].clone());
                            m.insert(key, value);
                            sender.send(m.into()).await?;
                        }

                        ExportKind::Sql { .. } => {
                            sender
                                .send(
                                    format!(
                                        "HMSET {} {} '{}'\n",
                                        key,
                                        kv[0],
                                        escape_string(kv[1].as_str())?
                                    )
                                    .into(),
                                )
                                .await?;
                        }
                        ExportKind::Xlsx { .. } => {
                            sender
                                .send(
                                    XlsxEdit {
                                        row: row_num,
                                        column: 0,
                                        content: kv[0].clone().into(),
                                    }
                                    .into(),
                                )
                                .await?;
                            sender
                                .send(
                                    XlsxEdit {
                                        row: row_num,
                                        column: 1,
                                        content: kv[1].clone().into(),
                                    }
                                    .into(),
                                )
                                .await?;
                            row_num += 1;
                        }
                        ExportKind::Html { .. } => {
                            sender
                                .send(
                                    create_html_table_data(&[vec![
                                        Some(kv[0].clone()),
                                        Some(kv[1].clone()),
                                    ]])
                                    .into(),
                                )
                                .await?
                        }
                        ExportKind::MarkDown => {
                            sender
                                .send(
                                    create_markdown_data(&[
                                        Some(kv[0].clone()),
                                        Some(kv[1].clone()),
                                    ])
                                    .into(),
                                )
                                .await?
                        }
                    }
                }
            }
            RedisType::Zset => {
                let mut stream: futures_util::stream::Chunks<AsyncIter<'_, String>> =
                    conn.zscan_match(key, key_match).await?.chunks(2);
                if matches!(&kind, ExportKind::MarkDown) {
                    sender
                        .send(
                            create_markdown_header(&["value".to_string(), "score".to_string()])
                                .into(),
                        )
                        .await?;
                }
                while let Some(value) = stream.next().await {
                    let kv = check_resp(value)?;
                    match &kind {
                        ExportKind::Csv { delimiter, .. } => {
                            sender
                                .send(
                                    create_csv(
                                        &[Some(kv[0].clone()), Some(kv[1].clone())],
                                        *delimiter,
                                    )?
                                    .into(),
                                )
                                .await?
                        }
                        ExportKind::Json { .. } | ExportKind::JsonLine => {
                            let mut m = serde_json::Map::new();

                            let key = kv[0].clone();
                            let value = serde_json::Value::from(kv[1].clone());
                            m.insert(key, value);

                            sender.send(m.into()).await?;
                        }

                        ExportKind::Sql { .. } => {
                            // ZADD myzset 1 "uno"
                            sender
                                .send(
                                    format!(
                                        "ZADD {} {} '{}'\n",
                                        key,
                                        kv[1],
                                        escape_string(kv[0].as_str())?
                                    )
                                    .into(),
                                )
                                .await?;
                        }
                        ExportKind::Xlsx { .. } => {
                            sender
                                .send(
                                    XlsxEdit {
                                        row: row_num,
                                        column: 0,
                                        content: kv[0].clone().into(),
                                    }
                                    .into(),
                                )
                                .await?;
                            sender
                                .send(
                                    XlsxEdit {
                                        row: row_num,
                                        column: 1,
                                        content: kv[1].clone().into(),
                                    }
                                    .into(),
                                )
                                .await?;
                            row_num += 1;
                        }
                        ExportKind::Html { .. } => {
                            sender
                                .send(
                                    create_html_table_data(&[vec![
                                        Some(kv[0].clone()),
                                        Some(kv[1].clone()),
                                    ]])
                                    .into(),
                                )
                                .await?
                        }
                        ExportKind::MarkDown => {
                            sender
                                .send(
                                    create_markdown_data(&[
                                        Some(kv[0].clone()),
                                        Some(kv[1].clone()),
                                    ])
                                    .into(),
                                )
                                .await?
                        }
                    }
                }
            }
        }
        anyhow::Ok(())
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        anyhow::bail!("use redis list table")
    }

    async fn show_table_columns(&self, key: &str) -> anyhow::Result<Vec<Field>> {
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
        &self,
        key: &str,
        para: ShowTableParameter,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        emit.send("trying to get connetion.".into())?;
        let mut conn = self.get_conn().await?;
        let ShowTableParameter {
            filter, min, max, ..
        } = &para;
        let typ = Self::get_key_type(&mut conn, key).await?;
        use futures_util::StreamExt;
        match typ {
            RedisType::String => {
                let ret: Option<Vec<u8>> = conn.get(key).await?;
                emit.send(
                    QueryResp::result(
                        format!("GET {}", key),
                        vec![ret.map(|x| hex::encode(x.as_slice()))],
                    )
                    .into(),
                )?;
                anyhow::Ok(())
            }
            RedisType::List => {
                emit.send("trying to get list.".into())?;
                let r: Vec<Vec<u8>> = conn.lrange(key, 0, -1).await?;
                emit.send(
                    QueryResp {
                        fields: vec![
                            Field::from_name_type("seq", "int"),
                            Field::from_name_type("value", "string"),
                        ],
                        total: r.len() as f64,
                        data: r
                            .into_iter()
                            .enumerate()
                            .map(|(seq, r)| {
                                vec![Some(seq.to_string()), Some(hex::encode(r.as_slice()))]
                            })
                            .collect(),
                        ..Default::default()
                    }
                    .into(),
                )?;
                anyhow::Ok(())
            }
            RedisType::Set => {
                emit.send("trying to query amount".into())?;
                emit.send(
                    QueryResp {
                        fields: vec![Field::from_name_type("value", "string")],
                        ..Default::default()
                    }
                    .into(),
                )?;
                let mut ret: AsyncIter<Vec<u8>> = conn
                    .sscan_match(key, filter.clone().unwrap_or("*".to_string()))
                    .await?;
                while let Some(value) = ret.next().await {
                    let value = value?;
                    let d = vec![Some(hex::encode(&value))];
                    emit.send(d.into())?;
                }
                anyhow::Ok(())
            }
            RedisType::Hash => {
                emit.send(
                    QueryResp {
                        fields: vec![
                            Field::from_name_type("key", "string"),
                            Field::from_name_type("value", "string"),
                        ],
                        ..Default::default()
                    }
                    .into(),
                )?;
                let mut ret = conn
                    .hscan_match::<_, _, Vec<u8>>(key, filter.clone().unwrap_or("*".to_string()))
                    .await?
                    .chunks(2);
                while let Some(row) = ret.next().await {
                    let mut values = Vec::with_capacity(2);
                    for r in row {
                        let r = r?;
                        values.push(r);
                    }
                    emit.send(
                        vec![Some(hex::encode(&values[0])), Some(hex::encode(&values[1]))].into(),
                    )?;
                }

                anyhow::Ok(())
            }
            RedisType::Zset => {
                let mut conn = self.get_conn().await?;
                let total: usize = conn.zcard(key).await?;
                let mut ret: AsyncIter<(Vec<u8>, f64)> = conn
                    .zscan_match(key, filter.as_deref().unwrap_or("*"))
                    .await?;
                emit.send(
                    QueryResp {
                        fields: vec![
                            Field::from_name_type("score", "f64"),
                            Field::from_name_type("key", "string"),
                        ],
                        total: total as f64,
                        ..Default::default()
                    }
                    .into(),
                )?;
                while let Some(item) = ret.next_item().await {
                    let (key, score) = item?;
                    // try match
                    if let Some(min) = min {
                        if score < *min {
                            continue;
                        }
                    }
                    if let Some(max) = max {
                        if score > *max {
                            continue;
                        }
                    }
                    emit.send(vec![Some(score.to_string()), Some(hex::encode(&key))].into())?;
                }
                anyhow::Ok(())
            }
        }
    }

    async fn truncate_table(&self, key: &str) -> anyhow::Result<Option<String>> {
        let mut conn = self.get_conn().await?;
        let typ = Self::get_key_type(&mut conn, key).await?;
        match typ {
            RedisType::String => {
                let _: () = conn.set(key, "").await?;
            }
            _ => {
                anyhow::bail!(
                    "redis collection type delete all element will cause the key been dropped"
                )
            }
        }
        anyhow::Ok(None)
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let mut conn: ConnectionManager = self.get_conn().await?;
        let _: () = conn.del(table).await?;
        anyhow::Ok(None)
    }
}

impl RedisDB {
    pub async fn redis_list_key(
        &self,
        filter: Option<&str>,
        max: usize,
    ) -> anyhow::Result<RedisListTableRet> {
        let mut conn = self.get_conn().await?;
        let mut stream: AsyncIter<'_, String> = conn.scan_match(filter.unwrap_or("*")).await?;
        let mut count: usize = 0;
        let mut has_more = false;
        let mut tables_no_table = vec![];

        let mut ret = RedisListTableRet::default();

        while let Some(table) = stream.next_item().await {
            let table = table?;
            tables_no_table.push(table);
            if count >= max {
                has_more = true;
                break;
            }
            count += 1;
        }
        drop(stream);
        for table in tables_no_table {
            let typ = Self::get_key_type(&mut conn, &table).await?;

            let size = match typ {
                RedisType::String => None,
                RedisType::List => {
                    let size: u64 = conn.llen(&table).await?;
                    Some(size)
                }
                RedisType::Set => {
                    let size: u64 = conn.scard(&table).await?;
                    Some(size)
                }
                RedisType::Hash => {
                    let size: u64 = conn.hlen(&table).await?;
                    Some(size)
                }
                RedisType::Zset => {
                    let size: u64 = conn.zcard(&table).await?;
                    Some(size)
                }
            };
            let size = size.map(|x| x as f64);
            match typ {
                RedisType::String => ret.string_tables.push(ListTable::new(table, size)),
                RedisType::List => ret.list_tables.push(ListTable::new(table, size)),
                RedisType::Set => ret.set_tables.push(ListTable::new(table, size)),
                RedisType::Hash => ret.hash_tables.push(ListTable::new(table, size)),
                RedisType::Zset => ret.zset_tables.push(ListTable::new(table, size)),
            }
        }
        ret.has_more = has_more;
        ret.sort();

        anyhow::Ok(ret)
    }
}
fn parse_db_number(db: &str) -> anyhow::Result<i32> {
    db[2..].parse::<i32>().map_err(|err| {
        anyhow::Error::msg(format!(
            "can't parse redis db number,err:{:?} db:{}",
            err, db
        ))
    })
}

#[async_trait::async_trait]
impl DBTrait for RedisDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let r: String = redis::cmd("info")
            .arg("keyspace")
            .query_async(&mut conn)
            .await?;

        let mut db_has_keys = extract_db_from_info_key_space(&r)?
            .into_iter()
            .collect::<HashSet<_>>();
        for r in self.created_database.clone().iter() {
            db_has_keys.insert(r.as_str().to_owned());
        }

        let mut ret = db_has_keys.into_iter().collect::<Vec<_>>();

        ret.sort_by(|x, y| {
            let index_x = parse_db_number(x)
                .map_err(|x| log::error!("parse db number failed,err:{}", x))
                .unwrap_or_default();
            let index_y = parse_db_number(y)
                .map_err(|x| log::error!("parse db number failed,err:{}", x))
                .unwrap_or_default();
            index_x.cmp(&index_y)
        });
        anyhow::Ok(ret)
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut config = self.config.clone();
        config.db = Some(name.to_owned());
        let db = Self::new(config).await?;
        anyhow::Ok(Arc::new(db))
    }

    async fn create_schema(
        &self,
        name: &str,
        _extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        // In Redis, the concept of a "database" is somewhat different from traditional
        // relational databases. Redis uses a single instance with multiple logical
        // databases, each identified by an index number (default is 0-15 for most
        // installations). To switch between these databases, you typically use the
        // SELECT command.

        // so we don't create database send a request to redis server we just save it
        // memory. but there is a problem if user don't create any data
        // when restart we will not have the create database
        // I think this is rare.
        self.can_write()?;
        self.created_database.insert(name.to_owned());
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let mut config = self.config.clone();
        config.db = Some(name.to_owned());
        let db = Self::new(config).await?;
        db.flush_db().await?;
        self.created_database.remove(&name.to_owned());
        anyhow::Ok(())
    }
}

impl RedisDB {
    async fn flush_db(&self) -> anyhow::Result<()> {
        let mut conn = self.get_client().await?;
        let _: () = conn.flushdb()?;
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl QuerySql for RedisDB {
    fn clone(&self) -> Box<dyn QuerySql> {
        Box::new(Clone::clone(self))
    }
    async fn get_update_sql_from_result_set(
        &self,
        _sql: &str,
        _old_values: &[Option<String>],
        _new_values: &[Option<String>],
    ) -> anyhow::Result<Vec<String>> {
        self.backend_kind()
            .not_support_feature("get update sql from result set")
    }
    fn support_tx(&self) -> bool {
        false
    }
    /// redis have no transaction any connection is fine
    /// so connection management for redis is dummy
    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        let client: redis::Client = self.get_client().await?;
        let conn = client.get_multiplexed_async_connection().await?;
        self.query_connection
            .insert(id.to_owned(), Arc::new(Mutex::new(conn)));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        // no need execute close connection staff
        self.query_connection.remove(&id.to_string());
        anyhow::Ok(())
    }
    async fn simple_execute_sql(&self, _sql: &str) -> anyhow::Result<Option<usize>> {
        anyhow::bail!("the frontend will not use the api for now.")
    }
    async fn execute_sql(
        &self,
        query_id: &str,
        sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let conn = match self.query_connection.get(&query_id.to_string()) {
            Some(x) => x.clone(),
            None => {
                anyhow::bail!(format!("can't find connection for id '{}'", query_id))
            }
        };
        let mut lock = conn.lock().await;
        let commands = get_command_from_sql(sql)?;
        let mut ret = vec![];
        for c in commands {
            let r = redis::cmd(&c[0])
                .arg(&c[1..])
                .exec_async(lock.deref_mut())
                .await;

            ret.push(Some(match r {
                Ok(_) => "execute ok".to_string(),
                Err(err) => format!("execute failed,err:{}", err),
            }));
        }
        anyhow::Ok(Ok(QueryResp::result(None, ret)))
    }

    async fn query_sql(
        &self,
        query_id: &str,
        sql: &str,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let conn = match self.query_connection.get(&query_id.to_string()) {
            Some(x) => x.clone(),
            None => {
                anyhow::bail!(format!("can't find connection for id '{}'", query_id))
            }
        };
        let mut conn = conn.lock().await;
        let commands = get_command_from_sql(sql)?;
        for c in commands.iter() {
            let args = &c[1..];
            let mut redis_cmd = redis::cmd(c[0].as_str());
            redis_cmd.arg(args);
            emit.send(
                QueryResp {
                    fields: vec![Field::from_name_type("query result", "")],
                    ..Default::default()
                }
                .into(),
            )?;
            if command_return_multi_value(c[0].as_str()) {
                let stream = redis_cmd.iter_async::<Value>(conn.deref_mut()).await;
                match stream {
                    Ok(mut stream) => {
                        while let Some(value) = stream.next_item().await {
                            let value = value?;
                            let r = encode_response_value(&value)?;
                            let r = r.into_iter().map(|x| vec![Some(x)]).collect::<Vec<_>>();
                            emit.send(QueryEvent::from(r))?;
                        }
                    }
                    Err(err) => {
                        log::error!("query failed:{}", err);
                        emit.send(QueryEvent::Err(err.to_string()))?;
                    }
                }
            } else {
                let value = redis_cmd.query_async::<Value>(conn.deref_mut()).await;
                match value {
                    Ok(value) => {
                        let values = encode_response_value(&value)?
                            .into_iter()
                            .map(|x| vec![Some(x)])
                            .collect::<Vec<_>>();

                        emit.send(values.into())?;
                    }
                    Err(err) => emit.send(QueryEvent::Err(err.to_string()))?,
                }
            }
        }
        anyhow::Ok(())
    }
}

pub fn command_return_multi_value(s: &str) -> bool {
    static ALL: LazyLock<HashSet<String>> = LazyLock::new(|| {
        REDIS_MULTI_VAL_CMDS
            .iter()
            .map(|x| x.to_string())
            .collect::<HashSet<_>>()
    });
    ALL.contains(&s.to_uppercase())
}

/// 完整的返回 Array/Multi-bulk 的 Redis 命令 (兼容 Redis 7.2)
const REDIS_MULTI_VAL_CMDS: &[&str] = &[
    // --- String ---
    "MGET",
    // --- List ---
    "LRANGE",
    "LPOP",
    "RPOP",
    "LMPOP",
    "BLPOP",
    "BRPOP",
    "BLMPOP",
    // --- Set ---
    "SMEMBERS",
    "SRANDMEMBER",
    "SPOP",
    "SINTER",
    "SUNION",
    "SDIFF",
    "SINTERCARD",
    // --- Sorted Set ---
    "ZRANGE",
    "ZREVRANGE",
    "ZRANGEBYSCORE",
    "ZREVRANGEBYSCORE",
    "ZRANGEBYLEX",
    "ZREVRANGEBYLEX",
    "ZPOPMIN",
    "ZPOPMAX",
    "ZMPOP",
    "BZPOPMIN",
    "BZPOPMAX",
    "ZINTER",
    "ZUNION",
    "ZDIFF",
    "ZRANDMEMBER",
    // --- Hash ---
    "HGETALL",
    "HKEYS",
    "HVALS",
    "HMGET",
    "HRANDFIELD",
    // --- Stream ---
    "XREAD",
    "XREADGROUP",
    "XRANGE",
    "XREVRANGE",
    "XCLAIM",
    "XAUTOCLAIM",
    "XINFO",
    // --- Geo (地理位置，容易遗漏) ---
    "GEOPOS",
    "GEORADIUS",
    "GEORADIUSBYMEMBER",
    "GEOSEARCH",
    // --- Generic / Keys ---
    "KEYS",
    "SCAN",
    "SSCAN",
    "HSCAN",
    "ZSCAN",
    "MIGRATE",
    // --- Scripting & Functions (Redis 7+) ---
    "EVAL",
    "EVALSHA",
    "FCALL",
    "FCALL_RO",
    "FUNCTION",
    // --- Admin / Introspection ---
    "ROLE",
    "SLOWLOG",
    "COMMAND",
    "CONFIG GET",
    "CLIENT LIST",
    "PUBSUB",
    "ACL",
    "LATENCY",
    // --- JSON (如果你安装了 RedisJSON 模块) ---
    "JSON.MGET",
    "JSON.OBJKEYS",
];

impl RedisDB {
    pub(crate) async fn get_key_type(
        conn: &mut ConnectionManager,
        key: &str,
    ) -> anyhow::Result<RedisType> {
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

pub struct RedisTokenXXX {
    pub token: String,
    pub start: usize, // start with 0
    pub end: usize,
}

pub fn redis_tokenize(command: &str) -> anyhow::Result<Vec<RedisTokenXXX>> {
    // 使用 shlex 分词
    if let Some(tokens) = shlex::split(command) {
        let mut position = 0;
        let mut tokens_with_positions = Vec::new();
        for token in tokens {
            // 找到 token 在原始字符串中的位置
            if let Some(start) = command[position..].find(&token) {
                let start = position + start;
                let end = start + token.len();
                tokens_with_positions.push(RedisTokenXXX { token, start, end });
                position = end;
            }
        }
        anyhow::Ok(tokens_with_positions)
    } else {
        anyhow::bail!(format!("shlex parse failed:{}", command))
    }
}

pub(crate) fn get_command_from_sql(sql: &str) -> anyhow::Result<Vec<Vec<String>>> {
    let mut commands = Vec::new();
    for line in sql.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("#") {
            continue;
        }
        let tokens = redis_tokenize(line)?;
        commands.push(tokens.into_iter().map(|x| x.token).collect());
    }
    anyhow::Ok(commands)
}

impl ReadOnlyTrait for RedisDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

fn encode_response_value(value: &Value) -> anyhow::Result<Vec<String>> {
    let ret = match value {
        Value::Set(vs) | Value::Array(vs) => {
            // a set
            let mut result = vec![];
            for v in vs.iter() {
                result.extend(encode_response_value(v)?);
            }
            result
        }
        Value::SimpleString(str) => vec![str.to_string()],
        Value::BulkString(str) => {
            vec![String::from_utf8(str.clone())?]
        }
        Value::Map(map) => {
            let mut result = vec![];
            for (k, v) in map.iter() {
                result.extend(encode_response_value(k)?);
                result.extend(encode_response_value(v)?);
            }
            result
        }
        Value::Attribute {
            data,
            attributes: _,
        } => encode_response_value(data)?,
        Value::VerbatimString { text, .. } => {
            vec![text.clone()]
        }
        Value::Nil => vec!["nil".to_string()],
        Value::Int(value) => vec![value.to_string()],
        Value::Okay => vec!["Okay".to_string()],
        Value::Double(value) => vec![value.to_string()],
        Value::Boolean(value) => vec![value.to_string()],
        Value::BigNumber(value) => vec![value.to_string()],
        Value::Push { data, .. } => {
            let mut result = vec![];
            for v in data.iter() {
                result.extend(encode_response_value(v)?);
            }
            result
        }
        Value::ServerError(server_error) => {
            vec![format!("ServerError: {:?}", server_error)]
        }
        _ => anyhow::bail!("unsupported value type: {:?}", value),
    };
    anyhow::Ok(ret)
}

impl RedisDB {
    async fn get_client(&self) -> anyhow::Result<redis::Client> {
        let client = redis::Client::open(self.config.make_conn_string().as_str())?;
        anyhow::Ok(client)
    }

    pub(crate) async fn get_conn(&self) -> anyhow::Result<redis::aio::ConnectionManager> {
        anyhow::Ok(self.conn_manager.clone())
    }
}

// extract database from those vec result.
//
// ```# Keyspace
// db0:keys=12,expires=0,avg_ttl=0
// db1:keys=1,expires=0,avg_ttl=0
// ```
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
pub(crate) async fn make_test_object() -> RedisDB {
    let db = RedisDB::new(RedisDBConfig {
        host: crate::TEST_HOST.to_string(),
        port: None,
        username: None,
        password: None,
        db: Some("db0".to_string()),
        read_only: false,
        conn_timeout: Some(5000.into()),

        enable_ssl: false,
    })
    .await
    .unwrap();
    db
}

impl GetTestExportTableKinds for RedisDB {
    fn get_kinds(&self) -> Vec<ExportKind> {
        vec![
            ExportKind::Sql {
                include_table_schema: false,
            },
            ExportKind::JsonLine,
            ExportKind::Json { pretty: true },
            ExportKind::Csv {
                include_header: false,
                delimiter: ',',
            },
            ExportKind::MarkDown,
            ExportKind::Html {
                include_header: false,
            },
            ExportKind::Xlsx {
                include_header: false,
                datetime_format: Default::default(),
            },
        ]
    }
}
#[cfg(test)]
mod tests {
    use super::make_test_object;
    use super::*;
    use super::{extract_db_from_info_key_space, split_info_colon, RedisDB};
    use crate::db::{DBTrait, QuerySql, SchemaTrait, ServerMetas, ShowTableParameter};
    use crate::enable_test_log;
    use crate::helper::{cert_files_dir, DockerContainerDropper};
    use std::process::Command;

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
    async fn test_server_metas() {
        let db = make_test_object().await;
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }

    #[tokio::test]
    async fn test_list_redis_table() {
        enable_test_log();
        let db = make_test_object().await;
        let mut conn = db.get_conn().await.unwrap();

        for x in 0..31550 {
            let _: () = conn
                .set(format!("key{}", x).as_str(), format!("value{}", x).as_str())
                .await
                .unwrap();
        }
        let redis_tables = db.redis_list_key(None, 100).await.unwrap();
        println!("redis string tables:{:?}", redis_tables.string_tables);
        println!("redis hash tables  :{:?}", redis_tables.hash_tables);
        println!("redis zset tables  :{:?}", redis_tables.zset_tables);
        println!("redis list tables  :{:?}", redis_tables.list_tables);
        println!("redis set tables   :{:?}", redis_tables.set_tables);
    }

    #[tokio::test]
    async fn test_rename_a_key() {
        enable_test_log();
        let db = make_test_object().await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            SET test_rename_key  your_value 
        "#,
        )
        .await
        .unwrap()
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

    #[tokio::test]
    async fn test_schema() {
        enable_test_log();
        let db = make_test_object().await;
        db.create_schema("db11", None).await.unwrap();
        let db11 = db.open_schema("db11").await.unwrap();
        let db11 = db11.as_any().downcast_ref::<RedisDB>().unwrap();
        db11.string_set("xxxxx", "yyyy".as_bytes()).await.unwrap();
        let db_names = db.list_schema().await.unwrap();
        for db_name in db_names.iter() {
            println!("db:{:?}", db_names);
            db.open_schema(db_name).await.unwrap();
        }
        db.drop_schema("db11").await.unwrap();
    }

    #[tokio::test]
    async fn test_large_key() {
        {
            // large list
            let db = make_test_object().await;
            let key_name = "large_list";
            let _ = db.drop_table(key_name, false).await;
            let query_id = "aaaa";
            db.open_connection(query_id).await.unwrap();
            for i in 0..3000 {
                db.execute_sql(
                    query_id,
                    format!(r#"RPUSH {} data{}"#, key_name, i).as_str(),
                )
                .await
                .unwrap()
                .unwrap();
            }
        }
        {
            // large set
            let db = make_test_object().await;
            let key_name = "large_set";
            let _ = db.drop_table(key_name, false).await;
            let query_id = "aaaa";
            db.open_connection(query_id).await.unwrap();
            for i in 0..3000 {
                db.execute_sql(query_id, format!(r#"sadd {} data{}"#, key_name, i).as_str())
                    .await
                    .unwrap()
                    .unwrap();
            }
        }
        {
            // large zset
            let db = make_test_object().await;
            let key_name = "large_zset";
            let _ = db.drop_table(key_name, false).await;
            let query_id = "aaaa";
            db.open_connection(query_id).await.unwrap();
            for i in 0..3000 {
                db.execute_sql(
                    query_id,
                    format!(r#"ZADD {} {} data{}"#, key_name, i, i).as_str(),
                )
                .await
                .unwrap()
                .unwrap();
            }
        }
        {
            // large hash
            let db = make_test_object().await;
            let key_name = "large_hash";
            let _ = db.drop_table(key_name, false).await;
            let query_id = "aaaa";
            db.open_connection(query_id).await.unwrap();
            for i in 0..3000 {
                db.execute_sql(
                    query_id,
                    format!(r#"HMSET {} key{} value{}"#, key_name, i, i).as_str(),
                )
                .await
                .unwrap()
                .unwrap();
            }
        }
    }
    #[tokio::test]
    async fn test_hash_key() {
        let db = make_test_object().await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let table_name = "hash_value";

        let _ = db.drop_table(table_name, false).await;

        db.execute_sql(query_id, "hset hash_value key11111 value2222")
            .await
            .unwrap()
            .unwrap();

        db.execute_sql(query_id, "hset hash_value key2222 value33333")
            .await
            .unwrap()
            .unwrap();
        // export

        GetTestExportTableKinds::test_export(&db, table_name).await;

        let cols = db.show_table_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(&table_name_dup, false).await;

        db.duplicate_table(table_name, &table_name_dup)
            .await
            .unwrap();
        let resp2 = db
            .show_table_collect(
                &table_name_dup,
                ShowTableParameter::new_stupid_test("id asc"),
            )
            .await
            .unwrap();
        assert_eq!(resp, resp2);
    }

    #[tokio::test]
    async fn test_zset_key() {
        let db = make_test_object().await;
        enable_test_log();
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let table_name = "zset_value";

        let _ = db.drop_table(table_name, false).await;

        let ret = db
            .execute_sql(query_id, "zadd zset_value 1 haha")
            .await
            .unwrap();
        println!("query ret :{:?}", ret);

        let ret = db
            .execute_sql(query_id, "zadd zset_value 2 xxx")
            .await
            .unwrap();
        println!("query ret :{:?}", ret);
        // export

        GetTestExportTableKinds::test_export(&db, table_name).await;

        let cols = db.show_table_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = (SchemaTrait::as_any(&db).downcast_ref::<RedisDB>())
            .unwrap()
            .show_table_collect(
                table_name,
                ShowTableParameter {
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(&table_name_dup, false).await;
        db.duplicate_table(table_name, &table_name_dup)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_set_key() {
        let db = make_test_object().await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let table_name = "set_value";

        let _ = db.drop_table(table_name, false).await;

        let ret = db
            .execute_sql(query_id, "sadd set_value a")
            .await
            .unwrap()
            .unwrap();
        println!("query ret :{:?}", ret);
        let ret = db
            .execute_sql(query_id, "sadd set_value b")
            .await
            .unwrap()
            .unwrap();
        println!("query ret :{:?}", ret);
        // export
        GetTestExportTableKinds::test_export(&db, table_name).await;
        let cols = db.show_table_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(table_name_dup.as_str(), false).await;
        db.duplicate_table(table_name, &table_name_dup)
            .await
            .unwrap();
        let resp2 = db
            .show_table_collect(
                &table_name_dup,
                ShowTableParameter::new_stupid_test("id asc"),
            )
            .await
            .unwrap();
        assert_eq!(resp, resp2);
    }

    #[tokio::test]
    async fn test_list_key() {
        let db = make_test_object().await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let table_name = "list_value";

        let _ = db.drop_table(table_name, false).await;

        let ret = db
            .execute_sql(query_id, "lpush list_value a")
            .await
            .unwrap();
        println!("query ret :{:?}", ret);

        let ret = db
            .execute_sql(query_id, "lpush list_value b")
            .await
            .unwrap();
        println!("query ret :{:?}", ret);
        // export
        GetTestExportTableKinds::test_export(&db, table_name).await;
        let cols = db.show_table_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(table_name_dup.as_str(), false).await;
        db.duplicate_table(table_name, &table_name_dup)
            .await
            .unwrap();
        let resp2 = db
            .show_table_collect(
                &table_name_dup,
                ShowTableParameter::new_stupid_test("id asc"),
            )
            .await
            .unwrap();
        assert_eq!(resp, resp2);
    }

    #[test]
    fn test_split_info_colon() {
        assert_eq!(
            split_info_colon("loading:0").unwrap(),
            ("loading".to_string(), "0".to_string())
        );
    }

    #[tokio::test]
    async fn test_string_key() {
        let db = make_test_object().await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let table_name = "string_value";

        let _ = db.drop_table(table_name, false).await;

        let ret = db
            .execute_sql(query_id, "set string_value string_value")
            .await
            .unwrap();
        println!("query_ret:{:?}", ret);

        // export
        GetTestExportTableKinds::test_export(&db, table_name).await;
        let cols = db.show_table_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(&table_name_dup, false).await;
        db.duplicate_table(table_name, &table_name_dup)
            .await
            .unwrap();
        let resp2 = db
            .show_table_collect(
                &table_name_dup,
                ShowTableParameter::new_stupid_test("id asc"),
            )
            .await
            .unwrap();
        assert_eq!(resp, resp2);
    }

    #[tokio::test]

    async fn test_ssl() {
        let port = portpicker::pick_unused_port().unwrap();
        println!("test port:{}", port);
        let cert_dir = cert_files_dir();

        // Redis SSL/TLS 配置
        let ca_volume = format!("{}/ca.crt:/etc/redis/ssl/ca.crt", cert_dir);
        let cert_volume = format!("{}/server.crt:/etc/redis/ssl/server.crt", cert_dir);
        let key_volume = format!("{}/server.key:/etc/redis/ssl/server.key", cert_dir);

        // 创建 Redis 配置文件
        let redis_config_content = r#"
port 0
tls-port 6379
tls-cert-file /etc/redis/ssl/server.crt
tls-key-file /etc/redis/ssl/server.key
tls-ca-cert-file /etc/redis/ssl/ca.crt
tls-auth-clients yes
tls-replication yes
requirepass testpass
# 强制使用 TLS
tls-protocols "TLSv1.2 TLSv1.3"
"#
        .to_string();

        let redis_config_path = format!("{}/redis.conf", cert_dir);
        std::fs::write(&redis_config_path, redis_config_content).unwrap();
        let config_volume = format!("{}:/usr/local/etc/redis/redis.conf", redis_config_path);

        // 启动 Redis 容器，开启 SSL/TLS
        let output = Command::new("docker")
            .arg("run")
            .arg("-d")
            .args(["--name", "redis-ssl-test"])
            .args(["-p", format!("{}:6379", port).as_str()])
            .args(["-v", &ca_volume])
            .args(["-v", &cert_volume])
            .args(["-v", &key_volume])
            .args(["-v", &config_volume])
            .arg("redis")
            .args(["redis-server", "/usr/local/etc/redis/redis.conf"])
            .output()
            .unwrap();

        let container_id = if output.status.success() {
            let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("Redis 容器启动成功，ID 为: {}", container_id);
            container_id
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("启动 Docker 容器失败: {}", stderr);
            return;
        };

        let _guard = DockerContainerDropper::from(&container_id);
        // no test here I don't want to work with rustls
        // rustls 的兼容性不如native-tls
        // https://docs.rs/redis/1.5.0/redis/struct.Client.html#method.build_with_tls
    }
}
