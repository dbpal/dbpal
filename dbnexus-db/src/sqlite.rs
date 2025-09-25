use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use futures_util::lock::Mutex;
use futures_util::{StreamExt, TryStreamExt};
use sqlparser::ast::{ColumnOption, Statement};
use sqlparser::tokenizer::Token;
use sqlx::pool::{PoolConnection, PoolOptions};
use sqlx::sqlite::SqliteRow;
use sqlx::{AssertSqlSafe, Column, Executor, Row, SqlSafeStr, SqliteConnection, TypeInfo};

use crate::MemoryLimit;
use crate::datasource_config::SqliteDBConfig;
use crate::db::{
    AutoCompletionQueryUuid, BackendKind, CheckConn, ColumnType, CreateTableAndIndexSql,
    ExportKind, ExportQueryResult, Field, GetBackendKind, Index, IndexKind, ListTable, NamedStaff,
    POOL_MAX_CONNECTIONS, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas,
    ShowExtraArgs, SqlBuilder, SqlExecuteRet, TriggerDetail, TriggerEvent, TriggerTiming, XlsxEdit,
    escape_sql_str, export_query_as_xlsx_send_data, not_support_feature,
};
use crate::helper::create_csv;
use crate::language_server::CompletionItem;
use crate::meta_value::MetaValue;

#[derive(Clone)]
pub struct SqliteDB {
    pool: sqlx::SqlitePool,
    read_only: bool,
    path: String,
    pub(crate) query_connection: Arc<DashMap<uuid::Uuid, Arc<Mutex<PoolConnection<sqlx::Sqlite>>>>>,
    pub(crate) build_in_fn_completion: Arc<Mutex<Vec<CompletionItem>>>,
    pub(crate) keywords_completion: Arc<Mutex<Vec<CompletionItem>>>,
    pub(crate) pragma_completion: Arc<Mutex<Vec<CompletionItem>>>,
}

impl SqliteDB {
    async fn create_table_or_view_sql(
        &self, table_name: &str, kind: &str,
    ) -> anyhow::Result<String> {
        let mut conn = self.get_conn().await?;
        let sql = format!(
            "SELECT sql FROM sqlite_master WHERE type = '{kind}' AND name = '{table_name}';"
        );
        log::info!("sql:{}", sql);
        let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_one(conn.deref_mut())
            .await?;
        let sql: String = row.try_get(0)?;
        anyhow::Ok(sql)
    }

    async fn get_table_indexes_sql(&self, table_name: &str) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let sql = format!(
            "SELECT sql FROM sqlite_master WHERE type = 'index' AND tbl_name = '{table_name}';"
        );
        log::info!("sql:{}", sql);
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;

        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let sql: String = r.try_get(0)?;
            ret.push(sql);
        }
        anyhow::Ok(ret)
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
        let mut conn = self.get_conn().await?;
        let cols = SchemaTrait::show_table_columns(self, table_name).await?;
        let sql = format!("SELECT * from {}", table_name);
        log::info!("sql:{}", sql);
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
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

                let insert_sql = self.insert_sql(None, table_name, &cols, &m)?;
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
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql)).fetch(conn.deref_mut());
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
        log::info!("sql:{}", sql);
        self.export_query_result_as_json_line(&sql, sender).await
    }

    async fn export_query_result_as_json(
        &self, sql: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql)).fetch(conn.deref_mut());
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
        log::info!("sql:{}", sql);
        self.export_query_result_as_json(sql.as_str(), pretty, sender)
            .await
    }

    async fn export_query_result_as_csv(
        &self, sql: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;

        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql))
            .fetch(conn.deref_mut())
            .try_chunks(100);
        let mut first = true;
        while let Some(rows) = stream.next().await {
            let rows = rows?;
            if first && !rows.is_empty() && include_header {
                let fs = rows
                    .first()
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

    async fn export_query_result_as_xlsx(
        &self, sql: &str, include_header: bool, sender: tokio::sync::mpsc::Sender<XlsxEdit>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql))
            .fetch(conn.deref_mut())
            .try_chunks(1000);
        let mut first = true;
        let mut row_num = 0;
        while let Some(rows) = stream.next().await {
            let rows = rows?;
            if first && !rows.is_empty() && include_header {
                for (index, col) in rows[0].columns().iter().enumerate() {
                    sender
                        .send(XlsxEdit {
                            row: row_num,
                            column: index as rust_xlsxwriter::ColNum,
                            content: col.name().to_string(),
                        })
                        .await?;
                }
                row_num += 1;
                first = false;
            }
            let data = encode_rows(&rows)?;
            export_query_as_xlsx_send_data(&mut row_num, data, sender.clone()).await?;
        }
        anyhow::Ok(())
    }

    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_csv(&sql, include_header, delimiter, sender.clone())
            .await
    }
}
// TODO sqlparser don't support parse trigger statement
// use lexer to solve this issue.
fn extract_trigger_detail_on_lexer(name: &str, sql: String) -> anyhow::Result<TriggerDetail> {
    let t = sqlparser::tokenizer::Tokenizer::new(&sqlparser::dialect::SQLiteDialect {}, &sql)
        .tokenize()?;
    let mut event = None;
    let mut timing = None;
    for token in t {
        if let Token::Word(x) = token {
            let word = x.to_string().to_uppercase();
            match word.as_str() {
                "INSERT" | "UPDATE" | "DELETE" => {
                    event = Some(word);
                }
                "AFTER" | "BEFORE" => {
                    timing = Some(word);
                }
                _ => {}
            }
        }
    }
    if event.is_none() || timing.is_none() {
        anyhow::bail!("trigger statement is not valid");
    }
    let event = event.unwrap();
    let timing = timing.unwrap();
    anyhow::Ok(TriggerDetail::new(
        name,
        match timing.as_str() {
            "AFTER" => TriggerTiming::After,
            "BEFORE" => TriggerTiming::Before,
            _ => unreachable!(),
        },
        TriggerEvent::try_from(event.as_str())?,
        sql,
        None,
    ))
}

