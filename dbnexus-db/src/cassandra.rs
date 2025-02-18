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
use dashmap::DashMap;

use num_bigint::{BigInt, ToBigInt};

use crate::{
    datasource_config::CassandraDBConfig,
    db::{
        BackendName, CheckConn, CreateSchemaExtra, CreateTableAndIndexSql, DBTrait, ExportKind,
        Field, Index, IndexKind, ListTable, QueryResp, QuerySql, SchemaTrait, ServerMetas,
        SqlExecuteRet, TableListResp,
    },
    helper::{SqlKind, SqlWithKind, create_csv, sql_to_sql_seq},
    meta_value::MetaValue,
    query_log::QueryLog,
};
use cdrs_tokio::cluster::session::SessionBuilder;

#[derive(Clone)]
pub(crate) struct CassandraDB {
    session: Arc<
        cdrs_tokio::cluster::session::Session<
            cdrs_tokio::transport::TransportTcp,
            cdrs_tokio::cluster::TcpConnectionManager,
            RoundRobinLoadBalancingStrategy<
                cdrs_tokio::transport::TransportTcp,
                cdrs_tokio::cluster::TcpConnectionManager,
            >,
        >,
    >,
    config: CassandraDBConfig,

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

        QueryResp {
            fields: self.result.fields.clone(),
            data: (&self.result.data[start..end]).to_vec(),
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
        if let Some(keyspace) = config.keyspace.as_deref() {
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
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl CheckConn for CassandraDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        unimplemented!()
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

fn get_sql_seq(sql: &str) -> anyhow::Result<Vec<SqlWithKind>> {
    // TODO use oracle later when sqlparser supported
    let dialect = sqlparser::dialect::GenericDialect {};
    sql_to_sql_seq(
        sql,
        &dialect,
        |s| SqlKind::get_from_statement(s),
        |x| SqlKind::get_from_sql(x),
    )
}

fn infer_fields(rows: &RowsMetadata) -> Vec<Field> {
    rows.col_specs
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
    format!(
        "{}.{}",
        x.as_plain().to_string(),
        x.unscaled.clone() % 10i32.pow(x.scale as u32).to_bigint().unwrap()
    )
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
    // TODO https://github.com/krojew/cdrs-tokio/issues/202

    days_since_epoch &= !(1 << 31);

    let date = NaiveDate::from_ymd_opt(1970, 1, 1)
        .unwrap()
        .checked_add_days(chrono::Days::new(days_since_epoch as i64 as u64));
    match date {
        Some(x) => anyhow::Ok(x.to_string()),

        None => anyhow::bail!("days is none"),
    }
}

#[async_trait::async_trait]
impl QuerySql for CassandraDB {
    async fn query_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        self.invalid_page_caching(None);
        let seq = get_sql_seq(sql).unwrap_or(vec![SqlWithKind {
            sql: sql.to_string(),
            kind: SqlKind::Query,
        }]);
        let mut query_log = QueryLog::get_guard(query_id, sql);

        let mut ret = vec![];
        for s in seq.iter() {
            match self.session.query(&s.sql).await {
                Ok(x) => {
                    let body = x.response_body()?;
                    let meta = body.as_rows_metadata().map(|x| x.clone());
                    let rows = body.into_rows().unwrap_or_default();
                    if rows.len() > 0 && meta.is_some() {
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

    fn is_dangerous_sql(&self, _sql: &str) -> anyhow::Result<bool> {
        unimplemented!()
    }
}

impl CassandraDB {
    async fn list_table_impl(&self, keyspace: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            "SELECT table_name FROM system_schema.tables WHERE keyspace_name = '{keyspace}' order by table_name;"
        );
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
        page_size: usize,
    ) -> anyhow::Result<QueryResp> {
        // try get value from cache
        let condition = PageCacheCondition {
            where_str: where_str.map(|x| x.to_owned()),
            order_by: order_by.map(|x| x.to_owned()),
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
        let sql = format!(
            r#"SELECT * FROM "{name}" {} {} LIMIT {} "#,
            if let Some(where_str) = where_str {
                format!(" WHERE {} ", where_str)
            } else {
                "".to_string()
            },
            if let Some(order_by) = order_by {
                format!(" ORDER BY {} ", order_by)
            } else {
                "".to_string()
            },
            Self::PAGE_CACHE_LIMIG_SIZE
        );
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

#[async_trait::async_trait]

impl SchemaTrait for CassandraDB {
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn create_index(
        &self, table: &str, name: &str, _kind: IndexKind, fields: &[String],
    ) -> anyhow::Result<()> {
        let sql = format!("CREATE INDEX {name} ON {table} ({});", fields.join(","));
        self.session.query(&sql).await?;
        anyhow::Ok(())
    }

    async fn create_table_and_index_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let sql = format!("DESCRIBE  TABLE {table_name}");

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
        let ret: Option<String> = row.get_by_index(0)?;
        match ret {
            Some(x) => anyhow::Ok(CreateTableAndIndexSql {
                create_table: x,
                indexes: vec![],
            }),
            None => anyhow::bail!("create table sql should not be null"),
        }
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        // duplicate and drop old table.
        self.duplicate_table(old, new).await?;
        self.drop_table(old, false).await?;
        anyhow::Ok(())
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        let sql = self.create_table_and_index_sql(table).await?.create_table;
        // replace table name.
        let sql = sql.replacen(table, dup, 1);

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

    fn quote_style(&self) -> &'static str {
        "\""
    }
    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.config.keyspace_str()).await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let sql = format!(
            r#" SELECT column_name, type, kind
        FROM system_schema.columns
        WHERE keyspace_name = '{}' AND table_name = '{}';"#,
            self.config.keyspace_str(),
            table_name
        );

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
            });
        }

        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let mut ret = self
            .page_caching_show(table_name, where_str, order_by, page, page_size)
            .await?;

        let fields = self.show_table_columns(&table_name).await?;
        let mut m = HashMap::new();

        fields.iter().for_each(|x| {
            m.insert(x.field.clone(), x);
        });

        ret.fields.iter_mut().for_each(|field| {
            let f = m.get(&field.field);
            if let Some(f) = f {
                field.is_primary = f.is_primary;
                field.nullable = f.nullable;
                field.typ = f.typ.clone();
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
            self.config.keyspace_str()
        );

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
                on: vec![],
            });
        }
        anyhow::Ok(ret)
    }
    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(&table_name).await?;
        let sql = (self as &dyn SchemaTrait).insert_sql(None, table_name, &fs, &values)?;
        log::info!("insert sql:{}", sql);
        self.session.query(&sql).await?;
        self.invalid_page_caching(Some(table_name));
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: 1,
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }
    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fs = self.show_table_columns(table_name).await?;
        let sql = (self as &dyn crate::db::SchemaTrait).update_sql(
            self.config.keyspace.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &values,
            &wheres,
            type_support_compare,
        )?;
        self.session.query(&sql).await?;
        self.invalid_page_caching(Some(table_name));
        anyhow::Ok(SqlExecuteRet::from((sql, 1)))
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"TRUNCATE "{table_name}""#);
        self.session.query(&sql).await?;
        anyhow::Ok(())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;

        let sql = (self as &dyn SchemaTrait).delete_sql(
            self.config.keyspace.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &wheres,
            type_support_compare,
        )?;
        self.session.query(&sql).await?;
        self.invalid_page_caching(Some(table_name));
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: 1,
            last_insert_id_string: None,
            last_insert_row_id: None,
        })
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"DROP TABLE "{}";"#, table);
        self.session.query(&sql).await?;
        anyhow::Ok(())
    }
    fn read_only(&self) -> bool {
        self.config.read_only
    }

    fn put_value_in_sql(&self, typ: &str, value: Option<&str>) -> anyhow::Result<String> {
        put_value_in_sql(typ, value)
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

#[async_trait::async_trait]
impl DBTrait for CassandraDB {
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
        config.keyspace = Some(name.to_string());
        let x = CassandraDB::new(config).await?;
        anyhow::Ok(Arc::new(x))
    }

    async fn create_schema(
        &self, name: &str, extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        let extra = match &extra {
            Some(x) => x,
            None => anyhow::bail!("cassandra need extra paramater."),
        };
        let (class_name, replication_factor) = match extra {
            CreateSchemaExtra::Cassandra {
                class_name,
                replication_factor,
            } => (class_name, replication_factor),
        };

        let sql = format!(
            r#"
            CREATE KEYSPACE {}
            WITH REPLICATION = {{
                'class': '{}',
                'replication_factor': {}
            }};
        "#,
            name, class_name, replication_factor
        );
        self.session.query(sql).await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!("DROP KEYSPACE {name};");
        self.session.query(&sql).await?;
        anyhow::Ok(())
    }
}

