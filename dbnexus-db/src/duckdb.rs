use crate::datasource_config::DuckDBConfig;
use crate::db::{
    AutoCompletionQueryId, BackendKind, CatalogTrait, CheckConn, ColumnType, CreateDatabaseExtra,
    CreateSchemaExtra, CreateTableAndIndexSql, DBTrait, ExportKind, ExportQueryResult, Field,
    GetBackendKind, Index, IndexKind, ListTable, MetaValue, POOL_MAX_CONNECTIONS,
    QueryEventEmitter, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas,
    ShowTableParameter, SqlBuilder, SqlExecuteRet, create_html_table_data,
    create_html_table_header, escape_sql_str, export_query_as_xlsx_send_blocking_data,
    not_support_feature,
};
use crate::helper::{create_csv, separate_sql_by_sqlparser_tokenizer};
use chrono::NaiveDate;
use dashmap::DashMap;
use duckdb::types::{TimeUnit, Value};
use duckdb::{AccessMode, Connection, DuckdbConnectionManager};
use r2d2::{Pool, PooledConnection};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::spawn_blocking;

fn gen_show_sql(
    schema: &str, table_name: &str, fields: &str, where_str: Option<&str>, order_by: Option<&str>,
    page: usize, page_size: usize,
) -> String {
    let mut sql = format!("SELECT {fields} from \"{schema}\".\"{table_name}\"");
    if let Some(where_str) = where_str {
        sql.push_str(format!(" where {where_str} ").as_str());
    }
    if let Some(order_by) = order_by {
        sql.push_str(format!(" ORDER BY {order_by} ").as_str());
    }
    let offset = (page - 1) * page_size;
    sql.push_str(format!(" OFFSET {} LIMIT {}", offset, page_size).as_str());
    sql
}

impl DuckDB {
    pub(crate) fn use_database(&self, conn: &Connection) -> anyhow::Result<()> {
        if let Some(db) = self.config.db.as_ref() {
            conn.execute(&format!("use {}", db), [])?;
        }
        if let Some(s) = self.config.schema.as_ref() {
            conn.execute(&format!("SET SCHEMA '{s}';"), [])?;
        }
        anyhow::Ok(())
    }
}

#[derive(Clone)]
pub struct DuckDB {
    pub(crate) config: DuckDBConfig,
    pool: Pool<DuckdbConnectionManager>,
    pub(crate) query_connection: Arc<DashMap<String, DuckDbPooledConn>>,
    pub(crate) completion_id: AutoCompletionQueryId,
}

#[async_trait::async_trait]
impl ServerMetas for DuckDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let sql = "PRAGMA version;";
            let conn = this.get_conn()?;
            let version = conn.query_row(sql, [], |row| {
                let version: String = row.get(0)?;
                Ok(version)
            })?;
            anyhow::Ok(vec![MetaValue::new("version", version)])
        })
        .await?
    }
}

impl TryFrom<&DuckDBConfig> for DuckdbConnectionManager {
    type Error = duckdb::Error;
    fn try_from(value: &DuckDBConfig) -> Result<Self, duckdb::Error> {
        let mut config = duckdb::Config::default();
        config = config.access_mode(if value.read_only {
            AccessMode::ReadOnly
        } else {
            AccessMode::ReadWrite
        })?;
        DuckdbConnectionManager::file_with_flags(value.path.clone(), config)
    }
}

impl DuckDB {
    pub fn new(config: DuckDBConfig) -> anyhow::Result<Self> {
        let mut builder = r2d2::Builder::<DuckdbConnectionManager>::new();
        builder = builder.max_size(POOL_MAX_CONNECTIONS);
        builder = builder.min_idle(Some(5));

        if let Some(timeout) = config.conn_timeout {
            builder = builder.connection_timeout(timeout.into());
        }
        let pool = builder.build(DuckdbConnectionManager::try_from(&config)?)?;
        anyhow::Ok(Self {
            config,
            pool,
            query_connection: Default::default(),
            completion_id: Default::default(),
        })
    }

    pub(crate) fn get_conn(&self) -> anyhow::Result<PooledConnection<DuckdbConnectionManager>> {
        let conn = self.pool.get()?;
        self.use_database(&conn)?;

        anyhow::Ok(conn)
    }
}

impl std::fmt::Debug for DuckDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "duckdb@{}", self.config.path)
    }
}

#[async_trait::async_trait]
impl CheckConn for DuckDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let now = std::time::Instant::now();
            conn.execute("select 1", [])?;
            anyhow::Ok(now.elapsed())
        })
        .await?
    }
}

impl GetBackendKind for DuckDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::DuckDB
    }
}

pub(crate) type DuckDbPooledConn = Arc<Mutex<PooledConnection<DuckdbConnectionManager>>>;

impl DuckDB {
    fn get_conn_by_query_id(&self, query_id: Option<&str>) -> anyhow::Result<DuckDbPooledConn> {
        match query_id {
            Some(id) => match self.query_connection.get(&id.to_string()) {
                Some(x) => anyhow::Ok(x.clone()),
                None => anyhow::bail!("should be able to get connection"),
            },
            None => {
                let conn = self.get_conn()?;
                anyhow::Ok(Arc::new(Mutex::new(conn)))
            }
        }
    }

    async fn export_query_result_as_csv(
        &self, sql: &str, query_id: Option<&str>, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let sql = sql.to_owned();
        let query_id = query_id.map(|s| s.to_owned());
        spawn_blocking(move || {
            let conn = this.get_conn_by_query_id(query_id.as_deref())?;
            let conn = conn.lock().expect("lock failed");
            let mut stmt = conn.prepare(&sql)?;
            let mut stream = stmt.query([])?;
            let mut first = true;
            while let Some(row) = stream.next()? {
                if first {
                    first = false;
                    if include_header {
                        let fs = row.as_ref().column_names().into_iter().map(Some).collect();
                        sender.blocking_send(Cow::Owned(create_csv(&vec![fs], delimiter)))?;
                    }
                }
                let d = encode_row(row)?;
                let r = create_csv(&d, delimiter);
                sender.blocking_send(r.into())?;
            }
            anyhow::Ok(())
        })
        .await??;
        anyhow::Ok(())
    }

