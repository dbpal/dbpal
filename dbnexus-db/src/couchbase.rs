// couchbase only allow use query on schema so we can get scope.
// docker run -d --name some-couchbase -p 8091-8094:8091-8094 -p 11210:11210 couchbase
use std::{collections::HashMap, str::FromStr, sync::Arc, time::Duration};

use couchbase::{
    authenticator::{Authenticator, PasswordAuthenticator},
    bucket::Bucket,
    cluster::Cluster,
    management::buckets::bucket_settings::{BucketSettings, BucketType},
    options::{cluster_options::ClusterOptions, query_options::QueryOptions},
    scope::Scope,
};
use futures_util::StreamExt;
use serde_json::Value;

use crate::{
    datasource_config::CouchBaseDBConfig,
    db::{
        BackendKind, CatalogTrait, CheckConn, CouchBaseBucketType, CreateDatabaseExtra, DBTrait,
        ExportKind, ExportQueryResult, Field, GetBackendKind, ListTable, MetaValue,
        QueryEventEmitter, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas,
        ShowTableParameter, SqlBuilder,
    },
};

#[derive(Clone)]
pub struct CouchBaseDB {
    cluster: Arc<Cluster>,
    pub(crate) config: CouchBaseDBConfig,
}

impl GetBackendKind for CouchBaseDB {
    fn backend_kind(&self) -> BackendKind {
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
        let cluster = tokio::time::timeout(
            config
                .conn_timeout
                .map(Duration::from)
                .unwrap_or(Duration::from_secs(60)),
            async {
                Cluster::connect(
                    &config.conn_str,
                    ClusterOptions::new(Authenticator::PasswordAuthenticator(
                        PasswordAuthenticator {
                            username: config.username.clone(),
                            password: config.password.clone(),
                        },
                    )),
                )
                .await
            },
        )
        .await??;
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
        anyhow::Ok(vec![
            MetaValue::new("version", x.version.to_string()),
            MetaValue::new("id", x.id.to_string()),
            MetaValue::new("sdk", x.sdk.to_string()),
        ])
    }
}

#[async_trait::async_trait]

impl QuerySql for CouchBaseDB {
    async fn query_sql(
        &self,
        _query_id: &str,
        sql: &str,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let scope = self.get_scope()?;
        let mut query_result = scope
            .query(sql, {
                let op = QueryOptions::default();
                let op = op.read_only(self.config.read_only);
                Some(op)
            })
            .await?;
        let mut stream = query_result.rows::<'_, serde_json::Value>();
        while let Some(x) = stream.next().await {
            let x = x?;
            let s = serde_json::to_string_pretty(&x)?;
            emit.send(vec![Some(s)].into())?;
        }
        anyhow::Ok(())
    }

    async fn execute_sql(
        &self,
        _query_id: &str,
        _sql: &str,
    ) -> anyhow::Result<Result<crate::db::QueryResp, String>> {
        self.backend_kind().not_support_feature()
    }

    async fn simple_execute_sql(&self, _sql: &str) -> anyhow::Result<Option<usize>> {
        self.backend_kind().not_support_feature()
    }

    async fn open_connection(&self, _id: &str) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature()
    }

    async fn close_connection(&self, _id: &str) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature()
    }
}

pub trait CouchBaseDBQueryEmit {
    fn emit(&self, _: serde_json::Value) -> anyhow::Result<()>;
}

fn extract_id_and_value(
    values: &HashMap<String, Option<String>>,
) -> anyhow::Result<(&String, serde_json::Value)> {
    // accept two parameter id and value
    let id = match values.get("id") {
        Some(Some(x)) => x,
        _ => anyhow::bail!("id is required"),
    };
    let value = match values.get("value") {
        Some(Some(x)) => serde_json::Value::from_str(x)?,
        _ => anyhow::bail!("value is required"),
    };
    anyhow::Ok((id, value))
}

