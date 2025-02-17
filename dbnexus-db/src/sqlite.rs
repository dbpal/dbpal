use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use futures_util::lock::Mutex;
use futures_util::{StreamExt, TryStreamExt};
use sqlparser::ast::{ColumnOption, Expr, Statement};
use sqlparser::tokenizer::Token;
use sqlx::pool::{PoolConnection, PoolOptions};
use sqlx::sqlite::SqliteRow;
use sqlx::{Column, Executor, Row, SqliteConnection, TypeInfo};

use crate::MemoryLimit;
use crate::db::{
    BackendName, CheckConn, ColumnType, CreateTableAndIndexSql, ExportKind, Field, Index,
    IndexKind, ListTable, NamedStaff, QueryResp, QuerySql, SchemaTrait, ServerMetas, SqlExecuteRet,
    TableListResp, TriggerDetail, TriggerEvent, TriggerTiming,
};
use crate::helper::{create_csv, is_dangerous_sql_by_parser};
use crate::meta_value::MetaValue;
use crate::query_log::QueryLog;

fn type_support_compare(_typ: &str) -> bool {
    true
}

#[derive(Clone)]
pub struct SqliteDB {
    pool: sqlx::SqlitePool,
    read_only: bool,
    path: String,
    query_connection: Arc<DashMap<uuid::Uuid, Arc<Mutex<PoolConnection<sqlx::Sqlite>>>>>,
}

impl SqliteDB {
    async fn create_table_or_view_sql(
        &self, table_name: &str, kind: &str,
    ) -> anyhow::Result<String> {
        let mut conn = self.get_conn().await?;
        let sql = format!(
            "SELECT sql FROM sqlite_master WHERE type = '{kind}' AND name = '{table_name}';"
        );
        let row = sqlx::query(sql.as_str())
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
        let rows = sqlx::query(sql.as_str())
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
            sender.send("\n".into()).await?;
            for index in ss.indexes.into_iter() {
                sender.send(Cow::Owned(index)).await?;
                sender.send("\n".into()).await?;
            }
        }
        let mut conn = self.get_conn().await?;
        let cols = SchemaTrait::show_table_columns(self, table_name).await?;
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

