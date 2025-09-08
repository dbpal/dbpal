use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::str::{FromStr, from_utf8};
use std::sync::Arc;
use std::time::Duration;
use std::vec;

use dashmap::{DashMap, DashSet};
use futures_util::StreamExt;
use redis::aio::{ConnectionManager, ConnectionManagerConfig, MultiplexedConnection};
use redis::{AsyncCommands, AsyncIter, Value};
use sqlparser::tokenizer::Token;
use tokio::sync::Mutex;
use typescript_type_def::TypeDef;

use crate::datasource_config::RedisDBConfig;
use crate::db::{
    BackendKind, CheckConn, CreateSchemaExtra, DBTrait, ExportKind, ExportQueryResult, Field,
    GetBackendKind, ListTable, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas,
    ShowExtraArgs, SqlExecuteRet,
};
use crate::helper::{OffsetLimitIter, create_csv};
use crate::meta_value::MetaValue;
use crate::redis_lang_server::RedisDialet;

/// https://stackoverflow.com/questions/30611215/redis-how-to-limit-the-return-number-of-keys-command

#[derive(Clone)]
pub struct RedisDB {
    config: RedisDBConfig,
    conn_manager: ConnectionManager,
    query_connections: Arc<DashMap<uuid::Uuid, Arc<Mutex<MultiplexedConnection>>>>,
    created_database: Arc<DashSet<String>>,
}

impl RedisDB {
    pub async fn new(config: RedisDBConfig) -> anyhow::Result<Self> {
        let conn_str = config.make_conn_string();
        log::info!("connect to redis var:{:?}", conn_str);

        let client = redis::Client::open(conn_str.as_str())?;
        let mut conn_config = ConnectionManagerConfig::default();
        if let Some(x) = config.conn_timeout {
            conn_config = conn_config.set_connection_timeout(Duration::from_millis(x as u64));
        }
        let m = redis::aio::ConnectionManager::new_with_config(client, conn_config).await?;
        anyhow::Ok(Self {
            config,
            conn_manager: m,
            query_connections: Default::default(),
            created_database: Default::default(),
        })
    }
}

impl RedisDB {
    pub async fn zset_show_table(
        &self, key: &str, where_str: Option<&str>, min: Option<f64>, max: Option<f64>,
    ) -> anyhow::Result<QueryResp> {
        let mut conn = self.get_conn().await?;

        let total: usize = conn.zcard(key).await?;
        let mut ret: AsyncIter<(Vec<u8>, f64)> =
            conn.zscan_match(key, where_str.unwrap_or("*")).await?;
        let mut key_values = Vec::new();
        let mut has_more = false;
        while let Some(value) = ret.next().await {
            // try match
            if let Some(min) = min {
                if value.1 < min {
                    continue;
                }
            }
            if let Some(max) = max {
                if value.1 > max {
                    continue;
                }
            }
            if key_values.len() >= Self::MAX_RETURNING {
                has_more = true;
                break;
            }
            key_values.push((value.1, value.0));
        }
        anyhow::Ok(QueryResp {
            fields: vec![
                Field::from_name_type("score", "f64"),
                Field::from_name_type("key", "string"),
            ],
            total,
            data: key_values
                .into_iter()
                .map(|(score, key)| {
                    vec![Some(score.to_string()), Some(hex::encode(key.as_slice()))]
                })
                .collect(),
            sql: None,
            cache_time: None,
            has_more,
        })
    }
}

impl std::fmt::Debug for RedisDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "redis on:{:?}", self.config)
    }
}

#[derive(Clone, Debug, serde::Serialize, Default, TypeDef)]
pub struct RedisListTable {
    string_tables: Vec<String>,
    list_tables: Vec<String>,
    set_tables: Vec<String>,
    hash_tables: Vec<String>,
    zset_tables: Vec<String>,
    has_more: bool,
}

