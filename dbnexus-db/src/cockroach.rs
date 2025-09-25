use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;

use sqlx::pool::PoolConnection;
use sqlx::{AssertSqlSafe, Postgres, Row};

use crate::datasource_config::PGDBConfig;
use crate::db::{
    BackendKind, CatalogTrait, CheckConn, ColumnType, CreateDatabaseExtra, CreateSchemaExtra,
    CreatedTypeDef, DBTrait, DefAndMetas, ExportKind, ExportQueryResult, Field, GetBackendKind,
    Index, IndexKind, ListTable, NamedStaff, ProcedureDetail, QueryResp, QuerySql, ReadOnlyTrait,
    SchemaTrait, ServerMetas, ShowExtraArgs, SqlBuilder, SqlExecuteRet,
    UdfOrProcedureExtraParameter, XlsxEdit,
};
use crate::meta_value::MetaValue;
use crate::pg::PostGresDB;
use crate::pg_format::encode_rows;
impl GetBackendKind for CockRoachDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::CockRoach
    }
}
#[derive(Clone)]
pub struct CockRoachDB {
    pub(crate) db: Arc<PostGresDB>,
}

impl CockRoachDB {
    pub async fn new(conn: PGDBConfig) -> anyhow::Result<Self> {
        let pg = PostGresDB::new(conn).await?;
        anyhow::Ok(Self { db: Arc::new(pg) })
    }

    pub(crate) async fn get_conn(&self) -> anyhow::Result<PoolConnection<Postgres>> {
        self.db.get_conn().await
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
        ssl: Default::default(),
        conn_timeout: Some(1000),
    })
    .await
    .unwrap();
    pg
}

#[async_trait::async_trait]
impl CheckConn for CockRoachDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        self.db.check_conn().await
    }
}

impl ReadOnlyTrait for CockRoachDB {
    fn read_only(&self) -> bool {
        self.db.config.read_only
    }
}

#[async_trait::async_trait]

impl QuerySql for CockRoachDB {
    async fn get_auto_commit(&self, _query_id: u32) -> anyhow::Result<bool> {
        self.db.get_auto_commit(_query_id).await
    }

    async fn set_auto_commit(&self, query_id: u32, c: bool) -> anyhow::Result<()> {
        self.db.set_auto_commit(query_id, c).await
    }

    async fn commit(&self, query_id: u32) -> anyhow::Result<()> {
        self.db.commit(query_id).await
    }

    async fn rollback(&self, query_id: u32) -> anyhow::Result<()> {
        self.db.rollback(query_id).await
    }

    async fn query_sql(
        &self, query_id: u32, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        self.db.query_sql(query_id, sql).await
    }
    async fn execute_sql_no_open_conn(
        &self, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        self.db.execute_sql_no_open_conn(sql).await
    }
    async fn execute_sql(
        &self, query_id: u32, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        self.db.execute_sql(query_id, sql).await
    }

    async fn open_connection(&self) -> anyhow::Result<u32> {
        self.db.open_connection().await
    }

    async fn re_connect(&self, id: u32) -> anyhow::Result<()> {
        self.db.re_connect(id).await
    }

    async fn close_connection(&self, id: u32) -> anyhow::Result<()> {
        self.db.close_connection(id).await
    }
}
#[async_trait::async_trait]

impl ExportQueryResult for CockRoachDB {
    async fn export_as_xlsx(
        &self, sql: &str, include_header: bool, sender: tokio::sync::mpsc::Sender<XlsxEdit>,
    ) -> anyhow::Result<()> {
        ExportQueryResult::export_as_xlsx(self.db.as_ref(), sql, include_header, sender).await
    }
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }

    async fn export_query_result(
        &self, sql: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        self.db.export_query_result(sql, kind, sender).await
    }
}

#[async_trait::async_trait]

impl ServerMetas for CockRoachDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        self.db.show_metas().await
    }
}

impl std::fmt::Debug for CockRoachDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cockroach on:{:?}", self.db.config)
    }
}

impl SqlBuilder for CockRoachDB {
    fn put_value_in_sql(
        &self, typ: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        self.db.put_value_in_sql(typ, None, value)
    }
}

