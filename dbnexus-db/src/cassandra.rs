/*
出现错误 InvalidRequest: Error from server: code=2200 [Invalid query] message="org.apache.cassandra.auth.CassandraRoleManager doesn't support PASSWORD" 的原因是 Cassandra 的身份验证机制未启用，或者配置不支持密码身份验证
。
要解决这个问题，请按照以下步骤操作：
1. 修改 cassandra.yaml 配置文件
默认情况下，Cassandra 使用 AllowAllAuthenticator，这表示禁用了身份验证。你需要将其改为 PasswordAuthenticator，以启用基于密码的身份验证
。

    打开 cassandra.yaml 文件（通常位于 /etc/cassandra/conf/cassandra.yaml 或 Cassandra 安装目录的 conf 文件夹中）。
    找到以下行：
    yaml复制

    authenticator: AllowAllAuthenticator

    将其更改为：
    yaml复制

    authenticator: PasswordAuthenticator

    保存文件并重启 Cassandra 服务

    InvalidRequest: Error from server: code=2200 [Invalid query] message="User-defined functions are disabled in cassandra.yaml - set user_defined_functions_enabled=true to enable"

*/

use std::{borrow::Cow, collections::HashMap, sync::Arc, time::Duration};

use cdrs_tokio::{
    authenticators::StaticPasswordAuthenticatorProvider,
    cluster::{NodeTcpConfigBuilder, session::TcpSessionBuilder},
    frame::message_result::{ColType, RowsMetadata},
    load_balancing::RoundRobinLoadBalancingStrategy,
    types::{
        AsCassandraType, IntoRustByIndex, blob::Blob, cassandra_type::CassandraType, list::List,
        map::Map, prelude::Decimal, rows::Row, tuple::Tuple, udt::Udt,
    },
};
use chrono::{DateTime, NaiveDate, NaiveTime};
use cql3_parser::{
    cassandra_ast::CassandraAST,
    cassandra_statement::CassandraStatement,
    common::{ColumnDefinition, DataType},
};
use dashmap::DashMap;

use num_bigint::{BigInt, ToBigInt};

use crate::{
    datasource_config::CassandraDBConfig,
    db::{
        BackendName, CheckConn, ColumnType, CreateSchemaExtra, CreateTableAndIndexSql, CreatedType,
        DBTrait, DefAndMetas, ExportKind, ExportQueryResult, Field, Index, IndexKind,
        ListMaterializedView, ListTable, NamedStaff, QueryResp, QuerySql, QuoteStyle,
        ReadOnlyTrait, SchemaTrait, ServerMetas, ShowExtraArgs, SqlBuilder, SqlExecuteRet,
        TableListResp, UdfOrProcedureExtraParameter,
    },
    helper::create_csv,
    language_server::{LangServerCommon, LexerCompletionTrait},
    meta_value::MetaValue,
    query_log::QueryLog,
};
use cdrs_tokio::cluster::session::SessionBuilder;

#[derive(Clone)]
pub struct CassandraDB {
    pub(crate) session: Arc<
        cdrs_tokio::cluster::session::Session<
            cdrs_tokio::transport::TransportTcp,
            cdrs_tokio::cluster::TcpConnectionManager,
            RoundRobinLoadBalancingStrategy<
                cdrs_tokio::transport::TransportTcp,
                cdrs_tokio::cluster::TcpConnectionManager,
            >,
        >,
    >,
    pub(crate) config: CassandraDBConfig,
    paging_cache: Arc<DashMap<String, Arc<PageCache>>>,
}

#[derive(Clone, Debug)]
struct PageCache {
    result: QueryResp,
    condition: PageCacheCondition,
    sql: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PageCacheCondition {
    where_str: Option<String>,
    order_by: Option<String>,
    extra: Option<ShowExtraArgs>,
}

impl PageCache {
    fn page(&self, page: usize, page_size: usize) -> QueryResp {
        let length = self.result.data.len();
        let start = (page - 1) * page_size;
        if start >= length {
            return QueryResp {
                fields: self.result.fields.clone(),
                data: vec![],
            };
        }
        let end = page * page_size;
        let end = std::cmp::min(end, length);
        let data = (&self.result.data[start..end]).to_vec();

        QueryResp {
            fields: self.result.fields.clone(),
            data,
        }
    }
}

impl CassandraDB {
    pub async fn new(config: CassandraDBConfig) -> anyhow::Result<Self> {
        let mut node_builder = NodeTcpConfigBuilder::new();

        node_builder = node_builder
            .with_contact_point(format!("{}:{}", config.host, config.port.unwrap_or(9042)).into());

        if config.username.is_some() && config.password.is_some() {
            let a = StaticPasswordAuthenticatorProvider::new(
                config.username.as_deref().unwrap(),
                config.password.as_deref().unwrap(),
            );
            node_builder = node_builder.with_authenticator_provider(Arc::new(a))
        }

        let cluster_config = node_builder.build().await?;

        let mut session_builder =
            TcpSessionBuilder::new(RoundRobinLoadBalancingStrategy::new(), cluster_config);
        if let Some(keyspace) = config.db.as_deref() {
            session_builder = session_builder.with_keyspace(keyspace.to_owned());
        }

        let session: cdrs_tokio::cluster::session::Session<
            cdrs_tokio::transport::TransportTcp,
            cdrs_tokio::cluster::TcpConnectionManager,
            RoundRobinLoadBalancingStrategy<
                cdrs_tokio::transport::TransportTcp,
                cdrs_tokio::cluster::TcpConnectionManager,
            >,
        > = session_builder.build().await?;

        anyhow::Ok(Self {
            session: Arc::new(session),
            config,
            paging_cache: Default::default(),
        })
    }
}

#[async_trait::async_trait]

impl ServerMetas for CassandraDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        anyhow::Ok(vec![])
    }
}

#[async_trait::async_trait]
impl CheckConn for CassandraDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let now = std::time::Instant::now();
        self.session
            .query("SELECT now() FROM system.local;")
            .await?
            .response_body()?;
        anyhow::Ok(now.elapsed())
    }
}

impl std::fmt::Debug for CassandraDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "host={} port={:?}", self.config.host, self.config.port)
    }
}

impl BackendName for CassandraDB {
    fn backend_name(&self) -> &str {
        "cassandra"
    }
}

fn infer_fields(meta: &RowsMetadata) -> Vec<Field> {
    meta.col_specs
        .iter()
        .map(|x| Field::from_name_type(x.name.clone(), x.col_type.id.to_string()))
        .collect()
}

