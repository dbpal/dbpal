// couchbase only allow use query on schema so we can get scope.
// docker run -d --name some-couchbase -p 8091-8094:8091-8094 -p 11210:11210 couchbase
use couchbase::{
    authenticator::{Authenticator, PasswordAuthenticator},
    bucket::Bucket,
    cluster::Cluster,
    management::buckets::bucket_settings::{BucketSettings, BucketType},
    options::{
        cluster_options::{ClusterOptions, TlsOptions},
        query_options::QueryOptions,
    },
    scope::Scope,
};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::{ser::PrettyFormatter, Serializer, Value};
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    sync::Arc,
    time::Duration,
};
use tokio::io::AsyncBufReadExt;

use crate::{
    datasource_config::CouchBaseDBConfig,
    db::{
        BackendKind, CatalogTrait, CheckConn, CouchBaseBucketType, CreateDatabaseArg, DBTrait,
        EncodeXlsxDatetime, ExportKind, ExportQueryResult, ExtractSqlValeFromJsonValue, Field,
        GetBackendKind, ListTable, MetaValue, QueryEventEmitter, QueryResp, QuerySql,
        ReadOnlyTrait, SchemaTrait, ServerMetas, ShowTableParameter, SqlBuilder, TableImportPara,
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
        let mut op = ClusterOptions::new(Authenticator::PasswordAuthenticator(
            PasswordAuthenticator {
                username: config.username.clone(),
                password: config.password.clone(),
            },
        ));
        if config.ssl_enable {
            let mut tls = TlsOptions::new();
            tls.danger_accept_invalid_certs = config.danger_accept_invalid_certificates;
            tls.danger_accept_invalid_hostnames = config.danger_accept_invalid_hostnames;
            for file in config.ca_certificates.iter() {
                tls = tls.add_ca_certificate(
                    tokio_native_tls::native_tls::Certificate::from_pem(
                        tokio::fs::read(file).await?.as_slice(),
                    )
                    .map_err(|e| {
                        anyhow::Error::msg(format!(
                            "'{}' not valid certificate file,err:{}",
                            file, e
                        ))
                    })?,
                );
            }
            op = op.tls_options(tls);
        }
        let cluster = Cluster::connect(&config.conn_str(), op).await?;
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

impl CouchBaseDB {
    pub async fn ai_query_sql(
        &self,
        sql: &str,
        max_row: usize,
    ) -> anyhow::Result<(Vec<serde_json::Value>, bool)> {
        let scope = self.get_scope()?;
        let mut ret = Vec::with_capacity(max_row + 1); // a extra position to check if exceed
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
            ret.push(x);
            if ret.len() > max_row {
                return anyhow::Ok((ret, true));
            }
        }
        anyhow::Ok((ret, false))
    }
}

#[async_trait::async_trait]

impl QuerySql for CouchBaseDB {
    fn clone(&self) -> Box<dyn QuerySql> {
        Box::new(Clone::clone(self))
    }
    async fn get_update_sql_from_result_set(
        &self,
        _sql: &str,
        _old_values: &[Option<String>],
        _new_values: &[Option<String>],
    ) -> anyhow::Result<Vec<String>> {
        self.backend_kind()
            .not_support_feature("get update sql form result set")
    }
    fn support_tx(&self) -> bool {
        false
    }
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
        self.backend_kind().not_support_feature("execute sql")
    }

    async fn simple_execute_sql(&self, _sql: &str) -> anyhow::Result<Option<usize>> {
        self.backend_kind()
            .not_support_feature("simple execute sql")
    }

    async fn open_connection(&self, _id: &str) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature("open connetion")
    }

    async fn close_connection(&self, _id: &str) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature("close connection")
    }
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