        self.export_query_result_as_json_line(&sql, sender).await
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

    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        self.export_query_result_as_csv(&sql, include_header, delimiter, sender)
            .await
    }
}
// TODO sqlparse don't support parse trigger statement
// use lexer to solve this issue.
fn extract_trigger_base_on_lexer(sql: String) -> anyhow::Result<TriggerDetail> {
    let t = sqlparser::tokenizer::Tokenizer::new(&sqlparser::dialect::SQLiteDialect {}, &sql)
        .tokenize()?;
    let mut event = None;
    let mut timing = None;
    for token in t {
        match token {
            Token::Word(x) => {
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
            _ => {}
        }
    }
    if event.is_none() || timing.is_none() {
        anyhow::bail!("trigger statement is not valid");
    }
    let event = event.unwrap();
    let timing = timing.unwrap();
    anyhow::Ok(TriggerDetail::new(
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

#[cfg(test)]
#[test]
fn test_extract_trigger_base_on_lexer() {
    {
        let sql = "after insert".to_string();
        assert_eq!(
            extract_trigger_base_on_lexer(sql.clone()).unwrap(),
            TriggerDetail::new(TriggerTiming::After, TriggerEvent::Insert, sql, None)
        );
    }
    {
        let sql = "after update".to_string();
        assert_eq!(
            extract_trigger_base_on_lexer(sql.clone()).unwrap(),
            TriggerDetail::new(TriggerTiming::After, TriggerEvent::Update, sql, None)
        );
    }
    {
        let sql = "after delete".to_string();
        assert_eq!(
            extract_trigger_base_on_lexer(sql.clone()).unwrap(),
            TriggerDetail::new(TriggerTiming::After, TriggerEvent::Delete, sql, None)
        );
    }
    {
        let sql = "before insert".to_string();
        assert_eq!(
            extract_trigger_base_on_lexer(sql.clone()).unwrap(),
            TriggerDetail::new(TriggerTiming::Before, TriggerEvent::Insert, sql, None)
        );
    }
    {
        let sql = "before update".to_string();
        assert_eq!(
            extract_trigger_base_on_lexer(sql.clone()).unwrap(),
            TriggerDetail::new(TriggerTiming::Before, TriggerEvent::Update, sql, None)
        );
    }
    {
        let sql = "before delete".to_string();
        assert_eq!(
            extract_trigger_base_on_lexer(sql.clone()).unwrap(),
            TriggerDetail::new(TriggerTiming::Before, TriggerEvent::Delete, sql, None)
        );
    }

    // error cases
    {
        let sql = "before xxx".to_string();
        assert!(extract_trigger_base_on_lexer(sql.clone()).is_err());
    }
    {
        let sql = "insert xxx".to_string();
        assert!(extract_trigger_base_on_lexer(sql.clone()).is_err());
    }
}

fn extract_trigger_detail_from_sql(sql: String) -> anyhow::Result<TriggerDetail> {
    extract_trigger_base_on_lexer(sql)
}

#[cfg(test)]
#[test]
fn test_extract_trigger_detail_from_sql() {
    {
        let x = extract_trigger_detail_from_sql(
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

async fn query_trigger_detail(
    conn: &mut SqliteConnection, name: &str,
) -> anyhow::Result<TriggerDetail> {
    let s = format!("SELECT sql from sqlite_master WHERE type = 'trigger' and name='{name}'");
    let row = sqlx::query(&s).fetch_one(conn).await?;
    let sql: String = row.get(0);
    extract_trigger_detail_from_sql(sql)
}

#[async_trait::async_trait]
impl CheckConn for SqliteDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        self.list_table().await?;
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
                    metas.push(MetaValue::new("version", version));
                }
                _ => {
                    anyhow::bail!("count the row number");
                }
            }
        }

        anyhow::Ok(metas)
    }
}

impl std::fmt::Debug for SqliteDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "sqlite on:{:?}", self.path)
    }
}

impl BackendName for SqliteDB {
    fn backend_name(&self) -> &str {
        "sqlite"
    }
}