fn encode_rows_as_json(
    meta: &RowsMetadata, rows: &Vec<Row>,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut ret = vec![];
    for row in rows.iter() {
        let mut data = serde_json::Map::new();

        for index in 0..meta.col_specs.len() {
            let ty = &meta.col_specs[index];

            let value = match ty.col_type.id {
                ColType::Custom => {
                    anyhow::bail!("not support custom datatype")
                }
                ColType::Ascii => {
                    let value: Option<String> = row.get_by_index(index)?;
                    value.map(serde_json::Value::from)
                }
                ColType::Bigint => {
                    let value: Option<i64> = row.get_by_index(index)?;
                    value.map(serde_json::Value::from)
                }
                ColType::Blob => {
                    let value: Option<Blob> = row.get_by_index(index)?;
                    value
                        .map(|x| x.into_vec())
                        .map(|x| format!("0x{}", hex::encode(&x[..])))
                        .map(serde_json::Value::from)
                }
                ColType::Boolean => {
                    let value: Option<bool> = row.get_by_index(index)?;
                    value.map(serde_json::Value::from)
                }
                ColType::Counter => {
                    let value: Option<i64> = row.get_by_index(index)?;
                    value.map(serde_json::Value::from)
                }

                ColType::Decimal => {
                    let value: Option<Decimal> = row.get_by_index(index)?;
                    if let Some(value) = value {
                        Some(format_decimal(&value))
                    } else {
                        None
                    }
                    .map(serde_json::Value::from)
                }
                ColType::Double => {
                    let value: Option<f64> = row.get_by_index(index)?;
                    value.map(serde_json::Value::from)
                }
                ColType::Float => {
                    let value: Option<f32> = row.get_by_index(index)?;
                    value.map(serde_json::Value::from)
                }
                ColType::Int => {
                    let value: Option<i32> = row.get_by_index(index)?;
                    value.map(serde_json::Value::from)
                }
                ColType::Timestamp => {
                    let value: Option<i64> = row.get_by_index(index)?;
                    if let Some(value) = value {
                        Some(format_timestamp(value)?)
                    } else {
                        None
                    }
                    .map(serde_json::Value::from)
                }
                ColType::Uuid => {
                    let value: Option<uuid::Uuid> = row.get_by_index(index)?;
                    value.map(|x| x.to_string()).map(serde_json::Value::from)
                }
                ColType::Varchar => {
                    let value: Option<String> = row.get_by_index(index)?;
                    value.map(serde_json::Value::from)
                }
                ColType::Varint => {
                    let value: Option<BigInt> = row.get_by_index(index)?;
                    value.map(|x| x.to_string()).map(serde_json::Value::from)
                }
                ColType::Timeuuid => {
                    let value: Option<uuid::Uuid> = row.get_by_index(index)?;
                    value.map(|x| x.to_string()).map(serde_json::Value::from)
                }
                ColType::Inet => {
                    let value: Option<core::net::IpAddr> = row.get_by_index(index)?;
                    value.map(|x| x.to_string()).map(serde_json::Value::from)
                }
                ColType::Date => {
                    //
                    let value: Option<i32> = row.get_by_index(index)?;
                    if let Some(value) = value {
                        let x = format_date(value)?;
                        Some(x)
                    } else {
                        None
                    }
                    .map(serde_json::Value::from)
                }
                ColType::Time => {
                    let value: Option<i64> = row.get_by_index(index)?;
                    value.map(|x| format_time(x)).map(serde_json::Value::from)
                }
                ColType::Smallint => {
                    let value: Option<i16> = row.get_by_index(index)?;
                    value.map(serde_json::Value::from)
                }
                ColType::Tinyint => {
                    let value: Option<i8> = row.get_by_index(index)?;
                    value.map(serde_json::Value::from)
                }
                ColType::Duration => {
                    anyhow::bail!("not support duration datatype")
                }
                ColType::List => {
                    let x: Option<List> = row.get_by_index(index)?;
                    match x {
                        Some(list) => match list.as_cassandra_type()? {
                            Some(CassandraType::List(mut vs)) => {
                                Some(serde_json::Value::Array(encode_list(&mut vs)?))
                            }
                            _ => anyhow::bail!("should be able to cast to list"),
                        },
                        None => None,
                    }
                }

                ColType::Map => {
                    let x: Option<Map> = row.get_by_index(index)?;
                    match x {
                        Some(map) => match map.as_cassandra_type()? {
                            Some(CassandraType::Map(mut vs)) => {
                                Some(serde_json::Value::Object(encode_map(&mut vs)?))
                            }
                            _ => anyhow::bail!("should be able to cast to map"),
                        },
                        None => None,
                    }
                }

                ColType::Set => {
                    let x: Option<List> = row.get_by_index(index)?;
                    match x {
                        Some(list) => match list.as_cassandra_type()? {
                            Some(CassandraType::List(mut vs)) => {
                                Some(serde_json::Value::Array(encode_tuple(&mut vs)?))
                            }
                            _ => anyhow::bail!("should be able to cast to list"),
                        },
                        None => None,
                    }
                }
                ColType::Udt => {
                    let x: Option<Udt> = row.get_by_index(index)?;
                    match x {
                        Some(map) => match map.as_cassandra_type()? {
                            Some(CassandraType::Udt(mut vs)) => {
                                Some(serde_json::Value::Object(encode_udf(&mut vs)?))
                            }
                            _ => anyhow::bail!("should be able to cast to map"),
                        },
                        None => None,
                    }
                }
                ColType::Tuple => {
                    let x: Option<Tuple> = row.get_by_index(index)?;
                    match x {
                        Some(map) => match map.as_cassandra_type()? {
                            Some(CassandraType::Tuple(mut vs)) => {
                                Some(serde_json::Value::Array(encode_tuple(&mut vs)?))
                            }
                            _ => anyhow::bail!("should be able to cast to tuple"),
                        },
                        None => None,
                    }
                }
                _ => anyhow::bail!(format!("type '{:?}' not handled", ty)),
            };
            data.insert(
                meta.col_specs[index].name.clone(),
                value.unwrap_or(serde_json::Value::Null),
            );
        }
        ret.push(serde_json::Value::Object(data));
    }

    return anyhow::Ok(ret);

    fn encode_list(vs: &mut Vec<CassandraType>) -> anyhow::Result<Vec<serde_json::Value>> {
        let mut ret = Vec::with_capacity(vs.len());
        for v in vs.iter_mut() {
            ret.push(encode_rows_canssrandra_type(v)?);
        }
        anyhow::Ok(ret)
    }

    fn encode_tuple(vs: &mut Vec<CassandraType>) -> anyhow::Result<Vec<serde_json::Value>> {
        encode_list(vs)
    }

    fn encode_map(
        vs: &mut Vec<(CassandraType, CassandraType)>,
    ) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
        let mut ret = serde_json::Map::new();

        for v in vs.iter_mut() {
            let key = encode_rows_canssrandra_type(&mut v.0)?;
            let key = serde_json::to_string(&key)?;
            ret.insert(key, encode_rows_canssrandra_type(&mut v.1)?);
        }
        anyhow::Ok(ret)
    }
    fn encode_set(vs: &mut Vec<CassandraType>) -> anyhow::Result<Vec<serde_json::Value>> {
        encode_list(vs)
    }
    fn encode_udf(
        hash_map: &mut HashMap<String, CassandraType>,
    ) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
        let mut ret = serde_json::Map::new();
        for mut v in hash_map.iter_mut() {
            ret.insert(v.0.clone(), encode_rows_canssrandra_type(&mut v.1)?);
        }
        anyhow::Ok(ret)
    }
    fn encode_rows_canssrandra_type(
        value: &mut CassandraType,
    ) -> anyhow::Result<serde_json::Value> {
        let ret = match value {
            CassandraType::Ascii(x) => serde_json::Value::from(x.to_string()),
            CassandraType::Bigint(x) => serde_json::Value::from(x.to_string()),
            CassandraType::Blob(blob) => {
                serde_json::Value::from(format!("0x{}", hex::encode(blob.as_mut_slice())))
            }
            CassandraType::Boolean(x) => serde_json::Value::from(*x),
            CassandraType::Counter(x) => serde_json::Value::from(*x),
            CassandraType::Decimal(decimal) => serde_json::Value::from(format_decimal(decimal)),
            CassandraType::Double(x) => serde_json::Value::from(*x),
            CassandraType::Float(x) => serde_json::Value::from(*x),
            CassandraType::Int(x) => serde_json::Value::from(*x),
            CassandraType::Timestamp(x) => serde_json::Value::from(format_timestamp(*x)?),
            CassandraType::Uuid(uuid) => serde_json::Value::from(uuid.to_string()),
            CassandraType::Varchar(x) => serde_json::Value::from(x.to_string()),
            CassandraType::Varint(big_int) => serde_json::Value::from(big_int.to_string()),
            CassandraType::Timeuuid(uuid) => serde_json::Value::from(uuid.to_string()),
            CassandraType::Inet(ip_addr) => serde_json::Value::from(ip_addr.to_string()),
            CassandraType::Date(x) => serde_json::Value::from(format_date(*x)?),
            CassandraType::Time(x) => serde_json::Value::from(format_time(*x)),
            CassandraType::Smallint(x) => serde_json::Value::from(*x),
            CassandraType::Tinyint(x) => serde_json::Value::from(*x),
            CassandraType::Duration(_) => anyhow::bail!("duration type not support"),
            CassandraType::List(cassandra_types) => {
                serde_json::Value::Array(encode_list(cassandra_types)?)
            }
            CassandraType::Map(items) => serde_json::Value::Object(encode_map(items)?),
            CassandraType::Set(cassandra_types) => {
                serde_json::Value::Array(encode_set(cassandra_types)?)
            }
            CassandraType::Udt(hash_map) => serde_json::Value::Object(encode_udf(hash_map)?),
            CassandraType::Tuple(cassandra_types) => {
                serde_json::Value::Array(encode_tuple(cassandra_types)?)
            }
            CassandraType::Vector(cassandra_types) => {
                serde_json::Value::Array(encode_list(cassandra_types)?)
            }
            CassandraType::Null => serde_json::Value::Null,
            _ => anyhow::bail!(format!("{:?} not handle", value)),
        };
        anyhow::Ok(ret)
    }
}