pub(crate) fn extract_trigger_detail_from_sql(
    name: &str, sql: String,
) -> anyhow::Result<TriggerDetail> {
    extract_trigger_detail_on_lexer(name, sql)
}

async fn query_trigger_detail(
    conn: &mut SqliteConnection, name: &str,
) -> anyhow::Result<TriggerDetail> {
    let s = format!("SELECT sql from sqlite_master WHERE type = 'trigger' and name='{name}'");
    let row = sqlx::raw_sql(AssertSqlSafe(s.as_str()))
        .fetch_one(conn)
        .await?;
    let sql: String = row.get(0);
    extract_trigger_detail_from_sql(name, sql)
}

#[async_trait::async_trait]
impl CheckConn for SqliteDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let mut conn = self.get_conn().await?;
        let ins = std::time::Instant::now();
        sqlx::raw_sql("select 1")
            .fetch_all(conn.deref_mut())
            .await?;
        anyhow::Ok(ins.elapsed())
    }
}

#[async_trait::async_trait]

impl ServerMetas for SqliteDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let mut conn = self.get_conn().await?;
        let sql = r#"
            SELECT sqlite_version(); -- 0 
        "#;
        let row = sqlx::raw_sql(AssertSqlSafe(sql))
            .fetch_one(conn.deref_mut())
            .await?;
        let mut metas = vec![];
        let version: String = row.try_get(0)?;
        metas.push(MetaValue::new("version", version));
        anyhow::Ok(metas)
    }
}

impl std::fmt::Debug for SqliteDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sqlite on:{:?}", self.path)
    }
}

impl GetBackendKind for SqliteDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::Sqlite
    }
}

impl SqliteDB {
    pub(crate) async fn list_table_impl(&self, db: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql =
            format!(r#"SELECT name FROM "{db}".sqlite_master WHERE type='table' ORDER BY name "#);
        let rows = match self.query_connection.get(&AutoCompletionQueryUuid::get()) {
            Some(conn) => {
                let mut conn = conn.lock().await;
                sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                    .fetch_all(conn.deref_mut().deref_mut())
                    .await?
            }
            None => {
                let mut conn = self.get_conn().await?;
                sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                    .fetch_all(conn.deref_mut())
                    .await?
            }
        };
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let table_name: String = r.get(0);
            ret.push(table_name);
        }
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    pub(crate) async fn list_view_impl(&self, db: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"
            SELECT name
            FROM "{db}".sqlite_master
            WHERE type = 'view'; 
        "#
        );
        let rows = match self.query_connection.get(&AutoCompletionQueryUuid::get()) {
            Some(conn) => {
                let mut conn = conn.lock().await;
                sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                    .fetch_all(conn.deref_mut().deref_mut())
                    .await?
            }
            None => {
                let mut conn = self.get_conn().await?;
                sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                    .fetch_all(conn.deref_mut())
                    .await?
            }
        };
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            ret.push(name);
        }
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let increment_key = self.query_auto_increment_key(table_name).await?;
        let increment_key = increment_key.as_deref();

        let s = format!(r#"pragma table_info("{table_name}")"#);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(s.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        // cid name type     notnull  delt_value pk
        // 0	id	INTEGER	 0		  null      1
        for r in rows.iter() {
            let name: String = r.get(1);
            let ty: String = r.get(2);
            let not_null: i64 = r.get(3);
            let delt_value: Option<String> = r.get(4);
            let pk: Option<i64> = r.get(5);
            let is_auto_increment = Some(name.as_str()) == increment_key;

            ret.push(Field::new(
                name,
                ty,
                not_null == 0,
                delt_value,
                pk.map(|x| x == 1).unwrap_or_default(),
                is_auto_increment,
            ));
        }

        anyhow::Ok(ret)
    }
}

impl SqliteDB {
    pub(crate) const MAIN_DB: &'static str = "main";
}
#[async_trait::async_trait]
impl SchemaTrait for SqliteDB {
    async fn export_table_or_view_as_xlsx(
        &self, name: &str, include_header: bool, sender: tokio::sync::mpsc::Sender<XlsxEdit>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"SELECT * FROM "{}""#, name);
        self.export_query_result_as_xlsx(&sql, include_header, sender)
            .await
    }