fn gen_show_sql(
    schema: &str, table_name: &str, fields: &[&str], where_str: Option<&str>,
    order_by: Option<&str>, page: usize, page_size: usize,
) -> String {
    let mut sql = format!(
        r#"SELECT "{}" from "{schema}"."{table_name}""#,
        fields.join(r#"",""#)
    );
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
pub(crate) const COCKROACH_TYPES: &[&str] = &[
    "BIT",
    "BIT[]",
    "BOOL",
    "BOOL[]",
    "BYTES",
    "BYTES[]",
    "COLLATE",
    "COLLATE[]",
    "DATE",
    "DATE[]",
    "DECIMAL",
    "DECIMAL[]",
    "FLOAT",
    "FLOAT[]",
    "INET",
    "INET[]",
    "INT",
    "INT[]",
    "INTERVAL",
    "INTERVAL[]",
    "NULL",
    "NULL[]",
    "JSONB",
    "JSONB[]",
    "OID",
    "OID[]",
    "SERIAL",
    "STRING",
    "STRING[]",
    "TIME",
    "TIME[]",
    "TIMETZ",
    "TIMETZ[]",
    "TIMESTAMP",
    "TIMESTAMP[]",
    "TIMESTAMPTZ",
    "TIMESTAMPTZ[]",
    "TSQUERY",
    "TSVECTOR",
    "UUID",
    "VECTOR",
];

#[async_trait::async_trait]

impl SchemaTrait for CockRoachDB {
    async fn export_table_or_view_as_xlsx(
        &self, name: &str, include_header: bool, sender: tokio::sync::mpsc::Sender<XlsxEdit>,
    ) -> anyhow::Result<()> {
        SchemaTrait::export_table_or_view_as_xlsx(self.db.as_ref(), name, include_header, sender)
            .await
    }
    async fn list_created_types(&self) -> anyhow::Result<Vec<CreatedTypeDef>> {
        self.db.list_created_types().await
    }
    async fn drop_created_type(&self, name: &str) -> anyhow::Result<()> {
        self.db.drop_created_type(name).await
    }
    fn list_sql_current_values(&self) -> &'static [&'static str] {
        self.db.list_sql_current_values()
    }
    async fn add_table_column(
        &self, table: &str, name: &str, col: ColumnType,
    ) -> anyhow::Result<()> {
        self.db.add_table_column(table, name, col).await
    }

    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        self.db
            .modify_table_column(table, old_name, new_name, typ)
            .await
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        self.db.drop_table_column(table, col).await
    }

    fn list_data_types(&self) -> anyhow::Result<&'static [&'static str]> {
        anyhow::Ok(COCKROACH_TYPES)
    }

    async fn export_table_or_view(
        &self, name: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        self.db.export_table_or_view(name, kind, sender).await
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        self.db.rename_table(old, new).await
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        self.db.duplicate_table(table, dup).await
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.db.list_table().await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        self.db.show_table_columns(table_name).await
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        let fields = self.show_table_columns(table_name).await?;
        let cols = fields.iter().map(|x| x.field.as_str()).collect::<Vec<_>>();
        let total = self.db.query_count(table_name, where_str).await?;
        let sql = gen_show_sql(
            self.db.config.schema_str(),
            table_name,
            &cols[..],
            where_str,
            order_by,
            page,
            page_size,
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await
            .map_err(|e| anyhow::Error::msg(format!("err:'{}' sql:'{}'", e, sql)))?;
        let data = encode_rows(&row)?;

        anyhow::Ok(QueryResp::new(fields, total, data, sql)?)
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.db.table_insert(table_name, values).await
    }

    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.db.table_update(table_name, values, wheres).await
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.db.truncate_table(table_name).await
    }

    async fn table_delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.db.table_delete(table_name, wheres).await
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        self.db.drop_table(table, cascade).await
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.db.list_procedure().await
    }

    async fn show_procedure(
        &self, name: &str, arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<ProcedureDetail> {
        self.db.show_procedure(name, arg).await
    }

    async fn drop_procedure(
        &self, name: &str, arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        self.db.drop_procedure(name, arg).await
    }

    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.db.list_materialized_view().await
    }

    async fn drop_materialized_view(&self, name: &str) -> anyhow::Result<()> {
        self.db.drop_materialized_view(name).await
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"
            SELECT  table_name 
            FROM information_schema.tables
            WHERE table_type = 'VIEW'
                AND table_schema = '{}' and table_name not in (SELECT 
                    matviewname
            FROM 
                pg_matviews where schemaname = '{}' ) 
        "#,
            self.db.config.schema_str(),
            self.db.config.schema_str(),
        );
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::new();
        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            ret.push(ListTable::from(name));
        }

        anyhow::Ok(ret)
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.db.drop_view(view_name).await
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        self.db
            .show_view(view_name, where_str, order_by, page, page_size, extra)
            .await
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.db.list_udf().await
    }

    async fn show_udf(
        &self, name: &str, arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<DefAndMetas> {
        self.db.show_udf(name, arg).await
    }

    async fn drop_udf(
        &self, name: &str, extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        self.db.drop_udf(name, extra).await
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        self.db.show_view_columns(view_name).await
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        self.db.show_indexes(table_name).await
    }

    async fn create_index(
        &self, table: &str, name: &str, kind: IndexKind, fields: &[String],
    ) -> anyhow::Result<()> {
        self.db.create_index(table, name, kind, fields).await
    }

    async fn drop_index(&self, table_name: &str, index: &str) -> anyhow::Result<()> {
        self.db.drop_index(table_name, index).await
    }

    async fn rename_index(
        &self, table: &str, old_name: &str, new_name: &str,
    ) -> anyhow::Result<()> {
        self.db.rename_index(table, old_name, new_name).await
    }
}