fn encode_rows(meta: &RowsMetadata, rows: &Vec<Row>) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let mut ret = vec![];
    for row in rows.iter() {
        let mut data = vec![];

        for index in 0..meta.col_specs.len() {
            let ty = &meta.col_specs[index];

            let value = match ty.col_type.id {
                ColType::Custom => {
                    anyhow::bail!("not support custom datatype")
                }
                ColType::Ascii => {
                    let value: Option<String> = row.get_by_index(index)?;
                    value
                }
                ColType::Bigint => {
                    let value: Option<i64> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Blob => {
                    let value: Option<Blob> = row.get_by_index(index)?;
                    value
                        .map(|x| x.into_vec())
                        .map(|x| format!("0x{}", hex::encode(&x[..])))
                }
                ColType::Boolean => {
                    let value: Option<bool> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Counter => {
                    let value: Option<i64> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Decimal => {
                    let value: Option<Decimal> = row.get_by_index(index)?;
                    if let Some(value) = value {
                        Some(format_decimal(&value))
                    } else {
                        None
                    }
                }
                ColType::Double => {
                    let value: Option<f64> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Float => {
                    let value: Option<f32> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Int => {
                    let value: Option<i32> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Timestamp => {
                    let value: Option<i64> = row.get_by_index(index)?;
                    if let Some(value) = value {
                        Some(format_timestamp(value)?)
                    } else {
                        None
                    }
                }
                ColType::Uuid => {
                    let value: Option<uuid::Uuid> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Varchar => {
                    let value: Option<String> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Varint => {
                    let value: Option<BigInt> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Timeuuid => {
                    let value: Option<uuid::Uuid> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Inet => {
                    let value: Option<core::net::IpAddr> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Date => {
                    //
                    let value: Option<i32> = row.get_by_index(index)?;
                    if let Some(value) = value {
                        let x = format_date(value)?;
                        Some(x)
                    } else {
                        None
                    }
                }
                ColType::Time => {
                    let value: Option<i64> = row.get_by_index(index)?;
                    value.map(|x| format_time(x))
                }
                ColType::Smallint => {
                    let value: Option<i16> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Tinyint => {
                    let value: Option<i8> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Duration => {
                    anyhow::bail!("not support duration datatype")
                }
                ColType::List => {
                    let x: Option<List> = row.get_by_index(index)?;
                    match x {
                        Some(list) => match list.as_cassandra_type()? {
                            Some(CassandraType::List(mut vs)) => Some(encode_list(&mut vs)?),
                            _ => anyhow::bail!("should be able to cast to list"),
                        },
                        None => None,
                    }
                }

                ColType::Map => {
                    let x: Option<Map> = row.get_by_index(index)?;
                    match x {
                        Some(map) => match map.as_cassandra_type()? {
                            Some(CassandraType::Map(mut vs)) => Some(encode_map(&mut vs)?),
                            _ => anyhow::bail!("should be able to cast to map"),
                        },
                        None => None,
                    }
                }

                ColType::Set => {
                    let x: Option<List> = row.get_by_index(index)?;
                    match x {
                        Some(list) => match list.as_cassandra_type()? {
                            Some(CassandraType::List(mut vs)) => Some(encode_tuple(&mut vs)?),
                            _ => anyhow::bail!("should be able to cast to list"),
                        },
                        None => None,
                    }
                }
                ColType::Udt => {
                    let x: Option<Udt> = row.get_by_index(index)?;
                    match x {
                        Some(map) => match map.as_cassandra_type()? {
                            Some(CassandraType::Udt(mut vs)) => Some(encode_udf(&mut vs)?),
                            _ => anyhow::bail!("should be able to cast to map"),
                        },
                        None => None,
                    }
                }
                ColType::Tuple => {
                    let x: Option<Tuple> = row.get_by_index(index)?;
                    match x {
                        Some(map) => match map.as_cassandra_type()? {
                            Some(CassandraType::Tuple(mut vs)) => Some(encode_tuple(&mut vs)?),
                            _ => anyhow::bail!("should be able to cast to tuple"),
                        },
                        None => None,
                    }
                }
                _ => anyhow::bail!(format!("type '{:?}' not handled", ty)),
            };
            data.push(value);
        }

        ret.push(data);
    }

    return anyhow::Ok(ret);

    fn encode_list(vs: &mut Vec<CassandraType>) -> anyhow::Result<String> {
        let mut ret = Vec::with_capacity(vs.len());
        for v in vs.iter_mut() {
            ret.push(encode_rows_canssrandra_type(v)?);
        }
        anyhow::Ok(format!("[{}]", ret.join(",")))
    }
    fn encode_tuple(vs: &mut Vec<CassandraType>) -> anyhow::Result<String> {
        let mut ret = Vec::with_capacity(vs.len());
        for v in vs.iter_mut() {
            ret.push(encode_rows_canssrandra_type(v)?);
        }
        anyhow::Ok(format!("({})", ret.join(",")))
    }

    fn encode_map(vs: &mut Vec<(CassandraType, CassandraType)>) -> anyhow::Result<String> {
        let mut ret = Vec::with_capacity(vs.len());
        for v in vs.iter_mut() {
            ret.push(format!(
                "{}:{}",
                encode_rows_canssrandra_type(&mut v.0)?,
                encode_rows_canssrandra_type(&mut v.1)?,
            ));
        }
        anyhow::Ok(format!("{{{}}}", ret.join(",")))
    }
    fn encode_set(vs: &mut Vec<CassandraType>) -> anyhow::Result<String> {
        let mut ret = Vec::with_capacity(vs.len());
        for v in vs.iter_mut() {
            ret.push(encode_rows_canssrandra_type(v)?);
        }
        anyhow::Ok(format!("{{{}}}", ret.join(",")))
    }
    fn encode_udf(hash_map: &mut HashMap<String, CassandraType>) -> anyhow::Result<String> {
        let mut ret = Vec::with_capacity(hash_map.len());
        for v in hash_map.iter_mut() {
            ret.push(format!("{}:{}", v.0, encode_rows_canssrandra_type(v.1)?));
        }
        anyhow::Ok(format!("{{{}}}", ret.join(",")))
    }

    fn encode_rows_canssrandra_type(value: &mut CassandraType) -> anyhow::Result<String> {
        let ret = match value {
            CassandraType::Ascii(x) => format!("'{}'", x),
            CassandraType::Bigint(x) => x.to_string(),
            CassandraType::Blob(blob) => format!("0x{}", hex::encode(blob.as_mut_slice())),
            CassandraType::Boolean(x) => x.to_string(),
            CassandraType::Counter(x) => x.to_string(),
            CassandraType::Decimal(decimal) => format_decimal(decimal),
            CassandraType::Double(x) => x.to_string(),
            CassandraType::Float(x) => x.to_string(),
            CassandraType::Int(x) => x.to_string(),
            CassandraType::Timestamp(x) => format!("'{}'", format_timestamp(*x)?),
            CassandraType::Uuid(uuid) => uuid.to_string(),
            CassandraType::Varchar(x) => format!("'{}'", x),
            CassandraType::Varint(big_int) => big_int.to_string(),
            CassandraType::Timeuuid(uuid) => uuid.to_string(),
            CassandraType::Inet(ip_addr) => format!("'{}'", ip_addr.to_string()),
            CassandraType::Date(x) => format!("'{}'", format_date(*x)?),
            CassandraType::Time(x) => format!("'{}'", format_time(*x)),
            CassandraType::Smallint(x) => x.to_string(),
            CassandraType::Tinyint(x) => x.to_string(),
            CassandraType::Duration(_) => anyhow::bail!("duration type not support"),
            CassandraType::List(cassandra_types) => encode_list(cassandra_types)?,
            CassandraType::Map(items) => encode_map(items)?,
            CassandraType::Set(cassandra_types) => encode_set(cassandra_types)?,
            CassandraType::Udt(hash_map) => encode_udf(hash_map)?,
            CassandraType::Tuple(cassandra_types) => encode_tuple(cassandra_types)?,
            CassandraType::Vector(cassandra_types) => encode_list(cassandra_types)?,
            CassandraType::Null => "null".to_string(),
            _ => anyhow::bail!(format!("{:?} not handle", value)),
        };
        anyhow::Ok(ret)
    }
}

fn format_decimal(x: &Decimal) -> String {
    let r = x.unscaled.clone() % 10i32.pow(x.scale as u32).to_bigint().unwrap();

    format!(
        "{}{}",
        x.as_plain().to_string(),
        if r == BigInt::from(0) {
            "".to_string()
        } else {
            format!(".{}", r)
        }
    )
}

#[cfg(test)]
#[test]
fn test_format_decimal() {
    println!("{}", format_decimal(&Decimal::from(123)));
    println!("{}", format_decimal(&Decimal::from(123.456)));
}

fn format_timestamp(x: i64) -> anyhow::Result<String> {
    let time = DateTime::from_timestamp_millis(x);
    match time {
        Some(x) => anyhow::Ok(x.naive_local().to_string()),
        None => Err(anyhow::Error::msg(format!(""))),
    }
}

fn format_time(x: i64) -> String {
    let time = NaiveTime::default() + std::time::Duration::from_micros(x as u64);
    time.to_string()
}
fn format_date(mut days_since_epoch: i32) -> anyhow::Result<String> {
    // see   https://github.com/krojew/cdrs-tokio/issues/202
    days_since_epoch &= !(1 << 31);

    let date = NaiveDate::from_ymd_opt(1970, 1, 1)
        .unwrap()
        .checked_add_days(chrono::Days::new(days_since_epoch as i64 as u64));
    match date {
        Some(x) => anyhow::Ok(x.to_string()),

        None => anyhow::bail!("days is none"),
    }
}

fn sql_to_each(sql: &str) -> Vec<&str> {
    CassandraAST::new(&sql)
        .statements
        .iter()
        .map(|x| &sql[x.start_byte..x.end_byte])
        .collect()
}

#[async_trait::async_trait]

impl ExportQueryResult for CassandraDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }

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
            ExportKind::JsonLine => self.export_query_result_as_json_line(sql, sender).await,
            ExportKind::Sql {
                include_table_schema: _,
            } => ExportKind::cannot_query_result_export_as_sql(),
        }
    }
}
#[async_trait::async_trait]
impl QuerySql for CassandraDB {
    async fn execute_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        self.invalid_page_caching(None);
        let mut query_log = QueryLog::get_guard(query_id, sql);

        let ret = match self.session.query(sql).await {
            Ok(_) => Ok(QueryResp::execute_ok()),
            Err(err) => Err(err.to_string()),
        };
        query_log.set_success(ret.is_ok());
        anyhow::Ok(ret)
    }

    async fn query_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        self.invalid_page_caching(None);
        let mut query_log = QueryLog::get_guard(query_id, sql);
        let mut ret = vec![];
        for sql in sql_to_each(&sql) {
            match self.session.query(&sql).await {
                Ok(x) => {
                    let body = x.response_body()?;
                    let meta = body.as_rows_metadata().map(|x| x.clone());
                    let rows = body.into_rows().unwrap_or_default();
                    if meta.is_some() {
                        let meta = meta.unwrap();
                        let fields = infer_fields(&meta);
                        let data = encode_rows(&meta, &rows)?;
                        ret.push(Ok(QueryResp { fields, data }));
                    }
                }
                Err(err) => {
                    ret.push(Err(err.to_string()));
                }
            }
        }
        query_log.set_success(ret.iter().all(|x| x.is_ok()));
        anyhow::Ok(ret)
    }
    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        anyhow::Ok(uuid::Uuid::new_v4())
    }

    async fn re_connect(&self, _id: uuid::Uuid) -> anyhow::Result<()> {
        anyhow::Ok(())
    }

    async fn close_connection(&self, _id: uuid::Uuid) -> anyhow::Result<()> {
        anyhow::Ok(())
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        let ast = CassandraAST::new(sql);
        for stmt in ast.statements.iter() {
            let x = match &stmt.statement {
                CassandraStatement::AlterKeyspace(..) => true,
                CassandraStatement::AlterMaterializedView(..) => true,
                CassandraStatement::AlterRole(..) => true,
                CassandraStatement::AlterTable(..) => true,
                CassandraStatement::AlterType(..) => true,
                CassandraStatement::AlterUser(..) => true,
                CassandraStatement::ApplyBatch => false,
                CassandraStatement::CreateAggregate(..) => false,
                CassandraStatement::CreateFunction(..) => false,
                CassandraStatement::CreateIndex(..) => false,
                CassandraStatement::CreateKeyspace(..) => false,
                CassandraStatement::CreateMaterializedView(..) => false,
                CassandraStatement::CreateRole(..) => false,
                CassandraStatement::CreateTable(..) => false,
                CassandraStatement::CreateTrigger(..) => false,
                CassandraStatement::CreateType(..) => false,
                CassandraStatement::CreateUser(..) => false,
                CassandraStatement::Delete(..) => false,
                CassandraStatement::DropAggregate(..) => true,
                CassandraStatement::DropFunction(..) => true,
                CassandraStatement::DropIndex(..) => true,
                CassandraStatement::DropKeyspace(..) => true,
                CassandraStatement::DropMaterializedView(..) => true,
                CassandraStatement::DropRole(..) => true,
                CassandraStatement::DropTable(..) => true,
                CassandraStatement::DropTrigger(..) => true,
                CassandraStatement::DropType(..) => true,
                CassandraStatement::DropUser(..) => true,
                CassandraStatement::Grant(..) => true,
                CassandraStatement::Insert(..) => false,
                CassandraStatement::ListPermissions(..) => false,
                CassandraStatement::ListRoles(..) => false,
                CassandraStatement::Revoke(..) => false,
                CassandraStatement::Select(..) => false,
                CassandraStatement::Truncate(..) => true,
                CassandraStatement::Update(..) => false,
                CassandraStatement::Use(..) => false,
                CassandraStatement::Unknown(_) => false,
            };
            if x {
                return anyhow::Ok(x);
            }
        }
        anyhow::Ok(false)
    }
}

impl CassandraDB {
    pub(crate) async fn list_table_impl(&self, keyspace: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            "SELECT table_name FROM system_schema.tables WHERE keyspace_name = '{keyspace}' order by table_name;"
        );
        log::info!("sql:{}", sql);
        let body = self.session.query(sql).await?.response_body()?;
        let mut ret = Vec::new();
        for row in body.into_rows().unwrap_or_default() {
            let name: Option<String> = row.get_by_index(0)?;
            ret.push(name.unwrap());
        }

        anyhow::Ok(ret.into_iter().map(|x| ListTable::from(x)).collect())
    }
}

impl CassandraDB {
    // page caching size
    const PAGE_CACHE_LIMIG_SIZE: usize = 2000;
    fn invalid_page_caching(&self, table: Option<&str>) {
        if let Some(table) = table {
            self.paging_cache.remove(&table.to_string());
        } else {
            self.paging_cache.clear();
        }
    }

    fn page_caching_totoal(&self, name: &str) -> usize {
        self.paging_cache
            .get(&name.to_string())
            .map(|x| x.result.data.len())
            .unwrap_or(usize::MAX)
    }
    fn page_caching_sql(&self, name: &str) -> Option<String> {
        self.paging_cache
            .get(&name.to_string())
            .map(|x| x.sql.clone())
    }

    async fn page_caching_show(
        &self, name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        // try get value from cache
        let condition = PageCacheCondition {
            where_str: where_str.map(|x| x.to_owned()),
            order_by: order_by.map(|x| x.to_owned()),
            extra: extra.clone(),
        };

        {
            let cache = self.paging_cache.get(&name.to_string()).map(|x| x.clone());
            if let Some(cache) = cache {
                if cache.condition.eq(&condition) {
                    // hit cache
                    log::info!(
                        "hit cache for table:{} page:{} page_size:{}",
                        name,
                        page,
                        page_size
                    );
                    return anyhow::Ok(cache.page(page, page_size));
                }
            }
        }
        //
        log::info!(
            "cache not hit table:{} page:{} page_size:{},trying load data from server",
            name,
            page,
            page_size
        );
        let allow_filtering = match &extra {
            Some(x) => match x {
                ShowExtraArgs::Cassandra { allow_filtering } => *allow_filtering,
            },
            None => false,
        };
        let sql = format!(
            r#"SELECT * FROM "{name}" {} {}  {} "#,
            if let Some(where_str) = where_str {
                format!(
                    " WHERE {} {}",
                    where_str,
                    if allow_filtering {
                        "ALLOW FILTERING"
                    } else {
                        ""
                    }
                )
            } else {
                "".to_string()
            },
            if allow_filtering && where_str.is_some() {
                "".to_string()
            } else {
                if let Some(order_by) = order_by {
                    format!(" ORDER BY {} ", order_by)
                } else {
                    "".to_string()
                }
            },
            if allow_filtering && where_str.is_some() {
                "".to_string()
            } else {
                format!("LIMIT {}", Self::PAGE_CACHE_LIMIG_SIZE)
            }
        );
        log::info!("sql:{:?}", sql);

        let body = self.session.query(&sql).await?.response_body()?;
        let meta = body.as_rows_metadata().map(|x| x.clone());
        if meta.is_none() {
            return anyhow::Ok(QueryResp {
                fields: vec![],
                data: vec![],
            });
        }
        let meta = meta.unwrap();
        let rows = body.into_rows().unwrap_or_default();
        let fields = infer_fields(&meta);

        let data = encode_rows(&meta, &rows)?;
        // put in cache
        let cache = PageCache {
            result: QueryResp { fields, data },
            condition,
            sql,
        };

        let ret = cache.page(page, page_size);
        self.paging_cache.insert(name.to_owned(), Arc::new(cache));
        anyhow::Ok(ret)
    }
}

impl CassandraDB {
    async fn export_query_result_as_json(
        &self, sql: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let resp = self.session.query(sql).await?.response_body()?;
        let meta = resp.as_rows_metadata().map(|x| x.clone());
        if meta.is_none() {
            anyhow::bail!("no as_rows_metadata")
        }
        let meta = meta.unwrap();
        let rows = resp.into_rows().unwrap_or_default();
        let array = encode_rows_as_json(&meta, &rows)?;

        let json = serde_json::Value::Array(array);
        let json_str = if pretty {
            serde_json::to_string_pretty(&json)
        } else {
            serde_json::to_string(&json)
        }?;
        sender.send(Cow::Owned(json_str)).await?;
        anyhow::Ok(())
    }

    async fn export_query_result_as_csv(
        &self, sql: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let resp = self.session.query(sql).await?.response_body()?;
        let meta = resp.as_rows_metadata().map(|x| x.clone());
        if meta.is_none() {
            anyhow::bail!("no as_rows_metadata")
        }
        let meta = meta.unwrap();
        let rows = resp.into_rows().unwrap_or_default();
        let rows = encode_rows(&meta, &rows)?;
        if include_header {
            sender
                .send(Cow::Owned(create_csv(
                    &vec![
                        meta.col_specs
                            .iter()
                            .map(|x| Some(x.name.clone()))
                            .collect(),
                    ],
                    delimiter,
                )))
                .await?;
        }
        sender
            .send(Cow::Owned(create_csv(&rows, delimiter)))
            .await?;

        anyhow::Ok(())
    }

    async fn export_query_result_as_json_line(
        &self, sql: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let resp = self.session.query(sql).await?.response_body()?;
        let meta = resp.as_rows_metadata().map(|x| x.clone());
        if meta.is_none() {
            anyhow::bail!("no as_rows_metadata")
        }
        let meta = meta.unwrap();
        let rows = resp.into_rows().unwrap_or_default();
        for value in encode_rows_as_json(&meta, &rows)? {
            sender
                .send(Cow::Owned(serde_json::to_string(&value)?))
                .await?;
        }
        anyhow::Ok(())
    }
}

impl ReadOnlyTrait for CassandraDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}
impl CassandraDB {
    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_csv(&sql, include_header, delimiter, sender)
            .await
    }
    async fn export_table_as_json(
        &self, table_name: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_json(&sql, pretty, sender).await
    }
    async fn export_table_as_json_line(
        &self, table_name: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_json_line(&sql, sender).await
    }

    async fn export_table_as_sql(
        &self, table_name: &str, include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            let ss = SchemaTrait::create_table_and_index_sql(self, table_name).await?;
            sender.send(Cow::Owned(ss.create_table.clone())).await?;
            sender.send(";\n".into()).await?;
            for index in ss.indexes.into_iter() {
                sender.send(Cow::Owned(index)).await?;
                sender.send(";\n".into()).await?;
            }
        }
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        log::info!("sql:{}", sql);
        let body = self.session.query(sql).await?.response_body()?;
        let meta = body.as_rows_metadata().map(|x| x.clone());

        let rows = body.into_rows().unwrap_or_default();
        let meta = match meta {
            Some(meta) => meta,
            None => return anyhow::Ok(()),
        };
        let fs = infer_fields(&meta);
        for x in encode_rows(&meta, &rows)? {
            let mut values = HashMap::new();
            fs.iter().zip(x.iter()).for_each(|(f, v)| {
                values.insert(f.field.clone(), v.clone());
            });
            let sql = self.insert_sql(self.config.db.as_deref(), table_name, &fs, &values)?;
            log::info!("sql:{}", sql);
            sender.send(Cow::Owned(sql)).await?;
        }

        anyhow::Ok(())
    }
}

impl CassandraDB {
    pub(crate) async fn list_materialized_view_impl(
        &self, key_space: &str,
    ) -> anyhow::Result<Vec<ListMaterializedView>> {
        let sql =
            format!("select view_name from system_schema.views where keyspace_name='{key_space}'",);
        log::info!("sql:{}", sql);
        let rows = self
            .session
            .query(sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();
        let mut ret = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            if let Some(name) = name {
                ret.push(ListTable::from(name));
            }
        }
        anyhow::Ok(ret)
    }
}

impl SqlBuilder for CassandraDB {
    fn put_value_in_sql(
        &self, typ: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        put_value_in_sql(typ, value)
    }
    fn type_support_compare(&self, typ: &str) -> bool {
        type_support_compare(typ)
    }
}
#[async_trait::async_trait]
impl SchemaTrait for CassandraDB {
    async fn drop_created_type(&self, typ: &str) -> anyhow::Result<()> {
        let sql = format!("DROP TYPE  {typ}");
        log::info!("sql:{}", sql);
        self.session.query(sql).await?;
        anyhow::Ok(())
    }

    async fn list_created_types(&self) -> anyhow::Result<Vec<CreatedType>> {
        let sql = format!(
            "SELECT type_name FROM system_schema.types where  keyspace_name = '{}'",
            self.config.key_space_str()
        );
        log::info!("sql:{}", sql);
        let rows = self
            .session
            .query(sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();

        let mut ret = Vec::with_capacity(rows.len());

        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            let name = name.unwrap();
            let sql = format!("DESCRIBE TYPE {name}");
            log::info!("sql:{}", sql);
            let deccribe_rows = self
                .session
                .query(sql)
                .await?
                .response_body()?
                .into_rows()
                .unwrap_or_default();

            if deccribe_rows.len() != 1 {
                anyhow::bail!("describe type should return one result.")
            }
            let create_statement: Option<String> = deccribe_rows[0].get_by_index(3)?;
            ret.push(CreatedType {
                name,
                sql: create_statement.unwrap_or_default(),
            });
        }
        anyhow::Ok(ret)
    }

    async fn drop_materialized_view(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!("DROP MATERIALIZED VIEW {name};");
        log::info!("sql:{}", sql);
        self.session.query(sql).await?;
        anyhow::Ok(())
    }

    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListMaterializedView>> {
        self.list_materialized_view_impl(self.config.key_space_str())
            .await
    }

    async fn export_table_or_view(
        &self, name: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::CSV {
                include_header,
                delimiter,
            } => {
                self.export_table_as_csv(name, include_header, delimiter, sender)
                    .await
            }
            ExportKind::Json { pretty } => self.export_table_as_json(name, pretty, sender).await,
            ExportKind::JsonLine => self.export_table_as_json_line(name, sender).await,
            ExportKind::Sql {
                include_table_schema,
            } => {
                self.export_table_as_sql(name, include_table_schema, sender)
                    .await
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn create_index(
        &self, table: &str, name: &str, _kind: IndexKind, fields: &[String],
    ) -> anyhow::Result<()> {
        let sql = format!("CREATE INDEX {name} ON {table} ({});", fields.join(","));
        log::info!("sql:{}", sql);
        self.session.query(&sql).await?;
        anyhow::Ok(())
    }

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<()> {
        let sql = format!("DROP INDEX IF EXISTS {index};");
        log::info!("sql:{}", sql);
        self.session.query(sql).await?;
        anyhow::Ok(())
    }

    async fn create_table_and_index_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let sql = format!(r#"DESCRIBE TABLE "{table_name}""#);
        log::info!("sql:{}", sql);
        let rows = self
            .session
            .query(&sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();
        if rows.len() == 0 {
            anyhow::bail!("query create table expect return on result.");
        }
        let row = rows.get(0).unwrap();

        /*
        { flags: RowsMetadataFlags(GLOBAL_TABLE_SPACE), columns_count: 4, paging_state: None, new_metadata_id: None, global_table_spec: Some(TableSpec { ks_name: "system", table_name: "describe" }), col_specs: [ColSpec { table_spec: None, name: "keyspace_name", col_type: ColTypeOption { id: Varchar, value: None } }, ColSpec { table_spec: None, name: "type", col_type: ColTypeOption { id: Varchar, value: None } }, ColSpec { table_spec: None, name: "name", col_type: ColTypeOption { id: Varchar, value: None } }, ColSpec { table_spec: None, name: "create_statement", col_type: ColTypeOption { id: Varchar, value: None } }] }
            */
        // why `row.get_by_index(3)?` looks cassandra return this value.
        let ret: Option<String> = row.get_by_index(3)?;
        match ret {
            Some(x) => anyhow::Ok(CreateTableAndIndexSql {
                create_table: x,
                indexes: vec![],
            }),
            None => anyhow::bail!("create table sql should not be null"),
        }
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        let sql = self.create_table_and_index_sql(table).await?.create_table;
        // replace table name.
        let sql = sql.replacen(table, dup, 1);
        log::info!("sql:{}", sql);
        log::info!("sql:{}", sql);
        log::info!("dup table create table sql:{}", sql);
        // here we create the table
        self.session.query(sql).await?;
        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
        let j1 = {
            // load all data
            let x = Clone::clone(self);
            let table = table.to_owned();
            tokio::spawn(async move {
                x.export_table_or_view(
                    &table,
                    ExportKind::Sql {
                        // we only need data now.
                        include_table_schema: false,
                    },
                    sender,
                )
                .await
            })
        };
        let j2 = {
            let table = table.to_owned();
            let dup = dup.to_owned();
            let x = Clone::clone(self);
            tokio::spawn(async move {
                while let Some(sql) = receiver.recv().await {
                    // replace table name.
                    let sql = sql.replacen(table.as_str(), &dup, 1);
                    x.session.query(sql).await?;
                }
                anyhow::Ok(())
            })
        };
        j1.await??;
        j2.await??;
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.config.key_space_str()).await
    }
    // cassandra can use this function for materlized view too.
    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let sql = format!(
            r#" SELECT column_name, type, kind
        FROM system_schema.columns
        WHERE keyspace_name = '{}' AND table_name = '{}';"#,
            self.config.key_space_str(),
            table_name
        );
        log::info!("sql:{}", sql);

        let rows = self
            .session
            .query(&sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();
        let mut ret = Vec::new();
        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            let name = name.unwrap();
            let typ: Option<String> = row.get_by_index(1)?;
            let typ = typ.unwrap();
            let kind: Option<String> = row.get_by_index(2)?;
            let kind = kind.unwrap();
            let is_primary = kind == "partition_key" || kind == "clustering";

            ret.push(Field {
                field: name,
                typ,
                nullable: !is_primary,
                default_value: None,
                is_primary,
                is_auto_increment: false,
                sortable: kind == "clustering",
                firebird_binary_sub_type: None,
            });
        }

        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<TableListResp> {
        let mut ret = self
            .page_caching_show(table_name, where_str, order_by, page, page_size, extra)
            .await?;

        let fields = self.show_table_columns(&table_name).await?;
        if ret.fields.len() == 0 {
            return anyhow::Ok(TableListResp {
                fields,
                data: vec![],
                total: 0,
                sql: None,
            });
        }
        let mut m = HashMap::new();

        fields.iter().for_each(|x| {
            m.insert(x.field.clone(), x);
        });

        ret.fields.iter_mut().for_each(|field| {
            let f = m.get(&field.field);
            if let Some(f) = f {
                *field = (*f).clone();
            }
        });

        let QueryResp { fields, data } = ret;
        anyhow::Ok(TableListResp {
            fields,
            data,
            total: self.page_caching_totoal(table_name),
            sql: self.page_caching_sql(table_name),
        })
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let sql = format!(
            "SELECT index_name from system_schema.indexes WHERE  table_name = '{table_name}' AND keyspace_name = '{}'",
            self.config.key_space_str()
        );
        log::info!("sql:{}", sql);
        let rows = self
            .session
            .query(sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();

        let mut ret = Vec::new();
        for row in rows {
            let name: Option<String> = row.get_by_index(0)?;
            ret.push(Index {
                name,
                kind: IndexKind::Index,
                columns: vec![],
            });
        }
        anyhow::Ok(ret)
    }
    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(&table_name).await?;
        let sql = self.insert_sql(None, table_name, &fs, &values)?;
        log::info!("sql:{}", sql);
        self.session.query(&sql).await?;
        self.invalid_page_caching(Some(table_name));
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(1),
        })
    }
    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fs = self.show_table_columns(table_name).await?;
        let sql = self.update_sql(
            self.config.db.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &values,
            &wheres,
        )?;

        log::info!("update sql:{}", sql);
        self.session.query(&sql).await?;
        self.invalid_page_caching(Some(table_name));
        anyhow::Ok(SqlExecuteRet::from((sql, 1)))
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"TRUNCATE "{table_name}""#);
        log::info!("sql:{}", sql);
        self.session.query(&sql).await?;
        anyhow::Ok(())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;

        let sql = self.delete_sql(
            self.config.db.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &wheres,
        )?;
        log::info!("sql:{}", sql);
        self.session.query(&sql).await?;
        self.invalid_page_caching(Some(table_name));
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(1),
        })
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"DROP TABLE "{}";"#, table);
        log::info!("sql:{}", sql);
        self.session.query(&sql).await?;
        anyhow::Ok(())
    }

    async fn add_table_column(
        &self, table: &str, name: &str, col: ColumnType,
    ) -> anyhow::Result<()> {
        let sql = format!("ALTER TABLE {table} ADD {name} {}", col.typ);
        log::info!("sql:{}", sql);
        self.session.query(&sql).await?;
        anyhow::Ok(())
    }
    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, _typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        let sql = format!("ALTER TABLE {table} RENAME {old_name} TO {new_name}");
        log::info!("sql:{}", sql);
        self.session.query(&sql).await?;
        anyhow::Ok(())
    }
    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        let sql = format!("ALTER TABLE {table} DROP {col};");
        log::info!("sql:{}", sql);
        self.session.query(&sql).await?;
        anyhow::Ok(())
    }
    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<TableListResp> {
        self.show_table(view_name, where_str, order_by, page, page_size, extra)
            .await
    }
    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        self.show_table_columns(view_name).await
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_udf_impl(self.config.key_space_str()).await
    }

    async fn show_udf(
        &self, name: &str, extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<DefAndMetas> {
        /*
        add parameter to udf have this error
        cassandra@cqlsh:test> DESCRIBE FUNCTION  test.test_udf(int);
        Improper DESCRIBE command.
         */
        let sql = format!(r#" DESCRIBE FUNCTION  "{}""#, name);
        log::info!("sql:{}", sql);
        let body = self.session.query(sql).await?.response_body()?;
        // let meta = body.as_rows_metadata();
        // println!("meta:{:?}", meta);

        let parameters = match &extra {
            Some(extra) => match &extra {
                UdfOrProcedureExtraParameter::Cassandra { parameters } => parameters,
                _ => {
                    anyhow::bail!("show udf for cassandra need extra parameter.");
                }
            },
            None => anyhow::bail!("show udf for cassandra need extra parameter."),
        };

        let rows = body.into_rows().unwrap_or_default();

        for row in rows.iter() {
            let typ: Option<String> = row.get_by_index(1)?;
            let create_statement: Option<String> = row.get_by_index(3)?;

            let create_statement = create_statement.unwrap();

            let parameters_try_match = extract_parameter_for_function(&create_statement)?;
            if parameters.len() != parameters_try_match.len() {
                continue;
            }
            if parameters
                .iter()
                .zip(parameters_try_match.iter())
                .any(|(x, y)| {
                    let x_type = x.replace(" ", "");
                    let y_type = y.data_type.to_string().replace(" ", "");
                    !x_type.eq_ignore_ascii_case(&y_type)
                })
            {
                continue;
            }
            return anyhow::Ok(DefAndMetas {
                def: create_statement,
                metas: vec![MetaValue::new_opt("type", typ)],
            });
        }
        anyhow::bail!("no match for parameter????");
    }

    async fn drop_udf(
        &self, name: &str, extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let parameters = match &extra {
            Some(extra) => match &extra {
                UdfOrProcedureExtraParameter::Cassandra { parameters } => parameters,
                _ => {
                    anyhow::bail!("drop udf for cassandra need extra parameter.");
                }
            },
            None => anyhow::bail!("drop udf for cassandra need extra parameter."),
        };

        let p = format!("({})", parameters.join(","));

        let sql = format!("DROP FUNCTION {name}{p} ");

        log::info!("sql:{}", sql);
        self.session.query(sql).await?;
        anyhow::Ok(())
    }
    fn list_data_types(&self) -> anyhow::Result<&'static [&'static str]> {
        anyhow::Ok(<Self as LexerCompletionTrait>::TYPES)
    }
}

