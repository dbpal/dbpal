#![allow(unused)]
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::iter::Cloned;
use std::sync::Arc;
use std::time::Duration;

use cdrs_tokio::types::rows;
use chrono::NaiveDate;
use dashmap::DashMap;
use duckdb::Connection;
use duckdb::arrow::compute::and;
use duckdb::types::FromSqlError;
use r2d2::PooledConnection;
use sqlparser::ast::{ColumnOption, Statement};
use sqlx::encode;
use tokio::sync::Mutex;
use tokio::task::block_in_place;

use crate::datasource_config::DuckDBConfig;
use crate::db::{
    BackendName, CatalogTrait, CheckConn, CreateDatabaseExtra, CreateSchemaExtra,
    CreateTableAndIndexSql, DBTrait, ExportKind, Field, ListTable, NamedStaff, QueryResp, QuerySql,
    ReadOnlyTrait, SchemaTrait, ServerMetas, ShowExtraArgs, SqlExecuteRet, TableListResp,
    not_support_features,
};
use crate::helper::create_csv;
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
        unimplemented!()
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
        let mut conn = self.get_conn().await?;

        let mut stream = sqlx::query(sql).fetch(conn.deref_mut()).try_chunks(100);
        let mut first = true;
        while let Some(rows) = stream.next().await {
            let rows = rows?;
            if first && rows.len() > 0 && include_header {
                let fs = rows
                    .get(0)
                    .unwrap()
                    .columns()
                    .iter()
                    .map(|x| x.name().to_string())
                    .map(Some)
                    .collect();

                sender
                    .send(Cow::Owned(create_csv(&vec![fs], delimiter)))
                    .await?;
                first = false;
            }
            let data = encode_rows(&rows)?;

            sender
                .send(Cow::Owned(create_csv(&data, delimiter)))
                .await?;
        }
        anyhow::Ok(())
    }

    async fn export_query_result_as_json(
        &self, sql: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;

        let mut stream = sqlx::query(sql).fetch(conn.deref_mut());

        sender.send("[".into()).await?;
        let mut first = true;
        while let Some(row) = stream.next().await {
            let row = row?;
            let value = encode_row_as_json(&row)?;
            if !first {
                sender.send(",".into()).await?;
            }
            if pretty {
                sender.send("\n".into()).await?;
            }
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
        anyhow::Ok(())
    }

    async fn export_query_result_as_json_line(
        &self, sql: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let handle = Clone::clone(self);
        let sql = sql.to_owned();

        tokio::task::spawn_blocking(move || {
            let mut conn = handle.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            let names = stmt.column_names();

            anyhow::Ok(())
        })
        .await;
        let mut stream = sqlx::query(sql).fetch(conn.deref_mut());
        while let Some(row) = stream.next().await {
            let row = row?;
            let value = encode_row_as_json(&row)?;

            sender
                .send(Cow::Owned(serde_json::to_string(&value)?))
                .await?;

            sender.send("\n".into()).await?;
        }
        anyhow::Ok(())
    }
}

fn encode_row_as_json<'stmt>(
    names: &Vec<String>, row: duckdb::Row<'stmt>,
) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    let ret = serde_json::Map::new();
    for (index, name) in names.iter().enumerate() {
        let v: duckdb::types::Value = row.get(index)?;
    }

    return anyhow::Ok(ret);

    fn value_2_json(value: &duckdb::types::Value) {
        match value {
            duckdb::types::Value::Null => serde_json::Value::Null,
            duckdb::types::Value::Boolean(x) => serde_json::Value::from(*x),
            duckdb::types::Value::TinyInt(x) => serde_json::Value::from(*x),
            duckdb::types::Value::SmallInt(x) => serde_json::Value::from(*x),
            duckdb::types::Value::Int(x) => serde_json::Value::from(*x),
            duckdb::types::Value::BigInt(x) => serde_json::Value::from(*x),
            duckdb::types::Value::HugeInt(x) => serde_json::Value::from(*x),
            duckdb::types::Value::UTinyInt(_) => todo!(),
            duckdb::types::Value::USmallInt(_) => todo!(),
            duckdb::types::Value::UInt(_) => todo!(),
            duckdb::types::Value::UBigInt(_) => todo!(),
            duckdb::types::Value::Float(_) => todo!(),
            duckdb::types::Value::Double(_) => todo!(),
            duckdb::types::Value::Decimal(decimal) => todo!(),
            duckdb::types::Value::Timestamp(time_unit, _) => todo!(),
            duckdb::types::Value::Text(_) => todo!(),
            duckdb::types::Value::Blob(items) => todo!(),
            duckdb::types::Value::Date32(_) => todo!(),
            duckdb::types::Value::Time64(time_unit, _) => todo!(),
            duckdb::types::Value::Interval {
                months,
                days,
                nanos,
            } => todo!(),
            duckdb::types::Value::List(values) => todo!(),
            duckdb::types::Value::Enum(_) => todo!(),
            duckdb::types::Value::Struct(ordered_map) => todo!(),
            duckdb::types::Value::Array(values) => todo!(),
            duckdb::types::Value::Map(ordered_map) => todo!(),
            duckdb::types::Value::Union(value) => todo!(),
        }
    }
}

