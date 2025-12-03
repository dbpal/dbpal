use crate::datasource_config::MysqlDBConfig;
use crate::db::{
    AutoCompletionQueryId, BackendKind, CheckConn, ColumnType, CreateSchemaExtra,
    CreateTableAndIndexSql, DBTrait, DefAndMetas, ExportKind, ExportQueryResult, Field,
    GetBackendKind, Index, IndexKind, ListTable, MetaValue, NamedStaff, OptionMustBeSomeErr,
    POOL_MAX_CONNECTIONS, ProcedureDetail, QueryEvent, QueryEventEmitter, QueryResp, QuerySql,
    ReadOnlyTrait, SchemaTrait, ServerMetas, ShowTableParameter, SqlBuilder, SqlEvent,
    SqlExecuteRet, TimeElapsed, TriggerDetail, TriggerEvent, TriggerTiming,
    UdfOrProcedureExtraParameter, XlsxEdit, create_html_table_data, create_html_table_header,
    escape_sql_str, export_query_as_xlsx_send_data,
};
use crate::helper::create_csv;
use crate::language_server::LexerCompletionTrait;
use chrono::Utc;
use dashmap::DashMap;
use futures_util::{StreamExt, TryStreamExt};
use sqlx::mysql::MySqlRow;
use sqlx::pool::{PoolConnection, PoolOptions};
use sqlx::types::BigDecimal;
use sqlx::{AssertSqlSafe, Column, Executor, MySqlConnection, Row, SqlSafeStr, TypeInfo};
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct MysqlDB {
    pool: sqlx::MySqlPool,
    pub(crate) config: MysqlDBConfig,
    pub(crate) query_connection: Arc<DashMap<String, Arc<Mutex<PoolConnection<sqlx::MySql>>>>>,
    pub(crate) key_words_cache: Arc<Mutex<Vec<String>>>,
    pub(crate) completion_id: AutoCompletionQueryId,
}

fn get_row_types(row: &MySqlRow) -> Vec<Field> {
    let mut ret = Vec::with_capacity(row.columns().len());
    for c in row.columns().iter() {
        let column = c;
        ret.push(Field {
            field: column.name().to_string(),
            data_type: column.type_info().to_string(),
            ..Default::default()
        });
    }
    ret
}