    fn list_sql_current_values(&self) -> &'static [&'static str] {
        <Self as SqlBuilder>::CURRENT_VALUES
    }

    fn list_data_types(&self) -> anyhow::Result<&'static [&'static str]> {
        anyhow::Ok(crate::sqlite_lang_server::SQLITE_TYPES)
    }

    /*
        在 SQLite 中，直接修改列类型（ALTER COLUMN TYPE）是不支持的。SQLite 的 ALTER TABLE 语句功能有限，它只允许：
    */
    async fn modify_table_column(
        &self, _table: &str, _old_name: &str, _new_name: &str, _typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
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
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        let sql = format!(r#"ALTER TABLE "{table}" DROP COLUMN "{col}";"#);
        let mut conn = self.get_conn().await?;
        conn.execute(AssertSqlSafe(sql.as_str())).await?;
        anyhow::Ok(())
    }

    async fn create_index(
        &self, table: &str, name: &str, kind: IndexKind, fields: &[String],
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"CREATE {} INDEX "{}" ON "{table}"({})"#,
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

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP INDEX "{index}";"#);
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        use sqlx::{Executor, Statement};
        let mut conn = self.get_conn().await?;
        let sql = format!(r#"select * from "{view_name}" limit 1"#);
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

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        if self.table_exists(dup).await? {
            anyhow::bail!(format!("table '{}' already exists", dup));
        }
        let sql = format!(r#"CREATE TABLE "{dup}" AS SELECT * FROM "{table}""#);
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;

        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let sql = format!(r#"ALTER TABLE "{old}" RENAME TO "{new}""#);
        log::info!("sql:{}", sql);
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn list_trigger(&self, table_name: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
            SELECT name 
            FROM sqlite_master 
            WHERE type = 'trigger' AND tbl_name = '{table_name}';
        "#
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

    async fn show_trigger(&self, _table: &str, name: &str) -> anyhow::Result<TriggerDetail> {
        let mut conn = self.get_conn().await?;
        query_trigger_detail(conn.deref_mut(), name).await
    }

    async fn drop_trigger(&self, _table: &str, name: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
        DROP TRIGGER "{name}"
        "#
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn create_table_and_index_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let table = self.create_table_or_view_sql(table_name, "table").await?;
        let indexes = self.get_table_indexes_sql(table_name).await?;

        anyhow::Ok(CreateTableAndIndexSql {
            create_table: table,
            indexes,
        })
    }

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
            ExportKind::Xlsx { .. } => anyhow::bail!("Not implemented"),
        }
    }

    async fn create_view_sql(&self, view_name: &str, _: bool) -> anyhow::Result<String> {
        self.create_table_or_view_sql(view_name, "view").await
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_view_impl(Self::MAIN_DB).await
    }
    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.can_write()?;

        let s = format!(r#"DROP VIEW "{view_name}""#);
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(s.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let s = format!(
            r#"DROP TABLE "{table}" {}"#,
            if cascade { " CASCADE " } else { "" }
        );
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(s.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
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
            DELETE FROM "{table_name}";
            DELETE FROM sqlite_sequence WHERE name = '{table_name}';
        "#
        );
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(s.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_table_columns(table_name).await?;

        let mut conn = self.get_conn().await?;
        let sql = self.update_sql(None, table_name, &fields, &values, &wheres)?;
        log::info!("sql:{}", sql);

        let r = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(r.rows_affected()),
        })
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_table_columns(table_name).await?;
        let sql = self.insert_sql(None, table_name, &fields, &values)?;
        log::info!("insert sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let r = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(SqlExecuteRet {
            sql,

            changes: Some(r.rows_affected()),
        })
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(Self::MAIN_DB).await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let increment_key = self.query_auto_increment_key(table_name).await?;
        let increment_key = increment_key.as_deref();

        let s = format!(r#"pragma table_info("{table_name}")"#);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(s.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        // cid name type     notnull  delt_value pk
        // 0	id	INTEGER	 0		  null      1
        for r in rows.iter() {
            let name: String = r.get(1);
            let ty: String = r.get(2);
            let not_null: i64 = r.get(3);
            let delt_value: Option<String> = r.get(4);
            let pk: Option<i64> = r.get(5);
            let is_auto_increment = Some(name.as_str()) == increment_key;

            ret.push(Field::new(
                name,
                ty,
                not_null == 0,
                delt_value,
                pk.map(|x| x == 1).unwrap_or_default(),
                is_auto_increment,
            ));
        }

        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        log::trace!("begin to show table");
        let total = self.query_count(table_name, where_str).await?;
        log::trace!("query count ok");
        let sql = gen_show_sql(table_name, where_str, order_by, page, page_size);
        log::info!("show table sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await
            .map_err(|e| anyhow::Error::msg(format!("err:'{}' sql:'{}'", e, sql)))?;

        let data = encode_rows(&row)?;

        log::trace!("encode rows ok");
        let fields = self.show_table_columns(table_name).await?;
        log::trace!("list columns ok");
        anyhow::Ok(QueryResp::new(fields, total, data, sql)?)
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        let mut conn = self.get_conn().await?;
        let total = self.query_count(view_name, where_str).await?;
        let sql = gen_show_sql(view_name, where_str, order_by, page, page_size);
        log::info!("show table sql:{}", sql);
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        if rows.is_empty() {
            let f = self.show_view_columns(view_name).await?;
            return anyhow::Ok(QueryResp::new(f, 0, vec![], sql)?);
        }
        let data = encode_rows(&rows)?;
        let fields = get_rows_types(&rows);
        anyhow::Ok(QueryResp::new(fields, total, data, sql)?)
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let sql = format!("PRAGMA index_list({table_name})");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;

        // seq name unique origin partial
        // 0   xxx  1      c      0

        let mut ret = Vec::with_capacity(rows.len());

        for r in rows.iter() {
            let name: String = r.get(1);
            let unique: i64 = r.get(2);
            // query create index sql
            let sql = format!("PRAGMA index_info('{name}')");
            log::info!("sql:{}", sql);
            let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .fetch_all(conn.deref_mut())
                .await?;
            let mut on = Vec::with_capacity(rows.len());

            for r in rows.iter() {
                let name: String = r.try_get(2)?;
                on.push(name);
            }
            ret.push(Index::new(
                name,
                if unique == 1 {
                    IndexKind::Unique
                } else {
                    IndexKind::Index
                },
                on,
            ))
        }
        anyhow::Ok(ret)
    }
}

impl SqliteDB {
    async fn query_auto_increment_key(&self, table_name: &str) -> anyhow::Result<Option<String>> {
        let s = format!(
            r#"SELECT sql from sqlite_master where type='table' and name='{}'"#,
            table_name
        );
        let mut conn = self.get_conn().await?;
        let row = sqlx::raw_sql(AssertSqlSafe(s.as_str()))
            .fetch_one(conn.deref_mut())
            .await?;
        let sql: String = row.get(0);
        extract_auto_increment_key(sql.as_str())
    }
}

fn get_rows_types(rows: &Vec<SqliteRow>) -> Vec<Field> {
    if rows.is_empty() {
        return vec![];
    }
    let mut ret = Vec::with_capacity(rows[0].columns().len());
    for c in rows[0].columns().iter() {
        ret.push(Field::from_name_type(
            c.name().to_string(),
            c.type_info().to_string(),
        ));
    }
    ret
}

/// sqlite有哪些值可以直接放在sql语句里面的想current timestamp给我完成的rust列表
/// SQLite 里可以直接写在 SQL 语句中的时间字面量
pub const SQLITE_CURRENT_LITERALS: &[&str] = &[
    "CURRENT_TIME",      // 15:23:56
    "CURRENT_DATE",      // 2025-08-25
    "CURRENT_TIMESTAMP", // 2025-08-25 15:23:56
];

impl SqlBuilder for SqliteDB {
    const CURRENT_VALUES: &'static [&'static str] = SQLITE_CURRENT_LITERALS;
    fn put_value_in_sql(
        &self, typ: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        if self.hit_current_value(value) {
            return anyhow::Result::Ok(value.unwrap().to_string());
        }
        let typ = typ.to_uppercase();

        let v = match typ.as_str() {
            "" | "NULL" => match value {
                Some(x) => {
                    if x.parse::<f64>().is_ok() {
                        x.to_string()
                    } else if x.parse::<i64>().is_ok() {
                        x.to_string()
                    } else {
                        format!("'{}'", escape_sql_str(x))
                    }
                }
                None => "null".to_string(),
            },
            "TEXT" | "DATE" | "TIME" | "DATETIME" => match value {
                Some(x) => format!("'{}'", escape_sql_str(x)),
                None => "null".to_string(),
            },
            _ if text_like(typ.as_str()) => match value {
                Some(x) => format!("'{}'", escape_sql_str(x)),
                None => "null".to_string(),
            },
            "REAL" | "INTEGER" | "NUMERIC" | "BOOLEAN" => value.unwrap_or("null").to_string(),
            _ if integer_like(typ.as_str()) || numeric_like(typ.as_str()) => {
                value.unwrap_or("null").to_string()
            }
            "BLOB" => value.unwrap_or("null").to_string(),
            _ => {
                let e = format!("type '{}' not handled", typ);
                log::error!("{}", e);
                anyhow::bail!(e)
            }
        };

        anyhow::Ok(v)
    }
}

pub(crate) fn encode_bytes(bs: &[u8]) -> String {
    format!("X'{}'", hex::encode_upper(bs))
}

fn encode_row_as_json(r: &SqliteRow) -> anyhow::Result<serde_json::Value> {
    let mut values = serde_json::Map::default();
    let null_value = serde_json::Value::Null;

    for c in 0..r.columns().len() {
        let typ = r.column(c).type_info().name().to_uppercase();
        let v: serde_json::Value = match typ.as_str() {
            "NULL" | "" => {
                // null table is not always null. I quote `suitable for
                // representing missing or unknown data`
                // try get possible values
                if let Result::Ok(s) = r.try_get::<'_, f64, _>(c) {
                    serde_json::Value::from(s)
                } else if let Result::Ok(s) = r.try_get::<'_, Vec<u8>, _>(c) {
                    serde_json::Value::from(encode_bytes(s.as_slice()))
                } else if let Result::Ok(s) = r.try_get::<'_, i64, _>(c) {
                    serde_json::Value::from(s)
                } else if let Result::Ok(s) = r.try_get::<'_, String, _>(c) {
                    serde_json::Value::from(s)
                } else {
                    null_value.clone()
                }
            }

            _ if text_like(&typ) => {
                let v: Option<String> = r.try_get(c)?;
                v.map(serde_json::Value::String)
                    .unwrap_or(null_value.clone())
            }
            _ if real_like(&typ) => {
                let v: Option<f64> = r.try_get(c)?;
                v.map(serde_json::Value::from).unwrap_or(null_value.clone())
            }
            _ if integer_like(&typ) => {
                let v: Result<Option<i64>, _> = r.try_get(c);
                match v {
                    Ok(x) => x.map(serde_json::Value::from).unwrap_or(null_value.clone()),
                    Err(_) => {
                        // sometimes can save a float in here
                        let v: Option<f64> = r.try_get(c)?;
                        v.map(serde_json::Value::from).unwrap_or(null_value.clone())
                    }
                }
            }
            "BLOB" => {
                let v: Option<Vec<u8>> = r.try_get(c)?;
                v.map(|x| {
                    serde_json::Value::Array(x.into_iter().map(serde_json::Value::from).collect())
                })
                .unwrap_or(null_value.clone())
            }

            _ if numeric_like(&typ) => {
                let v: Option<f64> = r.try_get(c)?;
                v.map(serde_json::Value::from).unwrap_or(null_value.clone())
            }
            // non-standard extensions 这些实际上不存在？
            "BOOLEAN" => {
                let v: Option<bool> = r.try_get(c)?;
                v.map(serde_json::Value::from).unwrap_or(null_value.clone())
            }
            "DATE" => {
                let v: Option<chrono::NaiveDate> = r.try_get(c)?;
                v.map(|x| serde_json::Value::from(x.to_string()))
                    .unwrap_or(null_value.clone())
            }
            "TIME" => {
                let v: Option<chrono::NaiveTime> = r.try_get(c)?;
                v.map(|x| serde_json::Value::from(x.to_string()))
                    .unwrap_or(null_value.clone())
            }
            "DATETIME" => {
                let v: Option<chrono::NaiveDateTime> = r.try_get(c)?;
                v.map(|x| serde_json::Value::from(x.to_string()))
                    .unwrap_or(null_value.clone())
            }
            _ => anyhow::bail!(format!("type '{typ}' not supported")),
        };

        values.insert(r.column(c).name().to_owned(), v);
    }
    anyhow::Ok(serde_json::Value::Object(values))
}
pub(crate) fn integer_like(typ: &str) -> bool {
    typ == "INT"
        || typ == "INTEGER"
        || typ == "TINYINT"
        || typ == "SMALLINT"
        || typ == "MEDIUMINT"
        || typ == "BIGINT"
        || typ == "UNSIGNED BIG INT"
        || typ == "INT2"
        || typ == "INT8"
}

