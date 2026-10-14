// The error InvalidRequest: Error from server: code=2200 [Invalid query]
// message="org.apache.cassandra.auth.CassandraRoleManager doesn't support
// PASSWORD" occurs because Cassandra authentication is disabled or the configuration does not support password authentication.
//
// To resolve this issue, follow these steps:
// 1. Edit the cassandra.yaml configuration file.
// By default, Cassandra uses
// AllowAllAuthenticator, which disables authentication. Change it to
// PasswordAuthenticator to enable password-based authentication.
//
// Open cassandra.yaml (usually located at /etc/cassandra/conf/cassandra.yaml or
// in the conf directory of your Cassandra installation). Find the following line:
// YAML (copy)
//
// authenticator: AllowAllAuthenticator
//
// Change it to:
// YAML (copy)
//
// authenticator: PasswordAuthenticator
//
// Save the file and restart the Cassandra service.
//
// InvalidRequest: Error from server: code=2200 [Invalid query]
// message="User-defined functions are disabled in cassandra.yaml - set
// user_defined_functions_enabled=true to enable"
//

#[cfg(test)]
use crate::datasource_config::CassandraDialect;
use crate::datasource_config::{CassandraConsistency, CassandraDBConfig};
use crate::db::{
    create_html_table_data, create_html_table_header, create_markdown_data, create_markdown_header,
    export_query_as_xlsx_send_data, quote_sql_str, AutoCompletionQueryId, BackendKind, CheckConn,
    ColumnType, CreateIndexReq, CreateSchemaArg, CreateTableAndIndexSql, CreatedTypeDef, DBTrait,
    DefAndMetas, EncodeXlsxDatetime, ExportFrame, ExportKind, ExportQueryResult,
    ExtractSqlValeFromJsonValue, Field, GetBackendKind, GetTestExportTableKinds, Index, IndexKind,
    ListTable, MetaValue, NamedStaff, QueryEventEmitter, QueryResp, QuerySql, ReadOnlyTrait,
    SchemaTrait, ServerMetas, ShowTableParameter, SqlBuilder, XlsxEdit, XlsxEditContent,
};

use crate::helper::create_csv;
use crate::language_server::{get_update_sql_from_result_set_impl, LexerCompletionTrait};
use chrono::{DateTime, NaiveDate, NaiveTime};
use cql3_parser::cassandra_ast::CassandraAST;
use cql3_parser::cassandra_statement::CassandraStatement;
use cql3_parser::common::{ColumnDefinition, DataType};
use dashmap::DashMap;
use futures_util::StreamExt;
use num_bigint::BigInt;
use scylla::authentication::{AuthenticatorProvider, PlainTextAuthenticator};

use scylla::client::execution_profile::ExecutionProfile;
use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use scylla::policies::address_translator::{AddressTranslator, UntranslatedPeer};
use scylla::value::{CqlDecimal, CqlValue, CqlVarint, Row};
use scylla_openssl::ssl::{SslContextBuilder, SslMethod};
use sqlx::types::BigDecimal;
use std::collections::HashMap;

use std::fmt::Debug;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
#[cfg(test)]
use tokio::sync::Mutex;
#[derive(Clone)]
pub struct CassandraDB {
    pub(crate) session: Arc<Session>,
    pub(crate) query_connection: Arc<DashMap<String, Arc<Session>>>,
    pub(crate) config: CassandraDBConfig,
    pub(crate) completion_id: AutoCompletionQueryId,
}

// Define a simple resolver that maps every address to an IP you specify manually.
#[derive(Debug)]
struct LocalhostTranslator {
    fixed_addr: SocketAddr,
}

#[async_trait::async_trait]
impl AddressTranslator for LocalhostTranslator {
    async fn translate_address(
        &self,
        _untranslated_peer: &UntranslatedPeer,
    ) -> Result<SocketAddr, scylla::errors::TranslationError> {
        Ok(self.fixed_addr)
    }
}

async fn make_session(config: &CassandraDBConfig) -> anyhow::Result<Session> {
    let target_ip = format!("{}:{}", config.host, config.port);
    let fixed_addr: SocketAddr = target_ip.parse()?;

    let profile: ExecutionProfile = ExecutionProfile::builder()
        .consistency(config.consistency.try_into()?)
        .request_timeout(config.conn_timeout.map(Duration::from))
        .build();

    let mut builder = SessionBuilder::new()
        .known_node(format!("{}:{}", config.host, config.port))
        .default_execution_profile_handle(profile.into_handle());
    if let (Some(user), Some(pass)) = (&config.username, &config.password) {
        builder = builder.authenticator_provider(Arc::new(PlainTextAuthenticator::new(
            user.clone(),
            pass.clone(),
        )) as Arc<dyn AuthenticatorProvider>);
    }

    if config.ssl_enable {
        let mut tls_builder = SslContextBuilder::new(SslMethod::tls_client())?;
        if let Some(f) = &config.ssl_ca_file {
            tls_builder.set_ca_file(f)?;
        }
        builder = builder.tls_context(tls_builder.build().into());
    }

    builder = builder.address_translator(Arc::new(LocalhostTranslator { fixed_addr }));

    let session = builder.build().await?;
    if let Some(key_space) = config.db.as_ref() {
        session.use_keyspace(key_space, true).await?;
    }
    anyhow::Ok(session)
}

impl CassandraDB {
    pub async fn new(config: CassandraDBConfig) -> anyhow::Result<Self> {
        let session = make_session(&config).await?;
        anyhow::Ok(Self {
            session: Arc::new(session),
            config,
            query_connection: Default::default(),
            completion_id: Default::default(),
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
            .query_unpaged("SELECT now() FROM system.local;", &[])
            .await?;
        anyhow::Ok(now.elapsed())
    }
}

impl std::fmt::Debug for CassandraDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "cassandra host={} port={:?}",
            self.config.host, self.config.port
        )
    }
}

impl GetBackendKind for CassandraDB {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::Cassandra
    }
}

const JSON_TYPE_FIELD: &str = "dbpal-export-json-type";

fn encode_row_as_json(cols: &[String], rows: Row) -> anyhow::Result<serde_json::Value> {
    debug_assert_eq!(cols.len(), rows.columns.len());

    let mut m = serde_json::Map::new();
    for (name, value) in cols.iter().zip(rows.columns) {
        let value = match value {
            Some(x) => encode_value_as_json(x)?,
            None => serde_json::Value::Null,
        };
        m.insert(name.clone(), value);
    }
    fn encode_value_as_json(value: CqlValue) -> anyhow::Result<serde_json::Value> {
        let ret = match value {
            CqlValue::Ascii(x) => serde_json::Value::from(x),
            CqlValue::Boolean(x) => serde_json::Value::from(x),
            CqlValue::Blob(items) => {
                let value = encode_bytes(&items);
                let mut map = serde_json::Map::new();
                map.insert(JSON_TYPE_FIELD.to_string(), "blob".into());
                map.insert("value".to_string(), value.into());
                serde_json::Value::from(map)
            }
            CqlValue::Counter(counter) => serde_json::Value::from(counter.0),
            CqlValue::Decimal(c) => {
                let value = cql_decimal_to_bigdecimal(&c).to_string();
                let mut map = serde_json::Map::new();
                map.insert(JSON_TYPE_FIELD.to_string(), "decimal".into());
                map.insert("value".to_string(), value.into());
                serde_json::Value::Object(map)
            }
            CqlValue::Date(cql_date) => serde_json::Value::from(cql_date.0),
            CqlValue::Double(x) => serde_json::Value::from(x),
            CqlValue::Duration(cql_duration) => {
                let mut obj = serde_json::Map::new();
                obj.insert(
                    "months".to_owned(),
                    serde_json::Value::from(cql_duration.months),
                );
                obj.insert(
                    "days".to_owned(),
                    serde_json::Value::from(cql_duration.days),
                );
                obj.insert(
                    "nanoseconds".to_owned(),
                    serde_json::Value::from(cql_duration.nanoseconds),
                );
                serde_json::Value::Object(obj)
            }
            CqlValue::Empty => serde_json::Value::Null,
            CqlValue::Float(x) => serde_json::Value::from(x),
            CqlValue::Int(x) => serde_json::Value::from(x),
            CqlValue::BigInt(x) => serde_json::Value::from(x),
            CqlValue::Text(x) => serde_json::Value::from(x),
            CqlValue::Timestamp(cql_timestamp) => serde_json::Value::from(cql_timestamp.0),
            CqlValue::Inet(ip_addr) => serde_json::Value::from(ip_addr.to_string()),
            CqlValue::List(cql_values)
            | CqlValue::Set(cql_values)
            | CqlValue::Vector(cql_values) => {
                let mut values = vec![];
                for c in cql_values {
                    let v = encode_value_as_json(c)?;
                    values.push(v);
                }
                serde_json::Value::Array(values)
            }
            CqlValue::Map(fields) => {
                let mut m = serde_json::Map::new();
                for (k, v) in fields {
                    let k = encode_value_as_string(k, false)?;
                    let v = encode_value_as_json(v)?;
                    m.insert(k, v);
                }
                serde_json::Value::Object(m)
            }
            CqlValue::UserDefinedType { fields, .. } => {
                let mut m = serde_json::Map::new();
                for f in fields {
                    let v = match f.1 {
                        Some(x) => encode_value_as_json(x)?,
                        None => serde_json::Value::Null,
                    };
                    m.insert(f.0, v);
                }
                serde_json::Value::Object(m)
            }
            CqlValue::SmallInt(x) => serde_json::Value::from(x),
            CqlValue::TinyInt(x) => serde_json::Value::from(x),
            CqlValue::Time(cql_time) => format_time(cql_time.0).into(),
            CqlValue::Timeuuid(cql_timeuuid) => {
                let mut map = serde_json::Map::new();
                map.insert(JSON_TYPE_FIELD.to_string(), "timeuuid".into());
                map.insert("value".to_string(), cql_timeuuid.to_string().into());
                serde_json::Value::Object(map)
            }

            CqlValue::Uuid(uuid) => {
                let mut map = serde_json::Map::new();
                map.insert(JSON_TYPE_FIELD.to_string(), "uuid".into());
                map.insert("value".to_string(), uuid.to_string().into());
                serde_json::Value::Object(map)
            }
            CqlValue::Varint(cql_varint) => {
                let v = varint_2_bigint(&cql_varint);
                let mut map = serde_json::Map::new();
                map.insert(JSON_TYPE_FIELD.to_string(), "varint".into());
                map.insert("value".to_string(), v.to_string().into());
                serde_json::Value::Object(map)
            }
            CqlValue::Tuple(cql_values) => {
                let mut values = vec![];
                for v in cql_values {
                    let v = match v {
                        Some(v) => encode_value_as_json(v)?,
                        None => serde_json::Value::Null,
                    };
                    values.push(v);
                }
                serde_json::Value::Array(values)
            }
            _ => anyhow::bail!("unsupported type,{:?}", value),
        };

        anyhow::Ok(ret)
    }
    anyhow::Ok(serde_json::Value::Object(m))
}