    async fn export_query_result_as_xlsx(
        &self, sql: &str, query_id: Option<&str>, include_header: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let sql = sql.to_owned();
        let query_id = query_id.map(|s| s.to_owned());
        spawn_blocking(move || {
            let conn = this.get_conn_by_query_id(query_id.as_deref())?;
            let conn = conn.lock().expect("lock failed");
            let mut stmt = conn.prepare(&sql)?;
            let mut row_num = 0;
            let mut stream = stmt.query([])?;
            let mut first = true;
            while let Some(row) = stream.next()? {
                if first {
                    first = false;
                    if include_header {
                        let names: Vec<_> =
                            row.as_ref().column_names().into_iter().map(Some).collect();
                        export_query_as_xlsx_send_blocking_data(
                            &mut row_num,
                            names,
                            sender.clone(),
                        )?;
                    }
                }
                let d = encode_row(row)?;
                export_query_as_xlsx_send_blocking_data(&mut row_num, d, sender.clone())?;
            }
            anyhow::Ok(())
        })
        .await??;
        anyhow::Ok(())
    }
    async fn export_query_result_as_html(
        &self, sql: &str, query_id: Option<&str>, include_header: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let sql = sql.to_owned();
        let query_id = query_id.map(|s| s.to_owned());
        spawn_blocking(move || {
            let conn = this.get_conn_by_query_id(query_id.as_deref())?;
            let conn = conn.lock().expect("lock failed");
            let mut stmt = conn.prepare(&sql)?;
            let mut stream = stmt.query([])?;
            let mut first = true;
            while let Some(row) = stream.next()? {
                if first {
                    first = false;
                    if include_header {
                        let names = row.as_ref().column_names();
                        sender.blocking_send(Cow::Owned(create_html_table_header(&names)))?;
                    }
                }
                let d = encode_row(row)?;
                let r = create_html_table_data(&[d]);
                sender.blocking_send(r.into())?;
            }
            anyhow::Ok(())
        })
        .await??;
        anyhow::Ok(())
    }

    async fn export_query_result_as_json(
        &self, sql: &str, query_id: Option<&str>, pretty: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let sql = sql.to_owned();
        sender.send("[".into()).await?;
        let query_id = query_id.map(|s| s.to_owned());
        spawn_blocking(move || {
            let conn = this.get_conn_by_query_id(query_id.as_deref())?;
            let conn = conn.lock().expect("lock failed");
            let mut stmt = conn.prepare(&sql)?;
            let mut first = true;
            let mut stream = stmt.query([])?;
            while let Some(row) = stream.next()? {
                let value = encode_row_as_json(row)?;
                if !first {
                    sender.blocking_send(",".into())?;
                }
                if pretty {
                    sender.blocking_send("\n".into())?;
                }
                sender.blocking_send(Cow::Owned(if pretty {
                    serde_json::to_string_pretty(&value)?
                } else {
                    serde_json::to_string(&value)?
                }))?;
                first = false;
            }

            if pretty {
                sender.blocking_send("\n".into())?;
            }
            sender.blocking_send("]".into())?;
            anyhow::Ok(())
        })
        .await??;

        anyhow::Ok(())
    }

    async fn export_query_result_as_json_line(
        &self, sql: &str, query_id: Option<&str>,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let sql = sql.to_owned();
        let query_id = query_id.map(|s| s.to_owned());
        spawn_blocking(move || {
            let conn = this.get_conn_by_query_id(query_id.as_deref())?;
            let conn = conn.lock().expect("lock failed");
            let mut stmt = conn.prepare(&sql)?;
            let mut stream = stmt.query([])?;
            while let Some(row) = stream.next()? {
                let serde_value = encode_row_as_json(row)?;
                let str = serde_json::to_string(&serde_value)?;
                sender.blocking_send(Cow::Owned(str))?;
            }
            anyhow::Ok(())
        })
        .await??;
        anyhow::Ok(())
    }
}

fn encode_row_as_json<'stmt>(
    row: &duckdb::Row<'stmt>,
) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    let mut ret = serde_json::Map::new();
    for (index, name) in row.as_ref().column_names().into_iter().enumerate() {
        let v: Value = row.get(index)?;
        ret.insert(name, value_2_json(&v)?);
    }
    return anyhow::Ok(ret);
    fn value_2_json(value: &Value) -> anyhow::Result<serde_json::Value> {
        let v = match value {
            Value::Null => serde_json::Value::Null,
            Value::Boolean(x) => serde_json::Value::from(*x),
            Value::TinyInt(x) => serde_json::Value::from(*x),
            Value::SmallInt(x) => serde_json::Value::from(*x),
            Value::Int(x) => serde_json::Value::from(*x),
            Value::BigInt(x) => serde_json::Value::from(*x),
            Value::HugeInt(x) => serde_json::Value::from(x.to_string()),
            Value::UTinyInt(x) => serde_json::Value::from(*x),
            Value::USmallInt(x) => serde_json::Value::from(*x),
            Value::UInt(x) => serde_json::Value::from(*x),
            Value::UBigInt(x) => serde_json::Value::from(*x),
            Value::Float(x) => serde_json::Value::from(*x),
            Value::Double(x) => serde_json::Value::from(*x),
            Value::Decimal(decimal) => serde_json::Value::from(decimal.to_string()),
            Value::Timestamp(time_unit, value) => {
                let v = format_timestamp(*time_unit, *value, false)?;
                serde_json::Value::from(v)
            }
            Value::Text(x) => serde_json::Value::from(x.clone()),
            Value::Blob(items) => {
                let items = items
                    .iter()
                    .map(|x| serde_json::Value::from(*x))
                    .collect::<Vec<_>>();
                serde_json::Value::Array(items)
            }
            Value::Date32(days) => {
                let d = format_date(*days)?;
                serde_json::Value::from(d)
            }
            Value::Time64(time_unit, value) => {
                let v = format_timestamp(*time_unit, *value, true)?;
                serde_json::Value::from(v)
            }
            Value::Interval {
                months,
                days,
                nanos,
            } => {
                let v = format_interval(*months, *days, *nanos);
                serde_json::Value::from(v)
            }
            Value::List(values) => {
                let mut ret = Vec::with_capacity(values.len());
                for value in values.iter() {
                    ret.push(value_2_json(value)?);
                }
                serde_json::Value::Array(ret)
            }
            Value::Enum(x) => serde_json::Value::from(x.clone()),
            Value::Struct(ordered_map) => {
                let mut m = serde_json::Map::new();
                for (k, v) in ordered_map.iter() {
                    m.insert(k.clone(), value_2_json(v)?);
                }
                serde_json::Value::Object(m)
            }
            Value::Array(values) => {
                let mut ret = Vec::with_capacity(values.len());
                for value in values.iter() {
                    ret.push(value_2_json(value)?);
                }
                serde_json::Value::Array(ret)
            }
            Value::Map(ordered_map) => {
                let mut m = serde_json::Map::new();
                for (k, v) in ordered_map.iter() {
                    m.insert(serde_json::to_string(&value_2_json(k)?)?, value_2_json(v)?);
                }
                serde_json::Value::Object(m)
            }
            Value::Union(value) => value_2_json(&value)?,
        };
        anyhow::Ok(v)
    }
}

impl SqlBuilder for DuckDB {
    fn put_value_in_sql(
        &self, typ: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        put_value_in_sql(typ, value)
    }
}

#[async_trait::async_trait]

impl ExportQueryResult for DuckDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }

    async fn export_query_result(
        &self, query_id: &str, sql: &str, kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::Csv {
                include_header,
                delimiter,
            } => {
                self.export_query_result_as_csv(
                    sql,
                    Some(query_id),
                    include_header,
                    delimiter,
                    sender,
                )
                .await
            }
            ExportKind::Json { pretty } => {
                self.export_query_result_as_json(sql, Some(query_id), pretty, sender)
                    .await
            }
            ExportKind::JsonLine => {
                self.export_query_result_as_json_line(sql, Some(query_id), sender)
                    .await
            }
            ExportKind::Sql {
                include_table_schema: _,
            } => ExportKind::cannot_query_result_export_as_sql(),
            ExportKind::Xlsx { include_header } => {
                self.export_query_result_as_xlsx(sql, Some(query_id), include_header, sender)
                    .await
            }
            ExportKind::Html { include_header } => {
                self.export_query_result_as_html(sql, Some(query_id), include_header, sender)
                    .await
            }
        }
    }
}