impl RedisDB {
    pub async fn list_lpush(&self, key: &str, value: &[u8]) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.lpush(key, value).await?;
        anyhow::Ok(())
    }

    pub async fn list_rpush(&self, key: &str, value: &[u8]) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.rpush(key, value).await?;
        anyhow::Ok(())
    }

    pub async fn list_remove(&self, key: &str, count: isize, value: &[u8]) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.lrem(key, count, value).await?;
        anyhow::Ok(())
    }

    pub async fn list_set(&self, key: &str, index: isize, value: &[u8]) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.lset(key, index, value).await?;
        anyhow::Ok(())
    }

    pub async fn set_replace(
        &self, key: &str, old_member: &[u8], new_member: &[u8],
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.srem(key, old_member).await?;
        let _: () = conn.sadd(key, new_member).await?;
        anyhow::Ok(())
    }

    pub async fn set_del(&self, key: &str, member: &[u8]) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.srem(key, member).await?;
        anyhow::Ok(())
    }

    pub async fn set_add(&self, key: &str, member: &[u8]) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.sadd(key, member).await?;
        anyhow::Ok(())
    }

    pub async fn hash_replace(
        &self, key: &str, old_member: &[u8], new_member: &[u8], new_value: &[u8],
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.hdel(key, old_member).await?;
        let _: () = conn.hset(key, new_member, new_value).await?;
        anyhow::Ok(())
    }

    pub async fn hash_del(&self, key: &str, member: &[u8]) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.hdel(key, member).await?;
        anyhow::Ok(())
    }

    pub async fn hash_add(&self, key: &str, member: &[u8], value: &[u8]) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.hset(key, member, value).await?;
        anyhow::Ok(())
    }

    pub async fn zset_replace(
        &self, key: &str, old_member: &[u8], new_member: &[u8], new_value: f64,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.zrem(key, old_member).await?;
        let _: () = conn.zadd(key, new_member, new_value).await?;

        anyhow::Ok(())
    }

    pub async fn zset_del(&self, key: &str, member: &[u8]) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.zrem(key, member).await?;
        anyhow::Ok(())
    }

    pub async fn zset_add(&self, key: &str, member: &[u8], value: f64) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let _: () = conn.zadd(key, member, value).await?;
        anyhow::Ok(())
    }

    pub async fn string_set(&self, key: &str, value: &[u8]) -> anyhow::Result<()> {
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

// export impl
impl RedisDB {
    async fn export_table_as_csv(
        &self, key: &str, delimiter: char, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
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

impl GetBackendKind for RedisDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::Redis
    }
}

impl RedisDB {
    pub async fn zset_range_by_score(
        &self, key: &str, min: Option<f64>, max: Option<f64>, page: isize, page_count: isize,
    ) -> anyhow::Result<QueryResp> {
        let mut conn = self.get_conn().await?;
        let offset = (page - 1) * page_count;
        let total: usize = conn.zcount(key, min, max).await?;
        let res: Vec<String> = conn
            .zrangebyscore_limit_withscores(key, min, max, offset, page_count)
            .await?;
        let data: Vec<_> = res
            .chunks(2)
            .map(|x| vec![Some(x[0].clone()), Some(x[1].clone())])
            .collect();

        anyhow::Ok(QueryResp::new(
            vec![
                Field::from_name_type("key", "string"),
                Field::from_name_type("value", "string"),
            ],
            total,
            data,
            None,
        )?)
    }
}

#[async_trait::async_trait]

impl ExportQueryResult for RedisDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }
}