fn put_value_in_sql(typ: &str, value: Option<&str>) -> anyhow::Result<String> {
    let typ = typ.to_lowercase();
    log::trace!("encode value '{:?}' on type '{}'", value, typ);
    match typ.as_str() {
        "ascii" | "varchar" | "timestamp" | "text" | "date" | "time" | "inet" => anyhow::Ok(
            value
                .map(|x| format!("'{}'", x))
                .unwrap_or("null".to_string()),
        ),
        _ => anyhow::Ok(value.map(|x| x.to_string()).unwrap_or("null".to_string())),
    }
}

fn type_support_compare(_typ: &str) -> bool {
    true
}

// TODO this parse only support one parameter or `cassandra`
// support multi parameter for aggregate functon.
fn extract_parameter_for_aggregate(sql: &str) -> anyhow::Result<DataType> {
    let x = CassandraAST::new(sql);

    if x.statements.len() != 1 {
        anyhow::bail!("should contains one statement.")
    }

    let s = &x.statements[0];

    match &s.statement {
        CassandraStatement::CreateAggregate(x) => anyhow::Ok(x.data_type.clone()),
        _ => {
            anyhow::bail!("this should be create function statement.")
        }
    }
}

fn extract_parameter_for_function(sql: &str) -> anyhow::Result<Vec<ColumnDefinition>> {
    let x = CassandraAST::new(sql);

    if x.statements.len() != 1 {
        anyhow::bail!("should contains one statement.")
    }

    let s = &x.statements[0];

    match &s.statement {
        CassandraStatement::CreateFunction(x) => {
            return anyhow::Ok(x.params.clone());
        }
        _ => {
            anyhow::bail!("this should be create function statement.")
        }
    }
}