#[async_trait::async_trait]
impl QuerySql for DuckDB {
    async fn query_sql(
        &self, query_id: uuid::Uuid, _sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        unimplemented!()
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
        let resp = match conn.execute(sql, []) {
            Ok(changes) => Ok(QueryResp::row_affected(changes as u64)),
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
        let mut conn = self.get_conn()?;
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

fn encode_value(value: &duckdb::types::Value) -> anyhow::Result<Option<String>> {
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
        duckdb::types::Value::Timestamp(time_unit, _) => todo!(),
        duckdb::types::Value::Text(x) => Some(x.to_string()),
        duckdb::types::Value::Blob(items) => Some(format!("x'{}'", hex::encode(items.as_slice()))),
        duckdb::types::Value::Date32(x) => {
            let x = format_date(*x)?;
            Some(x)
        }
        duckdb::types::Value::Time64(time_unit, _) => todo!(),
        duckdb::types::Value::Interval {
            months,
            days,
            nanos,
        } => {
            let x = format_interval(*months, *days, *nanos);
            Some(x)
        }
        duckdb::types::Value::List(values) => todo!(),
        duckdb::types::Value::Enum(_) => todo!(),
        duckdb::types::Value::Struct(ordered_map) => todo!(),
        duckdb::types::Value::Array(values) => todo!(),
        duckdb::types::Value::Map(ordered_map) => todo!(),
        duckdb::types::Value::Union(value) => todo!(),
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
    rows: &&duckdb::Row<'stmt>, col_count: usize,
) -> anyhow::Result<Vec<Option<String>>> {
    let mut values = Vec::with_capacity(col_count);

    for index in 0..col_count {
        let x: duckdb::types::Value = row.get(index)?;
        values.push(encode_value(&x)?);
    }
    anyhow::Ok(values)
}

#[async_trait::async_trait]
impl SchemaTrait for DuckDB {
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
        let mut conn = self.get_conn()?;
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
        let mut conn = self.get_conn()?;

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
        let sql = "SHOW TABLES";
        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(sql)?;
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
        page_size: usize, extra: Option<ShowExtraArgs>,
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
        let mut conn = self.get_conn()?;
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        let mut data = Vec::new();
        while let Some(row) = rows.next()? {
            data.push(encode_row(&row, fs.len())?);
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

        let mut conn = self.get_conn()?;
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

        let mut conn = self.get_conn()?;
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

        let mut conn = self.get_conn()?;
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

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
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
        "date" => anyhow::Ok(
            value
                .map(|x| format!("'{}'", x))
                .unwrap_or("null".to_string()),
        ),
        _ => anyhow::Ok(value.map(|x| x.to_string()).unwrap_or("null".to_string())),
    }
}

fn type_support_compare(typ: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enable_test_log;
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
        let db = make_test_object(None);
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
    async fn test_types1() {
        enable_test_log();
        let db = make_test_object(Some("main".to_string()));
        let table_name = "test_types1";
        let _ = db.truncate_table(table_name).await;
        let query_id = db.open_connection().await.unwrap();
        db.execute_sql(
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
    }

    #[test]
    fn test_xxx() {}
}
