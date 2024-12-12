use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use chrono::FixedOffset;
use dashmap::DashMap;
use futures_core::future::BoxFuture;
use futures_util::{StreamExt, TryStreamExt};
use sqlparser::tokenizer::Token;
use tiberius::{Client, ColumnData, ColumnType, FromSql, Row};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::completion::{AutoCompletionTrait, CompletionItem};

use crate::datasource_config::SqlServerDbConfig;
use crate::db::{
    format_data_time_fix_offset, BackendName, CatalogTrait, CheckConn, DBTrait,
    DropUdfExtraParameter, ExportKind, Field, FormatSql, FormatSqlOpts, Index, IndexKind,
    ListTable, NamedStaff, ProcedureDetail, QueryResp, QuerySql, SchemaTrait, ServerMetas,
    SqlExecuteRet, TableListResp, TriggerDetail, TriggerEnable, TriggerEvent, TriggerTiming,
    UdfDetail,
};
use crate::helper::{
    create_csv, is_dangerous_sql_by_parser, sql_to_sql_seq, sqlformat_format_sql, SqlKind, SqlSeq,
};
use crate::meta_value::MetaValue;
use crate::query_log::QueryLog;
use crate::MemoryLimit;

#[derive(Clone)]
pub struct SqlServer {
    pool: bb8::Pool<bb8_tiberius::ConnectionManager>,
    config: SqlServerDbConfig,

    query_connections: Arc<DashMap<uuid::Uuid, Arc<Mutex<Connection>>>>,
}

impl FormatSql for SqlServer {
    fn format_sql(&self, sql: &str, ops: Option<FormatSqlOpts>) -> anyhow::Result<String> {
        anyhow::Ok(sqlformat_format_sql(sql, ops.map(|x| x.into())))
    }
}

#[async_trait::async_trait]
impl ServerMetas for SqlServer {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let mut conn = self.get_conn().await?;
        let mut ret = Vec::new();

        let mut stream = conn
            .simple_query(
                r#"
                SELECT
                    'MachineName' AS [Property],
                    CAST(SERVERPROPERTY('MachineName') AS nvarchar(50)) AS [Value]
                UNION ALL
                SELECT
                    'InstanceName',
                    CAST(SERVERPROPERTY('InstanceName') AS nvarchar(50))
                UNION ALL 
                SELECT
                    'ComputerNamePhysicalNetBIOS',
                    CAST(SERVERPROPERTY('ComputerNamePhysicalNetBIOS') AS nvarchar(50))
                UNION ALL
                SELECT
                    'PhysicalMemoryInKilobytes',
                    CAST(SERVERPROPERTY('PhysicalMemoryInKilobytes') AS nvarchar(50))
                UNION ALL
                SELECT
                    'ProcessorCount',
                    CAST(SERVERPROPERTY('ProcessorCount') AS nvarchar(50))
                UNION ALL
                SELECT
                    'ProcessorCore',
                    CAST(SERVERPROPERTY('ProcessorCore') AS nvarchar(50))
                UNION ALL
                SELECT
                    'ProcessorCoreCount',
                    CAST(SERVERPROPERTY('ProcessorCoreCount') AS nvarchar(50))
                UNION ALL
                SELECT
                    'IsClustered',
                    CAST(SERVERPROPERTY('IsClustered') AS nvarchar(50))
                UNION ALL
                SELECT
                    'IsHadrEnabled',
                    CAST(SERVERPROPERTY('IsHadrEnabled') AS nvarchar(50))
                UNION ALL
                SELECT
                    'HadrManagerStatus',
                    CAST(SERVERPROPERTY('HadrManagerStatus') AS nvarchar(50))
                UNION ALL
                SELECT
                    'ProductVersion',
                    CAST(SERVERPROPERTY('ProductVersion') AS nvarchar(50))
                UNION ALL
                SELECT
                    'ProductLevel',
                    CAST(SERVERPROPERTY('ProductLevel') AS nvarchar(50))
                UNION ALL
                SELECT
                    'Edition',
                    CAST(SERVERPROPERTY('Edition') AS nvarchar(50))
                UNION ALL
                SELECT
                    'EngineEdition',
                    CAST(SERVERPROPERTY('EngineEdition') AS nvarchar(50))
                UNION ALL
                SELECT
                    'ProductUpdateLevel',
                    CAST(SERVERPROPERTY('ProductUpdateLevel') AS nvarchar(50))
                UNION ALL
                SELECT
                    'ProductUpdateReference',
                    CAST(SERVERPROPERTY('ProductUpdateReference') AS nvarchar(50))
                UNION ALL
                SELECT
                    'InstanceDefaultDataPath',
                    CAST(SERVERPROPERTY('InstanceDefaultDataPath') AS nvarchar(50))
                UNION ALL
                SELECT
                    'InstanceDefaultLogPath',
                    CAST(SERVERPROPERTY('InstanceDefaultLogPath') AS nvarchar(50))
                UNION ALL
                SELECT
                    'SqlCharSet',
                    CAST(SERVERPROPERTY('SqlCharSet') AS nvarchar(50))
                UNION ALL
                SELECT
                    'SqlCollation',
                    CAST(SERVERPROPERTY('SqlCollation') AS nvarchar(50))
                UNION ALL
                SELECT
                    'SqlSortOrder',
                    CAST(SERVERPROPERTY('SqlSortOrder') AS nvarchar(50))
            "#,
            )
            .await?;
        let mut rows = Vec::new();

        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(r) => {
                    rows.push(r);
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        let metas = encode_rows(&rows)?;
        for m in metas.iter() {
            ret.push(MetaValue::new_opt(
                m.get(0).unwrap().as_ref().unwrap().to_string(),
                (&m[1]).clone(),
            ));
        }

        anyhow::Ok(ret)
    }
}
impl SqlServer {
    pub async fn new(c: SqlServerDbConfig) -> anyhow::Result<Self> {
        let manager = bb8_tiberius::ConnectionManager::new(make_config(&c));
        let pool = bb8::Pool::builder().max_size(50).build(manager).await?;
        anyhow::Ok(Self {
            config: c,
            pool,
            query_connections: Default::default(),
        })
    }
}

fn make_config(c: &SqlServerDbConfig) -> tiberius::Config {
    let mut config = tiberius::Config::new();
    config.host(&c.host);
    config.port(c.port);
    config.authentication(tiberius::AuthMethod::sql_server(&c.username, &c.password));
    if let Some(db) = &c.db_name {
        config.database(db);
    }

    if c.trust_cert {
        config.trust_cert(); // on production, it is not a good idea to do this
    }
    config
}

fn get_column_type_string(c: ColumnType) -> String {
    let x = format!("{:?}", c);
    x.to_lowercase()
}

fn get_fields_from_meta(meta: &tiberius::ResultMetadata) -> Vec<Field> {
    let mut ret = Vec::with_capacity(meta.columns().len());
    for c in meta.columns().iter() {
        ret.push(Field::from_name_type(
            c.name().to_string(),
            get_column_type_string(c.column_type()),
        ));
    }
    ret
}

struct ConvertColumnDataToString {
    value: String,
}