#[async_trait::async_trait]
impl LangServerCommon for CassandraDB {
    async fn list_schema(&self, _db: Option<&str>) -> anyhow::Result<Vec<String>> {
        DBTrait::list_schema(self).await
    }
}

#[async_trait::async_trait]
impl DBTrait for CassandraDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let body = self
            .session
            .query("DESCRIBE KEYSPACES")
            .await?
            .response_body()?;
        let mut ret = Vec::new();
        for row in body.into_rows().unwrap_or_default() {
            let name: Option<String> = row.get_by_index(0)?;
            ret.push(name.unwrap());
        }
        anyhow::Ok(ret)
    }
    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut config = self.config.clone();
        config.db = Some(name.to_string());
        let x = CassandraDB::new(config).await?;
        anyhow::Ok(Arc::new(x))
    }

    async fn create_schema(
        &self, name: &str, extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.can_write()?;

        let extra = match &extra {
            Some(x) => x,
            None => anyhow::bail!("cassandra need extra paramater."),
        };
        let (replication_strategy, replication_factor) = match extra {
            CreateSchemaExtra::Cassandra {
                replication_strategy,
                replication_factor,
            } => (replication_strategy, replication_factor),
        };

        let sql = format!(
            r#"
            CREATE KEYSPACE {}
            WITH REPLICATION = {{
                'class': '{}',
                'replication_factor': {}
            }};
        "#,
            name, replication_strategy, replication_factor
        );
        log::info!("sql:{}", sql);
        self.session.query(sql).await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("DROP KEYSPACE {name}");
        log::info!("sql:{}", sql);
        self.session.query(&sql).await?;
        anyhow::Ok(())
    }
}

