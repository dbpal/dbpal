use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use chrono::Utc;
use dashmap::DashMap;
use futures_util::{StreamExt, TryStreamExt};
use sqlx::Executor;
use sqlx::mysql::MySqlRow;
use sqlx::pool::PoolConnection;
use sqlx::{Column, MySqlConnection, Row, TypeInfo};
use tokio::sync::Mutex;

use crate::MemoryLimit;
use crate::datasource_config::MysqlDbConfig;
use crate::db::{
    BackendName, CheckConn, CreateTableAndIndexSql, DBTrait, ExportKind, Field, FormatSql,
    FormatSqlOpts, Index, IndexKind, ListTable, ModifyTableColumnType, NamedStaff, ProcedureDetail,
    QueryResp, QuerySql, SchemaTrait, ServerMetas, SqlExecuteRet, TableListResp, TriggerDetail,
    TriggerEvent, TriggerTiming, UdfDetail, UdfOrPrecedureExtraParameter,
};
use crate::helper::{create_csv, is_dangerous_sql_by_parser, sqlformat_format_sql};
use crate::meta_value::MetaValue;
use crate::query_log::QueryLog;

#[derive(Clone)]
pub struct MysqlDB {
    pool: sqlx::MySqlPool,
    pub(crate) config: MysqlDbConfig,
    query_connection: Arc<DashMap<uuid::Uuid, Arc<Mutex<PoolConnection<sqlx::MySql>>>>>,
}

fn get_rows_types(rows: &Vec<MySqlRow>) -> Vec<Field> {
    if rows.len() == 0 {
        return vec![];
    }
    let mut ret = Vec::with_capacity(rows[0].columns().len());
    for c in rows[0].columns().iter() {
        let column = c;
        ret.push(Field {
            field: column.name().to_string(),
            typ: column.type_info().to_string(),
            nullable: true,
            default_value: None,
            is_primary: false,
            is_auto_increment: false,
        });
    }
    ret
}

impl FormatSql for MysqlDB {
    fn format_sql(&self, sql: &str, ops: Option<FormatSqlOpts>) -> anyhow::Result<String> {
        anyhow::Ok(sqlformat_format_sql(sql, ops.map(|x| x.into())))
    }
}

fn gen_select_sql(
    table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize,
) -> String {
    let mut sql = format!("SELECT * FROM {table_name}");
    if let Some(where_str) = where_str {
        sql.push_str(format!(" WHERE {where_str} ").as_str());
    }
    if let Some(order_by) = order_by {
        sql.push_str(format!(" ORDER BY {order_by} ").as_str());
    }
    let offset = (page - 1) * page_size;
    sql.push_str(format!(" LIMIT {} OFFSET {} ", page_size, offset).as_str());
    sql
}