impl RedisDB {
    pub const MAX_RETURNING: usize = 2000;
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
                let mut iter = conn.sscan::<_, Vec<u8>>(key).await?.ready_chunks(100);
                while let Some(set) = iter.next().await {
                    let _: () = conn_set.sadd(dup, set).await?;
                }
            }

            RedisType::Hash => {
                let mut stream: AsyncIter<'_, Vec<u8>> = conn.hscan(key).await?;
                let mut key = None as Option<Vec<u8>>;
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
                let mut stream: AsyncIter<'_, Vec<u8>> = conn.zscan(key).await?;
                let mut zkey = None as Option<Vec<u8>>;

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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    async fn export_table_or_view(
        &self, key: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::CSV {
                include_header,
                delimiter,
            } => {
                if include_header {
                    anyhow::bail!("redis have no header")
                }
                self.export_table_as_csv(key, delimiter, sender).await
            }
            ExportKind::Json { pretty } => self.export_table_as_json(key, pretty, sender).await,
            ExportKind::Sql {
                include_table_schema: _,
            } => self.export_table_to_command(key, sender).await,
            ExportKind::JsonLine => self.export_table_as_json_line(key, sender).await,
            ExportKind::Xlsx { .. } => anyhow::bail!("Not implemented yet"),
        }
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
        &self, key: &str, where_str: Option<&str>, _order_by: Option<&str>, _page: usize,
        _page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        let mut conn = self.get_conn().await?;
        let typ = Self::get_key_type(&mut conn, key).await?;
        use futures_util::StreamExt;
        match typ {
            RedisType::String => {
                let ret: Option<Vec<u8>> = conn.get(key).await?;
                anyhow::Ok(QueryResp::result(
                    "value",
                    format!("GET {}", key),
                    vec![ret.map(|x| hex::encode(x.as_slice()))],
                ))
            }

            RedisType::List => {
                let r: Vec<Vec<u8>> = conn.lrange(key, 0, -1).await?;
                anyhow::Ok(QueryResp::new(
                    vec![
                        Field::from_name_type("seq", "int"),
                        Field::from_name_type("value", "string"),
                    ],
                    r.len(),
                    r.into_iter()
                        .enumerate()
                        .map(|(seq, r)| {
                            vec![Some(seq.to_string()), Some(hex::encode(r.as_slice()))]
                        })
                        .collect(),
                    None,
                )?)
            }

            RedisType::Set => {
                let total: usize = conn.scard(key).await?;
                let mut ret: AsyncIter<Vec<u8>> =
                    conn.sscan_match(key, where_str.unwrap_or("*")).await?;
                let mut set_values = Vec::new();
                let mut has_more = false;
                while let Some(value) = ret.next().await {
                    if set_values.len() >= Self::MAX_RETURNING {
                        has_more = true;
                        break;
                    }
                    set_values.push(value);
                }
                anyhow::Ok(QueryResp {
                    fields: vec![Field::from_name_type("value", "string")],
                    total,
                    data: set_values
                        .into_iter()
                        .map(|x| vec![Some(hex::encode(x.as_slice()))])
                        .collect(),
                    sql: None,
                    cache_time: None,
                    has_more,
                })
            }
            RedisType::Hash => {
                let total: usize = conn.hlen(key).await?;
                let mut ret: AsyncIter<Vec<u8>> =
                    conn.hscan_match(key, where_str.unwrap_or("*")).await?;
                let mut index = 0;
                let mut key_values = Vec::new();
                let mut value_values = Vec::new();
                let mut has_more = false;
                while let Some(value) = ret.next().await {
                    if index >= Self::MAX_RETURNING << 1 {
                        has_more = true;
                        break;
                    }
                    if index % 2 == 0 {
                        key_values.push(value.clone());
                    } else {
                        value_values.push(value.clone());
                    }
                    index += 1;
                }
                anyhow::Ok(QueryResp {
                    fields: vec![
                        Field::from_name_type("key", "string"),
                        Field::from_name_type("value", "string"),
                    ],
                    total,
                    data: key_values
                        .into_iter()
                        .zip(value_values.into_iter())
                        .map(|(k, v)| {
                            vec![
                                Some(hex::encode(k.as_slice())),
                                Some(hex::encode(v.as_slice())),
                            ]
                        })
                        .collect(),
                    sql: None,
                    cache_time: None,
                    has_more,
                })
            }

            RedisType::Zset => {
                anyhow::bail!("use redis show table zset implementation");
            }
        }
    }

    async fn table_insert(
        &self, _table_name: &str, _values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        anyhow::bail!("call query sql instread")
    }

    async fn table_update(
        &self, _table_name: &str, _values: HashMap<String, Option<String>>,
        _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        anyhow::bail!("call query sql instread")
    }

    async fn truncate_table(&self, key: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let typ = Self::get_key_type(&mut conn, key).await?;
        match typ {
            RedisType::String => {
                let _: () = conn.set(key, "").await?;
            }
            _ => {
                anyhow::bail!(
                    "redis collection type delete all element will cause the key been droped"
                )
            }
        }
        anyhow::Ok(())
    }

    async fn table_delete(
        &self, _key: &str, _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        anyhow::bail!("not impl")
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn: ConnectionManager = self.get_conn().await?;
        let _: () = conn.del(table).await?;
        Ok(())
    }
}

