use crate::datasource_config::SqlServerDBConfig;
use crate::db::{
    create_html_table_data, create_html_table_header, create_markdown_data, create_markdown_header,
    export_query_as_xlsx_send_data, format_data_time_fix_offset, quote_sql_str, BackendKind,
    CatalogTrait, CheckConn, ColumnType, CreateDatabaseExtra, CreateIndexReq, CreateSchemaExtra,
    DBTrait, DefAndMetas, EncodeXlsxDatetime, ExportFrame, ExportKind, ExportQueryResult,
    ExtractSqlValeFromJsonValue, Field, FirstTrue, ForeignKeyDef, GetBackendKind,
    GetTestExportTableKinds, Index, IndexKind, ListTable, MetaValue, NamedStaff, ProcedureDetail,
    QueryEvent, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas, ShowTableParameter,
    SqlBuilder, SqlCollect, TriggerDetail, TriggerEnable, TriggerEvent, TriggerTiming, XlsxEdit,
    XlsxEditContent, POOL_MAX_CONNECTIONS,
};
use crate::db::{OptionMustBeSomeErr, QueryEventEmitter};
use crate::helper::{create_csv, extract_bytes_from_serde_values};
use crate::language_server::{get_update_sql_from_result_set_impl, LexerCompletionTrait};
use chrono::FixedOffset;
use dashmap::DashMap;
use futures_util::StreamExt;
use sqlparser::tokenizer::Token;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;
use tiberius::{Client, ColumnData, EncryptionLevel, FromSql, Row};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::{Compat, TokioAsyncWriteCompatExt};

#[derive(Clone)]
pub struct SqlServerDB {
    pool: bb8::Pool<bb8_tiberius::ConnectionManager>,
    pub(crate) config: SqlServerDBConfig,
    query_connection: Arc<DashMap<String, Arc<Mutex<Connection>>>>,
}

impl std::fmt::Debug for SqlServerDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mssql on:{:?}", self.config)
    }
}

impl SqlServerDB {
    pub(crate) const DBO_SCHEMA: &str = "dbo";
    pub(crate) const SYS_SCHEMA: &str = "sys";
    pub(crate) const INFORMATION_SCHEMA_SCHEMA: &str = "INFORMATION_SCHEMA";
}
#[async_trait::async_trait]
impl ServerMetas for SqlServerDB {
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
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(r) => {
                    let m = encode_row(&r)?;
                    ret.push(MetaValue::new(
                        m.first()
                            .expect("must have results")
                            .as_ref()
                            .option_must_be_some()?
                            .to_string(),
                        m[1].clone(),
                    ));
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        anyhow::Ok(ret)
    }
}

impl SqlServerDB {
    pub async fn new(c: SqlServerDBConfig) -> anyhow::Result<Self> {
        let manager = bb8_tiberius::ConnectionManager::new(make_config(&c)?);
        let mut builder = bb8::Pool::builder();
        if let Some(x) = c.conn_timeout {
            builder = builder.connection_timeout(x.into());
        }
        builder = builder.max_size(POOL_MAX_CONNECTIONS);
        builder = builder.min_idle(Some(5));
        let pool = builder.build(manager).await?;
        anyhow::Ok(Self {
            config: c,
            pool,
            query_connection: Default::default(),
        })
    }
}

fn make_config(c: &SqlServerDBConfig) -> anyhow::Result<tiberius::Config> {
    if c.trust_cert && c.cert_ca.is_some() {
        anyhow::bail!("can't be able both trust_cert and use trust certificate.")
    }
    let mut config = tiberius::Config::new();
    config.host(&c.host);
    config.port(c.port);
    if let Some(user) = &c.username {
        if let Some(pass) = &c.password {
            config.authentication(tiberius::AuthMethod::sql_server(user, pass));
        }
    }
    config.readonly(c.read_only);
    if let Some(db) = c.db.as_ref() {
        config.database(db);
    }
    if c.trust_cert {
        config.trust_cert(); // on production, it is not a good idea to do this
    }
    if let Some(path) = &c.cert_ca {
        config.trust_cert_ca(path);
        config.encryption(EncryptionLevel::Required);
    }
    anyhow::Ok(config)
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
pub struct ConvertColumnDataToString {
    pub(crate) value: String,
}

fn encode_bytes(x: &[u8]) -> String {
    format!("0X{}", hex::encode_upper(x))
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
            tiberius::ColumnData::Binary(x) => x.as_ref().map(|x| encode_bytes(x)),
            tiberius::ColumnData::Numeric(x) => x.as_ref().map(|x| x.to_string()),

            tiberius::ColumnData::Xml(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::DateTime(_)
            | tiberius::ColumnData::DateTime2(_)
            | tiberius::ColumnData::SmallDateTime(_) => chrono::NaiveDateTime::from_sql(value)
                .ok()
                .flatten()
                .map(|x| x.to_string()),

            tiberius::ColumnData::Date(_x) => chrono::NaiveDate::from_sql(value)
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

pub struct ConvertColumnDataToXlsxContent {
    pub(crate) value: XlsxEditContent,
}
impl<'a> tiberius::FromSql<'a> for ConvertColumnDataToXlsxContent {
    fn from_sql(value: &'a ColumnData<'static>) -> tiberius::Result<Option<Self>> {
        let value = match value {
            tiberius::ColumnData::U8(x) => x.as_ref().map(|x| XlsxEditContent::number(*x)),
            tiberius::ColumnData::I16(x) => x.as_ref().map(|x| XlsxEditContent::number(*x)),
            tiberius::ColumnData::I32(x) => x.as_ref().map(|x| XlsxEditContent::number(*x)),
            tiberius::ColumnData::I64(x) => x.as_ref().map(|x| (*x).into()),
            tiberius::ColumnData::F32(x) => x.as_ref().map(|x| XlsxEditContent::number(*x)),
            tiberius::ColumnData::F64(x) => x.as_ref().map(|x| XlsxEditContent::Number(*x)),
            tiberius::ColumnData::Bit(x) => x.as_ref().map(|x| XlsxEditContent::number(*x as u8)),
            tiberius::ColumnData::String(x) => x.as_ref().map(|x| x.to_string().into()),
            tiberius::ColumnData::Guid(x) => x.as_ref().map(|x| x.to_string().into()),
            tiberius::ColumnData::Binary(x) => x.as_ref().map(|x| encode_bytes(x).into()),
            tiberius::ColumnData::Numeric(x) => x.as_ref().map(|x| x.to_string().into()),

            tiberius::ColumnData::Xml(x) => x.as_ref().map(|x| x.to_string().into()),
            tiberius::ColumnData::DateTime(_)
            | tiberius::ColumnData::DateTime2(_)
            | tiberius::ColumnData::SmallDateTime(_) => chrono::NaiveDateTime::from_sql(value)
                .ok()
                .flatten()
                .map(|x| x.into()),

            tiberius::ColumnData::Date(_) => chrono::NaiveDate::from_sql(value)
                .ok()
                .flatten()
                .map(|x| x.into()),

            tiberius::ColumnData::Time(_x) => chrono::NaiveTime::from_sql(value)
                .ok()
                .flatten()
                .map(|x| x.into()),

            tiberius::ColumnData::DateTimeOffset(_) => {
                chrono::DateTime::<FixedOffset>::from_sql(value)
                    .ok()
                    .flatten()
                    .map(|x| x.naive_utc().into())
            }
        };
        Ok(value.map(|x| Self { value: x }))
    }
}
fn encode_row(row: &tiberius::Row) -> anyhow::Result<Vec<Option<String>>> {
    let mut values: Vec<Option<String>> = Vec::new();
    for index in 0..row.columns().len() {
        let v: Option<ConvertColumnDataToString> = row.try_get(index)?;
        values.push(v.map(|x| x.value));
    }
    anyhow::Ok(values)
}

impl EncodeXlsxDatetime for SqlServerDB {
    /*
            all date time types
            DateTimeData DATETIME,
            DateTime2Data DATETIME2,
            SmallDateTimeData SMALLDATETIME,
            DateData1 DATE,
            TimeData TIME,
            DateTimeOffsetData DATETIMEOFFSET
    */
    fn is_datatime(&self, ty: &str) -> anyhow::Result<bool> {
        anyhow::Ok(
            ["DATETIME", "DATETIME2", "SMALLDATETIME", "DATETIMEOFFSET"]
                .iter()
                .any(|x| x.eq_ignore_ascii_case(ty)),
        )
    }

    fn is_date(&self, ty: &str) -> anyhow::Result<bool> {
        anyhow::Ok(ty.eq_ignore_ascii_case("DATE"))
    }

    fn is_time(&self, ty: &str) -> anyhow::Result<bool> {
        anyhow::Ok(ty.eq_ignore_ascii_case("TIME"))
    }
}

fn encode_row_as_xlsx(row: &tiberius::Row) -> anyhow::Result<Vec<Option<XlsxEditContent>>> {
    let mut values = Vec::new();
    for index in 0..row.columns().len() {
        let v: Option<ConvertColumnDataToXlsxContent> = row.try_get(index)?;
        values.push(v.map(|x| x.value));
    }
    anyhow::Ok(values)
}

// use lexer to solve this issue.
fn extract_trigger_event_base_on_lexer(sql: &str) -> anyhow::Result<TriggerEvent> {
    let t = sqlparser::tokenizer::Tokenizer::new(&sqlparser::dialect::MsSqlDialect {}, sql)
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
    let event = event.option_must_be_some()?;
    anyhow::Ok(TriggerEvent::try_from(event.as_str())?)
}

#[async_trait::async_trait]
impl CheckConn for SqlServerDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        conn.query("select 1", &[]).await?;
        anyhow::Ok(ins.elapsed())
    }
}

impl SqlServerDB {
    pub(crate) async fn list_table_impl(
        &self,
        db: &str,
        schema: &str,
    ) -> anyhow::Result<Vec<ListTable>> {
        if schema.eq_ignore_ascii_case(Self::SYS_SCHEMA) {
            // No query was found here, so have AI generate a list.
            return anyhow::Ok(
                SYS_OBJECTS
                    .iter()
                    .map(|x| ListTable::from(x.to_string()))
                    .collect(),
            );
        }
        if schema.eq_ignore_ascii_case(Self::INFORMATION_SCHEMA_SCHEMA) {
            return anyhow::Ok(
                INFORMATION_SCHEMA_VIEWS
                    .iter()
                    .map(|x| ListTable::from(x.to_string()))
                    .collect(),
            );
        }
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream: tiberius::QueryStream<'_> = conn
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
                    if let Some(table_name) = table_name {
                        ret.push(table_name.to_string());
                    }
                }
                tiberius::QueryItem::Metadata(_) => {
                    // I know the meta pretty well
                }
            }
        }
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    pub(crate) async fn list_procedure_impl(
        &self,
        schema: &str,
    ) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
                SELECT  name
                    FROM    sys.all_objects
                    WHERE   type = 'P'              -- P 表示存储过程
                    AND   schema_id = SCHEMA_ID('{}');   -- 只取 sys schema 下的
            "#,
            schema
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

