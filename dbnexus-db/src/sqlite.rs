use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use futures_util::{StreamExt, TryStreamExt};
use sqlparser::ast::{ColumnOption, Expr, Statement};
use sqlparser::tokenizer::Token;
use sqlx::pool::PoolConnection;
use sqlx::sqlite::SqliteRow;
use sqlx::{Column, Row, SqliteConnection, TypeInfo};
use tokio::sync::Mutex;

use crate::completion::{
    AutoCompletionTrait, CompletionItem, CompletionKind, DBLexerCompletionBackend,
    DBListCompletionItem,
};
use crate::datasource::DBPoolKey;
use crate::db::{
    create_csv, sqlformat_format_sql, CheckConn, CreateTableAndIndexSql, DBTrait, ExportKind,
    Field, FormatSql, Index, IndexKind, ListTable, ListTrigger, ListView, QueryResp, QuerySql,
    SqlExecuteRet, TableListResp, TriggerDetail, TriggerEvent, TriggerTiming,
};
use crate::pool::{NexusPool, PoolManager};
use crate::CANNOT_GET_CONN_ERR;
impl FormatSql for SqliteDB {
    fn format_sql(&self, sql: &str) -> anyhow::Result<String> {
        anyhow::Ok(sqlformat_format_sql(sql, None))
    }
}
#[derive(Clone)]
pub struct SqliteDB {
    pool: PoolManager,
    path: String,
    read_only: bool,
    query_connection: Arc<Mutex<HashMap<uuid::Uuid, PoolConnection<sqlx::Sqlite>>>>,
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
            .await
            .context("can't query create sql")?;
        let sql: String = row.try_get(0).context("can't get sql def")?;
        anyhow::Ok(sql)
    }

    async fn get_table_indexes_sql(&self, table_name: &str) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let sql = format!(
            "SELECT sql FROM sqlite_master WHERE type = 'index' AND tbl_name = '{table_name}';"
        );
        let rows = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await
            .context("can't query sql")?;

        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let sql: String = r.try_get(0).context("can't get sql def")?;
            ret.push(sql);
        }
        anyhow::Ok(ret)
    }

    async fn export_table_as_sql(
        &self, table_name: &str, include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            let ss = DBTrait::create_table_and_index_sql(self, table_name).await?;
            sender.send(Cow::Owned(ss.create_table.clone())).await?;
            sender.send(Cow::Borrowed("\n")).await?;

            for index in ss.indexes.into_iter() {
                sender.send(Cow::Owned(index)).await?;
                sender.send(Cow::Borrowed("\n")).await?;
            }
        }
        let mut conn = self.get_conn().await?;
        let cols = DBTrait::show_columns(self, table_name).await?;
        let sql = format!("SELECT * from {}", table_name);
        let mut stream = sqlx::query(sql.as_str())
            .fetch(conn.deref_mut())
            .try_chunks(100);
        while let Some(rows) = stream.next().await {
            let rows = rows.context("stream returns error")?;

            let data = encode_rows(&rows).context("encode rows error")?;
            for d in data.into_iter() {
                let mut m = HashMap::new();
                for (d, f) in d.into_iter().zip(cols.iter()) {
                    m.insert(f.name.clone(), d);
                }

                let insert_sql = (self as &dyn DBTrait).insert_sql(table_name, &cols, &m)?;
                sender.send(Cow::Owned(insert_sql)).await?;
                sender.send(Cow::Borrowed("\n")).await?;
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
            let row = row.context("stream returns error")?;
            let value = encode_row_as_json(&row)?;

            sender
                .send(Cow::Owned(serde_json::to_string(&value)?))
                .await?;

            sender.send(Cow::Borrowed("\n")).await?;
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

        sender.send(Cow::Borrowed("[")).await?;
        let mut first = true;
        while let Some(row) = stream.next().await {
            let row = row.context("stream returns error")?;
            let value = encode_row_as_json(&row)?;
            if !first {
                sender.send(Cow::Borrowed(",")).await?;
            }
            if pretty {
                sender.send(Cow::Borrowed("\n")).await?;
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
            sender.send(Cow::Borrowed("\n")).await?;
        }
        sender.send(Cow::Borrowed("]")).await?;
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
            let rows = rows.context("stream return error")?;
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
        let sql = format!("SELECT * from {}", table_name);
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
    // let ss = sqlparser::parser::Parser::parse_sql(&
    // sqlparser::dialect::SQLiteDialect {}, sql)     .map_err(|e| ->
    // anyhow::Error { e.into() })     .map_err(|e| e.context("extract trigger
    // detail fail to parse sql"))?;

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
    let row = sqlx::query(&s)
        .fetch_one(conn)
        .await
        .context("can't query trigger sql")?;
    let sql: String = row.get(0);
    extract_trigger_detail_from_sql(sql)
}

#[async_trait::async_trait]
impl CheckConn for SqliteDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        let mut conn = self.get_conn().await?;
        sqlx::query("select 1").fetch_all(conn.deref_mut()).await?;

        anyhow::Ok(ins.elapsed())
    }
}