fn encode_row_as_json(row: &MySqlRow) -> anyhow::Result<serde_json::Value> {
    let mut values = serde_json::Map::new();
    let null_value = serde_json::Value::Null;

    for c in 0..row.columns().len() {
        let col = row.column(c);
        let ty = col.type_info().name();

        let v = match ty.to_uppercase().as_str() {
            "BOOLEAN" | "BOOL" => {
                let value: Option<bool> = row.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "TINYINT" => {
                let value: Option<i8> = row.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "SMALLINT" => {
                let value: Option<i16> = row.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "INT" => {
                let value: Option<i32> = row.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "BIGINT" => {
                let value: Option<i64> = row.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "TINYINT UNSIGNED" => {
                let value: Option<u8> = row.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "SMALLINT UNSIGNED" => {
                let value: Option<u16> = row.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "INT UNSIGNED" => {
                let value: Option<u32> = row.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "BIGINT UNSIGNED" => {
                let value: Option<u64> = row.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "FLOAT" => {
                let value: Option<f32> = row.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "DOUBLE" => {
                let value: Option<f64> = row.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "VARCHAR" | "CHAR" | "TEXT" => {
                let value: Option<String> = row.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            _ if ty.starts_with("VARCHAR") || ty.starts_with("CHAR") || ty.starts_with("TEXT") => {
                let value: Option<String> = row.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "VARBINARY" | "BINARY" | "BLOB" => {
                let value: Option<Vec<u8>> = row.get(c);
                value
                    .map(|x| format!("0x{}", hex::encode_upper(x.as_slice())))
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }

            "TIMESTAMP" => {
                let value: Option<chrono::DateTime<Utc>> = row.get(c);
                value
                    .map(|x| x.naive_utc().to_string())
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "DATETIME" => {
                let value: Option<chrono::NaiveDateTime> = row.get(c);
                value
                    .map(|x| x.to_string())
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "DATE" => {
                let value: Option<chrono::NaiveDate> = row.get(c);
                value
                    .map(|x| x.to_string())
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "TIME" => {
                let value: Option<chrono::NaiveTime> = row.get(c);
                value
                    .map(|x| x.to_string())
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }

            _ => anyhow::bail!(format!("type '{ty}' not supported")),
        };
        values.insert(row.column(c).name().to_string(), v);
    }

    anyhow::Ok(serde_json::Value::Object(values))
}

#[async_trait::async_trait]
impl CheckConn for MysqlDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        let mut conn = self.get_conn().await?;
        sqlx::query("select 1").fetch_all(conn.deref_mut()).await?;

        anyhow::Ok(ins.elapsed())
    }
}

impl MysqlDB {
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
        let mut conn = self.get_conn().await?;
        let cols = SchemaTrait::show_columns(self, table_name).await?;
        let sql = format!("SELECT * from {}", table_name);
        let mut stream = sqlx::query(sql.as_str())
            .fetch(conn.deref_mut())
            .try_chunks(100);
        while let Some(rows) = stream.next().await {
            let rows = rows?;

            let data = encode_rows(&rows)?;
            for d in data.into_iter() {
                let mut m = HashMap::new();
                for (d, f) in d.into_iter().zip(cols.iter()) {
                    m.insert(f.field.clone(), d);
                }

                let insert_sql =
                    (self as &dyn SchemaTrait).insert_sql(None, table_name, &cols, &m)?;
                sender.send(Cow::Owned(insert_sql)).await?;
                sender.send("\n".into()).await?;
            }
        }
        anyhow::Ok(())
    }

    async fn export_query_result_as_json_line(
        &self, sql: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;

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

    async fn export_table_as_json_line(
        &self, table_name: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from {}", table_name);
        self.export_query_result_as_json_line(sql.as_str(), sender)
            .await
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

    async fn export_table_as_json(
        &self, table_name: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from {}", table_name);
        self.export_query_result_as_json(sql.as_str(), pretty, sender)
            .await
    }

    async fn export_query_result_as_csv(
        &self, sql: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let mut first = true;

        let mut stream = sqlx::query(sql).fetch(conn.deref_mut()).try_chunks(100);

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
                    .collect::<Vec<_>>();
                sender
                    .send(Cow::Owned(create_csv(&vec![fs], delimiter)))
                    .await?;
                first = false
            }

            let data = encode_rows(&rows)?;
            sender
                .send(Cow::Owned(create_csv(&data, delimiter)))
                .await?;
        }
        anyhow::Ok(())
    }

    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from {}", table_name);
        self.export_query_result_as_csv(sql.as_str(), include_header, delimiter, sender)
            .await
    }
}

impl MysqlDB {
    pub async fn query_partition_name(
        &self, db: &str, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        let sql = format!(
            r#"
            SELECT 
    PARTITION_NAME
FROM 
    information_schema.partitions
WHERE 
    table_name = '{table_name}'
    AND table_schema = '{db}';
        "#
        );
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

#[allow(dead_code)]
async fn query_trigger_detail(
    conn: &mut MySqlConnection, name: &str,
) -> anyhow::Result<(TriggerEvent, TriggerTiming, String, Vec<MetaValue>)> {
    let sql = format!("SHOW TRIGGERS WHERE `Trigger`='{name}'");

    let row = sqlx::query(&sql).fetch_one(conn).await?;
    let event: String = row.try_get(1)?;
    let sql = row.try_get(3)?;
    let timing: String = row.try_get(4)?;
    let create_time: chrono::DateTime<Utc> = row.try_get(5)?;
    let sql_mode: String = row.try_get(6)?;
    let create_time = create_time.naive_utc();
    let timing = if timing == "BEFORE" {
        TriggerTiming::Before
    } else {
        TriggerTiming::After
    };

    anyhow::Ok((TriggerEvent::try_from(event.as_str())?, timing, sql, vec![
        MetaValue::new("created", create_time),
        MetaValue::new("sql_mode", sql_mode),
    ]))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionOrProcedure {
    Function,
    Procedure,
}

impl FunctionOrProcedure {
    pub(crate) async fn query_list(
        self, conn: &mut sqlx::mysql::MySqlConnection, db: &str,
    ) -> anyhow::Result<Vec<String>> {
        let sql = format!(
            r#"
        SELECT ROUTINE_NAME
            FROM INFORMATION_SCHEMA.ROUTINES
            WHERE ROUTINE_SCHEMA = '{}' -- 替换为你的数据库名
            AND ROUTINE_TYPE = '{}'
       "#,
            db,
            self.sql_condtion_str()
        );

        let rows = sqlx::query(&sql)
            .fetch_all(conn)
            .await
            .context("can't query")?;
        let mut ret = Vec::new();

        for r in rows.iter() {
            let name: String = r.get(0);
            ret.push(name);
        }

        anyhow::Ok(ret)
    }

    async fn query_details(
        self, conn: &mut MySqlConnection, db: &str, name: &str,
    ) -> anyhow::Result<(String, Vec<MetaValue>)> {
        let sql = format!(
            r#"
            SELECT
                ROUTINE_DEFINITION, 
                CREATED, 
                Last_Altered
            FROM 
                information_schema.Routines
            WHERE 
                ROUTINE_SCHEMA = '{}' 
                AND ROUTINE_TYPE = '{}' AND ROUTINE_NAME='{}'
                "#,
            db,
            self.sql_condtion_str(),
            name
        );

        let row = sqlx::query(&sql).fetch_one(conn).await?;
        let def: String = row.try_get(0)?;
        let create: chrono::DateTime<Utc> = row.try_get(1)?;
        let create = create.naive_utc();
        let last_altered: chrono::DateTime<Utc> = row.try_get(1)?;
        let last_altered = last_altered.naive_utc();
        anyhow::Ok((def, vec![
            MetaValue::new("created", create),
            MetaValue::new("last_altered", last_altered),
        ]))
    }

    fn sql_condtion_str(self) -> &'static str {
        match self {
            FunctionOrProcedure::Function => "FUNCTION",
            FunctionOrProcedure::Procedure => "PROCEDURE",
        }
    }
}

#[async_trait::async_trait]
impl ServerMetas for MysqlDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let mut conn = self.get_conn().await?;
        let sql = r#"
            SELECT VERSION();  -- 0 
            SELECT NOW();   -- 1 
            SELECT @@version_comment; -- 2 
            SELECT @@version_compile_os; -- 3
            SELECT @@version_compile_machine; -- 4 
            SELECT @@default_storage_engine; -- 5 
            SELECT @@thread_cache_size; -- 6
            SELECT @@max_connections; -- 7 
        "#;
        let mut stream = sqlx::raw_sql(sql).fetch_many(conn.deref_mut());
        let mut metas = Vec::with_capacity(8);

        while let Some(item) = stream.next().await {
            let item = item?;
            let row = match item {
                sqlx::Either::Left(_x) => {
                    // anyhow::bail!(format!("should be sqlrow but result,result:{:?}", x))
                    // every select contains an MysqlQueryResult . just ignore them.
                    continue;
                }

                sqlx::Either::Right(row) => row,
            };
            match metas.len() {
                0 => {
                    let version: String = row.try_get(0)?;
                    metas.push(MetaValue::new("server version", version));
                }
                1 => {
                    let now: chrono::DateTime<Utc> = row.try_get(0)?;
                    metas.push(MetaValue::new("now", now.naive_utc()));
                }
                2 => {
                    let comment: String = row.try_get(0)?;
                    metas.push(MetaValue::new("server comment", comment));
                }

                3 => {
                    let os: String = row.try_get(0)?;
                    metas.push(MetaValue::new("compile os", os));
                }
                4 => {
                    let machine: String = row.try_get(0)?;
                    metas.push(MetaValue::new("compile machine", machine));
                }
                5 => {
                    let engine: String = row.try_get(0)?;
                    metas.push(MetaValue::new("default storage engine", engine));
                }
                6 => {
                    let size: u64 = row.try_get(0)?;
                    metas.push(MetaValue::new("thread cache size", size));
                }
                7 => {
                    let max: u64 = row.try_get(0)?;
                    metas.push(MetaValue::new("max connection size", max));
                }
                _ => {
                    anyhow::bail!("count the row number");
                }
            }
        }

        anyhow::Ok(metas)
    }
}

impl BackendName for MysqlDB {
    fn backend_name(&self) -> &str {
        "mysql"
    }
}

impl MysqlDB {
    pub(crate) async fn list_table_impl(&self, schema: &str) -> anyhow::Result<Vec<ListTable>> {
        let mut conn = self.get_conn().await?;
        let s = format!(
            r#"SELECT table_name ,data_length
        FROM information_schema.tables
        WHERE table_schema = '{}' AND  table_type = 'BASE TABLE';"#,
            schema
        );
        let rows = sqlx::query(s.as_str()).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let table_name: String = r.get(0);
            let size: Option<u64> = r.get(1);
            ret.push(ListTable::new(table_name, size));
        }
        anyhow::Ok(ret)
    }

    pub(crate) async fn list_view_impl(&self, schema: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
        SELECT table_name FROM information_schema.tables WHERE table_type = 'VIEW' AND table_schema = '{}';
        "#,
            schema
        );
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            ret.push(NamedStaff::from(name));
        }
        anyhow::Ok(ret)
    }
}

impl std::fmt::Debug for MysqlDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mysql on:{:?}", self.config)
    }
}

#[async_trait::async_trait]

impl SchemaTrait for MysqlDB {
    fn list_data_types(&self) -> anyhow::Result<&'static [&'static str]> {
        anyhow::Ok(crate::mysql_completion::TYPES)
    }
    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, typ: Option<ModifyTableColumnType>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        if old_name != new_name {
            let sql = format!(
                r#"
            ALTER TABLE {table}
                RENAME COLUMN {old_name} TO {new_name};
            "#
            );
            sqlx::query(&sql).execute(conn.deref_mut()).await?;
        }
        if let Some(typ) = typ.as_ref() {
            let sql = format!(
                r#"
            ALTER TABLE {table}
                MODIFY COLUMN {new_name} {} {};
            "#,
                &typ.typ,
                if let Some(null) = typ.nullness.as_ref() {
                    null.as_str()
                } else {
                    ""
                }
            );
            sqlx::query(&sql).execute(conn.deref_mut()).await?;
        }
        anyhow::Ok(())
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
            ALTER TABLE {table} DROP COLUMN {col};
        "#
        );
        let mut conn = self.get_conn().await?;
        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        use sqlx::Statement;
        let mut conn = self.get_conn().await?;
        let sql = format!("select * from {view_name} limit 1");
        let s = conn.deref_mut().prepare_with(&sql, &[]).await?;
        let ret = s
            .columns()
            .iter()
            .map(|x| Field::from_name_type(x.name(), x.type_info().to_string()))
            .collect::<Vec<_>>();
        anyhow::Ok(ret)
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let mut conn = self.get_conn().await?;

        let ret = FunctionOrProcedure::Procedure
            .query_list(conn.deref_mut(), self.config.db.as_ref().unwrap())
            .await?;
        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
    }
    async fn show_procedure(
        &self, name: &str, _arg: Option<UdfOrPrecedureExtraParameter>,
    ) -> anyhow::Result<ProcedureDetail> {
        let mut conn = self.get_conn().await?;
        let (def, metas) = FunctionOrProcedure::Procedure
            .query_details(conn.deref_mut(), self.config.db.as_ref().unwrap(), name)
            .await?;

        anyhow::Ok(ProcedureDetail::new(def, metas))
    }
    async fn drop_procedure(
        &self, name: &str, _arg: Option<UdfOrPrecedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let sql = format!("DROP PROCEDURE {name}");
        let mut conn = self.get_conn().await?;
        conn.execute(sqlx::raw_sql(&sql)).await?;
        anyhow::Ok(())
    }

    async fn show_udf(&self, name: &str) -> anyhow::Result<UdfDetail> {
        let mut conn = self.get_conn().await?;
        let (def, metas) = FunctionOrProcedure::Function
            .query_details(conn.deref_mut(), self.config.db.as_ref().unwrap(), name)
            .await?;

        anyhow::Ok(UdfDetail::new(def, metas))
    }

    async fn drop_udf(
        &self, name: &str, _extra: Option<UdfOrPrecedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let sql = format!("DROP FUNCTION {name}");
        let mut conn = self.get_conn().await?;
        conn.execute(sqlx::raw_sql(&sql)).await?;
        anyhow::Ok(())
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let mut conn = self.get_conn().await?;

        let ret = FunctionOrProcedure::Function
            .query_list(conn.deref_mut(), self.config.db.as_ref().unwrap())
            .await?;
        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
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

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        if self.table_exists(dup).await? {
            anyhow::bail!(format!("table '{}' already exists", dup));
        }
        let sql = format!("CREATE TABLE {dup} AS SELECT * FROM {table}");
        let mut conn = self.get_conn().await?;
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let sql = format!("RENAME TABLE {old} TO {new}");
        let mut conn = self.get_conn().await?;
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn show_trigger(&self, _table: &str, name: &str) -> anyhow::Result<TriggerDetail> {
        let mut conn = self.get_conn().await?;

        let (event, timing, sql, metas) = query_trigger_detail(&mut conn, name).await?;
        let mut d = TriggerDetail::new(timing, event, sql, None);
        d.with_metas(metas);
        anyhow::Ok(d)
    }
    async fn drop_trigger(&self, _table: &str, name: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
        DROP TRIGGER {name}
        "#
        );
        let mut conn = self.get_conn().await?;

        conn.execute(sqlx::raw_sql(&sql)).await?;

        anyhow::Ok(())
    }

    async fn list_trigger(&self, table_name: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
            SELECT TRIGGER_NAME
                FROM INFORMATION_SCHEMA.TRIGGERS
                WHERE EVENT_OBJECT_TABLE = '{table_name}'
                AND TRIGGER_SCHEMA = DATABASE();
        "#
        );
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await?;

        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            ret.push(name);
        }
        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
    }

    async fn export_table(
        &self, table_name: &str, kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::CSV {
                include_header,
                delimiter,
            } => {
                self.export_table_as_csv(table_name, include_header, delimiter, sender)
                    .await
            }
            ExportKind::Json { pretty } => {
                self.export_table_as_json(table_name, pretty, sender).await
            }
            ExportKind::JsonLine => self.export_table_as_json_line(table_name, sender).await,
            ExportKind::Sql {
                include_table_schema,
            } => {
                self.export_table_as_sql(table_name, include_table_schema, sender)
                    .await
            }
        }
    }

    fn read_only(&self) -> bool {
        self.config.read_only
    }

    async fn create_table_and_index_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let mut conn = self.get_conn().await?;
        let row = sqlx::query(format!("SHOW CREATE TABLE {table_name};").as_str())
            .fetch_one(conn.deref_mut())
            .await?;

        let s: String = row.try_get(1)?;

        anyhow::Ok(CreateTableAndIndexSql {
            create_table: s,

            indexes: vec![],
        })
    }

    async fn create_view_sql(&self, view_name: &str) -> anyhow::Result<String> {
        let mut conn = self.get_conn().await?;
        let row = sqlx::query(format!("SHOW CREATE VIEW {view_name};").as_str())
            .fetch_one(conn.deref_mut())
            .await?;

        let s: String = row.try_get(1)?;

        anyhow::Ok(s)
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let sql = gen_select_sql(view_name, where_str, order_by, page, page_size);
        let mut conn = self.get_conn().await?;
        let total = self.query_count(view_name, where_str).await?;

        let rows = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await?;

        let data = encode_rows(&rows)?;
        let fields = get_rows_types(&rows);
        anyhow::Ok(TableListResp::new(fields, total, data, sql))
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.can_write()?;

        let mut conn = self.get_conn().await?;
        let sql = format!("DROP VIEW {view_name}");
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let sql = format!(
            "DROP TABLE {table} {}",
            if cascade { " CASCADE " } else { "" }
        );
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn list_view(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_view_impl(self.config.db.as_ref().unwrap()).await
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_columns(table_name).await?;
        let sql =
            (self as &dyn crate::db::SchemaTrait).delete_sql(None, table_name, &fields, &wheres)?;
        let mut conn = self.get_conn().await?;
        let r = sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: r.rows_affected(),
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let s = format!(
            r#"
            TRUNCATE TABLE {table_name};
        "#
        );
        let mut conn = self.get_conn().await?;
        sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    fn put_value_in_sql(&self, f: &Field, value: Option<&str>) -> anyhow::Result<String> {
        let default_str = value.map(|x| x.to_string()).unwrap_or("null".to_string());
        let single_quote_str = value
            .map(|x| format!("'{}'", x))
            .unwrap_or("null".to_string());
        let ty = f.typ.to_uppercase();

        let v = match ty.as_str() {
            "TINYINT" => default_str,
            "BOOLEAN" | "BOOL" => default_str,
            "SMALLINT" => default_str,
            "INT" => default_str,
            "BIGINT" => default_str,
            "TINYINT UNSIGNED" => default_str,
            "SMALLINT UNSIGNED" => default_str,
            "INT UNSIGNED" => default_str,
            "BIGINT UNSIGNED" => default_str,
            "FLOAT" => default_str,
            "DOUBLE" => default_str,
            "VARCHAR" | "CHAR" | "TEXT" => single_quote_str,
            _ if ty.starts_with("VARCHAR") || ty.starts_with("CHAR") || ty.starts_with("TEXT") => {
                single_quote_str
            }
            "VARBINARY" | "BINARY" | "BLOB" => default_str,
            "TIMESTAMP" => single_quote_str,
            "DATETIME" => single_quote_str,
            "DATE" => single_quote_str,
            "TIME" => single_quote_str,
            _ => {
                log::error!("mysql type '{}' not handled", ty);
                default_str
            }
        };

        log::trace!("type '{ty}' encode '{:?}' as #{v}#", value);
        anyhow::Ok(v)
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_columns(table_name).await?;
        let s = (self as &dyn crate::db::SchemaTrait).insert_sql(None, table_name, &fs, &values)?;

        let mut conn = self.get_conn().await?;

        let r = sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql: s,
            changes: r.rows_affected(),
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_columns(table_name).await?;
        let s = (self as &dyn crate::db::SchemaTrait)
            .update_sql(None, table_name, &fs, &values, &wheres)?;
        let mut conn = self.get_conn().await?;

        let r = sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql: s,
            changes: r.rows_affected(),
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.config.db.as_ref().map(|x| x.as_str()).unwrap())
            .await
    }

    async fn show_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        // 执行 DESCRIBE users; 或 DESC users;
        // 命令将返回描述表结构的结果集。结果集包含以下列信息：
        //
        // Field：列的名称。
        // Type：列的数据类型。
        // Null：指示列是否允许为空值（YES 或 NO）。
        // Key：指示列是否是索引列，以及索引的类型。
        // Default：列的默认值。
        // Extra：显示附加信息，如是否自增、是否使用了自定义规则等。

        // +----------+--------------+------+-----+-------------------+-----------------------------+
        // | Field    | Type         | Null | Key | Default           | Extra                       |
        // +----------+--------------+------+-----+-------------------+-----------------------------+
        // | id       | int(11)      | NO   | PRI | NULL              | auto_increment              |
        // | username | varchar(50)  | NO   |     | NULL              |                             |
        // | password | varchar(100) | NO   |     | NULL              |                             |
        // +----------+--------------+------+-----+-------------------+-----------------------------+

        let s = format!("describe {}", table_name);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(s.as_str()).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name = match r.try_get::<'_, String, _>(0) {
                Ok(x) => x,
                Err(_) => match r.try_get::<'_, Vec<u8>, _>(0) {
                    Ok(x) => String::from_utf8_lossy(x.as_slice()).to_string(),
                    Err(_err) => {
                        anyhow::bail!("can get column as string or vec<u8>")
                    }
                },
            };
            let ty: String = r.try_get(1)?;
            let null: String = r.try_get(2)?;
            let key: Option<String> = r.try_get(3)?;
            let default_value: Option<String> = r.try_get(4)?;
            let extra: String = r.try_get(5)?;

            ret.push(Field::new(
                name,
                ty,
                null != "NO",
                default_value,
                key.map_or(false, |x| x == "PRI"),
                extra == "auto_increment",
            ));
        }
        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let total = self.query_count(table_name, where_str.clone()).await?;
        let sql = gen_select_sql(table_name, where_str, order_by, page, page_size);

        let mut conn = self.get_conn().await?;
        let row = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await?;
        let data = encode_rows(&row)?;
        let fields = self.show_columns(table_name).await?;
        anyhow::Ok(TableListResp::new(fields, total, data, sql))
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let sql = format!("SHOW INDEX FROM {table_name}");
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut map: HashMap<String, Index> = HashMap::new();
        for r in rows.iter() {
            let unique: Option<i32> = r.get(1);
            let name: String = r.get(2);
            let column_name = r.get(4);
            if let Some(x) = map.get_mut(&name) {
                x.on.push(column_name);
            } else {
                map.insert(
                    name.clone(),
                    Index::new(
                        name.clone(),
                        if name == "PRIMARY" {
                            IndexKind::Primary
                        } else if unique.map(|x| x != 0).unwrap_or_default() {
                            IndexKind::Unique
                        } else {
                            IndexKind::Normal
                        },
                        vec![column_name],
                    ),
                );
            }
        }
        anyhow::Ok(map.into_iter().map(|x| x.1).collect())
    }
}

