#![allow(unused)]

// couchbase only allow use query on schema so we can get scope.
// docker run -d --name some-couchbase -p 8091-8094:8091-8094 -p 11210:11210 couchbase
use std::{collections::HashMap, str::FromStr, sync::Arc};

use couchbase::{
    authenticator::{Authenticator, PasswordAuthenticator},
    bucket::Bucket,
    cluster::Cluster,
    management::buckets::bucket_settings::BucketSettings,
    options::cluster_options::ClusterOptions,
    results::collections_mgmt_results::ScopeSpec,
    scope::Scope,
};
use futures_util::StreamExt;

use crate::{
    datasource_config::CouchBaseDBConfig,
    db::{
        BackendKind, CatalogTrait, CheckConn, DBTrait, ExportKind, ExportQueryResult, Field,
        GetBackendKind, MetaValue, QueryEventEmitter, QueryResp, QuerySql, ReadOnlyTrait,
        SchemaTrait, ServerMetas, SqlBuilder, not_support_feature,
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
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let x = self.cluster.diagnostics(None).await?;
        let mut ret = vec![];
        ret.push(MetaValue::new("version", x.version.to_string()));
        ret.push(MetaValue::new("id", x.id.to_string()));
        ret.push(MetaValue::new("sdk", x.sdk.to_string()));
        anyhow::Ok(ret)
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
        let scope = self.get_scope()?;
        let mut query_result = scope.query(sql, None).await?;
        let mut stream = query_result.rows::<'_, serde_json::Value>();
        while let Some(x) = stream.next().await {
            let x = x?;
            emit.emit(x)?;
        }
        anyhow::Ok(())
    }
}

fn extract_id_and_value(
    values: &HashMap<String, Option<String>>,
) -> anyhow::Result<(&String, serde_json::Value)> {
    // accept two parameter id and value
    let id = match values.get("id") {
        Some(x) => match x {
            Some(x) => x,
            None => anyhow::bail!("id is required"),
        },
        None => anyhow::bail!("id is required"),
    };
    let value = match values.get("id") {
        Some(x) => match x {
            Some(x) => serde_json::Value::from_str(x)?,
            None => anyhow::bail!("value is required"),
        },
        None => anyhow::bail!("value is required"),
    };
    anyhow::Ok((id, value))
}

#[async_trait::async_trait]

impl SchemaTrait for CouchBaseDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        let scope = self.get_scope()?;
        let (id, value) = extract_id_and_value(&values)?;
        println!("!!!!!!!!!!!!!!{} {}", id, value);
        scope.collection(table_name).insert(id, value, None).await?;
        anyhow::Ok(None)
    }

    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        let (id, value) = extract_id_and_value(&values)?;
        let scope = self.get_scope()?;
        let col = scope.collection(table_name);
        col.replace(id, value, None).await?;
        anyhow::Ok(Some(1))
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        unimplemented!()
    }

    async fn list_table(&self) -> anyhow::Result<Vec<crate::db::ListTable>> {
        let s = self.get_scope()?;
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
        let s = self.get_scope()?;
        let sql = format!("DELETE FROM `{table_name}`");
        s.query(sql, None).await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        let s = self.get_scope()?;
        let sql = format!("DROP COLLECTION `{table}`;");
        s.query(sql, None).await?;
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self)) as Box<dyn SchemaTrait>
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
}

impl CouchBaseDB {
    pub async fn collection_delete_one(&self, table_name: &str, id: &str) -> anyhow::Result<()> {
        let s = self.get_scope()?;
        s.collection(table_name).remove(id, None).await?;
        anyhow::Ok(())
    }
}

impl SqlBuilder for CouchBaseDB {
    fn get_schema(&self) -> Option<&str> {
        None
    }

    fn put_value_in_sql(&self, _typ: &str, _value: Option<&str>) -> anyhow::Result<String> {
        not_support_feature(self.backend_name())
    }
}
#[async_trait::async_trait]

impl ExportQueryResult for CouchBaseDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self)) as Box<dyn ExportQueryResult>
    }

    async fn export_query_result(
        &self, _query_id: Option<&str>, sql: &str, kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<crate::db::ExportFrame>,
    ) -> anyhow::Result<()> {
        let s = self.get_scope()?;
        let mut resp = s.query(sql, None).await?;
        let mut rows = resp.rows::<'_, serde_json::Value>();
        while let Some(x) = rows.next().await {
            let x = x?;
            match kind {
                ExportKind::Json { .. } | ExportKind::JsonLine => {
                    sender.send(x.into()).await?;
                }
                _ => {
                    anyhow::bail!(format!("couchbase not support export kind:{:?}", kind))
                }
            }
        }
        anyhow::Ok(())
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

    pub(crate) fn get_scope(&self) -> anyhow::Result<Scope> {
        let bucket = self.get_bucket()?;
        match &self.config.scope {
            Some(x) => anyhow::Ok(bucket.scope(x)),
            None => anyhow::bail!("need scope set But None."),
        }
    }
}

#[async_trait::async_trait]

impl DBTrait for CouchBaseDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        println!("trying to list schema");
        let bucket = self.get_bucket()?;
        println!("{:?}", bucket.name());
        let r = bucket.collections().get_all_scopes(None).await?;
        println!("about to return");
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
        let mut config = self.config.clone();
        config.bucket = Some(name.to_string());
        let ret = CouchBaseDB::new(config).await?;
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
        bucket: Some("test".to_string()),
        scope: Some("xxx".to_string()),
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
        let d = crate::db::CheckConn::check_conn(&db).await.unwrap();
        println!("ping time:{:?}", d);
    }

    #[tokio::test]
    async fn create_bucket_and_scope() {
        println!("make test object");
        let db = make_test_object().await;
        println!("trying to creteate bucket");
        let _ = db.create_database("test", None).await;
        println!("trying to create scope");
        let _ = db.create_schema("xxx", None).await;
        println!("create scope done");
    }

    #[tokio::test]

    async fn test_db() {
        let db = make_test_object().await;
        let bs = db.list_schema().await.unwrap();
        println!("scopes:{:?}", bs);
    }

    #[tokio::test]

    async fn test_catalog() {
        let db = make_test_object().await;
        let bs = db.list_database().await.unwrap();
        println!("buckets:{:?}", bs);
    }

    #[tokio::test]

    async fn test_duplicate() {
        let db = make_test_object().await;
        let table_name = "test_duplicate";
        let table_name_dup = "test_duplicate_dup";
        let _ = db.drop_table(table_name, false).await;
        let _ = db.drop_table(table_name_dup, false).await;

        let id = "aaaa";
        db.table_insert(table_name, {
            let mut values = HashMap::new();
            values.insert("id".to_string(), Some(id.to_owned()));
            values.insert("value".to_owned(), {
                let mut m = serde_json::Map::new();
                m.insert("data".to_string(), serde_json::Value::from("hello world."));
                Some(serde_json::to_string(&m).unwrap())
            });

            values
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_server_meta() {
        let db = make_test_object().await;
        let ms = db.show_metas().await.unwrap();
        for m in ms.iter() {
            println!("meta:{:?}", m);
        }
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