impl<'a> tiberius::FromSql<'a> for ConvertColumnDataToString {
    fn from_sql(value: &'a tiberius::ColumnData<'static>) -> tiberius::Result<Option<Self>> {
        let value = match value {
            tiberius::ColumnData::U8(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::I16(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::I32(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::I64(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::F32(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::F64(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::Bit(x) => x.as_ref().map(|x| if *x { 1 } else { 0 }.to_string()),
            tiberius::ColumnData::String(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::Guid(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::Binary(x) => {
                x.as_ref().map(|x| format!("0X{}", hex::encode_upper(x)))
            }
            tiberius::ColumnData::Numeric(x) => x.as_ref().map(|x| x.to_string()),

            tiberius::ColumnData::Xml(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::DateTime(_)
            | tiberius::ColumnData::DateTime2(_)
            | tiberius::ColumnData::SmallDateTime(_) => chrono::NaiveDateTime::from_sql(value)
                .ok()
                .flatten()
                .map(|x| x.to_string()),

            tiberius::ColumnData::Date(_) => chrono::NaiveTime::from_sql(value)
                .ok()
                .flatten()
                .map(|x| x.to_string()),

            tiberius::ColumnData::Time(_x) => chrono::NaiveTime::from_sql(value)
                .ok()
                .flatten()
                .map(|x| x.to_string()),

            tiberius::ColumnData::DateTimeOffset(_) => {
                chrono::DateTime::<FixedOffset>::from_sql(value)
                    .ok()
                    .flatten()
                    .map(|x| format_data_time_fix_offset(&x))
            }
        };
        Ok(value.map(|x| Self { value: x }))
    }
}

fn make_table_list_from_meta_and_row(
    meta: &tiberius::ResultMetadata, rows: &Vec<tiberius::Row>, total: usize,
    sql: impl Into<String>,
) -> anyhow::Result<TableListResp> {
    let fields = get_fields_from_meta(meta);
    let data = encode_rows(rows)?;

    anyhow::Ok(TableListResp::new(fields, total, data, Some(sql.into())))
}

fn encode_rows(rows: &Vec<tiberius::Row>) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    if rows.len() == 0 {
        return anyhow::Ok(vec![]);
    }
    let mut data = vec![];
    for r in rows.iter() {
        let mut values: Vec<Option<String>> = Vec::new();
        for index in 0..rows[0].columns().len() {
            let v: Option<ConvertColumnDataToString> = r.try_get(index)?;
            values.push(v.map(|x| x.value));
        }
        data.push(values);
    }
    anyhow::Ok(data)
}

fn gen_select_sql(
    schema: &str, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize,
) -> String {
    let mut sql = format!("SELECT * FROM {schema}.{table_name}");
    if let Some(where_str) = where_str {
        sql.push_str(format!(" WHERE {where_str} ").as_str());
    }
    if let Some(order_by) = order_by {
        sql.push_str(format!(" ORDER BY {order_by} ").as_str());
    } else {
        // sql server 的offet子句只能和order by一起使用
        sql.push_str(" ORDER  BY (SELECT NULL) ")
    }

    let offset = (page - 1) * page_size;
    sql.push_str(format!(" OFFSET {} ROWS FETCH NEXT {} ROWS ONLY", offset, page_size).as_str());

    sql
}

// TODO sqlparse don't support parse trigger statement
// use lexer to solve this issue.
fn extract_trigger_event_base_on_lexer(sql: &str) -> anyhow::Result<TriggerEvent> {
    let t = sqlparser::tokenizer::Tokenizer::new(&sqlparser::dialect::SQLiteDialect {}, &sql)
        .tokenize()?;
    let mut event = None;

    for token in t {
        match token {
            Token::Word(x) => {
                let word = x.to_string().to_uppercase();
                match word.as_str() {
                    "INSERT" | "UPDATE" | "DELETE" => {
                        event = Some(word);
                    }

                    _ => {}
                }
            }
            _ => {}
        }
    }
    if event.is_none() {
        anyhow::bail!("trigger statement is not valid");
    }
    let event = event.unwrap();

    anyhow::Ok(TriggerEvent::try_from(event.as_str())?)
}

#[async_trait::async_trait]
impl CheckConn for SqlServer {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let _x = conn.query("select 1", &[]).await?;
        anyhow::Ok(ins.elapsed())
    }
}

impl SqlServer {
    async fn execute_simple_execute(&self, sql: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream = conn.simple_query(sql).await?;
        while let Some(item) = stream.next().await {
            let _item = item?;
        }
        anyhow::Ok(())
    }
}

impl BackendName for SqlServer {
    fn backend_name(&self) -> &str {
        "sqlserver"
    }
}

#[async_trait::async_trait]
impl SchemaTrait for SqlServer {
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
        let schema = self.config.schema.as_ref().unwrap().as_str();
        let sql = format!(
            r#"
            SELECT * INTO {schema}.{dup} FROM {schema}.{table} 
        "#
        );
        self.execute_simple_execute(sql.as_str()).await
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
            EXEC sp_rename '{old}', '{new}';
        "#
        );
        self.execute_simple_execute(&sql).await
    }

    async fn list_trigger(&self, table_name: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
            SELECT 
                t.name AS trigger_name
            FROM 
                sys.triggers t
            WHERE 
                t.parent_class = 1 -- 1 表示与表关联的触发器
                AND OBJECT_NAME(t.parent_id) = '{table_name}' -- 指定要查看的表名
        "#
        );
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream = conn.simple_query(&sql).await?;
        let mut ret = Vec::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(x) => {
                    let name: Option<&str> = x.get(0);
                    ret.push(name.unwrap().to_string());
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
    }

    async fn show_trigger(&self, table: &str, name: &str) -> anyhow::Result<TriggerDetail> {
        let sql = format!(
            r#"
            SELECT
                t.is_disabled,
                t.is_instead_of_trigger,
                OBJECT_DEFINITION(t.object_id) AS trigger_definition
            FROM
                sys.triggers t
            WHERE
                t.parent_class_desc = 'OBJECT_OR_COLUMN'
                AND OBJECT_NAME(t.parent_id) = '{table}' and name ='{name}'
      "#
        );
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream = conn.simple_query(&sql).await?;
        let mut result = None;
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(x) => {
                    let name: Option<bool> = x.get(0);
                    let is_instead_of_trigger: Option<bool> = x.get(1);
                    let sql: Option<&str> = x.get(2);
                    result = Some((
                        name.unwrap_or_default(),
                        is_instead_of_trigger.unwrap_or_default(),
                        sql.unwrap().to_string(),
                    ));
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        let (disabled, instead_of, sql) = match result {
            Some(x) => x,
            None => anyhow::bail!("select trigger don't return a result"),
        };
        let event = extract_trigger_event_from_sql(&sql)?;
        anyhow::Ok(TriggerDetail::new(
            if instead_of {
                TriggerTiming::InsteadOf
            } else {
                TriggerTiming::After
            },
            event,
            sql,
            Some(TriggerEnable::Bool { enable: !disabled }),
        ))
    }

    async fn drop_trigger(&self, table: &str, name: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"table
         DROP TRIGGER dbo.{name} ON {table};
        "#
        );
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();

        conn.execute(sql.as_str(), &[])
            .await
            .context("can't drop trigger")?;

        anyhow::Ok(())
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
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let total = SqlServer::query_count(conn, view_name, where_str.clone()).await?;
        let sql = gen_select_sql(
            self.config.schema.as_ref().map(|x| x.as_str()).unwrap(),
            view_name,
            where_str,
            order_by,
            page,
            page_size,
        );

        let mut stream = conn.query(&sql, &[]).await?;
        let mut meta = None;
        let mut rows = Vec::with_capacity(page_size);
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    rows.push(row);
                }
                tiberius::QueryItem::Metadata(m) => meta = Some(m),
            }
        }
        let ret = match meta {
            Some(x) => make_table_list_from_meta_and_row(&x, &rows, total as usize, sql)?,
            None => anyhow::bail!("SELECT should yield at lease one meta"),
        };
        anyhow::Ok(ret)
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let schema = self.config.schema.as_ref().unwrap();

        let s = format!("DROP VIEW {schema}.{view_name}");
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();

        conn.execute(s.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("DROP TABLE {table}");
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        conn.execute(sql.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn list_view(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream = conn
            .query(
                r#"
                SELECT name
                FROM sys.views;
        "#,
                &[],
            )
            .await?;
        let mut ret = Vec::new();

        while let Some(item) = stream.next().await {
            let item = item?;

            match item {
                tiberius::QueryItem::Row(r) => {
                    let table_name: Option<&str> = r.try_get(0)?;
                    ret.push(table_name.unwrap().to_string());
                }
                tiberius::QueryItem::Metadata(_) => {
                    // I know the meta pretty well
                }
            }
        }
        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream = conn
            .query(
                r#"
            SELECT TABLE_NAME
            FROM INFORMATION_SCHEMA.TABLES
            WHERE TABLE_TYPE = 'BASE TABLE';
        "#,
                &[],
            )
            .await?;
        let mut ret = Vec::new();

        while let Some(item) = stream.next().await {
            let item = item?;

            match item {
                tiberius::QueryItem::Row(r) => {
                    let table_name: Option<&str> = r.try_get(0)?;
                    ret.push(table_name.unwrap().to_string());
                }

                tiberius::QueryItem::Metadata(_) => {
                    // I know the meta pretty well
                }
            }
        }
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    async fn show_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let mut conn = self.get_conn().await?;
        let mut conn = conn.deref_mut();
        let mut stream = conn
            .query(
                format!(
                    r#"
                SELECT COLUMN_NAME,DATA_TYPE,COLUMN_DEFAULT,IS_NULLABLE
                    FROM INFORMATION_SCHEMA.COLUMNS
                    WHERE TABLE_NAME = '{table_name}' ORDER BY  ORDINAL_POSITION
                "#
                ),
                &[],
            )
            .await?;
        let mut ret = Vec::new();

        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    let name: Option<&str> = row.get(0);
                    let name = name.unwrap().to_string();
                    let data_type: Option<&str> = row.get(1);
                    let data_type = data_type.unwrap().to_string();
                    let default_value: Option<&str> = row.get(2);
                    let nullable: Option<&str> = row.get(3);
                    ret.push(Field::new(
                        name,
                        data_type,
                        nullable
                            .map(|x| x.to_uppercase() == "YES")
                            .unwrap_or_default(),
                        default_value.map(|x| x.to_string()),
                        false,
                        false,
                    ));
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        drop(stream);
        for f in ret.iter_mut() {
            let is_primary = SqlServer::is_primary(&mut conn, table_name, &f.field).await?;
            let is_auto_increment =
                SqlServer::is_auto_increment(&mut conn, table_name, &f.field).await?;
            f.is_primary = is_primary;
            f.is_auto_increment = is_auto_increment;
        }

        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let total = SqlServer::query_count(conn, table_name, where_str.clone()).await?;
        let sql = gen_select_sql(
            self.config.schema.as_ref().map(|x| x.as_str()).unwrap(),
            table_name,
            where_str,
            order_by,
            page,
            page_size,
        );

        let mut stream = conn
            .query(&sql, &[])
            .await
            .expect(format!("sql :{}", sql).as_str());
        let mut meta = None;
        let mut rows = Vec::with_capacity(page_size);
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    rows.push(row);
                }
                tiberius::QueryItem::Metadata(m) => meta = Some(m),
            }
        }
        let cols = self.show_columns(table_name).await?;
        let mut ret = match meta {
            Some(x) => make_table_list_from_meta_and_row(&x, &rows, total as usize, sql)?,
            None => anyhow::bail!("SELECT should yield at lease one meta"),
        };
        // infered schema is accurate as show_columns
        ret.fields = cols;
        anyhow::Ok(ret)
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let sql = format!(
            r#"
        SELECT
            i.name AS IndexName,
            i.is_unique AS IsUnique,
            c.name AS ColumnName
        FROM sys.indexes AS i
        INNER JOIN sys.index_columns AS ic ON i.object_id = ic.object_id AND i.index_id = ic.index_id
        INNER JOIN sys.columns AS c ON ic.object_id = c.object_id AND ic.column_id = c.column_id
        WHERE i.object_id = OBJECT_ID('{table_name}')
        "#
        );

        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream = conn.query(sql.as_str(), &[]).await?;
        let mut ret: HashMap<String, Index> = HashMap::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(r) => {
                    let name: Option<&str> = r.try_get(0)?;
                    let name = name.unwrap().to_string();
                    let is_unique: Option<bool> = r.try_get(1)?;
                    let col: Option<&str> = r.try_get(2)?;
                    let col = col.unwrap().to_string();
                    if let Some(x) = ret.get_mut(&name) {
                        x.on.push(col);
                    } else {
                        ret.insert(
                            name.clone(),
                            Index {
                                name,
                                kind: if is_unique.unwrap_or_default() {
                                    IndexKind::Unique
                                } else {
                                    IndexKind::Normal
                                },
                                on: vec![col],
                            },
                        );
                    }
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        anyhow::Ok(ret.into_iter().map(|(_, v)| v).collect())
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fs = self.show_columns(table_name).await?;
        let s = (self as &dyn crate::db::SchemaTrait).insert_sql(
            self.config.schema.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &values,
        )?;

        log::trace!("insert sql:{}", s);

        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let _r = conn.execute(s.as_str(), &[]).await?;
        anyhow::Ok(SqlExecuteRet {
            sql: s,
            changes: 1, // I know this
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
        let s = (self as &dyn crate::db::SchemaTrait).update_sql(
            self.config.schema.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &values,
            &wheres,
        )?;
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let r = conn.execute(s.as_str(), &[]).await?;
        anyhow::Ok(SqlExecuteRet::from((s, r.rows_affected().iter().sum())))
    }

    /// format a value make it suitable for insert into sql.
    /// this method current use in `insert_sql` etc.
    fn put_value_in_sql(&self, f: &Field, value: Option<&str>) -> anyhow::Result<String> {
        let default_value = value.map(|x| x.to_string()).unwrap_or("null".to_string());
        let single_quote = value
            .map(|x| format!("'{}'", x))
            .unwrap_or("null".to_string());
        let v = match f.typ.as_str().to_uppercase().as_str() {
            "TINYINT" | "SMALLINT" | "INT" | "BIGINT" | "FLOAT" | "REAL" => default_value,
            "BIT" => default_value,
            "NVARCHAR" | "VARCHAR" | "NCHAR" | "CHAR" | "NTEXT" | "TEXT" => single_quote,
            "BINARY" | "VARBINARY" | "IMAGE" | "UNIQUEIDENTIFIER" | "NUMERIC" | "DECIMAL" => {
                default_value
            }
            "XML" | "DATETIME" | "DATETIME2" | "SMALLDATETIME" | "DATE" | "TIME"
            | "DATETIMEOFFSET" => single_quote,
            _ => anyhow::bail!("type '{}' not support", f.typ.as_str()),
        };
        log::trace!("type:{:?} encode '{:?}' as #{}#", f.typ, value, v);
        anyhow::Ok(v)
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let s = format!(
            r#"
            TRUNCATE TABLE {table_name};
        "#
        );
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        conn.execute(s.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fields = self.show_columns(table_name).await?;
        let sql = (self as &dyn crate::db::SchemaTrait).delete_sql(
            self.config.schema.as_ref().map(|x| x.as_str()),
            table_name,
            &fields,
            &wheres,
        )?;
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();

        let r = conn.execute(sql.as_str(), &[]).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: r.rows_affected().iter().sum(),
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }
    async fn disable_trigger(&self, table: &str, name: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();

        let sql = format!("ENABLE TRIGGER {name} ON {table}");
        conn.execute(sql.as_str(), &[]).await?;
        anyhow::Ok(())
    }
    async fn enable_trigger(&self, table: &str, name: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();

        let sql = format!("DISABLE TRIGGER {name} ON {table}");
        conn.execute(sql.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
            SELECT 
                name AS FunctionName
            FROM 
                sys.objects
            WHERE 
                type IN ('FN', 'IF', 'TF') AND  SCHEMA_NAME(schema_id) = 'dbo';
        "#
        );
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream = conn.query(sql.as_str(), &[]).await?;
        let mut ret = Vec::new();

        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    let name: Option<&str> = row.try_get(0)?;
                    ret.push(NamedStaff::from(name.unwrap().to_owned()));
                }

                tiberius::QueryItem::Metadata(_result_metadata) => {}
            }
        }

        anyhow::Ok(ret)
    }
    async fn show_udf(&self, name: &str) -> anyhow::Result<UdfDetail> {
        let sql = format!(
            r#"
        SELECT 
            o.type_desc AS FunctionType,
            o.create_date,
            o.modify_date
        FROM 
            sys.objects AS o
        WHERE 
            o.name = '{name}'  -- 替换为你的函数名
            AND o.type IN ('FN', 'IF', 'TF');  -- FN: 标量函数, IF: 内联表值函数, TF: 多语句表值函数
        "#
        );
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut resp = conn.simple_query(&sql).await?;
        let mut metas = Vec::with_capacity(3);

        while let Some(item) = resp.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    let type_desc: Option<&str> = row.try_get(0)?;
                    let create_date: Option<chrono::NaiveDateTime> = row.try_get(1)?;
                    let modify_date: Option<chrono::NaiveDateTime> = row.try_get(2)?;
                    metas.push(MetaValue::new(
                        "type_desc",
                        type_desc.unwrap_or_default().to_string(),
                    ));
                    metas.push(MetaValue::new(
                        "create_date",
                        create_date.unwrap_or_default(),
                    ));
                    metas.push(MetaValue::new(
                        "modify_date",
                        modify_date.unwrap_or_default(),
                    ));
                }
                tiberius::QueryItem::Metadata(_result_metadata) => {}
            }
        }
        drop(resp);
        let sql = format!(
            "  SELECT 
                OBJECT_DEFINITION(OBJECT_ID('dbo.{name}')) AS FunctionDefinition; "
        );
        let mut resp = conn.simple_query(&sql).await?;

        while let Some(item) = resp.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    let def: Option<&str> = row.try_get(0)?;
                    return anyhow::Ok(UdfDetail::new(def.unwrap_or_default().to_string(), metas));
                }
                tiberius::QueryItem::Metadata(_result_metadata) => {}
            }
        }
        anyhow::bail!("query udf defition failed, no result.")
    }

    async fn drop_udf(
        &self, name: &str, _extra: Option<DropUdfExtraParameter>,
    ) -> anyhow::Result<()> {
        let sql = format!("DROP FUNCTION dbo.{name}");
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        conn.execute(sql.as_str(), &[]).await?;

        anyhow::Ok(())
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = r#"
            SELECT 
                ROUTINE_NAME 
            FROM 
                INFORMATION_SCHEMA.ROUTINES 
            WHERE 
                ROUTINE_TYPE = 'PROCEDURE';
            "#;
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut resp = conn.simple_query(sql).await?;
        let mut ret = Vec::new();
        while let Some(item) = resp.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    let name: Option<&str> = row.try_get(0)?;
                    ret.push(NamedStaff::from(name.unwrap_or_default().to_string()));
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }

        anyhow::Ok(ret)
    }
    async fn show_procedure(&self, name: &str) -> anyhow::Result<ProcedureDetail> {
        let sql = format!(
            r#"
            SELECT 
                p.create_date AS CreationDate,
                p.modify_date AS LastModified,
                m.definition AS ProcedureDefinition
                FROM 
                    sys.procedures p
                JOIN 
                    sys.sql_modules m ON p.object_id = m.object_id
                WHERE 
                    p.name = '{name}';
        "#
        );

        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut resp = conn.simple_query(&sql).await?;

        while let Some(item) = resp.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    let create_date: Option<chrono::NaiveDateTime> = row.try_get(0)?;
                    let modify_date: Option<chrono::NaiveDateTime> = row.try_get(1)?;
                    let def: Option<&str> = row.try_get(2)?;
                    return anyhow::Ok(ProcedureDetail::new(
                        def.unwrap_or_default().to_string(),
                        vec![
                            MetaValue::new("create_date", create_date.unwrap_or_default()),
                            MetaValue::new("modify_date", modify_date.unwrap_or_default()),
                        ],
                    ));
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        anyhow::bail!("query precedure contains no result.")
    }
    async fn drop_procedure(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!(" DROP PROCEDURE dbo.{name}");
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        conn.execute(&sql, &[]).await?;
        anyhow::Ok(())
    }
}