impl RedisDB {
    pub async fn redis_list_table(
        &self, filter: Option<&str>, max: usize,
    ) -> anyhow::Result<RedisListTable> {
        let mut conn = self.get_conn().await?;
        let tables: Vec<String> = conn.keys(filter.unwrap_or("*")).await?;
        let mut ret = RedisListTable::default();
        let mut count: usize = 0;
        let mut has_more = false;
        for table in tables.into_iter() {
            if count >= max {
                has_more = true;
                break;
            }
            let typ = Self::get_key_type(&mut conn, table.as_str()).await?;
            match typ {
                RedisType::String => ret.string_tables.push(table),
                RedisType::List => ret.list_tables.push(table),
                RedisType::Set => ret.set_tables.push(table),
                RedisType::Hash => ret.hash_tables.push(table),
                RedisType::Zset => ret.zset_tables.push(table),
            }
            count += 1;
        }
        ret.has_more = has_more;
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
        &self, name: &str, _extra: Option<CreateSchemaExtra>,
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
        db.drop_all_keys().await?;
        self.created_database.remove(&name.to_owned());
        anyhow::Ok(())
    }
}

impl RedisDB {
    async fn drop_all_keys(&self) -> anyhow::Result<()> {
        let mut conn = self.get_client().await?;
        redis::cmd("flushdb").exec(&mut conn)?;
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl QuerySql for RedisDB {
    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        anyhow::Ok(get_command_from_sql(sql)?.iter().any(|x| {
            x.first()
                .map(|x| {
                    x.eq_ignore_ascii_case("FLUSHDB")
                        || x.eq_ignore_ascii_case("FLUSHALL")
                        || x.eq_ignore_ascii_case("DEL")
                        || x.eq_ignore_ascii_case("CONFIG")
                        || x.eq_ignore_ascii_case("SHUTDOWN")
                        || x.eq_ignore_ascii_case("DEBUG")
                        || x.eq_ignore_ascii_case("REPLICAOF")
                        || x.eq_ignore_ascii_case("SWAPDB")
                        || x.eq_ignore_ascii_case("EVAL")
                        || x.eq_ignore_ascii_case("EVALSHA")
                })
                .unwrap_or(false)
        }))
    }

    /// redis have no transaction any connection is fine
    /// so connection management for redis is dummy
    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        let client: redis::Client = self.get_client().await?;
        let conn = client.get_multiplexed_async_connection().await?;
        self.query_connections
            .insert(id, Arc::new(Mutex::new(conn)));
        anyhow::Ok(id)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        let client = self.get_client().await?;
        let conn = client.get_multiplexed_async_connection().await?;
        self.query_connections
            .insert(id, Arc::new(Mutex::new(conn)));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.query_connections.remove(&id);
        anyhow::Ok(())
    }
    async fn execute_sql_no_open_conn(
        &self, _sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        anyhow::bail!("the frontend will not use the api for now.")
    }
    async fn execute_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let ret = QuerySql::query_sql(self, query_id, sql).await?;
        match ret.len() {
            1 => {
                return anyhow::Ok(ret[0].clone());
            }
            0 => return anyhow::Ok(Err("empty statement???".into())),
            _ => {
                log::warn!("multi resp shoule be fine");
                return anyhow::Ok(ret[0].clone());
            }
        }
    }

    async fn query_sql(
        &self, id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let conn = match self.query_connections.get(&id) {
            Some(x) => x.clone(),
            None => {
                anyhow::bail!(format!("can't find connection for id '{}'", id))
            }
        };
        let mut lock = conn.lock().await;
        let ret = query_sql(&mut lock, sql).await;
        ret
    }
}