#[async_trait::async_trait]
impl DBTrait for SqliteDB {
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
        let sql = format!("CREATE TABLE {dup} AS SELECT * FROM {table}");
        let mut conn = self.get_conn().await.context("can't dup table")?;
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;

        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let sql = format!("ALTER TABLE {old} RENAME TO {new}");
        sqlx::query(sql.as_str())
            .execute(conn.deref_mut())
            .await
            .context("can't rename table")?;
        anyhow::Ok(())
    }

    async fn list_trigger(&self, table_name: &str) -> anyhow::Result<Vec<ListTrigger>> {
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
            .await
            .context("can query list triiger")?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            ret.push(name);
        }

        anyhow::Ok(ret.into_iter().map(ListTrigger::from).collect())
    }

    async fn show_trigger(&self, _table: &str, name: &str) -> anyhow::Result<TriggerDetail> {
        let mut conn = self.get_conn().await?;
        query_trigger_detail(conn.deref_mut(), &name).await
    }

    async fn drop_trigger(&self, _table: &str, name: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
         DROP TRIGGER {name}
        "#
        );
        let mut conn = self.get_conn().await?;

        sqlx::query(sql.as_str())
            .execute(conn.deref_mut())
            .await
            .context("can't execute sql")?;

        anyhow::Ok(())
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    fn backend_name(&self) -> &str {
        "sqlite"
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

    async fn create_view_sql(&self, view_name: &str) -> anyhow::Result<String> {
        self.create_table_or_view_sql(view_name, "view").await
    }

    fn clone(&self) -> Box<dyn DBTrait> {
        Box::new(Clone::clone(self))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListView>> {
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(
            r#"
            SELECT name
            FROM sqlite_master
            WHERE type = 'view';
        "#,
        )
        .fetch_all(conn.deref_mut())
        .await
        .context("can't query list view")?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            ret.push(name);
        }

        anyhow::Ok(ret.into_iter().map(ListView::from).collect())
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.can_write()?;

        let s = format!("DROP VIEW {view_name}");
        let mut conn = self.get_conn().await?;
        sqlx::query(s.as_str())
            .execute(conn.deref_mut())
            .await
            .context("can't drop view")?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let s = format!("DROP TABLE {table}");
        let mut conn = self.get_conn().await?;
        sqlx::query(s.as_str())
            .execute(conn.deref_mut())
            .await
            .context("can't drop table")?;
        anyhow::Ok(())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_columns(table_name).await?;
        let sql = (self as &dyn crate::db::DBTrait).delete_sql(table_name, &fields, &wheres)?;
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
            DELETE FROM {table_name};
            DELETE FROM sqlite_sequence WHERE name = '{table_name}';
        "#
        );
        let mut conn = self.get_conn().await?;
        sqlx::query(s.as_str())
            .execute(conn.deref_mut())
            .await
            .context("can't truncate table")?;
        anyhow::Ok(())
    }

    fn put_value_in_sql(&self, f: &Field, value: Option<&str>) -> anyhow::Result<String> {
        let v = if f.typ == "TEXT" {
            // escape '
            format!(
                "'{}'",
                value
                    .map(|x| x.replace("'", "''"))
                    .unwrap_or("null".to_string())
            )
        } else {
            let mut x = value.unwrap_or("null").to_string();
            if x.is_empty() {
                // todo null return "" string sometimes??
                x = "null".to_string();
            }
            x
        };
        anyhow::Ok(v)
    }

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_columns(table_name).await?;

        let mut conn = self.get_conn().await?;
        let s =
            (self as &dyn crate::db::DBTrait).update_sql(table_name, &fields, &values, &wheres)?;

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
        let fields = self.show_columns(table_name).await?;
        let sql = (self as &dyn crate::db::DBTrait).insert_sql(table_name, &fields, &values)?;
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
        let rows = sqlx::query("SELECT name FROM sqlite_master WHERE type='table';")
            .fetch_all(conn.deref_mut())
            .await
            .context("can't query sql")?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let table_name: String = r.get(0);
            ret.push(table_name);
        }
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    async fn show_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let increment_key = self.query_auto_increment_key(table_name).await?;
        let increment_key = increment_key.as_ref().map(|x| x.as_str());

        let s = format!("pragma table_info({table_name})");
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(s.as_str())
            .fetch_all(conn.deref_mut())
            .await
            .context("can't query sql")?;
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
            .await?;

        let data = encode_rows(&row)?;
        let fields = self.show_columns(table_name).await?;
        anyhow::Ok(TableListResp::new(fields, total, data, sql))
    }

    async fn show_view(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let mut conn = self.get_conn().await?;
        let total = self.query_count(table_name, where_str.clone()).await?;
        let sql = gen_show_sql(table_name, where_str, order_by, page, page_size);
        log::trace!("show table sql:{}", sql);
        let row = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await?;
        let data = encode_rows(&row)?;
        let fields = get_rows_types(&row);

        anyhow::Ok(TableListResp::new(fields, total, data, sql))
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let s = format!("PRAGMA index_list({table_name})");
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(s.as_str()).fetch_all(conn.deref_mut()).await?;
        // seq name unique origin partial
        // 0   xxx  1      c      0

        let mut ret = Vec::with_capacity(rows.len());

        for r in rows.iter() {
            let name: String = r.get(1);
            let unique: i64 = r.get(2);
            // query create index sql
            let s = format!(
                "SELECT sql from sqlite_master where type='index' and name='{name}' and tbl_name='{table_name}'; "
            );
            let row = sqlx::query(s.as_str()).fetch_one(conn.deref_mut()).await?;
            let create_index_sql: String = row.get(0);

            let indexs = extract_index(&create_index_sql)?;
            if indexs.is_none() {
                anyhow::bail!(format!(
                    "some we can't extract indexes this statement {}",
                    create_index_sql
                ));
            }
            let (name2, on) = indexs.unwrap();
            if let Some(name2) = &name2 {
                debug_assert_eq!(&name, name2);
            }

            ret.push(Index::new(
                name,
                if unique == 1 {
                    IndexKind::Unique
                } else {
                    IndexKind::Normal
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
            name: c.name().to_string(),
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
        let ty = r.column(c).type_info().name();
        let v: serde_json::Value = match ty {
            "NULL" | "" => {
                // null table is not always null. I quote 'suitable for
                // representing missing or unknown data'
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

            "TEXT" => {
                let v: Option<String> = r.get(c);
                v.map(|x| serde_json::Value::String(x))
                    .unwrap_or(null_value.clone())
            }
            "REAL" => {
                let v: Option<f64> = r.get(c);
                v.map(|x| serde_json::Value::from(x))
                    .unwrap_or(null_value.clone())
            }
            "BLOB" => {
                let v: Option<Vec<u8>> = r.get(c);
                v.map(|x| format!("X'{}'", encode_bytes(x.as_slice())))
                    .map(|x| serde_json::Value::from(x))
                    .unwrap_or(null_value.clone())
            }

            "INTEGER" => {
                let v: Option<i64> = r.get(c);
                v.map(|x| serde_json::Value::from(x))
                    .unwrap_or(null_value.clone())
            }
            "NUMERIC" => {
                let v: Option<f64> = r.get(c);
                v.map(|x| serde_json::Value::from(x))
                    .unwrap_or(null_value.clone())
            }

            // non-standard extensions 这些实际上不存在？
            // "BOOLEAN" => {
            //
            // }
            // "DATE" => {
            //
            // }
            // "TIME" => {
            //
            // }
            // "DATETIME" => {
            //
            // }
            _ => anyhow::bail!(format!("type '{ty}' not supported")),
        };

        values.insert(r.column(c).name().to_owned(), v);
    }
    anyhow::Ok(serde_json::Value::Object(values))
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
                    } else if let Result::Ok(s) = r.try_get::<'_, Vec<u8>, _>(c) {
                        Some(encode_bytes(s.as_slice()))
                    } else if let Result::Ok(s) = r.try_get::<'_, i64, _>(c) {
                        Some(s.to_string())
                    } else if let Result::Ok(s) = r.try_get(c) {
                        Some(s)
                    } else {
                        None
                    }
                }

                "TEXT" => {
                    let v: Option<String> = r.get(c);
                    v
                }
                "REAL" => {
                    let v: Option<f64> = r.get(c);
                    v.map(|x| format!("{}", x))
                }
                "BLOB" => {
                    let v: Option<Vec<u8>> = r.get(c);
                    v.map(|x| encode_bytes(x.as_slice()))
                }

                "INTEGER" => {
                    let v: Option<i64> = r.get(c);
                    v.map(|x| format!("{}", x))
                }
                "NUMERIC" => {
                    let v: Option<f64> = r.get(c);
                    v.map(|x| format!("{}", x))
                }

                // non-standard extensions 这些实际上不存在？
                // "BOOLEAN" => {
                //
                // }
                // "DATE" => {
                //
                // }
                // "TIME" => {
                //
                // }
                // "DATETIME" => {
                //
                // }
                _ => anyhow::bail!(format!("type '{ty}' not supported")),
            };

            values.push(v);
        }
        ret.push(values);
    }
    anyhow::Ok(ret)
}

