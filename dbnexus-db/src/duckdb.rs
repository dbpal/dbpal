#![allow(unused)]
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use r2d2::PooledConnection;
use tokio::task::block_in_place;

use crate::datasource_config::DuckDBConfig;
use crate::db::{
    BackendName, CatalogTrait, CheckConn, CreateDatabaseExtra, CreateSchemaExtra, DBTrait, Field,
    ListTable, QueryResp, QuerySql, SchemaTrait, ServerMetas, ShowExtraArgs, SqlExecuteRet,
    TableListResp, not_support_features,
};
use crate::meta_value::MetaValue;

pub struct PoolManage {
    path: String,
}

impl r2d2::ManageConnection for PoolManage {
    type Connection = duckdb::Connection;
    type Error = duckdb::Error;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let conn = duckdb::Connection::open(&self.path)?;
        Ok(conn)
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        let sql = "select 1";
        conn.execute(&sql, [])?;
        Ok(())
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        self.is_valid(conn).is_ok()
    }
}

#[derive(Clone)]
pub struct DuckDB {
    config: DuckDBConfig,
    pool: r2d2::Pool<PoolManage>,
}

#[async_trait::async_trait]

impl ServerMetas for DuckDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        unimplemented!()
    }
}

impl DuckDB {
    pub fn new(config: DuckDBConfig) -> anyhow::Result<Self> {
        let pool = r2d2::Builder::<PoolManage>::new().build(PoolManage {
            path: config.path.clone(),
        })?;
        anyhow::Ok(Self { config, pool })
    }

    pub(crate) fn get_conn(&self) -> anyhow::Result<PooledConnection<PoolManage>> {
        let conn = block_in_place(|| self.pool.get())?;
        anyhow::Ok(conn)
    }
}

impl std::fmt::Debug for DuckDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "duckdb@{}", self.config.path)
    }
}

#[async_trait::async_trait]
impl CheckConn for DuckDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let conn = self.get_conn()?;

        let now = std::time::Instant::now();
        conn.execute("select 1", [])?;
        anyhow::Ok(now.elapsed())
    }
}

impl BackendName for DuckDB {
    fn backend_name(&self) -> &str {
        "duckdb"
    }
}

#[async_trait::async_trait]
impl QuerySql for DuckDB {
    async fn query_sql(
        &self, query_id: uuid::Uuid, _sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        unimplemented!()
    }

    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        unimplemented!()
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        unimplemented!()
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        unimplemented!()
    }
}

#[async_trait::async_trait]

impl SchemaTrait for DuckDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        unimplemented!()
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        unimplemented!()
    }

    fn read_only(&self) -> bool {
        unimplemented!()
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        unimplemented!()
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        unimplemented!()
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
        unimplemented!()
    }

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        unimplemented!()
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        unimplemented!()
    }

    async fn delete(
        &self, _table_name: &str, _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        unimplemented!()
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        unimplemented!()
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        unimplemented!()
    }
}

#[async_trait::async_trait]

impl DBTrait for DuckDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let sql = "SHOW SCHEMAS;";
        let conn = self.get_conn()?;
        let mut p = conn.prepare(&sql)?;
        let mut rows = block_in_place(|| p.query([]))?;
        let mut ret = Vec::new();
        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        unimplemented!()
    }

    async fn create_schema(
        &self, name: &str, extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        unimplemented!()
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl CatalogTrait for DuckDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        let sql = "SELECT database_name FROM duckdb_databases();";
        let conn = self.get_conn()?;

        let mut p = conn.prepare(&sql)?;
        let mut rows = p.query([])?;
        let mut ret = Vec::new();

        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        let mut config = self.config.clone();
        let d = DuckDB::new(config)?;
        anyhow::Ok(Arc::new(d))
    }
    async fn create_database(
        &self, name: &str, extra: Option<CreateDatabaseExtra>,
    ) -> anyhow::Result<()> {
        let path = match &extra {
            Some(x) => match &x {
                CreateDatabaseExtra::DuckDB { path } => path,
            },
            None => anyhow::bail!("attch database need extra parameter."),
        };
        let sql = format!("ATTACH '{path}' AS {name};");
        log::info!("attach sql:{}", sql);
        let conn = self.get_conn()?;
        conn.execute(&sql, [])?;
        anyhow::Ok(())
    }

    async fn drop_database(&self, name: &str) -> anyhow::Result<()> {
        if self
            .config
            .database
            .as_ref()
            .map(|x| x.as_str().eq_ignore_ascii_case(&name))
            .unwrap_or_default()
        {
            anyhow::bail!("can't detach self.")
        }

        let sql = format!("DETACH  {name}");
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, []))?;
        anyhow::Ok(())
    }
}

#[cfg(test)]
fn make_test_object(schema: Option<String>) -> DuckDB {
    let dir = env!("CARGO_MANIFEST_DIR");
    let path = format!("{dir}/tests/test.duckdb");

    DuckDB::new(DuckDBConfig {
        path,
        read_only: false,
        database: Some("test".to_string()),
        schema,
    })
    .unwrap()
}

#[cfg(test)]
mod tests {
    use crate::enable_test_log;

    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_database() {
        enable_test_log();

        let db = make_test_object(None);
        db.create_database(
            "abc1111",
            Some(CreateDatabaseExtra::DuckDB {
                path: {
                    let dir = env!("CARGO_MANIFEST_DIR");
                    let path = format!("{dir}/tests/abc.duckdb");
                    path
                },
            }),
        )
        .await
        .unwrap();
        println!("11111111111111");

        tokio::time::sleep(Duration::from_secs(5)).await;
        println!("222222");
        let x = db.list_database().await.unwrap();
        println!("database:{:?}", x);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]

    async fn test_schema() {
        let db = make_test_object(None);

        let x = db.list_schema().await.unwrap();
        println!("schemas:{:?}", x);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_check_conn() {
        let db = make_test_object(None);
        let d = crate::db::CheckConn::check_conn(&db).await.unwrap();
        println!("ping time is:{:?}", d);
    }

    #[test]
    fn test_xxx() {}
}