#[async_trait::async_trait]
impl QuerySql for DuckDB {
    async fn set_auto_commit(&self, _query_id: &str, _commit: bool) -> anyhow::Result<()> {
        anyhow::bail!(
            "async rust not allow refcell and self reference,so this can't be implemented,we can't prove this is safe to rust compiler."
        )
    }

    async fn query_sql(
        &self, query_id: &str, sql: &str, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let sql = separate_sql_by_sqlparser_tokenizer(
            sql,
            crate::language_server::LangServerCommon::dialect(self).as_ref(),
        )?;
        let this = Clone::clone(self);
        let query_id = query_id.to_string();
        spawn_blocking(move || {
            let conn = match this.query_connection.get(&query_id) {
                Some(conn) => conn.clone(),
                None => anyhow::bail!("connection closed???"),
            };
            let conn = conn.lock().expect("lock failed");
            for sql in sql.iter() {
                let mut stmt = conn.prepare(&sql)?;
                let mut stream = stmt.query([])?;
                let mut first = true;
                while let Some(row) = stream.next()? {
                    if first {
                        first = false;
                        let stmt = row.as_ref();
                        emit.send(
                            QueryResp {
                                fields: (0..stmt.column_count())
                                    .map(|index| {
                                        Field::from_name_type(
                                            stmt.column_name(index).unwrap().to_string(),
                                            format!("{:?}", stmt.column_type(index)),
                                        )
                                    })
                                    .collect(),
                                sql: Some(sql.clone()),
                                ..Default::default()
                            }
                            .into(),
                        )?;
                    }
                    let d = encode_row(row)?;
                    emit.send(d.into())?;
                }
            }
            anyhow::Ok(())
        })
        .await?
    }

    async fn execute_sql(
        &self, query_id: &str, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let this = Clone::clone(self);
        let query_id = query_id.to_string();
        let sql = sql.to_string();
        spawn_blocking(move || {
            let conn = match this.query_connection.get(&query_id) {
                Some(x) => x.clone(),
                None => anyhow::bail!("connection closed???"),
            };
            let mut conn = conn.lock().expect("lock failed");
            let conn = conn.deref_mut();
            let resp = match conn.execute_batch(&sql) {
                Ok(_) => Ok(QueryResp::execute_ok(sql.to_string())),
                Err(err) => Err(err.to_string()),
            };
            anyhow::Ok(resp)
        })
        .await?
    }

    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let id = id.to_string();
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            this.query_connection.insert(id, Arc::new(Mutex::new(conn)));
            anyhow::Ok(())
        })
        .await?
    }

    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        self.query_connection.remove(&id.to_string());
        anyhow::Ok(())
    }

    async fn execute_sql_no_open_conn(
        &self, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let this = Clone::clone(self);
        let sql = sql.to_string();
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let resp = match conn.execute_batch(&sql) {
                Ok(_) => Ok(QueryResp::execute_ok(sql.to_string())),
                Err(err) => Err(err.to_string()),
            };
            anyhow::Ok(resp)
        })
        .await?
    }
}

impl ReadOnlyTrait for DuckDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

impl DuckDB {
    fn query_count(&self, name: &str, where_str: Option<&str>) -> anyhow::Result<usize> {
        let conn = self.get_conn()?;
        let schema = self.config.schema_str();
        let mut sql = format!(r#"SELECT count(1) from "{schema}"."{name}" "#);
        if let Some(where_str) = where_str {
            sql.push_str(format!(" where {where_str}").as_str());
        }
        let r = conn.query_row(&sql, [], |row| {
            let r: i64 = row.get(0)?;
            Ok(r)
        })?;
        anyhow::Ok(r as usize)
    }
}

fn format_timestamp(time_uint: TimeUnit, value: i64, is_time: bool) -> anyhow::Result<String> {
    use chrono::DateTime;
    let d = match time_uint {
        TimeUnit::Second => DateTime::from_timestamp(value, 0),
        TimeUnit::Millisecond => DateTime::from_timestamp_millis(value),
        TimeUnit::Microsecond => DateTime::from_timestamp_micros(value),
        TimeUnit::Nanosecond => Some(DateTime::from_timestamp_nanos(value)),
    };
    let d = match d {
        Some(x) => x,
        None => anyhow::bail!("empty datetime"),
    };
    return anyhow::Ok(if is_time {
        d.time().to_string()
    } else {
        d.naive_utc().to_string()
    });
}

fn encode_value(value: &Value, nested: bool) -> anyhow::Result<Option<String>> {
    let r = match value {
        Value::Null => None,
        Value::Boolean(x) => Some(x.to_string()),
        Value::TinyInt(x) => Some(x.to_string()),
        Value::SmallInt(x) => Some(x.to_string()),
        Value::Int(x) => Some(x.to_string()),
        Value::BigInt(x) => Some(x.to_string()),
        Value::HugeInt(x) => Some(x.to_string()),
        Value::UTinyInt(x) => Some(x.to_string()),
        Value::USmallInt(x) => Some(x.to_string()),
        Value::UInt(x) => Some(x.to_string()),
        Value::UBigInt(x) => Some(x.to_string()),
        Value::Float(x) => Some(x.to_string()),
        Value::Double(x) => Some(x.to_string()),
        Value::Decimal(decimal) => Some(decimal.to_string()),
        Value::Timestamp(time_unit, value) => {
            let v = format_timestamp(*time_unit, *value, false)?;
            Some(v)
        }
        Value::Text(x) => {
            let mut x = Some(x.to_string());
            if nested {
                x = x.map(|x| format!("'{}'", escape_sql_str(&x)))
            }
            x
        }
        Value::Blob(items) => {
            let x = items
                .iter()
                .map(|x| format!("\\x{}", hex::encode(&[*x])))
                .collect::<Vec<_>>()
                .join("");
            Some(format!("'{}'", x))
        }
        Value::Date32(x) => {
            let mut x = format_date(*x)?;
            if nested {
                x = format!("'{}'", x)
            }
            Some(x)
        }
        Value::Time64(time_unit, value) => {
            let mut v = format_timestamp(*time_unit, *value, true)?;
            if nested {
                v = format!("'{}'", v)
            }
            Some(v)
        }
        Value::Interval {
            months,
            days,
            nanos,
        } => {
            let x = format_interval(*months, *days, *nanos);
            Some(x)
        }
        Value::List(values) | Value::Array(values) => {
            let mut eles = Vec::with_capacity(values.len());
            for value in values.iter() {
                let v = encode_value(value, true)?;
                eles.push(v);
            }
            let eles = eles
                .into_iter()
                .map(|x| x.unwrap_or("null".to_string()))
                .collect::<Vec<_>>();
            Some(format!("[{}]", eles.join(",")))
        }
        Value::Enum(x) => Some(format!("'{}'", x)),
        Value::Struct(ordered_map) => {
            let mut values = Vec::new();
            for (k, v) in ordered_map.iter() {
                values.push(format!(
                    "{}:{}",
                    k,
                    encode_value(v, true)?.unwrap_or("null".to_string())
                ));
            }
            Some(format!("{{{}}}", values.join(",")))
        }
        Value::Map(ordered_map) => {
            let mut keys = Vec::new();
            let mut values = Vec::new();
            for (k, v) in ordered_map.iter() {
                keys.push(encode_value(k, true)?);
                values.push(encode_value(v, true)?);
            }
            let keys = keys
                .into_iter()
                .map(|x| x.unwrap_or("null".to_string()))
                .collect::<Vec<_>>();
            let values = values
                .into_iter()
                .map(|x| x.unwrap_or("null".to_string()))
                .collect::<Vec<_>>();
            Some(format!(
                "map([{}] , [{}])",
                keys.join(","),
                values.join(",")
            ))
        }
        Value::Union(value) => encode_value(&value, true)?,
    };
    anyhow::Ok(r)
}

fn format_interval(months: i32, days: i32, nanos: i64) -> String {
    let mut parts = Vec::new();
    if months != 0 {
        parts.push(format!("{} months", months));
    }
    if days != 0 {
        parts.push(format!("{} days", days));
    }
    if nanos != 0 {
        let seconds = nanos / 1_000_000_000;
        let remaining_nanos = nanos % 1_000_000_000;
        let mut nanos_part = String::new();
        if seconds != 0 {
            nanos_part.push_str(&format!("{} seconds", seconds));
        }
        if remaining_nanos != 0 {
            nanos_part.push_str(&format!("{} nanoseconds", remaining_nanos));
        }
        parts.push(nanos_part);
    }
    // Join all parts and trim any excess whitespace
    let interval_string = parts.join(" ").trim().to_string();
    format!("INTERVAL '{}'", interval_string)
}

fn encode_row<'stmt>(row: &duckdb::Row<'stmt>) -> anyhow::Result<Vec<Option<String>>> {
    let mut values = Vec::with_capacity(row.as_ref().column_count());
    for index in 0..row.as_ref().column_count() {
        let x: Value = row.get(index)?;
        values.push(encode_value(&x, false)?);
    }
    anyhow::Ok(values)
}