fn format_date(mut days_since_epoch: u32) -> anyhow::Result<String> {
    // see   https://github.com/krojew/cdrs-tokio/issues/202
    days_since_epoch &= !(1 << 31);
    let date = NaiveDate::from_ymd_opt(1970, 1, 1)
        .expect("date should be valid")
        .checked_add_days(chrono::Days::new(days_since_epoch as i64 as u64));
    match date {
        Some(x) => anyhow::Ok(x.to_string()),

        None => anyhow::bail!("days is none"),
    }
}

fn format_timestamp(x: i64) -> anyhow::Result<String> {
    let time = DateTime::from_timestamp_millis(x);
    match time {
        Some(x) => anyhow::Ok(x.naive_local().to_string()),
        None => Err(anyhow::Error::msg(String::new())),
    }
}

impl EncodeXlsxDatetime for CassandraDB {
    fn is_datatime(&self, ty: &str) -> anyhow::Result<bool> {
        anyhow::Ok(ty.eq_ignore_ascii_case("timestamp"))
    }

    fn is_date(&self, ty: &str) -> anyhow::Result<bool> {
        anyhow::Ok(ty.eq_ignore_ascii_case("date"))
    }

    fn is_time(&self, ty: &str) -> anyhow::Result<bool> {
        anyhow::Ok(ty.eq_ignore_ascii_case("time"))
    }
}
fn encode_row_as_xlsx(row: Row) -> anyhow::Result<Vec<Option<XlsxEditContent>>> {
    let mut values = vec![];
    for v in row.columns {
        let value = match v {
            Some(x) => Some(encode_value_as_xlsx(x)?),
            None => None,
        };
        values.push(value);
    }

    anyhow::Ok(values)
}

fn encode_value_as_xlsx(value: CqlValue) -> anyhow::Result<XlsxEditContent> {
    let v: XlsxEditContent = match value {
        CqlValue::Ascii(x) | CqlValue::Text(x) => XlsxEditContent::String(x),
        CqlValue::Boolean(x) => x.into(),
        CqlValue::Blob(x) => encode_bytes(&x).into(),
        CqlValue::Counter(counter) => counter.0.into(),
        CqlValue::Decimal(c) => cql_decimal_to_bigdecimal(&c).to_string().into(),
        CqlValue::Date(cql_date) => format_date(cql_date.0)?.into(),
        CqlValue::Double(x) => XlsxEditContent::number(x),
        CqlValue::Duration(..) => anyhow::bail!("duration not support"),
        CqlValue::Empty => XlsxEditContent::String("".to_string()),
        CqlValue::Float(x) => XlsxEditContent::number(x),
        CqlValue::Int(x) => XlsxEditContent::number(x),
        CqlValue::BigInt(x) => x.into(),
        CqlValue::Timestamp(cql_timestamp) => format_timestamp(cql_timestamp.0)?.into(),
        CqlValue::Inet(ip_addr) => ip_addr.to_string().into(),
        CqlValue::List(cql_values) => {
            let mut values = vec![];
            for v in cql_values {
                let v = encode_value_as_string(v, true)?;
                values.push(v);
            }
            format!("[{}]", values.join(",")).into()
        }
        CqlValue::Map(m) => {
            let mut values = vec![];
            for (k, v) in m {
                let v = encode_value_as_string(v, true)?;
                let k = encode_value_as_string(k, true)?;
                values.push(format!("{}:{}", k, v));
            }
            format!("{{{}}}", values.join(",")).into()
        }
        CqlValue::Set(cql_values) => {
            let mut values = vec![];
            for v in cql_values {
                let v = encode_value_as_string(v, true)?;
                values.push(v);
            }
            format!("{{{}}}", values.join(",")).into()
        }
        CqlValue::UserDefinedType { fields, .. } => {
            let mut values = vec![];
            for (k, v) in fields {
                let v = match v {
                    Some(x) => encode_value_as_string(x, true)?,
                    None => "null".to_string(),
                };
                values.push(format!("{}:{}", k, v));
            }
            format!("{{{}}}", values.join(",")).into()
        }
        CqlValue::SmallInt(x) => XlsxEditContent::number(x),
        CqlValue::TinyInt(x) => XlsxEditContent::number(x),
        CqlValue::Time(cql_time) => format_time(cql_time.0).into(),
        CqlValue::Timeuuid(cql_timeuuid) => cql_timeuuid.to_string().into(),
        CqlValue::Tuple(cql_values) => {
            let mut values = vec![];
            for v in cql_values {
                let v = match v {
                    Some(v) => encode_value_as_string(v, true)?,
                    None => "null".to_string(),
                };
                values.push(v);
            }
            format!("({})", values.join(",")).into()
        }
        CqlValue::Uuid(uuid) => uuid.to_string().into(),
        CqlValue::Varint(x) => varint_2_bigint(&x).to_string().into(),
        CqlValue::Vector(cql_values) => {
            let mut values = vec![];
            for v in cql_values {
                let v = encode_value_as_string(v, true)?;
                values.push(v);
            }
            format!("[{}]", values.join(",")).into()
        }
        _ => anyhow::bail!(format!("unsupported type:{:?}", value)),
    };
    anyhow::Ok(v)
}

fn encode_row(row: Row) -> anyhow::Result<Vec<Option<String>>> {
    let mut values = vec![];
    for v in row.columns {
        let value = match v {
            Some(x) => Some(encode_value_as_string(x, false)?),
            None => None,
        };
        values.push(value);
    }
    anyhow::Ok(values)
}

fn format_time(x: i64) -> String {
    let time = NaiveTime::default() + std::time::Duration::from_nanos(x as u64);
    time.to_string()
}

fn varint_2_bigint(c: &CqlVarint) -> BigInt {
    num_bigint::BigInt::from_signed_bytes_be(c.as_signed_bytes_be_slice())
}

fn cql_decimal_to_bigdecimal(cql_decimal: &CqlDecimal) -> BigDecimal {
    let (bytes, scale) = cql_decimal.as_signed_be_bytes_slice_and_exponent();
    if bytes.is_empty() {
        return BigDecimal::new(BigInt::from(0), scale as i64);
    }
    let big_int = BigInt::from_signed_bytes_be(bytes);
    // Note: The second argument to BigDecimal::new is negative.
    BigDecimal::new(big_int, scale as i64)
}

