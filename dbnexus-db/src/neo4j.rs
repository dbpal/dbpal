#![allow(unused)]

use std::sync::Arc;

use futures_util::StreamExt;
use neo4rs::{ConfigBuilder, Database, Graph, Query};

use crate::{
    datasource_config::Neo4JDbConfig,
    db::{
        BackendKind, CheckConn, DBTrait, ExportQueryResult, GetBackendKind, QuerySql,
        ReadOnlyTrait, SchemaTrait, ServerMetas, SqlBuilder,
    },
};

impl GetBackendKind for Neo4JDb {
    fn backend_name(&self) -> BackendKind {
        BackendKind::Neo4j
    }
}

#[async_trait::async_trait]

impl ServerMetas for Neo4JDb {
    async fn show_metas(&self) -> anyhow::Result<Vec<crate::db::MetaValue>> {
        todo!()
    }
}

#[async_trait::async_trait]

impl QuerySql for Neo4JDb {
    async fn query_sql(
        &self,
        query_id: &str,
        _sql: &str,
        emit: crate::db::QueryEventEmitter,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn execute_sql(
        &self,
        query_id: &str,
        _sql: &str,
    ) -> anyhow::Result<Result<crate::db::QueryResp, String>> {
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

impl std::fmt::Debug for Neo4JDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Neo4JDb@{:?}", self.config)
    }
}

#[async_trait::async_trait]

impl ExportQueryResult for Neo4JDb {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        todo!()
    }

    async fn export_query_result(
        &self,
        query_id: Option<&str>,
        _sql: &str,
        _kind: crate::db::ExportKind,
        _sender: tokio::sync::mpsc::Sender<crate::db::ExportFrame>,
    ) -> anyhow::Result<()> {
        todo!()
    }
}

pub struct Neo4JDb {
    pub config: Neo4JDbConfig,
    grpah: Graph,
}

impl ReadOnlyTrait for Neo4JDb {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

impl Neo4JDb {
    pub async fn new(config: Neo4JDbConfig) -> anyhow::Result<Self> {
        let mut cb = ConfigBuilder::new();
        cb = cb.uri(&config.uri);
        if !config.username.is_empty() {
            cb = cb.user(&config.username);
        }
        if !config.password.is_empty() {
            cb = cb.password(&config.password);
        }
        if let Some(db) = config.db.as_ref() {
            cb = cb.db(Database::from(db.as_str()));
        }
        if let Some(x) = config.client_certificate.as_ref() {
            cb = cb.with_client_certificate(x.clone());
        }

        let c = cb.build()?;
        let grpah = Graph::connect(c).await?;

        anyhow::Ok(Self { grpah, config })
    }
}

#[async_trait::async_trait]

impl CheckConn for Neo4JDb {
    async fn check_conn(&self) -> anyhow::Result<std::time::Duration> {
        let now = std::time::Instant::now();
        let sql = "RETURN 1 AS result;";
        self.grpah.execute(Query::new(sql.into())).await?;
        anyhow::Ok(now.elapsed())
    }
}

#[async_trait::async_trait]

impl DBTrait for Neo4JDb {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let sql = "SHOW DATABASES";
        let mut stream = self.grpah.execute(Query::new(sql.to_string())).await?;
        let mut ret = vec![];
        while let Some(r) = stream.next().await? {
            let name: String = r.get("name")?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn open_schema(
        &self,
        name: &str,
    ) -> anyhow::Result<std::sync::Arc<dyn crate::db::SchemaTrait>> {
        let mut config = self.config.clone();
        config.db = Some(name.to_owned());
        let ret = Self::new(config).await?;
        anyhow::Ok(Arc::new(ret))
    }

    async fn create_schema(
        &self,
        name: &str,
        extra: Option<crate::db::CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        let sql = format!("create database {name}");
        self.grpah.run(Query::new(sql)).await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!("drop database {name}");
        self.grpah.run(Query::new(sql)).await?;
        anyhow::Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait::async_trait]

impl SqlBuilder for Neo4JDb {
    fn get_schema(&self) -> Option<&str> {
        self.config.db.as_deref()
    }

    fn put_value_in_sql(&self, _typ: &str, _value: Option<&str>) -> anyhow::Result<String> {
        self.backend_name().not_support_feature()
    }
}

#[async_trait::async_trait]

impl SchemaTrait for Neo4JDb {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        todo!()
    }

    async fn list_table(&self) -> anyhow::Result<Vec<crate::db::ListTable>> {
        todo!()
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<crate::db::Field>> {
        todo!()
    }

    async fn show_table(
        &self,
        table_name: &str,
        _: crate::db::ShowTableParameter,
        _: crate::db::QueryEventEmitter,
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

#[cfg(test)]
pub(crate) async fn make_test_object() -> Neo4JDb {
    Neo4JDb::new(Neo4JDbConfig {
        uri: "127.0.0.1:7687".into(),
        username: "neo4j".into(),
        password: "test1234".into(),
        read_only: false,
        db: Some("neo4j".into()),
        client_certificate: None,
    })
    .await
    .unwrap()
}

#[cfg(test)]
mod tests {

    use super::*;

    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object().await;
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }

    #[tokio::test]

    async fn test_show_schema() {
        let db = make_test_object().await;

        let ss = db.list_schema().await.unwrap();
        for s in ss.iter() {
            println!("!!!!!!!!!!:{}", s);
        }
    }
}