fn encode_rows(rows: &Vec<MySqlRow>) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let mut ret = Vec::with_capacity(rows.len());
    for r in rows.iter() {
        let mut values = Vec::with_capacity(r.len());
        for c in 0..r.len() {
            let col = r.column(c);
            let ty = col.type_info().name();

            let v = match ty.to_uppercase().as_str() {
                "BOOLEAN" | "BOOL" => {
                    let value: Option<bool> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "TINYINT" => {
                    let value: Option<i8> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "SMALLINT" => {
                    let value: Option<i16> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "INT" => {
                    let value: Option<i32> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "BIGINT" => {
                    let value: Option<i64> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "TINYINT UNSIGNED" => {
                    let value: Option<u8> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "SMALLINT UNSIGNED" => {
                    let value: Option<u16> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "INT UNSIGNED" => {
                    let value: Option<u32> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "BIGINT UNSIGNED" => {
                    let value: Option<u64> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "FLOAT" => {
                    let value: Option<f32> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "DOUBLE" => {
                    let value: Option<f64> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "VARCHAR" | "CHAR" | "TEXT" => {
                    let value: Option<String> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                _ if ty.starts_with("VARCHAR")
                    || ty.starts_with("CHAR")
                    || ty.starts_with("TEXT") =>
                {
                    let value: Option<String> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "VARBINARY" | "BINARY" | "BLOB" => {
                    let value: Option<Vec<u8>> = r.get(c);
                    value.map(|x| format!("0x{}", hex::encode_upper(x.as_slice())))
                }
                "TIMESTAMP" => {
                    let value: Option<chrono::DateTime<Utc>> = r.get(c);
                    value.map(|x| x.naive_utc().to_string())
                }
                "DATETIME" => {
                    let value: Option<chrono::NaiveDateTime> = r.get(c);
                    value.map(|x| x.to_string())
                }
                "DATE" => {
                    let value: Option<chrono::NaiveDate> = r.get(c);
                    value.map(|x| x.to_string())
                }
                "TIME" => {
                    let value: Option<chrono::NaiveTime> = r.get(c);
                    value.map(|x| x.to_string())
                }

                _ => anyhow::bail!(format!("type '{ty}' not supported")),
            };
            values.push(v);
        }
        ret.push(values);
    }
    anyhow::Ok(ret)
}

impl MysqlDB {
    pub async fn new(config: MysqlDbConfig) -> anyhow::Result<Self> {
        log::info!("mysql config:{:?}", config);
        let mut opts = sqlx::mysql::MySqlConnectOptions::new();
        opts = opts
            .host(&config.host)
            .port(config.port)
            .username(&config.username);
        if let Some(pass) = &config.password {
            opts = opts.password(&pass);
        }
        if let Some(db) = &config.db {
            opts = opts.database(&db);
        }
        let pool = sqlx::mysql::MySqlPool::connect_with(opts).await?;

        anyhow::Ok(Self {
            pool,
            config,
            query_connection: Default::default(),
        })
    }

    async fn query_count(&self, name: &str, where_str: Option<&str>) -> anyhow::Result<usize> {
        let mut conn = self.get_conn().await?;
        let mut s = format!("SELECT count(1) FROM {name} ");
        if let Some(where_str) = where_str {
            s.push_str(format!(" where {where_str}").as_str());
        }
        let row = sqlx::query(s.as_str()).fetch_one(conn.deref_mut()).await?;
        let count: i64 = row.get(0);
        anyhow::Ok(count as usize)
    }

    pub async fn get_conn(&self) -> anyhow::Result<PoolConnection<sqlx::MySql>> {
        let conn = self.pool.acquire().await?;
        return anyhow::Ok(conn);
    }
}

#[async_trait::async_trait]
impl DBTrait for MysqlDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let mut conn: PoolConnection<sqlx::MySql> = self.get_conn().await?;
        let rows = sqlx::query("show databases")
            .fetch_all(conn.deref_mut())
            .await?;

        let mut ret = Vec::new();
        for r in rows.iter() {
            let name: String = r.get(0);
            if name.eq_ignore_ascii_case("information_schema")
                || name.eq_ignore_ascii_case("mysql")
                || name.eq_ignore_ascii_case("performance_schema")
            {
                continue;
            }
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut config = self.config.clone();
        config.db = Some(name.to_owned());
        let x = Self::new(config).await?;
        Ok(Arc::new(x))
    }
}

#[async_trait::async_trait]
impl QuerySql for MysqlDB {
    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        let conn = self.get_conn().await?;
        self.query_connection.insert(id, Arc::new(Mutex::new(conn)));
        anyhow::Ok(id)
    }
    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        is_dangerous_sql_by_parser(sql, &sqlparser::dialect::MySqlDialect {})
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.query_connection.remove(&id);

        let conn = self.get_conn().await?;
        self.query_connection.insert(id, Arc::new(Mutex::new(conn)));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        QueryLog::remove(&id);

        self.query_connection.remove(&id);
        anyhow::Ok(())
    }

    async fn query_sql(
        &self, id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let mut query_log = QueryLog::get_guard(id, sql);
        let conn = self.query_connection.get(&id);
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("id '{}' have no connection", id),
        };

        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();

        let mut ret = Vec::new();
        let mut stream = sqlx::raw_sql(sql).fetch_many(conn.deref_mut());
        let mut rows = Vec::new();

        while let Some(item) = stream.next().await {
            MemoryLimit::exceed_error()?;
            let item = match item {
                Ok(x) => x,
                Err(err) => {
                    ret.push(Err(err.to_string()));
                    continue;
                }
            };
            match item {
                sqlx::Either::Left(row_aff) => {
                    if rows.len() > 0 {
                        let fields = get_rows_types(&rows);
                        let data = encode_rows(&rows)?;
                        ret.push(Ok(QueryResp::new(fields, data)));

                        rows.clear();
                    };
                    if row_aff.rows_affected() > 0 {
                        ret.push(Ok(QueryResp::row_affected(row_aff.rows_affected())));
                    }
                }
                sqlx::Either::Right(row) => {
                    rows.push(row);
                }
            }
        }
        query_log.set_success(ret.iter().all(|x| x.is_ok()));
        anyhow::Ok(ret)
    }
}

#[cfg(test)]
pub(crate) mod test_fns {
    use std::collections::HashSet;

    use super::*;
    use crate::enable_test_log;
    // this test no need clean up, we use memory sqlite db.
    pub(crate) async fn make_test_object(config: MysqlDbConfig) -> MysqlDB {
        let db = MysqlDB::new(config).await.unwrap();
        db
    }

    pub(crate) async fn test_server_metas(db: MysqlDB) {
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }

    pub(crate) async fn test_procedure(db: MysqlDB) {
        let db_trait = &db as &dyn SchemaTrait;
        let query_id = db_trait.open_connection().await.unwrap();
        let f = "test_procedure";

        let _ = db_trait
            .drop_procedure(
                f,
                Some(UdfOrPrecedureExtraParameter::PostGres {
                    parameters: "".to_string(),
                }),
            )
            .await;

        let resp = db_trait
            .query_sql(
                query_id,
                r#" 
        CREATE PROCEDURE test_procedure(
            IN parameter1 INT,
            IN parameter2 INT
        )
        BEGIN
            DECLARE total INT;

            SET total = parameter1 + parameter2;

            SELECT total;
        END;
        "#,
            )
            .await
            .unwrap();

        resp.iter().for_each(|x| assert!(x.is_ok(), "{:?}", x));

        let udfs = db_trait.list_procedure().await.unwrap();
        for udf in udfs.iter() {
            let details = db_trait.show_procedure(&udf.name, None).await.unwrap();
            println!("details:{:?}", details);
        }
    }

    pub(crate) async fn test_udf(db: MysqlDB) {
        let db_trait = &db as &dyn SchemaTrait;
        let query_id = db_trait.open_connection().await.unwrap();
        let f = "test_function";

        let _ = db_trait.drop_udf(f, None).await;

        let resp = db_trait
            .query_sql(
                query_id,
                r#" 
        CREATE FUNCTION test_function()
        RETURNS VARCHAR(255)
        DETERMINISTIC
        BEGIN
            RETURN "hello world";
        END;
        "#,
            )
            .await
            .unwrap();

        resp.iter().for_each(|x| assert!(x.is_ok(), "{:?}", x));

        let udfs = db_trait.list_udf().await.unwrap();
        for udf in udfs.iter() {
            let details = db_trait.show_udf(&udf.name).await.unwrap();
            println!("details:{:?}", details);
        }
    }

    pub(crate) async fn test_rename(db: MysqlDB) {
        let db_trait = &db as &dyn SchemaTrait;
        let query_id = db_trait.open_connection().await.unwrap();
        let _ = db_trait.drop_table("test_rename", false).await;
        let _ = db_trait.drop_table("test_rename2", false).await;
        db_trait
            .query_sql(
                query_id,
                r#"
                    CREATE TABLE test_rename (
                        id INT AUTO_INCREMENT PRIMARY KEY,
                        name VARCHAR(255) NOT NULL,
                        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                    );
                "#,
            )
            .await
            .unwrap();
        db_trait
            .rename_table("test_rename", "test_rename2")
            .await
            .unwrap();

        db_trait
            .show_table("test_rename2", None, None, 1, 100)
            .await
            .unwrap();
    }

    pub(crate) async fn test_duplicate(db: MysqlDB) {
        let db_trait = &db as &dyn SchemaTrait;
        let query_id = db_trait.open_connection().await.unwrap();
        let _ = db_trait.drop_table("test_duplicate", false).await;
        let _ = db_trait.drop_table("test_duplicate2", false).await;

        db_trait
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_duplicate (
                    id INT AUTO_INCREMENT PRIMARY KEY,
                    data VARCHAR(255) NOT NULL,
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                );
                
                INSERT INTO test_duplicate (data) VALUES
                    ('测试数据1'),
                    ('测试数据2'),
                    ('测试数据3'),
                    ('测试数据4'),
                    ('测试数据5');
                "#,
            )
            .await
            .unwrap();
        db_trait
            .duplicate_table("test_duplicate", "test_duplicate2")
            .await
            .unwrap();

        let resp = db_trait
            .show_table("test_duplicate2", None, None, 1, 100)
            .await
            .unwrap();
        assert_eq!(resp.data.len(), 5);
    }

    pub(crate) async fn test_check_conn(db: MysqlDB) {
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }

    pub(crate) async fn test_triggers(db: MysqlDB) {
        let mut _conn = db.get_conn().await.unwrap();
        let db_trait = &db as &dyn SchemaTrait;

        let _ = db_trait.drop_table("test_create_trigger", false).await;
        let _ = db_trait
            .drop_table("test_create_trigger_update", false)
            .await;

        let query_id = db_trait.open_connection().await.unwrap();
        db_trait
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_create_trigger (
                id INT AUTO_INCREMENT, -- 自动递增的ID
                data VARCHAR(255) NOT NULL, -- 存储数据的字段，最大长度255字符，不允许为空
                PRIMARY KEY (id) -- 设置id为表的主键
            );

            
            CREATE TABLE test_create_trigger_update (
                id INT AUTO_INCREMENT, -- 自动递增的ID，作为主键
                data VARCHAR(255) NOT NULL, -- 数据字段，最大长度255字符，不允许为空
                last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, -- 记录每次更新的时间，默认值为当前时间，并在每次更新时自动更新
                PRIMARY KEY (id) -- 设置id为主键
            );

            CREATE TRIGGER after_insert_test_create_trigger
            AFTER INSERT ON test_create_trigger
            FOR EACH ROW
            BEGIN
                INSERT INTO test_create_trigger_update (data)
                VALUES (NEW.data);
            END; 



            CREATE TRIGGER after_insert_test_create_trigger2
            BEFORE INSERT ON test_create_trigger
            FOR EACH ROW
            BEGIN
                INSERT INTO test_create_trigger_update (data)
                VALUES (NEW.data);
            END; 

        "#,
            )
            .await
            .unwrap();
        let table = "test_create_trigger";

        let ts = db_trait.list_trigger(&table).await.unwrap();
        for t in ts.iter() {
            let detail = db_trait.show_trigger(&table, &t.name).await.unwrap();

            println!("trigger:{} {:?}", t.name, detail);
            if t.name == "after_insert_test_create_trigger2" {
                assert_eq!(detail.timing, TriggerTiming::Before);
            } else if t.name == "after_insert_test_create_trigger" {
                assert_eq!(detail.timing, TriggerTiming::After);
            }

            db_trait.drop_trigger(table, &t.name).await.unwrap();
        }
    }

    pub(crate) async fn show_build_in_fns(db: MysqlDB) {
        let mut conn = db.get_conn().await.unwrap();

        let rows = sqlx::query("SHOW FUNCTION STATUS;")
            .fetch_all(conn.deref_mut())
            .await
            .unwrap();

        for c in rows[0].columns().iter() {
            println!("{:?} {:?}", c.name(), c.type_info().to_string());
        }

        let mut names = HashSet::new();

        for r in rows.iter() {
            let db: String = r.get(0);
            let name: String = r.get(1);
            names.insert(db);
            names.insert(name);
        }

        let names: Vec<_> = names.into_iter().map(|x| format!("\"{}\"", x)).collect();
        for (index, name) in names.iter().enumerate() {
            print!("{}", name);
            if index != names.len() - 1 {
                println!(",")
            }
        }
    }

    pub(crate) async fn test_export(db: MysqlDB) {
        // init_log();
        let table_name = "test_export";

        let mut conn = db.get_conn().await.unwrap();
        let rs = vec![
            r#"drop table if exists test_export;"#,
            r#"CREATE TABLE test_export (
                id INT AUTO_INCREMENT PRIMARY KEY,
                name VARCHAR(50),
                age INT,
                email VARCHAR(100)
            );"#,
            "INSERT INTO test_export (name, age, email) VALUES ('John', 25, 'john@example.com');",
            "INSERT INTO test_export (name, age, email) VALUES ('Jane', 30, 'jane@example.com');",
            "INSERT INTO test_export (name, age, email) VALUES ('Bob', 40, 'bob@example.com');",
        ];
        for s in rs.iter() {
            sqlx::query(*s).execute(conn.deref_mut()).await.unwrap();
        }
        let db = &db as &dyn SchemaTrait;

        {
            let s = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", s);
            let _: serde_json::Value = serde_json::from_str(s.as_str()).unwrap();
        }
        {
            let s = db
                .export_table_to_string(table_name, ExportKind::CSV {
                    include_header: true,
                    delimiter: ',',
                })
                .await
                .unwrap();
            println!("csv:{}", s);
        }
        {
            let s = db
                .export_table_to_string(table_name, ExportKind::Sql {
                    include_table_schema: false,
                })
                .await
                .unwrap();
            println!("sql:{}", s);
        }
        {
            let s = db
                .export_table_to_string(table_name, ExportKind::Sql {
                    include_table_schema: true,
                })
                .await
                .unwrap();
            println!("sql:{}", s);
        }
        {
            let s = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json:{}", s);
        }

        println!("##################################");

        {
            let s = db
                .export_query_result_to_string(
                    format!("select * from {table_name}").as_str(),
                    ExportKind::Json { pretty: true },
                )
                .await
                .unwrap();
            println!("json:{}", s);
            let _: serde_json::Value = serde_json::from_str(s.as_str()).unwrap();
        }
        {
            let s = db
                .export_query_result_to_string(
                    format!("select * from {table_name}").as_str(),
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", s);
        }

        {
            let s = db
                .export_query_result_to_string(
                    format!("select * from {table_name}").as_str(),
                    ExportKind::JsonLine,
                )
                .await
                .unwrap();
            println!("json line:{}", s);
        }
    }

    pub(crate) async fn test_index(db: MysqlDB) {
        enable_test_log();
        let table_name = "test_index";

        let mut conn = db.get_conn().await.unwrap();
        let _ = sqlx::query(&format!("drop table {table_name}"))
            .execute(conn.deref_mut())
            .await;

        sqlx::query(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    id INT PRIMARY KEY,
                    name VARCHAR(255),
                    age INT,
                    email VARCHAR(255),
                    INDEX idx_name (name),
                    UNIQUE INDEX idx_email (email),
                    FULLTEXT INDEX idx_fulltext (name, email)
                );
            "#
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        let indexes = db.show_indexes(table_name).await.unwrap();
        println!("{:?}", indexes);
    }

    pub(crate) async fn test_types1(db: MysqlDB) {
        enable_test_log();

        let table_name = "test_types1";

        let mut conn = db.get_conn().await.unwrap();
        let _ = sqlx::query(&format!("drop table {table_name}"))
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
            CREATE TABLE {table_name} (
                tinyint_col TINYINT(1),
                boolean_col BOOLEAN,
                tinyint_col2 TINYINT,
                smallint_col SMALLINT,
                int_col INT,
                bigint_col BIGINT
            );
            "#
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();
        let db = &db as &dyn crate::db::SchemaTrait;
        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert("tinyint_col".to_string(), Some("1".to_string()));
        values.insert("boolean_col".to_string(), Some("false".to_string()));
        values.insert("tinyint_col2".to_string(), Some("2".to_string()));
        values.insert("smallint_col".to_string(), Some("3".to_string()));
        values.insert("int_col".to_string(), Some("4".to_string()));
        values.insert("bigint_col".to_string(), Some("5".to_string()));

        db.insert(table_name, values).await.unwrap();

        let cols = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", cols);
    }

    pub(crate) async fn test_types2(db: MysqlDB) {
        enable_test_log();

        let table_name = "test_types2";

        let mut conn = db.get_conn().await.unwrap();
        let _ = sqlx::query(&format!("drop table {table_name}"))
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    u8_col TINYINT UNSIGNED,
                    u16_col SMALLINT UNSIGNED,
                    u32_col INT UNSIGNED,
                    u64_col BIGINT UNSIGNED,
                    f32_col FLOAT,
                    f64_col DOUBLE
                );
            "#
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();
        let db = &db as &dyn crate::db::SchemaTrait;
        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert("u8_col".to_string(), Some("1".to_string()));
        values.insert("u16_col".to_string(), Some("2".to_string()));
        values.insert("u32_col".to_string(), Some("2".to_string()));
        values.insert("u64_col".to_string(), Some("3".to_string()));
        values.insert("f32_col".to_string(), Some("4".to_string()));
        values.insert("f64_col".to_string(), Some("5".to_string()));

        db.insert(table_name, values).await.unwrap();

        let cols = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", cols);
    }

    pub(crate) async fn test_types3(db: MysqlDB) {
        enable_test_log();

        let table_name = "test_types3";

        let mut conn = db.get_conn().await.unwrap();
        let _ = sqlx::query(&format!("drop table {table_name}"))
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    varchar_col VARCHAR(255),
                    char_col CHAR(10),
                    text_col TEXT,
                    varbinary_col VARBINARY(255),
                    binary_col BINARY(10),
                    blob_col BLOB
                );

            "#
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();
        let db = &db as &dyn crate::db::SchemaTrait;
        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert("varchar_col".to_string(), Some("x".to_string()));
        values.insert("char_col".to_string(), Some("y".to_string()));
        values.insert("text_col".to_string(), Some("z".to_string()));
        values.insert("varbinary_col".to_string(), Some("0x01020304".to_string()));
        values.insert("binary_col".to_string(), Some("0x0102030405".to_string()));
        values.insert("blob_col".to_string(), Some("0x01020304".to_string()));

        db.insert(table_name, values).await.unwrap();

        let cols = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", cols);
    }

    pub(crate) async fn test_types4(db: MysqlDB) {
        let table_name = "test_types4";

        let db = &db as &dyn crate::db::SchemaTrait;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        db.query_sql(
            query_id,
            r#"
                CREATE TABLE test_types4 (
                    timestamp_column TIMESTAMP,
                    datetime_column DATETIME,
                    date_column DATE,
                    time_column TIME
                );
        "#,
        )
        .await
        .unwrap();

        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert(
            "timestamp_column".to_string(),
            Some("2024-06-29 10:00:00".to_string()),
        );
        values.insert(
            "datetime_column".to_string(),
            Some("2024-06-29 10:00:00".to_string()),
        );
        values.insert("date_column".to_string(), Some("2024-06-29".to_string()));
        values.insert("time_column".to_string(), Some("10:00:00".to_string()));

        db.insert(table_name, values).await.unwrap();

        let cols = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", cols);
    }

    pub(crate) async fn test_list_database(db: MysqlDB) {
        let db = &db as &dyn crate::db::DBTrait;

        let resp = db.list_schema().await.unwrap();
        assert_eq!(resp, vec!["sys", "test"]);
    }

    pub(crate) async fn test_query(db: MysqlDB) {
        enable_test_log();

        let db = &db as &dyn crate::db::SchemaTrait;
        let id = db.open_connection().await.unwrap();

        let ret = db.query_sql(id.clone(), "begin").await.unwrap();
        db.query_sql(id.clone(), "drop table if exists test_query ")
            .await
            .unwrap();

        db.query_sql(
            id.clone(),
            r#"
        CREATE TABLE test_query (
            id INT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(255),
            age INT
        )
        "#,
        )
        .await
        .unwrap();

        db.query_sql(
            id.clone(),
            r#"
            INSERT INTO test_query (name, age)
            VALUES
                ('John', 25),
                ('Jane', 30),
                ('Alice', 28)
        "#,
        )
        .await
        .unwrap();

        db.query_sql(id.clone(), "commit").await.unwrap();

        println!("begin ret:{:?}", ret);
    }
}

#[cfg(test)]
mod tests {
    use super::test_fns::{self, make_test_object};
    use crate::{TEST_HOST, datasource_config::MysqlDbConfig};

    pub(crate) fn make_config() -> MysqlDbConfig {
        MysqlDbConfig {
            host: TEST_HOST.to_string(),
            username: "root".to_string(),
            password: Some("root".to_string()),
            port: 3306,
            db: Some("test".to_string()),
            read_only: false,
        }
    }
    #[tokio::test]
    async fn test_duplicate() {
        test_fns::test_duplicate(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_server_metas() {
        test_fns::test_server_metas(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_procedure() {
        test_fns::test_procedure(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_index() {
        test_fns::test_index(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_check_conn() {
        test_fns::test_check_conn(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_export() {
        test_fns::test_export(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_rename() {
        test_fns::test_rename(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_udf() {
        test_fns::test_udf(make_test_object(make_config()).await).await;
    }

    #[tokio::test]
    async fn test_types1() {
        test_fns::test_types1(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types2() {
        test_fns::test_types2(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types3() {
        test_fns::test_types3(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types4() {
        test_fns::test_types4(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_triggers() {
        test_fns::test_triggers(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_list_database() {
        test_fns::test_list_database(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_query() {
        test_fns::test_query(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn show_build_in_fns() {
        test_fns::show_build_in_fns(make_test_object(make_config()).await).await;
    }
}