fn encode_bytes(bs: &[u8]) -> String {
    format!("0x{}", hex::encode(bs))
}

fn encode_value_as_string(value: CqlValue, is_nested: bool) -> anyhow::Result<String> {
    let v = match value {
        CqlValue::Ascii(x) | CqlValue::Text(x) => {
            if is_nested {
                quote_sql_str(&x)
            } else {
                x
            }
        }
        CqlValue::Boolean(x) => x.to_string(),
        CqlValue::Blob(x) => encode_bytes(&x),
        CqlValue::Counter(counter) => counter.0.to_string(),
        CqlValue::Decimal(c) => cql_decimal_to_bigdecimal(&c).to_string(),
        CqlValue::Date(cql_date) => {
            let r = format_date(cql_date.0)?;
            if is_nested {
                quote_sql_str(&r)
            } else {
                r
            }
        }
        CqlValue::Double(x) => x.to_string(),
        CqlValue::Duration(..) => anyhow::bail!("duration not support"),
        CqlValue::Empty => "null".to_string(),
        CqlValue::Float(x) => x.to_string(),
        CqlValue::Int(x) => x.to_string(),
        CqlValue::BigInt(x) => x.to_string(),
        CqlValue::Timestamp(cql_timestamp) => {
            let r = format_timestamp(cql_timestamp.0)?;

            if is_nested {
                quote_sql_str(&r)
            } else {
                r
            }
        }
        CqlValue::Inet(ip_addr) => {
            let r = ip_addr.to_string();
            if is_nested {
                quote_sql_str(&r)
            } else {
                r
            }
        }

        CqlValue::List(cql_values) => {
            let mut values = vec![];
            for v in cql_values {
                let v = encode_value_as_string(v, true)?;
                values.push(v);
            }
            format!("[{}]", values.join(","))
        }
        CqlValue::Map(m) => {
            let mut values = vec![];
            for (k, v) in m {
                let v = encode_value_as_string(v, true)?;
                let k = encode_value_as_string(k, true)?;
                values.push(format!("{}:{}", k, v));
            }
            format!("{{{}}}", values.join(","))
        }
        CqlValue::Set(cql_values) => {
            let mut values = vec![];
            for v in cql_values {
                let v = encode_value_as_string(v, true)?;
                values.push(v);
            }
            format!("{{{}}}", values.join(","))
        }
        CqlValue::UserDefinedType { fields, .. } => {
            let mut values = vec![];
            for (k, v) in fields {
                let v = match v {
                    Some(x) => encode_value_as_string(x, true)?,
                    None => "null".to_string(),
                };
                values.push(format!("{}:{}", k, v));
            }
            format!("{{{}}}", values.join(","))
        }
        CqlValue::SmallInt(x) => x.to_string(),
        CqlValue::TinyInt(x) => x.to_string(),
        CqlValue::Time(cql_time) => {
            let r = format_time(cql_time.0);
            if is_nested {
                quote_sql_str(&r)
            } else {
                r
            }
        }
        CqlValue::Timeuuid(cql_timeuuid) => cql_timeuuid.to_string(),
        CqlValue::Tuple(cql_values) => {
            let mut values = vec![];
            for v in cql_values {
                let v = match v {
                    Some(v) => encode_value_as_string(v, true)?,
                    None => "null".to_string(),
                };
                values.push(v);
            }
            format!("({})", values.join(","))
        }
        CqlValue::Uuid(uuid) => uuid.to_string(),
        CqlValue::Varint(x) => varint_2_bigint(&x).to_string(),
        CqlValue::Vector(cql_values) => {
            let mut values = vec![];
            for v in cql_values {
                let v = encode_value_as_string(v, true)?;
                values.push(v);
            }
            format!("[{}]", values.join(","))
        }
        _ => anyhow::bail!(format!("unsupported type:{:?}", value)),
    };
    anyhow::Ok(v)
}

#[async_trait::async_trait]

impl ExportQueryResult for CassandraDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }

    async fn export_query_result(
        &self,
        query_id: Option<&str>,
        sql: &str,
        kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let session = match query_id {
            Some(query_id) => match self.query_connection.get(&query_id.to_string()) {
                Some(x) => x.clone(),
                None => anyhow::bail!("query_id not found"),
            },
            None => self.session.clone(),
        };
        let pager = session.query_iter(sql, &[]).await?;
        let cols: Vec<_> = pager
            .column_specs()
            .iter()
            .map(|x| x.name().to_owned())
            .collect();
        let mut row_num = 0;
        match kind {
            ExportKind::Csv {
                include_header,
                delimiter,
            } => {
                if include_header {
                    let cols: Vec<_> = pager
                        .column_specs()
                        .iter()
                        .map(|x| Some(x.name().to_owned()))
                        .collect();
                    sender.send(create_csv(&cols, delimiter)?.into()).await?;
                }
            }
            ExportKind::Json { .. } | ExportKind::JsonLine => {}
            ExportKind::Sql { .. } => {
                anyhow::bail!("can't export query result to sql")
            }
            ExportKind::Xlsx { include_header, .. } => {
                if include_header {
                    for (index, col) in pager.column_specs().iter().enumerate() {
                        sender
                            .send(
                                XlsxEdit {
                                    row: row_num,
                                    column: index as u16,
                                    content: col.name().to_string().into(),
                                }
                                .into(),
                            )
                            .await?;
                    }
                    row_num += 1;
                }
            }
            ExportKind::Html { include_header } => {
                if include_header {
                    let cols: Vec<_> = pager
                        .column_specs()
                        .iter()
                        .map(|x| x.name().to_owned())
                        .collect();
                    sender.send(create_html_table_header(&cols).into()).await?;
                }
            }
            ExportKind::MarkDown => {
                let v = create_markdown_header(&cols);
                sender.send(v.into()).await?;
            }
        }
        let mut stream = pager.rows_stream::<Row>()?;
        while let Some(row) = stream.next().await {
            let row = row?;
            match kind {
                ExportKind::Csv { delimiter, .. } => {
                    let d = encode_row(row)?;
                    sender.send(create_csv(&d, delimiter)?.into()).await?;
                }
                ExportKind::Json { .. } | ExportKind::JsonLine => {
                    let value = encode_row_as_json(&cols, row)?;
                    sender.send(value.into()).await?;
                }
                ExportKind::Sql { .. } => anyhow::bail!("can't export query result as sql"),
                ExportKind::Xlsx { .. } => {
                    let data = encode_row_as_xlsx(row)?;
                    export_query_as_xlsx_send_data(&mut row_num, data, sender.clone()).await?;
                }
                ExportKind::Html { .. } => {
                    let d = encode_row(row)?;
                    sender.send(create_html_table_data(&[d]).into()).await?;
                }
                ExportKind::MarkDown => {
                    let d = encode_row(row)?;
                    sender.send(create_markdown_data(&d).into()).await?;
                }
            }
        }
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl QuerySql for CassandraDB {
    fn clone(&self) -> Box<dyn QuerySql> {
        Box::new(Clone::clone(self))
    }
    async fn get_update_sql_from_result_set(
        &self,
        sql: &str,
        old_values: &[Option<String>],
        new_values: &[Option<String>],
    ) -> anyhow::Result<Vec<String>> {
        get_update_sql_from_result_set_impl(self, sql, old_values, new_values).await
    }
    async fn simple_execute_sql(&self, sql: &str) -> anyhow::Result<Option<usize>> {
        self.session
            .query_unpaged(sql, &[])
            .await
            .map_err(|err| anyhow::Error::msg(format!("Sql:{} Err:{}", sql, err)))?;
        anyhow::Ok(None)
    }

    async fn execute_sql(
        &self,
        query_id: &str,
        sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let conn = match self.query_connection.get(&query_id.to_string()) {
            Some(x) => x.clone(),
            None => anyhow::bail!("query connection closed???"),
        };
        let resp = conn
            .query_unpaged(sql, &[])
            .await
            .map(|_| QueryResp::execute_ok(Some(sql.to_owned())))
            .map_err(|e| format!("{:?}", e));
        anyhow::Ok(resp)
    }

    fn support_tx(&self) -> bool {
        false
    }
    async fn query_sql(
        &self,
        query_id: &str,
        sql: &str,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let conn = match self.query_connection.get(&query_id.to_string()) {
            Some(x) => x.clone(),
            None => anyhow::bail!("query connection closed???"),
        };
        let pager = conn.query_iter(sql, &[]).await?;
        emit.send(
            QueryResp {
                fields: pager
                    .column_specs()
                    .iter()
                    .map(|x| Field::from_name_type(x.name(), format!("{:?}", x.typ())))
                    .collect(),
                sql: Some(sql.to_string()),
                tables: crate::language_server::LangServerCommon::extract_tables(self, sql)
                    .unwrap_or_default(),
                ..Default::default()
            }
            .into(),
        )?;
        let mut stream = pager.rows_stream::<Row>()?;
        while let Some(x) = stream.next().await {
            let row = x?;
            let d = encode_row(row)?;
            emit.send(d.into())?;
        }
        anyhow::Ok(())
    }

    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        let session = make_session(&self.config).await?;
        self.query_connection
            .insert(id.to_string(), Arc::new(session));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        self.query_connection.remove(&id.to_string());
        anyhow::Ok(())
    }
}

