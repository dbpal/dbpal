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
use tokio::sync::Mutex;
use typescript_type_def::TypeDef;

use crate::datasource_config::RedisDBConfig;
use crate::db::{
    BackendKind, CheckConn, CreateSchemaExtra, DBTrait, ExportKind, ExportQueryResult, Field,
    GetBackendKind, ListTable, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas,
    ShowExtraArgs, SqlExecuteRet, XlsxEdit,
};
use crate::helper::{OffsetLimitIter, create_csv};
use crate::meta_value::MetaValue;

/// https://stackoverflow.com/questions/30611215/redis-how-to-limit-the-return-number-of-keys-command

#[derive(Clone)]
pub struct RedisDB {
    config: RedisDBConfig,
    conn_manager: ConnectionManager,
    pub(crate) query_connections: Arc<DashMap<uuid::Uuid, Arc<Mutex<MultiplexedConnection>>>>,
    created_database: Arc<DashSet<String>>,
}

impl RedisDB {
    pub async fn new(config: RedisDBConfig) -> anyhow::Result<Self> {
        let conn_str = config.make_conn_string();
        log::info!("connect to redis var:{:?}", conn_str);

        let client = redis::Client::open(conn_str.as_str())?;
        let mut conn_config = ConnectionManagerConfig::default();
        if let Some(timeout) = config.conn_timeout {
            conn_config = conn_config.set_connection_timeout(Duration::from_millis(timeout as u64));
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

fn to_string_or_hex(vec: Vec<u8>) -> String {
    let hex = format!("0x{}", hex::encode(vec.as_slice()));
    match String::from_utf8(vec) {
        Ok(x) => x,
        Err(_) => hex,
    }
}
fn to_json_value(x: Vec<u8>) -> serde_json::Value {
    let arr: Vec<_> = x.iter().map(|x| serde_json::Value::from(*x)).collect();
    match String::from_utf8(x) {
        Ok(x) => serde_json::Value::from(x),
        Err(_) => serde_json::Value::Array(arr),
    }
}

fn to_string_key(key: Vec<u8>) -> String {
    match String::from_utf8(key.clone()) {
        Ok(x) => x,
        Err(_) => hex::encode(key.as_slice()),
    }
}

fn escape_string(s: &str) -> String {
    s.replace("'", "\'").replace("\n", "\\n")
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
                let value: Vec<u8> = conn.get(key).await?;
                sender
                    .send(create_csv(&vec![vec![Some(to_string_or_hex(value))]], delimiter).into())
                    .await?;
            }

            RedisType::List => {
                let total: isize = conn.llen(key).await?;
                let size: isize = 100;
                let iter = OffsetLimitIter::new(total, 100);
                for x in iter {
                    let list: Vec<Vec<u8>> = conn.lrange(key, x, x + size).await?;
                    let list = list
                        .into_iter()
                        .map(|x| vec![Some(to_string_or_hex(x))])
                        .collect::<Vec<_>>();
                    sender.send(create_csv(&list, delimiter).into()).await?;
                }
            }

            RedisType::Set => {
                let mut iter = conn.sscan::<_, Vec<u8>>(key).await?.ready_chunks(100);
                while let Some(set) = iter.next().await {
                    let set = set
                        .into_iter()
                        .map(|x| vec![Some(to_string_or_hex(x))])
                        .collect::<Vec<_>>();
                    sender.send(create_csv(&set, delimiter).into()).await?;
                }
            }

            RedisType::Hash => {
                let mut stream = conn.hscan::<_, Vec<u8>>(key).await?.chunks(2);
                while let Some(value) = stream.next().await {
                    sender
                        .send(Cow::Owned(create_csv(
                            &vec![vec![
                                Some(to_string_or_hex(value[0].clone())),
                                Some(to_string_or_hex(value[1].clone())),
                            ]],
                            delimiter,
                        )))
                        .await?;
                }
            }
            RedisType::Zset => {
                let mut stream = conn.zscan::<_, Vec<u8>>(key).await?.chunks(2);
                while let Some(value) = stream.next().await {
                    sender
                        .send(Cow::Owned(create_csv(
                            &vec![vec![
                                Some(to_string_or_hex(value[0].clone())),
                                Some(to_string_or_hex(value[1].clone())),
                            ]],
                            delimiter,
                        )))
                        .await?;
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
                let value: Vec<u8> = conn.get(key).await?;
                let mut m = HashMap::new();
                m.insert(key, to_json_value(value));
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
                    let values: Vec<Vec<u8>> = conn.lrange(key, x, x + size).await?;
                    for value in values {
                        if !first {
                            sender.send(",".into()).await?;
                        }
                        if pretty {
                            sender.send("\n".into()).await?;
                        }
                        let value = to_json_value(value);

                        sender
                            .send(if pretty {
                                Cow::Owned(serde_json::to_string_pretty(&value)?)
                            } else {
                                Cow::Owned(serde_json::to_string(&value)?)
                            })
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
                let mut iter = conn.sscan::<_, Vec<u8>>(key).await?;
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
                    let value = to_json_value(s);

                    sender
                        .send(Cow::Owned(if pretty {
                            serde_json::to_string_pretty(&value)?
                        } else {
                            serde_json::to_string(&value)?
                        }))
                        .await?;
                    first = false;
                }
                if pretty {
                    sender.send("\n".into()).await?;
                }
                sender.send("]".into()).await?;
            }
            RedisType::Hash => {
                let mut stream = conn.hscan::<_, Vec<u8>>(key).await?.chunks(2);
                let mut m = HashMap::new();

                while let Some(value) = stream.next().await {
                    let key = to_string_key(value[0].clone());
                    let value = to_json_value(value[1].clone());
                    m.insert(key, value);
                }
                sender
                    .send(Cow::Owned(if pretty {
                        serde_json::to_string_pretty(&m)?
                    } else {
                        serde_json::to_string(&m)?
                    }))
                    .await?;
            }
            RedisType::Zset => {
                let mut stream = conn.zscan::<_, Vec<u8>>(key).await?.chunks(2);
                let mut m = HashMap::new();
                while let Some(value) = stream.next().await {
                    let key = to_string_key(value[0].clone());
                    let value = to_json_value(value[1].clone());
                    m.insert(key, value);
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
                let value: Vec<u8> = conn.get(key).await?;
                let mut m = HashMap::new();
                m.insert(key, to_json_value(value));
                let value = serde_json::to_string(&m);
                let value = value?;
                sender.send(Cow::Owned(value)).await?;
            }
            RedisType::List => {
                let total: isize = conn.llen(key).await?;
                let size: isize = 100;
                let iter = OffsetLimitIter::new(total, 100);
                for x in iter {
                    let list: Vec<Vec<u8>> = conn.lrange(key, x, x + size).await?;
                    let list = list.into_iter().map(to_json_value).collect::<Vec<_>>();
                    let value = serde_json::value::Value::from(list);
                    let json_str = serde_json::to_string(&value)?;
                    sender.send(json_str.into()).await?;
                    sender.send("\n".into()).await?;
                }
            }
            RedisType::Set => {
                let mut iter = conn.sscan::<_, Vec<u8>>(key).await?.ready_chunks(100);
                while let Some(s) = iter.next().await {
                    let json_value: Vec<_> = s.into_iter().map(to_json_value).collect();
                    let json_str = serde_json::to_string(&json_value)?;
                    sender.send(json_str.into()).await?;
                    sender.send("\n".into()).await?;
                }
            }

            RedisType::Hash => {
                let mut stream = conn.hscan::<_, Vec<u8>>(key).await?.chunks(2);
                while let Some(value) = stream.next().await {
                    let mut m = HashMap::new();
                    let key = to_string_key(value[0].clone());
                    m.insert(key, to_json_value(value[1].clone()));
                    let json_str = serde_json::to_string(&m)?;
                    sender.send(json_str.into()).await?;
                    sender.send("\n".into()).await?;
                }
            }
            RedisType::Zset => {
                let mut stream = conn.zscan::<_, Vec<u8>>(key).await?.chunks(2);

                while let Some(value) = stream.next().await {
                    let mut m = HashMap::new();
                    let key = to_string_key(value[0].clone());
                    m.insert(key, to_json_value(value[1].clone()));
                    let json_str = serde_json::to_string(&m)?;
                    sender.send(json_str.into()).await?;
                    sender.send("\n".into()).await?;
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
                    .send(Cow::Owned(format!(
                        "SET {} '{}'\n",
                        key,
                        escape_string(x.as_str())
                    )))
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
                            .send(Cow::Owned(format!(
                                "RPUSH {} '{}'\n",
                                key,
                                escape_string(l)
                            )))
                            .await?;
                    }
                }
            }
            RedisType::Set => {
                let mut iter = conn.sscan::<_, String>(key).await?;
                while let Some(value) = iter.next().await {
                    sender
                        .send(Cow::Owned(format!(
                            "SADD {} '{}'\n",
                            key,
                            escape_string(value.as_str())
                        )))
                        .await?;
                }
            }

            RedisType::Hash => {
                let mut stream = conn.hscan::<_, String>(key).await?.chunks(2);

                while let Some(values) = stream.next().await {
                    sender
                        .send(Cow::Owned(format!(
                            "HMSET {} {} '{}'\n",
                            key,
                            values[0],
                            escape_string(values[1].as_str())
                        )))
                        .await?;
                }
            }

            RedisType::Zset => {
                let mut stream = conn.zscan::<_, String>(key).await?.chunks(2);
                while let Some(values) = stream.next().await {
                    sender
                        .send(Cow::Owned(format!(
                            "ZADD {} {} '{}'\n",
                            key,
                            values[1],
                            escape_string(values[0].as_str())
                        )))
                        .await?;
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

fn value_2_json(v: &redis::Value) -> serde_json::Value {
    match v {
        Value::Set(vs) => serde_json::Value::Array(vs.iter().map(value_2_json).collect()),
        Value::Array(vs) => serde_json::Value::Array(vs.iter().map(value_2_json).collect()),
        Value::SimpleString(x) => serde_json::Value::from(x.clone()),
        Value::BulkString(x) => match String::from_utf8(x.clone()) {
            Ok(value) => serde_json::Value::from(value),
            Err(_) => serde_json::Value::from(x.to_vec()),
        },

        Value::Map(items) => {
            let mut m = serde_json::Map::new();
            for (k, v) in items.iter() {
                m.insert(encode_response_value_to_string(k), value_2_json(v));
            }
            serde_json::Value::Object(m)
        }
        Value::Attribute {
            data,
            attributes: _, // TODO how to handle this???
        } => value_2_json(data),
        Value::VerbatimString { format, text } => {
            let mut m = serde_json::Map::new();
            m.insert("format".to_string(), format.to_string().into());
            m.insert("text".to_string(), text.to_string().into());
            serde_json::Value::Object(m)
        }
        Value::Nil => serde_json::Value::Null,
        Value::Int(value) => serde_json::Value::from(*value),
        Value::Okay => serde_json::Value::from("Okay"),
        Value::Double(value) => serde_json::Value::from(*value),
        Value::Boolean(value) => serde_json::Value::from(*value),
        Value::BigNumber(value) => serde_json::Value::from(value.to_string()),

        Value::Push { kind, data } => {
            let mut m = serde_json::Map::new();
            m.insert("kind".to_string(), kind.to_string().into());
            m.insert(
                "data".to_string(),
                serde_json::Value::Array(data.iter().map(value_2_json).collect()),
            );
            serde_json::Value::Object(m)
        }
        Value::ServerError(server_error) => {
            let mut m = serde_json::Map::new();
            m.insert("code".into(), server_error.code().into());
            m.insert("detail".to_string(), server_error.details().into());
            serde_json::Value::Object(m)
        }
    }
}

async fn handle_export_xlsx(
    v: &redis::Value, sender: tokio::sync::mpsc::Sender<XlsxEdit>,
) -> anyhow::Result<()> {
    match v {
        Value::Set(vs) => {
            for (index, v) in vs.iter().enumerate() {
                sender
                    .send(XlsxEdit {
                        row: index as u32,
                        column: 0,
                        content: encode_response_value_to_string(v),
                    })
                    .await?;
            }
        }
        Value::Array(vs) => {
            for (index, v) in vs.iter().enumerate() {
                sender
                    .send(XlsxEdit {
                        row: index as u32,
                        column: 0,
                        content: encode_response_value_to_string(v),
                    })
                    .await?;
            }
        }
        Value::SimpleString(x) => {
            sender
                .send(XlsxEdit {
                    row: 0,
                    column: 0,
                    content: x.clone(),
                })
                .await?;
        }
        Value::BulkString(x) => match String::from_utf8(x.clone()) {
            Ok(value) => {
                sender
                    .send(XlsxEdit {
                        row: 0,
                        column: 0,
                        content: value,
                    })
                    .await?
            }
            Err(_) => {
                for (index, e) in x.iter().enumerate() {
                    sender
                        .send(XlsxEdit {
                            row: index as u32,
                            column: 0,
                            content: e.to_string(),
                        })
                        .await?;
                }
            }
        },

        Value::Map(items) => {
            for (index, (k, v)) in items.iter().enumerate() {
                sender
                    .send(XlsxEdit {
                        row: index as u32,
                        column: 0,
                        content: encode_response_value_to_string(k),
                    })
                    .await?;
                sender
                    .send(XlsxEdit {
                        row: index as u32,
                        column: 0,
                        content: encode_response_value_to_string(v),
                    })
                    .await?;
            }
        }
        Value::Attribute {
            data,
            attributes: _, // TODO how to handle this???
        } => {
            Box::pin(handle_export_xlsx(data, sender.clone())).await?;
        }
        Value::VerbatimString { format, text } => {
            sender
                .send(XlsxEdit {
                    row: 0,
                    column: 0,
                    content: format!("{}", format),
                })
                .await?;
            sender
                .send(XlsxEdit {
                    row: 1,
                    column: 0,
                    content: text.clone(),
                })
                .await?;
        }
        Value::Nil => {
            sender
                .send(XlsxEdit {
                    row: 0,
                    column: 0,
                    content: "nil".to_string(),
                })
                .await?
        }
        Value::Int(value) => {
            sender
                .send(XlsxEdit {
                    row: 0,
                    column: 0,
                    content: value.to_string(),
                })
                .await?
        }
        Value::Okay => {
            sender
                .send(XlsxEdit {
                    row: 0,
                    column: 0,
                    content: "Okay".to_string(),
                })
                .await?
        }
        Value::Double(value) => {
            sender
                .send(XlsxEdit {
                    row: 0,
                    column: 0,
                    content: value.to_string(),
                })
                .await?
        }
        Value::Boolean(value) => {
            sender
                .send(XlsxEdit {
                    row: 0,
                    column: 0,
                    content: value.to_string(),
                })
                .await?
        }
        Value::BigNumber(value) => {
            sender
                .send(XlsxEdit {
                    row: 0,
                    column: 0,
                    content: value.to_string(),
                })
                .await?
        }
        Value::Push { .. } => anyhow::bail!("push not supported"),
        Value::ServerError(server_error) => {
            sender
                .send(XlsxEdit {
                    row: 0,
                    column: 0,
                    content: server_error.code().to_string(),
                })
                .await?;
            sender
                .send(XlsxEdit {
                    row: 1,
                    column: 0,
                    content: server_error.details().unwrap_or_default().to_string(),
                })
                .await?
        }
    }
    anyhow::Ok(())
}

impl RedisDB {
    async fn export_query_result_as_value(&self, sql: &str) -> anyhow::Result<redis::Value> {
        let commands = get_command_from_sql(sql)?;
        if commands.len() != 1 {
            anyhow::bail!("Redis does not support multiple commands");
        }
        let c = &commands[0];
        log::info!("cmd is:{:?}", c);
        let client = self.get_client().await?;
        let args = &c[1..];
        let mut conn = client.get_multiplexed_async_connection().await?;
        let value = redis::cmd(c[0].as_str())
            .arg(args)
            .query_async(&mut conn)
            .await?;
        anyhow::Ok(value)
    }
    async fn export_query_result_as_csv(
        &self, sql: &str, include_header: bool, _delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        if include_header {
            anyhow::bail!("don't support export without header")
        }
        let value = self.export_query_result_as_value(sql).await?;
        match value {
            Value::Array(values) | Value::Set(values) => {
                for value in values.iter() {
                    sender
                        .send(encode_response_value_to_string(value).into())
                        .await?;
                    sender.send("\n".into()).await?;
                }
            }
            _ => {
                anyhow::bail!("unsupported data return from the server:{:?}", value)
            }
        }
        anyhow::Ok(())
    }
    async fn export_query_result_as_json(
        &self, sql: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let value = self.export_query_result_as_value(sql).await?;
        let value = value_2_json(&value);
        sender
            .send(Cow::Owned(if pretty {
                serde_json::to_string_pretty(&value)?
            } else {
                serde_json::to_string(&value)?
            }))
            .await?;
        anyhow::Ok(())
    }
    async fn export_query_result_as_jsonline(
        &self, sql: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let value = self.export_query_result_as_value(sql).await?;
        async fn handle(
            value: &Value, sender: &tokio::sync::mpsc::Sender<Cow<'static, str>>,
        ) -> anyhow::Result<()> {
            match &value {
                Value::Nil => {}
                Value::Int(x) => sender.send(x.to_string().into()).await?,
                Value::BulkString(items) => match String::from_utf8(items.to_vec()) {
                    Ok(x) => sender.send(x.into()).await?,
                    Err(_) => {
                        let value = serde_json::Value::Array(
                            items.iter().map(|x| serde_json::Value::from(*x)).collect(),
                        );
                        sender.send(serde_json::to_string(&value)?.into()).await?;
                    }
                },

                Value::Array(values) | Value::Set(values) => {
                    for value in values.iter() {
                        sender
                            .send(serde_json::to_string(&value_2_json(value))?.into())
                            .await?;

                        sender.send("\n".into()).await?;
                    }
                }
                Value::SimpleString(s) => {
                    sender.send(s.clone().into()).await?;
                }
                Value::Okay => sender.send("Okay".into()).await?,
                Value::Map(items) => {
                    for (k, v) in items.iter() {
                        let mut m = HashMap::new();
                        m.insert(encode_response_value_to_string(k), value_2_json(v));
                        sender.send(serde_json::to_string(&m)?.into()).await?;
                        sender.send("\n".into()).await?;
                    }
                }
                Value::Attribute { data, .. } => Box::pin(handle(data, sender)).await?,
                Value::Double(value) => sender.send(value.to_string().into()).await?,
                Value::Boolean(value) => sender.send(value.to_string().into()).await?,
                Value::VerbatimString { format, text } => {
                    let mut m = HashMap::new();
                    m.insert("format", format.to_string());
                    m.insert("text", text.to_string());
                    sender.send(serde_json::to_string(&m)?.into()).await?;
                }
                Value::BigNumber(big_int) => sender.send(big_int.to_string().into()).await?,
                Value::Push { .. } => {
                    anyhow::bail!("push not supported");
                }
                Value::ServerError(server_error) => {
                    let mut m = HashMap::new();
                    m.insert("code", server_error.code());
                    m.insert("detail", server_error.details().unwrap_or_default());
                    sender.send(serde_json::to_string(&m)?.into()).await?;
                }
            }
            anyhow::Ok(())
        }
        handle(&value, &sender).await?;
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]

impl ExportQueryResult for RedisDB {
    async fn export_query_result(
        &self, sql: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::CSV {
                include_header,
                delimiter,
            } => {
                self.export_query_result_as_csv(sql, include_header, delimiter, sender)
                    .await
            }
            ExportKind::Json { pretty } => {
                self.export_query_result_as_json(sql, pretty, sender).await
            }
            ExportKind::JsonLine => self.export_query_result_as_jsonline(sql, sender).await,
            ExportKind::Sql { .. } => ExportKind::cannot_query_result_export_as_sql(),
            ExportKind::Xlsx { .. } => anyhow::bail!("export as xlsx is not supported "),
        }
    }

    async fn export_as_xlsx(
        &self, sql: &str, include_header: bool, sender: tokio::sync::mpsc::Sender<XlsxEdit>,
    ) -> anyhow::Result<()> {
        if include_header {
            anyhow::bail!("Redis does not support exporting header");
        }
        let value = self.export_query_result_as_value(sql).await?;
        handle_export_xlsx(&value, sender.clone()).await?;
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }
}

impl RedisDB {
    pub const MAX_RETURNING: usize = 2000;
}

#[async_trait::async_trait]
impl SchemaTrait for RedisDB {
    async fn export_table_or_view_as_xlsx(
        &self, key: &str, include_header: bool, sender: tokio::sync::mpsc::Sender<XlsxEdit>,
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
                    .send(XlsxEdit {
                        row: 0,
                        column: 0,
                        content: value,
                    })
                    .await?;
            }

            RedisType::List => {
                let total: isize = conn.llen(key).await?;
                for i in 0..total {
                    let value: Vec<u8> = conn.lindex(key, i).await?;
                    sender
                        .send(XlsxEdit {
                            row: i as u32,
                            column: 0,
                            content: to_string_or_hex(value),
                        })
                        .await?;
                }
            }
            RedisType::Set => {
                let mut iter = conn.sscan::<_, Vec<u8>>(key).await?;
                let mut row_num = 0;
                while let Some(set) = iter.next().await {
                    sender
                        .send(XlsxEdit {
                            row: row_num,
                            column: 0,
                            content: to_string_or_hex(set),
                        })
                        .await?;
                    row_num += 1;
                }
            }
            RedisType::Hash => {
                let mut stream: AsyncIter<'_, Vec<u8>> = conn.hscan(key).await?;
                let mut offset = 0;
                while let Some(value) = stream.next().await {
                    sender
                        .send(XlsxEdit {
                            row: offset / 2,
                            column: (offset % 2) as rust_xlsxwriter::ColNum,
                            content: to_string_or_hex(value),
                        })
                        .await?;
                    offset += 1;
                }
            }
            RedisType::Zset => {
                let mut stream: AsyncIter<'_, Vec<u8>> = conn.zscan(key).await?;
                let mut offset = 0;
                while let Some(value) = stream.next().await {
                    sender
                        .send(XlsxEdit {
                            row: offset / 2,
                            column: (offset % 2) as rust_xlsxwriter::ColNum,
                            content: to_string_or_hex(value),
                        })
                        .await?;
                    offset += 1;
                }
            }
        }
        anyhow::Ok(())
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
                        .zip(value_values)
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
        anyhow::bail!("call query sql instead")
    }

    async fn table_update(
        &self, _table_name: &str, _values: HashMap<String, Option<String>>,
        _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        anyhow::bail!("call query sql instead")
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
                    "redis collection type delete all element will cause the key been dropped"
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
                log::warn!("multi resp should be fine");
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

#[derive(Default, Clone, Debug)]
enum RedisTokenState {
    #[default]
    Init,
    SingleQuote,
    DoubleQuote,
    Trans {
        count: usize,
        prev: Box<RedisTokenState>,
    },
    Whitespace,
}

impl RedisTokenState {
    fn trans(&self, count: usize) -> Self {
        Self::Trans {
            count,
            prev: Box::new(self.clone()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum RedisToken {
    String(String),
    NotString {
        line: u64,
        start_column: u64,
        end_column: u64,
        value: String,
    },
}
impl std::fmt::Display for RedisToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                RedisToken::String(x) => x,
                RedisToken::NotString { value, .. } => value,
            }
        )
    }
}

impl RedisToken {
    pub(crate) fn tokenize(line: &str, line_number: u64) -> anyhow::Result<Vec<RedisToken>> {
        let mut buf = vec![];
        let mut tokens = vec![];
        let mut state = RedisTokenState::default();

        fn push_token(
            index: usize, // 这个时候我已经处理完了 要添加到结果集了 所以这个index是加了1的
            tokens: &mut Vec<RedisToken>,
            buf: &mut Vec<u8>,
            line_number: u64,
        ) -> anyhow::Result<()> {
            if buf.is_empty() {
                return anyhow::Ok(());
            }
            let index = index as u64;
            let end_column = index + 1; // 加上自身的长度
            tokens.push(RedisToken::NotString {
                start_column: end_column - (buf.len() as u64),
                end_column,
                line: line_number,
                value: match String::from_utf8(std::mem::take(buf)) {
                    Ok(x) => x,
                    Err(_) => {
                        anyhow::bail!(format!("not a valid utf8 near:{}:{}", line_number, index))
                    }
                },
            });
            anyhow::Ok(())
        }
        let as_bytes = line.as_bytes();
        for (index, c) in as_bytes.iter().enumerate() {
            match state {
                RedisTokenState::Init => match c {
                    b'\'' => {
                        state = RedisTokenState::SingleQuote;
                        push_token(index, &mut tokens, &mut buf, line_number)?;
                    }
                    b'"' => {
                        state = RedisTokenState::DoubleQuote;
                        push_token(index, &mut tokens, &mut buf, line_number)?;
                    }
                    _ if c.is_ascii_whitespace() => {
                        push_token(index, &mut tokens, &mut buf, line_number)?;
                        state = RedisTokenState::Whitespace;
                    }
                    _ => {
                        buf.push(*c);
                    }
                },
                RedisTokenState::SingleQuote => match c {
                    b'\'' => {
                        tokens.push(RedisToken::String(
                            String::from_utf8_lossy(&buf).to_string(),
                        ));
                        buf.clear();
                        state = RedisTokenState::Init;
                    }
                    // try match \n
                    b'\\' if as_bytes.get(index + 1).map(|x| *x == b'n').unwrap_or(false) => {
                        buf.push(b'\n');
                        state = state.trans(1);
                    }
                    b'\\'
                        if as_bytes
                            .get(index + 1)
                            .map(|x| *x == b'\'')
                            .unwrap_or(false) =>
                    {
                        buf.push(b'\'');
                        state = state.trans(1);
                    }
                    _ => {
                        buf.push(*c);
                    }
                },
                RedisTokenState::DoubleQuote => {
                    match c {
                        b'\"' => {
                            tokens.push(RedisToken::String(
                                String::from_utf8_lossy(&buf).to_string(),
                            ));
                            buf.clear();
                            state = RedisTokenState::Init;
                        }
                        // try match \n
                        b'\\' if as_bytes.get(index + 1).map(|x| *x == b'n').unwrap_or(false) => {
                            buf.push(b'\n');
                            state = state.trans(1);
                        }
                        b'\\' if as_bytes.get(index + 1).map(|x| *x == b'"').unwrap_or(false) => {
                            buf.push(b'\"');
                            state = state.trans(1);
                        }
                        _ => {
                            buf.push(*c);
                        }
                    }
                }
                RedisTokenState::Trans { count, prev } => {
                    let x = count - 1;
                    if x == 0 {
                        state = prev.as_ref().clone();
                    } else {
                        state = RedisTokenState::Trans { count: x, prev }
                    }
                }
                RedisTokenState::Whitespace => {
                    if c.is_ascii_whitespace() {
                        // nothing here
                    } else if *c == b'\'' {
                        state = RedisTokenState::SingleQuote;
                    } else if *c == b'"' {
                        state = RedisTokenState::DoubleQuote;
                    } else {
                        buf.push(*c);
                        state = RedisTokenState::Init;
                    }
                }
            }
        }
        match state {
            RedisTokenState::Init => {
                push_token(as_bytes.len(), &mut tokens, &mut buf, line_number)?;
            }
            RedisTokenState::SingleQuote => anyhow::bail!("unclose single quote"),
            RedisTokenState::DoubleQuote => anyhow::bail!("unclose double quote"),
            RedisTokenState::Trans { .. } => anyhow::bail!("this is internal error"),
            RedisTokenState::Whitespace => {
                // nothing here
            }
        }
        anyhow::Ok(tokens)
    }
}

pub(crate) fn get_command_from_sql(sql: &str) -> anyhow::Result<Vec<Vec<String>>> {
    let mut commands = Vec::new();
    for line in sql.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let tokens = RedisToken::tokenize(line, 1)?;
        commands.push(
            tokens
                .into_iter()
                .map(|x| match x {
                    RedisToken::String(value) => value,
                    RedisToken::NotString { value, .. } => value,
                })
                .collect(),
        );
    }
    anyhow::Ok(commands)
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
            .map(|x| encode_response_value(&x));

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
        Value::Nil => "nil".to_string(),
        Value::Array(values) => {
            let x: Vec<_> = values.iter().map(encode_response_value_to_string).collect();
            format!("[{}]", x.join(", "))
        }
        Value::Okay => "Okay".to_string(),
        Value::Map(items) => {
            let x: Vec<_> = items
                .iter()
                .map(|(k, v)| {
                    format!(
                        "{}:{}",
                        encode_response_value_to_string(k),
                        encode_response_value_to_string(v)
                    )
                })
                .collect();
            format!("[{}]", x.join(", "))
        }
        Value::Attribute {
            data,
            attributes: _,
        } => encode_response_value_to_string(data),
        Value::Set(values) => {
            let x: Vec<_> = values.iter().map(encode_response_value_to_string).collect();
            format!("[{}]", x.join(", "))
        }
        Value::VerbatimString { format, text } => format!("type:{} text:{}", format, text),
        Value::Push { kind, data } => {
            let x: Vec<_> = data.iter().map(encode_response_value_to_string).collect();
            format!("kind:{}[{}]", kind, x.join(", "))
        }
        Value::ServerError(server_error) => format!("server error:{:?}", server_error),
    }
}

fn encode_response_value(v: &Value) -> anyhow::Result<QueryResp> {
    let ret = match v {
        Value::Set(vs) => {
            // a set
            let result: Vec<_> = vs
                .iter()
                .map(encode_response_value_to_string)
                .map(Some)
                .collect();
            QueryResp::result(None, result)
        }
        Value::Array(vs) => {
            let result: Vec<_> = vs
                .iter()
                .map(encode_response_value_to_string)
                .map(Some)
                .collect();
            QueryResp::result(None, result)
        }
        Value::SimpleString(x) => QueryResp::result(None, vec![Some(x.to_string())]),
        Value::BulkString(_x) => {
            QueryResp::result(None, vec![Some(encode_response_value_to_string(v))])
        }
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
        } => encode_response_value(data)?,
        Value::VerbatimString { format, text } => {
            let fields = vec![
                Field::from_name_type("format", ""),
                Field::from_name_type("value", ""),
            ];
            let data = vec![vec![Some(format.to_string()), Some(text.clone())]];
            QueryResp::new(fields, 0, data, None)?
        }
        Value::Nil => QueryResp::result(None, vec![]),
        Value::Int(value) => QueryResp::result(None, vec![Some(value.to_string())]),
        Value::Okay => QueryResp::result(None, vec![Some("Okay".to_string())]),
        Value::Double(value) => QueryResp::result(None, vec![Some(value.to_string())]),
        Value::Boolean(value) => QueryResp::result(None, vec![Some(value.to_string())]),
        Value::BigNumber(value) => QueryResp::result(None, vec![Some(value.to_string())]),
        Value::Push { kind, data } => QueryResp {
            fields: vec![Field::from_name_type("kind", kind.to_string())],
            total: 0,
            data: data
                .iter()
                .map(|x| vec![Some(encode_response_value_to_string(x))])
                .collect(),
            sql: None,
            cache_time: None,
            has_more: false,
        },
        Value::ServerError(server_error) => {
            QueryResp::result(None, vec![Some(format!("ServerError: {:?}", server_error))])
        }
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
        conn_timeout: Some(1000),
        save_password: true,
    })
    .await
    .unwrap();
    db
}
#[cfg(test)]
mod tests {
    use super::make_test_object;
    use super::*;
    use super::{RedisDB, extract_db_from_info_key_space, split_info_colon};
    use futures_util::StreamExt;
    use redis::AsyncCommands;
    use std::time::Duration;

    use crate::db::{
        DBTrait, ExportKind, ExportQueryResult, GetBackendKind, QuerySql, SchemaTrait, ServerMetas,
    };
    use crate::enable_test_log;
    use crate::language_server::completion_test::test_file_auto_completion;

    #[cfg(test)]
    #[test]
    fn test_token_xxx() {
        let tokens =
            RedisToken::tokenize(r#"abc xyz ccc 'aaaaaa"\'\n' "bbbbbb'\"\n" aaaa "#, 1).unwrap();
        for t in tokens.iter() {
            println!("token:{:?}", t);
        }

        {
            // error case
            let tokens = RedisToken::tokenize(r#" 'a"#, 1);
            match tokens {
                Ok(_) => panic!("should hit error"),
                Err(x) => assert!(x.to_string().contains("single")),
            }
        }

        {
            // error case
            let tokens = RedisToken::tokenize(r#" "a"#, 1);
            match tokens {
                Ok(_) => panic!("should hit error"),
                Err(x) => assert!(x.to_string().contains("double")),
            }
        }
    }

    #[tokio::test]
    async fn test_select_zset_binary() {
        let db = make_test_object().await;
        let mut conn = db.get_conn().await.unwrap();
        let mut x: redis::AsyncIter<'_, Vec<u8>> = conn.zscan("zset_value").await.unwrap();
        while let Some(val) = x.next().await {
            println!("{:?}", val);
        }
    }

    async fn test_query_export(db: &RedisDB, cmd: &str, key: &str, kind: ExportKind) {
        {
            // xlsx
            let name = db.backend_name().make_test_xlsx_file(key);
            db.export_query_result_to_file(cmd, kind, &name)
                .await
                .unwrap();
            opener::open(name.as_str()).unwrap();
        }
    }

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("aaa'",), "aaa\'");
        assert_eq!(
            escape_string(
                r#"aaa'
"#
            ),
            "aaa\'\\n"
        );
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
        let db_names = db.list_schema().await.unwrap();
        for db_name in db_names.iter() {
            println!("db:{:?}", db_names);
            db.open_schema(&db_name).await.unwrap();
        }

        db.drop_schema("db11").await.unwrap();
    }
    #[ignore = "run me by hand"]
    #[tokio::test]
    async fn test_check_a_lot_of_key() {
        let db = make_test_object().await;
        crate::db::CheckConn::check_conn(&db).await.unwrap();
        let query_id = db.open_connection().await.unwrap();
        for i in 0..100000 {
            db.query_sql(
                query_id,
                format!("set test_a_lot_of_key{} {}", i, i).as_str(),
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
        enable_test_log();
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

        test_query_export(
            &db,
            format!("zrange {table_name} 0 -1 WITHSCORES").as_str(),
            table_name,
            ExportKind::Xlsx {
                include_header: false,
            },
        )
        .await;
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