impl ReadOnlyTrait for MysqlDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}
fn gen_select_sql(
    table_name: &str, fields: Option<String>, where_str: Option<&str>, order_by: Option<&str>,
    page: usize, page_size: usize,
) -> String {
    let mut sql = format!(
        r#"SELECT {} FROM `{table_name}`"#,
        if let Some(fields) = fields {
            fields
        } else {
            "*".to_owned()
        }
    );
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

fn encode_row_as_json(row: &MySqlRow, try_blob_to_str: bool) -> anyhow::Result<serde_json::Value> {
    let mut values = serde_json::Map::new();
    let null_value = serde_json::Value::Null;

    for c in 0..row.columns().len() {
        let col = row.column(c);
        let ty = col.type_info().name();
        let ty = ty.to_uppercase();
        let v = match ty.as_str() {
            "BOOLEAN" | "BOOL" => {
                let value: Option<bool> = row.try_get(c)?;
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "TINYINT" => {
                let value: Option<i8> = row.try_get(c)?;
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "SMALLINT" => {
                let value: Option<i16> = row.try_get(c)?;
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "INT" => {
                let value: Option<i32> = row.try_get(c)?;
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "BIGINT" => {
                let value: Option<i64> = row.try_get(c)?;
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "TINYINT UNSIGNED" => {
                let value: Option<u8> = row.try_get(c)?;
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "SMALLINT UNSIGNED" => {
                let value: Option<u16> = row.try_get(c)?;
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "INT UNSIGNED" => {
                let value: Option<u32> = row.try_get(c)?;
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "BIGINT UNSIGNED" => {
                let value: Option<u64> = row.try_get(c)?;
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "FLOAT" => {
                let value: Option<f32> = row.try_get(c)?;
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "DOUBLE" => {
                let value: Option<f64> = row.try_get(c)?;
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }

            _ if type_is_string(&ty) => {
                let value: Option<String> = row.try_get(c)?;
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "ENUM" => {
                let value: Option<String> = row.try_get(c)?;
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }

            _ if type_is_blob(&ty) => {
                let string_value = row.try_get::<String, _>(c);

                match (string_value, try_blob_to_str) {
                    (Ok(v), true) => serde_json::Value::from(v),
                    _ => {
                        let value: Option<Vec<u8>> = row.try_get(c)?;
                        value
                            .map(|x| {
                                x.into_iter()
                                    .map(serde_json::Value::from)
                                    .collect::<Vec<_>>()
                            })
                            .map(serde_json::Value::Array)
                            .unwrap_or(null_value.clone())
                    }
                }
            }
            "TIMESTAMP" => {
                let value: Option<chrono::DateTime<Utc>> = row.try_get(c)?;
                value
                    .map(|x| x.naive_utc().to_string())
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "DATETIME" => {
                let value: Option<chrono::NaiveDateTime> = row.try_get(c)?;
                value
                    .map(|x| x.to_string())
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "DATE" => {
                let value: Option<chrono::NaiveDate> = row.try_get(c)?;
                value
                    .map(|x| x.to_string())
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "TIME" => {
                let value: Option<chrono::NaiveTime> = row.try_get(c)?;
                value
                    .map(|x| x.to_string())
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "JSON" => {
                let value: Option<serde_json::Value> = row.try_get(c)?;
                value.unwrap_or(null_value.clone())
            }
            "DECIMAL" => {
                let value: Option<BigDecimal> = row.try_get(c)?;
                value
                    .map(|x| x.to_string())
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "NULL" => serde_json::Value::Null,
            _ => anyhow::bail!(format!("type '{ty}' not supported")),
        };
        values.insert(row.column(c).name().to_string(), v);
    }

    anyhow::Ok(serde_json::Value::Object(values))
}

fn type_is_blob(ty: &str) -> bool {
    matches!(
        ty,
        "VARBINARY" | "BINARY" | "BLOB" | "LONGBLOB" | "TINYBLOB" | "MEDIUMBLOB"
    )
}

fn type_is_string(ty: &str) -> bool {
    match ty {
        "VARCHAR" | "CHAR" | "TEXT" | "TINYTEXT" | "MEDIUMTEXT" | "LONGTEXT" => true,
        _ if ty.starts_with("VARCHAR") || ty.starts_with("CHAR") || ty.starts_with("TEXT") => true,
        _ => false,
    }
}

#[async_trait::async_trait]
impl CheckConn for MysqlDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql("select 1")
            .fetch_all(conn.deref_mut())
            .await?;
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
        let cols = SchemaTrait::show_table_columns(self, table_name).await?;
        let sql = format!(r#"SELECT * from `{}`"#, table_name);
        log::info!("sql:{}", sql);
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch(conn.deref_mut())
            .try_chunks(100);
        while let Some(rows) = stream.next().await {
            let rows = rows?;

            let data = encode_rows(&rows, true)?;
            for d in data.into_iter() {
                let mut m = HashMap::new();
                for (d, f) in d.into_iter().zip(cols.iter()) {
                    m.insert(f.field.clone(), d);
                }

                let insert_sql = self.insert_sql(None, table_name, &cols, &m)?;
                sender.send(Cow::Owned(insert_sql)).await?;
                sender.send(";\n".into()).await?;
            }
        }
        anyhow::Ok(())
    }

    async fn get_conn_by_query_id(
        &self, query_id: Option<&str>,
    ) -> anyhow::Result<Arc<Mutex<PoolConnection<sqlx::MySql>>>> {
        match query_id {
            Some(id) => match self.query_connection.get(&id.to_string()) {
                Some(x) => anyhow::Ok(x.clone()),
                None => anyhow::bail!("should be able to get connection"),
            },
            None => {
                let conn = self.get_conn().await?;
                anyhow::Ok(Arc::new(Mutex::new(conn)))
            }
        }
    }
    async fn export_query_result_as_json_line(
        &self, query_id: Option<&str>, sql: &str,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn_by_query_id(query_id).await?;
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql)).fetch(conn.deref_mut());
        while let Some(row) = stream.next().await {
            let row = row?;
            let value = encode_row_as_json(&row, true)?;

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
        let sql = format!("SELECT * from `{}`", table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_json_line(None, sql.as_str(), sender)
            .await
    }

    async fn export_query_result_as_json(
        &self, query_id: Option<&str>, sql: &str, pretty: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn_by_query_id(query_id).await?;
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();

        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql)).fetch(conn.deref_mut());

        sender.send("[".into()).await?;
        let mut first = true;
        while let Some(row) = stream.next().await {
            let row = row?;
            let value = encode_row_as_json(&row, true)?;
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
        let sql = format!("SELECT * from `{}`", table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_json(None, sql.as_str(), pretty, sender)
            .await
    }

    async fn export_query_result_as_csv(
        &self, query_id: Option<&str>, sql: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn_by_query_id(query_id).await?;
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let mut first = true;

        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql))
            .fetch(conn.deref_mut())
            .try_chunks(100);

        while let Some(rows) = stream.next().await {
            let rows = rows?;

            if first && !rows.is_empty() && include_header {
                let fs = rows
                    .first()
                    .expect("row is not empty")
                    .columns()
                    .iter()
                    .map(|x| x.name().to_string())
                    .map(Some)
                    .collect::<Vec<_>>();
                sender
                    .send(Cow::Owned(create_csv(&[fs], delimiter)))
                    .await?;
                first = false
            }

            let data = encode_rows(&rows, true)?;
            sender
                .send(Cow::Owned(create_csv(&data, delimiter)))
                .await?;
        }
        anyhow::Ok(())
    }

    async fn export_query_result_as_html(
        &self, query_id: Option<&str>, sql: &str, include_header: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn_by_query_id(query_id).await?;
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let mut first = true;
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql))
            .fetch(conn.deref_mut())
            .try_chunks(100);

        while let Some(rows) = stream.next().await {
            let rows = rows?;
            if first && !rows.is_empty() && include_header {
                let fs = rows
                    .first()
                    .expect("rows is not empty ")
                    .columns()
                    .iter()
                    .map(|x| x.name().to_string())
                    .collect::<Vec<_>>();
                sender
                    .send(Cow::Owned(create_html_table_header(&fs)))
                    .await?;
                first = false
            }
            let data = encode_rows(&rows, true)?;
            sender
                .send(Cow::Owned(create_html_table_data(&data)))
                .await?;
        }
        anyhow::Ok(())
    }

    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from `{}`", table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_csv(None, sql.as_str(), include_header, delimiter, sender)
            .await
    }
    async fn export_table_as_xlsx(
        &self, table_name: &str, include_header: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from `{}`", table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_xlsx(None, sql.as_str(), include_header, sender)
            .await
    }
    async fn export_table_as_html(
        &self, table_name: &str, include_header: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from `{}`", table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_html(None, sql.as_str(), include_header, sender)
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
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

async fn query_trigger_detail(
    conn: &mut MySqlConnection, name: &str,
) -> anyhow::Result<(TriggerEvent, TriggerTiming, Vec<MetaValue>)> {
    let sql = format!("SHOW TRIGGERS WHERE `Trigger`='{name}'");
    log::info!("sql:{}", sql);
    let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
        .fetch_one(conn)
        .await?;
    let event: String = row.try_get_unchecked(1)?;

    let timing: String = row.try_get_unchecked(4)?;
    let create_time: chrono::DateTime<Utc> = row.try_get(5)?;
    let sql_mode: String = row.try_get_unchecked(6)?;
    let create_time = create_time.naive_utc();
    let timing = if timing == "BEFORE" {
        TriggerTiming::Before
    } else {
        TriggerTiming::After
    };

    anyhow::Ok((
        TriggerEvent::try_from(event.as_str())?,
        timing,
        vec![
            MetaValue::new("created", create_time.to_string()),
            MetaValue::new("sql_mode", sql_mode),
        ],
    ))
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
            self.routine_type()
        );
        log::info!("sql:{}", sql);

        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn)
            .await?;
        let mut ret = Vec::new();

        for r in rows.iter() {
            let name: String = r.get(0);
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
    fn routine_type(self) -> &'static str {
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
        "#;
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql)).fetch_many(conn.deref_mut());
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
                    metas.push(MetaValue::new("now", now.naive_utc().to_string()));
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
                _ => {
                    anyhow::bail!("count the row number");
                }
            }
        }

        anyhow::Ok(metas)
    }
}

impl GetBackendKind for MysqlDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::Mysql
    }
}

fn schema_is_information_schema(x: &str) -> bool {
    x.eq_ignore_ascii_case("information_schema")
}

impl MysqlDB {
    pub(crate) async fn list_table_impl(&self, schema: &str) -> anyhow::Result<Vec<ListTable>> {
        let mut conn = self.get_conn().await?;
        let s = format!(
            r#"SELECT table_name ,data_length
        FROM information_schema.tables
        WHERE table_schema = '{}' AND  table_type = 'BASE TABLE'  ORDER BY  table_name;"#,
            schema
        );
        let rows = sqlx::raw_sql(AssertSqlSafe(s.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let table_name: String = r.try_get_unchecked(0)?;
            // tidb return data_length as `BIGINT` no UNSIGNED
            let size: Option<u64> = match r.try_get::<'_, u64, _>(1) {
                Ok(x) => Some(x),
                Err(_) => match r.try_get::<'_, i64, _>(1) {
                    Ok(x) => Some(x as u64),
                    Err(_) => {
                        anyhow::bail!("extract data_length failed:{:?}", r)
                    }
                },
            };

            ret.push(ListTable::new(table_name, size));
        }
        anyhow::Ok(ret)
    }

    /*
        在 information_schema.tables 这张元数据表里，
        列 table_type 用来标识“这张表到底是什么类型”。MySQL 目前（8.0 及以后）只会出现下面 3 种取值：

            BASE TABLE
            普通表（包括系统表、用户表、分区表、临时表中的内部临时表等），一句话：磁盘上真实存在的表。
            VIEW
            逻辑视图。
            SYSTEM VIEW
            从 MySQL 8.0 开始，information_schema 里的所有“表” 官方都归到这一类，因为它们并不是磁盘文件，而是运行时动态生成的内存/性能_schema 视图。
            在 5.7 及更早版本，这些对象仍被标为 VIEW；8.0 以后为了与普通用户 VIEW 区分，统一改成 SYSTEM VIEW。
    */

    pub(crate) async fn list_view_impl(&self, schema: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"
        SELECT table_name FROM information_schema.tables WHERE table_type = '{}' AND table_schema = '{}';
        "#,
            if schema_is_information_schema(schema) {
                "SYSTEM VIEW"
            } else {
                "VIEW"
            },
            schema
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get_unchecked(0)?;
            ret.push(ListTable::from(name));
        }
        anyhow::Ok(ret)
    }
}

impl std::fmt::Debug for MysqlDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mysql on:{:?}", self.config)
    }
}

impl MysqlDB {
    pub(crate) async fn list_event_impl(&self, schema: &str) -> anyhow::Result<Vec<SqlEvent>> {
        let sql = format!(
            r#"
            SELECT EVENT_NAME,
        STATUS,          -- ENABLED / DISABLED / SLAVESIDE_DISABLED
        EVENT_TYPE,      -- ONE TIME / RECURRING
        INTERVAL_VALUE,
        INTERVAL_FIELD,  -- 周期频率（如 1 DAY）
        LAST_EXECUTED
        FROM   information_schema.EVENTS
        WHERE  EVENT_SCHEMA = '{schema}'  -- 这里换成你的库名
        ORDER  BY EVENT_NAME;
        "#
        );
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());

        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            let status: Option<String> = r.try_get_unchecked(1)?;
            let event_type: Option<String> = r.try_get(2)?;
            let interval_value: Option<String> = r.try_get(3)?;
            let interval_field: Option<String> = r.try_get(4)?;
            let last_executed: Option<chrono::NaiveDateTime> = r.try_get(5)?;

            let create_sql = {
                let sql = format!(r#"SHOW CREATE EVENT `{}`.`{}`"#, schema, name);
                let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                    .fetch_one(conn.deref_mut())
                    .await?;
                row.try_get::<'_, String, _>(3)?
            };
            ret.push(SqlEvent {
                name,
                sql: create_sql,
                metas: vec![
                    MetaValue::new("status", status),
                    MetaValue::new("event_type", event_type),
                    MetaValue::new(
                        "interval",
                        format!(
                            "{}{}",
                            interval_value.unwrap_or_default(),
                            interval_field.unwrap_or_default()
                        ),
                    ),
                    MetaValue::new("last_executed", last_executed.map(|x| x.to_string())),
                ],
            });
        }
        anyhow::Ok(ret)
    }
}
#[async_trait::async_trait]