impl SqlServer {
    async fn query_count(
        conn: &mut Connection, name: &str, where_str: Option<&str>,
    ) -> anyhow::Result<i32> {
        let mut stream = conn
            .query(
                format!(
                    "SELECT count(*) from {name} {}",
                    match where_str {
                        Some(x) => format!("  where {} ", x),
                        None => "".to_string(),
                    }
                ),
                &[],
            )
            .await?;

        let mut count: Option<i32> = None;
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    count = row.get(0);
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        anyhow::Ok(count.unwrap_or_default())
    }
}
impl SqlServer {
    async fn is_primary(
        conn: &mut Connection, table_name: &str, col_name: &str,
    ) -> anyhow::Result<bool> {
        let s = format!(
            r#"
        SELECT count(*)
        FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE
        WHERE TABLE_NAME = '{table_name}' AND COLUMN_NAME = '{col_name}' AND CONSTRAINT_NAME LIKE 'PK_%'
        "#
        );
        let mut count: Option<i32> = None;
        let mut stream = conn.query(s.as_str(), &[]).await?;
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(x) => {
                    count = x.get(0);
                    break;
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }

        let d = count.map(|x| x > 0).unwrap_or_default();
        anyhow::Ok(d)
    }

    async fn is_auto_increment(
        conn: &mut Connection, table_name: &str, col_name: &str,
    ) -> anyhow::Result<bool> {
        let s = format!(
            r#"
            SELECT COLUMN_NAME
            FROM INFORMATION_SCHEMA.COLUMNS
            WHERE TABLE_NAME = '{table_name}' AND COLUMNPROPERTY(object_id(TABLE_SCHEMA + '.{table_name}' ), COLUMN_NAME, 'IsIdentity') = 1
        "#
        );
        let mut name: Option<String> = None;
        let mut stream = conn.query(s.as_str(), &[]).await?;
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(x) => {
                    name = x.get::<'_, &str, _>(0).map(|x| x.to_string());

                    break;
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        let d = name.map(|x| x.as_str() == col_name).unwrap_or_default();
        anyhow::Ok(d)
    }
}

struct ExtractValueAsJson {
    value: serde_json::Value,
}

impl<'a> FromSql<'a> for ExtractValueAsJson {
    fn from_sql(value: &'a ColumnData<'static>) -> tiberius::Result<Option<Self>> {
        let value = match value {
            tiberius::ColumnData::U8(x) => x.clone().map(serde_json::Value::from),
            tiberius::ColumnData::I16(x) => x.clone().map(serde_json::Value::from),
            tiberius::ColumnData::I32(x) => x.clone().map(serde_json::Value::from),
            tiberius::ColumnData::I64(x) => x.clone().map(serde_json::Value::from),
            tiberius::ColumnData::F32(x) => x.clone().map(serde_json::Value::from),
            tiberius::ColumnData::F64(x) => x.clone().map(serde_json::Value::from),
            tiberius::ColumnData::Bit(x) => x.clone().map(serde_json::Value::from),
            tiberius::ColumnData::String(x) => x
                .as_ref()
                .map(|x| x.to_string())
                .map(serde_json::Value::from),
            tiberius::ColumnData::Guid(x) => x
                .as_ref()
                .map(|x| x.to_string())
                .map(serde_json::Value::from),
            tiberius::ColumnData::Binary(x) => x
                .as_ref()
                .map(|x| format!("0X{}", hex::encode_upper(x)))
                .map(serde_json::Value::from),
            tiberius::ColumnData::Numeric(x) => x
                .as_ref()
                .map(|x| x.to_string())
                .map(serde_json::Value::from),

            tiberius::ColumnData::Xml(x) => x
                .as_ref()
                .map(|x| x.to_string())
                .map(serde_json::Value::from),
            tiberius::ColumnData::DateTime(_)
            | tiberius::ColumnData::DateTime2(_)
            | tiberius::ColumnData::SmallDateTime(_) => chrono::NaiveDateTime::from_sql(value)
                .ok()
                .flatten()
                .map(|x| x.to_string())
                .map(serde_json::Value::from),

            tiberius::ColumnData::Date(_) => chrono::NaiveTime::from_sql(value)
                .ok()
                .flatten()
                .map(|x| x.to_string())
                .map(serde_json::Value::from),

            tiberius::ColumnData::Time(_x) => chrono::NaiveTime::from_sql(value)
                .ok()
                .flatten()
                .map(|x| x.to_string())
                .map(serde_json::Value::from),

            tiberius::ColumnData::DateTimeOffset(_) => {
                chrono::DateTime::<FixedOffset>::from_sql(value)
                    .ok()
                    .flatten()
                    .map(|x| format_data_time_fix_offset(&x))
                    .map(serde_json::Value::from)
            }
        };

        Ok(value.map(|x| ExtractValueAsJson { value: x }))
    }
}
fn encode_row_as_json(row: &Row) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    let mut values = serde_json::Map::new();
    for c in 0..row.len() {
        let value: Option<ExtractValueAsJson> = row.try_get(c)?;
        let value = value.map(|x| x.value);
        values.insert(
            row.columns()[c].name().to_string(),
            value.unwrap_or(serde_json::Value::Null),
        );
    }
    anyhow::Ok(values)
}