impl ExtractSqlValeFromJsonValue for CouchBaseDB {
    fn extrat_sql_value_from_json_value(
        &self,
        _value: &serde_json::Value,
        _ty: &str,
    ) -> anyhow::Result<Option<String>> {
        anyhow::bail!("couchbase don't need impl this")
    }
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

impl EncodeXlsxDatetime for CouchBaseDB {}

#[async_trait::async_trait]

impl SchemaTrait for CouchBaseDB {
    async fn table_import_data_from_file(
        &self,
        table: &str,
        para: TableImportPara,
    ) -> anyhow::Result<()> {
        /*
        {
            "id": "aaaa",
            "test_show_table": {
                "data": "hello world.",
                "id": "abc",
                "b": 1
            }
        }
        extrat a value like this.
        */
        fn extract_id_and_table_value(
            map: &serde_json::Map<String, serde_json::Value>,
        ) -> anyhow::Result<(&String, &serde_json::Value)> {
            if map.len() != 2 {
                anyhow::bail!("this json value should contains 2 keys")
            }
            let id = "id";
            let id_value = match map.get(id) {
                Some(value) => match value {
                    serde_json::Value::String(x) => x,
                    _ => {
                        anyhow::bail!("the id should have value string.")
                    }
                },
                None => anyhow::bail!("this value should contains id"),
            };

            let value_key = {
                let mut keys = map.keys().collect::<HashSet<_>>();
                keys.remove(&id.to_string());
                let keys = keys.into_iter().collect::<Vec<_>>();
                keys[0]
            };
            let table_value = match map.get(value_key) {
                Some(x) => x,
                None => unreachable!(),
            };
            anyhow::Ok((id_value, table_value))
        }

        let scope = self.get_scope()?;

        // try create table is not exists.

        if !self.table_exists(table).await? {
            let sql = format!("CREATE COLLECTION `{}`", table);
            scope.query(sql, None).await?;
        }

        let path_lower = para.path.to_lowercase();
        if path_lower.ends_with(".json") {
            let path = para.path.clone();

            let values = tokio::task::spawn_blocking(move || {
                let f = std::fs::File::open(&path)?;
                let buf = std::io::BufReader::new(f);
                let ret: Vec<serde_json::Map<String, serde_json::Value>> =
                    serde_json::from_reader(buf)?;
                anyhow::Ok(ret)
            })
            .await??;

            for v in values.iter() {
                let (id, value) = extract_id_and_table_value(v)?;
                scope.collection(table).insert(id, value, None).await?;
            }
            anyhow::Ok(())
        } else if path_lower.contains(".jsonl") {
            let f = tokio::fs::File::open(&para.path).await?;
            let reader = tokio::io::BufReader::new(f);
            let mut lines = reader.lines();
            while let Some(line) = lines.next_line().await? {
                if line.trim().is_empty() {
                    continue;
                }
                let value = serde_json::from_str(&line)?;
                let (id, value) = extract_id_and_table_value(&value)?;
                scope.collection(table).insert(id, value, None).await?;
            }
            anyhow::Ok(())
        } else {
            anyhow::bail!(format!("unsupported file type:{}", para.path))
        }
    }
    async fn export_table_or_view(
        &self,
        name: &str,
        para: Option<ShowTableParameter>,
        kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<crate::db::ExportFrame>,
    ) -> anyhow::Result<()> {
        let sql = self.gen_show_sql(name, None, para.as_ref());
        let mut resp = self.get_scope()?.query(sql, None).await?;
        let mut rows = resp.rows::<'_, serde_json::Map<String, serde_json::Value>>();

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
        values: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        self.can_write()?;

        let scope = self.get_scope()?;
        let (id, value) = extract_id_and_value(values)?;
        scope.collection(table_name).insert(id, value, None).await?;
        anyhow::Ok(None)
    }