impl SchemaTrait for MysqlDB {
    async fn list_event(&self) -> anyhow::Result<Vec<SqlEvent>> {
        self.list_event_impl(self.config.schema_str()?).await
    }
    async fn drop_event(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP EVENT `{}`"#, name);
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    fn list_sql_current_values(&self) -> &'static [&'static str] {
        <Self as SqlBuilder>::CURRENT_VALUES
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
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn drop_index(&self, table_name: &str, index: &str) -> anyhow::Result<()> {
        let sql = format!(r#"ALTER TABLE `{table_name}` DROP INDEX `{index}`"#);
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn add_table_column(
        &self, table: &str, name: &str, col: ColumnType,
    ) -> anyhow::Result<()> {
        let mut sql = format!(
            "ALTER TABLE `{table}` ADD COLUMN `{name}` {} {} {}",
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
        if let Some(a) = col.after.as_ref() {
            sql.push_str(format!(" after {}", a).as_str());
        }
        log::info!("sql:{}", sql);

        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        LexerCompletionTrait::get_types(self).await
    }

    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        if old_name != new_name {
            let sql = format!(
                r#"
            ALTER TABLE `{table}`
                RENAME COLUMN `{old_name}` TO `{new_name}`;
            "#
            );
            log::info!("sql:{}", sql);
            sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .execute(conn.deref_mut())
                .await?;
        }
        if let Some(typ) = typ.as_ref() {
            let sql = format!(
                r#"
            ALTER TABLE `{table}`
                MODIFY COLUMN `{new_name}` {} {};
            "#,
                &typ.typ,
                if let Some(null) = typ.nullness.as_ref() {
                    null.as_str()
                } else {
                    ""
                }
            );
            log::info!("sql:{}", sql);
            sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .execute(conn.deref_mut())
                .await?;
        }
        anyhow::Ok(())
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
            ALTER TABLE `{table}` DROP COLUMN `{col}`;
        "#
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        use sqlx::Statement;
        let mut conn = self.get_conn().await?;
        let sql = format!("select * from `{view_name}` limit 1");
        log::info!("sql:{}", sql);
        let s = conn
            .deref_mut()
            .prepare_with(AssertSqlSafe(sql.as_str()).into_sql_str(), &[])
            .await?;
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
            .query_list(conn.deref_mut(), self.config.schema_str()?)
            .await?;
        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
    }

    async fn show_procedure(
        &self, name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<ProcedureDetail> {
        let mut conn = self.get_conn().await?;

        let sql = format!("SHOW CREATE PROCEDURE {name};");

        let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_one(conn.deref_mut())
            .await?;
        let sql_mode: Option<String> = row.try_get(1)?;
        let sql: String = row.try_get(2)?;
        anyhow::Ok(ProcedureDetail::new(
            sql,
            vec![MetaValue::new("sql_mode", sql_mode)],
        ))
    }

    async fn drop_procedure(
        &self, name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let sql = format!("DROP PROCEDURE `{name}`");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        conn.execute(sqlx::raw_sql(AssertSqlSafe(sql.as_str())))
            .await?;
        anyhow::Ok(())
    }

    async fn show_udf(
        &self, name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<DefAndMetas> {
        let mut conn = self.get_conn().await?;

        let sql = format!("SHOW CREATE FUNCTION {name};");

        let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_one(conn.deref_mut())
            .await?;
        let sql_mode: Option<String> = row.try_get(1)?;
        let sql: String = row.try_get(2)?;
        anyhow::Ok(DefAndMetas::new(
            sql,
            vec![MetaValue::new("sql_mode", sql_mode)],
        ))
    }

    async fn drop_udf(
        &self, name: &str, _extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let sql = format!("DROP FUNCTION `{name}`");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        conn.execute(sqlx::raw_sql(AssertSqlSafe(sql.as_str())))
            .await?;
        anyhow::Ok(())
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let mut conn = self.get_conn().await?;

        let ret = FunctionOrProcedure::Function
            .query_list(conn.deref_mut(), self.config.schema_str()?)
            .await?;

        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        if self.table_exists(dup).await? {
            anyhow::bail!(format!("table '{}' already exists", dup));
        }
        let sql = format!("CREATE TABLE `{dup}` AS SELECT * FROM {table}");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn rename_index(
        &self, table_name: &str, oldname: &str, newname: &str,
    ) -> anyhow::Result<()> {
        let sql = format!("ALTER TABLE `{table_name}` RENAME INDEX `{oldname}` TO `{newname}`");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let sql = format!("RENAME TABLE `{old}` TO `{new}`");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn show_trigger(&self, _table: &str, name: &str) -> anyhow::Result<TriggerDetail> {
        let mut conn = self.get_conn().await?;
        let (event, timing, metas) = query_trigger_detail(&mut conn, name).await?;

        let create_stmt = {
            let sql = format!("show CREATE TRIGGER `{}`", name);
            let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .fetch_one(conn.deref_mut())
                .await?;

            row.try_get::<String, _>(2)?
        };

        let mut d = TriggerDetail::new(name, timing, event, create_stmt, None);
        d.with_metas(metas);
        anyhow::Ok(d)
    }

    async fn drop_trigger(&self, _table: &str, name: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
        DROP TRIGGER `{name}`
        "#
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        conn.execute(sqlx::raw_sql(AssertSqlSafe(sql.as_str())))
            .await?;

        anyhow::Ok(())
    }

    async fn list_trigger(&self, table_name: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
            SELECT TRIGGER_NAME
                FROM INFORMATION_SCHEMA.TRIGGERS
                WHERE EVENT_OBJECT_TABLE = '{table_name}'
                AND TRIGGER_SCHEMA = '{}';
        "#,
            self.config.schema_str()?
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;

        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            ret.push(name);
        }
        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
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
        let mut conn = self.get_conn().await?;
        let row = sqlx::raw_sql(AssertSqlSafe(
            format!("SHOW CREATE TABLE `{table_name}`;").as_str(),
        ))
        .fetch_one(conn.deref_mut())
        .await?;

        let s: String = row.try_get(1)?;

        anyhow::Ok(CreateTableAndIndexSql {
            create_table: s,
            indexes: vec![],
        })
    }

    async fn create_view_sql(&self, view_name: &str, _: bool) -> anyhow::Result<String> {
        let mut conn = self.get_conn().await?;
        let row = sqlx::raw_sql(AssertSqlSafe(
            format!("SHOW CREATE VIEW `{view_name}`;").as_str(),
        ))
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
        &self, view_name: &str, para: ShowTableParameter, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let ShowTableParameter {
            where_str,
            order_by,
            page,
            page_size,
            ..
        } = &para;
        let sql = gen_select_sql(
            view_name,
            None,
            where_str.as_deref(),
            order_by.as_deref(),
            *page,
            *page_size,
        );
        log::info!("sql:{}", sql);
        emit.send("trying to get connection...".into())?;
        let mut conn = self.get_conn().await?;
        emit.send("trying to query total amount.".into())?;
        let total = self.query_count(view_name, where_str.as_deref()).await?;
        emit.send("trying to query data.".into())?;
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql.as_str())).fetch(conn.deref_mut());
        let mut rows = vec![];
        let mut time_elapsed = TimeElapsed::new();
        let mut first = true;
        while let Some(row) = stream.next().await {
            let row = row?;
            if first {
                first = false;
                emit.send(
                    QueryResp {
                        total,
                        fields: get_row_types(&row),
                        sql: sql.clone().into(),
                        ..Default::default()
                    }
                    .into(),
                )?;
            }
            rows.push(row);
            if rows.len() >= self.query_buffer_size() || time_elapsed.elapsed() {
                emit.send(encode_rows(&std::mem::take(&mut rows), true)?.into())?;
            }
        }
        if !rows.is_empty() {
            emit.send(encode_rows(&std::mem::take(&mut rows), true)?.into())?;
        }
        anyhow::Ok(())
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.can_write()?;

        let mut conn = self.get_conn().await?;
        let sql = format!("DROP VIEW `{view_name}`");
        log::info!("sql:{}", sql);
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let sql = format!(
            "DROP TABLE `{table}` {}",
            if cascade { " CASCADE " } else { "" }
        );
        log::info!("sql:{}", sql);
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_view_impl(self.config.schema_str()?).await
    }

    async fn table_delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_table_columns(table_name).await?;
        let sql = self.delete_sql(None, table_name, &fields, &wheres)?;
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let r = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(r.rows_affected()),
        })
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let s = format!(
            r#"
            TRUNCATE TABLE `{table_name}`;
        "#
        );
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(s.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;
        let s = self.insert_sql(None, table_name, &fs, &values)?;
        let mut conn = self.get_conn().await?;
        let r = sqlx::raw_sql(AssertSqlSafe(s.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(SqlExecuteRet {
            sql: s,
            changes: Some(r.rows_affected()),
        })
    }

    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;
        let s = self.update_sql(None, table_name, &fs, &values, &wheres)?;
        let mut conn = self.get_conn().await?;

        let r = sqlx::raw_sql(AssertSqlSafe(s.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(SqlExecuteRet {
            sql: s,
            changes: Some(r.rows_affected()),
        })
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.config.schema_str()?).await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
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

        let s = format!("describe `{}`", table_name);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(s.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
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
            // due to issue https://github.com/launchbadge/sqlx/issues/3390
            // use `try_get_unchecked` to extract
            let ty: String = r.try_get_unchecked(1)?;
            let null: String = r.try_get(2)?;
            let key: Option<String> = r.try_get_unchecked(3)?;
            let default_value: Option<String> = r.try_get_unchecked(4)?;
            let extra: String = r.try_get_unchecked(5)?;

            ret.push(Field::new(
                name,
                ty,
                null != "NO",
                default_value,
                key.is_some_and(|x| x == "PRI"),
                extra == "auto_increment",
            ));
        }
        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, para: ShowTableParameter, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let ShowTableParameter {
            where_str,
            order_by,
            page,
            page_size,
            ..
        } = &para;
        emit.send("trying to query table columns.".into())?;
        let fields = self.show_table_columns(table_name).await?;
        emit.send("trying to query total amount".into())?;
        let total = self.query_count(table_name, where_str.as_deref()).await?;
        let sql = gen_select_sql(
            table_name,
            Some(self.backend_name().format_fields(Some(&fields))),
            where_str.as_deref(),
            order_by.as_deref(),
            *page,
            *page_size,
        );
        log::info!("sql:{}", sql);
        emit.send(
            QueryResp {
                fields,
                total,
                sql: sql.clone().into(),
                ..Default::default()
            }
            .into(),
        )?;
        emit.send("trying get connection.".into())?;
        let mut conn = self.get_conn().await?;
        emit.send("trying to query data".into())?;
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql.as_str())).fetch(conn.deref_mut());
        let mut rows = vec![];
        let mut time_elapsed = TimeElapsed::new();
        while let Some(row) = stream.next().await {
            let row = row?;
            rows.push(row);
            if rows.len() >= self.query_buffer_size() || time_elapsed.elapsed() {
                let d = encode_rows(&std::mem::take(&mut rows), true)?;
                emit.send(d.into())?;
            }
        }
        let d = encode_rows(&std::mem::take(&mut rows), true)?;
        emit.send(d.into())?;
        anyhow::Ok(())
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let sql = format!("SHOW INDEX FROM `{table_name}`");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut map: HashMap<String, Index> = HashMap::new();
        for r in rows.iter() {
            let non_unique: Option<i32> = r.get(1);
            let name: String = r.get(2);
            let column_name = r.get(4);
            if let Some(x) = map.get_mut(&name) {
                x.columns.push(column_name);
            } else {
                map.insert(
                    name.clone(),
                    Index::new(
                        name.clone(),
                        if name == "PRIMARY" {
                            IndexKind::Primary
                        } else if non_unique.map(|x| x == 0).unwrap_or_default() {
                            IndexKind::Unique
                        } else {
                            IndexKind::Index
                        },
                        vec![column_name],
                    ),
                );
            }
        }
        anyhow::Ok(map.into_iter().map(|x| x.1).collect())
    }
}

fn encode_rows(
    rows: &[MySqlRow], try_blob_to_str: bool,
) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let mut ret = Vec::with_capacity(rows.len());
    for r in rows.iter() {
        let mut values = Vec::with_capacity(r.len());
        for c in 0..r.len() {
            let col = r.column(c);
            let ty = col.type_info().name();
            let ty = ty.to_uppercase();
            let v = match ty.as_str() {
                "BOOLEAN" | "BOOL" => {
                    let value: Option<bool> = r.try_get(c)?;
                    value.map(|x| format!("{}", x))
                }
                "TINYINT" => {
                    let value: Option<i8> = r.try_get(c)?;
                    value.map(|x| format!("{}", x))
                }
                "SMALLINT" => {
                    let value: Option<i16> = r.try_get(c)?;
                    value.map(|x| format!("{}", x))
                }
                "INT" => {
                    let value: Option<i32> = r.try_get(c)?;
                    value.map(|x| format!("{}", x))
                }
                "BIGINT" => {
                    let value: Option<i64> = r.try_get(c)?;
                    value.map(|x| format!("{}", x))
                }
                "TINYINT UNSIGNED" => {
                    let value: Option<u8> = r.try_get(c)?;
                    value.map(|x| format!("{}", x))
                }
                "SMALLINT UNSIGNED" => {
                    let value: Option<u16> = r.try_get(c)?;
                    value.map(|x| format!("{}", x))
                }
                "INT UNSIGNED" => {
                    let value: Option<u32> = r.try_get(c)?;
                    value.map(|x| format!("{}", x))
                }
                "BIGINT UNSIGNED" => {
                    let value: Option<u64> = r.try_get(c)?;
                    value.map(|x| format!("{}", x))
                }
                "FLOAT" => {
                    let value: Option<f32> = r.try_get(c)?;
                    value.map(|x| format!("{}", x))
                }
                "DOUBLE" => {
                    let value: Option<f64> = r.try_get(c)?;
                    value.map(|x| format!("{}", x))
                }

                _ if type_is_string(&ty) => {
                    let value: Option<String> = r.try_get(c)?;
                    value.map(|x| x.to_string())
                }
                "ENUM" => {
                    let value: Option<String> = r.try_get(c)?;
                    value.map(|x| x.to_string())
                }
                _ if type_is_blob(&ty) => {
                    let x: Option<Vec<u8>> = r.try_get(c)?;
                    let str_value: sqlx::Result<Option<String>> = r.try_get(c);
                    let str_value = str_value.ok().flatten();
                    x.map(|x| match (str_value, try_blob_to_str) {
                        (Some(value), true) => value,
                        _ => {
                            format!("0x{}", hex::encode_upper(x.as_slice()))
                        }
                    })
                }
                "TIMESTAMP" => {
                    let value: Option<chrono::DateTime<Utc>> = r.try_get(c)?;
                    value.map(|x| x.naive_utc().to_string())
                }
                "DATETIME" => {
                    let value: Option<chrono::NaiveDateTime> = r.try_get(c)?;
                    value.map(|x| x.to_string())
                }
                "DATE" => {
                    let value: Option<chrono::NaiveDate> = r.try_get(c)?;
                    value.map(|x| x.to_string())
                }
                "TIME" => {
                    let value: Option<chrono::NaiveTime> = r.try_get(c)?;
                    value.map(|x| x.to_string())
                }
                "DECIMAL" => {
                    let value: Option<BigDecimal> = r.try_get(c)?;
                    value.map(|x| x.to_string())
                }
                "JSON" => {
                    let value: Option<serde_json::Value> = r.try_get(c)?;
                    value.map(|x| x.to_string())
                }
                "NULL" => None,

                _ => anyhow::bail!(format!("type '{ty}' not supported")),
            };
            values.push(v);
        }
        ret.push(values);
    }
    anyhow::Ok(ret)
}

impl MysqlDB {
    pub async fn new(config: MysqlDBConfig) -> anyhow::Result<Self> {
        let mut opts = sqlx::mysql::MySqlConnectOptions::new();
        opts = opts
            .host(&config.host)
            .port(config.port)
            .username(&config.username);
        println!("ssl:{:?}", config.ssl);

        match config.ssl.ssl_mode {
            crate::datasource_config::MySqlSslMode::Disabled => {}
            _ => {
                opts = opts.ssl_mode(config.ssl.ssl_mode.into());
                if let Some(ca) = config.ssl.ssl_files.ssl_ca.as_ref() {
                    opts = opts.ssl_ca(ca);
                }
                if let Some(cert) = config.ssl.ssl_files.ssl_cert.as_ref() {
                    opts = opts.ssl_client_cert(cert);
                }
                if let Some(key) = config.ssl.ssl_files.ssl_key.as_ref() {
                    opts = opts.ssl_client_key(key);
                }
            }
        }
        if let Some(pass) = &config.password {
            opts = opts.password(pass);
        }
        if let Some(db) = &config.db {
            opts = opts.database(db);
        }
        let mut pool_options = PoolOptions::new();
        if let Some(x) = config.conn_timeout {
            pool_options = pool_options.acquire_timeout(x.into());
        }
        pool_options = pool_options.max_connections(POOL_MAX_CONNECTIONS);
        let pool = pool_options.connect_with(opts).await?;
        anyhow::Ok(Self {
            pool,
            config,
            query_connection: Default::default(),
            key_words_cache: Default::default(),
            completion_id: Default::default(),
        })
    }

