#![allow(unused)]

use gcp_bigquery_client::client_builder::ClientBuilder;

use crate::{
    datasource_config::BigQueryDbConfig,
    db::{
        BackendKind, CheckConn, ExportFrame, ExportKind, ExportQueryResult, Field, GetBackendKind,
        ListTable, MetaValue, QueryEventEmitter, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait,
        ServerMetas, ShowTableParameter, SqlExecuteRet, not_support_feature,
    },
    language_server::GetKeyWords,
};

#[derive(Clone)]
pub struct BigQueryDb {
    pub(crate) config: BigQueryDbConfig,
}

impl BigQueryDb {
    pub fn new(config: BigQueryDbConfig) -> anyhow::Result<Self> {
        let mut builder = ClientBuilder::new();
        builder.with_v2_base_url(config.base_url.clone());

        unimplemented!()
    }
}

#[async_trait::async_trait]

impl GetKeyWords for BigQueryDb {
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
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
        &self, query_id: &str, _sql: &str, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn execute_sql(
        &self, query_id: &str, _sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        todo!()
    }

    async fn execute_sql_no_open_conn(
        &self, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
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
        &self, table_name: &str, _: ShowTableParameter, _: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn table_insert(
        &self, table_name: &str, values: std::collections::HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        todo!()
    }

    async fn table_update(
        &self, table_name: &str, values: std::collections::HashMap<String, Option<String>>,
        wheres: std::collections::HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        todo!()
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        todo!()
    }

    async fn table_delete(
        &self, _table_name: &str, _wheres: std::collections::HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        todo!()
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        todo!()
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        todo!()
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        todo!()
    }

    async fn create_table_sql(
        &self, req: &crate::db::CreateTableReq,
    ) -> anyhow::Result<Vec<String>> {
        todo!()
    }
}

#[async_trait::async_trait]

impl ExportQueryResult for BigQueryDb {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        todo!()
    }

    async fn export_query_result(
        &self, query_id: Option<&str>, _sql: &str, _kind: ExportKind,
        _sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        todo!()
    }
}

#[cfg(test)]
fn make_test_object() -> BigQueryDb {
    unimplemented!()
}

#[cfg(test)]
mod tests {}
