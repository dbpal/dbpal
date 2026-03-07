#![allow(unused)]

use std::future::Future;

use bb8::PooledConnection;
use gcp_bigquery_client::{
    client_builder::ClientBuilder, error::BQError, model::query_request::QueryRequest, Client,
};

use crate::{
    datasource_config::BigQueryDbConfig,
    db::{
        BackendKind, CheckConn, ExportFrame, ExportKind, ExportQueryResult, Field, GetBackendKind,
        ListTable, MetaValue, QueryEventEmitter, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait,
        ServerMetas, ShowTableParameter, SqlBuilder, POOL_MAX_CONNECTIONS,
    },
    language_server::GetKeyWords,
};

#[derive(Clone)]
pub struct BigQueryDb {
    pub(crate) config: BigQueryDbConfig,
    pool: bb8::Pool<ConnManager>,
    pub(crate) project_id: String,
}

impl BigQueryDb {
    pub async fn new(config: BigQueryDbConfig) -> anyhow::Result<Self> {
        let mut builder = bb8::Pool::builder();
        builder = builder.max_size(POOL_MAX_CONNECTIONS);
        builder = builder.min_idle(Some(2));
        if let Some(timeout) = config.conn_timeout {
            builder = builder.connection_timeout(timeout.into());
        }
        let pool = builder
            .build(ConnManager {
                config: config.clone(),
            })
            .await?;

        let project_id: String = {
            let content = tokio::fs::read_to_string(&config.credentials_file).await?;
            let r: serde_json::Value = serde_json::from_str(&content)?;
            match r {
                serde_json::Value::Object(x) => match x.get("project_id") {
                    Some(x) => match x {
                        serde_json::Value::String(x) => x.clone(),
                        _ => {
                            anyhow::bail!("projet_id should be string")
                        }
                    },
                    None => anyhow::bail!("project_id not found in server account key file"),
                },
                _ => {
                    anyhow::bail!("server account key file should object")
                }
            }
        };

        anyhow::Ok(Self {
            config,
            pool,
            project_id,
        })
    }
}

#[async_trait::async_trait]

impl GetKeyWords for BigQueryDb {
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        self.backend_name().not_support_feature()
    }
}

impl GetBackendKind for BigQueryDb {
    fn backend_name(&self) -> BackendKind {
        BackendKind::BigQuery
    }
}

impl ReadOnlyTrait for BigQueryDb {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

#[async_trait::async_trait]

impl QuerySql for BigQueryDb {
    async fn query_sql(
        &self,
        query_id: &str,
        _sql: &str,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn execute_sql(
        &self,
        query_id: &str,
        _sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        todo!()
    }

    async fn execute_sql_no_open_conn(&self, sql: &str) -> anyhow::Result<Option<usize>> {
        todo!()
    }

    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        todo!()
    }

    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        todo!()
    }
}

#[async_trait::async_trait]
impl ServerMetas for BigQueryDb {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        todo!()
    }
}

impl std::fmt::Debug for BigQueryDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

#[async_trait::async_trait]
impl CheckConn for BigQueryDb {
    async fn check_conn(&self) -> anyhow::Result<std::time::Duration> {
        let conn = self.get_conn().await?;
        let job = conn.job();
        let now = std::time::Instant::now();
        job.query(&self.project_id, QueryRequest::new("SELECT 1"))
            .await?;
        anyhow::Ok(now.elapsed())
    }
}

impl BigQueryDb {
    pub(crate) async fn get_conn(&self) -> anyhow::Result<PooledConnection<'static, ConnManager>> {
        let conn = self.pool.get_owned().await?;
        anyhow::Ok(conn)
    }
}

impl SqlBuilder for BigQueryDb {
    fn get_schema(&self) -> Option<&str> {
        todo!()
    }

    fn put_value_in_sql(&self, _typ: &str, _value: Option<&str>) -> anyhow::Result<String> {
        todo!()
    }
}

#[async_trait::async_trait]

impl SchemaTrait for BigQueryDb {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        todo!()
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        todo!()
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        todo!()
    }

    async fn show_table(
        &self,
        table_name: &str,
        _: ShowTableParameter,
        _: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<String> {
        todo!()
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<String> {
        todo!()
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        todo!()
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        todo!()
    }
}

#[async_trait::async_trait]

impl ExportQueryResult for BigQueryDb {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        todo!()
    }

    async fn export_query_result(
        &self,
        query_id: Option<&str>,
        _sql: &str,
        _kind: ExportKind,
        _sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        todo!()
    }
}

#[cfg(test)]
async fn make_test_object() -> BigQueryDb {
    BigQueryDb::new(BigQueryDbConfig {
        dataset_id: None,
        credentials_file: "/Users/yuyang/Documents/bigquery/bigquery-cl-9713252fc1a5.json".into(),
        read_only: false,
        conn_timeout: None,
    })
    .await
    .unwrap()
}

pub struct ConnManager {
    config: BigQueryDbConfig,
}

impl bb8::ManageConnection for ConnManager {
    type Connection = Client;

    type Error = BQError;

    fn connect(&self) -> impl Future<Output = Result<Self::Connection, Self::Error>> + Send {
        async {
            let build = ClientBuilder::new();
            let conn = build
                .build_from_service_account_key_file(&self.config.credentials_file)
                .await?;
            Ok(conn)
        }
    }

    fn is_valid(
        &self,
        conn: &mut Self::Connection,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async {
            // TOOD maybe more check???
            Ok(())
        }
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {

    use crate::enable_test_log;

    use super::*;

    #[tokio::test]
    async fn test_check_conn() {
        enable_test_log();
        // export https_proxy=http://127.0.0.1:7897 http_proxy=http://127.0.0.1:7897 all_proxy=socks5://127.0.0.1:7897
        std::env::set_var("https_proxy", "http://127.0.0.1:7897");
        std::env::set_var("http_proxy", "http://127.0.0.1:7897");
        std::env::set_var("all_proxy", "socks5://127.0.0.1:7897");
        let db = make_test_object().await;
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }

    #[tokio::test]

    async fn test_xxx() {
        let c = gcp_bigquery_client::Client::from_service_account_key_file(
            "/Users/yuyang/Documents/bigquery/bigquery-cl-9713252fc1a5.json",
        )
        .await
        .unwrap();
    }
}