    async fn query_count(&self, name: &str, where_str: Option<&str>) -> anyhow::Result<usize> {
        let mut conn = self.get_conn().await?;
        let mut s = format!("SELECT count(1) FROM `{name}` ");
        if let Some(where_str) = where_str {
            s.push_str(format!(" where {where_str}").as_str());
        }
        let row = sqlx::raw_sql(AssertSqlSafe(s.as_str()))
            .fetch_one(conn.deref_mut())
            .await?;
        let count: i64 = row.get(0);
        anyhow::Ok(count as usize)
    }

    pub async fn get_conn(&self) -> anyhow::Result<PoolConnection<sqlx::MySql>> {
        let conn = self.pool.acquire().await?;
        anyhow::Ok(conn)
    }
}

impl MysqlDB {
    pub(crate) async fn list_schema_impl(&self) -> anyhow::Result<Vec<String>> {
        let mut conn: PoolConnection<sqlx::MySql> = self.get_conn().await?;
        let rows = sqlx::raw_sql("show databases")
            .fetch_all(conn.deref_mut())
            .await?;

        let mut ret = Vec::new();
        for r in rows.iter() {
            let name: String = r.try_get_unchecked(0)?;
            ret.push(name);
        }
        ret.sort();
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl DBTrait for MysqlDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let mut ret = self.list_schema_impl().await?;
        ret.sort();
        anyhow::Ok(ret)
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut config = self.config.clone();
        config.db = Some(name.to_owned());
        let x = Self::new(config).await?;
        Ok(Arc::new(x))
    }