#[async_trait::async_trait]
impl DBTrait for CockRoachDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        self.db.list_schema().await
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let pg = self.db.open_schema(name).await?;
        let pg = pg.as_any().downcast_ref::<PostGresDB>().unwrap();
        let pg = Clone::clone(pg);
        anyhow::Ok(Arc::new(Self { db: Arc::new(pg) }))
    }

    async fn create_schema(
        &self, name: &str, extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.db.create_schema(name, extra).await
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.db.drop_schema(name).await
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait::async_trait]
impl CatalogTrait for CockRoachDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        self.db.list_database().await
    }

    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        let pg = self.db.open_db(name).await?;
        let pg = pg.as_any().downcast_ref::<PostGresDB>().unwrap();
        let pg = Clone::clone(pg);
        anyhow::Ok(Arc::new(Self { db: Arc::new(pg) }))
    }

    async fn create_database(
        &self, name: &str, extra: Option<CreateDatabaseExtra>,
    ) -> anyhow::Result<()> {
        self.db.create_database(name, extra).await
    }

    async fn drop_database(&self, name: &str) -> anyhow::Result<()> {
        self.db.drop_database(name).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{IndexKind, UdfOrProcedureExtraParameter};
    use crate::enable_test_log;

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
                typ: "INT2".to_string(),
                nullness: Some("null".to_string()),
                after: None,
                default_value: Some("1".to_string()),
            },
        )
        .await
        .unwrap();

        db.modify_table_column(
            table_name,
            "name",
            "name2",
            Some(ColumnType {
                typ: "INT4".to_string(),
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
    async fn test_index() {
        let table_name = "index_test";
        let db = make_test_object().await;
        let index_name = "index_test_index_xxx";

        let _ = db.drop_index(table_name, &index_name).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();

        db.query_sql(query_id, format!(
            r#" create table {table_name} (a int, b int, c int, constraint pk_test primary key(a, b)); "#,
        ).as_str()).await .unwrap()
        ;

        let index_name2 = format!("{}2", index_name);
        db.create_index(
            table_name,
            index_name,
            IndexKind::Unique,
            &["a".to_string(), "b".to_string()],
        )
        .await
        .unwrap();
        db.rename_index(table_name, index_name, &index_name2)
            .await
            .unwrap();
        db.rename_index(table_name, &index_name2, index_name)
            .await
            .unwrap();

        let indexes = db.show_indexes(table_name).await.unwrap();
        for index in indexes.iter() {
            println!("index:{:?}", index)
        }
    }

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
    async fn test_rename() {
        let db = make_test_object().await;

        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_table("test_rename", false).await;
        let _ = db.drop_table("test_rename2", false).await;
        db.query_sql(
            query_id,
            r#"
                CREATE TABLE test_rename (
                    
                    name VARCHAR(255) NOT NULL,
                    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT CURRENT_TIMESTAMP
                );"#,
        )
        .await
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
                CREATE TABLE test_duplicate (
                    
                    data TEXT,
                    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT CURRENT_TIMESTAMP
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
    async fn test_procedure() {
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_procedure("test_procedure", None).await;
        db.query_sql(
            query_id,
            r#"
            CREATE PROCEDURE test_procedure()
            LANGUAGE plpgsql AS
            $$
            BEGIN
                -- procedure logic goes here
            END;
            $$;
        "#,
        )
        .await
        .unwrap();
        let list = db.list_procedure().await.unwrap();
        for l in list.iter() {
            let details = db
                .show_procedure(
                    &l.name,
                    Some(UdfOrProcedureExtraParameter::PostGres {
                        parameters: "".to_string(),
                    }),
                )
                .await
                .unwrap();
            println!("udf:{},details:{:?}", l.name, details);
        }
    }
    #[tokio::test]
    async fn test_view() {
        enable_test_log();
        let db = make_test_object().await;
        let _ = db.drop_view("test_view").await;
        let _ = db.drop_table("test_view_table", true).await;

        let query_id = db.open_connection().await.unwrap();
        let query_resp = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_view_table (
                
                name TEXT,
                value INT
            );
            INSERT INTO test_view_table (name, value) VALUES
                ('Item A', 10),
                ('Item B', 20),
                ('Item C', 30);

            CREATE VIEW test_view AS
                SELECT * FROM test_view_table;
        "#,
            )
            .await
            .unwrap();
        assert!(
            query_resp.iter().all(|x| x.is_ok()),
            "result:{:?}",
            query_resp
        );
        let list = db.list_view().await.unwrap();
        for l in list.iter() {
            let table_resp = db
                .show_view(&l.name, None, None, 1, 100, None)
                .await
                .unwrap();
            println!("resp:{:?}", table_resp);
        }
    }

    #[tokio::test]
    async fn test_materialized_view() {
        enable_test_log();
        let db = make_test_object().await;
        let _ = db.drop_materialized_view("test_material_view").await;
        let _ = db.drop_table("test_material_view_table", true).await;

        let query_id = db.open_connection().await.unwrap();
        let query_resp = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_material_view_table (
                
                name TEXT,
                value INT
            );
            INSERT INTO test_material_view_table (name, value) VALUES
                ('Item A', 10),
                ('Item B', 20),
                ('Item C', 30);

            CREATE MATERIALIZED VIEW test_material_view AS
                SELECT * FROM test_material_view_table;
        "#,
            )
            .await
            .unwrap();
        assert!(
            query_resp.iter().all(|x| x.is_ok()),
            "result:{:?}",
            query_resp
        );
        let list = db.list_materialized_view().await.unwrap();
        for l in list.iter() {
            let table_resp = db
                .show_view(&l.name, None, None, 1, 100, None)
                .await
                .unwrap();
            println!("resp:{:?}", table_resp);
        }
    }

    #[tokio::test]
    async fn test_server_metas() {
        let db = make_test_object().await;
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }
    #[tokio::test]
    async fn test_list_table() {
        let db = make_test_object().await;
        let t = db.list_table().await.unwrap();
        for t in t.iter() {
            println!("{:?}", t);
        }
    }

    #[tokio::test]
    async fn test_udf() {
        let pg = make_test_object().await;
        let query_id = pg.open_connection().await.unwrap();
        let udf_name = "test_udf";
        let _ = pg
            .drop_udf(
                udf_name,
                Some(UdfOrProcedureExtraParameter::PostGres {
                    parameters: "a integer, b integer".to_string(),
                }),
            )
            .await;

        pg.query_sql(
            query_id,
            r#"
            CREATE OR REPLACE FUNCTION test_udf(a INT, b INT)
            RETURNS INT AS $$
            BEGIN
                RETURN a + b;
            END;
            $$ LANGUAGE plpgsql;
        "#,
        )
        .await
        .unwrap();
        let list = pg.list_udf().await.unwrap();
        for l in list.iter() {
            // I created some other functions to test.
            // so we only test on `test_udf`.
            if l.name != udf_name {
                continue;
            }
            println!("udf:{:?}", l);
            let details = pg
                .show_udf(
                    &l.name,
                    Some(UdfOrProcedureExtraParameter::PostGres {
                        parameters: "int8, int8".to_string(),
                    }),
                )
                .await
                .unwrap();
            println!("name:{} , udf:{:?}", l.name, details);
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
        db.table_insert(table_name, values.clone()).await.unwrap();
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