#[cfg(test)]
pub(crate) async fn make_test_object(db: Option<String>) -> CassandraDB {
    use crate::TEST_HOST;

    let ss = CassandraDB::new(CassandraDBConfig {
        host: TEST_HOST.to_string(),
        port: Some(9042),
        username: None,
        password: None,
        read_only: false,
        keyspace: db,
    })
    .await
    .unwrap();
    ss
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
                    class_name: "SimpleStrategy".to_string(),
                    replication_factor: 1,
                }),
            )
            .await;

        let schemas = db.list_schema().await.unwrap();
        println!("list schema:{:?}", schemas)
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
    async fn test_xxxxx() {
        let db = make_test_object(None).await;
        let x = db.session.query("xxxxxxxxx").await;
        // should have error on `x` no need check on response.
        assert!(x.is_err());
    }

    #[tokio::test]
    async fn test_types1() {
        let table_name = "test_types1";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
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
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);
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
            let resp = db.show_table(table_name, None, None, 1, 100).await;

            println!("resp:{:?}", resp);
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
            let resp = db.show_table(table_name, None, None, 1, 100).await;

            println!("resp:{:?}", resp);
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
            let resp = db.show_table(table_name, None, None, 1, 100).await;

            println!("resp:{:?}", resp);
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
            let resp = db.show_table(table_name, None, None, 1, 100).await;

            println!("resp:{:?}", resp);
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
            let resp = db.show_table(table_name, None, None, 1, 100).await;

            println!("resp:{:?}", resp);
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
            let resp = db.show_table(table_name, None, None, 1, 100).await;

            println!("resp:{:?}", resp);
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
            let resp = db.show_table(table_name, None, None, 1, 100).await;

            println!("resp:{:?}", resp);
        }
    }

    #[tokio::test]
    async fn test_counter() {
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

        let resp = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("xxxx:{:?}", resp);
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
        let _ = db.drop_table(table_name, false).await;
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
        let table_name2 = format!("{}2", table_name);
        db.rename_table(&table_name, &table_name2).await.unwrap();
        db.rename_table(&table_name2, &table_name).await.unwrap();
    }

    #[tokio::test]
    async fn test_index() {
        let db = make_test_object(Some("test".to_string())).await;
        let table_name = "test_index";
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
        db.create_index(&table_name, "email_index", IndexKind::Index, &[
            "email".to_string()
        ])
        .await
        .unwrap();

        let list = db.show_indexes(&table_name).await.unwrap();
        println!("indexes:{:?}", list);
    }
}