    async fn create_schema(
        &self, name: &str, _extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("CREATE DATABASE `{name}`");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("DROP DATABASE `{name}`");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }
}

impl MysqlDB {
    async fn export_query_result_as_xlsx(
        &self, query_id: Option<&str>, sql: &str, include_header: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn_by_query_id(query_id).await?;
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let mut first = true;
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql))
            .fetch(conn.deref_mut())
            .try_chunks(100);
        let mut row_num = 0;
        while let Some(rows) = stream.next().await {
            let rows = rows?;
            if first && !rows.is_empty() && include_header {
                for (index, col) in rows[0].columns().iter().enumerate() {
                    let col = col.name();
                    sender
                        .send(
                            XlsxEdit {
                                row: row_num,
                                column: index as rust_xlsxwriter::ColNum,
                                content: col.to_owned(),
                            }
                            .to_string()
                            .into(),
                        )
                        .await?;
                }
                first = false;
                row_num += 1;
            }

            let data = encode_rows(&rows, true)?;
            export_query_as_xlsx_send_data(&mut row_num, data, sender.clone()).await?;
        }

        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl ExportQueryResult for MysqlDB {
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
                    Some(query_id),
                    sql,
                    include_header,
                    delimiter,
                    sender,
                )
                .await
            }
            ExportKind::Json { pretty } => {
                self.export_query_result_as_json(Some(query_id), sql, pretty, sender)
                    .await
            }
            ExportKind::JsonLine => {
                self.export_query_result_as_json_line(Some(query_id), sql, sender)
                    .await
            }
            ExportKind::Sql {
                include_table_schema: _,
            } => ExportKind::cannot_query_result_export_as_sql(),
            ExportKind::Xlsx { include_header } => {
                self.export_query_result_as_xlsx(Some(query_id), sql, include_header, sender)
                    .await
            }
            ExportKind::Html { include_header } => {
                self.export_query_result_as_html(Some(query_id), sql, include_header, sender)
                    .await
            }
        }
    }
}