impl CouchBaseDB {
    pub(crate) async fn query_count(
        &self,
        table: &str,
        where_str: Option<&str>,
    ) -> anyhow::Result<usize> {
        let mut sql = format!("SELECT COUNT(*) as count FROM `{}`", table);
        if let Some(x) = where_str {
            sql.push_str(format!(" where {}", x).as_str());
        }
        let s = self.get_scope()?;
        let mut resp = s.query(sql, None).await?;
        let mut rows = resp.rows::<'_, Value>();
        #[allow(clippy::never_loop)]
        while let Some(x) = rows.next().await {
            let x = x?;
            match x {
                Value::Object(x) => match x.get("count") {
                    Some(Value::Number(x)) => {
                        return anyhow::Ok(x.as_u64().unwrap_or_default() as usize);
                    }
                    _ => {
                        anyhow::bail!("should have count field")
                    }
                },
                _ => {
                    anyhow::bail!("should be a object")
                }
            };
        }
        anyhow::Ok(0)
    }
}

impl CouchBaseDB {
    pub(crate) async fn list_table_impl(&self, scope: &str) -> anyhow::Result<Vec<String>> {
        let b = self.get_bucket()?;
        for s in b.collections().get_all_scopes(None).await? {
            if s.name() == scope {
                return anyhow::Ok(
                    s.collections()
                        .iter()
                        .map(|x| x.name().to_owned())
                        .collect(),
                );
            }
        }
        anyhow::bail!(format!(
            "scope specs for '{:?}' not found",
            self.config.scope
        ))
    }
}

#[async_trait::async_trait]

impl SchemaTrait for CouchBaseDB {
    async fn export_table_or_view(
        &self,
        name: &str,
        para: Option<ShowTableParameter>,
        kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<crate::db::ExportFrame>,
    ) -> anyhow::Result<()> {
        let sql = self.gen_show_sql(name, None, para.as_ref());
        let mut resp = self.get_scope()?.query(sql, None).await?;
        let mut rows = resp.rows::<'_, serde_json::Value>();

        while let Some(row) = rows.next().await {
            let row = row?;
            match kind {
                ExportKind::Json { .. } | ExportKind::JsonLine => {
                    sender.send(row.into()).await?;
                }
                _ => {
                    anyhow::bail!(format!("not support export kind {:?}", kind))
                }
            }
        }
        anyhow::Ok(())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn table_insert(
        &self,
        table_name: &str,
        values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        self.can_write()?;

        let scope = self.get_scope()?;
        let (id, value) = extract_id_and_value(&values)?;
        scope.collection(table_name).insert(id, value, None).await?;
        anyhow::Ok(None)
    }

    async fn table_update(
        &self,
        table_name: &str,
        values: HashMap<String, Option<String>>,
        _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        self.can_write()?;
        let (id, value) = extract_id_and_value(&values)?;
        let scope = self.get_scope()?;
        let col = scope.collection(table_name);
        col.replace(id, value, None).await?;
        anyhow::Ok(Some(1))
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let bucket = self.get_bucket()?;

        if self.table_exists(dup).await? {
            // todo truncate table???
            self.truncate_table(dup).await?;
        } else {
            bucket
                .collections()
                .create_collection(self.config.scope_str(), dup, None, None)
                .await?;
        }

        let s = self.get_scope()?;
        let mut resp = s
            .query(format!("select META().id,* from `{table}`"), None)
            .await?;
        let mut rows = resp.rows::<'_, serde_json::Map<String, Value>>();

        while let Some(value) = rows.next().await {
            let value = value?;
            let id = match value.get("id") {
                Some(Value::String(x)) => x,
                _ => {
                    anyhow::bail!("id should be string")
                }
            };
            s.collection(dup).insert(id, value.get(table), None).await?;
        }
        anyhow::Ok(())
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.config.scope_str())
            .await
            .map(|x| x.into_iter().map(ListTable::from).collect())
    }

    async fn table_delete(
        &self,
        table_name: &str,
        mut wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        self.can_write()?;
        let id = wheres.remove("id").unwrap_or_default().unwrap_or_default();
        let scope = self.get_scope()?;
        scope.collection(table_name).remove(id, None).await?;
        anyhow::Ok(None)
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let sql = format!("select * from `{table_name}` limit 1");
        let s = self.get_scope()?;
        let mut resp = s.query(sql, None).await?;
        let mut rows = resp.rows::<'_, serde_json::Map<String, Value>>();
        #[allow(clippy::never_loop)]
        while let Some(row) = rows.next().await {
            let row = row?;
            let row = row.get(table_name).unwrap_or_default();
            match row {
                Value::Object(value) => {
                    return anyhow::Ok(
                        value.keys().map(|x| Field::from_name_type(x, "")).collect(),
                    );
                }
                _ => return anyhow::Ok(vec![]),
            }
        }
        anyhow::Ok(vec![])
    }