impl DuckDB {
    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"SELECT * from "{}"."{}""#,
            self.config.schema_str(),
            table_name
        );
        log::info!("sql:{}", sql);

        self.export_query_result_as_csv(sql.as_str(), None, include_header, delimiter, sender)
            .await
    }

    async fn export_table_as_json(
        &self, table_name: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"SELECT * from "{}"."{}""#,
            self.config.schema_str(),
            table_name
        );
        log::info!("sql:{}", sql);
        self.export_query_result_as_json(&sql, None, pretty, sender)
            .await
    }

    async fn export_table_as_html(
        &self, table_name: &str, include_header: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"SELECT * from "{}"."{}""#,
            self.config.schema_str(),
            table_name
        );
        log::info!("sql:{}", sql);
        self.export_query_result_as_html(&sql, None, include_header, sender)
            .await
    }

    async fn export_table_as_xlsx(
        &self, table_name: &str, include_header: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"SELECT * from "{}"."{}""#,
            self.config.schema_str(),
            table_name
        );
        log::info!("sql:{}", sql);
        self.export_query_result_as_xlsx(&sql, None, include_header, sender)
            .await
    }

    async fn export_table_as_json_line(
        &self, table_name: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"SELECT * from "{}"."{}""#,
            self.config.schema_str(),
            table_name
        );
        log::info!("sql:{}", sql);
        self.export_query_result_as_json_line(&sql, None, sender)
            .await
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
        let this = Clone::clone(self);
        let table_name = table_name.to_owned();
        let fs = self.show_table_columns(&table_name).await?;
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let sql = format!(
                r#"SELECT {} from "{}"."{}""#,
                this.backend_name().format_fields(Some(&fs)),
                this.config.schema_str(),
                table_name
            );
            let mut stmt = conn.prepare(&sql)?;
            let mut stream = stmt.query([])?;
            while let Some(row) = stream.next()? {
                let data = encode_row(row)?;
                let mut values = HashMap::new();
                for (f, value) in fs.iter().zip(data.into_iter()) {
                    values.insert(f.field.clone(), value);
                }
                let sql =
                    this.insert_sql(this.config.schema.as_deref(), &table_name, &fs, &values)?;
                sender.blocking_send(Cow::Owned(sql))?;
            }
            anyhow::Ok(())
        })
        .await??;
        anyhow::Ok(())
    }
}

impl DuckDB {
    pub(crate) async fn list_table_impl(
        &self, db: &str, schema: &str,
    ) -> anyhow::Result<Vec<ListTable>> {
        let this = Clone::clone(self);
        let sql = format!(
            r#"SELECT  table_name
            FROM information_schema.tables
            WHERE table_schema = '{schema}' AND table_type = 'BASE TABLE'  and table_catalog='{db}';"#,
        );
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut ret = Vec::new();
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                ret.push(name);
            }
            anyhow::Ok(ret.into_iter().map(|x| ListTable::from(x)).collect())
        })
        .await?
    }

    pub(crate) async fn list_view_impl(
        &self, db: &str, schema: &str,
    ) -> anyhow::Result<Vec<ListTable>> {
        let this = Clone::clone(self);
        let sql = format!(
            r#"SELECT table_name 
                FROM information_schema.views 
                WHERE table_schema = '{schema}' and table_catalog = '{db}'"#,
        );
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut ret = Vec::new();
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                ret.push(name);
            }
            anyhow::Ok(
                ret.into_iter()
                    .map(|x| ListTable::from(x))
                    .collect::<Vec<_>>(),
            )
        })
        .await?
    }
}

impl DuckDB {
    async fn execute(&self, sql: impl Into<String>) -> anyhow::Result<usize> {
        let sql = sql.into();
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let aff = conn.execute(&sql, [])?;
            anyhow::Ok(aff)
        })
        .await?
    }
}