#[async_trait::async_trait]
impl QuerySql for MysqlDB {
    async fn get_auto_commit(&self, query_id: &str) -> anyhow::Result<bool> {
        use sqlx_core::any::AnyConnectionBackend;

        let conn = self.query_connection.get(&query_id.to_string());
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };

        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();

        let d = conn.get_transaction_depth();

        anyhow::Ok(d == 0)
    }

    async fn set_auto_commit(&self, query_id: &str, commit: bool) -> anyhow::Result<()> {
        if commit {
            anyhow::bail!("sqlite use commit or rollback to end transaction");
        }
        use sqlx_core::any::AnyConnectionBackend;
        let conn = self.query_connection.get(&query_id.to_string());
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };

        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.begin(None).await?;
        anyhow::Ok(())
    }

    async fn commit(&self, query_id: &str) -> anyhow::Result<()> {
        use sqlx_core::any::AnyConnectionBackend;
        let conn = self.query_connection.get(&query_id.to_string());
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.commit().await?;

        anyhow::Ok(())
    }

    async fn rollback(&self, query_id: &str) -> anyhow::Result<()> {
        use sqlx_core::any::AnyConnectionBackend;
        let conn = self.query_connection.get(&query_id.to_string());
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.rollback().await?;
        anyhow::Ok(())
    }

    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.get_conn().await?;
        self.query_connection
            .insert(id.to_string(), Arc::new(Mutex::new(conn)));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        self.query_connection.remove(&id.to_string());

        anyhow::Ok(())
    }
    async fn execute_sql_no_open_conn(
        &self, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let mut conn = self.get_conn().await?;
        let ret = match conn.execute(sqlx::raw_sql(AssertSqlSafe(sql))).await {
            Ok(x) => Ok(QueryResp::row_affected(x.rows_affected(), sql.to_owned())),
            Err(err) => Err(err.to_string()),
        };
        anyhow::Ok(ret)
    }
    async fn execute_sql(
        &self, query_id: &str, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        crate::language_server::LangServerCommon::contains_tx_operation_error(self, sql)?;
        let conn = self.query_connection.get(&query_id.to_string());
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };

        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();

        let ret = match conn.execute(sqlx::raw_sql(AssertSqlSafe(sql))).await {
            Ok(x) => Ok(QueryResp::row_affected(x.rows_affected(), sql.to_owned())),
            Err(err) => Err(err.to_string()),
        };

        anyhow::Ok(ret)
    }

    async fn query_sql(
        &self, query_id: &str, sql: &str, _max_row: usize, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        crate::language_server::LangServerCommon::contains_tx_operation_error(self, sql)?;
        let conn = self.query_connection.get(&query_id.to_string());
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql)).fetch_many(conn.deref_mut());
        let mut rows = Vec::new();
        let mut emit_fields: Vec<Field> = vec![];
        let mut has_result = false;
        let mut timeelapsed = TimeElapsed::new();
        while let Some(item) = stream.next().await {
            let item = match item {
                Ok(x) => x,
                Err(err) => {
                    emit.send(QueryEvent::Err(err.to_string()))?;
                    continue;
                }
            };
            match item {
                sqlx::Either::Left(row_aff) => {
                    log::info!("row aff:{:?}", row_aff);
                }
                sqlx::Either::Right(row) => {
                    has_result = true;
                    let fields = get_row_types(&row);
                    if emit_fields.is_empty() || !fields.eq(&emit_fields) {
                        if !rows.is_empty() {
                            let data = encode_rows(&std::mem::take(&mut rows), true)?;
                            emit.send(data.into())?;
                        }
                        emit.send(
                            QueryResp {
                                fields: fields.clone(),
                                ..Default::default()
                            }
                            .into(),
                        )?;
                        emit_fields = fields;
                    }
                    rows.push(row);
                    if rows.len() >= self.query_buffer_size() || timeelapsed.elapsed() {
                        let data = encode_rows(&std::mem::take(&mut rows), true)?;
                        emit.send(data.into())?;
                    }
                }
            }
        }
        if !rows.is_empty() {
            emit.send(encode_rows(&rows, true)?.into())?;
        }
        drop(stream);
        if !has_result {
            use sqlx::Statement;
            let fields = conn
                .deref_mut()
                .prepare_with(AssertSqlSafe(sql).into_sql_str(), &[])
                .await
                .map(|s| {
                    s.columns()
                        .iter()
                        .map(|x| Field::from_name_type(x.name(), x.type_info().to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            emit.send(
                QueryResp {
                    fields,
                    ..Default::default()
                }
                .into(),
            )?;
        }
        anyhow::Ok(())
    }
}

/// mysql有哪些值可以直接放在sql语句里面的想current timestamp给我完成的rust列表
/// MySQL 里可以直接写在 SQL 语句中的 CURRENT_* 字面量
pub const MYSQL_CURRENT_LITERALS: &[&str] = &[
    "CURRENT_DATE",      //  2025-08-25
    "CURRENT_TIME",      //  14:36:07
    "CURRENT_TIMESTAMP", //  2025-08-25 14:36:07
    "CURRENT_USER",      //  'user'@'host'
];

impl SqlBuilder for MysqlDB {
    const CURRENT_VALUES: &'static [&'static str] = MYSQL_CURRENT_LITERALS;
    fn put_value_in_sql(
        &self, f: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        if self.hit_current_value(value) {
            return anyhow::Result::Ok(value.option_must_be_some()?.to_string());
        }
        let default_str = value.map(|x| x.to_string()).unwrap_or("null".to_string());
        let single_quote_str = value
            .map(|x| format!("'{}'", escape_sql_str(x)))
            .unwrap_or("null".to_string());
        let ty = f.to_uppercase();

        let v = match ty.as_str() {
            "TINYINT" => default_str,
            _ if ty.starts_with("TINYINT") => default_str,
            "BOOLEAN" | "BOOL" => default_str,
            "SMALLINT" => default_str,
            _ if ty.starts_with("SMALLINT") => default_str,
            "INT" => default_str,
            _ if ty.starts_with("INT") => default_str,
            "BIGINT" => default_str,
            _ if ty.starts_with("BIGINT") => default_str,
            "TINYINT UNSIGNED" => default_str,
            "SMALLINT UNSIGNED" => default_str,
            "INT UNSIGNED" => default_str,
            "BIGINT UNSIGNED" => default_str,
            "FLOAT" => default_str,
            "DOUBLE" => default_str,
            _ if type_is_string(&ty) => single_quote_str,
            "DECIMAL" => default_str,
            _ if ty.starts_with("DECIMAL") => default_str,
            "VARBINARY" | "BINARY" | "BLOB" | "LONGBLOB" => default_str,
            "TIMESTAMP" => single_quote_str,
            "DATETIME" => single_quote_str,
            "DATE" => single_quote_str,
            "TIME" => single_quote_str,
            "JSON" => single_quote_str,
            _ if ty.starts_with("ENUM") => single_quote_str,
            _ => {
                log::error!("mysql type '{}' not handled", ty);
                default_str
            }
        };
        log::trace!("type '{ty}' encode '{:?}' as #{v}#", value);
        anyhow::Ok(v)
    }
}

#[cfg(test)]
pub(crate) mod test_fns {

    use super::*;
    use crate::enable_test_log;
    // this test no need clean up, we use memory sqlite db.
    pub(crate) async fn make_test_object(config: MysqlDBConfig) -> MysqlDB {
        let db = MysqlDB::new(config).await.unwrap();
        db
    }

    pub(crate) async fn test_server_metas(db: MysqlDB) {
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }

    pub(crate) async fn test_transaction(db: MysqlDB) {
        let table_name = "test_commit";
        let _ = db.drop_table(table_name, false).await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_commit (
                ID INTEGER PRIMARY KEY,
                NAME VARCHAR(50)
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.set_auto_commit(query_id, false).await.unwrap();

        db.execute_sql(
            query_id,
            r#"
            INSERT INTO test_commit (ID, NAME) VALUES (1, 'Test');
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.rollback(query_id).await.unwrap();

        assert!(
            db.show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"),)
                .await
                .unwrap()
                .total
                == 0
        );
        db.set_auto_commit(query_id, false).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            INSERT INTO test_commit (ID, NAME) VALUES (1, 'Test');
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.commit(query_id).await.unwrap();
        assert!(
            db.show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"),)
                .await
                .unwrap()
                .total
                == 1
        );
    }

    pub(crate) async fn test_procedure(db: MysqlDB) {
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let f = "test_procedure";

        let _ = db
            .drop_procedure(
                f,
                Some(UdfOrProcedureExtraParameter::PostGres {
                    parameters: "".to_string(),
                }),
            )
            .await;

        db.execute_sql(
            query_id,
            r#" 
            CREATE PROCEDURE test_procedure()
            BEGIN
                -- 这个存储过程什么都不做
            END;
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let udfs = db.list_procedure().await.unwrap();
        for udf in udfs.iter() {
            let details = db.show_procedure(&udf.name, None).await.unwrap();
            println!("details:{:?}", details);
        }
    }

    pub(crate) async fn test_udf(db: MysqlDB) {
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let f = "test_function";

        let _ = db.drop_udf(f, None).await;

        db.execute_sql(
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
        .unwrap()
        .unwrap();

        let udfs = db.list_udf().await.unwrap();
        for udf in udfs.iter() {
            let details = db.show_udf(&udf.name, None).await.unwrap();
            println!("details:{:?}", details);
        }
    }

    pub(crate) async fn test_rename(db: MysqlDB) {
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("test_rename", false).await;
        let _ = db.drop_table("test_rename2", false).await;
        db.execute_sql(
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
        .unwrap()
        .unwrap();

        db.rename_table("test_rename", "test_rename2")
            .await
            .unwrap();

        db.show_table_collect(
            "test_rename2",
            ShowTableParameter::new_stupid_test("id asc"),
        )
        .await
        .unwrap();
    }

    pub(crate) async fn test_table_column(db: MysqlDB) {
        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _resp = db
            .execute_sql(
                query_id,
                r#"
                CREATE TABLE test_column
                (
                    id INT AUTO_INCREMENT PRIMARY KEY                     -- 主键，无符号32位整数
                )
                "#,
            )
            .await
            .unwrap()
            .unwrap();

        db.add_table_column(
            table_name,
            "name",
            ColumnType {
                typ: "VARCHAR(255)".to_string(),
                nullness: Some("null".to_string()),
                after: Some("id".to_string()),
                default_value: Some("abc".to_string()),
            },
        )
        .await
        .unwrap();

        db.modify_table_column(
            table_name,
            "name",
            "name2",
            Some(ColumnType {
                typ: "VARCHAR(100)".to_string(),
                nullness: Some("null".to_string()),
                after: None,
                default_value: Some("abc111".to_string()),
            }),
        )
        .await
        .unwrap();

        db.drop_table_column(table_name, "name2").await.unwrap();
    }

    pub(crate) async fn test_duplicate(db: MysqlDB) {
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("test_duplicate", false).await;
        let _ = db.drop_table("test_duplicate2", false).await;

        db.execute_sql(
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
        .unwrap()
        .unwrap();
        db.duplicate_table("test_duplicate", "test_duplicate2")
            .await
            .unwrap();

        let resp = db
            .show_table_collect(
                "test_duplicate2",
                ShowTableParameter::new_stupid_test("id asc"),
            )
            .await
            .unwrap();
        assert_eq!(resp.data.len(), 5);
    }

    pub(crate) async fn test_check_conn(db: MysqlDB) {
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }

    pub(crate) async fn test_triggers(db: MysqlDB) {
        let mut _conn = db.get_conn().await.unwrap();

        {
            // clean up
            let _ = db.drop_table("test_create_trigger", false).await;
            let _ = db.drop_table("test_create_trigger_update", false).await;
            let _ = db
                .drop_trigger("test_create_trigger", "after_insert_test_create_trigger")
                .await;
            let _ = db
                .drop_trigger("test_create_trigger", "after_insert_test_create_trigger2")
                .await;
        }

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"CREATE TABLE test_create_trigger (
                id INT AUTO_INCREMENT, 
                data VARCHAR(255) NOT NULL, 
                PRIMARY KEY (id) 
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_create_trigger_update (
                id INT AUTO_INCREMENT, 
                data VARCHAR(255) NOT NULL,  
                last_updated TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP, 
                PRIMARY KEY (id) 
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TRIGGER after_insert_test_create_trigger
            AFTER INSERT ON test_create_trigger
            FOR EACH ROW
            BEGIN
                INSERT INTO test_create_trigger_update (data)
                VALUES (NEW.data);
            END; 
    "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.execute_sql(
            query_id,
            r#"
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
        .unwrap()
        .unwrap();

        let table = "test_create_trigger";

        let ts = db.list_trigger(table).await.unwrap();
        for t in ts.iter() {
            let detail = db.show_trigger(table, &t.name).await.unwrap();

            println!("trigger:{} {:?}", t.name, detail);
            if t.name == "after_insert_test_create_trigger2" {
                assert_eq!(detail.timing, TriggerTiming::Before);
            } else if t.name == "after_insert_test_create_trigger" {
                assert_eq!(detail.timing, TriggerTiming::After);
            }
        }
    }

    pub(crate) async fn test_export(db: MysqlDB) {
        // init_log();
        let table_name = "test_export";
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let rs = [
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
            let _ = db.execute_sql(query_id, s).await.unwrap();
        }

        {
            let s = (db)
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", s);
            let _: serde_json::Value = serde_json::from_str(s.as_str()).unwrap();
        }
        {
            let s = (db)
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", s);
        }
        {
            let s = (db)
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

        let schema_trait = db;
        {
            let s = schema_trait
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", s);
        }
        {
            let s = schema_trait
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json:{}", s);
        }

        println!("##################################");

        {
            let s = schema_trait
                .export_query_result_to_string(
                    query_id,
                    format!("select * from {table_name}").as_str(),
                    ExportKind::Json { pretty: true },
                )
                .await
                .unwrap();
            println!("json:{}", s);
            let _: serde_json::Value = serde_json::from_str(s.as_str()).unwrap();
        }
        {
            let s = schema_trait
                .export_query_result_to_string(
                    "",
                    format!("select * from {table_name}").as_str(),
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", s);
        }

        {
            let s = schema_trait
                .export_query_result_to_string(
                    query_id,
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

        let _ = db.drop_table(table_name, false).await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();

        db.execute_sql(
            query_id,
            format!(
                r#"
            CREATE TABLE {} (
                id INT PRIMARY KEY,
                name VARCHAR(255),
                age INT,
                email VARCHAR(255),
                INDEX idx_name (name),
                UNIQUE INDEX idx_email (email),
                FULLTEXT INDEX idx_fulltext (name, email)
            );
        "#,
                table_name
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let indexes = db.show_indexes(table_name).await.unwrap();
        println!("{:?}", indexes);
    }

    pub(crate) async fn test_types1(db: MysqlDB) {
        // enable_test_log();
        let table_name = "test_types1";

        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();

        db.execute_sql(
            query_id,
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
        .await
        .unwrap()
        .unwrap();

        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert("tinyint_col".to_string(), Some("1".to_string()));
        values.insert("boolean_col".to_string(), Some("false".to_string()));
        values.insert("tinyint_col2".to_string(), Some("2".to_string()));
        values.insert("smallint_col".to_string(), Some("3".to_string()));
        values.insert("int_col".to_string(), Some("4".to_string()));
        values.insert("bigint_col".to_string(), Some("5".to_string()));

        db.table_insert(table_name, values).await.unwrap();

        let cols = db
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("tinyint_col asc"),
            )
            .await
            .unwrap();
        println!("{:?}", cols);
        {
            // export test.
            let e = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", e);
            let e = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();

            println!("csv:{}", e);
            // export test.
            let e = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            println!("json:{}", e);

            let e = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();

            println!("json line:{}", e);
        }
    }

    pub(crate) async fn test_types2(db: MysqlDB) {
        // enable_test_log();
        let table_name = "test_types2";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
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
        .await
        .unwrap()
        .unwrap();

        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert("u8_col".to_string(), Some("1".to_string()));
        values.insert("u16_col".to_string(), Some("2".to_string()));
        values.insert("u32_col".to_string(), Some("2".to_string()));
        values.insert("u64_col".to_string(), Some("3".to_string()));
        values.insert("f32_col".to_string(), Some("4".to_string()));
        values.insert("f64_col".to_string(), Some("5".to_string()));

        db.table_insert(table_name, values).await.unwrap();

        let cols = db
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("u8_col asc"),
            )
            .await
            .unwrap();
        println!("{:?}", cols);
        {
            // export test.
            let e = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", e);
            let e = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();

            println!("csv:{}", e);
            // export test.
            let e = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            println!("json:{}", e);

            let e = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();

            println!("json line:{}", e);
        }
    }

    pub(crate) async fn test_types3(db: MysqlDB) {
        // enable_test_log();
        let table_name = "test_types3";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
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
        .await
        .unwrap()
        .unwrap();

        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert("varchar_col".to_string(), Some("x".to_string()));
        values.insert("char_col".to_string(), Some("y".to_string()));
        values.insert("text_col".to_string(), Some("z".to_string()));
        values.insert("varbinary_col".to_string(), Some("0x01020304".to_string()));
        values.insert("binary_col".to_string(), Some("0x0102030405".to_string()));
        values.insert("blob_col".to_string(), Some("0x01020304".to_string()));

        db.table_insert(table_name, values).await.unwrap();

        let cols = db
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("varchar_col asc"),
            )
            .await
            .unwrap();
        println!("{:?}", cols);
        {
            // export test.
            let e = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", e);
            let e = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();

            println!("csv:{}", e);
            // export test.
            let e = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            println!("json:{}", e);

            let e = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();

            println!("json line:{}", e);
        }
    }

    pub(crate) async fn test_types4(db: MysqlDB) {
        enable_test_log();
        let table_name = "test_types4";

        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_types4 (
                    timestamp_column TIMESTAMP,
                    datetime_column DATETIME,
                    date_column DATE,
                    time_column TIME,
                    status ENUM('pending', 'completed', 'canceled') NOT NULL,
                    decimal_column DECIMAL(10, 2) NOT NULL,
                    json_column JSON NOT NULL
                );
        "#,
        )
        .await
        .unwrap()
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
        values.insert("status".to_string(), Some("completed".to_string()));
        values.insert("decimal_column".to_string(), Some("123.45".to_string()));
        values.insert(
            "json_column".to_string(),
            Some(r#"{"key": "value"}"#.to_string()),
        );

        db.table_insert(table_name, values).await.unwrap();

        let cols = db
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("timestamp_column asc"),
            )
            .await
            .unwrap();
        println!("{:?}", cols);
        {
            // export test.
            let e = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", e);
            let e = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();

            println!("csv:{}", e);
            // export test.
            let e = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            println!("json:{}", e);

            let e = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();

            println!("json line:{}", e);
        }
    }

    pub(crate) async fn test_list_database(db: MysqlDB) {
        let db = &db as &dyn crate::db::DBTrait;
        let resp = db.list_schema().await.unwrap();
        println!("database are:{:?}", resp);
    }

    pub(crate) async fn test_view(db: MysqlDB) {
        enable_test_log();
        let table_name = "test_view_table";
        let view_name = "test_view";
        let _ = db.drop_view(view_name).await;
        let _ = db.drop_table(table_name, true).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_view_table (
                id INT AUTO_INCREMENT PRIMARY KEY,
                test_time DATETIME NOT NULL,
                speed DECIMAL(5, 2) NOT NULL,
                unit VARCHAR(10) NOT NULL
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"               
            INSERT INTO test_view_table (test_time, speed, unit) VALUES
            ('2025-04-15 10:00:00', 100.50, 'Mbps'),
            ('2025-04-15 10:30:00', 95.75, 'Mbps'),
            ('2025-04-15 11:00:00', 102.30, 'Mbps'),
            ('2025-04-15 11:30:00', 98.20, 'Mbps'),
            ('2025-04-15 12:00:00', 101.00, 'Mbps');

        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"               
            CREATE VIEW test_view AS
            SELECT *
            FROM test_view_table;

        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let views = db.list_view().await.unwrap();
        for view in views.iter() {
            println!("view:{:?}", view);
            let resp = db
                .show_view_collect(&view.name, ShowTableParameter::new_stupid_test("id asc"))
                .await
                .unwrap();
            println!("resp:{:?}", resp);
        }
    }

    pub(crate) async fn test_event(db: MysqlDB) {
        enable_test_log();
        let id = "";
        db.open_connection(id).await.unwrap();
        let name = "test_event";
        let _ = db.drop_event(name).await;
        db.execute_sql(
            id,
            r#"
            CREATE EVENT test_event
            ON SCHEDULE EVERY 10 SECOND
            STARTS CURRENT_TIMESTAMP
            DO
            BEGIN
                /* 空语句块，可永久存在 */
            END;
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        for d in db.list_event().await.unwrap() {
            println!("event:{:?}", d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_fns::{self, make_test_object};
    use crate::TEST_HOST;
    use crate::datasource_config::MysqlDBConfig;
    use crate::db::{SchemaTrait, ShowTableParameter};

    pub(crate) fn make_config() -> MysqlDBConfig {
        MysqlDBConfig {
            conn_timeout: Some(5000.into()),
            host: TEST_HOST.to_string(),
            username: "root".to_string(),
            password: Some("root".to_string()),
            port: 3306,
            db: Some("test".to_string()),
            read_only: false,
            save_password: true,
            ssl: Default::default(),
        }
    }
    #[ignore = "xxxx"]
    #[tokio::test]
    async fn test_xxx_yyyy() {
        let mut config = make_config();
        config.db = Some("mysql".to_string());
        let db = make_test_object(config).await;

        // SELECT * FROM `innodb_index_stats` LIMIT 100 OFFSET 0
        let resp = db
            .show_table_collect(
                "innodb_index_stats",
                ShowTableParameter::new_stupid_test("id asc"),
            )
            .await
            .unwrap();

        println!("{:?}", resp);
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
    async fn test_transaction() {
        test_fns::test_transaction(make_test_object(make_config()).await).await;
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
    async fn test_table_column() {
        test_fns::test_table_column(make_test_object(make_config()).await).await;
    }

    #[tokio::test]
    async fn test_view() {
        test_fns::test_view(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_event() {
        test_fns::test_event(make_test_object(make_config()).await).await;
    }
}