    async fn show_table(
        &self,
        table_name: &str,
        para: ShowTableParameter,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let sql = self.gen_show_sql(table_name, None, Some(&para));
        let total = self
            .query_count(table_name, para.where_str.as_deref())
            .await?;
        emit.send(
            QueryResp {
                sql: Some(sql.clone()),
                total,
                ..Default::default()
            }
            .into(),
        )?;
        let mut resp = self.get_scope()?.query(sql, None).await?;
        let mut rows = resp.rows::<'_, serde_json::Map<String, Value>>();
        while let Some(row) = rows.next().await {
            let row = row?;
            let id = match row.get("id") {
                Some(serde_json::Value::String(x)) => x.clone(),
                _ => anyhow::bail!(format!(
                    "should be able to find a id as string but:{:?}",
                    row
                )),
            };
            let data = serde_json::to_string_pretty(row.get(table_name).unwrap_or(&Value::Null))?;
            emit.send(vec![Some(id), Some(data)].into())?;
        }
        anyhow::Ok(())
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let s = self.get_scope()?;
        let sql = format!("DELETE FROM `{table_name}`");
        s.query(sql.clone(), None).await?;
        anyhow::Ok(sql.into())
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let s = self.get_scope()?;
        let sql = format!("DROP COLLECTION `{table}`;");
        s.query(sql.clone(), None).await?;
        anyhow::Ok(sql.into())
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
}

impl CouchBaseDB {
    pub async fn collection_delete_one(&self, table_name: &str, id: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let s = self.get_scope()?;
        s.collection(table_name).remove(id, None).await?;
        anyhow::Ok(())
    }
}

impl SqlBuilder for CouchBaseDB {
    fn get_schema(&self) -> Option<&str> {
        self.config.scope.as_deref()
    }

    fn put_value_in_sql(&self, _typ: &str, _value: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }

    fn gen_show_sql(
        &self,
        table_name: &str,
        _fields: Option<&[Field]>,
        para: Option<&ShowTableParameter>,
    ) -> String {
        let mut sql = format!("SELECT META().id, * FROM `{table_name}`");

        if let Some(para) = para {
            let ShowTableParameter {
                where_str,
                order_by,
                page,
                page_size,
                ..
            } = &para;
            if let Some(w) = where_str {
                sql.push_str(format!(" where {}", w).as_str());
            }
            if let Some(o) = order_by {
                sql.push_str(format!(" ORDER BY {}", o).as_str());
            }
            sql.push_str(
                format!(" LIMIT {} OFFSET {}", page_size, (page - 1) * page_size).as_str(),
            );
        }
        sql
    }
}
#[async_trait::async_trait]

impl ExportQueryResult for CouchBaseDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }

    async fn export_query_result(
        &self,
        _query_id: Option<&str>,
        sql: &str,
        kind: ExportKind,
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
        println!("about to return:{:?}", r);
        anyhow::Ok(r.into_iter().map(|x| x.name().to_owned()).collect())
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut ret = Clone::clone(self);
        ret.config.scope = Some(name.to_owned());
        anyhow::Ok(Arc::new(ret))
    }

    async fn create_schema(
        &self,
        name: &str,
        _extra: Option<crate::db::CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.can_write()?;
        self.get_bucket()?
            .collections()
            .create_scope(name, None)
            .await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
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
        ret.config.bucket = Some(name.to_owned());
        anyhow::Ok(Arc::new(ret))
    }

    async fn create_database(
        &self,
        name: &str,
        extra: Option<CreateDatabaseExtra>,
    ) -> anyhow::Result<()> {
        self.can_write()?;
        let setting = match extra {
            Some(CreateDatabaseExtra::Couchbase {
                ram_quota_mb,
                flush_enabled,
                num_replicas,
                max_expiry,
                bucket_type,
            }) => {
                let mut setting = BucketSettings::new(name);
                setting = setting.ram_quota_mb(ram_quota_mb);
                setting = setting.flush_enabled(flush_enabled);
                setting = setting.num_replicas(num_replicas);
                if let Some(max_expiry) = max_expiry {
                    setting = setting.max_expiry(Duration::from_secs(max_expiry));
                }
                let ty = match bucket_type {
                    CouchBaseBucketType::Couchbase => BucketType::COUCHBASE,
                    CouchBaseBucketType::Ephemeral => BucketType::EPHEMERAL,
                };
                setting = setting.bucket_type(ty);
                setting
            }
            _ => {
                anyhow::bail!(format!(
                    "couch base create bucket need extra parameter:{:?}",
                    extra
                ))
            }
        };
        self.cluster.buckets().create_bucket(setting, None).await?;
        anyhow::Ok(())
    }

    async fn drop_database(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        self.cluster.buckets().drop_bucket(name, None).await?;
        anyhow::Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// http://localhost:8091/ui/index.html 通过这个初始化
#[cfg(test)]
async fn make_test_object() -> CouchBaseDB {
    CouchBaseDB::new(CouchBaseDBConfig {
        conn_str: "couchbase://127.0.0.1".into(),
        username: "Administrator".into(),
        password: "test1234".into(),
        read_only: false,
        bucket: Some("test".to_string()),
        scope: Some("xxx".to_string()),
        conn_timeout: Some(4000.into()),
    })
    .await
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enable_test_log;
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

    async fn test_db() {
        enable_test_log();
        let db = make_test_object().await;
        let bs = db.list_schema().await.unwrap();
        println!("scopes:{:?}", bs);
    }

    #[tokio::test]

    async fn test_catalog() {
        enable_test_log();
        let db = make_test_object().await;
        let bs = db.list_database().await.unwrap();
        println!("buckets:{:?}", bs);
    }

    #[tokio::test]

    async fn test_duplicate() {
        let db = make_test_object().await;
        let table_name = "test_duplicate";
        let table_name_dup = "test_duplicate_dup";

        {
            let bucket = db.get_bucket().unwrap();
            let _ = bucket
                .collections()
                .create_collection(db.config.scope.clone().unwrap(), table_name, None, None)
                .await;
            let _ = db.truncate_table(table_name).await;
            let _ = db.truncate_table(table_name_dup).await;
        }
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

        db.duplicate_table(table_name, table_name_dup)
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
        // 不能执行多条语句呢
        let mut resp = c.query("select 1", None).await.unwrap();
        let mut stream = resp.rows::<'_, serde_json::Value>();
        while let Some(row) = stream.next().await {
            println!("!!:{:?}", row);
        }
    }

    #[tokio::test]

    async fn test_large_table() {
        let table_name = "test_large_table";
        let db = make_test_object().await;
        if db.table_exists(table_name).await.unwrap() {
            db.truncate_table(table_name).await.unwrap();
        } else {
            db.get_bucket()
                .unwrap()
                .collections()
                .create_collection(db.config.scope_str(), table_name, None, None)
                .await
                .unwrap();
        }

        for i in 0..155 {
            db.table_insert(table_name, {
                let mut values = HashMap::new();
                values.insert("id".to_string(), Some(format!("id_{}", i)));

                values.insert("value".to_owned(), {
                    let mut m = serde_json::Map::new();
                    m.insert(
                        "data".to_string(),
                        serde_json::Value::from(format!("hello world.{}", i)),
                    );
                    Some(serde_json::to_string(&m).unwrap())
                });

                values
            })
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn test_show_table() {
        let table_name = "test_show_table";
        let db = make_test_object().await;
        let _ = db
            .get_bucket()
            .unwrap()
            .collections()
            .create_collection(db.config.scope_str(), table_name, None, None)
            .await;
        db.truncate_table(table_name).await.unwrap();
        let id = "aaaa";
        db.table_insert(table_name, {
            let mut values = HashMap::new();
            values.insert("id".to_string(), Some(id.to_owned()));
            values.insert("value".to_owned(), {
                let mut m = serde_json::Map::new();
                m.insert("data".to_string(), serde_json::Value::from("hello world."));
                m.insert("id".into(), "abc".into());
                Some(serde_json::to_string(&m).unwrap())
            });
            values
        })
        .await
        .unwrap();
        let resp = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }
}