impl SqlServer {
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
        let conn = conn.deref_mut();
        let cols = SchemaTrait::show_columns(self, table_name).await?;
        let sql = format!("SELECT * from {}", table_name);

        let mut stream = conn.query(sql.as_str(), &[]).await?.try_chunks(100);
        while let Some(rows) = stream.next().await {
            let rows = rows?;
            let mut rows_row = Vec::with_capacity(rows.len());
            for r in rows.into_iter() {
                match r {
                    tiberius::QueryItem::Row(x) => rows_row.push(x),
                    tiberius::QueryItem::Metadata(_) => {}
                }
            }
            let data = encode_rows(&rows_row)?;
            for d in data.into_iter() {
                let mut m = HashMap::new();
                for (d, f) in d.into_iter().zip(cols.iter()) {
                    m.insert(f.field.clone(), d);
                }

                let insert_sql = (self as &dyn SchemaTrait).insert_sql(
                    self.config.schema.as_ref().map(|x| x.as_str()),
                    table_name,
                    &cols,
                    &m,
                )?;
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
        let conn = conn.deref_mut();

        let mut stream = conn.query(sql, &[]).await?;

        while let Some(row) = stream.next().await {
            let row = row?;
            let row = match row {
                tiberius::QueryItem::Row(r) => r,
                tiberius::QueryItem::Metadata(_) => {
                    continue;
                }
            };
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
        self.export_query_result_as_json_line(&sql, sender).await
    }

    async fn export_query_result_as_json(
        &self, sql: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();

        let mut stream = conn.query(sql, &[]).await?;

        sender.send("[".into()).await?;
        let mut first = true;
        while let Some(row) = stream.next().await {
            let row = row?;
            let row = match row {
                tiberius::QueryItem::Row(r) => r,
                tiberius::QueryItem::Metadata(_) => {
                    continue;
                }
            };
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
        self.export_query_result_as_json(&sql, pretty, sender).await
    }

    async fn export_query_result_as_csv(
        &self, sql: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut first = true;

        let mut stream = conn.query(sql, &[]).await?.try_chunks(100);
        while let Some(rows) = stream.next().await {
            let rows = rows?;

            let mut rows_row = Vec::new();
            for r in rows.into_iter() {
                match r {
                    tiberius::QueryItem::Row(r) => {
                        rows_row.push(r);
                    }
                    tiberius::QueryItem::Metadata(meta) => {
                        if first && include_header {
                            let fs = meta
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
                    }
                }
            }

            if rows_row.len() > 0 {
                let s = create_csv(&encode_rows(&rows_row)?, delimiter);
                sender.send(Cow::Owned(s)).await?;
            }
        }

        anyhow::Ok(())
    }

    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from {}", table_name);
        self.export_query_result_as_csv(&sql, include_header, delimiter, sender)
            .await
    }
}

impl SqlServer {
    /// this fn create a `Connection` only use for query connection
    /// because `query_connections` use in `SqlServer`,
    /// we can't create self reference struct in rust.
    async fn new_tcp_conn(&self) -> anyhow::Result<Connection> {
        let config = make_config(&self.config);
        let tcp = TcpStream::connect(config.get_addr()).await?;
        tcp.set_nodelay(true)?;

        let client = tiberius::Client::connect(config, tcp.compat_write()).await?;
        // if let Some(s) = self.config.schema.as_ref() {
        //     set_current_schema(&mut client, s).await?;
        // }

        anyhow::Ok(client)
    }
}

type Connection = Client<Compat<TcpStream>>;
#[async_trait::async_trait]
impl QuerySql for SqlServer {
    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        is_dangerous_sql_by_parser(sql, &sqlparser::dialect::GenericDialect {})
    }
    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let conn = self.new_tcp_conn().await?;

        let id = uuid::Uuid::new_v4();
        self.query_connections
            .insert(id.clone(), Arc::new(Mutex::new(conn)));
        anyhow::Ok(id)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        let conn = self.new_tcp_conn().await?;
        self.query_connections
            .insert(id.clone(), Arc::new(Mutex::new(conn)));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        QueryLog::remove(&id);

        self.query_connections.remove(&id);
        anyhow::Ok(())
    }

    async fn query_sql(
        &self, id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let mut query_log = QueryLog::get_guard(id, sql);
        let conn = match self.query_connections.get(&id) {
            Some(x) => x.clone(),
            None => anyhow::bail!(format!("id connection '{}' not opened", id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();

        let mut ret = Vec::with_capacity(0);

        let mut stream = conn.simple_query(sql).await?;
        let mut meta_prev = None;
        let mut rows = Vec::new();
        while let Some(item) = stream.next().await {
            MemoryLimit::exceed_error()?;

            let item = match item {
                Ok(item) => item,
                Err(err) => {
                    ret.push(Err(err.to_string()));
                    continue;
                }
            };
            match item {
                tiberius::QueryItem::Row(row) => rows.push(row),
                tiberius::QueryItem::Metadata(meta) => {
                    if meta_prev.is_some() {
                        let table_list = make_table_list_from_meta_and_row(
                            meta_prev.as_ref().unwrap(),
                            &rows,
                            rows.len(),
                            "".to_string(),
                        )?;
                        ret.push(Ok(table_list.to_query_resp()));
                        rows = vec![];
                    }
                    meta_prev = Some(meta);
                }
            }
            // last one
            if meta_prev.is_some() {
                let table_list = make_table_list_from_meta_and_row(
                    meta_prev.as_ref().unwrap(),
                    &rows,
                    rows.len(),
                    "",
                )?;
                ret.push(Ok(table_list.to_query_resp()));
            }
        }
        query_log.set_success(ret.iter().all(|x| x.is_ok()));
        anyhow::Ok(ret)
    }
}

fn extract_trigger_event_from_sql(sql: &str) -> anyhow::Result<TriggerEvent> {
    extract_trigger_event_base_on_lexer(sql)
}

// todo how to set current schema for sql server
#[allow(dead_code)]
async fn set_current_schema(conn: &mut Connection, schema: &str) -> anyhow::Result<()> {
    let sql = format!("SET CURRENT_SCHEMA = '{schema}'");
    println!("sql:{}", sql);
    conn.simple_query(sql.as_str()).await.unwrap();
    anyhow::Ok(())
}

impl SqlServer {
    async fn get_conn<'a>(
        &'a self,
    ) -> anyhow::Result<bb8::PooledConnection<'a, bb8_tiberius::ConnectionManager>> {
        let conn = self.pool.get().await?;
        // if let Some(s) = self.config.schema.as_ref() {
        //     set_current_schema(conn.deref_mut(), s).await?;
        // }
        anyhow::Ok(conn)
    }

    #[allow(unused)]
    async fn with_conn<R>(
        &self,
        call_back: impl FnOnce(&mut Client<Compat<TcpStream>>) -> BoxFuture<anyhow::Result<R>>,
    ) -> anyhow::Result<R> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        call_back(conn).await
    }
}

#[allow(dead_code)]
fn get_sql_seq(sql: &str) -> anyhow::Result<Vec<SqlSeq>> {
    let dialect = sqlparser::dialect::MsSqlDialect {};

    sql_to_sql_seq(
        sql,
        &dialect,
        |s| SqlKind::get_from_statement(s),
        |x| SqlKind::get_from_sql(x),
    )
}

#[async_trait::async_trait]
impl crate::db::DBTrait for SqlServer {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec!["dbo".to_string()])
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut x = Clone::clone(self);
        x.config.schema = Some(name.to_owned());
        x.query_connections = Default::default();
        Ok(Arc::new(x))
    }
}

