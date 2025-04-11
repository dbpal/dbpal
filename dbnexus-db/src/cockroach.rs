#![allow(unused)]
use std::{borrow::Cow, collections::HashMap, sync::Arc, time::Duration};

use crate::{
    db::{
        BackendName, CatalogTrait, CreateDatabaseExtra, CreateSchemaExtra, DBTrait, ExportKind,
        ExportQueryResult, Field, ListTable, QueryResp, QuoteStyle, SchemaTrait, ServerMetas,
        ShowExtraArgs, SqlBuilder, SqlExecuteRet, TableListResp,
    },
    helper::is_dangerous_sql_by_parser,
    meta_value::MetaValue,
};
use sqlx::{Postgres, pool::PoolConnection, types::BigDecimal};

use crate::{
    datasource_config::PGDBConfig,
    db::{CheckConn, QuerySql, ReadOnlyTrait},
    pg::PostGresDB,
};

impl BackendName for CockRoachDB {
    fn backend_name(&self) -> &str {
        "cockroachdb"
    }
}
#[derive(Clone)]
pub struct CockRoachDB {
    pg: Arc<PostGresDB>,
}

impl CockRoachDB {
    pub async fn new(conn: PGDBConfig) -> anyhow::Result<Self> {
        let pg = PostGresDB::new(conn).await?;
        anyhow::Ok(Self { pg: Arc::new(pg) })
    }
    pub(crate) async fn get_conn(&self) -> anyhow::Result<PoolConnection<Postgres>> {
        self.pg.get_conn().await
    }
}
#[cfg(test)]
pub(crate) async fn make_test_object() -> CockRoachDB {
    let pg = CockRoachDB::new(PGDBConfig {
        host: crate::TEST_HOST.to_string(),
        port: 26257,
        username: "root".to_string(),
        password: None,
        db: Some("test".into()),
        schema: Some("xxx".to_string()),
        read_only: false,
        save_password: true,
    })
    .await
    .unwrap();
    pg
}

#[async_trait::async_trait]
impl CheckConn for CockRoachDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        self.pg.check_conn().await
    }
}

impl ReadOnlyTrait for CockRoachDB {
    fn read_only(&self) -> bool {
        self.pg.config.read_only
    }
}

#[async_trait::async_trait]

impl QuerySql for CockRoachDB {
    async fn query_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        self.pg.query_sql(query_id, sql).await
    }

    async fn execute_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        self.pg.execute_sql(query_id, sql).await
    }

    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        self.pg.open_connection().await
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.pg.re_connect(id).await
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.pg.close_connection(id).await
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        is_dangerous_sql_by_parser(sql, &sqlparser::dialect::PostgreSqlDialect {})
    }
}
#[async_trait::async_trait]

impl ExportQueryResult for CockRoachDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }

    async fn export_query_result(
        &self, sql: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        self.pg.export_query_result(sql, kind, sender).await
    }
}

#[async_trait::async_trait]

impl ServerMetas for CockRoachDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        self.pg.show_metas().await
    }
}

impl std::fmt::Debug for CockRoachDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cochroach on:{:?}", self.pg.config)
    }
}

impl QuoteStyle for CockRoachDB {
    fn quote_style(&self) -> &'static str {
        self.pg.quote_style()
    }
}

impl SqlBuilder for CockRoachDB {
    fn put_value_in_sql(
        &self, typ: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        self.pg.put_value_in_sql(typ, None, value)
    }
}

#[async_trait::async_trait]

impl SchemaTrait for CockRoachDB {
    async fn export_table_or_view(
        &self, name: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        self.pg.export_table_or_view(name, kind, sender).await
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        self.pg.rename_table(old, new).await
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        self.pg.duplicate_table(table, dup).await
    }
    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        unimplemented!()
    }
    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        self.pg.show_table_columns(table_name).await
    }
    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<TableListResp> {
        unimplemented!()
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.pg.insert(table_name, values).await
    }
    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.pg.insert(table_name, values).await
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.pg.truncate_table(table_name).await
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.pg.delete(table_name, wheres).await
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        self.pg.drop_table(table, cascade).await
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }
}