pub(crate) fn numeric_like(typ: &str) -> bool {
    typ == "NUMERIC" || typ.starts_with("DECIMAL")
}

pub(crate) fn text_like(typ: &str) -> bool {
    typ.starts_with("CHARACTER")
        || typ.starts_with("VARCHAR")
        || typ.starts_with("VARYING CHARACTER")
        || typ.starts_with("NCHAR")
        || typ.starts_with("NATIVE CHARACTER")
        || typ.starts_with("NVARCHAR")
        || typ == "TEXT"
        || typ == "CLOB"
}

pub(crate) fn real_like(typ: &str) -> bool {
    typ == "REAL" || typ == "DOUBLE" || typ == "DOUBLE PRECISION" || typ == "FLOAT"
}

fn encode_rows(rows: &Vec<SqliteRow>) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let mut ret = Vec::with_capacity(rows.len());

    for r in rows.iter() {
        let mut values = Vec::with_capacity(r.len());
        for c in 0..r.len() {
            let ty = r.column(c).type_info().name();

            let v = match ty {
                "NULL" | "" => {
                    // null table is not always null. I quote 'suitable for
                    // representing missing or unknown data'
                    // try get possible values
                    if let Result::Ok(s) = r.try_get::<'_, f64, _>(c) {
                        Some(s.to_string())
                    } else if let Result::Ok(s) = r.try_get::<'_, i64, _>(c) {
                        Some(s.to_string())
                    } else if let Result::Ok(s) = r.try_get(c) {
                        Some(s)
                    } else if let Result::Ok(s) = r.try_get::<'_, bool, _>(c) {
                        Some(s.to_string())
                    } else if let Result::Ok(s) = r.try_get::<'_, Vec<u8>, _>(c) {
                        Some(encode_bytes(s.as_slice()))
                    } else {
                        None
                    }
                }

                "TEXT" => {
                    let v: Option<String> = r.try_get(c)?;
                    v
                }
                "REAL" => {
                    let v: Option<f64> = r.try_get(c)?;
                    v.map(|x| format!("{}", x))
                }
                "BLOB" => {
                    let v: Option<Vec<u8>> = r.try_get(c)?;
                    v.map(|x| encode_bytes(x.as_slice()))
                }

                _ if integer_like(ty) => {
                    let v: Result<Option<i64>, _> = r.try_get(c);
                    match v {
                        Ok(x) => x.map(|x| x.to_string()),
                        Err(_) => {
                            // sometimes can save a float in here
                            let v: Option<f64> = r.try_get(c)?;
                            v.map(|x| format!("{}", x))
                        }
                    }
                }
                _ if numeric_like(ty) => {
                    let v: Option<f64> = r.try_get(c)?;
                    v.map(|x| format!("{}", x))
                }

                "BOOLEAN" => {
                    let v: Option<bool> = r.try_get(c)?;
                    v.map(|x| format!("{}", x))
                }
                "DATE" => {
                    let v: Option<chrono::NaiveDate> = r.try_get(c)?;
                    v.map(|x| format!("{}", x))
                }
                "TIME" => {
                    let v: Option<chrono::NaiveTime> = r.try_get(c)?;
                    v.map(|x| format!("{}", x))
                }
                "DATETIME" => {
                    let v: Option<chrono::NaiveDateTime> = r.try_get(c)?;
                    v.map(|x| format!("{}", x))
                }
                _ => anyhow::bail!(format!("type '{ty}' not supported")),
            };

            values.push(v);
        }
        ret.push(values);
    }
    anyhow::Ok(ret)
}