    async fn table_update(
        &self,
        table_name: &str,
        values: &HashMap<String, Option<String>>,
        _wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        self.can_write()?;
        let (id, value) = extract_id_and_value(values)?;
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
        wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        self.can_write()?;
        let id = wheres
            .clone()
            .remove("id")
            .expect("must have id filed")
            .clone()
            .expect("must have id");
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
        let total = self.query_count(table_name, para.filter.as_deref()).await?;
        emit.send(
            QueryResp {
                sql: Some(sql.clone()),
                total: total as f64,
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

            let indent = " ".repeat(para.tab_size as usize); // This could be b"\t" or any byte sequence.
            let formatter = PrettyFormatter::with_indent(indent.as_bytes());

            let mut buf = Vec::new();
            let mut serializer = Serializer::with_formatter(&mut buf, formatter);
            row.get(table_name)
                .unwrap_or(&Value::Null)
                .serialize(&mut serializer)?;

            emit.send(vec![Some(id), Some(String::from_utf8(buf)?)].into())?;
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

    fn encode_value(&self, _typ: &str, _value: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("put value in sql")
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
                filter: where_str,
                sort: order_by,
                ..
            } = &para;
            if let Some(w) = where_str {
                sql.push_str(format!(" where {}", w).as_str());
            }
            if let Some(o) = order_by {
                sql.push_str(format!(" ORDER BY {}", o).as_str());
            }
            if let Some(offset) = para.offset() {
                sql.push_str(
                    format!(" LIMIT {} OFFSET {}", offset.page_size, offset.offset).as_str(),
                );
            }
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
        _extra: Option<crate::db::CreateSchemaArg>,
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
        extra: Option<CreateDatabaseArg>,
    ) -> anyhow::Result<()> {
        self.can_write()?;
        let setting = match extra {
            Some(CreateDatabaseArg::Couchbase {
                ram_quota_mb,
                flush_enabled,
                num_replicas,
                max_expiry,
                bucket_type,
            }) => {
                let mut setting = BucketSettings::new(name);
                setting = setting.ram_quota_mb(ram_quota_mb as u64);
                setting = setting.flush_enabled(flush_enabled);
                setting = setting.num_replicas(num_replicas);
                if let Some(max_expiry) = max_expiry {
                    setting = setting.max_expiry(Duration::from_secs(max_expiry as u64));
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

// Initialize it through http://localhost:8091/ui/index.html.
#[cfg(test)]
async fn make_test_object() -> CouchBaseDB {
    CouchBaseDB::new(CouchBaseDBConfig {
        host: "127.0.0.1".into(),
        port: 11210,
        username: "Administrator".into(),
        password: "test1234".into(),
        read_only: false,
        bucket: Some("test".to_string()),
        scope: Some("xxx".to_string()),
        conn_timeout: Some(4000.into()),
        ssl_enable: false,
        danger_accept_invalid_hostnames: None,
        danger_accept_invalid_certificates: None,
        ca_certificates: vec![],
    })
    .await
    .unwrap()
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use super::*;
    use crate::{
        datasource_config::SSLFiles,
        enable_test_log,
        helper::{cert_files_dir, DockerContainerDropper},
    };
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

        let mut values = HashMap::new();
        values.insert("id".to_string(), Some(id.to_owned()));
        values.insert("value".to_owned(), {
            let mut m = serde_json::Map::new();
            m.insert("data".to_string(), serde_json::Value::from("hello world."));
            Some(serde_json::to_string(&m).unwrap())
        });
        db.table_insert(table_name, &values).await.unwrap();

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
        // Multiple statements cannot be executed.
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
            db.table_insert(table_name, &values).await.unwrap();
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
        let mut values = HashMap::new();
        values.insert("id".to_string(), Some(id.to_owned()));
        values.insert("value".to_owned(), {
            let mut m = serde_json::Map::new();
            m.insert("name".to_string(), serde_json::Value::from("john"));
            m.insert("age".into(), 18.into());
            Some(serde_json::to_string(&m).unwrap())
        });
        db.table_insert(table_name, &values).await.unwrap();
        let resp = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        let cols = db.show_table_columns(table_name).await.unwrap();
        for col in cols.iter() {
            println!("column:{:?}", col);
        }

        db.test_table_export_and_import_json(table_name).await;
        db.test_table_export_and_import_jsonline(table_name).await;

        let resp = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }

    #[tokio::test]
    async fn test_ssl_couchbase() {
        enable_test_log();
        let port = portpicker::pick_unused_port().unwrap();
        let port2 = portpicker::pick_unused_port().unwrap();
        println!("test port:{} port2:{}", port, port2);
        let cert_dir = cert_files_dir();

        let ca_volume = format!(
            "{}/ca.crt:/opt/couchbase/var/lib/couchbase/ca.pem",
            cert_dir
        );
        let cert_volume = format!(
            "{}/server.crt:/opt/couchbase/var/lib/couchbase/chain.pem",
            cert_dir
        );
        let key_volume = format!(
            "{}/server.key:/opt/couchbase/var/lib/couchbase/pkey.key",
            cert_dir
        );

        // Start the container (without setting environment variables).
        let output = Command::new("docker")
            .arg("run")
            .arg("-d")
            .args(["--name", "couchbase-ssl-test"])
            .args(["-p", format!("{}:8091", port).as_str()])
            .args(["-p", format!("{}:11207", port2).as_str()])
            .args(["-v", &ca_volume])
            .args(["-v", &cert_volume])
            .args(["-v", &key_volume])
            .arg("couchbase/server")
            .output()
            .unwrap();

        let container_id = if output.status.success() {
            let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("Couchbase 容器启动成功，ID 为: {}", container_id);
            container_id
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("启动 Docker 容器失败: {}", stderr);
            return;
        };
        let _guard = DockerContainerDropper::from(&container_id);

        // ===== Wait for Couchbase to start =====
        println!("等待 Couchbase 启动...");
        tokio::time::sleep(std::time::Duration::from_secs(20)).await;

        // ===== Manually initialize the cluster (set the password) =====
        let init_output = Command::new("docker")
            .arg("exec")
            .arg(&container_id)
            .arg("couchbase-cli")
            .arg("cluster-init")
            .arg("-c")
            .arg("localhost:8091")
            .arg("--cluster-username=Administrator")
            .arg("--cluster-password=test1234") // ← Set the password.
            .arg("--cluster-port=8091")
            .arg("--services=data,index,query,fts")
            .output()
            .unwrap();

        if !init_output.status.success() {
            eprintln!(
                "集群初始化失败: {}",
                String::from_utf8_lossy(&init_output.stderr)
            );
            return;
        }
        println!("✅ 集群初始化成功，密码已设置为: test1234");

        // ===== Create a bucket =====
        let bucket_output = Command::new("docker")
            .arg("exec")
            .arg(&container_id)
            .arg("couchbase-cli")
            .arg("bucket-create")
            .arg("-c")
            .arg(format!("localhost:{}", port))
            .arg("-u")
            .arg("Administrator")
            .arg("-p")
            .arg("test1234")
            .arg("--bucket=test")
            .arg("--bucket-type=couchbase")
            .arg("--bucket-ramsize=256")
            .output()
            .unwrap();

        if !bucket_output.status.success() {
            eprintln!(
                "Bucket 创建失败: {}",
                String::from_utf8_lossy(&bucket_output.stderr)
            );
            return;
        }
        println!("✅ Bucket 'test' 创建成功");

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        println!("trying to connect to server.",);

        let db = CouchBaseDB::new(CouchBaseDBConfig {
            host: "127.0.0.1".into(),
            port: port2,
            username: "Administrator".to_string(),
            password: "test1234".to_string(), // ← Use the configured password.
            read_only: false,
            bucket: "test".to_string().into(),
            scope: None,
            conn_timeout: Some(8000.into()),
            ssl_enable: true,
            danger_accept_invalid_hostnames: Some(true),
            danger_accept_invalid_certificates: Some(true),
            ca_certificates: vec![SSLFiles::test_cert().ssl_ca.unwrap()],
        })
        .await
        .unwrap();

        db.check_conn().await.unwrap();
        println!("✅ 连接测试成功！");
    }
}