impl SqliteDB {
    pub fn new(pool: PoolManager, path: String, read_only: bool) -> Self {
        Self {
            path,
            pool,
            read_only,
            query_connection: Default::default(),
        }
    }

    async fn query_count(&self, name: &str, where_str: Option<&str>) -> anyhow::Result<usize> {
        let mut conn = self.get_conn().await?;
        let mut s = format!("SELECT count(1) from {name} ");
        if let Some(where_str) = where_str {
            s.push_str(format!(" where {where_str}").as_str());
        }
        let row = sqlx::query(s.as_str()).fetch_one(conn.deref_mut()).await?;
        let count: i64 = row.get(0);
        anyhow::Ok(count as usize)
    }

    pub async fn get_conn(&self) -> anyhow::Result<PoolConnection<sqlx::Sqlite>> {
        let pool = match self
            .pool
            .get(&DBPoolKey::Sqlite {
                path: self.path.clone(),
            })
            .await?
        {
            NexusPool::Sqlite(p) => p.clone(),
            _ => unreachable!(),
        };

        let conn = pool.acquire().await.context(CANNOT_GET_CONN_ERR)?;
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
        Statement::CreateTable {
            or_replace: _,
            temporary: _,
            external: _,
            global: _,
            if_not_exists: _,
            transient: _,
            name: _,
            columns,
            constraints: _,
            hive_distribution: _,
            hive_formats: _,
            table_properties: _,
            with_options: _,
            file_format: _,
            location: _,
            query: _,
            without_rowid: _,
            like: _,
            clone: _,
            engine: _,
            comment: _,
            auto_increment_offset: _,
            default_charset: _,
            collation: _,
            on_commit: _,
            on_cluster: _,
            order_by: _,
            partition_by: _,
            cluster_by: _,
            options: _,
            strict: _,
        } => {
            for c in columns.iter() {
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
        &self, id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let mut conn = self.query_connection.lock().await;
        let conn = conn.get_mut(&id);
        let conn = match conn {
            Some(x) => x,
            None => anyhow::bail!("id '{}' have no connection", id),
        };
        let mut ret = Vec::new();
        let mut stream = sqlx::raw_sql(sql).fetch_many(conn.deref_mut());

        let mut rows = Vec::new();
        while let Some(item) = stream.next().await {
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
        anyhow::Ok(ret)
    }

    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        let conn = self.get_conn().await?;
        self.query_connection.lock().await.insert(id, conn);
        anyhow::Ok(id)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        if let Some(x) = self.query_connection.lock().await.remove(&id) {
            tokio::spawn(async move {
                let _ = x.close();
            });
        }
        let conn = self.get_conn().await?;
        self.query_connection.lock().await.insert(id, conn);
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        let x = self.query_connection.lock().await.remove(&id);
        if let Some(x) = x {
            x.close().await?;
        }
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl AutoCompletionTrait for SqliteDB {
    async fn auto_completion(
        &self, sql: &str, pos: sqlparser::tokenizer::Location,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        DBLexerCompletionBackend::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::SQLiteDialect {},
            pos,
        )
        .await
    }
}

#[async_trait::async_trait]
impl DBListCompletionItem for SqliteDB {
    fn is_multi_db(&self) -> bool {
        false
    }

    async fn list_table(&self, _db: Option<String>) -> anyhow::Result<Vec<String>> {
        crate::db::DBTrait::list_table(self)
            .await
            .map(|x| x.into_iter().map(|x| x.table).collect())
    }

    async fn list_db(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }

    async fn list_columns(
        &self, _db: Option<String>, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        crate::db::DBTrait::show_columns(self, table_name)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }
}

#[async_trait::async_trait]

impl crate::completion::DBLexerCompletionBackend for SqliteDB {
    type Dialect = sqlparser::dialect::SQLiteDialect;

    const BUILD_IN_FUNCTIONS: &'static [&'static str] = SQLITE_FUNTIONS;
    // https://sqlite.org/lang_keywords.html
    const KEY_WORDS: &'static [&'static str] = SQLITE_KEY_WORDS;
    const TEXTS: &'static [&'static str] = &["true"];
    const TYPES: &'static [&'static str] = SQLITE_TYPES;

    async fn extra_completion_item(&self) -> Vec<CompletionItem> {
        PRAGMA_ITEMS
            .iter()
            .map(|x| CompletionItem {
                name: Cow::Borrowed(*x),
                kind: CompletionKind::Text,
            })
            .collect()
    }
}