impl SqliteDB {
    pub async fn new(conn: SqliteDBConfig, read_only: bool) -> anyhow::Result<Self> {
        let mut opts: PoolOptions<sqlx::sqlite::Sqlite> = PoolOptions::new();
        opts = opts.max_connections(POOL_MAX_CONNECTIONS);
        if let Some(timeout) = conn.conn_timeout {
            opts = opts.acquire_timeout(Duration::from_millis(timeout as u64));
        }
        let pool = opts.connect(conn.path.as_str()).await?;

        if read_only {
            let r = sqlx::raw_sql("PRAGMA query_only = true;")
                .execute(pool.acquire().await?.deref_mut())
                .await;
            match r {
                Ok(_) => {}
                Err(err) => anyhow::bail!(format!("execute query only failed,err:{}", err)),
            }
        }
        anyhow::Ok(Self {
            pool,
            read_only,
            query_connection: Default::default(),
            path: conn.path.clone(),
            build_in_fn_completion: Default::default(),
            keywords_completion: Default::default(),
            pragma_completion: Default::default(),
        })
    }

    async fn query_count(&self, name: &str, where_str: Option<&str>) -> anyhow::Result<usize> {
        let mut conn = self.get_conn().await?;
        let mut s = format!(r#"SELECT count(1) from "{name}" "#);
        if let Some(where_str) = where_str {
            s.push_str(format!(" where {where_str}").as_str());
        }
        let row = sqlx::raw_sql(AssertSqlSafe(s.as_str()))
            .fetch_one(conn.deref_mut())
            .await
            .map_err(|e| anyhow::Error::msg(format!("err:'{}' sql:'{}'", e, s)))?;
        let count: i64 = row.get(0);
        anyhow::Ok(count as usize)
    }

    pub async fn get_conn(&self) -> anyhow::Result<PoolConnection<sqlx::Sqlite>> {
        let conn = self.pool.acquire().await?;
        anyhow::Ok(conn)
    }
}

pub(crate) fn extract_auto_increment_key(sql: &str) -> anyhow::Result<Option<String>> {
    use sqlparser::dialect::SQLiteDialect;
    let ss = sqlparser::parser::Parser::parse_sql(&SQLiteDialect {}, sql)?;
    if ss.len() != 1 {
        anyhow::bail!("should contains one statement");
    }
    let s = ss.first().unwrap();
    match s {
        Statement::CreateTable(create_table) => {
            for c in create_table.columns.iter() {
                if c.options.iter().any(|x| match &x.option {
                    ColumnOption::DialectSpecific(x) => x.iter().any(|x| match x {
                        Token::Word(w) => {
                            let x = w.value.to_uppercase();
                            x == "AUTOINCREMENT"
                        }
                        _ => false,
                    }),
                    _ => false,
                }) {
                    return anyhow::Ok(Some(c.name.to_string()));
                }
            }
        }
        Statement::CreateVirtualTable { .. } => return anyhow::Ok(None),
        _ => {
            log::error!("should be a create table statement,but got {}", sql);
            return anyhow::Ok(None);
        }
    };
    anyhow::Ok(None)
}
#[async_trait::async_trait]
impl ExportQueryResult for SqliteDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }

    async fn export_as_xlsx(
        &self, sql: &str, include_header: bool, sender: tokio::sync::mpsc::Sender<XlsxEdit>,
    ) -> anyhow::Result<()> {
        self.export_query_result_as_xlsx(sql, include_header, sender)
            .await
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
            ExportKind::Xlsx { .. } => {
                anyhow::bail!("xlsx export is not supported for sqlite")
            }
        }
    }
}

