#![allow(unused)]

use std::{sync::Arc, time::Duration};

use r2d2::PooledConnection;
use tokio::task::block_in_place;

use crate::{
    datasource_config::ClickHouseConfig,
    db::{
        BackendName, CheckConn, CreateSchemaExtra, DBTrait, QueryResp, QuerySql, SchemaTrait,
        ServerMetas,
    },
    meta_value::MetaValue,
};

impl BackendName for ClickHouseDB {
    fn backend_name(&self) -> &str {
        "clickhouse"
    }
}

pub struct PoolManager {
    config: ClickHouseConfig,
}

impl r2d2::ManageConnection for PoolManager {
    type Connection = clickhouse::Client;
    type Error = clickhouse::error::Error;
    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let mut client = clickhouse::Client::default();
        client = client.with_url(format!("http://{}:{}", self.config.host, self.config.port));
        client = client.with_user(self.config.username.clone());
        if let Some(pass) = self.config.password.as_ref() {
            client = client.with_password(pass);
        }
        if let Some(db) = self.config.db.as_ref() {
            client = client.with_database(db);
        }
        Ok(client)
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        let sql = "select 1";
        conn.query(&sql).fetch::<i32>()?;
        Ok(())
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        self.is_valid(conn).is_ok()
    }
}

#[async_trait::async_trait]
impl ServerMetas for ClickHouseDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        unimplemented!()
    }
}

#[async_trait::async_trait]

impl QuerySql for ClickHouseDB {
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

impl std::fmt::Debug for ClickHouseDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "host={} port={} username={}",
            self.config.host, self.config.port, self.config.username
        )
    }
}

pub struct ClickHouseDB {
    config: ClickHouseConfig,
    pool: r2d2::Pool<PoolManager>,
}
#[async_trait::async_trait]

impl CheckConn for ClickHouseDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        unimplemented!()
    }
}

impl ClickHouseDB {
    pub fn new(config: ClickHouseConfig) -> anyhow::Result<Self> {
        let pool = r2d2::Builder::<PoolManager>::new().build(PoolManager {
            config: config.clone(),
        })?;
        anyhow::Ok(Self { config, pool })
    }
    fn get_conn(&self) -> anyhow::Result<PooledConnection<PoolManager>> {
        let conn = block_in_place(|| self.pool.get())?;

        anyhow::Ok(conn)
    }
}

#[async_trait::async_trait]
impl DBTrait for ClickHouseDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let conn = self.get_conn()?;
        let databases: Vec<String> = conn.query("show databases").fetch_all().await.unwrap();
        anyhow::Ok(databases)
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

#[cfg(test)]
fn make_test_object() -> ClickHouseDB {
    ClickHouseDB::new(ClickHouseConfig {
        host: "127.0.0.1".to_string(),
        port: 8123,
        read_only: false,
        db: Some("default".to_string()),
        username: "default".to_string(),
        password: Some("root".to_string()),
    })
    .unwrap()
}

#[cfg(test)]
mod tests {
    use crate::db::DBTrait;

    use super::make_test_object;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_list_databases() {
        let db = make_test_object();
        let schemas = db.list_schema().await.unwrap();
        println!("schemas :{:?}", schemas);
    }
}