impl CassandraDB {
    pub(crate) async fn list_udf_impl(&self, key_space: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"SELECT function_name, argument_types, return_type, language 
                FROM system_schema.functions 
                WHERE keyspace_name = '{key_space}';"#
        );
        log::info!("sql:{}", sql);
        let rows = self
            .session
            .query(sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();

        let mut ret = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            let argument_types: Option<List> = row.get_by_index(1)?;
            let return_type: Option<String> = row.get_by_index(2)?;
            let language: Option<String> = row.get_by_index(3)?;
            let parametres = if let Some(argument_types) = argument_types {
                extract_parameters_from_server(argument_types)?
            } else {
                vec![]
            };
            let name = name.unwrap();
            ret.push(NamedStaff {
                name,
                parameters: None,
                cassandra_parameters: Some(parametres),
                metas: vec![
                    MetaValue::new_opt("return_type", return_type),
                    MetaValue::new_opt("language", language),
                ],
            });
        }
        anyhow::Ok(ret)
    }
}

fn extract_parameters_from_server(argument_types: List) -> anyhow::Result<Vec<String>> {
    let mut parametres = vec![];
    let c = argument_types.as_cassandra_type()?;
    if let Some(c) = c {
        match c {
            CassandraType::List(mut list) => {
                for ele in list.iter_mut() {
                    match ele {
                        CassandraType::Ascii(x) => {
                            parametres.push(x.clone());
                        }
                        CassandraType::Varchar(x) => {
                            parametres.push(x.clone());
                        }
                        _ => {
                            log::error!("parameter here should be string")
                        }
                    }
                }
            }
            _ => {
                log::error!(" parameters should be a list")
            }
        }
    }
    anyhow::Ok(parametres)
}

#[cfg(test)]
pub(crate) async fn make_test_object(db: Option<String>) -> CassandraDB {
    use crate::TEST_HOST;

    let ss = CassandraDB::new(CassandraDBConfig {
        host: TEST_HOST.to_string(),
        port: Some(9042),
        username: Some("cassandra".to_string()),
        password: Some("cassandra".to_string()),
        read_only: false,
        db,
        save_password: true,
    })
    .await
    .unwrap();
    ss
}

impl CassandraDB {
    pub async fn list_aggregate_impl(&self, key_space: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
        SELECT aggregate_name, argument_types, return_type, state_func, final_func, initcond
            FROM system_schema.aggregates
            WHERE keyspace_name = '{key_space}';
        "#
        );
        log::info!("sql:{}", sql);
        let rows = self
            .session
            .query(&sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();

        let mut ret = Vec::with_capacity(rows.len());

        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            let parameters: Option<List> = row.get_by_index(1)?;
            let return_type: Option<String> = row.get_by_index(2)?;
            let state_func: Option<String> = row.get_by_index(3)?;
            let final_func: Option<String> = row.get_by_index(4)?;
            let initcond: Option<String> = row.get_by_index(5)?;
            let cassandra_parameters = if let Some(list) = parameters {
                extract_parameters_from_server(list)?
            } else {
                vec![]
            };
            ret.push(NamedStaff {
                parameters: None,
                name: name.unwrap(),
                metas: vec![
                    MetaValue::new_opt("return_type", return_type),
                    MetaValue::new_opt("state_func", state_func),
                    MetaValue::new_opt("final_func", final_func),
                    MetaValue::new_opt("initcond", initcond),
                ],
                cassandra_parameters: Some(cassandra_parameters),
            });
        }
        anyhow::Ok(ret)
    }
    pub async fn list_aggregate(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_aggregate_impl(self.config.key_space_str()).await
    }

    pub async fn show_aggregate(
        &self, name: &str, parameters: Vec<String>,
    ) -> anyhow::Result<DefAndMetas> {
        let sql = format!("DESCRIBE AGGREGATE  {name}");
        log::info!("sql:{}", sql);
        let body = self.session.query(&sql).await?.response_body()?;
        let _meta = body.as_rows_metadata();
        // println!("meta:{:?}", _meta);
        let rows = body.into_rows().unwrap_or_default();

        for row in rows.iter() {
            let typ: Option<String> = row.get_by_index(1)?;
            let create_statement: Option<String> = row.get_by_index(3)?;
            let create_statement = create_statement.unwrap();
            let parameters_try_match = vec![extract_parameter_for_aggregate(&create_statement)?];
            if parameters.len() != parameters_try_match.len() {
                continue;
            }
            if parameters
                .iter()
                .zip(parameters_try_match.iter())
                .any(|(x, y)| {
                    let x_type = x.replace(" ", "");
                    let y_type = y.to_string().replace(" ", "");
                    !x_type.eq_ignore_ascii_case(&y_type)
                })
            {
                continue;
            }
            return anyhow::Ok(DefAndMetas {
                def: create_statement,
                metas: vec![MetaValue::new_opt("type", typ)],
            });
        }
        anyhow::bail!("no match for parameter????");
    }

    pub async fn drop_aggregate(&self, name: &str, parameters: Vec<String>) -> anyhow::Result<()> {
        let p = format!("({})", parameters.join(","));
        let sql = format!("DROP AGGREGATE {name}{p} ");
        log::info!("sql:{}", sql);
        self.session.query(sql).await?;
        anyhow::Ok(())
    }
}

impl QuoteStyle for CassandraDB {
    fn quote_style(&self) -> &'static str {
        "\""
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enable_test_log;
    #[tokio::test]
    async fn test_list_schema() {
        let db = make_test_object(None).await;

        let _ = db
            .create_schema(
                "test",
                Some(CreateSchemaExtra::Cassandra {
                    replication_strategy: "SimpleStrategy".to_string(),
                    replication_factor: 1,
                }),
            )
            .await;

        let schemas = DBTrait::list_schema(&db).await.unwrap();
        println!("list schema:{:?}", schemas)
    }