impl RedisDB {
    pub(crate) async fn get_key_type(
        conn: &mut ConnectionManager, key: &str,
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

/// splite sql by line and whitespace
/// to list of commmands and args
fn get_command_from_sql(sql: &str) -> anyhow::Result<Vec<Vec<String>>> {
    let mut commands = Vec::new();
    let dialet = RedisDialet {};

    for line in sql.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let tokens = sqlparser::tokenizer::Tokenizer::new(&dialet, line).tokenize()?;
        let c = tokens
            .iter()
            .filter(|x| match x {
                Token::Whitespace(_) => false,
                _ => true,
            })
            .map(|x| match x {
                Token::SingleQuotedString(x) => x.to_string(),
                Token::DoubleQuotedString(x) => x.to_string(),
                Token::Word(x) if x.quote_style == Some('"') => x.value.to_string(),
                _ => x.to_string(),
            })
            .collect();
        commands.push(c);
    }
    anyhow::Ok(commands)
}

#[cfg(test)]
#[test]
fn test_xxx() {
    let cs = get_command_from_sql(r#"set my_key "value"   'value' "#).unwrap();
    let c = &cs[0];

    for x in c.iter() {
        println!("x:{}", x);
    }
}
impl ReadOnlyTrait for RedisDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

async fn query_sql(
    conn: &mut redis::aio::MultiplexedConnection, sql: &str,
) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
    let commands = get_command_from_sql(sql)?;
    let mut ret = Vec::new();
    for c in commands.iter() {
        let args = &c[1..];
        let v = redis::cmd(c[0].as_str())
            .arg(args)
            .query_async(conn)
            .await
            .map(|x| encode_response_value(&x, RedisValueEncodeHit::from_cmd(&c[..])));

        match v {
            Ok(x) => {
                ret.push(x.map_err(|x| x.to_string()));
            }
            Err(x) => ret.push(Err(x.to_string())),
        }
    }
    anyhow::Ok(ret)
}

/// redis only support limited nested value.
/// if hit this fn should somehow always return string.
/// right ???
fn encode_response_value_to_string(v: &Value) -> String {
    match v {
        Value::Int(v) => v.to_string(),
        Value::BulkString(x) => match from_utf8(x.as_slice()) {
            Ok(x) => x.to_owned(),
            Err(_) => format!("0x{}", hex::encode(x.as_slice())),
        },
        Value::SimpleString(x) => x.to_owned(),
        Value::Double(d) => d.to_string(),
        Value::Boolean(x) => x.to_string(),
        Value::BigNumber(v) => v.to_string(),
        _ => format!("{:?}", v),
    }
}

#[derive(Clone, Copy, Debug, Default)]
enum RedisValueEncodeHit {
    Hash,
    Zset,
    #[default]
    None,
}

impl RedisValueEncodeHit {
    fn from_cmd(cmd: &[String]) -> Option<Self> {
        if cmd
            .first()
            .map(|x| x.eq_ignore_ascii_case("HGETALL"))
            .unwrap_or(false)
        {
            return Some(Self::Hash);
        }
        if cmd
            .first()
            .map(|x| x.eq_ignore_ascii_case("ZRANGE"))
            .unwrap_or(false)
            && cmd
                .last()
                .map(|x| x.eq_ignore_ascii_case("WITHSCORES"))
                .unwrap_or(false)
        {
            return Some(Self::Zset);
        }
        None
    }
}

fn encode_response_value(
    v: &Value, array_type_notice: Option<RedisValueEncodeHit>,
) -> anyhow::Result<QueryResp> {
    let ret = match v {
        Value::Set(vs) => {
            // a set
            let result: Vec<_> = vs
                .iter()
                .map(encode_response_value_to_string)
                .map(Some)
                .collect();
            QueryResp::result("set", None, result)
        }
        Value::Array(vs) => {
            let array_type_notice = array_type_notice.unwrap_or_default();
            match array_type_notice {
                RedisValueEncodeHit::Hash => {
                    let data = vs
                        .chunks(2)
                        .map(|x| {
                            vec![
                                Some(encode_response_value_to_string(&x[0])),
                                Some(encode_response_value_to_string(&x[1])),
                            ]
                        })
                        .collect::<Vec<_>>();
                    let fields = vec![
                        Field::from_name_type("key", ""),
                        Field::from_name_type("value", ""),
                    ];
                    QueryResp::new(fields, 0, data, None)?
                }

                RedisValueEncodeHit::Zset => {
                    let data = vs
                        .chunks(2)
                        .map(|x| {
                            vec![
                                Some(encode_response_value_to_string(&x[0])),
                                Some(encode_response_value_to_string(&x[1])),
                            ]
                        })
                        .collect::<Vec<_>>();
                    let fields = vec![
                        Field::from_name_type("value", ""),
                        Field::from_name_type("score", ""),
                    ];
                    QueryResp::new(fields, 0, data, None)?
                }
                RedisValueEncodeHit::None => {
                    let result: Vec<_> = vs
                        .iter()
                        .map(encode_response_value_to_string)
                        .map(Some)
                        .collect();
                    QueryResp::result("array", None, result)
                }
            }
        }

        Value::SimpleString(x) => QueryResp::result("result", None, vec![Some(x.to_string())]),
        Value::BulkString(_x) => QueryResp::result(
            "result",
            None,
            vec![Some(encode_response_value_to_string(v))],
        ),
        Value::Map(x) => {
            let fields = vec![
                Field::from_name_type("key", "string"),
                Field::from_name_type("value", ""),
            ];
            let data = x
                .iter()
                .map(|x| {
                    vec![
                        Some(encode_response_value_to_string(&x.0)),
                        Some(encode_response_value_to_string(&x.1)),
                    ]
                })
                .collect::<Vec<_>>();

            QueryResp::new(fields, 0, data, None)?
        }
        Value::Attribute {
            data,
            attributes: _, // TODO how to handle this???
        } => encode_response_value(data, array_type_notice)?,
        Value::VerbatimString { format, text } => {
            let fields = vec![
                Field::from_name_type("format", ""),
                Field::from_name_type("value", ""),
            ];
            let data = vec![vec![Some(format.to_string()), Some(text.clone())]];
            QueryResp::new(fields, 0, data, None)?
        }
        _ => QueryResp::result("result", None, vec![Some(format!("{:?}", v))]),
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
        conn_timeout: None,
        save_password: true,
    })
    .await
    .unwrap();
    db
}
#[cfg(test)]
mod tests {