#[cfg(test)]
#[tokio::test]
async fn test_auto_completion() {
    use crate::completion::DBLexerCompletionBackend;
    let s = SqliteDB {
        pool: Default::default(),
        path: ":memory:".to_string(),
        read_only: false,
        query_connection: Default::default(),
    };
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
        let completion = DBLexerCompletionBackend::do_auto_completion(
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
        let completion = DBLexerCompletionBackend::do_auto_completion(
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
        let completion = DBLexerCompletionBackend::do_auto_completion(
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
        let completion = DBLexerCompletionBackend::do_auto_completion(
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
        let completion = DBLexerCompletionBackend::do_auto_completion(
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

fn extract_index(sql: &str) -> anyhow::Result<Option<(Option<String>, Vec<String>)>> {
    use sqlparser::dialect::SQLiteDialect;
    let ss = sqlparser::parser::Parser::parse_sql(&SQLiteDialect {}, sql)?;
    if ss.len() != 1 {
        anyhow::bail!("should contains one statement");
    }
    let s = ss.get(0).unwrap();

    match s {
        Statement::CreateIndex {
            name,
            table_name: _,
            using: _,
            columns,
            unique: _,
            concurrently: _,
            if_not_exists: _,
            include: _,
            nulls_distinct: _,
            predicate: _,
        } => {
            let name = name.clone().map(|x| x.to_string());
            let mut on = Vec::with_capacity(columns.len());
            for c in columns.iter() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_log;

    // this test no need clean up, we use memory sqlite db.
    fn make_test_object() -> SqliteDB {
        let pool_manager = PoolManager::new();
        let db = SqliteDB::new(pool_manager, ":memory:".to_string(), false);

        db
    }

    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object();
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }
    #[tokio::test]

    async fn test_rename() {
        let db = make_test_object();
        let db_trait = &db as &dyn DBTrait;
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
        let db = make_test_object();
        let db_trait = &db as &dyn DBTrait;
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
        let db = make_test_object();
        let mut conn = db.get_conn().await.unwrap();
        let db_trait = &db as &dyn DBTrait;
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
            // drop trigger now.
            db_trait.drop_trigger(table, &trigger.name).await.unwrap();
        }
    }
    #[tokio::test]
    async fn test_export_table() {
        let db = make_test_object();
        let mut conn = db.get_conn().await.unwrap();
        let table_name = "Persons";
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
        let db = &db as &dyn DBTrait;
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
    }

    #[tokio::test]
    async fn test_standard() {
        init_log();
        let db = make_test_object();
        let mut conn = db.get_conn().await.unwrap();
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS test_table (
                id INTEGER PRIMARY KEY autoincrement,
                null_col NULL,
                text_col TEXT,
                real_col REAL,
                blob_col BLOB,
                int_col INTEGER,
                numeric_col NUMERIC
            );
            CREATE INDEX idx ON test_table (real_col , int_col );
        "#,
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        // sqlx::query(
        //     r#"
        //     INSERT INTO test_table (null_col, text_col, real_col, blob_col, int_col,
        // numeric_col)         VALUES (NULL, 'Some text', 3.14,
        // X'0123456789ABCDEF', 42, 123.45); "#,
        // )
        // .execute(conn.deref_mut())
        // .await
        // .unwrap();
        let table_name = "test_table";

        let db = &db as &dyn crate::db::DBTrait;
        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert("text_col".to_string(), Some("Some text".to_string()));
        values.insert("real_col".to_string(), Some("3.14".to_string()));
        values.insert(
            "blob_col".to_string(),
            Some("X'0123456789ABCDEF'".to_string()),
        );
        values.insert("int_col".to_string(), Some("42".to_string()));
        values.insert("numeric_col".to_string(), Some("123.45".to_string()));

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
        println!("{:?}", indexes);
        let id = db.open_connection().await.unwrap();

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

        db.drop_view("test_view").await.unwrap();
        db.drop_table(table_name).await.unwrap();
    }
}

const SQLITE_FUNTIONS: &[&str] = &[
    "abs",
    "acos",
    "acosh",
    "asin",
    "asinh",
    "atan",
    "atan2",
    "atanh",
    "avg",
    "bm25",
    "ceil",
    "ceiling",
    "changes",
    "char",
    "coalesce",
    "cos",
    "cosh",
    "count",
    "cume_dist",
    "current_date",
    "current_time",
    "current_timestamp",
    "date",
    "datetime",
    "degrees",
    "dense_rank",
    "exp",
    "first_value",
    "floor",
    "format",
    "fts3_tokenizer",
    "fts5",
    "fts5_source_id",
    "geopoly_area",
    "geopoly_bbox",
    "geopoly_blob",
    "geopoly_ccw",
    "geopoly_contains_point",
    "geopoly_debug",
    "geopoly_group_bbox",
    "geopoly_json",
    "geopoly_overlap",
    "geopoly_regular",
    "geopoly_svg",
    "geopoly_within",
    "geopoly_xform",
    "glob",
    "group_concat",
    "hex",
    "highlight",
    "ifnull",
    "iif",
    "instr",
    "json",
    "json_array",
    "json_array_length",
    "json_extract",
    "json_group_array",
    "json_group_object",
    "json_insert",
    "json_object",
    "json_patch",
    "json_quote",
    "json_remove",
    "json_replace",
    "json_set",
    "json_type",
    "json_valid",
    "julianday",
    "lag",
    "last_insert_rowid",
    "last_value",
    "lead",
    "length",
    "like",
    "likelihood",
    "likely",
    "ln",
    "load_extension",
    "log",
    "log10",
    "log2",
    "lower",
    "ltrim",
    "match",
    "matchinfo",
    "max",
    "min",
    "mod",
    "nth_value",
    "ntile",
    "nullif",
    "offsets",
    "optimize",
    "percent_rank",
    "pi",
    "pow",
    "power",
    "printf",
    "quote",
    "radians",
    "random",
    "randomblob",
    "rank",
    "replace",
    "round",
    "row_number",
    "rtreecheck",
    "rtreedepth",
    "rtreenode",
    "rtrim",
    "sign",
    "sin",
    "sinh",
    "snippet",
    "soundex",
    "sqlite_compileoption_get",
    "sqlite_compileoption_used",
    "sqlite_log",
    "sqlite_source_id",
    "sqlite_version",
    "sqrt",
    "strftime",
    "substr",
    "substring",
    "subtype",
    "sum",
    "tan",
    "tanh",
    "time",
    "total",
    "total_changes",
    "trim",
    "trunc",
    "typeof",
    "unhex",
    "unicode",
    "unixepoch",
    "unlikely",
    "upper",
    "zeroblob",
];
const SQLITE_TYPES: &[&str] = &["NULL", "INTEGER", "REAL", "TEXT", "BLOB"];

const SQLITE_KEY_WORDS: &[&str] = &[
    "ABORT",
    "ACTION",
    "ADD",
    "AFTER",
    "ALL",
    "ALTER",
    "ALWAYS",
    "ANALYZE",
    "AND",
    "AS",
    "ASC",
    "ATTACH",
    "AUTOINCREMENT",
    "BEFORE",
    "BEGIN",
    "BETWEEN",
    "BY",
    "CASCADE",
    "CASE",
    "CAST",
    "CHECK",
    "COLLATE",
    "COLUMN",
    "COMMIT",
    "CONFLICT",
    "CONSTRAINT",
    "CREATE",
    "CROSS",
    "CURRENT",
    "CURRENT_DATE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "DATABASE",
    "DEFAULT",
    "DEFERRABLE",
    "DEFERRED",
    "DELETE",
    "DESC",
    "DETACH",
    "DISTINCT",
    "DO",
    "DROP",
    "EACH",
    "ELSE",
    "END",
    "ESCAPE",
    "EXCEPT",
    "EXCLUDE",
    "EXCLUSIVE",
    "EXISTS",
    "EXPLAIN",
    "FAIL",
    "FILTER",
    "FIRST",
    "FOLLOWING",
    "FOR",
    "FOREIGN",
    "FROM",
    "FULL",
    "GENERATED",
    "GLOB",
    "GROUP",
    "GROUPS",
    "HAVING",
    "IF",
    "IGNORE",
    "IMMEDIATE",
    "IN",
    "INDEX",
    "INDEXED",
    "INITIALLY",
    "INNER",
    "INSERT",
    "INSTEAD",
    "INTERSECT",
    "INTO",
    "IS",
    "ISNULL",
    "JOIN",
    "KEY",
    "LAST",
    "LEFT",
    "LIKE",
    "LIMIT",
    "MATCH",
    "MATERIALIZED",
    "NATURAL",
    "NO",
    "NOT",
    "NOTHING",
    "NOTNULL",
    "NULL",
    "NULLS",
    "OF",
    "OFFSET",
    "ON",
    "OR",
    "ORDER",
    "OTHERS",
    "OUTER",
    "OVER",
    "PARTITION",
    "PLAN",
    "PRAGMA",
    "PRECEDING",
    "PRIMARY",
    "QUERY",
    "RAISE",
    "RANGE",
    "RECURSIVE",
    "REFERENCES",
    "REGEXP",
    "REINDEX",
    "RELEASE",
    "RENAME",
    "REPLACE",
    "RESTRICT",
    "RETURNING",
    "RIGHT",
    "ROLLBACK",
    "ROW",
    "ROWS",
    "SAVEPOINT",
    "SELECT",
    "SET",
    "TABLE",
    "TEMP",
    "TEMPORARY",
    "THEN",
    "TIES",
    "TO",
    "TRANSACTION",
    "TRIGGER",
    "UNBOUNDED",
    "UNION",
    "UNIQUE",
    "UPDATE",
    "USING",
    "VACUUM",
    "VALUES",
    "VIEW",
    "VIRTUAL",
    "WHEN",
    "WHERE",
    "WINDOW",
    "WITH",
    "WITHOUT",
];

const PRAGMA_ITEMS: &'static [&'static str] = &[
    "analysis_limit",
    "application_id",
    "auto_vacuum",
    "automatic_index",
    "busy_timeout",
    "cache_size",
    "cache_spill",
    "case_sensitive_like",
    "cell_size_check",
    "checkpoint_fullfsync",
    "collation_list",
    "compile_options",
    "count_changes",
    "data_store_directory",
    "data_version",
    "database_list",
    "default_cache_size",
    "defer_foreign_keys",
    "empty_result_callbacks",
    "encoding",
    "foreign_key_check",
    "foreign_key_list",
    "foreign_keys",
    "freelist_count",
    "full_column_names",
    "fullfsync",
    "function_list",
    "hard_heap_limit",
    "ignore_check_constraints",
    "incremental_vacuum",
    "index_info",
    "index_list",
    "index_xinfo",
    "integrity_check",
    "journal_mode",
    "journal_size_limit",
    "legacy_alter_table",
    "legacy_file_format",
    "locking_mode",
    "max_page_count",
    "mmap_size",
    "module_list",
    "optimize",
    "page_count",
    "page_size",
    "parser_trace",
    "pragma_list",
    "query_only",
    "quick_check",
    "read_uncommitted",
    "recursive_triggers",
    "reverse_unordered_selects",
    "schema_version",
    "secure_delete",
    "short_column_names",
    "shrink_memory",
    "soft_heap_limit",
    "stats",
    "synchronous",
    "table_info",
    "table_list",
    "table_xinfo",
    "temp_store",
    "temp_store_directory",
    "threads",
    "trusted_schema",
    "user_version",
    "vdbe_addoptrace",
    "vdbe_debug",
    "vdbe_listing",
    "vdbe_trace",
    "wal_autocheckpoint",
    "wal_checkpoint",
    "writable_schema",
];