    #[tokio::test]
    async fn test_aggregate() {
        let db = make_test_object(Some("test".to_string())).await;

        {
            // drop some function.
            let _ = db
                .drop_aggregate("test_aggregate", vec!["int".to_string()])
                .await;
            let _ = db.drop_udf(
                "sum_state_function",
                Some(UdfOrProcedureExtraParameter::Cassandra {
                    parameters: vec!["int".to_string(), "int".to_string()],
                }),
            );

            let _ = db.drop_udf(
                "sum_final_function",
                Some(UdfOrProcedureExtraParameter::Cassandra {
                    parameters: vec!["int".to_string()],
                }),
            );
        }
        let query_id = db.open_connection().await.unwrap();

        let _resp = db
            .query_sql(
                query_id,
                r#"
        CREATE OR REPLACE FUNCTION sum_state_function(state int, val int)
            RETURNS NULL ON NULL INPUT
            RETURNS int
            LANGUAGE java
            AS $$
                return state + val; 
            $$;
        "#,
            )
            .await
            .unwrap();
        assert!(_resp.iter().all(|x| x.is_ok()), "resp:{:?}", _resp);

        let _resp = db
            .query_sql(
                query_id,
                r#"
                CREATE OR REPLACE FUNCTION sum_final_function(state int)
                RETURNS NULL ON NULL INPUT
                RETURNS int
                LANGUAGE java
                AS $$
                    return state; // 直接返回累加结果
                $$;
    "#,
            )
            .await
            .unwrap();
        assert!(_resp.iter().all(|x| x.is_ok()), "resp:{:?}", _resp);

        let _resp = db
            .query_sql(
                query_id,
                r#"
            CREATE AGGREGATE test_aggregate(int)
            SFUNC sum_state_function
            STYPE int
            FINALFUNC sum_final_function
            INITCOND 0;
"#,
            )
            .await
            .unwrap();
        assert!(_resp.iter().all(|x| x.is_ok()), "resp:{:?}", _resp);

        let list = db.list_aggregate().await.unwrap();

        list.iter().for_each(|x| println!("aggregate:{:?}", x));

        let details = db
            .show_aggregate("test_aggregate", vec!["int".to_string()])
            .await
            .unwrap();

        println!("details:{:?}", details);
    }

    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object(None).await;
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_sort() {
        let table_name = "test_sort";

        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_sort (
                id UUID,
                sort_key INT,
                name TEXT,
                value INT,
                PRIMARY KEY (id , sort_key )
            ) ;

        "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);

        db.query_sql(
            query_id,
            r#"
        INSERT INTO test_sort (sort_key, id, name, value) VALUES (1, uuid(), 'Alice', 30);
        INSERT INTO test_sort (sort_key, id, name, value) VALUES (1, uuid(), 'Bob', 20);
        INSERT INTO test_sort (sort_key, id, name, value) VALUES (1, uuid(), 'Charlie', 25);
        INSERT INTO test_sort (sort_key, id, name, value) VALUES (2, uuid(), 'Diana', 35);
        "#,
        )
        .await
        .unwrap();

        let resp = db
            .show_table(&table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }

    #[tokio::test]
    async fn test_list_table() {
        let db = make_test_object(Some("system".to_string())).await;
        let schemas = db.list_table().await.unwrap();
        println!("list table:{:?}", schemas)
    }

    #[tokio::test]
    async fn test_show_table_column() {
        let db = make_test_object(Some("test".to_string())).await;
        let fields = db.show_table_columns("example_table").await.unwrap();
        for field in fields {
            println!("field:{:?}", field);
        }
    }

    #[tokio::test]
    async fn test_error_position() {
        let db = make_test_object(None).await;
        let x = db.session.query("xxxxxxxxx").await;
        // should have error on `x` no need check on response.
        assert!(x.is_err());
    }

    #[tokio::test]
    async fn test_duplicate() {
        let table_name = "test_duplicate_table";
        let table_dup = "test_duplicate";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let _ = db.drop_table(table_dup, false).await;

        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_duplicate_table (
                id UUID PRIMARY KEY,
                name TEXT,
                value INT
            );

        "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);
        db.query_sql(
            query_id,
            r#"
        INSERT INTO test_duplicate_table (id, name, value) VALUES (uuid(), 'Alice', 10);
        INSERT INTO test_duplicate_table (id, name, value) VALUES (uuid(), 'Bob', 20);
        INSERT INTO test_duplicate_table (id, name, value) VALUES (uuid(), 'Charlie', 30);
    "#,
        )
        .await
        .unwrap();
        db.duplicate_table(table_name, table_dup).await.unwrap();

        let resp = db
            .show_table(table_dup, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }

    #[tokio::test]
    async fn test_list_created_types() {
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_created_type("test_type").await;

        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .query_sql(
                query_id,
                r#"
            CREATE TYPE test_type (
                id int,
                name text,
                created_at timestamp
            );
        "#,
            )
            .await
            .unwrap();
        assert!(_resp.iter().all(|x| x.is_ok()), "resp:{:?}", _resp);

        let list = db.list_created_types().await.unwrap();
        for l in list.iter() {
            println!("create type:{:?}", l);
        }
    }
    #[tokio::test]
    async fn test_types1() {
        let table_name = "test_types1";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .execute_sql(
                query_id,
                r#"
                CREATE TABLE test_types1 (
                    id UUID PRIMARY KEY,
                    ascii_column ASCII,
                    bigint_column BIGINT,
                    blob_column BLOB,
                    boolean_column BOOLEAN,
                    decimal_column DECIMAL,
                    double_column DOUBLE
                );
        "#,
            )
            .await
            .unwrap();
        assert!(x.is_ok());
        /*
        INSERT INTO test_types1 (id, ascii_column, bigint_column, blob_column, boolean_column, decimal_column, double_column)
        VALUES (e6361ec1-b0a6-4eb6-b7bd-2db2d13329ab, 'example_ascii', 1234567890123456789, 0x48656c6c6f, true, 123.45, 3.14159);
        */

        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert(
            "ascii_column".to_string(),
            Some("example_ascii".to_string()),
        );
        values.insert(
            "bigint_column".to_string(),
            Some("1234567890123456789".to_string()),
        );
        values.insert("blob_column".to_string(), Some("0x48656c6c6f".to_string()));
        values.insert("boolean_column".to_string(), Some("true".to_string()));

        values.insert("decimal_column".to_string(), Some("123.45".to_string()));
        values.insert("double_column".to_string(), Some("3.14159".to_string()));

        db.insert(&table_name, values).await.unwrap();

        {
            let resp = db.show_table(table_name, None, None, 1, 100, None).await;
            println!("resp:{:?}", resp);
        }
        {
            //export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            //export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            //export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            //export jsonline
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }
    #[tokio::test]
    async fn test_types1_list() {
        let table_name = "test_types_list1";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_types_list1 (
                    id UUID PRIMARY KEY,
                    ascii_column list<ASCII>,
                    bigint_column list<BIGINT>,
                    blob_column list<BLOB>,
                    boolean_column list<BOOLEAN>,
                    decimal_column list<DECIMAL>,
                    double_column list<DOUBLE>
                );
        "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);

        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert(
            "ascii_column".to_string(),
            Some("['example_ascii']".to_string()),
        );
        values.insert(
            "bigint_column".to_string(),
            Some("[1234567890123456789]".to_string()),
        );
        values.insert(
            "blob_column".to_string(),
            Some("[0x48656c6c6f]".to_string()),
        );
        values.insert("boolean_column".to_string(), Some("[true]".to_string()));

        values.insert("decimal_column".to_string(), Some("[123.45]".to_string()));
        values.insert("double_column".to_string(), Some("[3.14159]".to_string()));

        db.insert(&table_name, values).await.unwrap();

        {
            let resp = db.show_table(table_name, None, None, 1, 100, None).await;

            println!("resp:{:?}", resp);
        }
        {
            //export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            //export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            //export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            //export jsonline
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test]
    async fn test_types2() {
        enable_test_log();
        let table_name = "test_types2";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_types2 (
                    id UUID PRIMARY KEY,
                    float_value FLOAT,
                    int_value INT,
                    timestamp_value TIMESTAMP,
                    varchar_value VARCHAR,
                    varint_value VARINT
                );
        "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);
        /*
        INSERT INTO test_types2 (id, float_value, int_value, timestamp_value, varchar_value, varint_value)
            VALUES (uuid(), 3.14, 42, '2023-01-01 10:00:00', 'Sample Text', '123456789012345678901234567890');

        */

        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert("float_value".to_string(), Some("3.14".to_string()));
        values.insert("int_value".to_string(), Some("42".to_string()));
        values.insert(
            "timestamp_value".to_string(),
            Some("2023-01-01 10:00:00".to_string()),
        );
        values.insert("varchar_value".to_string(), Some("Sample Text".to_string()));

        values.insert(
            "varint_value".to_string(),
            Some("123456789012345678901234567890".to_string()),
        );

        db.insert(&table_name, values).await.unwrap();

        {
            let resp = db.show_table(table_name, None, None, 1, 100, None).await;

            println!("resp:{:?}", resp);
        }
        {
            //export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            //export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            //export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            //export jsonline
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test]
    async fn test_types2_list() {
        enable_test_log();
        let table_name = "test_types_list2";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_types_list2 (
                    id UUID PRIMARY KEY,
                    float_value list<FLOAT>,
                    int_value list<INT>,
                    timestamp_value list<TIMESTAMP>,
                    varchar_value list<VARCHAR>,
                    varint_value list<VARINT>
                );
        "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);
        /*
        INSERT INTO test_types2 (id, float_value, int_value, timestamp_value, varchar_value, varint_value)
            VALUES (uuid(), 3.14, 42, '2023-01-01 10:00:00', 'Sample Text', '123456789012345678901234567890');

        */

        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert("float_value".to_string(), Some("[3.14]".to_string()));
        values.insert("int_value".to_string(), Some("[42]".to_string()));
        values.insert(
            "timestamp_value".to_string(),
            Some("['2023-01-01 10:00:00']".to_string()),
        );
        values.insert(
            "varchar_value".to_string(),
            Some("['Sample Text']".to_string()),
        );

        values.insert(
            "varint_value".to_string(),
            Some("[123456789012345678901234567890]".to_string()),
        );

        db.insert(&table_name, values).await.unwrap();

        {
            let resp = db.show_table(table_name, None, None, 1, 100, None).await;

            println!("resp:{:?}", resp);
        }
        {
            //export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            //export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            //export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            //export jsonline
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test]
    async fn test_types3() {
        enable_test_log();
        let table_name = "test_types3";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_types3 (
                    id UUID PRIMARY KEY,
                    timeuuid_value TIMEUUID,
                    inet_value INET,
                    date_value DATE,
                    time_value TIME,
                    smallint_value SMALLINT,
                    tinyint_value TINYINT
                );

        "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);
        /*
        INSERT INTO test_types3 (id, timeuuid_value, inet_value, date_value, time_value, smallint_value, tinyint_value)
        VALUES (
            uuid(),
            now(),                          -- 生成当前的timeuuid
            '192.168.1.1',                 -- 示例IP地址
            '2023-01-01',                  -- 示例日期
            '10:30:00',                    -- 示例时间
            123,                            -- 示例smallint值
            5,                              -- 示例tinyint值
        );
        */

        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert("timeuuid_value".to_string(), Some("now()".to_string()));
        values.insert("inet_value".to_string(), Some("192.168.1.1".to_string()));
        values.insert("date_value".to_string(), Some("2023-01-01".to_string()));
        values.insert("time_value".to_string(), Some("10:30:00".to_string()));

        values.insert("smallint_value".to_string(), Some("123".to_string()));
        values.insert("tinyint_value".to_string(), Some("5".to_string()));

        db.insert(&table_name, values).await.unwrap();

        {
            let resp = db.show_table(table_name, None, None, 1, 100, None).await;

            println!("resp:{:?}", resp);
        }
        {
            //export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            //export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            //export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            //export jsonline
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test]
    async fn test_types_list3() {
        enable_test_log();
        let table_name = "test_types_list3";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_types_list3 (
                    id UUID PRIMARY KEY,
                    timeuuid_value list<TIMEUUID>,
                    inet_value list<INET>,
                    date_value list<DATE>,
                    time_value list<TIME>,
                    smallint_value list<SMALLINT>,
                    tinyint_value list<TINYINT>
                );

        "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);
        /*
        INSERT INTO test_types3 (id, timeuuid_value, inet_value, date_value, time_value, smallint_value, tinyint_value)
        VALUES (
            uuid(),
            now(),                          -- 生成当前的timeuuid
            '192.168.1.1',                 -- 示例IP地址
            '2023-01-01',                  -- 示例日期
            '10:30:00',                    -- 示例时间
            123,                            -- 示例smallint值
            5,                              -- 示例tinyint值
        );
        */

        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert("timeuuid_value".to_string(), Some("[now()]".to_string()));
        values.insert(
            "inet_value".to_string(),
            Some("['192.168.1.1']".to_string()),
        );
        values.insert("date_value".to_string(), Some("['2023-01-01']".to_string()));
        values.insert("time_value".to_string(), Some("['10:30:00']".to_string()));

        values.insert("smallint_value".to_string(), Some("[123]".to_string()));
        values.insert("tinyint_value".to_string(), Some("[5]".to_string()));

        db.insert(&table_name, values).await.unwrap();

        {
            let resp = db.show_table(table_name, None, None, 1, 100, None).await;

            println!("resp:{:?}", resp);
        }
        {
            //export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            //export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            //export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            //export jsonline
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test]
    async fn test_types4() {
        enable_test_log();
        let table_name = "test_types4";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
               CREATE TABLE test_types4 (
                    id UUID PRIMARY KEY,
                    map_field map<int, int>,
                    set_field set<int>,
                    person TypePerson,  -- 确保 TypePerson 已经定义
                    tuple_field tuple<int, int>
                );
        "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);

        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert("map_field".to_string(), Some("{1 :2}".to_string()));
        values.insert("set_field".to_string(), Some("{1,2}".to_string()));
        values.insert(
            "person".to_string(),
            Some("{first_name : 'yu' ,last_name : 'yang' , email : 'xxx',age : 100}".to_string()),
        );
        values.insert("tuple_field".to_string(), Some("(1,100)".to_string()));
        db.insert(&table_name, values).await.unwrap();

        {
            let resp = db.show_table(table_name, None, None, 1, 100, None).await;

            println!("resp:{:?}", resp);
        }
        {
            //export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            //export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            //export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            //export jsonline
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test]
    async fn test_table_column() {
        enable_test_log();
        let db = make_test_object(Some("test".to_string())).await;
        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;

        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .query_sql(
                query_id,
                r#"
               CREATE TABLE test_column (
                    id1 int,
                    id2 int,
                    value text,
                    PRIMARY KEY (id1, id2)
                );
                        "#,
            )
            .await
            .unwrap();

        db.add_table_column(
            table_name,
            "name",
            ColumnType {
                typ: "tinyint".to_string(),
                nullness: None,
                after: None,
                default_value: Some("1".to_string()),
            },
        )
        .await
        .unwrap();

        db.modify_table_column(
            table_name,
            "id2",
            "name2",
            Some(ColumnType {
                typ: "smallint".to_string(),
                nullness: None,
                after: None,
                default_value: Some("2".to_string()),
            }),
        )
        .await
        .unwrap();

        db.drop_table_column(table_name, "value").await.unwrap();
    }
    #[tokio::test]
    async fn test_counter() {
        enable_test_log();

        let table_name = "test_counter";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_counter (
                page_id UUID PRIMARY KEY,
                view_count COUNTER
            );
            
                "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);

        let mut values = HashMap::new();
        let mut wheres = HashMap::new();
        values.insert("view_count".to_string(), Some("view_count + 1".to_string()));
        wheres.insert("page_id".to_string(), Some("uuid()".to_string()));
        db.update(table_name, values, wheres).await.unwrap();

        let resp = db
            .show_table(
                table_name,
                None,
                None,
                1,
                100,
                Some(ShowExtraArgs::Cassandra {
                    allow_filtering: true,
                }),
            )
            .await
            .unwrap();
        println!("xxxx:{:?}", resp);
    }

    #[tokio::test]
    async fn test_udf() {
        enable_test_log();
        let db = make_test_object(Some("test".to_string())).await;
        let name = "test_udf";
        {
            let err = db
                .drop_udf(
                    name,
                    Some(UdfOrProcedureExtraParameter::Cassandra {
                        parameters: vec!["text".to_string()],
                    }),
                )
                .await;
            if let Err(err) = err {
                println!("drop udf error:{}", err);
            }
            let err = db
                .drop_udf(
                    name,
                    Some(UdfOrProcedureExtraParameter::Cassandra {
                        parameters: vec!["list<int>".to_string()],
                    }),
                )
                .await;
            if let Err(err) = err {
                println!("drop udf error:{}", err);
            }
        }

        db.session
            .query(
                r#"
            CREATE FUNCTION IF NOT EXISTS  test_udf(input text)
            RETURNS NULL ON NULL INPUT
            RETURNS text
            LANGUAGE java
            AS 'return input;';
        "#,
            )
            .await
            .unwrap();

        db.session
            .query(
                r#"
            CREATE FUNCTION IF NOT EXISTS  test_udf(input list<int>)
            RETURNS NULL ON NULL INPUT
            RETURNS text
            LANGUAGE java
            AS 'return String.valueOf(input);';
        "#,
            )
            .await
            .unwrap();
        let list = db.list_udf().await.unwrap();
        list.iter().for_each(|x| println!("udf:{:?}", x));

        let detail = db
            .show_udf(
                name,
                Some(UdfOrProcedureExtraParameter::Cassandra {
                    parameters: vec!["list <int>".to_string()],
                }),
            )
            .await
            .unwrap();
        println!("detail:{:?}", detail);
    }

    #[tokio::test]
    async fn test_create_type() {
        let db = make_test_object(Some("test".to_string())).await;
        db.session
            .query(
                r#"
            CREATE TYPE IF NOT EXISTS TypePerson (
                first_name text,
                last_name text,
                age int,
                email text
            );
        "#,
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_rename() {
        let db = make_test_object(Some("test".to_string())).await;
        let table_name = "test_rename";
        let table_name2 = format!("{}2", table_name);
        let _ = db.drop_table(table_name, false).await;
        let _ = db.drop_table(&table_name2, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_rename (
                page_id UUID PRIMARY KEY,
                view_count COUNTER
            );
                "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);

        db.rename_table(&table_name, &table_name2).await.unwrap();
        db.rename_table(&table_name2, &table_name).await.unwrap();
    }

    #[tokio::test]
    async fn test_index() {
        let db = make_test_object(Some("test".to_string())).await;
        let table_name = "test_index";
        let _ = db.drop_index(table_name, "test_index_email_index").await;
        let _ = db.drop_table(table_name, false).await;

        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_index (
                    id UUID PRIMARY KEY,
                    name TEXT,
                    age INT,
                    email TEXT
                );
                "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);
        db.create_index(
            &table_name,
            "test_index_email_index",
            IndexKind::Index,
            &["email".to_string()],
        )
        .await
        .unwrap();

        let list = db.show_indexes(&table_name).await.unwrap();
        println!("indexes:{:?}", list);
    }

    #[tokio::test]
    async fn test_view() {
        let db = make_test_object(Some("test".to_string())).await;
        let table_name = "test_view_table";
        let view_name = "test_view";
        let _ = db.drop_materialized_view(view_name).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_view_table (
                    id UUID PRIMARY KEY,
                    name TEXT,
                    age INT
                );
                "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);
        let x = db
            .query_sql(
                query_id,
                r#"
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Alice', 30);
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Bob', 25);
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Charlie', 35);
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Diana', 28);
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Eve', 22);
            "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);

        let x = db
            .query_sql(
                query_id,
                r#"
                CREATE MATERIALIZED VIEW test_view AS
                SELECT * FROM test_view_table
                WHERE age IS NOT NULL AND id IS NOT NULL
                PRIMARY KEY (age, id);
            "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);

        let list = db.list_materialized_view().await.unwrap();
        println!("view list:{:?}", list);
        {
            let resp = db
                .show_view(&view_name, None, None, 1, 100, None)
                .await
                .unwrap();
            println!("resp:{:?}", resp);
        }
    }

    #[test]
    fn test_parser_start_bytes() {
        let sql = "select * from test1 ; select * from test2;";
        let ss = sql_to_each(&sql);
        println!("{:?}", ss);
        assert!(ss.len() == 2);
    }

    #[test]
    fn test_agg() {
        let sql = r#"CREATE AGGREGATE my_count(text)
                SFUNC count_state_function
                STYPE int
                FINALFUNC count_final_function
                INITCOND 0;"#;
        CassandraAST::new(&sql)
            .statements
            .iter()
            .for_each(|x| println!("{:?}", x.statement));
    }
}