    use std::time::Duration;

    use super::make_test_object;
    use futures_util::StreamExt;
    use redis::AsyncCommands;

    use super::{RedisDB, extract_db_from_info_key_space, split_info_colon};

    use crate::db::{DBTrait, ExportKind, QuerySql, SchemaTrait, ServerMetas};
    use crate::enable_test_log;
    use crate::language_server::completion_test::test_file_auto_completion;

    #[tokio::test]

    async fn test_select_zset_binary() {
        let db = make_test_object().await;
        let mut conn = db.get_conn().await.unwrap();
        let mut x: redis::AsyncIter<'_, Vec<u8>> = conn.zscan("zset_value").await.unwrap();
        while let Some(val) = x.next().await {
            println!("{:?}", val);
        }
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
    async fn test_zrange_score() {
        enable_test_log();
        let db = make_test_object().await;
        let resp = db
            .zset_range_by_score("zset_value", Some(1f64), Some(100f64), 1, 100)
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }

    #[tokio::test]
    async fn test_auto_completion_xxx() {
        enable_test_log();
        let content = r#"SSCAN set
1 SSCAN ; 10 set_value"#;
        let db = make_test_object().await;
        test_file_auto_completion(&content, &db, false).await;
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
        tokio::time::sleep(Duration::from_secs(2)).await;
        let redis_tables = db.redis_list_table(None, 100).await.unwrap();
        println!("redis string tables:{:?}", redis_tables.string_tables);
        println!("redis hash tables  :{:?}", redis_tables.hash_tables);
        println!("redis zset tables  :{:?}", redis_tables.zset_tables);
        println!("redis list tables  :{:?}", redis_tables.list_tables);
        println!("redis set tables   :{:?}", redis_tables.set_tables);
    }

    #[tokio::test]
    async fn test_rename() {
        enable_test_log();
        let db = make_test_object().await;

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

    #[tokio::test]
    async fn test_schema() {
        enable_test_log();
        let db = make_test_object().await;
        db.create_schema("db11", None).await.unwrap();
        let db11 = db.open_schema("db11").await.unwrap();
        let db11 = db11.as_any().downcast_ref::<RedisDB>().unwrap();
        db11.string_set("xxxxx", "yyyy".as_bytes()).await.unwrap();
        let dbnames = db.list_schema().await.unwrap();
        for dbname in dbnames.iter() {
            println!("db:{:?}", dbnames);
            db.open_schema(&dbname).await.unwrap();
        }

        db.drop_schema("db11").await.unwrap();
    }
    #[ignore = "run me by hand"]
    #[tokio::test]
    async fn test_check_alot_of_key() {
        let db = make_test_object().await;
        crate::db::CheckConn::check_conn(&db).await.unwrap();
        let query_id = db.open_connection().await.unwrap();
        for i in 0..100000 {
            db.query_sql(
                query_id,
                format!("set test_alot_of_key{} {}", i, i).as_str(),
            )
            .await
            .unwrap();

            println!("{}", i);
        }
        let now = std::time::Instant::now();
        let x = db.redis_list_table(None, 100).await.unwrap();
        println!("used{:?} key_len:{}", now.elapsed(), x.string_tables.len());
    }
    #[tokio::test]
    async fn test_large_key() {
        {
            // large list
            let db = make_test_object().await;
            let key_name = "large_list";
            let _ = db.drop_table(key_name, false).await;
            let query_id = db.open_connection().await.unwrap();
            for i in 0..3000 {
                db.query_sql(
                    query_id,
                    format!(r#"RPUSH {} data{}"#, key_name, i).as_str(),
                )
                .await
                .unwrap();
            }
        }
        {
            // large set
            let db = make_test_object().await;
            let key_name = "large_set";
            let _ = db.drop_table(key_name, false).await;
            let query_id = db.open_connection().await.unwrap();
            for i in 0..3000 {
                db.query_sql(query_id, format!(r#"sadd {} data{}"#, key_name, i).as_str())
                    .await
                    .unwrap();
            }
        }
        {
            // large zset
            let db = make_test_object().await;
            let key_name = "large_zset";
            let _ = db.drop_table(key_name, false).await;
            let query_id = db.open_connection().await.unwrap();
            for i in 0..3000 {
                db.query_sql(
                    query_id,
                    format!(r#"ZADD {} {} data{}"#, key_name, i, i).as_str(),
                )
                .await
                .unwrap();
            }
        }
        {
            // large hash
            let db = make_test_object().await;
            let key_name = "large_hash";
            let _ = db.drop_table(key_name, false).await;
            let query_id = db.open_connection().await.unwrap();
            for i in 0..3000 {
                db.query_sql(
                    query_id,
                    format!(r#"HMSET {} key{} value{}"#, key_name, i, i).as_str(),
                )
                .await
                .unwrap();
            }
        }
    }
    #[tokio::test]
    async fn test_hash_key() {
        let db = make_test_object().await;

        let query_id = db.open_connection().await.unwrap();
        let table_name = "hash_value";

        let _ = db.drop_table(table_name, false).await;

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
        let cols = db.show_table_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(&table_name_dup, false).await;

        db.duplicate_table(&table_name, &table_name_dup)
            .await
            .unwrap();
        let resp2 = db
            .show_table(&table_name_dup, None, None, 1, 100, None)
            .await
            .unwrap();
        assert_eq!(resp, resp2);
    }

    #[tokio::test]
    async fn test_query_sql() {
        let db = make_test_object().await;

        let query_id = db.open_connection().await.unwrap();
        let resp = db.query_sql(query_id, "keys *").await.unwrap();
        println!("key*:{:?}", resp);
        let resp = db
            .query_sql(query_id, "LRANGE list_value 0 -1")
            .await
            .unwrap();
        println!("all list:{:?}", resp);
        let resp = db.query_sql(query_id, "HGETALL hash_value").await.unwrap();
        println!("all hash:{:?}", resp);
        let resp = db.query_sql(query_id, "SMEMBERS set_value").await.unwrap();
        println!("all set:{:?}", resp);

        let resp = db
            .query_sql(query_id, "ZRANGE zset_value 0 -1 WITHSCORES")
            .await
            .unwrap();
        println!("all zset:{:?}", resp);
    }

    #[tokio::test]
    async fn test_zset_key() {
        let db = make_test_object().await;

        let query_id = db.open_connection().await.unwrap();
        let table_name = "zset_value";

        let _ = db.drop_table(table_name, false).await;

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
        let cols = db.show_table_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = (SchemaTrait::as_any(&db).downcast_ref::<RedisDB>())
            .unwrap()
            .zset_show_table(table_name, None, None, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(&table_name_dup, false).await;
        db.duplicate_table(&table_name, &table_name_dup)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_set_key() {
        let db = make_test_object().await;

        let query_id = db.open_connection().await.unwrap();
        let table_name = "set_value";

        let _ = db.drop_table(table_name, false).await;

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
        let cols = db.show_table_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(table_name_dup.as_str(), false).await;
        db.duplicate_table(&table_name, &table_name_dup)
            .await
            .unwrap();
        let resp2 = db
            .show_table(&table_name_dup, None, None, 1, 100, None)
            .await
            .unwrap();
        assert_eq!(resp, resp2);
    }

    #[tokio::test]
    async fn test_list_key() {
        let db = make_test_object().await;

        let query_id = db.open_connection().await.unwrap();
        let table_name = "list_value";

        let _ = db.drop_table(table_name, false).await;

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
        let cols = db.show_table_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(table_name_dup.as_str(), false).await;
        db.duplicate_table(&table_name, &table_name_dup)
            .await
            .unwrap();
        let resp2 = db
            .show_table(&table_name_dup, None, None, 1, 100, None)
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

        let query_id = db.open_connection().await.unwrap();
        let table_name = "string_value";

        let _ = db.drop_table(table_name, false).await;

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
        let cols = db.show_table_columns(table_name).await.unwrap();
        println!("{:?}", cols);
        let resp = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        // dup table and test eq
        let table_name_dup = format!("{}_dup", table_name);
        let _ = db.drop_table(&table_name_dup, false).await;
        db.duplicate_table(&table_name, &table_name_dup)
            .await
            .unwrap();
        let resp2 = db
            .show_table(&table_name_dup, None, None, 1, 100, None)
            .await
            .unwrap();
        assert_eq!(resp, resp2);
    }
}