#[async_trait::async_trait]
impl QuerySql for SqliteDB {
    async fn get_auto_commit(&self, query_id: uuid::Uuid) -> anyhow::Result<bool> {
        use sqlx_core::any::AnyConnectionBackend;

        let conn = self.query_connection.get(&query_id);
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let d = conn.get_transaction_depth();
        anyhow::Ok(d == 0)
    }

    async fn set_auto_commit(&self, query_id: uuid::Uuid, commit: bool) -> anyhow::Result<()> {
        if commit {
            anyhow::bail!("sqlite use  commit or rollback to end transaction");
        }
        use sqlx_core::any::AnyConnectionBackend;
        let conn = self.query_connection.get(&query_id);
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };

        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.begin(None).await?;

        anyhow::Ok(())
    }

    async fn commit(&self, query_id: uuid::Uuid) -> anyhow::Result<()> {
        use sqlx_core::any::AnyConnectionBackend;

        let conn = self.query_connection.get(&query_id);
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.commit().await?;

        anyhow::Ok(())
    }

    async fn rollback(&self, query_id: uuid::Uuid) -> anyhow::Result<()> {
        use sqlx_core::any::AnyConnectionBackend;

        let conn = self.query_connection.get(&query_id);
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.rollback().await?;

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
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        crate::language_server::LangServerCommon::contains_tx_operation_error(self, sql)?;
        let conn = self.query_connection.get(&query_id);
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
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        crate::language_server::LangServerCommon::contains_tx_operation_error(self, sql)?;
        let conn = self.query_connection.get(&query_id);
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };

        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();

        let mut ret: Vec<Result<QueryResp, String>> = Vec::new();
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql)).fetch_many(conn.deref_mut());

        let mut rows = Vec::new();
        let mut ml = MemoryLimit::new();
        while let Some(item) = stream.next().await {
            ml.exceed_error()?;
            let item = match item {
                Ok(x) => x,
                Err(err) => {
                    ret.push(Err(err.to_string()));

                    break;
                }
            };
            match item {
                sqlx::Either::Left(row_aff) => {
                    if !rows.is_empty() {
                        let fields = get_rows_types(&rows);
                        let data = encode_rows(&rows)?;
                        ret.push(Ok(QueryResp::new(fields, 0, data, None)?));
                        rows.clear();
                    };

                    if row_aff.rows_affected() > 0 {
                        ret.push(Ok(QueryResp::row_affected(row_aff.rows_affected(), None)));
                    }
                }
                sqlx::Either::Right(row) => {
                    rows.push(row);
                }
            }
        }
        drop(stream);

        if ret.is_empty() {
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
            ret = vec![Ok(QueryResp::new(fields, 0, vec![], sql.to_owned())?)];
        }

        anyhow::Ok(ret)
    }

    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        let conn = self.get_conn().await?;
        self.query_connection.insert(id, Arc::new(Mutex::new(conn)));
        anyhow::Ok(id)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        if let Some((_, _x)) = self.query_connection.remove(&id) {}
        let conn = self.get_conn().await?;
        self.query_connection.insert(id, Arc::new(Mutex::new(conn)));

        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        if let Some((_, _x)) = self.query_connection.remove(&id) {}
        anyhow::Ok(())
    }
}