#[async_trait::async_trait]
impl SchemaTrait for DuckDB {
    async fn rename_index(
        &self, _table: &str, _old_name: &str, _new_name: &str,
    ) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP INDEX "{index}";"#);
        self.execute(sql).await.map(|_| ())
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let this = Clone::clone(self);
        let table_name = table_name.to_string();
        spawn_blocking(move || {
            let sql = format!(
                r#"
            select index_name,
            is_unique,sql from duckdb_indexes() 
            where database_name='{}' and schema_name='{}' and  table_name =  '{}'"#,
                this.config.db_str(),
                this.config.schema_str(),
                table_name
            );
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut ret = Vec::new();
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                let is_unique: bool = row.get(1)?;
                let sql: String = row.get(2)?;
                let columns = extract_index_columns(&sql)?;
                ret.push(Index {
                    name: Some(name),
                    kind: if is_unique {
                        IndexKind::Unique
                    } else {
                        IndexKind::Index
                    },
                    columns,
                });
            }
            anyhow::Ok(ret)
        })
        .await?
    }

    async fn create_index(
        &self, table: &str, name: &str, kind: IndexKind, fields: &[String],
    ) -> anyhow::Result<()> {
        let sql = format!(
            "CREATE {} INDEX {} ON {table}({})",
            match kind {
                IndexKind::Index => "",
                IndexKind::Primary => anyhow::bail!("can't create primary index"),
                IndexKind::Unique => "Unique",
            },
            name,
            fields.join(",")
        );
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
             ALTER TABLE "{table}" DROP COLUMN "{col}";
        "#
        );
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        if old_name != new_name {
            let sql = format!(
                r#"
            ALTER TABLE "{table}" RENAME COLUMN "{old_name}" TO "{new_name}";
            "#
            );
            log::info!("sql:{}", sql);
            self.execute(sql).await?;
        }
        if let Some(typ) = typ.as_ref() {
            let sql = format!(
                r#"
            ALTER TABLE {table} 
                    ALTER COLUMN "{new_name}" SET DATA TYPE {};
            "#,
                typ.typ
            );
            log::info!("sql:{}", sql);

            self.execute(sql).await?;

            if let Some(null) = typ.nullness.as_ref() {
                let sql = if null.trim().to_lowercase() == "null" {
                    format!(
                        r#"
                        ALTER TABLE {table} 
                            ALTER COLUMN {new_name} DROP NOT NULL;
                    "#
                    )
                } else {
                    format!(
                        r#"
                        ALTER TABLE {table} 
                            ALTER COLUMN {new_name} SET NOT NULL;
                    "#
                    )
                };
                log::info!("sql:{}", sql);
                self.execute(sql).await?;
            }

            if let Some(x) = typ.default_value.as_ref() {
                let sql = format!(
                    r#"
                    ALTER TABLE "{table}" ALTER COLUMN "{new_name}" SET DEFAULT {};
                    "#,
                    self.put_value_in_sql(&typ.typ, None, Some(x.as_str()))?
                );

                self.execute(sql).await?;
            }
        }
        anyhow::Ok(())
    }

    async fn add_table_column(
        &self, table: &str, name: &str, col: ColumnType,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"ALTER TABLE "{table}" ADD COLUMN "{name}" {} {} {}"#,
            col.typ,
            if let Some(nullness) = col.nullness.as_ref() {
                nullness.as_str()
            } else {
                ""
            },
            if let Some(d) = col.default_value.as_ref() {
                format!(
                    "DEFAULT {}",
                    self.put_value_in_sql(&col.typ, None, Some(d.as_str()))?
                )
            } else {
                "".to_owned()
            }
        );

        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn export_table_or_view(
        &self, name: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::Csv {
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
            ExportKind::Xlsx { include_header } => {
                self.export_table_as_xlsx(name, include_header, sender)
                    .await
            }
            ExportKind::Html { include_header } => {
                self.export_table_as_html(name, include_header, sender)
                    .await
            }
        }
    }

    async fn create_table_and_index_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let sql = format!(
            r#"
            SELECT sql
            FROM sqlite_master
            WHERE type = 'table' AND name = '{table_name}';
        "#
        );
        let this = Clone::clone(self);
        let table_name = table_name.to_string();
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let mut ret = conn.query_row(&sql, [], |x| {
                let sql: String = x.get(0)?;
                Ok(CreateTableAndIndexSql {
                    create_table: sql,
                    indexes: vec![],
                })
            })?;
            ret.indexes = {
                let sql = format!(
                    r#"
                SELECT sql
                FROM sqlite_master
                WHERE type = 'index' AND tbl_name = '{table_name}';
            "#
                );

                let mut stmt = conn.prepare(&sql)?;
                let mut indexes = vec![];
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    let s: String = row.get(0)?;
                    indexes.push(s);
                }
                indexes
            };
            anyhow::Ok(ret)
        })
        .await?
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_view_impl(self.config.db_str(), self.config.schema_str())
            .await
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP VIEW "{view_name}";"#);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn show_view(
        &self, view_name: &str, para: ShowTableParameter, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let cols = self.show_view_columns(view_name).await?;
        let view_name = view_name.to_owned();
        spawn_blocking(move || {
            let ShowTableParameter {
                where_str,
                order_by,
                page,
                page_size,
                ..
            } = &para;
            let total = this.query_count(&view_name, where_str.as_deref())?;

            let sql = gen_show_sql(
                this.config.schema_str(),
                &view_name,
                &this.backend_name().format_fields(Some(&cols)),
                where_str.as_deref(),
                order_by.as_deref(),
                *page,
                *page_size,
            );
            log::info!("sql:{}", sql);
            emit.send(
                QueryResp {
                    fields: cols.clone(),
                    total,
                    sql: Some(sql.clone()),
                    ..Default::default()
                }
                .into(),
            )?;
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let data = encode_row(&row)?;
                emit.send(data.into())?;
            }

            anyhow::Ok(())
        })
        .await?
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let this = Clone::clone(self);
        let table_name = table_name.to_owned();
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let primary_keys = {
                let sql = format!(
                    "
                SELECT c.column_name
                    FROM information_schema.table_constraints tc
                    JOIN information_schema.constraint_column_usage c
                    ON tc.constraint_name = c.constraint_name
                    WHERE tc.table_name = '{}'
                    AND tc.table_schema = '{}'  -- 指定 schema 名称
                    and tc.table_catalog = '{}'
                    AND tc.constraint_type = 'PRIMARY KEY';
                    ",
                    table_name,
                    this.config.db_str(),
                    this.config.schema_str()
                );

                log::info!("sql:{}", sql);
                let mut primary_keys = HashSet::new();
                let mut stmt = conn.prepare(&sql)?;
                let mut rows = stmt.query([])?;
                while let Some(row) = rows.next()? {
                    let name: String = row.get(0)?;
                    primary_keys.insert(name);
                }
                primary_keys
            };

            let sql = format!(
                r#"
            SELECT column_name, data_type, is_nullable,column_default
                FROM information_schema.columns
                WHERE table_name = '{}'
                AND table_schema = '{}' 
                AND table_catalog = '{}';  -- 指定 schema 名称
        "#,
                table_name,
                this.config.schema_str(),
                this.config.db_str(),
            );
            log::info!("sql:{}", sql);
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut ret = Vec::new();
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                let data_type: String = row.get(1)?;
                let is_nullable: String = row.get(2)?;
                let column_default: Option<String> = row.get(3)?;
                let is_primary = primary_keys.contains(&name);
                ret.push(Field {
                    field: name,
                    data_type,
                    nullable: is_nullable.eq_ignore_ascii_case("yes"),
                    default_value: column_default,
                    is_primary,
                    ..Default::default()
                });
            }
            anyhow::Ok(ret)
        })
        .await?
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let sql = format!(r#"ALTER TABLE "{old}"  RENAME TO "{new}";"#);
        self.execute(sql).await?;

        anyhow::Ok(())
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        let sql = format!(r#"CREATE TABLE "{dup}" AS SELECT * FROM "{table}";"#);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.config.db_str(), self.config.schema_str())
            .await
    }

    async fn show_view_columns(&self, name: &str) -> anyhow::Result<Vec<Field>> {
        let sql = format!(
            "
        SELECT column_name, data_type
        FROM information_schema.columns
        WHERE table_name = '{name}'  and table_catalog = '{}' and table_schema='{}' ;
",
            self.config.db_str(),
            self.config.schema_str()
        );
        let name = name.to_owned();
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            let mut fs = Vec::new();
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                let typ: String = row.get(1)?;
                fs.push(Field::from_name_type(name, typ));
            }
            if fs.len() > 0 {
                return anyhow::Ok(fs);
            }
            let sql = format!(
                r#"select * from "{}"."{}" limit 1"#,
                this.config.schema_str(),
                name
            );
            let mut stmt = conn.prepare(&sql)?;
            let mut stream = stmt.query([])?;
            let mut ret = vec![];
            while let Some(row) = stream.next()? {
                let name: String = row.get(0)?;
                let typ: Option<String> = row.get(1)?;
                ret.push(Field::from_name_type(name, typ.unwrap_or_default()));
            }
            anyhow::Ok(ret)
        })
        .await?
    }

    async fn show_table(
        &self, table_name: &str, para: ShowTableParameter, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let cols = self.show_table_columns(table_name).await?;
        let this = Clone::clone(self);
        let table_name = table_name.to_string();
        spawn_blocking(move || {
            let ShowTableParameter {
                where_str,
                order_by,
                page,
                page_size,
                ..
            } = &para;
            let total = this.query_count(&table_name, where_str.as_deref())?;
            let sql = gen_show_sql(
                this.config.schema_str(),
                &table_name,
                &this.backend_name().format_fields(Some(&cols)),
                where_str.as_deref(),
                order_by.as_deref(),
                *page,
                *page_size,
            );
            log::info!("sql:{}", sql);
            emit.send(
                QueryResp {
                    fields: cols.clone(),
                    sql: Some(sql.to_string()),
                    total,
                    ..Default::default()
                }
                .into(),
            )?;
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            let mut stream = stmt.query([])?;
            while let Some(row) = stream.next()? {
                let d = encode_row(row)?;
                emit.send(d.into())?;
            }
            anyhow::Ok(())
        })
        .await?
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = self.insert_sql(self.config.schema.as_deref(), table_name, &fs, &values)?;
        log::info!("sql:{}", sql);
        let changes = self.execute(&sql).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(changes as u64),
        })
    }

    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;

        let sql = self.update_sql(
            self.config.schema.as_deref(),
            table_name,
            &fs,
            &values,
            &wheres,
        )?;

        let changes = self.execute(&sql).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(changes as u64),
        })
    }

    async fn table_delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = self.delete_sql(self.config.schema.as_deref(), table_name, &fs, &wheres)?;
        let changes = self.execute(&sql).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(changes as u64),
        })
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DELETE FROM "{}";"#, table_name);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        let sql = format!(r#"DROP TABLE "{}";"#, table);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        crate::language_server::LexerCompletionTrait::get_types(self).await
    }
}