impl CassandraDB {
    pub(crate) async fn list_table_impl(&self, keyspace: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            "SELECT table_name FROM system_schema.tables WHERE keyspace_name = '{keyspace}' order by table_name;"
        );
        log::info!("sql:{}", sql);
        let pager = self.session.query_iter(sql, &[]).await?;
        let mut stream = pager.rows_stream::<(String,)>()?;
        let mut ret = Vec::new();
        while let Some(name) = stream.next().await {
            let name = name?;
            ret.push(ListTable::from(name.0));
        }
        anyhow::Ok(ret)
    }
}

impl ReadOnlyTrait for CassandraDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}
impl CassandraDB {
    async fn export_table_as_sql(
        &self,
        table_name: &str,
        para: Option<ShowTableParameter>,
        include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            let create_table = SchemaTrait::show_create_table_sql(self, table_name).await?;
            sender.send(create_table.to_string().into()).await?;
        }
        let cols = self.show_table_columns(table_name).await?;
        let sql = self.gen_show_sql(table_name, Some(&cols), para.as_ref());
        log::info!("sql:{}", sql);
        let pager = self.session.query_iter(sql.as_str(), &[]).await?;
        let mut stream = pager.rows_stream::<Row>()?;
        while let Some(row) = stream.next().await {
            let row = row?;
            let data = encode_row(row)?;
            let mut m = HashMap::new();
            cols.iter().zip(data.iter()).for_each(|(f, v)| {
                m.insert(f.field.clone(), v.clone());
            });
            let sql = self.insert_sql(table_name, &cols, &m)?;
            let sql = format!("{sql};\n");
            sender.send(sql.into()).await?;
        }
        anyhow::Ok(())
    }
}

impl CassandraDB {
    pub(crate) async fn list_materialized_view_impl(
        &self,
        key_space: &str,
    ) -> anyhow::Result<Vec<ListTable>> {
        let sql =
            format!("select view_name from system_schema.views where keyspace_name='{key_space}'");
        log::info!("sql:{}", sql);
        let rows = self.session.query_iter(sql, &[]).await?;
        let mut stream = rows.rows_stream::<(String,)>()?;
        let mut ret = Vec::new();
        while let Some(name) = stream.next().await {
            let name = name?;
            ret.push(ListTable::from(name.0));
        }

        anyhow::Ok(ret)
    }
}

/// CURRENT_* literals that can be used directly in Cassandra/CQL statements.
pub const CQL_CURRENT_LITERALS: &[&str] = &[
    "CURRENT_TIMESTAMP", // Microsecond timestamp, such as 1692962187000000.
    "CURRENT_TIME",      // Current 64-bit time value (time).
    "CURRENT_DATE",      // Current date (date).
];

impl SqlBuilder for CassandraDB {
    fn encode_value(&self, typ: &str, value: &str) -> anyhow::Result<String> {
        let typ = typ.to_lowercase();
        log::trace!("encode value '{:?}' on type '{}'", value, typ);
        match typ.as_str() {
            "ascii" | "varchar" | "text" => anyhow::Ok(quote_sql_str(value)),
            "timestamp" | "date" | "time" | "inet" => anyhow::Ok(format!("'{}'", value)),
            _ => anyhow::Ok(value.to_string()),
        }
    }

    fn get_predefine_values(&self) -> &'static [&'static str] {
        CQL_CURRENT_LITERALS
    }

    fn get_schema(&self) -> Option<&str> {
        self.config.db.as_deref()
    }

    fn gen_show_sql(
        &self,
        table_name: &str,
        fields: Option<&[Field]>,
        para: Option<&ShowTableParameter>,
    ) -> String {
        let mut sql = format!(
            r#"SELECT {} FROM {}"{table_name}" "#,
            self.backend_kind().format_fields(fields),
            self.config
                .db
                .as_ref()
                .map(|x| format!("{}.", x))
                .unwrap_or_default(),
        );
        if let Some(para) = para {
            let ShowTableParameter {
                filter: where_str,
                sort: order_by,
                allow_filtering,
                ..
            } = para;
            let x = format!(
                r#" {} {} {} "#,
                if let Some(where_str) = where_str.as_ref() {
                    format!(" WHERE {} ", where_str)
                } else {
                    "".to_string()
                },
                if let Some(order) = order_by {
                    format!(" ORDER BY {} ", order)
                } else {
                    "".to_string()
                },
                if *allow_filtering {
                    "ALLOW FILTERING"
                } else {
                    ""
                }
            );
            sql.push_str(x.as_str());
        }

        sql
    }
}

impl CassandraDB {
    pub(crate) async fn list_created_types_impl(
        &self,
        schema: &str,
    ) -> anyhow::Result<Vec<CreatedTypeDef>> {
        let sql = format!(
            "SELECT type_name FROM system_schema.types where  keyspace_name = '{}'",
            schema
        );
        log::info!("sql:{}", sql);
        let pager = self.session.query_iter(sql, &[]).await?;
        let mut stream = pager.rows_stream::<(String,)>()?;
        let mut ret = Vec::new();
        while let Some(row) = stream.next().await {
            let row = row?;
            let sql = format!("DESCRIBE TYPE {}", row.0);
            log::info!("sql:{}", sql);
            let r = self
                .session
                .query_unpaged(sql.as_str(), &[])
                .await?
                .into_rows_result()?;
            let pager = r.rows::<(String, String, String, String)>()?;
            let mut create_statement = "".to_string();
            #[allow(clippy::never_loop)]
            for row in pager {
                let row = row?;
                create_statement = row.3;
                break;
            }
            ret.push(CreatedTypeDef {
                name: row.0,
                sql: create_statement.into(),
            });
        }
        anyhow::Ok(ret)
    }
}

impl CassandraDB {
    fn make_create_index_sql(&self, table: &str, req: &CreateIndexReq) -> String {
        let CreateIndexReq { name, .. } = req;
        let sql = format!(
            r#"CREATE INDEX "{name}" ON "{table}" ({});"#,
            req.format_cols(false, self.backend_kind().quote_style())
        );
        sql
    }
}

impl ExtractSqlValeFromJsonValue for CassandraDB {
    fn extrat_sql_value_from_json_value(
        &self,
        value: &serde_json::Value,
        _ty: &str,
    ) -> anyhow::Result<Option<String>> {
        fn encode_value(v: &serde_json::Value, is_nested: bool) -> anyhow::Result<Option<String>> {
            let r = match v {
                serde_json::Value::Null => None,
                serde_json::Value::Bool(x) => Some(x.to_string()),
                serde_json::Value::Number(number) => Some(number.to_string()),
                serde_json::Value::String(x) => Some(if is_nested {
                    quote_sql_str(x)
                } else {
                    x.clone()
                }),
                serde_json::Value::Array(values) => {
                    let mut results = Vec::with_capacity(values.len());
                    for v in values.iter() {
                        results.push(encode_value(v, true)?.unwrap_or("null".to_string()));
                    }
                    Some(format!("[{}]", results.join(",")))
                }
                serde_json::Value::Object(map) => {
                    if map.get(JSON_TYPE_FIELD).is_some() {
                        match map.get("value") {
                            Some(x) => match x {
                                serde_json::Value::Null => return anyhow::Ok(None),
                                serde_json::Value::String(x) => {
                                    return anyhow::Ok(x.clone().into())
                                }
                                _ => {
                                    anyhow::bail!("not a valid value for field value")
                                }
                            },
                            None => anyhow::bail!("should contains value field"),
                        }
                    }

                    let mut results = Vec::with_capacity(map.len());
                    for (k, v) in map.iter() {
                        let v = encode_value(v, true)?.unwrap_or("null".to_string());
                        results.push(format!("{}:{}", k, v));
                    }
                    Some(format!("{{{}}}", results.join(",")))
                }
            };
            anyhow::Ok(r)
        }

        anyhow::Ok(encode_value(value, false)?)
    }
}

