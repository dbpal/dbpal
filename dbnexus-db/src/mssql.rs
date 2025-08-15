use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;

use chrono::FixedOffset;
use dashmap::DashMap;
use futures_core::future::BoxFuture;
use futures_util::{StreamExt, TryStreamExt};
use sqlparser::tokenizer::Token;
use tiberius::{Client, ColumnData, FromSql, Row};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

use crate::MemoryLimit;
#[cfg(test)]
use crate::TEST_HOST;
use crate::datasource_config::SqlServerDBConfig;
use crate::db::{
    BackendKind, CatalogTrait, CheckConn, ColumnType, CreateDatabaseExtra, CreateSchemaExtra,
    DBTrait, DefAndMetas, ExportKind, ExportQueryResult, Field, GetBackendKind, Index, IndexKind,
    ListTable, NamedStaff, ProcedureDetail, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait,
    ServerMetas, ShowExtraArgs, SqlBuilder, SqlExecuteRet, TriggerDetail, TriggerEnable,
    TriggerEvent, TriggerTiming, UdfOrProcedureExtraParameter, escape_sql_str,
    format_data_time_fix_offset, not_support_feature,
};
use crate::helper::{TXExecuteTimes, create_csv, is_dangerous_sql_by_parser};

use crate::meta_value::MetaValue;
use crate::query_log::{QueryLogEvent, QueryLogMap};

#[derive(Clone)]
pub struct MsSqlServerDB {
    pool: bb8::Pool<bb8_tiberius::ConnectionManager>,
    pub(crate) config: SqlServerDBConfig,
    query_logs: QueryLogMap,
    query_connections: Arc<DashMap<uuid::Uuid, Arc<Mutex<Connection>>>>,
    tx_execute_times: TXExecuteTimes,
}

impl std::fmt::Debug for MsSqlServerDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mssql on:{:?}", self.config)
    }
}

impl MsSqlServerDB {
    pub const ONLY_SCHEMA: &str = "dbo";
}
#[async_trait::async_trait]
impl ServerMetas for MsSqlServerDB {
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
                m.first().unwrap().as_ref().unwrap().to_string(),
                m[1].clone(),
            ));
        }

        anyhow::Ok(ret)
    }
}
impl MsSqlServerDB {
    pub async fn new(c: SqlServerDBConfig) -> anyhow::Result<Self> {
        let manager = bb8_tiberius::ConnectionManager::new(make_config(&c));
        let pool = bb8::Pool::builder().max_size(50).build(manager).await?;
        anyhow::Ok(Self {
            config: c,
            pool,
            query_connections: Default::default(),
            query_logs: Default::default(),
            tx_execute_times: Default::default(),
        })
    }
}

fn make_config(c: &SqlServerDBConfig) -> tiberius::Config {
    let mut config = tiberius::Config::new();
    config.host(&c.host);
    config.port(c.port);
    if let Some(user) = &c.username {
        if let Some(pass) = &c.password {
            config.authentication(tiberius::AuthMethod::sql_server(user, pass));
        }
    }

    if let Some(db) = c.db.as_ref() {
        config.database(db);
    }

    if c.trust_cert {
        config.trust_cert(); // on production, it is not a good idea to do this
    }
    config
}

fn get_column_type_string(c: tiberius::ColumnType) -> String {
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
) -> anyhow::Result<QueryResp> {
    let fields = get_fields_from_meta(meta);
    let data = encode_rows(rows)?;

    anyhow::Ok(QueryResp::new(fields, total, data, Some(sql.into()))?)
}

fn encode_rows(rows: &Vec<tiberius::Row>) -> anyhow::Result<Vec<Vec<Option<String>>>> {
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
    let mut sql = format!(r#"SELECT * FROM "{schema}"."{table_name}""#);
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
    let t = sqlparser::tokenizer::Tokenizer::new(&sqlparser::dialect::SQLiteDialect {}, sql)
        .tokenize()?;
    let mut event = None;

    for token in t {
        if let Token::Word(x) = token {
            let word = x.to_string().to_uppercase();
            match word.as_str() {
                "INSERT" | "UPDATE" | "DELETE" => {
                    event = Some(word);
                }

                _ => {}
            }
        }
    }
    if event.is_none() {
        anyhow::bail!("trigger statement is not valid");
    }
    let event = event.unwrap();

    anyhow::Ok(TriggerEvent::try_from(event.as_str())?)
}

#[async_trait::async_trait]
impl CheckConn for MsSqlServerDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let _x = conn.query("select 1", &[]).await?;
        anyhow::Ok(ins.elapsed())
    }
}