impl GetBackendKind for SqlServerDB {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::SqlServer
    }
}

impl SqlServerDB {
    pub(crate) async fn list_udf_impl(&self, schema: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
            SELECT 
                name AS FunctionName
            FROM 
                sys.objects
            WHERE 
                type IN ('FN', 'IF', 'TF') AND  SCHEMA_NAME(schema_id) = '{}';
        "#,
            schema
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
                    if let Some(name) = name {
                        ret.push(NamedStaff::from(name.to_owned()));
                    }
                }
                tiberius::QueryItem::Metadata(..) => {}
            }
        }
        anyhow::Ok(ret)
    }

    pub(crate) async fn list_view_impl(&self, schema: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"
            SELECT 
                TABLE_NAME AS ViewName
            FROM 
                INFORMATION_SCHEMA.VIEWS
            WHERE 
                TABLE_SCHEMA = '{schema}';
        "#
        );
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream = conn.query(sql, &[]).await?;
        let mut ret = vec![];
        while let Some(row) = stream.next().await {
            let row = row?;
            match row {
                tiberius::QueryItem::Row(row) => {
                    let name: Option<&str> = row.try_get(0)?;
                    if let Some(name) = name {
                        ret.push(ListTable::from(name.to_string()));
                    }
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        anyhow::Ok(ret)
    }
}
#[async_trait::async_trait]
impl SchemaTrait for SqlServerDB {
    async fn drop_foreign_key(&self, table: &str, name: &str) -> anyhow::Result<()> {
        // 1. Drop the foreign key constraint (SQL Server uses DROP CONSTRAINT; the index does not need to be dropped).
        // Note: SQL Server recommends enclosing table and constraint names in square brackets [].
        let sql = format!(r#"ALTER TABLE [{}] DROP CONSTRAINT [{}]"#, table, name);
        self.simple_execute_sql(&sql).await?;

        anyhow::Ok(())
    }

    async fn list_foreign_key(&self, table: &str) -> anyhow::Result<Vec<ForeignKeyDef>> {
        let mut conn = self.get_conn().await?;

        // Query SQL Server foreign keys using sys.foreign_keys and sys.foreign_key_columns.
        // Note: Do not use format to interpolate table names into SQL strings with tiberius (to prevent injection); use the @P1 placeholder.
        let sql = r#"
        SELECT
            fk.name AS constraint_name,
            OBJECT_SCHEMA_NAME(fk.referenced_object_id) + '.' + OBJECT_NAME(fk.referenced_object_id) AS referenced_table_name,
            c.name AS column_name,
            pc.name AS referenced_column_name,
            CASE fk.update_referential_action_desc 
                WHEN 'CASCADE' THEN 'CASCADE' 
                WHEN 'SET_NULL' THEN 'SET NULL' 
                WHEN 'SET_DEFAULT' THEN 'SET DEFAULT' 
                ELSE 'NO ACTION' 
            END AS update_rule,
            CASE fk.delete_referential_action_desc 
                WHEN 'CASCADE' THEN 'CASCADE' 
                WHEN 'SET_NULL' THEN 'SET NULL' 
                WHEN 'SET_DEFAULT' THEN 'SET DEFAULT' 
                ELSE 'NO ACTION' 
            END AS delete_rule
        FROM
            sys.foreign_keys fk
        JOIN
            sys.foreign_key_columns fkc ON fk.object_id = fkc.constraint_object_id
        JOIN
            sys.columns c ON fkc.parent_object_id = c.object_id AND fkc.parent_column_id = c.column_id
        JOIN
            sys.columns pc ON fkc.referenced_object_id = pc.object_id AND fkc.referenced_column_id = pc.column_id
        WHERE
            OBJECT_NAME(fk.parent_object_id) = @P1
    "#;

        log::info!("sql:{}", sql);

        // tiberius queries require parameters (@P1 corresponds to the first parameter).
        let mut rows = conn.query(sql, &[&table]).await?;

        let mut ret = Vec::new();

        while let Some(row) = rows.next().await {
            let row = row?;
            match row {
                tiberius::QueryItem::Row(row) => {
                    // Note: tiberius's get method differs from sqlx; Option types require special handling.
                    let constraint_name: Option<&str> = row.get(0);
                    let referenced_table: &str = row.get(1).unwrap_or_default();
                    let from: &str = row.get(2).unwrap_or_default();
                    let to: &str = row.get(3).unwrap_or_default();
                    let on_update: &str = row.get(4).unwrap_or_default();
                    let on_delete: &str = row.get(5).unwrap_or_default();

                    ret.push(ForeignKeyDef {
                        name: constraint_name.map(|x| x.to_owned()),
                        table: referenced_table.to_string(),
                        from: from.to_owned(),
                        to: to.to_owned(),
                        on_update: on_update.to_owned(),
                        on_delete: on_delete.to_owned(),
                    });
                }

                tiberius::QueryItem::Metadata(..) => {}
            }
        }

        anyhow::Ok(ret)
    }

    fn list_sql_current_values(&self) -> &'static [&'static str] {
        <Self as SqlBuilder>::get_predefine_names(self)
    }
    async fn create_index(&self, table: &str, req: &CreateIndexReq) -> anyhow::Result<String> {
        self.can_write()?;
        let sql = req.common_sql(table, true, self.backend_kind().quote_style())?;
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        conn.execute(&sql, &[]).await?;
        anyhow::Ok(sql)
    }

    async fn drop_index(&self, table_name: &str, index: &str) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let sql = format!(r#"DROP INDEX "{index}" ON "{table_name}";"#);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn rename_index(
        &self,
        table: &str,
        old_name: &str,
        new_name: &str,
    ) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let sql = format!("EXEC sp_rename '{table}.{old_name}', '{new_name}', 'INDEX';");
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
        let sql = format!(
            r#"ALTER TABLE "{table}" ADD "{name}" {} {} {}"#,
            col.typ,
            col.nullable.unwrap_or_default(),
            if let Some(dv) = col.default_value.as_ref() {
                format!(
                    "DEFAULT {}",
                    self.encode_value_hit_predefine(&col.typ, dv.as_str())?
                )
            } else {
                "".to_owned()
            }
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        conn.deref_mut().execute(&sql, &[]).await?;
        anyhow::Ok(sql)
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        LexerCompletionTrait::get_types(self).await
    }

    async fn modify_table_column(
        &self,
        table: &str,
        old_name: &str,
        new_name: &str,
        typ: Option<crate::db::ColumnType>,
    ) -> anyhow::Result<String> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let mut sc = SqlCollect::default();
        if let Some(typ) = typ.as_ref() {
            if typ.default_value.is_some() {
                anyhow::bail!("sql server does not support modify column with default value");
            }
        }
        if old_name != new_name {
            let sql = format!(r#"EXEC sp_rename '{table}.{old_name}', '{new_name}', 'COLUMN'"#);
            log::info!("sql:{}", sql);
            conn.deref_mut().execute(&sql, &[]).await?;
            sc.collect(sql);
        }
        if let Some(typ) = typ.as_ref() {
            let sql = format!(
                r#"ALTER TABLE {table} ALTER COLUMN {new_name} {} {}"#,
                typ.typ,
                typ.nullable.unwrap_or_default(),
            );
            log::info!("sql:{}", sql);
            conn.deref_mut().execute(&sql, &[]).await?;
            sc.collect(sql);
        }
        anyhow::Ok(sc.into())
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<String> {
        self.can_write()?;
        let sql = format!(
            r#"
            ALTER TABLE "{table}" DROP COLUMN "{col}";
        "#
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        conn.execute(&sql, &[]).await?;
        anyhow::Ok(sql)
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        self.can_write()?;
        if self.table_exists(dup).await? {
            anyhow::bail!(format!("table '{}' already exists", dup));
        }
        let schema = self.config.schema_str()?.as_str();
        let sql = format!(
            r#"
            SELECT * INTO "{schema}"."{dup}" FROM "{schema}"."{table}" 
        "#
        );
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let sql = format!(
            r#"
            EXEC sp_rename '{old}', '{new}';
        "#
        );
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
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
                    ret.push(name.option_must_be_some()?.to_string());
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
                        sql.option_must_be_some()?.to_string(),
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

    async fn drop_trigger(&self, table: &str, name: &str) -> anyhow::Result<String> {
        self.can_write()?;
        let sql = format!(
            r#"
            DROP TRIGGER "{}"."{name}" ON "{table}";
        "#,
            Self::DBO_SCHEMA
        );
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn export_table_or_view(
        &self,
        table_name: &str,
        para: Option<ShowTableParameter>,
        kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::Sql {
                include_table_schema,
            } => {
                self.export_table_as_sql(table_name, para, include_table_schema, sender)
                    .await
            }
            _ => {
                let sql = self.gen_show_sql(table_name, None, para.as_ref());
                self.export_query_result(None, &sql, kind, sender).await
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
        &self,
        view_name: &str,
        para: ShowTableParameter,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        emit.send("trying to get connection.".into())?;
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        emit.send("trying to get total amount.".into())?;
        let total = SqlServerDB::query_count(
            conn,
            self.config.schema_str()?,
            view_name,
            para.filter.as_deref(),
        )
        .await?;
        let sql = self.gen_show_sql(view_name, None, Some(&para));
        log::info!("sql:{}", sql);
        emit.send("trying to get data.".into())?;
        let mut stream = conn.query(&sql, &[]).await?;
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    let data = encode_row(&row)?;
                    emit.send(data.into())?;
                }
                tiberius::QueryItem::Metadata(m) => {
                    emit.send(
                        QueryResp {
                            fields: get_fields_from_meta(&m),
                            sql: sql.clone().into(),
                            total: total as f64,
                            ..Default::default()
                        }
                        .into(),
                    )?;
                }
            }
        }
        anyhow::Ok(())
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<String> {
        self.can_write()?;
        let schema = self.config.schema_str()?.as_str();
        let sql = format!(r#"DROP VIEW "{schema}"."{view_name}""#);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let sql = format!(r#"DROP TABLE "{table}""#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;

        anyhow::Ok(sql.into())
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_view_impl(self.config.schema_str()?.as_str())
            .await
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(
            self.config.db_str()?.as_str(),
            self.config.schema_str()?.as_str(),
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
                    let name = name.option_must_be_some()?.to_string();
                    let data_type: Option<&str> = row.try_get(1)?;
                    let data_type = data_type.option_must_be_some()?.to_string();
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
            let is_primary = SqlServerDB::is_primary(conn, table_name, &f.field).await?;
            let is_auto_increment =
                SqlServerDB::is_auto_increment(conn, table_name, &f.field).await?;
            f.is_primary = is_primary;
            f.is_auto_increment = is_auto_increment;
        }

        anyhow::Ok(ret)
    }

    async fn show_table(
        &self,
        table_name: &str,
        para: ShowTableParameter,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        emit.send("trying to get connection.".into())?;
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        emit.send("trying to get total amount.".into())?;
        let total = SqlServerDB::query_count(
            conn,
            self.config.schema_str()?,
            table_name,
            para.filter.as_deref(),
        )
        .await?;
        let fields = self.show_table_columns(table_name).await?;
        let sql = self.gen_show_sql(table_name, Some(&fields), Some(&para));
        emit.send(
            QueryResp {
                fields,
                total: total as f64,
                sql: sql.clone().into(),
                ..Default::default()
            }
            .into(),
        )?;
        emit.send("trying to query data.".into())?;
        let mut stream = conn
            .query(&sql, &[])
            .await
            .map_err(|e| anyhow::Error::msg(format!("err:'{}' sql:'{}'", e, sql)))?;
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    let d = encode_row(&row)?;
                    emit.send(d.into())?;
                }
                tiberius::QueryItem::Metadata(..) => {
                    // we have send the fields.
                }
            }
        }
        anyhow::Ok(())
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
                    let name = name.option_must_be_some()?.to_string();
                    let is_unique: Option<bool> = r.try_get(1)?;
                    let col: Option<&str> = r.try_get(2)?;
                    let col = col.option_must_be_some()?.to_string();
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

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let sql = format!(
            r#"
            TRUNCATE TABLE "{table_name}";
        "#
        );
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn enable_trigger(
        &self,
        table: &str,
        name: &str,
        enable: bool,
    ) -> anyhow::Result<String> {
        self.can_write()?;
        let sql = format!(
            "{} TRIGGER {name} ON {table}",
            if enable { "ENABLE" } else { "DISABLE" }
        );

        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_udf_impl(self.config.schema_str()?).await
    }

    async fn show_udf(&self, name: &NamedStaff) -> anyhow::Result<DefAndMetas> {
        let sql = format!(
            r#"
        SELECT 
            o.type_desc AS FunctionType,
            o.create_date,
            o.modify_date
        FROM 
            sys.objects AS o
        WHERE 
            o.name = '{0}'  -- 替换为你的函数名
            AND o.type IN ('FN', 'IF', 'TF');  -- FN: 标量函数, IF: 内联表值函数, TF: 多语句表值函数
        "#,
            name.name
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
                        type_desc.map(|x| x.to_string()),
                    ));
                    metas.push(MetaValue::new(
                        "create_date",
                        create_date.map(|x| x.to_string()),
                    ));
                    metas.push(MetaValue::new(
                        "modify_date",
                        modify_date.map(|x| x.to_string()),
                    ));
                }
                tiberius::QueryItem::Metadata(_result_metadata) => {}
            }
        }
        drop(resp);
        let sql = format!(
            "  SELECT 
                OBJECT_DEFINITION(OBJECT_ID('dbo.{0}')) AS FunctionDefinition; ",
            name.name
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
        anyhow::bail!("query udf definition failed, no result.")
    }

    async fn drop_udf(&self, name: &NamedStaff) -> anyhow::Result<String> {
        self.can_write()?;
        let sql = format!(r#"DROP FUNCTION "{}"."{1}""#, Self::DBO_SCHEMA, name.name);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_procedure_impl(self.config.schema_str()?.as_str())
            .await
    }

    async fn show_procedure(&self, name: &NamedStaff) -> anyhow::Result<ProcedureDetail> {
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
                    p.name = '{}';
        "#,
            name.name
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
                            MetaValue::new("create_date", create_date.map(|x| x.to_string())),
                            MetaValue::new("modify_date", modify_date.map(|x| x.to_string())),
                        ],
                    ));
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        anyhow::bail!("query procedure failed, no result.")
    }

    async fn drop_procedure(&self, name: &NamedStaff) -> anyhow::Result<String> {
        self.can_write()?;
        let sql = format!(r#"DROP PROCEDURE dbo."{}""#, name.name);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }
}
/// CURRENT_* / no-parentheses literals that can be written directly in SQL Server statements.
pub const SQLSERVER_CURRENT_LITERALS: &[&str] = &[
    "CURRENT_TIMESTAMP", //  2025-08-25 14:36:07.123
    "CURRENT_DATE",      //  2025-08-25 (supported in SQL Server 2022+).
    "CURRENT_TIME",      //  14:36:07.123 (supported in SQL Server 2022+).
    "CURRENT_USER",      //  dbo
];
impl SqlBuilder for SqlServerDB {
    fn get_schema(&self) -> Option<&str> {
        self.config.schema.as_deref()
    }

    fn get_predefine_names(&self) -> &'static [&'static str] {
        SQLSERVER_CURRENT_LITERALS
    }
    /// format a value make it suitable for insert into sql.
    /// this method current use in `insert_sql` etc.
    fn encode_value(&self, typ: &str, value: &str) -> anyhow::Result<String> {
        let default_value = value.to_string();
        let single_quote = quote_sql_str(value);

        let v = match typ.to_uppercase().as_str() {
            "TINYINT" | "SMALLINT" | "INT" | "BIGINT" | "FLOAT" | "REAL" | "INT4" | "INT2"
            | "INT1" | "INT8" | "FLOAT4" | "FLOAT8" | "INTN" | "DECIMALN" | "NUMERICN"
            | "FLOATN" | "NUMERIC" | "DECIMAL" => default_value,
            "BIT" | "BITN" | "BIGVARBIN" => default_value,
            "NVARCHAR" | "VARCHAR" | "NCHAR" | "CHAR" | "NTEXT" | "TEXT" | "BIGVARCHAR"
            | "BIGCHAR" => single_quote,
            "UNIQUEIDENTIFIER" => single_quote,
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

    fn gen_show_sql(
        &self,
        table_name: &str,
        fields: Option<&[Field]>,
        para: Option<&ShowTableParameter>,
    ) -> String {
        let schema = self.config.schema_str().expect("must be some");
        let mut sql = format!(
            r#"SELECT {} FROM "{schema}"."{table_name}""#,
            self.backend_kind().format_fields(fields)
        );
        if let Some(para) = para {
            let ShowTableParameter {
                filter: where_str,
                sort: order_by,
                ..
            } = &para;
            if let Some(where_str) = where_str {
                sql.push_str(format!(" WHERE {where_str} ").as_str());
            }
            if let Some(order_by) = order_by {
                sql.push_str(format!(" ORDER BY {order_by} ").as_str());
                if let Some(offset) = para.offset() {
                    sql.push_str(
                        format!(
                            " OFFSET {} ROWS FETCH NEXT {} ROWS ONLY",
                            offset.offset, offset.page_size
                        )
                        .as_str(),
                    );
                }
            } else if let Some(offset) = para.offset() {
                // SQL Server's OFFSET clause can only be used with ORDER BY.
                sql.push_str(" ORDER  BY (SELECT NULL) ");
                sql.push_str(
                    format!(
                        " OFFSET {} ROWS FETCH NEXT {} ROWS ONLY",
                        offset.offset, offset.page_size
                    )
                    .as_str(),
                );
            }
        }

        sql
    }
}

impl ExtractSqlValeFromJsonValue for SqlServerDB {
    fn extrat_sql_value_from_json_value(
        &self,
        value: &serde_json::Value,
        _ty: &str,
    ) -> anyhow::Result<Option<String>> {
        let r = match value {
            serde_json::Value::Null => return anyhow::Ok(None),
            serde_json::Value::Bool(x) => if *x { "1" } else { "0" }.to_string(),
            serde_json::Value::Number(number) => number.to_string(),
            serde_json::Value::String(x) => x.clone(),
            serde_json::Value::Array(values) => {
                let r = extract_bytes_from_serde_values(values)?;
                encode_bytes(&r)
            }
            serde_json::Value::Object(..) => {
                anyhow::bail!("sql server should not have object value in json")
            }
        };
        anyhow::Ok(Some(r))
    }
}

impl SqlServerDB {
    async fn query_count(
        conn: &mut Connection,
        schema: &str,
        name: &str,
        where_str: Option<&str>,
    ) -> anyhow::Result<i32> {
        let sql = format!(
            r#"SELECT count(*) from "{schema}"."{name}" {}"#,
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

impl SqlServerDB {
    async fn is_primary(
        conn: &mut Connection,
        table_name: &str,
        col_name: &str,
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
        conn: &mut Connection,
        table_name: &str,
        col_name: &str,
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

            tiberius::ColumnData::Date(_) => chrono::NaiveDate::from_sql(value)
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

impl SqlServerDB {
    async fn export_table_as_sql(
        &self,
        table_name: &str,
        para: Option<ShowTableParameter>,
        include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            anyhow::bail!("sqlserver not suport export table schema");
        }

        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let cols = SchemaTrait::show_table_columns(self, table_name).await?;
        let sql = self.gen_show_sql(table_name, Some(&cols), para.as_ref());
        log::info!("sql:{}", sql);
        let mut stream = conn.query(sql.as_str(), &[]).await?;
        while let Some(row) = stream.next().await {
            let row = row?;
            match row {
                tiberius::QueryItem::Row(row) => {
                    let data = encode_row(&row)?;
                    let mut m = HashMap::new();
                    for (d, f) in data.into_iter().zip(cols.iter()) {
                        m.insert(f.field.clone(), d);
                    }
                    let insert_sql = self.insert_sql(table_name, &cols, &m)?;
                    sender.send(insert_sql.into()).await?;
                    sender.send(";\n".into()).await?;
                }
                tiberius::QueryItem::Metadata(..) => {}
            }
        }
        anyhow::Ok(())
    }
}

impl SqlServerDB {
    /// this fn create a `Connection` only use for query connection
    /// because `query_connection` use in `SqlServer`,
    /// we can't create self reference struct in rust.
    async fn new_tcp_conn(&self) -> anyhow::Result<Connection> {
        let config = make_config(&self.config)?;
        let tcp = TcpStream::connect(config.get_addr()).await?;
        tcp.set_nodelay(true)?;
        let client = tiberius::Client::connect(config, tcp.compat_write()).await?;
        anyhow::Ok(client)
    }
}

type Connection = Client<Compat<TcpStream>>;

#[async_trait::async_trait]
impl ExportQueryResult for SqlServerDB {
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
        let conn = match query_id {
            Some(query_id) => match self.query_connection.get(&query_id.to_string()) {
                Some(x) => x.clone(),
                None => anyhow::bail!("query_id not found"),
            },
            None => {
                let r = self.new_tcp_conn().await?;
                Arc::new(Mutex::new(r))
            }
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let first = FirstTrue::default();

        let mut stream = conn.query(sql, &[]).await?;
        let mut row_num = 0;
        while let Some(rows) = stream.next().await {
            let row = rows?;
            match row {
                tiberius::QueryItem::Row(row) => match kind {
                    ExportKind::Csv { delimiter, .. } => {
                        let s = create_csv(&encode_row(&row)?, delimiter);
                        sender.send(s?.into()).await?;
                    }
                    ExportKind::Json { .. } | ExportKind::JsonLine => {
                        let value = encode_row_as_json(&row)?;
                        sender.send(value.into()).await?;
                    }
                    ExportKind::Sql { .. } => anyhow::bail!("can't export query result to sql"),
                    ExportKind::Xlsx { .. } => {
                        let d = encode_row_as_xlsx(&row)?;
                        export_query_as_xlsx_send_data(&mut row_num, d, sender.clone()).await?;
                    }
                    ExportKind::Html { .. } => {
                        let s = create_html_table_data(&[encode_row(&row)?]);
                        sender.send(s.into()).await?;
                    }
                    ExportKind::MarkDown => {
                        let s = create_markdown_data(&encode_row(&row)?);
                        sender.send(s.into()).await?;
                    }
                },
                tiberius::QueryItem::Metadata(meta) => match kind {
                    ExportKind::Csv {
                        include_header,
                        delimiter,
                    } => {
                        if first.get() && include_header {
                            let fs = meta
                                .columns()
                                .iter()
                                .map(|x| x.name().to_string())
                                .map(Some)
                                .collect::<Vec<_>>();
                            sender.send(create_csv(&fs, delimiter)?.into()).await?;
                        }
                    }
                    ExportKind::Json { .. } | ExportKind::JsonLine => {}
                    ExportKind::Sql { .. } => {
                        anyhow::bail!("can't export query result to sql")
                    }
                    ExportKind::Xlsx { include_header, .. } => {
                        if first.get() && include_header {
                            for (index, col) in meta.columns().iter().enumerate() {
                                sender
                                    .send(
                                        XlsxEdit {
                                            row: row_num,
                                            column: index as rust_xlsxwriter::ColNum,
                                            content: col.name().to_owned().into(),
                                        }
                                        .into(),
                                    )
                                    .await?;
                            }
                            row_num += 1;
                        }
                    }
                    ExportKind::Html { include_header } => {
                        if first.get() && include_header {
                            let fs = meta
                                .columns()
                                .iter()
                                .map(|x| x.name().to_string())
                                .collect::<Vec<_>>();
                            sender.send(create_html_table_header(&fs).into()).await?;
                        }
                    }
                    ExportKind::MarkDown => {
                        if first.get() {
                            let fs = meta
                                .columns()
                                .iter()
                                .map(|x| x.name().to_string())
                                .collect::<Vec<_>>();
                            sender.send(create_markdown_header(&fs).into()).await?;
                        }
                    }
                },
            }
        }
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl QuerySql for SqlServerDB {
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
    fn support_tx(&self) -> bool {
        true
    }
    async fn get_auto_commit(&self, query_id: &str) -> anyhow::Result<bool> {
        let conn = match self.query_connection.get(&query_id.to_string()) {
            Some(x) => x.clone(),
            None => anyhow::bail!(format!("id connection '{}' not found", query_id)),
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

    async fn set_auto_commit(&self, query_id: &str, commit: bool) -> anyhow::Result<()> {
        if commit {
            anyhow::bail!("mssql use commit or rollback to end transaction");
        }
        let conn = match self.query_connection.get(&query_id.to_string()) {
            Some(x) => x.clone(),
            None => anyhow::bail!(format!("id connection '{}' not found", query_id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.simple_query("BEGIN TRANSACTION;").await?;
        anyhow::Ok(())
    }

    async fn commit(&self, query_id: &str) -> anyhow::Result<()> {
        let conn = match self.query_connection.get(&query_id.to_string()) {
            Some(x) => x.clone(),
            None => anyhow::bail!(format!("id connection '{}' not found", query_id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.simple_query("COMMIT TRANSACTION;").await?;
        anyhow::Ok(())
    }

    async fn rollback(&self, query_id: &str) -> anyhow::Result<()> {
        let conn = match self.query_connection.get(&query_id.to_string()) {
            Some(x) => x.clone(),
            None => anyhow::bail!(format!("id connection '{}' not found", query_id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.simple_query("ROLLBACK TRANSACTION;").await?;

        anyhow::Ok(())
    }

    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.new_tcp_conn().await?;
        self.query_connection
            .insert(id.to_owned(), Arc::new(Mutex::new(conn)));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        // no need execute close connection staff this connection not going to put back in pool
        self.query_connection.remove(&id.to_string());
        anyhow::Ok(())
    }
    async fn simple_execute_sql(&self, sql: &str) -> anyhow::Result<Option<usize>> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        conn.simple_query(sql.trim())
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
            None => anyhow::bail!(format!("id connection '{}' not found", query_id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let ret = match conn.simple_query(sql.trim()).await {
            Ok(..) => Ok(QueryResp::execute_ok(sql.to_owned())),
            Err(err) => Err(err.to_string()),
        };
        anyhow::Ok(ret)
    }

    async fn query_sql(
        &self,
        query_id: &str,
        sql: &str,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let conn = match self.query_connection.get(&query_id.to_string()) {
            Some(x) => x.clone(),
            None => anyhow::bail!(format!("id connection '{}' not found", query_id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let mut stream = conn.simple_query(sql).await?;
        while let Some(item) = stream.next().await {
            let item = match item {
                Ok(item) => item,
                Err(err) => {
                    emit.send(QueryEvent::Err(err.to_string()))?;
                    continue;
                }
            };
            match item {
                tiberius::QueryItem::Row(row) => {
                    emit.send(encode_row(&row)?.into())?;
                }
                tiberius::QueryItem::Metadata(meta) => {
                    let fs = get_fields_from_meta(&meta);
                    emit.send(
                        QueryResp {
                            fields: fs,
                            sql: Some(sql.to_owned()),
                            tables: crate::language_server::LangServerCommon::extract_tables(
                                self, sql,
                            )
                            .unwrap_or_default(),
                            ..Default::default()
                        }
                        .into(),
                    )?;
                }
            }
        }
        anyhow::Ok(())
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

    conn.simple_query(sql.as_str()).await?;
    anyhow::Ok(())
}

impl SqlServerDB {
    pub(crate) async fn get_conn(
        &self,
    ) -> anyhow::Result<bb8::PooledConnection<'static, bb8_tiberius::ConnectionManager>> {
        let conn = tokio::time::timeout(Duration::from_secs(5), async {
            self.pool.get_owned().await
        })
        .await??;
        anyhow::Ok(conn)
    }
}

#[async_trait::async_trait]
impl crate::db::DBTrait for SqlServerDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![
            Self::DBO_SCHEMA.to_string(),
            // Self::INFORMATION_SCHEMA_SCHEMA.to_string(),
            // Self::SYS_SCHEMA.to_string(),
        ])
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut x = Clone::clone(self);
        x.config.schema = Some(name.to_owned());
        x.query_connection = Default::default();
        Ok(Arc::new(x))
    }

    async fn create_schema(&self, _name: &str, _: Option<CreateSchemaExtra>) -> anyhow::Result<()> {
        self.can_write()?;
        self.backend_kind().not_support_feature("create schema")
    }

    async fn drop_schema(&self, _name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        self.backend_kind().not_support_feature("drop schema")
    }
}

#[async_trait::async_trait]
impl CatalogTrait for SqlServerDB {
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
        &self,
        name: &str,
        _extra: Option<CreateDatabaseExtra>,
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
pub(crate) async fn make_test_object() -> SqlServerDB {
    SqlServerDB::new(SqlServerDBConfig {
        host: crate::TEST_HOST.to_string(),
        conn_timeout: Some(5000.into()),
        port: 1433,
        username: Some("sa".to_string()),
        password: Some("sfdewRE123".to_string()),
        db: Some("test".to_string()),
        trust_cert: true,
        schema: Some(SqlServerDB::DBO_SCHEMA.to_string()),
        read_only: false,

        cert_ca: None,
    })
    .await
    .unwrap()
}
#[cfg(test)]
pub(crate) fn test_lock() -> &'static Mutex<()> {
    static LOCK: Mutex<()> = Mutex::const_new(());

    &LOCK
}
impl ReadOnlyTrait for SqlServerDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

/// Complete list of fixed catalog view/table names in the SQL Server `sys` schema.
pub const SYS_OBJECTS: &[&str] = &[
    "all_columns",
    "all_objects",
    "all_parameters",
    "all_sql_modules",
    "all_views",
    "allocation_units",
    "assemblies",
    "assembly_files",
    "assembly_modules",
    "assembly_references",
    "asymmetric_keys",
    "availability_databases_cluster",
    "availability_group_listener_ip_addresses",
    "availability_group_listeners",
    "availability_groups",
    "availability_groups_cluster",
    "availability_read_only_routing_lists",
    "availability_replicas",
    "backup_devices",
    "certificates",
    "change_tracking_databases",
    "change_tracking_tables",
    "check_constraints",
    "column_encryption_key_values",
    "column_encryption_keys",
    "column_master_keys",
    "column_store_dictionaries",
    "column_store_row_groups",
    "column_store_segments",
    "column_type_usages",
    "column_xml_schema_collection_usages",
    "columns",
    "computed_columns",
    "configurations",
    "conversation_endpoints",
    "conversation_groups",
    "conversation_priorities",
    "crypt_properties",
    "cryptographic_providers",
    "data_spaces",
    "database_audit_specification_details",
    "database_audit_specifications",
    "database_files",
    "database_filestream_options",
    "database_mirroring",
    "database_mirroring_endpoints",
    "database_mirroring_witnesses",
    "database_principals",
    "database_recovery_status",
    "database_role_members",
    "databases",
    "default_constraints",
    "destination_data_spaces",
    "dm_audit_actions",
    "dm_audit_class_type_map",
    "dm_broker_activated_tasks",
    "dm_broker_connections",
    "dm_broker_forwarded_messages",
    "dm_broker_queue_monitors",
    "dm_clr_appdomains",
    "dm_clr_loaded_assemblies",
    "dm_clr_properties",
    "dm_clr_tasks",
    "dm_cdc_errors",
    "dm_cdc_log_scan_sessions",
    "dm_cdc_errors",
    "dm_db_file_space_usage",
    "dm_db_log_space_usage",
    "dm_db_partition_stats",
    "dm_db_session_space_usage",
    "dm_db_task_space_usage",
    "dm_exec_background_job_queue",
    "dm_exec_background_job_queue_stats",
    "dm_exec_cached_plans",
    "dm_exec_connections",
    "dm_exec_procedure_stats",
    "dm_exec_query_memory_grants",
    "dm_exec_query_optimizer_info",
    "dm_exec_query_stats",
    "dm_exec_requests",
    "dm_exec_sessions",
    "dm_exec_sql_text",
    "dm_exec_text_query_plan",
    "dm_exec_trigger_stats",
    "dm_filestream_file_io_handles",
    "dm_filestream_file_io_requests",
    "dm_fts_active_catalogs",
    "dm_fts_fdhosts",
    "dm_fts_index_keywords",
    "dm_fts_index_keywords_by_document",
    "dm_fts_index_population",
    "dm_fts_memory_buffers",
    "dm_fts_memory_pools",
    "dm_fts_outstanding_batches",
    "dm_fts_population_ranges",
    "dm_io_backup_tapes",
    "dm_io_cluster_shared_drives",
    "dm_io_pending_io_requests",
    "dm_os_buffer_descriptors",
    "dm_os_child_instances",
    "dm_os_cluster_nodes",
    "dm_os_dispatcher_pools",
    "dm_os_hosts",
    "dm_os_latch_stats",
    "dm_os_loaded_modules",
    "dm_os_memory_allocations",
    "dm_os_memory_brokers",
    "dm_os_memory_cache_clock_hands",
    "dm_os_memory_cache_counters",
    "dm_os_memory_cache_entries",
    "dm_os_memory_cache_hash_tables",
    "dm_os_memory_clerks",
    "dm_os_memory_nodes",
    "dm_os_memory_objects",
    "dm_os_memory_pools",
    "dm_os_nodes",
    "dm_os_performance_counters",
    "dm_os_process_memory",
    "dm_os_schedulers",
    "dm_os_stacks",
    "dm_os_sys_info",
    "dm_os_sys_memory",
    "dm_os_tasks",
    "dm_os_virtual_address_dump",
    "dm_os_wait_stats",
    "dm_os_waiting_tasks",
    "dm_os_windows_info",
    "dm_qn_subscriptions",
    "dm_repl_articles",
    "dm_repl_schemas",
    "dm_repl_tranhash",
    "dm_repl_traninfo",
    "dm_resource_governor_configuration",
    "dm_resource_governor_resource_pools",
    "dm_resource_governor_workload_groups",
    "dm_server_audit_status",
    "dm_server_registry",
    "dm_server_services",
    "dm_tran_active_snapshot_database_transactions",
    "dm_tran_active_transactions",
    "dm_tran_commit_table",
    "dm_tran_current_snapshot",
    "dm_tran_current_transaction",
    "dm_tran_database_transactions",
    "dm_tran_locks",
    "dm_tran_session_transactions",
    "dm_tran_top_version_generators",
    "dm_tran_transactions_snapshot",
    "dm_tran_version_store",
    "dm_xe_map_values",
    "dm_xe_object_columns",
    "dm_xe_objects",
    "dm_xe_packages",
    "dm_xe_session_events",
    "dm_xe_session_object_columns",
    "dm_xe_session_targets",
    "dm_xe_sessions",
    "endpoints",
    "event_notification_event_types",
    "event_notifications",
    "events",
    "extended_procedures",
    "extended_properties",
    "filegroups",
    "filetable_system_defined_objects",
    "foreign_key_columns",
    "foreign_keys",
    "fulltext_catalogs",
    "fulltext_index_catalog_usages",
    "fulltext_index_columns",
    "fulltext_indexes",
    "fulltext_languages",
    "fulltext_stoplists",
    "fulltext_stopwords",
    "function_order_columns",
    "geography_columns",
    "geometry_columns",
    "hobt_tuples",
    "http_endpoints",
    "identity_columns",
    "index_columns",
    "indexes",
    "internal_tables",
    "key_constraints",
    "key_encryptions",
    "linked_logins",
    "linked_servers",
    "login_token",
    "master_files",
    "master_key_passwords",
    "message_type_xml_schema_collection_usages",
    "messages",
    "module_assembly_usages",
    "numbered_procedures",
    "objects",
    "parameter_type_usages",
    "parameter_xml_schema_collection_usages",
    "parameters",
    "partition_functions",
    "partition_parameters",
    "partition_range_values",
    "partition_schemes",
    "partitions",
    "plan_guides",
    "procedures",
    "queue_messages_1977058079",
    "queue_messages_2009058193",
    "queue_messages_2041058307",
    "remote_logins",
    "remote_service_bindings",
    "resource_governor_configuration",
    "resource_governor_resource_pools",
    "resource_governor_workload_groups",
    "routes",
    "schemas",
    "securable_classes",
    "server_audits",
    "server_event_notifications",
    "server_event_sessions",
    "server_permissions",
    "server_principals",
    "server_role_members",
    "service_broker_endpoints",
    "service_contract_message_usages",
    "service_contract_usages",
    "service_contracts",
    "service_message_types",
    "service_queue_usages",
    "service_queues",
    "services",
    "spatial_index_tessellations",
    "spatial_indexes",
    "spatial_reference_systems",
    "sql_dependencies",
    "sql_expression_dependencies",
    "sql_modules",
    "stats",
    "stats_columns",
    "symmetric_keys",
    "synonyms",
    "syslanguages",
    "table_types",
    "tables",
    "tcp_endpoints",
    "trace_categories",
    "trace_columns",
    "trace_event_bindings",
    "trace_events",
    "trace_subclass_values",
    "traces",
    "transmission_queue",
    "trigger_events",
    "triggers",
    "type_assembly_usages",
    "types",
    "user_token",
    "via_endpoints",
    "views",
    "xml_indexes",
    "xml_schema_attributes",
    "xml_schema_collections",
    "xml_schema_component_placements",
    "xml_schema_components",
    "xml_schema_elements",
    "xml_schema_facets",
    "xml_schema_model_groups",
    "xml_schema_namespaces",
    "xml_schema_types",
    "xml_schema_wildcards",
    "xml_schema_wildcard_namespaces",
];
/// All SQL Server INFORMATION_SCHEMA system views.
pub const INFORMATION_SCHEMA_VIEWS: &[&str] = &[
    "CHECK_CONSTRAINTS",
    "COLUMN_DOMAIN_USAGE",
    "COLUMN_PRIVILEGES",
    "COLUMNS",
    "CONSTRAINT_COLUMN_USAGE",
    "CONSTRAINT_TABLE_USAGE",
    "DOMAIN_CONSTRAINTS",
    "DOMAINS",
    "KEY_COLUMN_USAGE",
    "PARAMETERS",
    "REFERENTIAL_CONSTRAINTS",
    "ROUTINES",
    "ROUTINE_COLUMNS",
    "SCHEMATA",
    "TABLE_CONSTRAINTS",
    "TABLE_PRIVILEGES",
    "TABLES",
    "VIEW_COLUMN_USAGE",
    "VIEW_TABLE_USAGE",
    "VIEWS",
];

impl GetTestExportTableKinds for SqlServerDB {
    fn support_table_schema(&self) -> bool {
        false
    }
}
#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;
    use crate::{
        datasource_config::SSLFiles,
        db::{
            CatalogTrait, CreateIndexColumn, CreateTableReq, CreateTableReqColumn,
            ShowTableParameter, SqlNullAble, TestQueryEventEmit,
        },
        helper::{cert_files_dir, start_docker_container, DockerContainerDropper},
    };

    #[allow(unused_imports)]
    use crate::enable_test_log;

    #[tokio::test]
    async fn test_foreign_key() {
        let db = make_test_object().await;

        let table_name_posts = "test_posts";
        let table_name_comments = "test_comments";

        // SQL Server foreign key names must be specified by the user (usually custom or generated as table_name_column_name_FK).
        let _err = db
            .drop_foreign_key(table_name_comments, "fk_comments_posts")
            .await;

        if let Err(_err) = _err {
            println!("drop foreign key failed, err: {}", _err);
        }

        let _ = db.drop_table(table_name_comments, false).await;
        let _ = db.drop_table(table_name_posts, false).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();

        // 1. Create the parent table test_posts.
        // SQL Server uses IDENTITY(1,1) instead of AUTO_INCREMENT and NVARCHAR to support Unicode text.
        db.execute_sql(
            query_id,
            r#"
        CREATE TABLE test_posts (
            id INT IDENTITY(1,1) PRIMARY KEY,
            title NVARCHAR(255) NOT NULL,
            content NVARCHAR(MAX)
        );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        // 2. Create the child table test_comments.
        // SQL Server constraints are defined at the table level, with syntax similar to:
        db.execute_sql(
            query_id,
            r#"
        CREATE TABLE test_comments (
            comment_id INT IDENTITY(1,1) PRIMARY KEY,
            author_name NVARCHAR(255),
            post_id INT,
            CONSTRAINT fk_comments_posts FOREIGN KEY (post_id) 
            REFERENCES test_posts(id) 
            ON DELETE CASCADE
        );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.close_connection(query_id).await.unwrap();

        // 3. List and print the foreign keys.
        let f = db.list_foreign_key(table_name_comments).await.unwrap();

        assert!(f.len() == 1);

        for f in f.iter() {
            println!("foreign key: {:?}", f);
        }
    }

    #[tokio::test]
    async fn test_table_pagination() {
        let _guard = test_lock().lock().await;

        let db = make_test_object().await;
        let table_name = "test_table_pagination";
        let _ = db.drop_table(table_name, false).await;
        db.create_table(&CreateTableReq {
            name: table_name.into(),
            columns: vec![
                CreateTableReqColumn {
                    name: "id".into(),
                    data_type: "SMALLINT".into(),
                    auto_increment: false,
                    primary_key: true,
                    default_value: None,
                    nullable: SqlNullAble::Unspecified,
                },
                CreateTableReqColumn {
                    name: "name".into(),
                    data_type: "varchar(255)".into(),
                    auto_increment: false,
                    primary_key: true,
                    default_value: None,
                    nullable: SqlNullAble::Unspecified,
                },
            ],
            indexes: vec![],
        })
        .await
        .unwrap();

        for i in 0..155 {
            let mut values = HashMap::new();
            values.insert("id".to_owned(), Some((i + 1).to_string()));
            values.insert("name".to_owned(), Some(format!("name{}", i + 1)));
            db.table_insert(table_name, &values).await.unwrap();
        }
    }
    #[tokio::test]
    async fn test_procedure() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _ = db
            .drop_procedure(&NamedStaff::from("test_procedure".to_string()))
            .await;
        db.execute_sql(
            query_id,
            r#"
CREATE PROCEDURE dbo.test_procedure
AS
BEGIN
    SELECT 1;
END;
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let list = db.list_procedure().await.unwrap();
        for l in list.iter() {
            let details = db.show_procedure(l).await.unwrap();
            println!("udf:{},details:{:?}", l.name, details);
        }
    }

    #[tokio::test]
    async fn test_udf() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_udf(&NamedStaff::from("test_udf".to_string())).await;
        db.execute_sql(
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
        .unwrap()
        .unwrap();
        let list = db.list_udf().await.unwrap();
        for l in list.iter() {
            let details = db.show_udf(l).await.unwrap();
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
    async fn test_check_conn() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let metas = CheckConn::check_conn(&db).await.unwrap();
        println!("{:?}", metas);
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
        let db = SqlServerDB::new(SqlServerDBConfig {
            host: crate::TEST_HOST.to_string(),
            conn_timeout: Some(5000.into()),
            port: 1433,
            username: Some("sa".to_string()),
            password: Some("sfdewRE123".to_string()),
            db: None,
            trust_cert: true,
            schema: None,
            read_only: false,
            cert_ca: None,
        })
        .await
        .unwrap();

        let dbs = CatalogTrait::list_database(&db).await.unwrap();
        println!("{:?}", dbs);
    }

    #[tokio::test]
    async fn test_transaction() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let table_name = "test_commit";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
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
                == 0f64
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
                == 1f64
        );
    }

    #[tokio::test]
    async fn test_rename() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
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

        db.show_table_collect(
            "test_rename2",
            ShowTableParameter::new_stupid_test("id asc"),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_duplicate() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("test_duplicate", false).await;
        let _ = db.drop_table("test_duplicate2", false).await;

        db.execute_sql(
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

    #[tokio::test]
    async fn test_triggers() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("test_create_trigger", false).await;
        let _ = db.drop_table("test_create_trigger_update", false).await;
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_create_trigger (
                id INT PRIMARY KEY,
                name VARCHAR(50)
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
            id INT PRIMARY KEY,
            email VARCHAR(100)
        );
    "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();
        let tris = db.list_trigger("test_create_trigger").await.unwrap();
        for tri in tris.iter() {
            let detail = db
                .show_trigger("test_create_trigger", &tri.name)
                .await
                .unwrap();
            db.enable_trigger("test_create_trigger", &tri.name, false)
                .await
                .unwrap();
            db.enable_trigger("test_create_trigger", &tri.name, true)
                .await
                .unwrap();

            println!("trigger:{} details:{:?}", tri.name, detail);
        }
    }

    #[tokio::test]
    async fn test_query() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;

        let id = "";
        db.open_connection("").await.unwrap();
        db.execute_sql(
            id,
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
        .unwrap()
        .unwrap();
    }

    #[tokio::test]
    async fn test_list_schema() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let db_names = db.list_schema().await.unwrap();
        assert_eq!(db_names, vec![SqlServerDB::DBO_SCHEMA]);
        for db_name in db_names.iter() {
            db.open_schema(db_name).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_view() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let _ = db.drop_view("test_view").await;
        let _ = db.drop_table("test_view_table", false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"CREATE TABLE test_view_table (
                id INT PRIMARY KEY,
                name VARCHAR(50),
                age INT,
                email VARCHAR(100)
            )"#,
        )
        .await
        .unwrap()
        .unwrap();
        db.execute_sql(
            query_id,
            r#" INSERT INTO test_view_table (id, name, age, email)
                VALUES
                    (1, 'John Doe', 35, 'john.doe@example.com'),
                    (2, 'Jane Smith', 28, 'jane.smith@example.com'),
                    (3, 'Bob Johnson', 42, 'bob.johnson@example.com'),
                    (4, 'Sarah Lee', 31, 'sarah.lee@example.com'),
                    (5, 'Tom Wilson', 25, 'tom.wilson@example.com')"#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"CREATE VIEW test_view AS
                SELECT
                    id,
                    name,
                    age,
                    email
                FROM
                    test_view_table"#,
        )
        .await
        .unwrap()
        .unwrap();

        let resp = db
            .show_view_collect("test_view", ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }

    #[tokio::test]
    async fn test_index() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let table_name = "test_index_table";
        let _ = db.drop_table(table_name, false).await;
        let req = CreateTableReq {
            name: table_name.to_owned(),
            columns: vec![
                CreateTableReqColumn {
                    name: "Column1".to_string(),
                    data_type: "int".to_string(),
                    auto_increment: false,
                    primary_key: true,
                    default_value: Some(1.to_string()),
                    nullable: SqlNullAble::NotNull,
                },
                CreateTableReqColumn {
                    name: "name".to_string(),
                    data_type: "int".to_string(),
                    auto_increment: false,
                    primary_key: false,
                    default_value: None,
                    nullable: Default::default(),
                },
                CreateTableReqColumn {
                    name: "age".to_string(),
                    data_type: "int".to_string(),
                    auto_increment: false,
                    primary_key: false,
                    default_value: None,
                    nullable: Default::default(),
                },
            ],
            indexes: vec![],
        };
        db.create_table(&req).await.unwrap();
        std::thread::sleep(Duration::from_secs(1));
        let index_name = format!("{}_name_age", table_name);
        let index_name2 = format!("{}2", index_name);
        db.create_index(
            table_name,
            &CreateIndexReq {
                name: index_name.clone(),
                columns: vec![
                    CreateIndexColumn {
                        col: "name".into(),
                        asc: Some(true),
                    },
                    CreateIndexColumn {
                        col: "age".into(),
                        asc: Some(true),
                    },
                ],
                kind: IndexKind::Unique,
            },
        )
        .await
        .unwrap();

        db.rename_index(table_name, &index_name, &index_name2)
            .await
            .unwrap();
        db.rename_index(table_name, &index_name2, &index_name)
            .await
            .unwrap();

        let indexes = db.show_indexes(table_name).await.unwrap();
        for index in indexes.iter() {
            println!("index:{:?}", index);
        }
    }

    #[tokio::test]
    async fn test_export() {
        let _guard = test_lock().lock().await;
        let table_name = "test_export";
        let db = make_test_object().await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let rs = [
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
            let _ = db.execute_sql(query_id, s).await.unwrap();
        }

        GetTestExportTableKinds::test_export(&db, table_name).await;
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
    async fn test_table_not_impl_will_panic() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.query_sql(
            query_id,
            r#"
            SELECT   *
                FROM sys.sequences;

        "#,
            Arc::new(TestQueryEventEmit::default()),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_table_column() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;

        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_column
                (
                    id INT PRIMARY KEY                     -- 主键，无符号32位整数
                )
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
                    typ: "TINYINT".to_string(),
                    nullable: SqlNullAble::Null.into(),
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

        println!("add table column sql:{}", sql);

        let sql = db
            .modify_table_column(
                table_name,
                "name",
                "name2",
                Some(ColumnType {
                    typ: "SMALLINT".to_string(),
                    nullable: SqlNullAble::Null.into(),
                    after: None,
                    default_value: None,
                }),
            )
            .await
            .unwrap();

        println!("modify table column sql:{}", sql);

        let sql = db.drop_table_column(table_name, "name2").await.unwrap();

        println!("drop table column sql:{}", sql);
    }

    #[ignore = "run by hand"]
    #[tokio::test]
    async fn test_offset() {
        enable_test_log();

        let para = ShowTableParameter::new_stupid_test("id asc");
        let table_name = "test_type1";

        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        {
            let para = para.clone();
            db.show_table_collect(table_name, para).await.unwrap();
        }
        {
            let mut para = para.clone();
            para.page = None;
            para.page_size = None;
            db.show_table_collect(table_name, para).await.unwrap();
        }
        {
            let mut para = para.clone();
            para.page = None;
            para.sort = None;
            para.page_size = None;
            db.show_table_collect(table_name, para).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_type1() {
        enable_test_log();

        let _guard = test_lock().lock().await;
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
        db.table_insert(table_name, &values).await.unwrap();
        let mut wheres = HashMap::new();
        wheres.insert("id".to_string(), Some("1".to_string()));

        db.table_update(table_name, &values, &wheres).await.unwrap();
        let list = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("ret:{:?}", list);

        GetTestExportTableKinds::test_export(&db, table_name).await;
        db.test_table_export_and_import(table_name).await;
        db.test_table_equal(table_name, &values, &["id"]).await;
    }

    #[tokio::test]
    async fn test_type2() {
        let _guard = test_lock().lock().await;
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
                    nchar_column nchar(3),
                    char_column char(3),
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

        values.insert("nchar_column".to_string(), Some("d  ".to_string()));
        values.insert("char_column".to_string(), Some("e  ".to_string()));
        values.insert("ntext_column".to_string(), Some("f".to_string()));
        values.insert("text_column".to_string(), Some("g".to_string()));

        db.table_insert(table_name, &values).await.unwrap();
        let mut wheres = HashMap::new();
        wheres.insert("id".to_string(), Some("1".to_string()));
        db.table_update(table_name, &values, &wheres).await.unwrap();
        let list = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("ret:{:?}", list);
        GetTestExportTableKinds::test_export(&db, table_name).await;
        db.test_table_export_and_import(table_name).await;
        db.test_table_equal(table_name, &values, &["id"]).await;
    }

    #[tokio::test]
    async fn test_type3() {
        let _guard = test_lock().lock().await;
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
        values.insert(
            "BinaryColumn".to_string(),
            Some("0X01234567890000000000".to_string()),
        );
        values.insert("VarBinaryColumn".to_string(), Some("0XABCDEF".to_string()));
        values.insert(
            "ImageColumn".to_string(),
            Some("0X0123456789ABCDEF".to_string()),
        );

        values.insert(
            "UniqueIdentifierColumn".to_string(),
            Some("228e0edb-8941-4062-ab0b-0c9c25e23eb9".to_string()),
        );
        values.insert("NumericColumn".to_string(), Some("123.45".to_string()));
        values.insert("DecimalColumn".to_string(), Some("12.3456".to_string()));
        db.table_insert(table_name, &values).await.unwrap();
        let mut wheres = HashMap::new();
        wheres.insert("id".to_string(), Some("1".to_string()));
        db.table_update(table_name, &values, &wheres).await.unwrap();
        let list = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("ret:{:?}", list);
        GetTestExportTableKinds::test_export(&db, table_name).await;
        db.test_table_export_and_import(table_name).await;
        db.test_table_equal(table_name, &values, &["id"]).await;
    }

    #[tokio::test]
    async fn test_type4() {
        // enable_test_log();
        let _guard = test_lock().lock().await;
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
                    DateData1 DATE,
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
            Some("2022-01-01 12:34:56.123".to_string()),
        );
        values.insert(
            "SmallDateTimeData".to_string(),
            Some("2022-01-01 12:34:00".to_string()),
        );

        values.insert("DateData1".to_string(), Some("2022-01-01".to_string()));
        values.insert("TimeData".to_string(), Some("12:34:56".to_string()));
        values.insert(
            "DateTimeOffsetData".to_string(),
            Some("2022-01-01 12:34:56.1234567 +01:00".to_string()),
        );

        println!("values:{:?}", values);

        db.table_insert(table_name, &values).await.unwrap();

        let list = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("ret:{:?}", list);
        GetTestExportTableKinds::test_export(&db, table_name).await;
        db.test_table_export_and_import(table_name).await;
        db.test_table_equal(
            table_name,
            &values,
            &[
                "id",
                /*

                expect: 2022-01-01 12:34:56.1234567 +01:00
                This is 12:34:56 as seen in the UTC+1 time zone (such as standard time in Paris or Berlin).

                actual: 2022-01-01 11:34:56.123 +00:00
                 This is 11:34:56 as seen in UTC+0 (Coordinated Universal Time). */
                "DateTimeOffsetData",
            ],
        )
        .await;
    }

    // but macos not passed `A host name mismatch has occurred.`
    // This may pass on Linux; validation may be stricter on macOS.
    #[tokio::test]
    async fn test_ssl_sqlserver() {
        let port = portpicker::pick_unused_port().unwrap();
        println!("test port:{}", port);
        let cert_dir = cert_files_dir();

        // 1. Prepare the certificates and configuration required by SQL Server.
        // SQL Server requires the certificate and private key to be in the same .pem file or specified separately.
        // Here, assume server.crt and server.key are used and configured through mssql.conf.
        let ca_volume = format!("{}/ca.crt:/etc/ssl/certs/ca.crt", cert_dir);
        let cert_volume = format!("{}/server.crt:/etc/ssl/certs/server.crt", cert_dir);
        let key_volume = format!("{}/server.key:/etc/ssl/certs/server.key", cert_dir);

        // 2. Create the SQL Server configuration file mssql.conf to enable SSL.
        let mssql_conf_content = r#"
[network]
tlscert = /etc/ssl/certs/server.crt
tlskey = /etc/ssl/certs/server.key
tlsprotocols = 1.2
forceencryption = 1
"#;
        let conf_path = format!("{}/mssql.conf", cert_dir);
        std::fs::write(&conf_path, mssql_conf_content).unwrap();

        // 3. Create the initialization script (create a test database and user).
        let init_script_content = r#"
CREATE DATABASE testdb;
GO
USE testdb;
GO
CREATE LOGIN testuser WITH PASSWORD = 'testpass';
GO
CREATE USER testuser FOR LOGIN testuser;
GO
ALTER ROLE db_owner ADD MEMBER testuser;
GO
"#;
        let init_script_path = format!("{}/init-sqlserver.sql", cert_dir);
        std::fs::write(&init_script_path, init_script_content).unwrap();

        // 4. Start the SQL Server container with SSL configured correctly.
        let output = Command::new("docker")
            .arg("run")
            .arg("-d")
            .args(["--name", "sqlserver-ssl-test"])
            .args(["-p", format!("{}:1433", port).as_str()])
            .args(["-e", "ACCEPT_EULA=Y"])
            .args(["-e", "MSSQL_PID=Express"])
            .args(["-e", "SA_PASSWORD=YourStrong!Password"])
            .args(["-e", "MSSQL_AGENT_ENABLED=true"])
            // Mount the certificates and configuration file.
            .args(["-v", &ca_volume])
            .args(["-v", &cert_volume])
            .args(["-v", &key_volume])
            .args([
                "-v",
                format!("{}:/var/opt/mssql/mssql.conf", conf_path).as_str(),
            ])
            .args([
                "-v",
                format!("{}:/docker-entrypoint-initdb.d/init.sql", init_script_path).as_str(),
            ])
            .arg("mcr.microsoft.com/mssql/server")
            .output()
            .unwrap();

        let container_id = if output.status.success() {
            let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("SQL Server 容器启动成功，ID 为: {}", container_id);
            container_id
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("启动 Docker 容器失败: {}", stderr);
            return;
        };
        let _guard = DockerContainerDropper::from(&container_id);

        // SQL Server starts slowly, so wait longer.
        tokio::time::sleep(Duration::from_secs(10)).await;

        // Check the container logs to confirm the SSL configuration.
        let check_output = Command::new("docker")
            .args(["logs", &container_id, "--tail", "30"])
            .output()
            .unwrap();
        println!(
            "SQL Server logs: {}",
            String::from_utf8_lossy(&check_output.stdout)
        );

        start_docker_container(&container_id).unwrap();
        tokio::time::sleep(Duration::from_secs(5)).await;

        {
            let db = SqlServerDB::new(SqlServerDBConfig {
                host: "localhost".to_string(),
                port,
                username: Some("sa".to_string()),
                password: "YourStrong!Password".to_owned().into(),
                db: None,
                trust_cert: false,
                schema: None,
                read_only: false,
                conn_timeout: Some(5000.into()),
                cert_ca: SSLFiles::test_cert().ssl_ca,
            })
            .await
            .unwrap();

            let d = db.check_conn().await.unwrap();
            println!("check conn:{:?}", d);
        }
    }
}
