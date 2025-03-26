use chrono::NaiveDate;
use dashmap::DashMap;
use duckdb::Connection;
use duckdb::arrow::datatypes::DataType;
use duckdb::types::TimeUnit;
use r2d2::PooledConnection;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::task::block_in_place;

use crate::datasource_config::DuckDBConfig;
use crate::db::{
    BackendName, CatalogTrait, CheckConn, CreateDatabaseExtra, CreateSchemaExtra,
    CreateTableAndIndexSql, DBTrait, ExportKind, Field, ListTable, NamedStaff, QueryResp, QuerySql,
    ReadOnlyTrait, SchemaTrait, ServerMetas, ShowExtraArgs, SqlExecuteRet, TableListResp,
};
use crate::helper::{SqlKind, SqlWithKind, create_csv, sql_to_sql_seq};
use crate::meta_value::MetaValue;
use crate::query_log::QueryLog;
use std::ops::DerefMut;
pub struct PoolManage {
    path: String,
}

fn gen_show_sql(
    schema: &str, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize,
) -> String {
    let mut sql = format!("SELECT * from \"{schema}\".\"{table_name}\"");
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

fn make_conn(path: &str) -> duckdb::Result<Connection> {
    let mut config = duckdb::Config::default();
    config = config.access_mode(duckdb::AccessMode::ReadWrite)?;
    let conn = duckdb::Connection::open_with_flags(path, config)?;
    Ok(conn)
}

impl r2d2::ManageConnection for PoolManage {
    type Connection = duckdb::Connection;
    type Error = duckdb::Error;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        make_conn(&self.path)
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        let sql = "select 1";
        conn.execute(&sql, [])?;
        Ok(())
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        self.is_valid(conn).is_err()
    }
}

#[derive(Clone)]
pub struct DuckDB {
    config: DuckDBConfig,
    pool: r2d2::Pool<PoolManage>,
    query_connections: Arc<DashMap<uuid::Uuid, Arc<Mutex<duckdb::Connection>>>>,
}

#[async_trait::async_trait]

impl ServerMetas for DuckDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let sql = "PRAGMA version;";
        let conn = self.get_conn()?;

        let version = conn.query_row(sql, [], |row| {
            let version: String = row.get(0)?;
            Ok(version)
        })?;
        anyhow::Ok(vec![MetaValue::new("version", version)])
    }
}

impl DuckDB {
    pub fn new(config: DuckDBConfig) -> anyhow::Result<Self> {
        let pool = r2d2::Builder::<PoolManage>::new().build(PoolManage {
            path: config.path.clone(),
        })?;
        anyhow::Ok(Self {
            config,
            pool,
            query_connections: Default::default(),
        })
    }

    pub(crate) fn get_conn(&self) -> anyhow::Result<PooledConnection<PoolManage>> {
        let conn = block_in_place(|| self.pool.get())?;

        if let Some(d) = self.config.db.as_ref() {
            conn.execute(&format!("use {}", d), [])?;
        }
        if let Some(s) = self.config.schema.as_ref() {
            conn.execute(&format!("SET SCHEMA '{s}';"), [])?;
        }

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
        let conn = self.get_conn()?;
        let now = std::time::Instant::now();
        conn.execute("select 1", [])?;
        anyhow::Ok(now.elapsed())
    }
}

impl BackendName for DuckDB {
    fn backend_name(&self) -> &str {
        "duckdb"
    }
}

impl DuckDB {
    async fn export_query_result_as_csv(
        &self, sql: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let handle = Clone::clone(self);
        let sql = sql.to_owned();

        tokio::task::spawn_blocking(move || {
            let conn = handle.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            stmt.execute([])?;

            let names = stmt.column_names();
            let types = (0..names.len())
                .map(|x| stmt.column_type(x))
                .collect::<Vec<_>>();
            if include_header {
                let fs = names.iter().map(|x| x.clone()).map(Some).collect();
                sender.blocking_send(Cow::Owned(create_csv(&vec![fs], delimiter)))?;
            }
            let mut stream = stmt.query([])?;
            while let Some(row) = stream.next()? {
                let data = encode_row(&row, &types[..], None)?;
                sender.blocking_send(Cow::Owned(create_csv(&vec![data], delimiter)))?;
            }
            anyhow::Ok(())
        })
        .await??;
        anyhow::Ok(())
    }