#[async_trait::async_trait]
impl AutoCompletionTrait for SqlServer {
    async fn auto_completion(
        &self, _uuid: uuid::Uuid, _sql: &str, _line: u32, _col: u32, _use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        // DBLexerCompletionBackend::do_auto_completion(
        //     self,
        //     sql,
        //     &sqlparser::dialect::MsSqlDialect {},
        //     pos,
        // )
        // .await
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl CatalogTrait for SqlServer {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let mut stream = conn
            .deref_mut()
            .simple_query(
                r#"
            SELECT name 
            FROM sys.databases
        "#,
            )
            .await?;
        let mut ret = Vec::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    let name: Option<&str> = row.get(0);
                    ret.push(name.expect("db name can be null").to_owned());
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }

        anyhow::Ok(ret)
    }
    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        let mut config = self.config.clone();
        config.db_name = Some(name.to_owned());
        let x = Self::new(config).await?;
        anyhow::Ok(Arc::new(x))
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{enable_test_log, TEST_HOST};

    async fn make_test_object() -> SqlServer {
        let ss = SqlServer::new(SqlServerDbConfig {
            host: TEST_HOST.to_string(),
            port: 1433,
            username: "sa".to_string(),
            password: "sfdewRE123".to_string(),
            db_name: Some("test".to_string()),
            trust_cert: true,
            schema: Some("dbo".to_string()),
            read_only: false,
        })
        .await
        .unwrap();
        ss
    }