#[allow(unused)]
pub(crate) fn extract_cql3_parser_ident(x: &cql3_parser::common::Identifier) -> &str {
    match x {
        cql3_parser::common::Identifier::Quoted(x) => x.as_str(),
        cql3_parser::common::Identifier::Unquoted(x) => x.as_str(),
    }
}
#[async_trait::async_trait]
impl SchemaTrait for CassandraDB {
    fn list_sql_predefine_values(&self) -> &'static [&'static str] {
        <Self as SqlBuilder>::get_predefine_values(self)
    }

    async fn drop_created_type(&self, name: &str) -> anyhow::Result<String> {
        self.can_write()?;

        let sql = format!(r#"DROP TYPE  "{name}""#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn list_created_types(&self) -> anyhow::Result<Vec<CreatedTypeDef>> {
        self.list_created_types_impl(self.config.key_space_str()?)
            .await
    }

    async fn drop_materialized_view(&self, name: &str) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let sql = format!(r#"DROP MATERIALIZED VIEW "{name}";"#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_materialized_view_impl(self.config.key_space_str()?)
            .await
    }

    async fn export_table_or_view(
        &self,
        name: &str,
        para: Option<ShowTableParameter>,
        kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::Sql {
                include_table_schema,
            } => {
                self.export_table_as_sql(name, para, include_table_schema, sender)
                    .await
            }
            _ => {
                let sql = self.gen_show_sql(name, None, para.as_ref());
                self.export_query_result(None, &sql, kind, sender).await
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn create_index(&self, table: &str, req: &CreateIndexReq) -> anyhow::Result<String> {
        self.can_write()?;
        let sql = self.make_create_index_sql(table, req);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let sql = format!(r#"DROP INDEX IF EXISTS "{index}";"#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn show_create_table_sql(
        &self,
        table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let sql = format!(r#"DESCRIBE TABLE "{table_name}""#);
        log::info!("sql:{}", sql);
        let rows = self.session.query_iter(sql.as_ref(), &[]).await?;

        let mut stream = rows.rows_stream::<(String, String, String, String)>()?;
        #[allow(clippy::never_loop)]
        while let Some(row) = stream.next().await {
            let row = row?;
            return anyhow::Ok(CreateTableAndIndexSql {
                create_table: row.3,
                indexes: vec![],
            });
        }

        anyhow::bail!("not found create table sql")
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = self.show_create_table_sql(table).await?.create_table;
        // replace table name.
        let sql = sql.replacen(table, dup, 1);
        log::info!("sql:{}", sql);
        log::info!("sql:{}", sql);
        log::info!("dup table create table sql:{}", sql);
        // here we create the table
        self.session.query_unpaged(sql, &[]).await?;
        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
        let j1 = {
            // load all data
            let x = Clone::clone(self);
            let table = table.to_owned();
            tokio::spawn(async move {
                x.export_table_or_view(
                    &table,
                    None,
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
                    let sql = sql.as_str()?.replacen(table.as_str(), &dup, 1);
                    x.session.query_unpaged(sql, &[]).await?;
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
        self.list_table_impl(self.config.key_space_str()?).await
    }

    // cassandra can use this function for materialized view too.
    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let sql = format!(
            r#" SELECT column_name, type, kind
        FROM system_schema.columns
        WHERE keyspace_name = '{}' AND table_name = '{}';"#,
            self.config.key_space_str()?,
            table_name
        );
        log::info!("sql:{}", sql);
        let pager = self.session.query_iter(sql.as_str(), &[]).await?;
        let mut stream = pager.rows_stream::<(String, String, String)>()?;
        let mut ret = Vec::new();
        while let Some(row) = stream.next().await {
            let (name, data_type, kind) = row?;
            let is_primary = kind == "partition_key" || kind == "clustering";
            ret.push(Field {
                field: name,
                data_type,
                nullable: !is_primary,
                is_primary,
                ..Default::default()
            });
        }
        anyhow::Ok(ret)
    }

    async fn show_table(
        &self,
        table_name: &str,
        para: ShowTableParameter,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let cols = self.show_table_columns(table_name).await?;
        let cql = self.gen_show_sql(table_name, Some(&cols), Some(&para));
        emit.send(
            QueryResp {
                fields: cols,
                sql: Some(cql.clone()),
                ..Default::default()
            }
            .into(),
        )?;
        let pager = self.session.query_iter(cql.as_str(), &[]).await?;
        let mut stream = pager.rows_stream::<Row>()?;
        while let Some(row) = stream.next().await {
            let row = row?;
            let d = encode_row(row)?;
            emit.send(d.into())?;
        }
        anyhow::Ok(())
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let sql = format!(
            "SELECT index_name from system_schema.indexes WHERE  table_name = '{table_name}' AND keyspace_name = '{}'",
            self.config.key_space_str()?
        );
        log::info!("sql:{}", sql);
        let pager = self.session.query_iter(sql, &[]).await?;
        let mut stream = pager.rows_stream::<(String,)>()?;

        let mut ret = Vec::new();
        while let Some(row) = stream.next().await {
            let row = row?;
            ret.push(Index {
                name: row.0.into(),
                kind: IndexKind::Index,
                columns: vec![],
            });
        }
        anyhow::Ok(ret)
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let sql = format!(r#"TRUNCATE "{table_name}""#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let sql = format!(r#"DROP TABLE "{}";"#, table);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn add_table_column(
        &self,
        table: &str,
        name: &str,
        col: ColumnType,
    ) -> anyhow::Result<String> {
        self.can_write()?;
        let sql = format!("ALTER TABLE {table} ADD {name} {}", col.typ);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn modify_table_column(
        &self,
        table: &str,
        old_name: &str,
        new_name: &str,
        _typ: Option<ColumnType>,
    ) -> anyhow::Result<String> {
        self.can_write()?;
        let sql = format!("ALTER TABLE {table} RENAME {old_name} TO {new_name}");
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<String> {
        self.can_write()?;
        let sql = format!("ALTER TABLE {table} DROP {col};");
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn show_view(
        &self,
        view_name: &str,
        para: ShowTableParameter,
        status_emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        self.show_table(view_name, para, status_emit).await
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        self.show_table_columns(view_name).await
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_udf_impl(self.config.key_space_str()?).await
    }

    async fn show_udf(&self, name: &NamedStaff) -> anyhow::Result<DefAndMetas> {
        // add parameter to udf have this error
        // cassandra@cqlsh:test> DESCRIBE FUNCTION  test.test_udf(int);
        // Improper DESCRIBE command.
        let sql = format!(r#" DESCRIBE FUNCTION  "{}""#, name.name);
        log::info!("sql:{}", sql);
        let body = self.session.query_iter(sql, &[]).await?;
        let mut stream = body.rows_stream::<(String, Option<String>, String, String)>()?;
        let parameters = name.cassandra_parameters.clone().unwrap_or_default();

        while let Some(row) = stream.next().await {
            let (_, typ, _, create_statement) = row?;
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
                metas: vec![MetaValue::new("type", typ)],
            });
        }

        anyhow::bail!(format!(
            "no match for parameters:{:?},this should be a bug",
            parameters
        ));
    }

    async fn drop_udf(&self, name: &NamedStaff) -> anyhow::Result<String> {
        self.can_write()?;

        let p = format!(
            "({})",
            name.cassandra_parameters
                .clone()
                .unwrap_or_default()
                .join(",")
        );
        let sql = format!(r#"DROP FUNCTION "{}"{p} "#, name.name);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        LexerCompletionTrait::get_types(self).await
    }
}

// TODO this parse only support one parameter or `cassandra`
// support multi parameter for aggregate function.
fn extract_parameter_for_aggregate(sql: &str) -> anyhow::Result<DataType> {
    let x = CassandraAST::new(sql);
    if x.statements.len() != 1 {
        anyhow::bail!("should contains one statement.")
    }
    let s = &x.statements[0];
    match &s.statement {
        CassandraStatement::CreateAggregate(x) => anyhow::Ok(x.data_type.clone()),
        _ => {
            anyhow::bail!("this should be aggregate function statement.")
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
        CassandraStatement::CreateFunction(x) => anyhow::Ok(x.params.clone()),
        _ => {
            anyhow::bail!("this should be create function statement.")
        }
    }
}

#[async_trait::async_trait]
impl DBTrait for CassandraDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let pager = self.session.query_iter("DESCRIBE KEYSPACES", &[]).await?;
        let mut stream = pager.rows_stream::<(String, String, String)>()?;
        let mut ret = Vec::new();
        while let Some(row) = stream.next().await {
            let row = row?;
            ret.push(row.0);
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
        &self,
        name: &str,
        extra: Option<CreateSchemaArg>,
    ) -> anyhow::Result<()> {
        self.can_write()?;
        let extra = match &extra {
            Some(x) => x,
            None => anyhow::bail!("cassandra need extra parameter."),
        };
        let (replication_strategy, replication_factor) = match extra {
            CreateSchemaArg::Cassandra {
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
        self.session.query_unpaged(sql, &[]).await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("DROP KEYSPACE `{name}`");
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
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
        let pager = self.session.query_iter(sql, &[]).await?;
        let mut stream =
            pager.rows_stream::<(String, Vec<String>, Option<String>, Option<String>)>()?;
        let mut ret = Vec::new();
        while let Some(row) = stream.next().await {
            let (name, argument_types, return_type, language) = row?;
            ret.push(NamedStaff {
                name,
                parameters: None,
                cassandra_parameters: Some(argument_types),
                metas: vec![
                    MetaValue::new("return_type", return_type),
                    MetaValue::new("language", language),
                ],
            });
        }
        anyhow::Ok(ret)
    }
}

impl TryFrom<CassandraConsistency> for scylla::statement::Consistency {
    type Error = anyhow::Error;

    fn try_from(value: CassandraConsistency) -> Result<Self, Self::Error> {
        scylla::statement::Consistency::try_from(value.0)
            .map_err(|e| anyhow::Error::msg(e.to_string()))
    }
}

impl Default for CassandraConsistency {
    fn default() -> Self {
        Self(scylla::statement::Consistency::One as u16)
    }
}

// cqlsh -u cassandra -p  cassandra
#[cfg(test)]
pub(crate) async fn make_test_object(db: Option<String>) -> CassandraDB {
    use crate::TEST_HOST;
    let ss = CassandraDB::new(CassandraDBConfig {
        host: TEST_HOST.to_string(),
        port: 9042,
        username: Some("cassandra".to_string()),
        password: Some("cassandra".to_string()),
        read_only: false,
        db,
        dialect: CassandraDialect::Cassandra,
        conn_timeout: None,
        ssl_ca_file: None,
        consistency: Default::default(),
        ssl_enable: false,
    })
    .await
    .unwrap();
    ss
}

#[cfg(test)]
pub(crate) fn test_lock() -> &'static Mutex<()> {
    static LOCK: Mutex<()> = Mutex::const_new(());
    &LOCK
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
        let pager = self.session.query_iter(sql.as_str(), &[]).await?;
        let mut stream = pager.rows_stream::<(
            String,
            Option<Vec<String>>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        )>()?;
        let mut ret = Vec::new();
        while let Some(row) = stream.next().await {
            let (name, argument_types, return_type, state_func, final_func, initcond) = row?;

            ret.push(NamedStaff {
                parameters: None,
                name,
                metas: vec![
                    MetaValue::new("return_type", return_type),
                    MetaValue::new("state_func", state_func),
                    MetaValue::new("final_func", final_func),
                    MetaValue::new("initcond", initcond),
                ],
                cassandra_parameters: Some(argument_types.unwrap_or_default()),
            });
        }

        anyhow::Ok(ret)
    }

    pub async fn list_aggregate(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_aggregate_impl(self.config.key_space_str()?).await
    }

    pub async fn show_aggregate(
        &self,
        name: &str,
        parameters: Vec<String>,
    ) -> anyhow::Result<DefAndMetas> {
        let sql = format!("DESCRIBE AGGREGATE  {name}");
        log::info!("sql:{}", sql);
        let body = self.session.query_iter(sql.as_str(), &[]).await?;
        let mut steam = body.rows_stream::<(String, Option<String>, String, String)>()?;

        while let Some(row) = steam.next().await {
            let (_, typ, _, create_statement) = row?;
            let parameters_try_match = [extract_parameter_for_aggregate(&create_statement)?];
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
                metas: vec![MetaValue::new("type", typ)],
            });
        }
        anyhow::bail!(format!(
            "no match for parameter:{:?},this should be a bug",
            parameters
        ));
    }

    pub async fn drop_aggregate(
        &self,
        name: &str,
        parameters: Vec<String>,
    ) -> anyhow::Result<String> {
        self.can_write()?;
        let p = format!("({})", parameters.join(","));
        let sql = format!("DROP AGGREGATE {name}{p} ");
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }
}

impl GetTestExportTableKinds for CassandraDB {}

#[cfg(test)]
pub(crate) mod test_impls {
    use std::io::Write;

    use super::*;
    use crate::{
        db::{CreateIndexColumn, CreateTableReq, CreateTableReqColumn, TableImportPara},
        enable_test_log,
    };

    pub fn new_test_show_parameter() -> ShowTableParameter {
        ShowTableParameter {
            ..Default::default()
        }
    }

    pub async fn test_list_schema_inner(db: CassandraDB) {
        let _ = db
            .create_schema(
                "test",
                Some(CreateSchemaArg::Cassandra {
                    replication_strategy: "SimpleStrategy".to_string(),
                    replication_factor: 1,
                }),
            )
            .await;

        let schemas = DBTrait::list_schema(&db).await.unwrap();
        println!("list schema:{:?}", schemas)
    }

    pub async fn test_aggregate_inner(db: CassandraDB) {
        {
            let _ = db
                .drop_aggregate("test_aggregate", vec!["int".to_string()])
                .await;
            let _ = db
                .drop_udf(&NamedStaff::from("sum_state_function".to_string()))
                .await;

            let _ = db
                .drop_udf(&NamedStaff {
                    name: "sum_final_function".to_string(),
                    cassandra_parameters: Some(vec!["int".to_string()]),
                    ..Default::default()
                })
                .await;
        }
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _resp = db
            .execute_sql(
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
            .unwrap()
            .unwrap();

        let _resp = db
            .execute_sql(
                query_id,
                r#"
                CREATE OR REPLACE FUNCTION sum_final_function(state int)
                RETURNS NULL ON NULL INPUT
                RETURNS int
                LANGUAGE java
                AS $$
                    return state;
                $$;
    "#,
            )
            .await
            .unwrap()
            .unwrap();

        let _resp = db
            .execute_sql(
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

        let list = db.list_aggregate().await.unwrap();
        list.iter().for_each(|x| println!("aggregate:{:?}", x));

        let details = db
            .show_aggregate("test_aggregate", vec!["int".to_string()])
            .await
            .unwrap();

        println!("details:{:?}", details);
    }

    pub async fn test_check_conn_inner(db: CassandraDB) {
        enable_test_log();

        let d = crate::db::CheckConn::check_conn(&db).await.unwrap();
        println!("d:{:?}", d);
    }

    pub async fn test_a_lot_data_inner(db: CassandraDB) {
        let table_name = "test_a_lot_data";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_a_lot_data (
                    id UUID PRIMARY KEY,
                    ascii_column VARCHAR,
                );
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let size = 1550;
        for k in 0..size {
            let mut values = HashMap::new();
            values.insert(
                "id".to_string(),
                Some("7e8af693-c33e-4533-a423-27fb19990377".to_string()),
            );
            values.insert(
                "ascii_column".to_string(),
                Some(format!("example_ascii {}", k)),
            );
            db.table_insert(table_name, &values).await.unwrap();
            if k % 500 == 0 {
                println!("insert {}", k);
            }
        }
    }

    pub async fn test_sort_inner(db: CassandraDB) {
        let table_name = "test_sort";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();

        for sql in
            r#"INSERT INTO test_sort (sort_key, id, name, value) VALUES (1, uuid(), 'Alice', 30);
        INSERT INTO test_sort (sort_key, id, name, value) VALUES (1, uuid(), 'Bob', 20);
        INSERT INTO test_sort (sort_key, id, name, value) VALUES (1, uuid(), 'Charlie', 25);
        INSERT INTO test_sort (sort_key, id, name, value) VALUES (2, uuid(), 'Diana', 35);"#
                .lines()
        {
            db.execute_sql(query_id, sql).await.unwrap().unwrap();
        }

        let resp = db
            .show_table_collect(table_name, new_test_show_parameter())
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }

    pub async fn test_show_table_column_inner(db: CassandraDB) {
        let fields = db.show_table_columns("example_table").await.unwrap();
        for field in fields {
            println!("field:{:?}", field);
        }
    }

    pub async fn test_duplicate_inner(db: CassandraDB) {
        let table_name = "test_duplicate_table";
        let table_dup = "test_duplicate";
        let _ = db.drop_table(table_name, false).await;
        let _ = db.drop_table(table_dup, false).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();
        for sql in
            r#"INSERT INTO test_duplicate_table (id, name, value) VALUES (uuid(), 'Alice', 10);
        INSERT INTO test_duplicate_table (id, name, value) VALUES (uuid(), 'Bob', 20);
        INSERT INTO test_duplicate_table (id, name, value) VALUES (uuid(), 'Charlie', 30);"#
                .lines()
        {
            db.execute_sql(query_id, sql).await.unwrap().unwrap();
        }

        db.duplicate_table(table_name, table_dup).await.unwrap();

        let resp = db
            .show_table_collect(table_dup, new_test_show_parameter())
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }

    pub async fn test_list_created_types_inner(db: CassandraDB) {
        let _ = db.drop_created_type("test_type").await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();

        let list = db.list_created_types().await.unwrap();
        for l in list.iter() {
            println!("create type:{:?}", l);
        }
    }

    pub async fn test_types1_inner(db: CassandraDB) {
        let now = std::time::Instant::now();

        let table_name = "test_types1";
        let _ = db.drop_table(table_name, false).await;

        println!("drop table:{:?}", now.elapsed());
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();

        println!("create table:{:?}", now.elapsed());
        let mut values = HashMap::new();
        values.insert(
            "id".to_string(),
            Some("7e8af693-c33e-4533-a423-27fb19990377".to_string()),
        );
        values.insert(
            "ascii_column".to_string(),
            Some("example_ascii".to_string()),
        );
        values.insert(
            "bigint_column".to_string(),
            Some("12345678901234".to_string()),
        );
        values.insert("blob_column".to_string(), Some("0x48656c6c6f".to_string()));
        values.insert("boolean_column".to_string(), Some("true".to_string()));
        values.insert("decimal_column".to_string(), Some("123.5".to_string()));
        values.insert("double_column".to_string(), Some("3.25".to_string()));
        db.table_insert(table_name, &values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;
            println!("resp:{:?}", resp);
        }
        GetTestExportTableKinds::test_export(&db, table_name).await;
        db.test_table_export_and_import(table_name).await;
        db.test_table_equal(table_name, &values, &[]).await;
    }

    pub async fn test_types1_list_inner(db: CassandraDB) {
        let table_name = "test_types_list1";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert(
            "id".to_string(),
            Some("7e8af693-c33e-4533-a423-27fb19990377".to_string()),
        );
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

        db.table_insert(table_name, &values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;
            println!("resp:{:?}", resp);
        }
        GetTestExportTableKinds::test_export(&db, table_name).await;
        db.test_table_export_and_import(table_name).await;
        db.test_table_equal(table_name, &values, &[]).await;
    }

    pub async fn test_types2_inner(db: CassandraDB) {
        let table_name = "test_types2";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert(
            "id".to_string(),
            Some("7e8af693-c33e-4533-a423-27fb19990377".to_string()),
        );
        values.insert("float_value".to_string(), Some("3.14".to_string()));
        values.insert("int_value".to_string(), Some("42".to_string()));
        values.insert(
            "timestamp_value".to_string(),
            Some("2023-01-01 10:00:00".to_string()),
        );
        values.insert("varchar_value".to_string(), Some("Sample Text".to_string()));
        values.insert("varint_value".to_string(), Some("1234567890".to_string()));

        db.table_insert(table_name, &values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;
            println!("resp:{:?}", resp);
        }
        GetTestExportTableKinds::test_export(&db, table_name).await;
        db.test_table_export_and_import(table_name).await;
        db.test_table_equal(table_name, &values, &[]).await;
    }

    pub async fn test_types2_list_inner(db: CassandraDB) {
        let table_name = "test_types_list2";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert(
            "id".to_string(),
            Some("79b5f399-cd33-472a-b31c-4eb0b89f18e6".to_string()),
        );
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

        db.table_insert(table_name, &values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;
            println!("resp:{:?}", resp);
        }
        GetTestExportTableKinds::test_export(&db, table_name).await;
        db.test_table_export_and_import(table_name).await;
        db.test_table_equal(table_name, &values, &[]).await;
    }

    pub async fn test_types3_inner(db: CassandraDB) {
        let table_name = "test_types3";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert(
            "id".to_string(),
            Some("79b5f399-cd33-472a-b31c-4eb0b89f18e6".to_string()),
        );
        values.insert(
            "timeuuid_value".to_string(),
            Some("19905370-a72a-11f1-a1e7-dbedeac510d7".to_string()),
        );
        values.insert("inet_value".to_string(), Some("192.168.1.1".to_string()));
        values.insert("date_value".to_string(), Some("2023-01-01".to_string()));
        values.insert("time_value".to_string(), Some("10:30:00".to_string()));
        values.insert("smallint_value".to_string(), Some("123".to_string()));
        values.insert("tinyint_value".to_string(), Some("5".to_string()));

        db.table_insert(table_name, &values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;
            println!("resp:{:?}", resp);
        }
        GetTestExportTableKinds::test_export(&db, table_name).await;
        db.test_table_export_and_import(table_name).await;
        db.test_table_equal(table_name, &values, &[]).await;
    }

    pub async fn test_types_list3_inner(db: CassandraDB) {
        let table_name = "test_types_list3";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert(
            "id".to_string(),
            Some("79b5f399-cd33-472a-b31c-4eb0b89f18e6".to_string()),
        );
        values.insert(
            "timeuuid_value".to_string(),
            Some("[49b06ce8-a72a-11f1-a1e7-dbedeac510d7]".to_string()),
        );
        values.insert(
            "inet_value".to_string(),
            Some("['192.168.1.1']".to_string()),
        );
        values.insert("date_value".to_string(), Some("['2023-01-01']".to_string()));
        values.insert("time_value".to_string(), Some("['10:30:00']".to_string()));
        values.insert("smallint_value".to_string(), Some("[123]".to_string()));
        values.insert("tinyint_value".to_string(), Some("[5]".to_string()));

        db.table_insert(table_name, &values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;
            println!("resp:{:?}", resp);
        }
        GetTestExportTableKinds::test_export(&db, table_name).await;
        db.test_table_export_and_import(table_name).await;
        db.test_table_equal(table_name, &values, &[]).await;
    }

    pub async fn test_types4_inner(db: CassandraDB) {
        let table_name = "test_types4";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_types4 (
                    id UUID PRIMARY KEY,
                    map_field map<int, int>,
                    set_field set<int>,
                    person TypePerson,
                    tuple_field tuple<int, int>
                );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert(
            "id".to_string(),
            Some("79b5f399-cd33-472a-b31c-4eb0b89f18e6".to_string()),
        );
        values.insert("map_field".to_string(), Some("{1:2}".to_string()));
        values.insert("set_field".to_string(), Some("{1,2}".to_string()));
        values.insert(
            "person".to_string(),
            Some("{first_name:'yu',last_name:'yang',age:100,email:'xxx'}".to_string()),
        );
        values.insert("tuple_field".to_string(), Some("(1,100)".to_string()));
        db.table_insert(table_name, &values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;
            println!("resp:{:?}", resp);
        }
        GetTestExportTableKinds::test_export(&db, table_name).await;
        db.test_table_export_and_import_csv(table_name).await;
        db.test_table_export_and_import_excel(table_name).await;
        db.test_table_equal(table_name, &values, &[]).await;
    }

    pub async fn test_table_column_inner(db: CassandraDB) {
        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _resp = db
            .execute_sql(
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
            .unwrap()
            .unwrap();

        let sql = db
            .add_table_column(
                table_name,
                "name",
                ColumnType {
                    typ: "tinyint".to_string(),
                    nullable: Default::default(),
                    after: None,
                    default_value: Some("1".to_string()),
                },
            )
            .await
            .unwrap();
        println!("add table column sql :{}", sql);

        let sql = db
            .modify_table_column(
                table_name,
                "id2",
                "name2",
                Some(ColumnType {
                    typ: "smallint".to_string(),
                    nullable: Default::default(),
                    after: None,
                    default_value: Some("2".to_string()),
                }),
            )
            .await
            .unwrap();
        println!("modify table column sql :{}", sql);

        let sql = db.drop_table_column(table_name, "value").await.unwrap();
        println!("drop table column sql :{}", sql);
    }

    pub async fn test_counter_inner(db: CassandraDB) {
        let table_name = "test_counter";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_counter (
                page_id UUID PRIMARY KEY,
                view_count COUNTER
            );
                "#,
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        let mut wheres = HashMap::new();
        values.insert("view_count".to_string(), Some("view_count + 1".to_string()));
        wheres.insert(
            "page_id".to_string(),
            Some("79b5f399-cd33-472a-b31c-4eb0b89f18e6".to_string()),
        );
        db.table_update(table_name, &values, &wheres).await.unwrap();

        let resp = db
            .show_table_collect(table_name, new_test_show_parameter())
            .await
            .unwrap();
        println!("xxxx:{:?}", resp);
    }

    pub async fn test_udf_inner(db: CassandraDB) {
        let name = "test_udf";
        {
            let err = db
                .drop_udf(&NamedStaff {
                    name: name.to_string(),
                    cassandra_parameters: Some(vec!["text".to_string()]),
                    ..Default::default()
                })
                .await;
            if let Err(err) = err {
                println!("drop udf error:{}", err);
            }
            let err = db
                .drop_udf(&NamedStaff {
                    name: name.to_string(),
                    cassandra_parameters: Some(vec!["list<int>".to_string()]),
                    ..Default::default()
                })
                .await;
            if let Err(err) = err {
                println!("drop udf error:{}", err);
            }
        }

        db.session
            .query_unpaged(
                r#"
            CREATE FUNCTION IF NOT EXISTS  test_udf(input text)
            RETURNS NULL ON NULL INPUT
            RETURNS text
            LANGUAGE java
            AS 'return input;';
        "#,
                &[],
            )
            .await
            .unwrap();

        db.session
            .query_unpaged(
                r#"
            CREATE FUNCTION IF NOT EXISTS  test_udf(input list<int>)
            RETURNS NULL ON NULL INPUT
            RETURNS text
            LANGUAGE java
            AS 'return String.valueOf(input);';
        "#,
                &[],
            )
            .await
            .unwrap();
        let list = db.list_udf().await.unwrap();
        list.iter().for_each(|x| println!("udf:{:?}", x));

        let detail = db
            .show_udf(&NamedStaff {
                name: "test_udf".to_string(),
                cassandra_parameters: Some(vec!["text".to_string()]),
                ..Default::default()
            })
            .await
            .unwrap();
        println!("detail:{:?}", detail);
    }

    pub async fn test_create_type_inner(db: CassandraDB) {
        db.session
            .query_unpaged(
                r#"
            CREATE TYPE IF NOT EXISTS TypePerson (
                first_name text,
                last_name text,
                age int,
                email text
            );
        "#,
                &[],
            )
            .await
            .unwrap();
    }

    pub async fn test_rename_inner(db: CassandraDB) {
        let table_name = "test_rename";
        let table_name2 = format!("{}2", table_name);
        let _ = db.drop_table(table_name, false).await;
        let _ = db.drop_table(&table_name2, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_rename (
                page_id UUID PRIMARY KEY,
                view_count COUNTER
            );
                "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.rename_table(table_name, &table_name2).await.unwrap();
        db.rename_table(&table_name2, table_name).await.unwrap();
    }

    pub async fn test_index_inner(db: CassandraDB) {
        let table_name = "test_index";
        let _ = db.drop_index(table_name, "test_index_email_index").await;
        let _ = db.drop_table(table_name, false).await;
        let req = CreateTableReq {
            name: table_name.to_owned(),
            columns: vec![
                CreateTableReqColumn {
                    name: "id".to_string(),
                    data_type: "UUID".into(),
                    primary_key: true,
                    ..Default::default()
                },
                CreateTableReqColumn {
                    name: "name".into(),
                    data_type: "TEXT".into(),
                    ..Default::default()
                },
                CreateTableReqColumn {
                    name: "email".to_string(),
                    data_type: "TEXT".to_string(),
                    ..Default::default()
                },
            ],
            indexes: vec![],
        };

        {
            let sql = db.create_table_sql_string(&req).unwrap();
            println!("create table sql:{}", sql);
        }

        db.create_table(&req).await.unwrap();

        db.create_index(
            table_name,
            &CreateIndexReq {
                name: "test_index_email_index".to_owned(),
                columns: vec![CreateIndexColumn {
                    col: "email".to_string(),
                    asc: None,
                }],
                kind: IndexKind::Index,
            },
        )
        .await
        .unwrap();

        let list = db.show_indexes(table_name).await.unwrap();
        for index in list {
            println!("index:{:?}", index);
        }
    }

    pub async fn test_view_inner(db: CassandraDB) {
        let table_name = "test_view_table";
        let view_name = "test_view";
        let _ = db.drop_materialized_view(view_name).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();
        for sql in r#"INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Alice', 30);
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Bob', 25);
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Charlie', 35);
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Diana', 28);
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Eve', 22);"#
            .lines()
        {
            db.execute_sql(query_id, sql).await.unwrap().unwrap();
        }

        db.execute_sql(
            query_id,
            r#"
                CREATE MATERIALIZED VIEW test_view AS
                SELECT * FROM test_view_table
                WHERE age IS NOT NULL AND id IS NOT NULL
                PRIMARY KEY (age, id);
            "#,
        )
        .await
        .unwrap()
        .unwrap();

        let list = db.list_materialized_view().await.unwrap();
        println!("view list:{:?}", list);
        {
            let resp = db
                .show_view_collect(view_name, new_test_show_parameter())
                .await
                .unwrap();
            println!("resp:{:?}", resp);
        }
    }

    pub async fn test_import_failed(db: CassandraDB) {
        enable_test_log();

        use csv::Writer;
        let table_name = "test_import_err";
        let query_id = "aaaa";

        let _err = db.drop_table(table_name, false).await;
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_import_err (
                id uuid,
                int_value int,
                PRIMARY KEY (id)
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let mut file = tempfile::NamedTempFile::with_suffix(".csv").unwrap();

        let mut wtr = Writer::from_writer(vec![]);
        wtr.write_record(["int_value"]).unwrap();
        wtr.write_record(["eeee"]).unwrap();

        file.write_all(&wtr.into_inner().unwrap()).unwrap();
        let x = std::fs::read_to_string(file.path()).unwrap();
        println!("csv content:{}", x);

        let err = db
            .table_import_data_from_file(
                table_name,
                TableImportPara {
                    path: file.path().to_str().unwrap().to_owned(),
                    mongo_data: None,
                    csv_delimiter: None,
                },
            )
            .await;

        match err {
            Ok(_) => unreachable!(),
            Err(x) => {
                let err_msg = x.to_string();
                println!("import error:{}", err_msg);
                assert!(err_msg.contains("rollback"));
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use crate::enable_test_log;

    use super::test_impls;
    use super::*;

    #[tokio::test]
    async fn test_list_schema() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(None).await;
        test_impls::test_list_schema_inner(db).await;
    }

    #[tokio::test]
    async fn test_aggregate() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_aggregate_inner(db).await;
    }

    #[tokio::test]
    async fn test_check_conn() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(None).await;
        test_impls::test_check_conn_inner(db).await;
    }

    #[ignore]
    #[tokio::test]
    async fn test_a_lot_data() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_a_lot_data_inner(db).await;
    }

    #[tokio::test]
    async fn test_sort() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_sort_inner(db).await;
    }

    #[tokio::test]
    async fn test_show_table_column() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_show_table_column_inner(db).await;
    }

    #[tokio::test]
    async fn test_duplicate() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_duplicate_inner(db).await;
    }

    #[tokio::test]
    async fn test_list_created_types() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_list_created_types_inner(db).await;
    }

    #[tokio::test]
    async fn test_types1() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types1_inner(db).await;
    }

    #[tokio::test]
    async fn test_types1_list() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types1_list_inner(db).await;
    }

    #[tokio::test]
    async fn test_types2() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types2_inner(db).await;
    }

    #[tokio::test]
    async fn test_types2_list() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types2_list_inner(db).await;
    }

    #[tokio::test]
    async fn test_types3() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types3_inner(db).await;
    }

    #[tokio::test]
    async fn test_types_list3() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types_list3_inner(db).await;
    }

    #[tokio::test]
    async fn test_types4() {
        enable_test_log();

        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types4_inner(db).await;
    }

    #[tokio::test]
    async fn test_table_column() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_table_column_inner(db).await;
    }

    #[tokio::test]
    async fn test_counter() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_counter_inner(db).await;
    }

    #[tokio::test]
    async fn test_udf() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_udf_inner(db).await;
    }

    #[tokio::test]
    async fn test_create_type() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_create_type_inner(db).await;
    }

    #[tokio::test]
    async fn test_rename() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_rename_inner(db).await;
    }

    #[tokio::test]
    async fn test_index() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_index_inner(db).await;
    }

    #[tokio::test]
    async fn test_view() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_view_inner(db).await;
    }
    #[tokio::test]
    async fn test_import_failed() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_import_failed(db).await;
    }

    #[test]
    fn test_agg() {
        let sql = r#"CREATE AGGREGATE my_count(text)
                SFUNC count_state_function
                STYPE int
                FINALFUNC count_final_function
                INITCOND 0;"#;
        CassandraAST::new(sql)
            .statements
            .iter()
            .for_each(|x| println!("{:?}", x.statement));
    }

    #[tokio::test]

    async fn test_ssl() {}
}