    async fn export_query_result_as_json(
        &self, sql: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let handle = Clone::clone(self);
        let sql = sql.to_owned();
        sender.send("[".into()).await?;
        tokio::task::spawn_blocking(move || {
            let conn = handle.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            stmt.execute([])?;
            let names = stmt.column_names();
            let mut first = true;
            let mut stream = stmt.query([])?;
            while let Some(row) = stream.next()? {
                let value = encode_row_as_json(&names, row)?;
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
        &self, sql: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let handle = Clone::clone(self);
        let sql = sql.to_owned();

        tokio::task::spawn_blocking(move || {
            let conn = handle.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            // after  `execute` can call column_names
            stmt.execute([])?;

            let names = stmt.column_names();
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let serde_value = encode_row_as_json(&names, row)?;
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
    names: &Vec<String>, row: &duckdb::Row<'stmt>,
) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    let mut ret = serde_json::Map::new();
    for (index, name) in names.iter().enumerate() {
        let v: duckdb::types::Value = row.get(index)?;
        ret.insert(name.clone(), value_2_json(&v)?);
    }
    return anyhow::Ok(ret);
    fn value_2_json(value: &duckdb::types::Value) -> anyhow::Result<serde_json::Value> {
        let v = match value {
            duckdb::types::Value::Null => serde_json::Value::Null,
            duckdb::types::Value::Boolean(x) => serde_json::Value::from(*x),
            duckdb::types::Value::TinyInt(x) => serde_json::Value::from(*x),
            duckdb::types::Value::SmallInt(x) => serde_json::Value::from(*x),
            duckdb::types::Value::Int(x) => serde_json::Value::from(*x),
            duckdb::types::Value::BigInt(x) => serde_json::Value::from(*x),
            duckdb::types::Value::HugeInt(x) => serde_json::Value::from(x.to_string()),
            duckdb::types::Value::UTinyInt(x) => serde_json::Value::from(*x),
            duckdb::types::Value::USmallInt(x) => serde_json::Value::from(*x),
            duckdb::types::Value::UInt(x) => serde_json::Value::from(*x),
            duckdb::types::Value::UBigInt(x) => serde_json::Value::from(*x),
            duckdb::types::Value::Float(x) => serde_json::Value::from(*x),
            duckdb::types::Value::Double(x) => serde_json::Value::from(*x),
            duckdb::types::Value::Decimal(decimal) => serde_json::Value::from(decimal.to_string()),
            duckdb::types::Value::Timestamp(time_unit, value) => {
                let mut m = serde_json::Map::new();
                m.insert(
                    "timeUnit".to_string(),
                    serde_json::Value::from(format!("{:?}", time_unit).to_lowercase()),
                );
                m.insert("value".to_string(), serde_json::Value::from(*value));
                serde_json::Value::Object(m)
            }
            duckdb::types::Value::Text(x) => serde_json::Value::from(x.clone()),
            duckdb::types::Value::Blob(items) => {
                let items = items
                    .iter()
                    .map(|x| serde_json::Value::from(*x))
                    .collect::<Vec<_>>();
                serde_json::Value::Array(items)
            }
            duckdb::types::Value::Date32(days) => {
                let d = format_date(*days)?;
                serde_json::Value::from(d)
            }
            duckdb::types::Value::Time64(time_unit, value) => {
                let mut m = serde_json::Map::new();
                m.insert(
                    "timeUnit".to_string(),
                    serde_json::Value::from(format!("{:?}", time_unit).to_lowercase()),
                );
                m.insert("value".to_string(), serde_json::Value::from(*value));
                serde_json::Value::Object(m)
            }
            duckdb::types::Value::Interval {
                months,
                days,
                nanos,
            } => {
                let v = format_interval(*months, *days, *nanos);
                serde_json::Value::from(v)
            }
            duckdb::types::Value::List(values) => {
                let mut ret = Vec::with_capacity(values.len());
                for value in values.iter() {
                    ret.push(value_2_json(value)?);
                }
                serde_json::Value::Array(ret)
            }
            duckdb::types::Value::Enum(x) => serde_json::Value::from(x.clone()),
            duckdb::types::Value::Struct(ordered_map) => {
                let mut m = serde_json::Map::new();
                for (k, v) in ordered_map.iter() {
                    m.insert(k.clone(), value_2_json(v)?);
                }
                serde_json::Value::Object(m)
            }
            duckdb::types::Value::Array(values) => {
                let mut ret = Vec::with_capacity(values.len());
                for value in values.iter() {
                    ret.push(value_2_json(value)?);
                }
                serde_json::Value::Array(ret)
            }
            duckdb::types::Value::Map(ordered_map) => {
                let mut m = serde_json::Map::new();
                for (k, v) in ordered_map.iter() {
                    m.insert(serde_json::to_string(&value_2_json(k)?)?, value_2_json(v)?);
                }
                serde_json::Value::Object(m)
            }
            duckdb::types::Value::Union(value) => value_2_json(&value)?,
        };

        anyhow::Ok(v)
    }
}

fn get_sql_seq(sql: &str) -> anyhow::Result<Vec<SqlWithKind>> {
    let dialect = sqlparser::dialect::DuckDbDialect {};
    sql_to_sql_seq(
        sql,
        &dialect,
        |s| SqlKind::get_from_statement(s),
        |x| SqlKind::get_from_sql(x),
    )
}

impl DuckDB {
    fn query_resp(conn: &Connection, sql: &str) -> anyhow::Result<QueryResp> {
        let mut stmt = conn.prepare(sql)?;
        stmt.execute([])?;
        let types: Vec<_> = (0..stmt.column_count())
            .map(|x| stmt.column_type(x))
            .collect();
        let names = stmt.column_names();
        let mut rows = block_in_place(|| stmt.query([]))?;
        let mut data = vec![];
        while let Some(row) = block_in_place(|| rows.next())? {
            let d = encode_row(row, &types[..], None)?;
            data.push(d);
        }
        anyhow::Ok(QueryResp {
            fields: names
                .iter()
                .zip(types.iter())
                .map(|(name, typ)| Field::from_name_type(name, typ.to_string()))
                .collect::<Vec<_>>(),
            data,
        })
    }
}

#[async_trait::async_trait]
impl QuerySql for DuckDB {
    async fn query_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let mut _log_gurad = QueryLog::get_guard(query_id, sql);
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();

        let ss = match get_sql_seq(sql) {
            Ok(x) => x,
            Err(_err) => {
                vec![SqlWithKind {
                    sql: sql.to_string(),
                    kind: SqlKind::Query,
                }]
            }
        };
        let mut ret = Vec::new();
        for s in ss.iter() {
            match s.kind {
                SqlKind::Query => match Self::query_resp(&conn, &s.sql) {
                    Ok(x) => ret.push(Ok(x)),
                    Err(err) => ret.push(Err(err.to_string())),
                },
                _ => match conn.execute(&s.sql, []) {
                    Ok(x) => ret.push(Ok(QueryResp::row_affected(x as u64))),
                    Err(err) => ret.push(Err(err.to_string())),
                },
            }
        }
        _log_gurad.set_success(ret.iter().all(|x| x.is_ok()));
        anyhow::Ok(ret)
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

    async fn execute_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let mut _log_gurad = QueryLog::get_guard(query_id, sql);
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let resp = match conn.execute_batch(sql) {
            Ok(_) => Ok(QueryResp::execute_ok()),
            Err(err) => Err(err.to_string()),
        };
        _log_gurad.set_success(resp.is_ok());
        anyhow::Ok(resp)
    }

    fn clone(&self) -> Box<dyn QuerySql> {
        Box::new(Clone::clone(self))
    }

    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let uuid = uuid::Uuid::new_v4();
        let conn = make_conn(&self.config.path)?;
        self.query_connections
            .insert(uuid, Arc::new(Mutex::new(conn)));
        anyhow::Ok(uuid)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        let conn = make_conn(&self.config.path)?;
        self.query_connections
            .insert(id, Arc::new(Mutex::new(conn)));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.query_connections.remove(&id);
        anyhow::Ok(())
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
        let r = block_in_place(|| {
            conn.query_row(&sql, [], |row| {
                let r: i64 = row.get(0)?;
                Ok(r)
            })
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

fn encode_value(
    value: &duckdb::types::Value, ty: &DataType, prased_type: Option<&sqlparser::ast::DataType>,
    nested: bool,
) -> anyhow::Result<Option<String>> {
    log::trace!("value:{:?} ty:{:?} parse type:{:?}", value, ty, prased_type);
    let is_time = matches!(ty, DataType::Time32(_)) || matches!(ty, DataType::Time64(_));
    let is_bit = prased_type
        .map(|x| match x {
            sqlparser::ast::DataType::Bit(_) => true,
            _ => false,
        })
        .unwrap_or(false);
    let r = match value {
        duckdb::types::Value::Null => None,
        duckdb::types::Value::Boolean(x) => Some(x.to_string()),
        duckdb::types::Value::TinyInt(x) => Some(x.to_string()),
        duckdb::types::Value::SmallInt(x) => Some(x.to_string()),
        duckdb::types::Value::Int(x) => Some(x.to_string()),
        duckdb::types::Value::BigInt(x) => Some(x.to_string()),
        duckdb::types::Value::HugeInt(x) => Some(x.to_string()),
        duckdb::types::Value::UTinyInt(x) => Some(x.to_string()),
        duckdb::types::Value::USmallInt(x) => Some(x.to_string()),
        duckdb::types::Value::UInt(x) => Some(x.to_string()),
        duckdb::types::Value::UBigInt(x) => Some(x.to_string()),
        duckdb::types::Value::Float(x) => Some(x.to_string()),
        duckdb::types::Value::Double(x) => Some(x.to_string()),
        duckdb::types::Value::Decimal(decimal) => Some(decimal.to_string()),
        duckdb::types::Value::Timestamp(time_unit, value) => {
            let v = format_timestamp(*time_unit, *value, is_time)?;
            Some(v)
        }
        duckdb::types::Value::Text(x) => {
            let mut x = Some(x.to_string());
            if nested {
                x = x.map(|x| format!("'{}'", x))
            }
            x
        }
        duckdb::types::Value::Blob(items) => {
            if is_bit {
                let last = items.last();
                let last = match last {
                    Some(x) => x,
                    None => anyhow::bail!("bit expect at least one byte"),
                };
                if (last & 1) != 0 {
                    Some(1.to_string())
                } else {
                    Some(0.to_string())
                }
            } else {
                Some(format!("x'{}'", hex::encode(items.as_slice())))
            }
        }
        duckdb::types::Value::Date32(x) => {
            let mut x = format_date(*x)?;
            if nested {
                x = format!("'{}'", x)
            }
            Some(x)
        }
        duckdb::types::Value::Time64(time_unit, value) => {
            let mut v = format_timestamp(*time_unit, *value, is_time)?;
            if nested {
                v = format!("'{}'", v)
            }
            Some(v)
        }
        duckdb::types::Value::Interval {
            months,
            days,
            nanos,
        } => {
            let x = format_interval(*months, *days, *nanos);
            Some(x)
        }

        duckdb::types::Value::List(values) | duckdb::types::Value::Array(values) => {
            let ety = match ty {
                DataType::List(x) => x.as_ref().data_type(),
                DataType::FixedSizeList(x, _) => x.as_ref().data_type(),
                _ => {
                    anyhow::bail!("expect a list type but:{:?}", ty)
                }
            };
            let parsed_type = match prased_type {
                Some(x) => match x {
                    sqlparser::ast::DataType::Array(x) => match x {
                        sqlparser::ast::ArrayElemTypeDef::SquareBracket(data_type, _) => {
                            Some(data_type.as_ref())
                        }
                        _ => {
                            anyhow::bail!("not suitable type for duckdb :{:?}", x)
                        }
                    },
                    _ => {
                        anyhow::bail!("expect an array type but:{:?}", ty)
                    }
                },
                None => None,
            };

            let mut eles = Vec::with_capacity(values.len());
            for value in values.iter() {
                let v = encode_value(value, ety, parsed_type, true)?;
                eles.push(v);
            }
            let eles = eles
                .into_iter()
                .map(|x| x.unwrap_or("null".to_string()))
                .collect::<Vec<_>>();
            Some(format!("[{}]", eles.join(",")))
        }
        duckdb::types::Value::Enum(x) => Some(format!("'{}'", x)),
        duckdb::types::Value::Struct(ordered_map) => {
            let mut values = Vec::new();

            for (k, v) in ordered_map.iter() {
                let ty = match ty {
                    DataType::Struct(x) => match x.find(&k) {
                        Some(x) => x.1.data_type(),
                        None => anyhow::bail!("field {} not found,this should not happen", k),
                    },
                    _ => {
                        anyhow::bail!("expect struct type but {:?}", ty)
                    }
                };
                let prased_type = match prased_type {
                    Some(x) => match x {
                        sqlparser::ast::DataType::Struct(x, _) => {
                            let f = x.iter().find(|x| match &x.field_name {
                                Some(x) => x.value.to_string().eq_ignore_ascii_case(k),
                                None => false,
                            });
                            match f {
                                Some(f) => Some(f.field_type.clone()),
                                None => {
                                    anyhow::bail!(
                                        "field '{}' not found,availables are:{:?}",
                                        k,
                                        x.iter()
                                            .map(|x| x.field_name.as_ref().map(|x| &x.value))
                                            .flatten()
                                            .collect::<Vec<_>>()
                                    )
                                }
                            }
                        }
                        _ => {
                            anyhow::bail!("expect a struct type but {:?}", x)
                        }
                    },
                    None => None,
                };
                values.push(format!(
                    "{}:{}",
                    k,
                    encode_value(v, ty, prased_type.as_ref(), true)?.unwrap_or("null".to_string())
                ));
            }

            Some(format!("{{{}}}", values.join(",")))
        }
        duckdb::types::Value::Map(ordered_map) => {
            let (kty, vty) = match ty {
                DataType::Map(k, _) => match k.data_type() {
                    DataType::Struct(fs) => {
                        let kty = fs.find("key");
                        let vty = fs.find("value");

                        match (kty, vty) {
                            (Some(k), Some(v)) => (k.1.data_type(), v.1.data_type()),
                            _ => {
                                anyhow::bail!("map must have key and value field")
                            }
                        }
                    }
                    _ => {
                        anyhow::bail!("type here should a struct")
                    }
                },
                _ => {
                    anyhow::bail!("data should be a map but {:?}", ty)
                }
            };

            let (pkty, pvty) = match prased_type {
                None => (None, None),
                Some(prased_type) => match prased_type {
                    sqlparser::ast::DataType::Map(k, v) => {
                        (Some(k.as_ref().clone()), Some(v.as_ref().clone()))
                    }
                    sqlparser::ast::DataType::Custom(x, subtypes) => {
                        if x.to_string().to_uppercase() == "MAP" {
                            if subtypes.len() != 2 {
                                anyhow::bail!("sub type should have length 2 but {:?}", subtypes)
                            }
                            (
                                Some(parse_type(&subtypes[0])?),
                                Some(parse_type(&subtypes[1])?),
                            )
                        } else {
                            anyhow::bail!("custon should be map but {:?}", x)
                        }
                    }
                    _ => {
                        anyhow::bail!("type must be map but {:?}", prased_type)
                    }
                },
            };
            let mut keys = Vec::new();
            let mut values = Vec::new();
            for (k, v) in ordered_map.iter() {
                keys.push(encode_value(k, kty, pkty.as_ref(), nested)?);
                values.push(encode_value(v, vty, pvty.as_ref(), nested)?);
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
        duckdb::types::Value::Union(value) => encode_value(&value, ty, None, true)?,
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

fn encode_row<'stmt>(
    row: &duckdb::Row<'stmt>, types: &[DataType], parsed_types: Option<&[sqlparser::ast::DataType]>,
) -> anyhow::Result<Vec<Option<String>>> {
    let mut values = Vec::with_capacity(types.len());
    for index in 0..types.len() {
        let ty = &types[index];
        let parsed_type = parsed_types.as_ref().map(|x| x.get(index)).flatten();
        let x: duckdb::types::Value = row.get(index)?;
        values.push(encode_value(&x, ty, parsed_type, false)?);
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
        self.export_query_result_as_csv(sql.as_str(), include_header, delimiter, sender)
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
        self.export_query_result_as_json(&sql, pretty, sender).await
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
        let handle = Clone::clone(self);
        let table_name = table_name.to_owned();
        let fs = self.show_table_columns(&table_name).await?;
        let mut parsed_types = Vec::with_capacity(fs.len());
        for f in fs.iter() {
            parsed_types.push(parse_type(&f.typ)?);
        }
        tokio::task::spawn_blocking(move || {
            let conn = handle.get_conn()?;

            let sql = format!(
                r#"SELECT * from "{}"."{}""#,
                handle.config.schema_str(),
                table_name
            );
            let mut stmt = conn.prepare(&sql)?;
            stmt.execute([])?;
            let names = stmt.column_names();
            let types: Vec<_> = (0..names.len()).map(|x| stmt.column_type(x)).collect();
            let mut stream = stmt.query([])?;
            while let Some(row) = stream.next()? {
                let data = encode_row(row, &types[..], Some(&parsed_types[..]))?;
                let mut values = HashMap::new();
                for (name, value) in names.iter().zip(data.into_iter()) {
                    values.insert(name.clone(), value);
                }
                let sql = (&handle as &dyn SchemaTrait).insert_sql(
                    handle.config.schema.as_ref().map(|x| x.as_str()),
                    &table_name,
                    &fs,
                    &values,
                )?;
                sender.blocking_send(Cow::Owned(sql))?;
            }
            anyhow::Ok(())
        })
        .await??;
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl SchemaTrait for DuckDB {
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
        let conn = self.get_conn()?;
        let mut ret = block_in_place(|| {
            conn.query_row(&sql, [], |x| {
                let sql: String = x.get(0)?;
                Ok(CreateTableAndIndexSql {
                    create_table: sql,
                    indexes: vec![],
                })
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
            let mut rows = block_in_place(|| stmt.query([]))?;
            while let Some(row) = rows.next()? {
                let s: String = row.get(0)?;
                indexes.push(s);
            }
            indexes
        };
        anyhow::Ok(ret)
    }

    async fn list_view(&self) -> anyhow::Result<Vec<NamedStaff>> {
        unimplemented!()
    }
    async fn drop_view(&self, _view_name: &str) -> anyhow::Result<()> {
        unimplemented!()
    }
    async fn show_view(
        &self, _view_name: &str, _where_str: Option<&str>, _order_by: Option<&str>, _page: usize,
        _page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<TableListResp> {
        unimplemented!()
    }
    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let conn = self.get_conn()?;

        let primary_keys = {
            let sql = format!(
                "
                SELECT c.column_name
                    FROM information_schema.table_constraints tc
                    JOIN information_schema.constraint_column_usage c
                    ON tc.constraint_name = c.constraint_name
                    WHERE tc.table_name = '{}'
                    AND tc.table_schema = '{}'  -- 指定 schema 名称
                    AND tc.constraint_type = 'PRIMARY KEY';
           ",
                table_name,
                self.config.schema_str()
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
            SELECT column_name, data_type, is_nullable , column_default
                FROM information_schema.columns
                WHERE table_name = '{}'
                AND table_schema = '{}';  -- 指定 schema 名称
        "#,
            table_name,
            self.config.schema_str()
        );
        log::info!("sql:{}", sql);

        let mut stmt = conn.prepare(&sql)?;

        let mut rows = block_in_place(|| stmt.query([]))?;
        let mut ret = Vec::new();
        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            let data_type: String = row.get(1)?;
            let is_nullable: String = row.get(2)?;
            let column_default: Option<String> = row.get(3)?;
            let is_primary = primary_keys.contains(&name);

            ret.push(Field {
                field: name,
                typ: data_type,
                nullable: is_nullable.eq_ignore_ascii_case("yes"),
                default_value: column_default,
                is_primary,
                is_auto_increment: false,
                sortable: true,
                firebird_binary_sub_type: None,
            });
        }
        anyhow::Ok(ret)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let sql = format!(r#"ALTER TABLE "{old}"  RENAME TO "{new}";"#);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, []))?;
        anyhow::Ok(())
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        let sql = format!(r#"CREATE TABLE "{dup}" AS SELECT * FROM "{table}";"#);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, []))?;
        anyhow::Ok(())
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"SELECT  table_name
            FROM information_schema.tables
            WHERE table_schema = '{}' AND table_type = 'BASE TABLE';"#,
            self.config.schema_str()
        );

        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        let mut ret = Vec::new();
        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret.into_iter().map(|x| ListTable::from(x)).collect())
    }

    async fn show_view_columns(&self, name: &str) -> anyhow::Result<Vec<Field>> {
        unimplemented!()
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<TableListResp> {
        let total = self.query_count(table_name, where_str.clone())?;
        let sql = gen_show_sql(
            self.config.schema_str(),
            table_name,
            where_str,
            order_by,
            page,
            page_size,
        );
        log::info!("sql:{}", sql);
        let fs = self.show_table_columns(table_name).await?;
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(&sql)?;
        stmt.execute([])?;
        let types: Vec<_> = (0..stmt.column_count())
            .map(|x| stmt.column_type(x))
            .collect();
        let mut parsed_type = Vec::with_capacity(fs.len());
        for f in fs.iter() {
            parsed_type.push(parse_type(&f.typ)?);
        }
        let mut rows = stmt.query([])?;
        let mut data = Vec::new();
        while let Some(row) = rows.next()? {
            data.push(encode_row(&row, &types[..], Some(&parsed_type[..]))?);
        }
        anyhow::Ok(TableListResp::new(fs, total, data, sql))
    }

    fn put_value_in_sql(
        &self, typ: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        put_value_in_sql(typ, value)
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;

        let sql = (self as &dyn SchemaTrait).insert_sql(
            self.config.schema.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &values,
        )?;
        log::info!("sql:{}", sql);

        let conn = self.get_conn()?;
        let changes = block_in_place(|| conn.execute(&sql, []))?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(changes as u64),
        })
    }

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;

        let sql = (self as &dyn SchemaTrait).update_sql(
            self.config.schema.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &values,
            &wheres,
            type_support_compare,
        )?;

        let conn = self.get_conn()?;
        let changes = block_in_place(|| conn.execute(&sql, []))?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(changes as u64),
        })
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;

        let sql = (self as &dyn SchemaTrait).delete_sql(
            self.config.schema.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &wheres,
            type_support_compare,
        )?;

        let conn = self.get_conn()?;
        let changes = block_in_place(|| conn.execute(&sql, []))?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(changes as u64),
        })
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DELETE FROM "{}";"#, table_name);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, []))?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        let sql = format!(r#"DROP TABLE "{}";"#, table);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, []))?;
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
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
        _ => anyhow::Ok(value.map(|x| x.to_string()).unwrap_or("null".to_string())),
    }
}

fn type_support_compare(_typ: &str) -> bool {
    true
}

#[async_trait::async_trait]

impl DBTrait for DuckDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let sql = format!(
            "SELECT schema_name FROM information_schema.schemata  where catalog_name = '{}' ;",
            self.config.database_str()
        );
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        let mut p = conn.prepare(&sql)?;
        let mut rows = block_in_place(|| p.query([]))?;
        let mut ret = Vec::new();
        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
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
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, []))?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("DROP SCHEMA {} CASCADE;", name);
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, []))?;
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl CatalogTrait for DuckDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        let sql = "SELECT database_name FROM duckdb_databases();";
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        let mut p = conn.prepare(&sql)?;
        let mut rows = p.query([])?;
        let mut ret = Vec::new();
        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        let mut x = Clone::clone(self);
        x.config.db = Some(name.to_owned());
        anyhow::Ok(Arc::new(x))
    }
    async fn create_database(
        &self, name: &str, extra: Option<CreateDatabaseExtra>,
    ) -> anyhow::Result<()> {
        self.can_write()?;
        let path = match &extra {
            Some(x) => match &x {
                CreateDatabaseExtra::DuckDB { path } => path,
            },
            None => anyhow::bail!("attach database need extra parameter."),
        };
        let sql = format!("ATTACH '{path}' AS {name}");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        log::info!("attach sql:{} {} ", sql, conn.is_autocommit());
        conn.execute(&sql, [])?;
        anyhow::Ok(())
    }

    async fn drop_database(&self, name: &str) -> anyhow::Result<()> {
        if self
            .config
            .db
            .as_ref()
            .map(|x| x.as_str().eq_ignore_ascii_case(&name))
            .unwrap_or_default()
        {
            anyhow::bail!("can't detach self.")
        }

        let sql = format!("DETACH  {name}");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, []))?;
        anyhow::Ok(())
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
fn make_test_object(schema: Option<String>) -> DuckDB {
    let dir = env!("CARGO_MANIFEST_DIR");
    let path = format!("{dir}/tests/test.duckdb");

    DuckDB::new(DuckDBConfig {
        path,
        read_only: false,
        db: Some("test".to_string()),
        schema,
    })
    .unwrap()
}

fn parse_type(typ: &str) -> anyhow::Result<sqlparser::ast::DataType> {
    let d = sqlparser::dialect::DuckDbDialect {};
    let mut p = sqlparser::parser::Parser::new(&d);
    p = p.try_with_sql(&typ)?;
    let ret = p.parse_data_type()?;
    anyhow::Ok(ret)
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::enable_test_log;

    #[test]
    fn test_parse_type() {
        {
            let x = parse_type("BIT[]").unwrap();

            println!("parsed type:{:?}", x);
        }
        {
            let x = parse_type("STRUCT(name VARCHAR, age INTEGER)").unwrap();
            println!("parsed type:{:?}", x);
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_database() {
        enable_test_log();
        let db = make_test_object(None);
        db.create_database(
            "abc",
            Some(CreateDatabaseExtra::DuckDB {
                path: {
                    let dir = env!("CARGO_MANIFEST_DIR");
                    let path = format!("{dir}/tests/abc.duckdb");
                    path
                },
            }),
        )
        .await
        .unwrap();

        let x = db.list_database().await.unwrap();
        println!("database:{:?}", x);
        db.drop_database("abc").await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_server_metas() {
        let db = make_test_object(None);
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_schema() {
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_list_table() {
        let db = make_test_object(Some("main".to_string()));
        let x = db.list_table().await.unwrap();
        println!("tables:{:?}", x);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_show_table_columns() {
        let db = make_test_object(Some("main".to_string()));
        let x = db.show_table_columns("employees").await.unwrap();
        for f in x.iter() {
            println!("coldef:{:?}", f);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_check_conn() {
        let db = make_test_object(None);
        let d = crate::db::CheckConn::check_conn(&db).await.unwrap();
        println!("ping time is:{:?}", d);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_show_table_columns1111() {
        let db = make_test_object(Some("main".to_string()));
        let conn = db.get_conn().unwrap();
        let mut stmt = conn.prepare("SELECT * from main.test_types1").unwrap();
        let _xxxx = stmt.execute([]).unwrap();

        println!("{:?}", stmt.column_names());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_types1() {
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types1";
        let _ = db.truncate_table(table_name).await;
        let query_id = db.open_connection().await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
            CREATE TABLE test_types1 (
                bigint_column BIGINT,
                bit_column BIT,
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
        values.insert("bit_column".to_string(), Some("1".to_string()));
        values.insert("blob_column".to_string(), Some("x'0102030405'".to_string()));
        values.insert("boolean_column".to_string(), Some("TRUE".to_string()));
        values.insert("date_column".to_string(), Some("2025-03-25".to_string()));
        values.insert("decimal_column".to_string(), Some("12345.67".to_string()));
        db.insert(table_name, values).await.unwrap();
        let resp = db
            .show_table(
                table_name,
                Some("1 = 1 "),
                Some("decimal_column asc "),
                1,
                100,
                None,
            )
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            //export test
            let db = &db as &dyn SchemaTrait;
            let ret = db
                .export_table_to_string(table_name, ExportKind::CSV {
                    include_header: true,
                    delimiter: ',',
                })
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
                .export_table_to_string(table_name, ExportKind::Sql {
                    include_table_schema: true,
                })
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_types1_array() {
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types1_array";
        let _ = db.truncate_table(table_name).await;
        let query_id = db.open_connection().await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
             CREATE TABLE test_types1_array (
                bigint_column BIGINT[],
                bit_column BIT[],
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
        values.insert("bit_column".to_string(), Some("[1, 0]".to_string()));
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
        db.insert(table_name, values).await.unwrap();
        let resp = db
            .show_table(
                table_name,
                Some("1 = 1 "),
                Some("decimal_column asc "),
                1,
                100,
                None,
            )
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            //export test
            let db = &db as &dyn SchemaTrait;
            let ret = db
                .export_table_to_string(table_name, ExportKind::CSV {
                    include_header: true,
                    delimiter: ',',
                })
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
                .export_table_to_string(table_name, ExportKind::Sql {
                    include_table_schema: true,
                })
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_types2() {
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types2";
        let _ = db.truncate_table(table_name).await;
        let query_id = db.open_connection().await.unwrap();
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
        db.insert(table_name, values).await.unwrap();
        let resp = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            //export test
            let db = &db as &dyn SchemaTrait;
            let ret = db
                .export_table_to_string(table_name, ExportKind::CSV {
                    include_header: true,
                    delimiter: ',',
                })
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
                .export_table_to_string(table_name, ExportKind::Sql {
                    include_table_schema: true,
                })
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_types3() {
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types3";
        let _ = db.truncate_table(table_name).await;
        let query_id = db.open_connection().await.unwrap();
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
        db.insert(table_name, values).await.unwrap();
        let resp = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            //export test
            let db = &db as &dyn SchemaTrait;
            let ret = db
                .export_table_to_string(table_name, ExportKind::CSV {
                    include_header: true,
                    delimiter: ',',
                })
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
                .export_table_to_string(table_name, ExportKind::Sql {
                    include_table_schema: true,
                })
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_types4() {
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types4";
        let _ = db.truncate_table(table_name).await;
        let query_id = db.open_connection().await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
           CREATE TABLE test_types4 (
                my_uhugeint UHUGEINT,
                my_uinteger UINTEGER,
                my_usmallint USMALLINT,
                my_utinyint UTINYINT,
                my_varchar VARCHAR
            );
        "#,
            )
            .await
            .unwrap();
        let mut values = HashMap::new();
        values.insert(
            "my_uhugeint".to_string(),
            Some("12345678901234567890".to_string()),
        );
        values.insert("my_uinteger".to_string(), Some("3000000000".to_string()));
        values.insert("my_usmallint".to_string(), Some("65000".to_string()));
        values.insert("my_utinyint".to_string(), Some("123".to_string()));
        values.insert("my_usmallint".to_string(), Some("127".to_string()));
        values.insert("my_varchar".to_string(), Some("Example1".to_string()));
        db.insert(table_name, values).await.unwrap();
        let resp = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            //export test
            let db = &db as &dyn SchemaTrait;
            let ret = db
                .export_table_to_string(table_name, ExportKind::CSV {
                    include_header: true,
                    delimiter: ',',
                })
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
                .export_table_to_string(table_name, ExportKind::Sql {
                    include_table_schema: true,
                })
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_types5() {
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types5";
        let _ = db.truncate_table(table_name).await;
        let query_id = db.open_connection().await.unwrap();
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

        db.insert(table_name, values).await.unwrap();
        let resp = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        {
            //export test
            let db = &db as &dyn SchemaTrait;
            let ret = db
                .export_table_to_string(table_name, ExportKind::CSV {
                    include_header: true,
                    delimiter: ',',
                })
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
                .export_table_to_string(table_name, ExportKind::Sql {
                    include_table_schema: true,
                })
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[test]
    fn test_xxx() {}
}