    #[tokio::test]
    async fn test_procedure() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_procedure("test_procedure").await;
        db.query_sql(
            query_id,
            r#"
            CREATE PROCEDURE dbo.test_procedure
                @EmployeeID INT
            AS
            BEGIN
                SET NOCOUNT ON;  -- 防止影响行计数的消息

                SELECT 
                    Name, 
                    Position
                FROM 
                    Employees
                WHERE 
                    EmployeeID = @EmployeeID;
            END;
        "#,
        )
        .await
        .unwrap();
        let list = db.list_procedure().await.unwrap();
        for l in list.iter() {
            let details = db.show_procedure(&l.name).await.unwrap();
            println!("udf:{},details:{:?}", l.name, details);
        }
    }

    #[tokio::test]
    async fn test_udf() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_udf("test_udf", None).await;
        db.query_sql(
            query_id,
            r#"
            CREATE FUNCTION dbo.test_udf
            (
                @input INT
            )
            RETURNS INT
            AS
            BEGIN
                RETURN @input * @input;
            END;
        "#,
        )
        .await
        .unwrap();
        let list = db.list_udf().await.unwrap();
        for l in list.iter() {
            let details = db.show_udf(&l.name).await.unwrap();
            println!("udf:{},details:{:?}", l.name, details);
        }
    }

    #[test]
    fn test_extract_trigger_base_on_lexer() {
        {
            let sql = "after insert";
            assert_eq!(
                extract_trigger_event_base_on_lexer(sql).unwrap(),
                TriggerEvent::Insert
            );
        }
        {
            let sql = "after update";
            assert_eq!(
                extract_trigger_event_base_on_lexer(sql).unwrap(),
                TriggerEvent::Update
            );
        }
        {
            let sql = "after delete";
            assert_eq!(
                extract_trigger_event_base_on_lexer(sql).unwrap(),
                TriggerEvent::Delete
            );
        }

        // error cases
        {
            let sql = "xxx";
            assert!(extract_trigger_event_base_on_lexer(sql).is_err());
        }
        {
            let sql = "yyy";
            assert!(extract_trigger_event_base_on_lexer(sql).is_err());
        }
    }

    #[test]
    fn test_extract_trigger_detail_from_sql() {
        {
            let x = extract_trigger_event_from_sql(
                r#"
             CREATE TRIGGER tr_test_create_trigger_insert
            ON test_create_trigger
            AFTER INSERT
            AS
            BEGIN
                INSERT INTO test_create_trigger_update (id, email)
                SELECT id, 'test@example.com' FROM inserted
            END
        "#,
            )
            .unwrap();
            assert_eq!(x, TriggerEvent::Insert);
        }
    }

    // when I test via `cargo test`
    // sometimes I have this error
    // ERROR - Transaction (Process ID 76) was deadlocked on lock resources with another process and has been chosen as the deadlock victim. Rerun the transaction. code=1205
    // maybe I need mutex to run one test a time.
    fn test_lock() -> &'static Mutex<()> {
        static MUTEX: Mutex<()> = Mutex::const_new(());
        &MUTEX
    }

    #[tokio::test]
    async fn test_server_metas() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }

    #[tokio::test]
    async fn test_list_database() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let dbs = CatalogTrait::list_database(&db).await.unwrap();
        println!("{:?}", dbs);
        assert_eq!(dbs, vec!["master", "tempdb", "model", "msdb", "test"]);
    }

    #[tokio::test]
    async fn test_rename() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let db_trait = &db as &dyn SchemaTrait;
        let query_id = db_trait.open_connection().await.unwrap();
        let _ = db_trait.drop_table("test_rename").await;
        let _ = db_trait.drop_table("test_rename2").await;
        db_trait
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_rename (
                    id INT IDENTITY(1,1) PRIMARY KEY,
                    name NVARCHAR(255) NOT NULL,
                    created_at DATETIME DEFAULT GETDATE()
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

    #[tokio::test]
    async fn test_duplicate() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let db_trait = &db as &dyn SchemaTrait;
        let query_id = db_trait.open_connection().await.unwrap();
        let _ = db_trait.drop_table("test_duplicate").await;
        let _ = db_trait.drop_table("test_duplicate2").await;

        db_trait
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_duplicate (
                    id INT IDENTITY(1,1) PRIMARY KEY,
                    data NVARCHAR(255) NOT NULL,
                    created_at DATETIME DEFAULT GETDATE()
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

    #[tokio::test]
    async fn test_triggers() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let db = &db as &dyn crate::db::SchemaTrait;
        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_table("test_create_trigger").await;
        let _ = db.drop_table("test_create_trigger_update").await;
        let x = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_create_trigger (
                id INT PRIMARY KEY,
                name VARCHAR(50)
            );
        "#,
            )
            .await
            .unwrap();
        println!("!!!{:?}", x);

        let x = db
            .query_sql(
                query_id,
                r#"
        
        CREATE TABLE test_create_trigger_update (
            id INT PRIMARY KEY,
            email VARCHAR(100)
        );
    "#,
            )
            .await
            .unwrap();
        println!("!!!{:?}", x);
        let x = db
            .query_sql(
                query_id,
                r#"CREATE TRIGGER tr_test_create_trigger_insert
            ON test_create_trigger
            AFTER INSERT
            AS
            BEGIN
                INSERT INTO test_create_trigger_update (id, email)
                SELECT id, 'test@example.com' FROM inserted
            END
        "#,
            )
            .await
            .unwrap();
        println!("!!!{:?}", x);
        let tris = db.list_trigger("test_create_trigger").await.unwrap();
        for tri in tris.iter() {
            let detail = db
                .show_trigger("test_create_trigger", &tri.name)
                .await
                .unwrap();
            db.disable_trigger("test_create_trigger", &tri.name)
                .await
                .unwrap();
            db.enable_trigger("test_create_trigger", &tri.name)
                .await
                .unwrap();

            println!("trigger:{} details:{:?}", tri.name, detail);
        }
    }

    #[tokio::test]
    async fn test_query() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object().await;
        let db = &db as &dyn crate::db::SchemaTrait;
        let id = db.open_connection().await.unwrap();

        let ret = db
            .query_sql(
                id.clone(),
                r#"BEGIN TRANSACTION;
        
            drop table if exists test_query;
            CREATE TABLE test_query (
                id INT PRIMARY KEY,
                name VARCHAR(50),
                age INT
              );

              INSERT INTO test_query (id, name, age) VALUES (1, 'John', 25);
              INSERT INTO test_query (id, name, age) VALUES (2, 'Jane', 30);
              INSERT INTO test_query (id, name, age) VALUES (3, 'Mike', 35);
              COMMIT;
        "#,
            )
            .await
            .unwrap();

        println!("begin ret:{:?}", ret);
    }

    #[tokio::test]
    async fn test_list_schema() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let db = &db as &dyn crate::db::DBTrait;

        let dbnames = db.list_schema().await.unwrap();
        assert_eq!(dbnames, vec!["dbo",]);
        for dbname in dbnames.iter() {
            db.open_schema(&dbname).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_view() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let db = &db as &dyn crate::db::SchemaTrait;
        let _ = db.drop_table("test_view_table").await;
        let _ = db.drop_view("test_view").await;
        let conn_id = db.open_connection().await.unwrap();

        let _ = db
            .query_sql(
                conn_id,
                r#"
            CREATE TABLE test_view_table (
                id INT PRIMARY KEY,
                name VARCHAR(50),
                age INT,
                email VARCHAR(100)
            );
            INSERT INTO test_view_table (id, name, age, email)
                VALUES
                    (1, 'John Doe', 35, 'john.doe@example.com'),
                    (2, 'Jane Smith', 28, 'jane.smith@example.com'),
                    (3, 'Bob Johnson', 42, 'bob.johnson@example.com'),
                    (4, 'Sarah Lee', 31, 'sarah.lee@example.com'),
                    (5, 'Tom Wilson', 25, 'tom.wilson@example.com');
             
        "#,
            )
            .await;

        let _ = db
            .query_sql(
                conn_id,
                r#"CREATE VIEW test_view AS
                SELECT
                    id,
                    name,
                    age,
                    email
                FROM
                    test_view_table;
        "#,
            )
            .await
            .unwrap();

        let resp = db.show_view("test_view", None, None, 1, 100).await.unwrap();
        println!("resp:{:?}", resp);
    }

    #[tokio::test]
    async fn test_index() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let _ = db.drop_table("test_index_table").await;

        let query_id = db.open_connection().await.unwrap();

        let _ = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_index_table (
                        Column1 int,
                        Column2 int,
                    );
                    -- 添加唯一索引
                CREATE UNIQUE INDEX IX_YourTableName_Column1_Column2
                    ON test_index_table (Column1, Column2);
        "#,
            )
            .await
            .unwrap();

        let db = &db as &dyn crate::db::SchemaTrait;
        std::thread::sleep(Duration::from_secs(1));
        let indexes = db.show_indexes("test_index_table").await.unwrap();
        println!("indexes:{:?}", indexes);
        db.drop_table("test_index_table").await.unwrap();
    }

    #[tokio::test]
    async fn test_export() {
        let _guard = test_lock().lock().await;
        let table_name = "test_export";
        let db = make_test_object().await;
        let mut conn = db.get_conn().await.unwrap();
        let conn = conn.deref_mut();
        let rs = vec![
            r#"drop table if exists test_export;"#,
            r#"CREATE TABLE test_export (
                id INT IDENTITY(1,1) PRIMARY KEY,
                name VARCHAR(50),
                age INT,
                email VARCHAR(100)
            );"#,
            "INSERT INTO test_export (name, age, email) VALUES ('John', 25, 'john@example.com');",
            "INSERT INTO test_export (name, age, email) VALUES ('Jane', 30, 'jane@example.com');",
            "INSERT INTO test_export (name, age, email) VALUES ('Bob', 40, 'bob@example.com');",
        ];
        for s in rs.iter() {
            conn.execute(*s, &[]).await.unwrap();
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
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json:{}", s);
        }
        {
            let s = db
                .export_table_to_string(
                    table_name,
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
        println!("################################");

        let sql = format!("select * from {}", table_name);
        let sql = sql.as_str();
        {
            let s = db
                .export_query_result_to_string(sql, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json:{}", s);
        }
        {
            let s = db
                .export_query_result_to_string(sql, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", s);
            let _: serde_json::Value = serde_json::from_str(s.as_str()).unwrap();
        }
        {
            let s = db
                .export_query_result_to_string(
                    sql,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", s);
        }
    }
    #[tokio::test]
    async fn test_list_table() {
        let _guard = test_lock().lock().await;
        // list all table and query
        let pg = make_test_object().await;

        let pg = &pg as &dyn SchemaTrait;
        let table = pg.list_table().await.unwrap();
        println!("{:?}", table);
    }

    #[tokio::test]
    async fn test_type1() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object().await;
        let mut conn = db.get_conn().await.unwrap();
        let conn = conn.deref_mut();
        let table_name = "test_type1";
        let _ = conn.execute(format!("drop table {table_name}"), &[]).await;

        conn.execute(
            format!(
                r#"
            CREATE TABLE {table_name} (
                id INT IDENTITY(1,1) PRIMARY KEY,
                col_tinyint TINYINT,
                col_smallint SMALLINT,
                col_int INT,
                col_bigint BIGINT,
                col_float FLOAT(24),
                col_double FLOAT(53)
            );
            "#
            ),
            &[],
        )
        .await
        .unwrap();
        let db = &db as &dyn crate::db::SchemaTrait;
        let mut values = HashMap::new();
        values.insert("col_tinyint".to_string(), Some("1".to_string()));
        values.insert("col_smallint".to_string(), Some("100".to_string()));
        values.insert("col_int".to_string(), Some("1000".to_string()));
        values.insert("col_bigint".to_string(), Some("10000".to_string()));
        values.insert("col_float".to_string(), Some("3.14".to_string()));
        values.insert("col_double".to_string(), Some("3.14159".to_string()));
        db.insert(table_name, values.clone()).await.unwrap();
        db.update(table_name, values, {
            let mut wheres = HashMap::new();
            wheres.insert("id".to_string(), Some("1".to_string()));
            wheres
        })
        .await
        .unwrap();
        let list = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("ret:{:?}", list);
    }

    #[tokio::test]
    async fn test_type2() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object().await;
        let mut conn = db.get_conn().await.unwrap();

        let conn = conn.deref_mut();
        let table_name = "test_type2";
        let _ = conn.execute(format!("drop table {table_name}"), &[]).await;

        conn.execute(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    id INT IDENTITY(1,1) PRIMARY KEY,
                    bit_column bit,
                    nvarchar_column nvarchar(50),
                    varchar_column varchar(50),
                    nchar_column nchar(50),
                    char_column char(50),
                    ntext_column ntext,
                    text_column text
                  );
            "#
            ),
            &[],
        )
        .await
        .unwrap();
        let db = &db as &dyn crate::db::SchemaTrait;
        let mut values = HashMap::new();
        values.insert("bit_column".to_string(), Some("1".to_string()));
        values.insert("nvarchar_column".to_string(), Some("b".to_string()));
        values.insert("varchar_column".to_string(), Some("c".to_string()));

        values.insert("nchar_column".to_string(), Some("d".to_string()));
        values.insert("char_column".to_string(), Some("e".to_string()));
        values.insert("ntext_column".to_string(), Some("f".to_string()));
        values.insert("text_column".to_string(), Some("g".to_string()));

        db.insert(table_name, values.clone()).await.unwrap();
        db.update(table_name, values, {
            let mut wheres = HashMap::new();
            wheres.insert("id".to_string(), Some("1".to_string()));
            wheres
        })
        .await
        .unwrap();
        let list = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("ret:{:?}", list);
    }

    #[tokio::test]
    async fn test_type3() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object().await;
        let mut conn = db.get_conn().await.unwrap();
        let conn = conn.deref_mut();
        let table_name = "test_type3";
        let _ = conn.execute(format!("drop table {table_name}"), &[]).await;

        conn.execute(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    id INT IDENTITY(1,1) PRIMARY KEY,
                    BinaryColumn binary(10),
                    VarBinaryColumn varbinary(50),
                    ImageColumn image,
                    UniqueIdentifierColumn uniqueidentifier,
                    NumericColumn numeric(10, 2),
                    DecimalColumn decimal(12, 4)
                  );
            "#
            ),
            &[],
        )
        .await
        .unwrap();
        let db = &db as &dyn crate::db::SchemaTrait;
        let mut values = HashMap::new();
        values.insert("BinaryColumn".to_string(), Some("0x0123456789".to_string()));
        values.insert("VarBinaryColumn".to_string(), Some("0xABCDEF".to_string()));
        values.insert(
            "ImageColumn".to_string(),
            Some("0x0123456789ABCDEF".to_string()),
        );

        values.insert(
            "UniqueIdentifierColumn".to_string(),
            Some("NEWID()".to_string()),
        );
        values.insert("NumericColumn".to_string(), Some("123.45".to_string()));
        values.insert("DecimalColumn".to_string(), Some("12.3456".to_string()));
        db.insert(table_name, values.clone()).await.unwrap();
        db.update(table_name, values, {
            let mut wheres = HashMap::new();
            wheres.insert("id".to_string(), Some("1".to_string()));
            wheres
        })
        .await
        .unwrap();
        let list = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("ret:{:?}", list);
    }

    #[tokio::test]
    async fn test_type4() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object().await;
        let mut conn = db.get_conn().await.unwrap();

        let conn = conn.deref_mut();
        let table_name = "test_type4";
        let _ = conn.execute(format!("drop table {table_name}"), &[]).await;

        conn.execute(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    id INT IDENTITY(1,1) PRIMARY KEY,
                    XmlData XML,
                    DateTimeData DATETIME,
                    DateTime2Data DATETIME2,
                    SmallDateTimeData SMALLDATETIME,
                    DateData DATE,
                    TimeData TIME,
                    DateTimeOffsetData DATETIMEOFFSET
                );
            "#
            ),
            &[],
        )
        .await
        .unwrap();
        let db = &db as &dyn crate::db::SchemaTrait;
        let mut values = HashMap::new();
        values.insert(
            "XmlData".to_string(),
            Some("<root><data>XML Data</data></root>".to_string()),
        );
        values.insert(
            "DateTimeData".to_string(),
            Some("2022-01-01 12:34:56".to_string()),
        );
        values.insert(
            "DateTime2Data".to_string(),
            Some("2022-01-01 12:34:56.1234567".to_string()),
        );
        values.insert(
            "SmallDateTimeData".to_string(),
            Some("2022-01-01 12:34:00".to_string()),
        );

        values.insert("DateData".to_string(), Some("2022-01-01".to_string()));
        values.insert("TimeData".to_string(), Some("12:34:56".to_string()));
        values.insert(
            "DateTimeOffsetData".to_string(),
            Some("2022-01-01 12:34:56.1234567 +01:00".to_string()),
        );
        db.insert(table_name, values.clone()).await.unwrap();
        db.update(table_name, values, {
            let mut wheres = HashMap::new();
            wheres.insert("id".to_string(), Some("1".to_string()));
            wheres
        })
        .await
        .unwrap();
        let list = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("ret:{:?}", list);
    }
}