pub(crate) fn gen_show_sql(
    table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize,
) -> String {
    let mut sql = format!(r#"SELECT * FROM "{table_name}""#);
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

#[cfg(test)]

// this test no need clean up, we use memory sqlite db.
pub(crate) async fn make_test_object() -> SqliteDB {
    let path = std::env::var("DBPAL_SQLITE_TEST_FILE").unwrap();

    let db = SqliteDB::new(
        SqliteDBConfig {
            path,
            read_only: false,
            conn_timeout: Some(1000),
        },
        false,
    )
    .await
    .unwrap();
    db
}

impl ReadOnlyTrait for SqliteDB {
    fn read_only(&self) -> bool {
        self.read_only
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enable_test_log;

    #[ignore = "this test run by hand"]
    #[tokio::test]
    async fn test_do_has_data_query() {
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        let now = std::time::Instant::now();
        let _x = db
            .query_sql(query_id, "SELECT * from has_data_0")
            .await
            .unwrap();

        // println!("{:?}", x);
        println!("time used:{:?}", std::time::Instant::now() - now);
    }
    #[tokio::test]
    async fn test_show_xxx() {
        let db = make_test_object().await;
        let r = db
            .show_table("Persons", None, None, 1, 100, None)
            .await
            .unwrap();
        for c in r.fields.iter() {
            println!("{:?}", c);
        }
        for d in r.data.iter() {
            println!("{:?}", d);
        }
    }

    #[tokio::test]
    async fn test_table_column() {
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
                    id INTEGER PRIMARY KEY AUTOINCREMENT                      -- 主键，无符号32位整数
                )
                "#,
            )
            .await
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

        db.drop_table_column(table_name, "name").await.unwrap();
    }

    #[test]
    fn test_extract_trigger_detail_from_sql() {
        {
            let x = extract_trigger_detail_from_sql(
                "after_insert_test_create_trigger",
                r#"
            CREATE TRIGGER after_insert_test_create_trigger
            AFTER INSERT ON test_create_trigger
            FOR EACH ROW
            BEGIN
                INSERT INTO test_create_trigger_update (info)
                VALUES (NEW.info);
            END
        "#
                .to_owned(),
            )
            .unwrap();
            assert_eq!(x.event, TriggerEvent::Insert);
        }
    }
    #[test]
    fn test_detect_auto_increment() {
        let x = extract_auto_increment_key(
            r#"
    CREATE TABLE users (
        id INTEGER PRIMARY KEY autoincrement,
        name TEXT,
        age INTEGER
      )
    "#,
        )
        .unwrap();
        assert!(x == Some("id".to_string()));
        let x = extract_auto_increment_key(
            r#"
    CREATE TABLE users (
        id INTEGER PRIMARY KEY,
        name TEXT,
        age INTEGER
      )
    "#,
        )
        .unwrap();
        assert!(x == None);
    }

    #[test]
    fn test_extract_trigger_base_on_lexer() {
        {
            let sql = "after insert".to_string();
            assert_eq!(
                extract_trigger_detail_on_lexer("", sql.clone()).unwrap(),
                TriggerDetail::new("", TriggerTiming::After, TriggerEvent::Insert, sql, None)
            );
        }
        {
            let sql = "after update".to_string();
            assert_eq!(
                extract_trigger_detail_on_lexer("", sql.clone()).unwrap(),
                TriggerDetail::new("", TriggerTiming::After, TriggerEvent::Update, sql, None)
            );
        }
        {
            let sql = "after delete".to_string();
            assert_eq!(
                extract_trigger_detail_on_lexer("", sql.clone()).unwrap(),
                TriggerDetail::new("", TriggerTiming::After, TriggerEvent::Delete, sql, None)
            );
        }
        {
            let sql = "before insert".to_string();
            assert_eq!(
                extract_trigger_detail_on_lexer("", sql.clone()).unwrap(),
                TriggerDetail::new("", TriggerTiming::Before, TriggerEvent::Insert, sql, None)
            );
        }
        {
            let sql = "before update".to_string();
            assert_eq!(
                extract_trigger_detail_on_lexer("", sql.clone()).unwrap(),
                TriggerDetail::new("", TriggerTiming::Before, TriggerEvent::Update, sql, None)
            );
        }
        {
            let sql = "before delete".to_string();
            assert_eq!(
                extract_trigger_detail_on_lexer("", sql.clone()).unwrap(),
                TriggerDetail::new("", TriggerTiming::Before, TriggerEvent::Delete, sql, None)
            );
        }

        // error cases
        {
            let sql = "before xxx".to_string();
            assert!(extract_trigger_detail_on_lexer("", sql.clone()).is_err());
        }
        {
            let sql = "insert xxx".to_string();
            assert!(extract_trigger_detail_on_lexer("", sql.clone()).is_err());
        }
    }

    #[tokio::test]
    async fn test_transaction() {
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

        assert!(!db.get_auto_commit(query_id).await.unwrap());

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

    #[ignore = "this test run by hand"]
    #[tokio::test]
    async fn test_do_has_data() {
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        for i in 0..10 {
            let table_name = format!("has_data_{}", i);
            let err = db.drop_table(&table_name, false).await;
            if let Err(err) = err {
                println!("drop table error:{:?}", err);
            }
            let create_table = format!(
                r#"create table {table_name} (
                col1 real,
                col2 real,
                col3 real,
                col4 real,
                col5 real,
                col6 real,
                col7 real
        )
            "#
            );

            let result = db.query_sql(query_id.clone(), &create_table).await;
            println!("create table result:{:?}", result);
            for _ in 0..1550 {
                let sql = format!(
                    "insert into {table_name} values({} , {} , {} , {} ,{} ,{} ,{});",
                    rand::random::<f64>() * 100000f64,
                    rand::random::<f64>() * 100000f64,
                    rand::random::<f64>() * 100000f64,
                    rand::random::<f64>() * 100000f64,
                    rand::random::<f64>() * 100000f64,
                    rand::random::<f64>() * 100000f64,
                    rand::random::<f64>() * 1100000f64,
                );
                db.query_sql(query_id.clone(), &sql).await.unwrap();
            }
        }
    }

    #[tokio::test]
    async fn test_server_metas() {
        let db = make_test_object().await;
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }

    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object().await;
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }
    #[tokio::test]
    async fn test_rename() {
        let db = make_test_object().await;

        let _ = db.drop_table("test_rename", false).await;
        let _ = db.drop_table("test_rename2", false).await;
        let query_id = db.open_connection().await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE IF NOT EXISTS test_rename (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,  
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                );"#,
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
        let db = make_test_object().await;

        let _ = db.drop_table("test_duplicate", false).await;
        let _ = db.drop_table("test_duplicate2", false).await;
        let query_id = db.open_connection().await.unwrap();
        db.query_sql(
            query_id,
            r#"
                CREATE TABLE IF NOT EXISTS test_duplicate (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,  
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                );"#,
        )
        .await
        .unwrap();
        db.duplicate_table("test_duplicate", "test_duplicate2")
            .await
            .unwrap();

        db.show_table("test_duplicate2", None, None, 1, 100, None)
            .await
            .unwrap();
    }
    #[tokio::test]
    async fn test_triggers() {
        let db = make_test_object().await;
        let _ = db.drop_table("test_create_trigger", false).await;
        let _ = db.drop_table("test_create_trigger_update", false).await;
        let mut conn = db.get_conn().await.unwrap();

        sqlx::raw_sql(
            r#"
            CREATE TABLE test_create_trigger (
                id INTEGER PRIMARY KEY AUTOINCREMENT, -- 自动递增的ID作为主键
                info TEXT NOT NULL 
            );
        "#,
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();
        sqlx::raw_sql(
            r#"
        CREATE TABLE test_create_trigger_update (
            id INTEGER PRIMARY KEY AUTOINCREMENT, -- 自动递增的ID作为主键
            info TEXT NOT NULL
        );
    "#,
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        sqlx::raw_sql(
            r#"
        CREATE TRIGGER after_insert_test_create_trigger
        AFTER INSERT ON test_create_trigger
        FOR EACH ROW
        BEGIN
            INSERT INTO test_create_trigger_update (info)
            VALUES (NEW.info);
        END;
    "#,
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();
        let table = "test_create_trigger";

        let triggers = db.list_trigger(table).await.unwrap();
        for trigger in triggers.iter() {
            let details = db
                .show_trigger("test_create_trigger", trigger.name.as_str())
                .await
                .unwrap();
            println!("name:{} , detail:{:?}", trigger.name, details);
        }
    }

    #[tokio::test]
    async fn test_index() {
        let index_name = "name_index";

        let db = make_test_object().await;
        let table_name = "test_index";
        let _ = db.drop_index(table_name, index_name).await;
        let _err = db.drop_table(table_name, false).await;
        if let Err(err) = _err {
            println!("drop table failed,err:{}", err.to_string());
        }
        let mut conn = db.get_conn().await.unwrap();
        sqlx::raw_sql(
            r#"
                CREATE TABLE test_index (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,  -- 主键，自动增长
                    name TEXT NOT NULL,                    -- 名称，非j空
                    age INTEGER,                           -- 年龄
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP  -- 创建时间，默认为当前时间
                ); "#,
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        db.create_index(
            table_name,
            index_name,
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]

    async fn test_export_table() {
        let db = make_test_object().await;
        let table_name = "Persons";
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            -- 创建表
            CREATE TABLE Persons (
                ID INTEGER PRIMARY KEY,
                Name TEXT,
                Age INTEGER
            );
            CREATE INDEX persons_idx_age ON Persons (Age);
            -- 插入测试数据
            INSERT INTO Persons (Name, Age) VALUES ('John', 25);
            INSERT INTO Persons (Name, Age) VALUES ('Jane', 30);
            INSERT INTO Persons (Name, Age) VALUES ('Mike', 35);
            "#,
        )
        .await
        .unwrap()
        .unwrap();

        let db = db;
        {
            let s = db
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
            let s = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", s);
            let _: serde_json::Value = serde_json::from_str(s.as_str()).unwrap();
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
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json_line:{}", s);
        }
        println!("########################################");

        {
            let s = db
                .export_query_result_to_string(
                    "select * from persons",
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
                    "select * from persons",
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
                .export_query_result_to_string("select * from persons", ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json_line:{}", s);
        }
        {
            let filepath = db.backend_name().make_test_xlsx_file(table_name);
            db.export_table_or_view_to_file(
                table_name,
                &filepath,
                ExportKind::Xlsx {
                    include_header: true,
                },
            )
            .await
            .unwrap();
            opener::open(&filepath).unwrap();
        }
    }

    #[tokio::test]
    async fn test_show_documents() {
        let db = make_test_object().await;

        let r = db
            .show_table("documents", None, None, 1, 100, None)
            .await
            .unwrap();

        dbg!(r);
    }

    #[tokio::test]
    async fn test_all_types() {
        enable_test_log();
        let db = make_test_object().await;
        let table_name = "test_table";
        let _ = db.drop_table(table_name, false).await;
        let mut conn = db.get_conn().await.unwrap();
        sqlx::raw_sql(
            r#"
            CREATE table test_table (
                id INTEGER PRIMARY KEY,
                null_col NULL,
                text_col TEXT,
                real_col REAL,
                blob_col BLOB,
                int_col INTEGER,
                numeric_col NUMERIC,
                datetime_col datetime,
                time_col time,
                date_col date
            );
            CREATE INDEX idx ON test_table (real_col , int_col );
        "#,
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert("text_col".to_string(), Some("Some text".to_string()));
        values.insert("id".to_string(), Some("1".to_string()));
        values.insert("real_col".to_string(), Some("3.14".to_string()));
        values.insert(
            "blob_col".to_string(),
            Some("X'0123456789ABCDEF'".to_string()),
        );
        values.insert("int_col".to_string(), Some("42".to_string()));
        values.insert("numeric_col".to_string(), Some("123.45".to_string()));
        values.insert(
            "datetime_col".to_string(),
            Some("2024-01-01 12:00:00".to_string()),
        );
        values.insert("time_col".to_string(), Some("12:00:00".to_string()));
        values.insert("date_col".to_string(), Some("2024-01-01".to_string()));

        db.table_insert(table_name, values.clone()).await.unwrap();
        let mut wheres = HashMap::new();
        wheres.insert("id".to_string(), Some(1.to_string()));

        let r = db
            .table_update(table_name, values.clone(), wheres)
            .await
            .unwrap();
        println!("{:?}", r);

        let cols = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("{:?}", cols);

        let indexes = db.show_indexes(table_name).await.unwrap();
        println!("indexes{:?}", indexes);
        let id = db.open_connection().await.unwrap();
        let _ = db.drop_view("test_view").await;
        {
            let r = db
                .query_sql(
                    id.clone(),
                    r#"
            SELECT * from test_table;
            PRAGMA table_info('test_table')

        "#,
                )
                .await
                .unwrap();

            for (index, x) in r.iter().enumerate() {
                println!("result {} r:{:?}", index, x);
            }
        }
        sqlx::raw_sql(AssertSqlSafe(
            format!("create view test_view as SELECT * from {table_name}").as_str(),
        ))
        .execute(conn.deref_mut())
        .await
        .unwrap();
        println!("\n after here are view test.\n");

        let views = db.list_view().await.unwrap();
        for v in views.iter() {
            let list = db
                .show_view(&v.name.as_str(), None, None, 1, 100, None)
                .await
                .unwrap();
            println!("view value:{:?}", list);
        }
    }
}