impl MsSqlServerDB {
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

impl MsSqlServerDB {
    pub(crate) async fn list_table_impl(
        &self, db: &str, schema: &str,
    ) -> anyhow::Result<Vec<ListTable>> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream = conn
            .query(
                format!(
                    r#"
            SELECT TABLE_NAME
            FROM INFORMATION_SCHEMA.TABLES
            WHERE TABLE_TYPE = 'BASE TABLE' AND TABLE_CATALOG='{}' AND TABLE_SCHEMA='{}' ORDER BY TABLE_NAME ;
        "#,
                    db, schema
                )
                .as_str(),
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

    pub(crate) async fn list_procedure_impl(
        &self, db: &str, schema: &str,
    ) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
            SELECT 
                ROUTINE_NAME 
            FROM 
                INFORMATION_SCHEMA.ROUTINES 
            WHERE 
                ROUTINE_TYPE = 'PROCEDURE'  AND SPECIFIC_CATALOG='{}' AND  SPECIFIC_SCHEMA='{}';
            "#,
            db, schema
        );
        log::info!("sql:{}", sql);
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
}

impl GetBackendKind for MsSqlServerDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::SqlServer
    }
}

fn type_support_compare(_tpe: &str) -> bool {
    true
}

#[async_trait::async_trait]
impl SchemaTrait for MsSqlServerDB {
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
        conn.execute(&sql, &[]).await?;
        anyhow::Ok(())
    }

    async fn drop_index(&self, table_name: &str, index: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP INDEX "{index}" ON "{table_name}";"#);
        self.execute_simple_execute(&sql).await
    }

    async fn rename_index(
        &self, table: &str, old_name: &str, new_name: &str,
    ) -> anyhow::Result<()> {
        let sql = format!("EXEC sp_rename '{table}.{old_name}', '{new_name}', 'INDEX';");
        log::info!("sql:{}", sql);
        self.execute_simple_execute(&sql).await
    }

    async fn add_table_column(
        &self, table: &str, name: &str, col: ColumnType,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"ALTER TABLE "{table}" ADD "{name}" {} {} {}"#,
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
        log::info!("sql:{}", sql);

        let mut conn = self.get_conn().await?;
        conn.deref_mut().execute(&sql, &[]).await?;
        anyhow::Ok(())
    }

    fn list_data_types(&self) -> anyhow::Result<&'static [&'static str]> {
        anyhow::Ok(crate::mysql_lang_server::TYPES)
    }

    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, typ: Option<crate::db::ColumnType>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        if old_name != new_name {
            let sql = format!(
                r#"
            EXEC sp_rename '{table}.{old_name}', '{new_name}', 'COLUMN';
            "#
            );
            log::info!("sql:{}", sql);
            conn.deref_mut().execute(&sql, &[]).await?;
        }
        if let Some(typ) = typ.as_ref() {
            let sql = format!(
                r#"
            ALTER TABLE {table} ALTER COLUMN {new_name} {} {};
            "#,
                typ.typ,
                if let Some(null) = typ.nullness.as_ref() {
                    null.as_str()
                } else {
                    ""
                }
            );
            log::info!("sql:{}", sql);
            conn.deref_mut().execute(&sql, &[]).await?;
        }
        anyhow::Ok(())
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
            ALTER TABLE "{table}" DROP COLUMN "{col}";
        "#
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        conn.execute(&sql, &[]).await?;
        anyhow::Ok(())
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        if self.table_exists(dup).await? {
            anyhow::bail!(format!("table '{}' already exists", dup));
        }
        let schema = self.config.must_schema().as_str();
        let sql = format!(
            r#"
            SELECT * INTO "{schema}"."{dup}" FROM "{schema}"."{table}" 
        "#
        );
        log::info!("sql:{}", sql);
        self.execute_simple_execute(sql.as_str()).await
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
            EXEC sp_rename '{old}', '{new}';
        "#
        );
        log::info!("sql:{}", sql);
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
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream = conn.simple_query(&sql).await?;
        let mut ret = Vec::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(x) => {
                    let name: Option<&str> = x.try_get(0)?;
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
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream = conn.simple_query(&sql).await?;
        let mut result = None;
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(x) => {
                    let name: Option<bool> = x.try_get(0)?;
                    let is_instead_of_trigger: Option<bool> = x.try_get(1)?;
                    let sql: Option<&str> = x.try_get(2)?;
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
            name,
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
            r#"
         DROP TRIGGER "{}"."{name}" ON "{table}";
        "#,
            Self::ONLY_SCHEMA
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();

        conn.execute(sql.as_str(), &[]).await?;

        anyhow::Ok(())
    }

    async fn export_table_or_view(
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

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let total = MsSqlServerDB::query_count(conn, view_name, where_str).await?;
        let sql = gen_select_sql(
            self.config.must_schema().as_str(),
            view_name,
            where_str,
            order_by,
            page,
            page_size,
        );
        log::info!("sql:{}", sql);
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
            Some(meta) => make_table_list_from_meta_and_row(&meta, &rows, total as usize, sql)?,
            None => anyhow::bail!("SELECT should yield at lease one meta"),
        };
        anyhow::Ok(ret)
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let schema = self.config.must_schema().as_str();

        let s = format!(r#"DROP VIEW "{schema}"."{view_name}""#);
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();

        conn.execute(s.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"DROP TABLE "{table}""#);
        log::info!("sql:{}", sql);
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
        self.list_table_impl(
            self.config.must_db().as_str(),
            self.config.must_schema().as_str(),
        )
        .await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
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
                    let name: Option<&str> = row.try_get(0)?;
                    let name = name.unwrap().to_string();
                    let data_type: Option<&str> = row.try_get(1)?;
                    let data_type = data_type.unwrap().to_string();
                    let default_value: Option<&str> = row.try_get(2)?;
                    let nullable: Option<&str> = row.try_get(3)?;
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
            let is_primary = MsSqlServerDB::is_primary(conn, table_name, &f.field).await?;
            let is_auto_increment =
                MsSqlServerDB::is_auto_increment(conn, table_name, &f.field).await?;
            f.is_primary = is_primary;
            f.is_auto_increment = is_auto_increment;
        }

        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let total = MsSqlServerDB::query_count(conn, table_name, where_str).await?;
        let sql = gen_select_sql(
            self.config.must_schema().as_str(),
            table_name,
            where_str,
            order_by,
            page,
            page_size,
        );

        let mut stream = conn
            .query(&sql, &[])
            .await
            .map_err(|e| anyhow::Error::msg(format!("err:'{}' sql:'{}'", e, sql)))?;
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
        let cols = self.show_table_columns(table_name).await?;
        let mut ret = match meta {
            Some(x) => make_table_list_from_meta_and_row(&x, &rows, total as usize, sql)?,
            None => anyhow::bail!("SELECT should yield at lease one meta"),
        };
        // infered schema is accurate as show_table_columns
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
        log::info!("sql:{}", sql);

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
                        x.columns.push(col);
                    } else {
                        ret.insert(
                            name.clone(),
                            Index {
                                name: Some(name),
                                kind: if is_unique.unwrap_or_default() {
                                    IndexKind::Unique
                                } else {
                                    IndexKind::Index
                                },
                                columns: vec![col],
                            },
                        );
                    }
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        anyhow::Ok(ret.into_values().collect())
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fs = self.show_table_columns(table_name).await?;
        let s = self.insert_sql(self.config.schema.as_deref(), table_name, &fs, &values)?;

        log::trace!("insert sql:{}", s);

        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let _r = conn.execute(s.as_str(), &[]).await?;
        anyhow::Ok(SqlExecuteRet {
            sql: s,
            changes: Some(1), // I know this
        })
    }

    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fs = self.show_table_columns(table_name).await?;
        let s = self.update_sql(
            self.config.schema.as_deref(),
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

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let s = format!(
            r#"
            TRUNCATE TABLE "{table_name}";
        "#
        );
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        conn.execute(s.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn table_delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fields = self.show_table_columns(table_name).await?;
        let sql = self.delete_sql(self.config.schema.as_deref(), table_name, &fields, &wheres)?;
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();

        let r = conn.execute(sql.as_str(), &[]).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(r.rows_affected().iter().sum()),
        })
    }

    async fn disable_trigger(&self, table: &str, name: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();

        let sql = format!("ENABLE TRIGGER {name} ON {table}");
        log::info!("sql:{}", sql);
        conn.execute(sql.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn enable_trigger(&self, table: &str, name: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();

        let sql = format!("DISABLE TRIGGER {name} ON {table}");
        log::info!("sql:{}", sql);
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
                type IN ('FN', 'IF', 'TF') AND  SCHEMA_NAME(schema_id) = '{}';
        "#,
            Self::ONLY_SCHEMA
        );
        log::info!("sql:{}", sql);
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

    async fn show_udf(
        &self, name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<DefAndMetas> {
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
        log::info!("sql:{}", sql);
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
        log::info!("sql:{}", sql);
        let mut resp = conn.simple_query(&sql).await?;

        while let Some(item) = resp.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    let def: Option<&str> = row.try_get(0)?;
                    return anyhow::Ok(DefAndMetas::new(
                        def.unwrap_or_default().to_string(),
                        metas,
                    ));
                }
                tiberius::QueryItem::Metadata(_result_metadata) => {}
            }
        }
        anyhow::bail!("query udf defition failed, no result.")
    }

    async fn drop_udf(
        &self, name: &str, _extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"DROP FUNCTION "{}"."{name}""#, Self::ONLY_SCHEMA);
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        conn.execute(sql.as_str(), &[]).await?;

        anyhow::Ok(())
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_procedure_impl(
            self.config.must_db().as_str(),
            self.config.must_schema().as_str(),
        )
        .await
    }

    async fn show_procedure(
        &self, name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<ProcedureDetail> {
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
        log::info!("sql:{}", sql);
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

    async fn drop_procedure(
        &self, name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"DROP PROCEDURE dbo."{name}""#);
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        conn.execute(&sql, &[]).await?;
        anyhow::Ok(())
    }
}

impl SqlBuilder for MsSqlServerDB {
    /// format a value make it suitable for insert into sql.
    /// this method current use in `insert_sql` etc.
    fn put_value_in_sql(
        &self, typ: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        let default_value = value.map(|x| x.to_string()).unwrap_or("null".to_string());
        let single_quote = value
            .map(|x| format!("'{}'", escape_sql_str(x)))
            .unwrap_or("null".to_string());
        let v = match typ.to_uppercase().as_str() {
            "TINYINT" | "SMALLINT" | "INT" | "BIGINT" | "FLOAT" | "REAL" | "INT4" | "INT2"
            | "INT1" | "INT8" | "FLOAT4" | "FLOAT8" | "INTN" | "DECIMALN" | "NUMERICN"
            | "FLOATN" | "NUMERIC" | "DECIMAL" => default_value,
            "BIT" | "BITN" | "BIGVARBIN" => default_value,
            "NVARCHAR" | "VARCHAR" | "NCHAR" | "CHAR" | "NTEXT" | "TEXT" | "BIGVARCHAR"
            | "BIGCHAR" => single_quote,
            "UNIQUEIDENTIFIER" => default_value,
            "BINARY" | "VARBINARY" | "IMAGE" | "BIGBINARY" => default_value,
            "XML" | "DATETIME" | "DATETIME2" | "SMALLDATETIME" | "DATE" | "TIME" | "DATETIME4"
            | "DATEN" | "DATETIMEN" | "DATETIMEOFFSET" | "TIMEN" | "DATETIMEOFFSETN" => {
                single_quote
            }
            "NULL" => default_value,
            _ => anyhow::bail!("type '{}' not support", typ),
        };
        log::trace!("type:{:?} encode '{:?}' as #{}#", typ, value, v);
        anyhow::Ok(v)
    }

    fn type_support_compare(&self, typ: &str) -> bool {
        type_support_compare(typ)
    }
}

impl MsSqlServerDB {
    async fn query_count(
        conn: &mut Connection, name: &str, where_str: Option<&str>,
    ) -> anyhow::Result<i32> {
        let sql = format!(
            r#"SELECT count(*) from "{name}" {}"#,
            match where_str {
                Some(x) => format!("  where {} ", x),
                None => "".to_string(),
            }
        );
        log::info!("sql:{}", sql);
        let mut stream = conn
            .query(&sql, &[])
            .await
            .map_err(|e| anyhow::Error::msg(format!("err:'{}' sql:'{}'", e, sql)))?;

        let mut count: Option<i32> = None;
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    count = row.try_get(0)?;
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        anyhow::Ok(count.unwrap_or_default())
    }
}
impl MsSqlServerDB {
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
                    count = x.try_get(0)?;
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
            tiberius::ColumnData::U8(x) => (*x).map(serde_json::Value::from),
            tiberius::ColumnData::I16(x) => (*x).map(serde_json::Value::from),
            tiberius::ColumnData::I32(x) => (*x).map(serde_json::Value::from),
            tiberius::ColumnData::I64(x) => (*x).map(serde_json::Value::from),
            tiberius::ColumnData::F32(x) => (*x).map(serde_json::Value::from),
            tiberius::ColumnData::F64(x) => (*x).map(serde_json::Value::from),
            tiberius::ColumnData::Bit(x) => (*x).map(serde_json::Value::from),
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

impl MsSqlServerDB {
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
        let cols = SchemaTrait::show_table_columns(self, table_name).await?;
        let sql = format!("SELECT * from {}", table_name);
        log::info!("sql:{}", sql);
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

                let insert_sql =
                    self.insert_sql(self.config.schema.as_deref(), table_name, &cols, &m)?;
                sender.send(Cow::Owned(insert_sql)).await?;
                sender.send(";\n".into()).await?;
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
        log::info!("sql:{}", sql);
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
        log::info!("sql:{}", sql);
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

            if !rows_row.is_empty() {
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
        log::info!("sql:{}", sql);
        self.export_query_result_as_csv(&sql, include_header, delimiter, sender)
            .await
    }
}

impl MsSqlServerDB {
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
impl ExportQueryResult for MsSqlServerDB {
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
impl QuerySql for MsSqlServerDB {
    fn tx_execute_counts(&self, query_id: uuid::Uuid) -> anyhow::Result<usize> {
        anyhow::Ok(self.tx_execute_times.get(query_id))
    }

    async fn get_auto_commit(&self, query_id: uuid::Uuid) -> anyhow::Result<bool> {
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!(format!("id connection '{}' not opened", query_id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();

        let mut steam = conn
            .simple_query("SELECT @@TRANCOUNT as count ")
            .await
            .map_err(|e| anyhow::Error::msg(format!("err:'{}'", e)))?;

        while let Some(item) = steam.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    let count: Option<i32> = row.try_get(0)?;
                    return anyhow::Ok(count.map(|x| x == 0).unwrap_or_default());
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        anyhow::Ok(true)
    }

    async fn set_auto_commit(&self, query_id: uuid::Uuid, commit: bool) -> anyhow::Result<()> {
        if commit {
            anyhow::bail!("mssql use commit or rollback to end transaction");
        }
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!(format!("id connection '{}' not opened", query_id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.simple_query("BEGIN TRANSACTION;").await?;
        anyhow::Ok(())
    }

    async fn commit(&self, query_id: uuid::Uuid) -> anyhow::Result<()> {
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!(format!("id connection '{}' not opened", query_id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.simple_query("COMMIT TRANSACTION;").await?;
        self.tx_execute_times.reset(query_id);
        anyhow::Ok(())
    }

    async fn rollback(&self, query_id: uuid::Uuid) -> anyhow::Result<()> {
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!(format!("id connection '{}' not opened", query_id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.simple_query("ROLLBACK TRANSACTION;").await?;
        self.tx_execute_times.reset(query_id);
        anyhow::Ok(())
    }

    fn get_query_log(&self, id: uuid::Uuid) -> anyhow::Result<Vec<QueryLogEvent>> {
        anyhow::Ok(self.query_logs.get(&id))
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        is_dangerous_sql_by_parser(sql, &sqlparser::dialect::GenericDialect {})
    }

    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let conn = self.new_tcp_conn().await?;

        let id = uuid::Uuid::new_v4();
        self.query_connections
            .insert(id, Arc::new(Mutex::new(conn)));
        anyhow::Ok(id)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        let conn = self.new_tcp_conn().await?;
        self.query_connections
            .insert(id, Arc::new(Mutex::new(conn)));
        self.tx_execute_times.reset(id);
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.query_logs.remove(&id);
        self.query_connections.remove(&id);
        self.tx_execute_times.reset(id);
        anyhow::Ok(())
    }

    async fn execute_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        if !self.get_auto_commit(query_id).await? {
            self.tx_execute_times.inc(query_id);
        }
        let mut query_log = self.query_logs.get_guard(query_id, sql);
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!(format!("id connection '{}' not opened", query_id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();

        let ret = match conn.execute(sql, &[]).await {
            Ok(x) => Ok(QueryResp::row_affected(x.total(), sql.to_owned())),
            Err(err) => Err(err.to_string()),
        };
        query_log.set_success(ret.is_ok());
        anyhow::Ok(ret)
    }

    async fn query_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        if !self.get_auto_commit(query_id).await? {
            self.tx_execute_times.inc(query_id);
        }
        let mut query_log = self.query_logs.get_guard(query_id, sql);
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!(format!("id connection '{}' not opened", query_id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();

        let mut ret = Vec::with_capacity(0);

        let mut stream = conn.simple_query(sql).await?;
        let mut meta_prev = None;
        let mut rows = Vec::new();
        let mut ml = MemoryLimit::new();

        while let Some(item) = stream.next().await {
            let item = match item {
                Ok(item) => item,
                Err(err) => {
                    ret.push(Err(err.to_string()));
                    continue;
                }
            };
            ml.exceed_error()?;
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
                        ret.push(Ok(table_list));
                        rows = vec![];
                    }
                    meta_prev = Some(meta);
                }
            }
        }
        if meta_prev.is_some() {
            let table_list = make_table_list_from_meta_and_row(
                meta_prev.as_ref().unwrap(),
                &rows,
                rows.len(),
                "",
            )?;
            ret.push(Ok(table_list));
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
    log::info!("sql:{}", sql);

    conn.simple_query(sql.as_str()).await.unwrap();
    anyhow::Ok(())
}

impl MsSqlServerDB {
    pub(crate) async fn get_conn<'a>(
        &'a self,
    ) -> anyhow::Result<bb8::PooledConnection<'a, bb8_tiberius::ConnectionManager>> {
        let conn =
            tokio::time::timeout(Duration::from_secs(5), async { self.pool.get().await }).await??;

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

#[async_trait::async_trait]
impl crate::db::DBTrait for MsSqlServerDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec!["dbo".to_string()])
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut x = Clone::clone(self);
        x.config.schema = Some(name.to_owned());
        x.query_connections = Default::default();
        Ok(Arc::new(x))
    }

    async fn create_schema(&self, _name: &str, _: Option<CreateSchemaExtra>) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn drop_schema(&self, _name: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }
}

#[async_trait::async_trait]
impl CatalogTrait for MsSqlServerDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let mut stream = conn
            .deref_mut()
            .simple_query(
                r#"
            SELECT name
            FROM sys.databases order by name
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
        config.db = Some(name.to_owned());
        let x = Self::new(config).await?;
        anyhow::Ok(Arc::new(x))
    }

    async fn create_database(
        &self, name: &str, _extra: Option<CreateDatabaseExtra>,
    ) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("CREATE DATABASE {name}");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        conn.execute(&sql, &[]).await?;
        anyhow::Ok(())
    }

    async fn drop_database(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("DROP DATABASE {name};");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        conn.execute(&sql, &[]).await?;
        anyhow::Ok(())
    }
}

#[allow(dead_code)]
#[cfg(test)]
pub(crate) async fn make_test_object() -> MsSqlServerDB {
    let ss = MsSqlServerDB::new(SqlServerDBConfig {
        host: TEST_HOST.to_string(),
        port: 1433,
        username: Some("sa".to_string()),
        password: Some("sfdewRE123".to_string()),
        db: Some("test".to_string()),
        trust_cert: true,
        schema: Some(MsSqlServerDB::ONLY_SCHEMA.to_string()),
        read_only: false,
        save_password: true,
    })
    .await
    .unwrap();
    ss
}
impl ReadOnlyTrait for MsSqlServerDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

// when I test via `cargo test`
// sometimes I have this error
// ERROR - Transaction (Process ID 76) was deadlocked on lock resources with
// another process and has been chosen as the deadlock victim. Rerun the
// transaction. code=1205 maybe I need mutex to run one test a time.
#[cfg(test)]
pub(crate) fn test_lock() -> &'static Mutex<()> {
    static MUTEX: Mutex<()> = Mutex::const_new(());
    &MUTEX
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::db::CatalogTrait;
    use crate::enable_test_log;

    #[tokio::test]
    async fn test_procedure() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_procedure("test_procedure", None).await;
        db.query_sql(
            query_id,
            r#"
            CREATE PROCEDURE dbo.test_procedure
                
            AS
            BEGIN
                SELECT 1
            END;
        "#,
        )
        .await
        .unwrap();
        let list = db.list_procedure().await.unwrap();
        for l in list.iter() {
            let details = db.show_procedure(&l.name, None).await.unwrap();
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
            let details = db.show_udf(&l.name, None).await.unwrap();
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
        let _ = db.create_database("abc", None).await;
        let dbs = CatalogTrait::list_database(&db).await.unwrap();
        println!("{:?}", dbs);
        assert_eq!(
            dbs,
            vec!["abc", "master", "model", "msdb", "tempdb", "test"]
        );
        db.drop_database("abc").await.unwrap();
    }

    #[tokio::test]
    async fn test_transaction() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let table_name = "test_commit";
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
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
            db.show_table(table_name, None, None, 1, 100, None)
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
            db.show_table(table_name, None, None, 1, 100, None)
                .await
                .unwrap()
                .total
                == 1
        );
        db.re_connect(query_id).await.unwrap();

        assert!(db.get_auto_commit(query_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_rename() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;

        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_table("test_rename", false).await;
        let _ = db.drop_table("test_rename2", false).await;
        db.execute_sql(
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
        .unwrap()
        .unwrap();
        db.rename_table("test_rename", "test_rename2")
            .await
            .unwrap();

        db.show_table("test_rename2", None, None, 1, 100, None)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_duplicate() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;

        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_table("test_duplicate", false).await;
        let _ = db.drop_table("test_duplicate2", false).await;

        db.query_sql(
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
        db.duplicate_table("test_duplicate", "test_duplicate2")
            .await
            .unwrap();

        let resp = db
            .show_table("test_duplicate2", None, None, 1, 100, None)
            .await
            .unwrap();
        assert_eq!(resp.data.len(), 5);
    }

    #[tokio::test]
    async fn test_triggers() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;

        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_table("test_create_trigger", false).await;
        let _ = db.drop_table("test_create_trigger_update", false).await;
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
        assert_eq!(dbnames, vec![MsSqlServerDB::ONLY_SCHEMA]);
        for dbname in dbnames.iter() {
            db.open_schema(&dbname).await.unwrap();
        }
    }
    #[ignore = "run manually"]
    #[tokio::test]
    async fn test_select_xxx() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();

        let x = db
            .query_sql(query_id, "SELECT  COUNT(*) from test_type1 ")
            .await
            .unwrap();

        for x in x.iter() {
            println!("query resp:{:?}", x);
        }
    }
    #[tokio::test]
    async fn test_view() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;

        let _ = db.drop_table("test_view_table", false).await;
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

        let resp = db
            .show_view("test_view", None, None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }

    #[tokio::test]
    async fn test_index() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let _ = db.drop_table("test_index_table", false).await;

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

        std::thread::sleep(Duration::from_secs(1));
        let indexes = db.show_indexes("test_index_table").await.unwrap();
        println!("indexes:{:?}", indexes);
        db.drop_table("test_index_table", false).await.unwrap();
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

    #[ignore = "xxx"]
    #[tokio::test]
    async fn test_list_table() {
        let _guard = test_lock().lock().await;
        // list all table and query
        let pg = make_test_object().await;

        let table = pg.list_table().await.unwrap();
        println!("{:?}", table);
    }

    #[ignore = "xxx"]
    #[tokio::test]
    async fn test_table_xxxx() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        let r = db
            .query_sql(
                query_id,
                r#"
            SELECT   *
FROM sys.sequences;


        "#,
            )
            .await
            .unwrap();
        for x in r.iter() {
            println!("{:?}", x);
        }
    }

    #[tokio::test]
    async fn test_table_column() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object().await;

        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;

        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_column
                (
                    id INT PRIMARY KEY                     -- 主键，无符号32位整数
                )
                "#,
            )
            .await
            .unwrap();

        db.add_table_column(
            table_name,
            "name",
            ColumnType {
                typ: "TINYINT".to_string(),
                nullness: Some("null".to_string()),
                after: None,
                // must be none here
                // query failed
                // Token error: 'The object 'DF__test_colum__name__6C2EE8AB' is dependent on column
                // 'name2'.' on server sqlpreview executing on line 1 (code: 5074, state: 1, class:
                // 16)
                default_value: None,
            },
        )
        .await
        .unwrap();

        db.modify_table_column(
            table_name,
            "name",
            "name2",
            Some(ColumnType {
                typ: "SMALLINT".to_string(),
                nullness: Some("null".to_string()),
                after: None,
                default_value: Some("2".to_string()),
            }),
        )
        .await
        .unwrap();

        db.drop_table_column(table_name, "name2").await.unwrap();
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

        let mut values = HashMap::new();
        values.insert("col_tinyint".to_string(), Some("1".to_string()));
        values.insert("col_smallint".to_string(), Some("100".to_string()));
        values.insert("col_int".to_string(), Some("1000".to_string()));
        values.insert("col_bigint".to_string(), Some("10000".to_string()));
        values.insert("col_float".to_string(), Some("3.14".to_string()));
        values.insert("col_double".to_string(), Some("3.14159".to_string()));
        db.table_insert(table_name, values.clone()).await.unwrap();
        db.table_update(table_name, values, {
            let mut wheres = HashMap::new();
            wheres.insert("id".to_string(), Some("1".to_string()));
            wheres
        })
        .await
        .unwrap();
        let list = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
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

        let mut values = HashMap::new();
        values.insert("bit_column".to_string(), Some("1".to_string()));
        values.insert("nvarchar_column".to_string(), Some("b".to_string()));
        values.insert("varchar_column".to_string(), Some("c".to_string()));

        values.insert("nchar_column".to_string(), Some("d".to_string()));
        values.insert("char_column".to_string(), Some("e".to_string()));
        values.insert("ntext_column".to_string(), Some("f".to_string()));
        values.insert("text_column".to_string(), Some("g".to_string()));

        db.table_insert(table_name, values.clone()).await.unwrap();
        db.table_update(table_name, values, {
            let mut wheres = HashMap::new();
            wheres.insert("id".to_string(), Some("1".to_string()));
            wheres
        })
        .await
        .unwrap();
        let list = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
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
        db.table_insert(table_name, values.clone()).await.unwrap();
        db.table_update(table_name, values, {
            let mut wheres = HashMap::new();
            wheres.insert("id".to_string(), Some("1".to_string()));
            wheres
        })
        .await
        .unwrap();
        let list = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
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
        db.table_insert(table_name, values.clone()).await.unwrap();
        db.table_update(table_name, values, {
            let mut wheres = HashMap::new();
            wheres.insert("id".to_string(), Some("1".to_string()));
            wheres
        })
        .await
        .unwrap();
        let list = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("ret:{:?}", list);
    }
}