#[async_trait::async_trait]
impl DBTrait for CockRoachDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        self.pg.list_schema().await
    }
    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let pg = self.pg.open_db(name).await?;
        let pg = pg.as_any().downcast_ref::<PostGresDB>().unwrap();
        let pg = Clone::clone(pg);
        anyhow::Ok(Arc::new(Self { pg: Arc::new(pg) }))
    }
    async fn create_schema(
        &self, name: &str, extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.pg.create_schema(name, extra).await
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.pg.drop_schema(name).await
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait::async_trait]
impl CatalogTrait for CockRoachDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        self.pg.list_database().await
    }
    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        let pg = self.pg.open_db(name).await?;
        let pg = pg.as_any().downcast_ref::<PostGresDB>().unwrap();
        let pg = Clone::clone(pg);
        anyhow::Ok(Arc::new(Self { pg: Arc::new(pg) }))
    }

    async fn create_database(
        &self, name: &str, extra: Option<CreateDatabaseExtra>,
    ) -> anyhow::Result<()> {
        self.pg.create_database(name, extra).await
    }

    async fn drop_database(&self, name: &str) -> anyhow::Result<()> {
        self.pg.drop_database(name).await
    }
}

#[cfg(test)]
mod tests {
    use crate::enable_test_log;

    use super::*;
    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object().await;
        let dur = CheckConn::check_conn(&db).await.unwrap();
        println!("duration:{:?}", dur)
    }
    #[tokio::test]
    async fn test_databases() {
        let db = make_test_object().await;
        let _ = CatalogTrait::drop_database(&db, "abc").await;
        CatalogTrait::create_database(&db, "abc", None)
            .await
            .unwrap();
        let dbs = CatalogTrait::list_database(&db).await.unwrap();
        println!("{:?}", dbs);
    }
    #[tokio::test]
    async fn test_schemas() {
        let db = make_test_object().await;
        let _ = db.drop_schema("abc").await;
        let _ = db.create_schema("abc", None).await;
        let resp = db.list_schema().await.unwrap();
        println!("{:?}", resp);
    }
    #[tokio::test]
    async fn test_list_table_columns() {
        let db: CockRoachDB = make_test_object().await;

        let x = db.show_table_columns("test_types1").await.unwrap();
        for f in x.iter() {
            println!("{:?}", f);
        }
    }

    #[tokio::test]
    async fn test_types1() {
        enable_test_log();
        let table_name = "test_types1";
        let db = make_test_object().await;
        let _ = db.drop_table(&table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        db.query_sql(
            query_id,
            format!(
                r#"
        CREATE TABLE {table_name} (
            bool_column BOOL,
            bytea_column BYTEA,
            char_column CHAR,
            name_column NAME,
            int8_column INT8,
            int2_column INT2,
            int4_column INT4
        );"#
            )
            .as_str(),
        )
        .await
        .unwrap();
        let mut values = HashMap::new();
        values.insert("bool_column".to_string(), Some("true".to_string()));
        values.insert(
            "bytea_column".to_string(),
            Some("\\\\x0123456789ABCDEF".to_string()),
        );
        values.insert("char_column".to_string(), Some("A".to_string()));
        values.insert("name_column".to_string(), Some("John".to_string()));
        values.insert("int8_column".to_string(), Some("1".to_string()));
        values.insert("int2_column".to_string(), Some("2".to_string()));
        values.insert("int4_column".to_string(), Some("100".to_string()));
        values.insert("rowid".to_string(), Some("unique_rowid()".to_string()));
        db.insert(table_name, values.clone()).await.unwrap();
        let mut wheres = values.clone();
        wheres.insert("id".to_string(), Some("1".to_string()));

        let json = db
            .export_table_to_string("test_types1", ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(json.as_str()).unwrap();

        println!("export json:{}", json);
    }
}