fn put_value_in_sql(typ: &str, value: Option<&str>) -> anyhow::Result<String> {
    let typ = typ.to_lowercase();
    log::trace!("encode type:{} value:{:?}", typ, value);

    match typ.as_str() {
        "date" | "json" | "time" | "timestamp with time zone" | "timestamp" | "varchar" => {
            anyhow::Ok(
                value
                    .map(|x| format!("'{}'", x))
                    .unwrap_or("null".to_string()),
            )
        }
        _ if typ.ends_with("[]") => anyhow::Ok(
            value
                .map(|x| format!("{}", x))
                .unwrap_or("null".to_string()),
        ),
        _ if typ.starts_with("time")
            || typ.starts_with("date")
            || typ.starts_with("utf8")
            || typ.starts_with("largeutf8") =>
        {
            anyhow::Ok(
                value
                    .map(|x| format!("'{}'", x))
                    .unwrap_or("null".to_string()),
            )
        }
        _ => anyhow::Ok(value.map(|x| x.to_string()).unwrap_or("null".to_string())),
    }
}

impl DuckDB {
    pub(crate) async fn list_schema_impl(&self, db: &str) -> anyhow::Result<Vec<String>> {
        let sql = format!(
            "SELECT schema_name FROM information_schema.schemata  where catalog_name = '{db}' ;",
        );
        log::info!("sql:{}", sql);

        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let mut p = conn.prepare(&sql)?;
            let mut rows = p.query([])?;
            let mut ret = Vec::new();
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                ret.push(name);
            }
            anyhow::Ok(ret)
        })
        .await?
    }
}

#[async_trait::async_trait]

impl DBTrait for DuckDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        self.list_schema_impl(self.config.db_str()).await
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut x = Clone::clone(self);
        x.config.schema = Some(name.to_owned());
        anyhow::Ok(Arc::new(x))
    }

    async fn create_schema(
        &self, name: &str, _extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("CREATE SCHEMA {};", name);
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("DROP SCHEMA {} CASCADE;", name);
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl CatalogTrait for DuckDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let sql = "SELECT database_name FROM duckdb_databases();";
            log::info!("sql:{}", sql);
            let conn = this.get_conn()?;
            let mut p = conn.prepare(&sql)?;
            let mut rows = p.query([])?;
            let mut ret = Vec::new();
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                if name.eq_ignore_ascii_case("system") || name.eq_ignore_ascii_case("temp") {
                    continue;
                }
                ret.push(name);
            }
            anyhow::Ok(ret)
        })
        .await?
    }

    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        let mut x = Clone::clone(self);
        x.config.db = Some(name.to_owned());
        anyhow::Ok(Arc::new(x))
    }

    async fn create_database(
        &self, _name: &str, _extra: Option<CreateDatabaseExtra>,
    ) -> anyhow::Result<()> {
        anyhow::bail!("duckdb can't create database")
    }

    async fn drop_database(&self, _name: &str) -> anyhow::Result<()> {
        anyhow::bail!("duckdb can't drop database")
    }
}

fn format_date(days_since_epoch: i32) -> anyhow::Result<String> {
    let date = NaiveDate::from_ymd_opt(1970, 1, 1)
        .unwrap()
        .checked_add_days(chrono::Days::new(days_since_epoch as i64 as u64));
    match date {
        Some(x) => anyhow::Ok(x.to_string()),
        None => anyhow::bail!("days is none"),
    }
}
#[cfg(test)]

pub(crate) fn duckdb_test_file_path() -> String {
    std::env::var("DBPAL_DUCKDB_TEST_FILE").unwrap()
}

#[cfg(test)]
pub(crate) fn make_test_object(schema: Option<String>) -> DuckDB {
    let path = duckdb_test_file_path();
    DuckDB::new(DuckDBConfig {
        conn_timeout: None,
        path,
        read_only: false,
        db: Some("test".to_string()),
        schema,
    })
    .unwrap()
}