#[async_trait::async_trait]
impl SchemaTrait for SqliteDB {
    fn quote_style(&self) -> &'static str {
        "\""
    }

    async fn create_index(
        &self, table: &str, name: &str, kind: IndexKind, fileds: &[String],
    ) -> anyhow::Result<()> {
        let sql = format!(
            "CREATE {} INDEX {} ON {table}({})",
            match kind {
                IndexKind::Index => "",
                IndexKind::Primary => anyhow::bail!("can't create primary index"),
                IndexKind::Unique => "Unique",
            },
            name,
            fileds.join(",")
        );
        let mut conn = self.get_conn().await?;
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP INDEX "{index}";"#);
        let mut conn = self.get_conn().await?;
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
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
                    self.put_value_in_sql(&col.typ, Some(d.as_str()))?
                )
            } else {
                "".to_owned()
            }
        );
        let mut conn = self.get_conn().await?;
        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        use sqlx::{Executor, Statement};
        let mut conn = self.get_conn().await?;
        let sql = format!(r#"select * from "{view_name}" limit 1"#);
        let s = conn.deref_mut().prepare_with(&sql, &[]).await?;
        let ret = s
            .columns()
            .iter()
            .map(|x| Field::from_name_type(x.name(), x.type_info().to_string()))
            .collect::<Vec<_>>();
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

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        if self.table_exists(dup).await? {
            anyhow::bail!(format!("table '{}' already exists", dup));
        }
        let sql = format!(r#"CREATE TABLE "{dup}" AS SELECT * FROM "{table}""#);
        let mut conn = self.get_conn().await?;
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;

        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let sql = format!(r#"ALTER TABLE "{old}" RENAME TO "{new}""#);
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
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

    async fn show_trigger(&self, _table: &str, name: &str) -> anyhow::Result<TriggerDetail> {
        let mut conn = self.get_conn().await?;
        query_trigger_detail(conn.deref_mut(), &name).await
    }

    async fn drop_trigger(&self, _table: &str, name: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
         DROP TRIGGER "{name}"
        "#
        );
        let mut conn = self.get_conn().await?;
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    fn read_only(&self) -> bool {
        self.read_only
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

    async fn list_view(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(
            r#"
            SELECT name
            FROM sqlite_master
            WHERE type = 'view';
        "#,
        )
        .fetch_all(conn.deref_mut())
        .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            ret.push(name);
        }

        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.can_write()?;

        let s = format!(r#"DROP VIEW "{view_name}""#);
        let mut conn = self.get_conn().await?;
        sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let s = format!(
            r#"DROP TABLE "{table}" {}"#,
            if cascade { " CASCADE " } else { "" }
        );
        let mut conn = self.get_conn().await?;
        sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_table_columns(table_name).await?;
        let sql = (self as &dyn crate::db::SchemaTrait).delete_sql(
            None,
            table_name,
            &fields,
            &wheres,
            type_support_compare,
        )?;
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
            DELETE FROM "{table_name}";
            DELETE FROM sqlite_sequence WHERE name = '{table_name}';
        "#
        );
        let mut conn = self.get_conn().await?;
        sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    fn put_value_in_sql(&self, typ: &str, value: Option<&str>) -> anyhow::Result<String> {
        let typ = typ.to_uppercase();

        let v = match typ.as_str() {
            "" | "TEXT" | "DATE" | "TIME" | "DATETIME" | "NULL" => {
                format!(
                    "'{}'",
                    value
                        .map(|x| x.replace("'", "''"))
                        .unwrap_or("null".to_string())
                )
            }
            _ if text_like(typ.as_str()) => {
                format!(
                    "'{}'",
                    value
                        .map(|x| x.replace("'", "''"))
                        .unwrap_or("null".to_string())
                )
            }
            "REAL" | "INTEGER" | "NUMERIC" | "BOOLEAN" => value.unwrap_or("null").to_string(),
            _ if integer_like(typ.as_str()) || numreic_like(typ.as_str()) => {
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

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_table_columns(table_name).await?;

        let mut conn = self.get_conn().await?;
        let s = (self as &dyn crate::db::SchemaTrait).update_sql(
            None,
            table_name,
            &fields,
            &values,
            &wheres,
            type_support_compare,
        )?;

        let r = sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql: s,
            changes: r.rows_affected(),
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_table_columns(table_name).await?;
        let sql =
            (self as &dyn crate::db::SchemaTrait).insert_sql(None, table_name, &fields, &values)?;
        log::trace!("insert sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let r = sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            last_insert_row_id: Some(r.last_insert_rowid()),
            changes: r.rows_affected(),
            last_insert_id_string: None,
        })
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name not in ('sqlite_sequence' , 'sqlite_stat1' , 'sqlite_stat4') ORDER BY name ")
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let table_name: String = r.get(0);

            ret.push(table_name);
        }
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let increment_key = self.query_auto_increment_key(table_name).await?;
        let increment_key = increment_key.as_ref().map(|x| x.as_str());

        let s = format!(r#"pragma table_info("{table_name}")"#);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(s.as_str()).fetch_all(conn.deref_mut()).await?;
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
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let total = self.query_count(table_name, where_str.clone()).await?;
        let sql = gen_show_sql(table_name, where_str, order_by, page, page_size);
        log::trace!("show table sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let row = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await
            .map_err(|e| anyhow::Error::msg(format!("err:'{}' sql:'{}'", e, sql)))?;

        let data = encode_rows(&row)?;
        let fields = self.show_table_columns(table_name).await?;
        anyhow::Ok(TableListResp::new(fields, total, data, sql))
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let mut conn = self.get_conn().await?;
        let total = self.query_count(view_name, where_str.clone()).await?;
        let sql = gen_show_sql(view_name, where_str, order_by, page, page_size);
        log::trace!("show table sql:{}", sql);
        let rows = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await?;
        if rows.len() == 0 {
            let f = self.show_view_columns(view_name).await?;
            return anyhow::Ok(TableListResp::new(f, 0, vec![], sql));
        }
        let data = encode_rows(&rows)?;
        let fields = get_rows_types(&rows);

        anyhow::Ok(TableListResp::new(fields, total, data, sql))
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let sql = format!("PRAGMA index_list({table_name})");
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(sql.as_str())
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

            let rows = sqlx::query(sql.as_str())
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
        let row = sqlx::query(s.as_str()).fetch_one(conn.deref_mut()).await?;
        let sql: String = row.get(0);
        extract_auto_increment_key(sql.as_str())
    }
}

fn get_rows_types(rows: &Vec<SqliteRow>) -> Vec<Field> {
    if rows.len() == 0 {
        return vec![];
    }
    let mut ret = Vec::with_capacity(rows[0].columns().len());
    for c in rows[0].columns().iter() {
        ret.push(Field {
            field: c.name().to_string(),
            typ: c.type_info().name().to_string(),
            nullable: true,
            default_value: None,
            is_primary: false,
            is_auto_increment: false,
        });
    }
    ret
}

fn encode_bytes(bs: &[u8]) -> String {
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
                v.map(|x| serde_json::Value::String(x))
                    .unwrap_or(null_value.clone())
            }
            _ if real_like(&typ) => {
                let v: Option<f64> = r.try_get(c)?;
                v.map(|x| serde_json::Value::from(x))
                    .unwrap_or(null_value.clone())
            }
            _ if integer_like(&typ) => {
                let v: Option<i64> = r.try_get(c)?;
                v.map(|x| serde_json::Value::from(x))
                    .unwrap_or(null_value.clone())
            }
            "BLOB" => {
                let v: Option<Vec<u8>> = r.try_get(c)?;
                v.map(|x| format!("X'{}'", encode_bytes(x.as_slice())))
                    .map(|x| serde_json::Value::from(x))
                    .unwrap_or(null_value.clone())
            }

            _ if numreic_like(&typ) => {
                let v: Option<f64> = r.try_get(c)?;
                v.map(|x| serde_json::Value::from(x))
                    .unwrap_or(null_value.clone())
            }
            // non-standard extensions 这些实际上不存在？
            "BOOLEAN" => {
                let v: Option<bool> = r.try_get(c)?;
                v.map(|x| serde_json::Value::from(x))
                    .unwrap_or(null_value.clone())
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

fn integer_like(typ: &str) -> bool {
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

fn numreic_like(typ: &str) -> bool {
    return typ == "NUMERIC" || typ.starts_with("DECIMAL");
}

fn text_like(typ: &str) -> bool {
    return typ.starts_with("CHARACTER")
        || typ.starts_with("VARCHAR")
        || typ.starts_with("VARYING CHARACTER")
        || typ.starts_with("NCHAR")
        || typ.starts_with("NATIVE CHARACTER")
        || typ.starts_with("NVARCHAR")
        || typ == "TEXT"
        || typ == "CLOB";
}

fn real_like(typ: &str) -> bool {
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
                    let v: Option<i64> = r.try_get(c)?;
                    v.map(|x| format!("{}", x))
                }
                _ if numreic_like(ty) => {
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
    pub async fn new(path: String, read_only: bool) -> anyhow::Result<Self> {
        let mut opts: PoolOptions<sqlx::sqlite::Sqlite> = PoolOptions::new();
        opts = opts.max_connections(100);
        let pool = opts.connect(path.as_str()).await?;

        if read_only {
            let r = sqlx::query("PRAGMA query_only = true;")
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
            path,
        })
    }

    async fn query_count(&self, name: &str, where_str: Option<&str>) -> anyhow::Result<usize> {
        let mut conn = self.get_conn().await?;
        let mut s = format!(r#"SELECT count(1) from "{name}" "#);
        if let Some(where_str) = where_str {
            s.push_str(format!(" where {where_str}").as_str());
        }
        let row = sqlx::query(s.as_str())
            .fetch_one(conn.deref_mut())
            .await
            .map_err(|e| anyhow::Error::msg(format!("err:'{}' sql:'{}'", e, s)))?;
        let count: i64 = row.get(0);
        anyhow::Ok(count as usize)
    }

    pub async fn get_conn(&self) -> anyhow::Result<PoolConnection<sqlx::Sqlite>> {
        let conn = self.pool.acquire().await?;
        return anyhow::Ok(conn);
    }
}

fn extract_auto_increment_key(sql: &str) -> anyhow::Result<Option<String>> {
    use sqlparser::dialect::SQLiteDialect;
    let ss = sqlparser::parser::Parser::parse_sql(&SQLiteDialect {}, sql)?;
    if ss.len() != 1 {
        anyhow::bail!("should contains one statement");
    }
    let s = ss.get(0).unwrap();
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
        _ => anyhow::bail!("should be a create table statement"),
    };
    anyhow::Ok(None)
}

#[async_trait::async_trait]
impl QuerySql for SqliteDB {
    async fn query_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let mut query_log = QueryLog::get_guard(query_id, sql);

        let conn = self.query_connection.get(&query_id);
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("id '{}' have no connection", query_id),
        };

        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();

        let mut ret = Vec::new();
        let mut stream = sqlx::raw_sql(sql).fetch_many(conn.deref_mut());

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
        drop(stream);
        query_log.set_success(ret.iter().all(|x| x.is_ok()));

        if ret.len() == 0 {
            use sqlx::Statement;
            let s = conn.deref_mut().prepare_with(&sql, &[]).await?;
            let field = s
                .columns()
                .iter()
                .map(|x| Field::from_name_type(x.name(), x.type_info().to_string()))
                .collect::<Vec<_>>();
            ret = vec![Ok(QueryResp::new(field, vec![]))];
        }

        anyhow::Ok(ret)
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        is_dangerous_sql_by_parser(sql, &sqlparser::dialect::SQLiteDialect {})
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
        QueryLog::remove(&id);

        if let Some((_, _x)) = self.query_connection.remove(&id) {}
        anyhow::Ok(())
    }
}

#[cfg(test)]
#[tokio::test]
async fn test_auto_completion() {
    use crate::language_server::LexerCompletionTrait;
    let s = SqliteDB::new(":memory:".to_string(), false).await.unwrap();

    let mut conn = s.get_conn().await.unwrap();
    sqlx::query(
        r#"

    CREATE TABLE users (
        id INTEGER PRIMARY KEY,
        username TEXT NOT NULL,
        password TEXT NOT NULL,
        email TEXT NOT NULL
    );
    "#,
    )
    .execute(conn.deref_mut())
    .await
    .unwrap();

    {
        let completion = LexerCompletionTrait::do_auto_completion(
            &s,
            "sel  * from",
            &sqlparser::dialect::SQLiteDialect {},
            sqlparser::tokenizer::Location { line: 1, column: 4 },
        )
        .await
        .unwrap();

        println!("{:?}", completion)
    }
    {
        let completion = LexerCompletionTrait::do_auto_completion(
            &s,
            "cou  xxxx",
            &sqlparser::dialect::SQLiteDialect {},
            sqlparser::tokenizer::Location { line: 1, column: 4 },
        )
        .await
        .unwrap();
        println!("{:?}", completion)
    }
    {
        let completion = LexerCompletionTrait::do_auto_completion(
            &s,
            "nul",
            &sqlparser::dialect::SQLiteDialect {},
            sqlparser::tokenizer::Location { line: 1, column: 4 },
        )
        .await
        .unwrap();
        println!("{:?}", completion)
    }
    {
        let completion = LexerCompletionTrait::do_auto_completion(
            &s,
            "emai  dddd",
            &sqlparser::dialect::SQLiteDialect {},
            sqlparser::tokenizer::Location { line: 1, column: 4 },
        )
        .await
        .unwrap();
        println!("{:?}", completion)
    }
    {
        let completion = LexerCompletionTrait::do_auto_completion(
            &s,
            "use 1111",
            &sqlparser::dialect::SQLiteDialect {},
            sqlparser::tokenizer::Location { line: 1, column: 4 },
        )
        .await
        .unwrap();
        println!("{:?}", completion)
    }
}

#[cfg(test)]
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

#[allow(unused)]
fn extract_index(sql: &str) -> anyhow::Result<Option<(Option<String>, Vec<String>)>> {
    use sqlparser::dialect::SQLiteDialect;
    let ss = sqlparser::parser::Parser::parse_sql(&SQLiteDialect {}, sql)?;
    if ss.len() != 1 {
        log::error!("statements are:{:?}", ss);
        anyhow::bail!("should contains one statement.");
    }
    let s = ss.get(0).unwrap();

    match s {
        Statement::CreateIndex(create_index) => {
            let name = create_index.name.clone().map(|x| x.to_string());
            let mut on = Vec::with_capacity(create_index.columns.len());
            for c in create_index.columns.iter() {
                match &c.expr {
                    Expr::Identifier(x) => {
                        on.push(x.to_string());
                    }
                    _ => {
                        anyhow::bail!(format!("not a valid index expr:{:?}", &c.expr));
                    }
                }
            }
            anyhow::Ok(Some((name, on)))
        }

        _ => anyhow::Ok(None),
    }
}

#[cfg(test)]
#[test]
fn test_extract_index() {
    let extract_index =
        extract_index("CREATE INDEX idx_employees_age ON employees (name , age )").unwrap();
    assert_eq!(
        format!("{:?}", extract_index),
        "Some((Some(\"idx_employees_age\"), [\"name\", \"age\"]))"
    );
}

fn gen_show_sql(
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
mod tests {
    use super::*;
    use crate::enable_test_log;

    // this test no need clean up, we use memory sqlite db.
    async fn make_test_object() -> SqliteDB {
        let path = std::env::var("NEXUS_TEST_SQLITE")
            .unwrap_or("/Users/yuyang/tests/test.sqlite.db".to_string());
        println!("db path:{:?}", path);
        let db = SqliteDB::new(path, false).await.unwrap();
        db
    }
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
    #[ignore = "this test run by hand"]
    #[tokio::test]
    async fn test_do_has_data() {
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        for i in 0..10 {
            let tablename = format!("has_data_{}", i);
            let err = db.drop_table(&tablename, false).await;
            if let Err(err) = err {
                println!("drop table error:{:?}", err);
            }
            let create_table = format!(
                r#"create table {tablename} (
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
                    "insert into {tablename} values({} , {} , {} , {} ,{} ,{} ,{});",
                    rand::random::<f64>() * 100000f64,
                    rand::random::<f64>() * 100000f64,
                    rand::random::<f64>() * 100000f64,
                    rand::random::<f64>() * 100000f64,
                    rand::random::<f64>() * 100000f64,
                    rand::random::<f64>() * 100000f64,
                    rand::random::<f64>() * 1100000f64,
                );
                let err = db.query_sql(query_id.clone(), &sql).await.unwrap();
                println!("insert table result:{:?} sql:{}", err, sql);
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
        let db_trait = &db as &dyn SchemaTrait;
        let _ = db_trait.drop_table("test_rename", false).await;
        let _ = db_trait.drop_table("test_rename2", false).await;
        let query_id = db_trait.open_connection().await.unwrap();
        db_trait
            .query_sql(
                query_id,
                r#"
                CREATE TABLE IF NOT EXISTS test_rename (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,  
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                );"#,
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
        let db = make_test_object().await;
        let db_trait = &db as &dyn SchemaTrait;
        let _ = db_trait.drop_table("test_duplicate", false).await;
        let _ = db_trait.drop_table("test_duplicate2", false).await;
        let query_id = db_trait.open_connection().await.unwrap();
        db_trait
            .query_sql(
                query_id,
                r#"
                CREATE TABLE IF NOT EXISTS test_duplicate (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,  
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                );"#,
            )
            .await
            .unwrap();
        db_trait
            .rename_table("test_duplicate", "test_duplicate2")
            .await
            .unwrap();

        db_trait
            .show_table("test_duplicate2", None, None, 1, 100)
            .await
            .unwrap();
    }
    #[tokio::test]

    async fn test_triggers() {
        let db = make_test_object().await;
        let _ = db.drop_table("test_create_trigger", false).await;
        let _ = db.drop_table("test_create_trigger_update", false).await;
        let mut conn = db.get_conn().await.unwrap();
        let db_trait = &db as &dyn SchemaTrait;
        sqlx::query(
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
        sqlx::query(
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

        sqlx::query(
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

        let triggers = db_trait.list_trigger(table).await.unwrap();
        for trigger in triggers.iter() {
            let details = db_trait
                .show_trigger("test_create_trigger", trigger.name.as_str())
                .await
                .unwrap();
            println!("name:{} , detail:{:?}", trigger.name, details);
        }
    }
    #[tokio::test]
    async fn test_export_table() {
        let db = make_test_object().await;

        let mut conn = db.get_conn().await.unwrap();
        let table_name = "Persons";
        let _ = db.drop_table(table_name, false).await;
        sqlx::query(
            r#"
        -- 创建表
        CREATE TABLE Persons (
            ID INTEGER PRIMARY KEY,
            Name TEXT,
            Age INTEGER
        );
        -- 插入测试数据
        INSERT INTO Persons (Name, Age) VALUES ('John', 25);
        INSERT INTO Persons (Name, Age) VALUES ('Jane', 30);
        INSERT INTO Persons (Name, Age) VALUES ('Mike', 35);
        "#,
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();
        let db = &db as &dyn SchemaTrait;
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
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json_line:{}", s);
        }
        println!("########################################");

        {
            let s = db
                .export_query_result_to_string("select * from persons", ExportKind::Json {
                    pretty: true,
                })
                .await
                .unwrap();
            println!("json:{}", s);
            let _: serde_json::Value = serde_json::from_str(s.as_str()).unwrap();
        }
        {
            let s = db
                .export_query_result_to_string("select * from persons", ExportKind::CSV {
                    include_header: true,
                    delimiter: ',',
                })
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
    }
    #[tokio::test]
    async fn test_querylog() {
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        db.query_sql(query_id, "select 1").await.unwrap();
        let event = db.get_query_log(query_id).await.unwrap();
        println!("{:?}", event);
        assert_eq!(event.len(), 1);
    }

    #[tokio::test]
    async fn test_all_types() {
        enable_test_log();
        let db = make_test_object().await;
        let table_name = "test_table";
        let _ = db.drop_table(table_name, false).await;
        let mut conn = db.get_conn().await.unwrap();
        sqlx::query(
            r#"
            CREATE table test_table (
                id INTEGER PRIMARY KEY autoincrement,
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

        let db = &db as &dyn crate::db::SchemaTrait;
        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert("text_col".to_string(), Some("Some text".to_string()));
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

        let r = db.insert(table_name, values.clone()).await.unwrap();
        let mut wheres = HashMap::new();
        wheres.insert(
            "id".to_string(),
            Some(r.last_insert_row_id.unwrap().to_string()),
        );

        let r = db.update(table_name, values.clone(), wheres).await.unwrap();
        println!("{:?}", r);

        let cols = db.show_table(table_name, None, None, 1, 100).await.unwrap();
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
        sqlx::query(format!("create view test_view as SELECT * from {table_name}").as_str())
            .execute(conn.deref_mut())
            .await
            .unwrap();
        println!("\nafetr here are view test.\n");

        let views = db.list_view().await.unwrap();
        for v in views.iter() {
            let list = db
                .show_view(&v.name.as_str(), None, None, 1, 100)
                .await
                .unwrap();
            println!("view value:{:?}", list);
        }
    }
}
