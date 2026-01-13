#![allow(unused)]

use std::sync::Arc;

use couchbase::{
    authenticator::{Authenticator, PasswordAuthenticator},
    bucket::Bucket,
    cluster::Cluster,
    management::buckets::bucket_settings::BucketSettings,
    options::cluster_options::ClusterOptions,
};
use futures_util::StreamExt;

use crate::{
    datasource_config::CouchBaseDBConfig,
    db::{
        BackendKind, CatalogTrait, CheckConn, DBTrait, ExportQueryResult, Field, GetBackendKind,
        QueryEventEmitter, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas,
        SqlBuilder, not_support_feature,
    },
};

#[derive(Clone)]
pub struct CouchBaseDB {
    cluster: Arc<Cluster>,
    config: CouchBaseDBConfig,
}

impl GetBackendKind for CouchBaseDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::CouchBase
    }
}

#[async_trait::async_trait]

impl CheckConn for CouchBaseDB {
    async fn check_conn(&self) -> anyhow::Result<std::time::Duration> {
        let now = std::time::Instant::now();
        self.cluster.ping(None).await?;
        anyhow::Ok(now.elapsed())
    }
}

impl CouchBaseDB {
    pub async fn new(config: CouchBaseDBConfig) -> anyhow::Result<Self> {
        let cluster = Cluster::connect(
            &config.conn_str,
            ClusterOptions::new(Authenticator::PasswordAuthenticator(
                PasswordAuthenticator {
                    username: config.username.clone(),
                    password: config.password.clone(),
                },
            )),
        )
        .await?;
        anyhow::Ok(Self {
            cluster: Arc::new(cluster),
            config,
        })
    }
}

impl ReadOnlyTrait for CouchBaseDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

impl std::fmt::Debug for CouchBaseDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "couchbase@{:?}", self.config)
    }
}

#[async_trait::async_trait]

impl ServerMetas for CouchBaseDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<crate::db::MetaValue>> {
        todo!()
    }
}

#[async_trait::async_trait]

impl QuerySql for CouchBaseDB {
    async fn query_sql(
        &self, _query_id: &str, sql: &str, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn execute_sql(
        &self, query_id: &str, sql: &str,
    ) -> anyhow::Result<Result<crate::db::QueryResp, String>> {
        not_support_feature(self.backend_name())
    }

    async fn execute_sql_no_open_conn(&self, sql: &str) -> anyhow::Result<Option<usize>> {
        not_support_feature(self.backend_name())
    }

    async fn open_connection(&self, _id: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }
}

pub trait CouchBaseDBQueryEmit {
    fn emit(&self, _: serde_json::Value) -> anyhow::Result<()>;
}

impl CouchBaseDB {
    pub async fn query_sql(
        &self, sql: &str, emit: Arc<dyn CouchBaseDBQueryEmit>,
    ) -> anyhow::Result<()> {
        let mut query_result = self.cluster.query(sql, None).await?;
        let mut stream = query_result.rows::<'_, serde_json::Value>();
        while let Some(x) = stream.next().await {
            let x = x?;
            emit.emit(x)?;
        }
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]

impl SchemaTrait for CouchBaseDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        unimplemented!()
    }

    async fn list_table(&self) -> anyhow::Result<Vec<crate::db::ListTable>> {
        unimplemented!()
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        todo!()
    }

    async fn show_table(
        &self, table_name: &str, _: crate::db::ShowTableParameter, _: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
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
}

impl SqlBuilder for CouchBaseDB {
    fn get_schema(&self) -> Option<&str> {
        todo!()
    }

    fn put_value_in_sql(&self, _typ: &str, _value: Option<&str>) -> anyhow::Result<String> {
        todo!()
    }
}
#[async_trait::async_trait]

impl ExportQueryResult for CouchBaseDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self)) as Box<dyn ExportQueryResult>
    }

    async fn export_query_result(
        &self, query_id: Option<&str>, _sql: &str, _kind: crate::db::ExportKind,
        _sender: tokio::sync::mpsc::Sender<crate::db::ExportFrame>,
    ) -> anyhow::Result<()> {
        todo!()
    }
}

impl CouchBaseDB {
    pub(crate) fn get_bucket(&self) -> anyhow::Result<Bucket> {
        let name = match &self.config.bucket {
            Some(x) => x,
            None => anyhow::bail!("bucket name is required,this is interval server error"),
        };
        anyhow::Ok(self.cluster.bucket(name))
    }
}

#[async_trait::async_trait]

impl DBTrait for CouchBaseDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let r = self
            .get_bucket()?
            .collections()
            .get_all_scopes(None)
            .await?;
        anyhow::Ok(r.into_iter().map(|x| x.name().to_owned()).collect())
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut ret = Clone::clone(self);
        ret.config.scope = Some(name.to_owned());
        anyhow::Ok(Arc::new(ret))
    }

    async fn create_schema(
        &self, name: &str, extra: Option<crate::db::CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.get_bucket()?
            .collections()
            .create_scope(name, None)
            .await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.get_bucket()?
            .collections()
            .drop_scope(name, None)
            .await?;
        anyhow::Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait::async_trait]

impl CatalogTrait for CouchBaseDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        let r = self.cluster.buckets().get_all_buckets(None).await?;
        anyhow::Ok(r.into_iter().map(|x| x.name).collect())
    }

    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        let mut ret = Clone::clone(self);
        ret.config.bucket = Some(name.to_string());
        anyhow::Ok(Arc::new(ret))
    }

    async fn create_database(
        &self, name: &str, extra: Option<crate::db::CreateDatabaseExtra>,
    ) -> anyhow::Result<()> {
        self.cluster
            .buckets()
            .create_bucket(BucketSettings::new(name), None)
            .await?;
        anyhow::Ok(())
    }

    async fn drop_database(&self, name: &str) -> anyhow::Result<()> {
        self.cluster.buckets().drop_bucket(name, None).await?;
        anyhow::Ok(())
    }
}

#[cfg(test)]
async fn make_test_object() -> CouchBaseDB {
    CouchBaseDB::new(CouchBaseDBConfig {
        conn_str: "couchbase://localhost".into(),
        username: "Administrator".into(),
        password: "test1234".into(),
        read_only: false,
        bucket: None,
        scope: None,
    })
    .await
    .unwrap()
}

#[cfg(test)]

mod tests {
    use super::*;
    use couchbase::{
        authenticator::{Authenticator, PasswordAuthenticator},
        cluster::Cluster,
        options::cluster_options::ClusterOptions,
    };
    use futures_util::StreamExt;

    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object().await;
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_xxx() {
        let c = Cluster::connect(
            "couchbase://localhost",
            ClusterOptions::new(Authenticator::PasswordAuthenticator(
                PasswordAuthenticator {
                    username: "Administrator".to_string(),
                    password: "test1234".to_string(),
                },
            )),
        )
        .await
        .unwrap();
        let mut resp = c.query("select 1 ", None).await.unwrap();
        let mut stream = resp.rows::<'_, serde_json::Value>();
        while let Some(row) = stream.next().await {
            println!("!!:{:?}", row);
        }
    }
}