fn extract_index_columns(sql: &str) -> anyhow::Result<Vec<String>> {
    use sqlparser::ast::{Expr, Statement};
    let d = sqlparser::dialect::DuckDbDialect {};
    let mut p = sqlparser::parser::Parser::new(&d);
    p = p.try_with_sql(sql)?;
    let s = p.parse_statement()?;
    match &s {
        Statement::CreateIndex(create_index) => {
            let mut ret = Vec::new();
            for col in create_index.columns.iter() {
                match &col.column.expr {
                    Expr::Identifier(x) => {
                        ret.push(x.value.clone());
                    }
                    _ => {
                        anyhow::bail!("should be a identifier")
                    }
                }
            }
            return anyhow::Ok(ret);
        }
        _ => {
            anyhow::bail!("should be a create index statement but {:?}", s)
        }
    }
}

#[cfg(test)]
pub(crate) fn test_lock() -> &'static tokio::sync::Mutex<()> {
    use tokio::sync::Mutex;
    static MUTEX: Mutex<()> = Mutex::const_new(());
    &MUTEX
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::ShowTableParameter, enable_test_log};

    #[test]
    fn test_extract_index_columns() {
        let s =
            extract_index_columns(r#"CREATE UNIQUE INDEX test_index_name ON test_index("name")"#)
                .unwrap();
        assert_eq!(s, vec!["name".to_string()])
    }

    #[tokio::test]
    async fn test_view() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("test_table_view", false).await;
        let _ = db
            .execute_sql(
                query_id,
                r#"
            CREATE TABLE IF NOT EXISTS test_table_view (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                age INTEGER,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
            
        "#,
            )
            .await
            .unwrap();

        let _ = db
            .execute_sql(
                query_id,
                r#"
            INSERT INTO test_table_view (id, name, age)
            VALUES 
            (1, 'Alice', 30),
            (2, 'Bob', 25),
            (3, 'Charlie', 22),
            (4, 'Diana', 28),
            (5, 'Eve', 21);
        "#,
            )
            .await
            .unwrap();

        let _ = db
            .execute_sql(
                query_id,
                r#"
            CREATE VIEW test_view AS
            SELECT name, age
            FROM test_table_view;

        "#,
            )
            .await
            .unwrap();
        let view_name = "test_view";

        {
            let views = db.list_view().await.unwrap();
            assert!(views.iter().any(|x| x.name.eq_ignore_ascii_case(view_name)));
        }
        {
            let cols = db.show_view_columns(view_name).await.unwrap();
            for col in cols.iter() {
                println!("col:{:?}", col);
            }
        }
        let resp = db
            .show_view_collect(view_name, ShowTableParameter::new_stupid_test("age asc"))
            .await
            .unwrap();
        println!("{:?}", resp);
    }

    #[tokio::test]
    async fn test_database() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object(None);
        let x = db.list_database().await.unwrap();
        println!("database:{:?}", x);
    }

    #[tokio::test]
    async fn test_server_metas() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(None);
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }

    #[tokio::test]
    async fn test_schema() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(None);
        let schema_name = "abc";
        let _err = db.drop_schema(schema_name).await;
        if let Err(err) = _err {
            log::error!("drop schema failed,err:{}", err);
        }
        db.create_schema(schema_name, None).await.unwrap();
        db.open_schema(schema_name).await.unwrap();

        let x = db.list_schema().await.unwrap();
        println!("schemas:{:?}", x);
    }

    #[tokio::test]
    async fn test_list_table() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("main".to_string()));
        let tables = db.list_table().await.unwrap();
        for t in tables.iter() {
            println!("table:{:?}", t.name);
        }

        let db = make_test_object(Some("main".to_string()));
        let views = db.list_view().await.unwrap();
        for t in views.iter() {
            println!("table:{:?}", t.name);
        }
    }

    #[tokio::test]
    async fn test_show_table_columns() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("main".to_string()));
        let x = db.show_table_columns("employees").await.unwrap();
        for f in x.iter() {
            println!("coldef:{:?}", f);
        }
    }

    #[tokio::test]
    async fn test_check_conn() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(None);
        let d = crate::db::CheckConn::check_conn(&db).await.unwrap();
        println!("ping time is:{:?}", d);
    }

    #[tokio::test]
    async fn test_index() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_index";
        let _ = db.drop_index(table_name, "test_index_name").await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
            CREATE TABLE test_index (
                id INTEGER PRIMARY KEY,
                name VARCHAR,
                age INTEGER NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
        "#,
            )
            .await
            .unwrap();

        db.create_index(
            table_name,
            "test_index_name",
            IndexKind::Unique,
            &["name".to_string()],
        )
        .await
        .unwrap();

        let indexes = db.show_indexes(table_name).await.unwrap();
        for index in indexes.iter() {
            println!("index:{:?}", index);
        }
    }
    #[tokio::test]
    async fn test_column() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_column";

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
            CREATE TABLE test_column (
                id INTEGER PRIMARY KEY,
                name VARCHAR,
                age INTEGER,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            ); 
        "#,
            )
            .await
            .unwrap();

        db.add_table_column(
            table_name,
            "email",
            ColumnType {
                typ: "BIGINT".to_string(),
                nullness: Some("null".to_string()),
                after: None,
                default_value: Some(1.to_string()),
            },
        )
        .await
        .unwrap();

        db.modify_table_column(
            table_name,
            "email",
            "email2",
            Some(ColumnType {
                typ: "INT4".to_string(),
                nullness: Some("not null".to_string()),
                after: None,
                default_value: Some(2.to_string()),
            }),
        )
        .await
        .unwrap();

        db.modify_table_column(
            table_name,
            "email2",
            "email2",
            Some(ColumnType {
                typ: "INT4".to_string(),
                nullness: Some("null".to_string()),
                after: None,
                default_value: Some(2.to_string()),
            }),
        )
        .await
        .unwrap();
        db.drop_table_column(table_name, "email2").await.unwrap();
    }

    #[tokio::test]
    async fn test_types1() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types1";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
            CREATE TABLE test_types1 (
                bigint_column BIGINT,
                blob_column BLOB,
                boolean_column BOOLEAN,
                date_column DATE,
                decimal_column DECIMAL(10, 2) -- 假设精度为10，小数点后2位
            );
        "#,
            )
            .await
            .unwrap();
        let mut values = HashMap::new();
        values.insert("bigint_column".to_string(), Some("123".to_string()));
        values.insert(
            "blob_column".to_string(),
            Some("'\x01\x02\x03'".to_string()),
        );
        values.insert("boolean_column".to_string(), Some("TRUE".to_string()));
        values.insert("date_column".to_string(), Some("2025-03-25".to_string()));
        values.insert("decimal_column".to_string(), Some("12345.67".to_string()));
        db.table_insert(table_name, values).await.unwrap();
        let resp = db
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("bigint_column asc"),
            )
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            // export test
            let db = db;
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
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
    async fn test_types1_array() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name: &str = "test_types1_array";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
             CREATE TABLE test_types1_array (
                bigint_column BIGINT[],
                blob_column BLOB[],
                boolean_column BOOLEAN[],
                date_column DATE[],
                decimal_column DECIMAL(10, 2)[]
            );
        "#,
            )
            .await
            .unwrap();
        let mut values = HashMap::new();
        values.insert(
            "bigint_column".to_string(),
            Some("[1234567890, 9876543210]".to_string()),
        );
        values.insert(
            "blob_column".to_string(),
            Some("[X'12345678', X'87654321']".to_string()),
        );
        values.insert(
            "boolean_column".to_string(),
            Some("[TRUE, FALSE]".to_string()),
        );
        values.insert(
            "date_column".to_string(),
            Some("['2025-03-25', '2025-03-26']".to_string()),
        );
        values.insert(
            "decimal_column".to_string(),
            Some("[123.45, 678.90]".to_string()),
        );
        db.table_insert(table_name, values).await.unwrap();
        let resp = db
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("bigint_column asc"),
            )
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            // export test
            let db = db;
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
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
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types2";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
            CREATE TABLE test_types2 (
                col_double DOUBLE,
                col_float FLOAT,
                col_hugeint HUGEINT,
                col_integer INTEGER,
                col_interval INTERVAL,
                col_json JSON
            );

        "#,
            )
            .await
            .unwrap();
        {
            let mut values = HashMap::new();
            values.insert("col_double".to_string(), Some("123.456".to_string()));
            values.insert("col_float".to_string(), Some("78.9".to_string()));
            values.insert(
                "col_hugeint".to_string(),
                Some("1234567890123456789".to_string()),
            );
            values.insert("col_integer".to_string(), Some("42".to_string()));
            values.insert(
                "col_interval".to_string(),
                Some("INTERVAL '1 year 2 months 3 days 4 hours 5 minutes 6 seconds'".to_string()),
            );
            values.insert(
                "col_json".to_string(),
                Some(r#"{"key1": "value1", "key2": 123, "key3": [1, 2, 3]}"#.to_string()),
            );
            db.table_insert(table_name, values).await.unwrap();
        }

        let resp = db
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("col_double asc"),
            )
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            // export test
            let db = db;
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
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
    async fn test_types2_array() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types2_array";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
            CREATE TABLE test_types2_array (
                col_double DOUBLE[],
                col_float FLOAT[],
                col_hugeint HUGEINT[],
                col_integer INTEGER[],
                col_interval INTERVAL[],
                col_json JSON
            );

        "#,
            )
            .await
            .unwrap();
        let mut values = HashMap::new();
        values.insert("col_double".to_string(), Some("[123.456]".to_string()));
        values.insert("col_float".to_string(), Some("[78.9]".to_string()));
        values.insert(
            "col_hugeint".to_string(),
            Some("[1234567890123456789]".to_string()),
        );
        values.insert("col_integer".to_string(), Some("[42]".to_string()));
        values.insert(
            "col_interval".to_string(),
            Some("[INTERVAL '1 year 2 months 3 days 4 hours 5 minutes 6 seconds']".to_string()),
        );
        values.insert(
            "col_json".to_string(),
            Some(r#"{"key1": "value1", "key2": 123, "key3": [1, 2, 3]}"#.to_string()),
        );

        db.table_insert(table_name, values).await.unwrap();
        let resp = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test(None))
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            // export test
            let db = db;
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
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
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types3";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
           CREATE TABLE test_types3 (
                col_smallint SMALLINT,
                col_time TIME,
                col_timestamptz TIMESTAMPTZ,
                col_timestamp TIMESTAMP,
                col_tinyint TINYINT,
                col_ubigint UBIGINT
            );
        "#,
            )
            .await
            .unwrap();
        let mut values = HashMap::new();
        values.insert("col_smallint".to_string(), Some("32767".to_string()));
        values.insert("col_time".to_string(), Some("14:30:45".to_string()));
        values.insert(
            "col_timestamptz".to_string(),
            Some("2025-03-25 14:30:45+08".to_string()),
        );
        values.insert(
            "col_timestamp".to_string(),
            Some("2025-03-25 14:30:45".to_string()),
        );
        values.insert("col_tinyint".to_string(), Some("127".to_string()));
        values.insert(
            "col_ubigint".to_string(),
            Some("18446744073709551615".to_string()),
        );
        db.table_insert(table_name, values).await.unwrap();
        let resp = db
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("col_smallint asc"),
            )
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            // export test
            let db = db;
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
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
    async fn test_types3_array() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types3_array";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
           CREATE TABLE test_types3_array (
                col_smallint SMALLINT[],
                col_time TIME[],
                col_timestamptz TIMESTAMPTZ[],
                col_timestamp TIMESTAMP[],
                col_tinyint TINYINT[],
                col_ubigint UBIGINT[]
            );
        "#,
            )
            .await
            .unwrap();
        let mut values = HashMap::new();
        values.insert("col_smallint".to_string(), Some("[32767]".to_string()));
        values.insert("col_time".to_string(), Some("['14:30:45']".to_string()));
        values.insert(
            "col_timestamptz".to_string(),
            Some("['2025-03-25 14:30:45+08']".to_string()),
        );
        values.insert(
            "col_timestamp".to_string(),
            Some("['2025-03-25 14:30:45']".to_string()),
        );
        values.insert("col_tinyint".to_string(), Some("[127]".to_string()));
        values.insert(
            "col_ubigint".to_string(),
            Some("[18446744073709551615]".to_string()),
        );
        db.table_insert(table_name, values).await.unwrap();
        let resp = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test(None))
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            // export test
            let db = db;
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
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

    #[ignore = "https://github.com/duckdb/duckdb-rs/issues/478"]
    #[tokio::test]
    async fn test_types4() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types4";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
           CREATE TABLE test_types4 (
                -- my_uhugeint UHUGEINT,
                my_uinteger UINTEGER,
                my_usmallint USMALLINT,
                my_utinyint UTINYINT,
                my_varchar VARCHAR
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let mut values = HashMap::new();
        // values.insert(
        //     "my_uhugeint".to_string(),
        //     Some("12345678901234567890".to_string()),
        // );
        values.insert("my_uinteger".to_string(), Some("3000000000".to_string()));
        values.insert("my_usmallint".to_string(), Some("65000".to_string()));
        values.insert("my_utinyint".to_string(), Some("123".to_string()));
        values.insert("my_varchar".to_string(), Some("Example1".to_string()));
        db.table_insert(table_name, values).await.unwrap();
        let resp = db
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("my_uinteger asc"),
            )
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            // export test
            let db = db;
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
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
    async fn test_types5() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types5";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
            CREATE TABLE test_types5 (
                my_array INTEGER[3],
                my_list VARCHAR[],
                my_map MAP(VARCHAR, INTEGER),
                my_struct STRUCT(name VARCHAR, age INTEGER),
                my_union UNION(num INTEGER, text VARCHAR)
            );
        "#,
            )
            .await
            .unwrap();
        let mut values = HashMap::new();
        values.insert("my_array".to_string(), Some("[1, 2, 3]".to_string()));
        values.insert(
            "my_list".to_string(),
            Some("['apple', 'banana', 'cherry']".to_string()),
        );
        values.insert(
            "my_map".to_string(),
            Some("map(['key1', 'key2'] , [1,2])".to_string()),
        );
        values.insert(
            "my_struct".to_string(),
            Some("{'name' : 'xxx' , 'age' : 100}".to_string()),
        );
        values.insert("my_union".to_string(), Some("'hello'".to_string()));
        db.table_insert(table_name, values).await.unwrap();
        let resp = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test(None))
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            // export test
            let db = db;
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
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
}
