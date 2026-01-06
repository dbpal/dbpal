use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bson::Bson;
use dashmap::DashSet;
use futures_util::StreamExt;
use mongodb::bson::{Document, doc};
use mongodb::options::{IndexOptions, ServerAddress, Tls, TlsOptions};
use mongodb::results::{CollectionType, DeleteResult, InsertOneResult, UpdateResult};
use mongodb::{IndexModel, bson};

#[cfg(test)]
use crate::TEST_HOST;
use crate::datasource_config::MongoDBConfig;
use crate::db::{
    BackendKind, CheckConn, CreateSchemaExtra, DBTrait, ExportFrame, ExportKind, ExportQueryResult,
    Field, GetBackendKind, Index, IndexKind, ListTable, MetaValue, OptionMustBeSomeErr,
    QueryEventEmitter, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas,
    ShowTableParameter, not_support_feature,
};

#[derive(Clone)]
pub struct MongoDB {
    client: mongodb::Client,
    db_name: Option<String>,
    read_only: bool,
    created_databases: Arc<DashSet<String>>,
}

impl std::fmt::Debug for MongoDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mssql on:{:?}", self.db_name)
    }
}

impl MongoDB {
    pub fn new(config: MongoDBConfig) -> anyhow::Result<Self> {
        let mut opts = mongodb::options::ClientOptions::default();
        opts.max_connecting = Some(100);
        opts.hosts = vec![ServerAddress::Tcp {
            host: config.host.to_string(),
            port: Some(config.port),
        }];

        opts.connect_timeout = config.conn_timeout.map(Duration::from);
        if let Some(user) = &config.username {
            let b = mongodb::options::Credential::builder();
            let b = b.username(Some(user.to_owned()));
            let b = b.password(Some(config.password.as_deref().unwrap_or("").to_owned()));
            let x = b.build();
            opts.credential = Some(x);
        }
        if config.ssl.enable_ssl {
            let mut tls_option = TlsOptions::default();
            if let Some(ca) = config.ssl.ssl_files.ssl_ca.as_ref() {
                tls_option.ca_file_path = Some(Path::new(ca).to_path_buf());
            }
            if let Some(key) = config.ssl.ssl_files.ssl_key.as_ref() {
                tls_option.cert_key_file_path = Some(Path::new(key).to_path_buf());
            }
            opts.tls = Some(Tls::Enabled(tls_option));
        }
        let client = mongodb::Client::with_options(opts)?;

        anyhow::Ok(Self {
            client,
            db_name: config.db.clone(),
            read_only: config.read_only,
            created_databases: Default::default(),
        })
    }
}

impl GetBackendKind for MongoDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::Mongo
    }
}

impl ReadOnlyTrait for MongoDB {
    fn read_only(&self) -> bool {
        self.read_only
    }
}

#[async_trait::async_trait]
impl ExportQueryResult for MongoDB {
    async fn export_query_result(
        &self, _query_id: Option<&str>, _sql: &str, _kind: ExportKind,
        _sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self)) as Box<dyn ExportQueryResult>
    }
}
#[async_trait::async_trait]
impl DBTrait for MongoDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let conn = self.get_conn().await?;
        let mut names = conn
            .list_database_names()
            .await?
            .into_iter()
            .collect::<HashSet<String>>();
        names.extend(<DashSet<std::string::String> as Clone>::clone(
            &self.created_databases,
        ));
        let mut names = names.into_iter().collect::<Vec<_>>();
        names.sort();
        anyhow::Ok(names)
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut x = Clone::clone(self);
        x.db_name = Some(name.to_string());
        return anyhow::Ok(Arc::new(x));
    }

    async fn create_schema(
        &self, name: &str, _extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.can_write()?;
        self.created_databases.insert(name.to_owned());
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let conn = self.get_conn().await?;
        conn.database(name).drop().await?;
        self.created_databases.remove(&name.to_owned());
        anyhow::Ok(())
    }
}

fn bson_value_to_string(b: &bson::Bson) -> anyhow::Result<Option<String>> {
    match b {
        Bson::Null | Bson::Undefined | Bson::MaxKey | Bson::MinKey => return anyhow::Ok(None),

        // Bson::String(x) => return anyhow::Ok(Some(x.to_owned())),
        _ => {}
    }
    let x = serde_json::to_string_pretty(b)?;
    anyhow::Ok(Some(x))
}

fn collect_table_resp(fs: &[Field], docs: &[Document]) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let mut ret = Vec::with_capacity(docs.len());
    for doc in docs.iter() {
        let mut values = vec![];
        for f in fs.iter() {
            let value = match doc.get(&f.field) {
                Some(x) => Some(bson_value_to_string(x)?),
                None => None,
            }
            .flatten();
            values.push(value);
        }
        ret.push(values);
    }

    anyhow::Ok(ret)
}

fn infer_multi_value_column_names(docs: &[Document]) -> anyhow::Result<Vec<Field>> {
    let mut ret: HashMap<String, Field> = HashMap::new();
    for doc in docs.iter() {
        for (k, v) in doc.iter() {
            if ret.contains_key(k) {
                continue;
            }
            let ty = match v {
                bson::Bson::Double(_) => "double",
                bson::Bson::String(_) => "string",
                bson::Bson::Array(_) => "array",
                bson::Bson::Document(_) => "document",
                bson::Bson::Boolean(_) => "bool",
                bson::Bson::Null => "null",
                bson::Bson::RegularExpression(_) => "regexp",
                bson::Bson::JavaScriptCode(_) => "jscode",
                bson::Bson::JavaScriptCodeWithScope(_) => "jscode",
                bson::Bson::Int32(_) => "int32",
                bson::Bson::Int64(_) => "int64",
                bson::Bson::Timestamp(_) => "timestamp",
                bson::Bson::Binary(_) => "binary",
                bson::Bson::ObjectId(_) => "object_id",
                bson::Bson::DateTime(_) => "datetime",
                bson::Bson::Symbol(_) => "symbol",
                bson::Bson::Decimal128(_) => "decimal128",
                bson::Bson::Undefined => "undefined",
                bson::Bson::MaxKey => "max_key",
                bson::Bson::MinKey => "min_key",
                // Deprecated
                bson::Bson::DbPointer(_) => "db_pointer",
            };
            ret.insert(k.clone(), Field::from_name_type(k.as_str(), ty));
        }
    }

    let mut ret: Vec<_> = ret.into_values().collect();
    let id_field = "_id";
    ret.sort_by(|a, b| {
        if a.field.as_str() == id_field {
            std::cmp::Ordering::Less
        } else if b.field.as_str() == id_field {
            std::cmp::Ordering::Greater
        } else {
            a.field.cmp(&b.field)
        }
    });
    anyhow::Ok(ret)
}

#[async_trait::async_trait]
impl CheckConn for MongoDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        let conn = self.get_conn().await?;
        let _ = conn.list_database_names().await?;
        anyhow::Ok(ins.elapsed())
    }
}

impl MongoDB {
    /// normal `insert` , `update` ,`delete` works very bad with mongodb
    /// So i just impl those method on  type`Document`
    pub async fn insert(
        &self, collection_name: &str, doc: Document,
    ) -> anyhow::Result<InsertOneResult> {
        let db = self.get_db().await?;
        let ret = db
            .collection::<Document>(collection_name)
            .insert_one(doc)
            .await?;
        anyhow::Ok(ret)
    }

    pub async fn create_index(
        &self, collection_name: &str, index_name: Option<String>, unique: bool, keys: Document,
    ) -> anyhow::Result<()> {
        let db = self.get_db().await?;
        let mut options = IndexOptions::default();
        options.unique = Some(unique);
        options.name = index_name;

        db.collection::<Document>(collection_name)
            .create_index(IndexModel::builder().keys(keys).options(options).build())
            .await?;
        anyhow::Ok(())
    }

    pub async fn get_one_document_by_id(
        &self, collection_name: &str, id: bson::Bson,
    ) -> anyhow::Result<Document> {
        let db = self.get_db().await?;
        if let Some(document) = db
            .collection(collection_name)
            .find_one(bson::doc! { "_id" :  id.clone() })
            .await?
        {
            anyhow::Ok(document)
        } else {
            anyhow::bail!(format!("uuid '{:?}' not found doc", id));
        }
    }

    pub async fn update_one(
        &self, collection_name: &str, mut doc: Document,
    ) -> anyhow::Result<UpdateResult> {
        self.can_write()?;
        let id = match doc.remove("_id") {
            Some(x) => x,
            None => anyhow::bail!("no _id provide,this should be a bug"),
        };
        let db = self.get_db().await?;
        let ret = db
            .collection::<Document>(collection_name)
            .replace_one(bson::doc! {"_id" : id }, doc)
            .await?;
        anyhow::Ok(ret)
    }

    pub async fn delete_by_ids(
        &self, collection_name: &str, ids: Vec<bson::Bson>,
    ) -> anyhow::Result<DeleteResult> {
        self.can_write()?;
        let db = self.get_db().await?;
        log::info!("ids:{:?}", bson::doc! {"_id" : { "$in" : ids.clone()  } });
        let ret = db
            .collection::<Document>(collection_name)
            .delete_many(bson::doc! {"_id" : { "$in" : ids } })
            .await?;

        anyhow::Ok(ret)
    }

    pub async fn show_collection_table_list(
        &self, collection_name: &str, filter: Document, sort: Document, page: u64, page_size: u64,
    ) -> anyhow::Result<QueryResp> {
        let db = self.get_db().await?;
        let count: u64 = db
            .collection::<Document>(collection_name)
            .count_documents(filter.clone())
            .await?;
        let mut stream = db
            .collection::<Document>(collection_name)
            .find(filter)
            .skip((page - 1) * page_size)
            .limit(page_size as i64)
            .sort(sort)
            .await?;

        let mut docs = Vec::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            docs.push(item);
        }
        let cols = infer_multi_value_column_names(&docs[..])?;
        let data = collect_table_resp(&cols, docs.as_slice())?;
        anyhow::Ok(QueryResp::new(cols, count as usize, data, None)?)
    }

    pub async fn show_collection_json(
        &self, collection_name: &str, filter: Document, sort: Document, page: u64, page_size: u64,
    ) -> anyhow::Result<DocsAndCount> {
        let db = self.get_db().await?;
        let count = db
            .collection::<u64>(collection_name)
            .count_documents(filter.clone())
            .await?;
        let mut stream = db
            .collection::<bson::Document>(collection_name)
            .find(filter)
            .skip((page - 1) * page_size)
            .limit(page_size as i64)
            .sort(sort)
            .await?;

        let mut docs = Vec::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            docs.push(item);
        }
        anyhow::Ok(DocsAndCount { docs, count })
    }

    pub async fn show_collection_docs(
        &self, collection_name: &str, filter: Document, sort: Document, page: u64, page_size: u64,
    ) -> anyhow::Result<DocsAndCount> {
        let db = self.get_db().await?;
        let count = db
            .collection::<u64>(collection_name)
            .count_documents(filter.clone())
            .await?;
        let mut stream = db
            .collection::<Document>(collection_name)
            .find(filter)
            .skip((page - 1) * page_size)
            .limit(page_size as i64)
            .sort(sort)
            .await?;
        let mut docs = Vec::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            docs.push(item);
        }
        anyhow::Ok(DocsAndCount { docs, count })
    }

    pub async fn run_command(&self, db: &str, cmd: Document) -> anyhow::Result<Document> {
        let conn = self.get_conn().await?;
        let doc = conn.database(db).run_command(cmd).await?;
        anyhow::Ok(doc)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DocsAndCount {
    pub docs: Vec<Document>,
    pub count: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct JsonAndCount {
    pub docs: Vec<serde_json::Value>,
    pub count: u64,
}
impl MongoDB {
    async fn duplicate_collection_impl(
        &self, old_collection: &str, dup: &str,
    ) -> anyhow::Result<()> {
        let db = self.get_db().await?;
        let old_table = db.collection::<Document>(old_collection);
        let new_table = db.collection::<Document>(dup);
        let mut stream = old_table.find(doc! {}).await?;
        while let Some(doc) = stream.next().await {
            let d = doc?;
            new_table.insert_one(d).await?;
        }
        anyhow::Ok(())
    }

    async fn delete_collection(&self, collection_name: &str) -> anyhow::Result<()> {
        let db = self.get_db().await?;
        db.collection::<bson::Document>(collection_name)
            .drop()
            .await?;
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl ServerMetas for MongoDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let admin_db = self.get_conn().await?.database("admin");
        let server_status = admin_db.run_command(doc! {"serverStatus": 1}).await?;
        // write_bson_s_json_to_file(&server_status, "./test.json");
        let mut ret = Vec::new();

        {
            let version = server_status.get_str("version")?.to_string();
            ret.push(MetaValue::new("version", version));
        }
        {
            let process = server_status.get_str("process")?.to_string();
            ret.push(MetaValue::new("process", process));
        }

        {
            let pid = server_status.get("pid").option_must_be_some()?.to_string();
            ret.push(MetaValue::new("pid", pid));
        }
        {
            let up_time = server_status.get_i64("uptimeMillis")?;
            ret.push(MetaValue::new(
                "up_time",
                humantime::format_duration(Duration::from_millis(up_time as u64)).to_string(),
            ));
        }
        let to_string_keys = &["catalogStats", "connections", "storageEngine"];
        for k in to_string_keys.iter() {
            if let Some(x) = server_status.get(*k) {
                ret.push(MetaValue::new(*k, x.to_string()))
            }
        }
        anyhow::Ok(ret)
    }
}

impl MongoDB {
    pub(crate) async fn list_collection_name_by_type(
        &self, ty: Option<CollectionType>,
    ) -> anyhow::Result<Vec<String>> {
        let conn = self.get_db().await?;
        let mut cols = conn.list_collections().await?;
        let mut ret = Vec::new();

        while let Some(col) = cols.next().await {
            let col = col?;
            if col.name.starts_with("system.") {
                continue;
            }
            if ty
                .as_ref()
                .map(|x| x == &col.collection_type)
                .unwrap_or(true)
            {
                ret.push(col.name);
            }
        }
        ret.sort();
        anyhow::Ok(ret)
    }
}

impl MongoDB {
    pub async fn list_time_serial_collection(&self) -> anyhow::Result<Vec<ListTable>> {
        let ts = self
            .list_collection_name_by_type(Some(CollectionType::Timeseries))
            .await?;
        anyhow::Ok(ts.into_iter().map(ListTable::from).collect())
    }

    pub async fn drop_mongo_time_serial_collection(&self, name: &str) -> anyhow::Result<()> {
        self.drop_table(name, false).await
    }
}

impl MongoDB {
    pub async fn show_columns_impl(&self, name: &str) -> anyhow::Result<Vec<Field>> {
        let db = self.get_db().await?;
        let value: Option<Document> = db.collection(name).find_one(doc! {}).await?;
        if value.is_none() {
            return anyhow::Ok(vec![]);
        }
        let value = value.option_must_be_some()?;
        let mut cols = infer_multi_value_column_names(&vec![value][..])?;
        cols.iter_mut().for_each(|x| {
            if x.field.as_str() == "_id" {
                x.is_primary = true;
            }
        });
        anyhow::Ok(cols)
    }
}

#[async_trait::async_trait]
impl SchemaTrait for MongoDB {
    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
    async fn duplicate_table(&self, old_collection: &str, dup: &str) -> anyhow::Result<()> {
        if self.table_exists(dup).await? {
            anyhow::bail!(format!("table '{}' already exists", dup));
        }
        if let Err(err) = self.duplicate_collection_impl(old_collection, dup).await {
            if let Err(err) = self.delete_collection(dup).await {
                log::error!(
                    "dup error has happen,but delete old collection failed,err:{:?}",
                    err
                );
            }
            return Err(err);
        }
        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let old = format!("{}.{}", self.db_name.as_ref().option_must_be_some()?, old);
        let new = format!("{}.{}", self.db_name.as_ref().option_must_be_some()?, new);
        let cmd = bson::doc! { "renameCollection":  old , "to": new};
        let client = self.get_conn().await?;
        let db = client.database("admin");

        db.run_command(cmd).await?;
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self)) as Box<dyn SchemaTrait>
    }

    async fn export_table_or_view(
        &self, name: &str, _para: Option<ShowTableParameter>, kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::Json { .. } | ExportKind::JsonLine => {
                let db = self.get_db().await?;
                let mut stream = db
                    .collection::<serde_json::Value>(name)
                    .find(doc! {})
                    .await?;
                while let Some(x) = stream.next().await {
                    let x = x?;
                    sender.send(x.into()).await?;
                }
                anyhow::Ok(())
            }
            _ => anyhow::bail!(format!("export kind not support:{:?}", kind)),
        }
    }

    async fn show_view(
        &self, _view_name: &str, _para: ShowTableParameter, _: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        anyhow::bail!("use mongo special method")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.drop_table(view_name, false).await
    }

    async fn drop_table(&self, collect_name: &str, _cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let conn = self.get_db().await?;
        conn.collection::<Document>(collect_name).drop().await?;
        anyhow::Ok(())
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        let views = self
            .list_collection_name_by_type(Some(CollectionType::View))
            .await?;

        anyhow::Ok(views.into_iter().map(ListTable::from).collect())
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let tables = self
            .list_collection_name_by_type(Some(CollectionType::Collection))
            .await?;
        anyhow::Ok(tables.into_iter().map(ListTable::from).collect())
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        self.show_columns_impl(table_name).await
    }

    async fn drop_index(&self, table_name: &str, index: &str) -> anyhow::Result<()> {
        if index == "_id_" {
            anyhow::bail!("can't drop this index");
        }
        let db = self.get_db().await?;
        db.collection::<Document>(table_name)
            .drop_index(index)
            .await?;
        anyhow::Ok(())
    }

    async fn rename_index(
        &self, table_name: &str, oldname: &str, newname: &str,
    ) -> anyhow::Result<()> {
        if oldname == "_id_" {
            anyhow::bail!("can't rename this index");
        }
        let db = self.get_db().await?;
        let mut indexes = db.collection::<Document>(table_name).list_indexes().await?;

        while let Some(index_model) = indexes.next().await {
            let mut index_model = index_model?;
            if index_model
                .options
                .as_ref()
                .and_then(|x| x.name.as_ref().map(|x| x == oldname))
                .unwrap_or(false)
            {
                // found
                db.collection::<Document>(table_name)
                    .drop_index(oldname)
                    .await?;
                if index_model.options.is_none() {
                    index_model.options = Some(IndexOptions::default());
                }
                index_model.options.as_mut().expect("must be some").name =
                    Some(newname.to_string());

                db.collection::<Document>(table_name)
                    .create_index(index_model)
                    .await?;
                return anyhow::Ok(());
            }
        }
        anyhow::bail!(format!("index '{}' not found", oldname))
    }

    async fn show_table(
        &self, _table_name: &str, _para: ShowTableParameter, _: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        anyhow::bail!("use mongo special method")
    }

    async fn show_indexes(&self, col_name: &str) -> anyhow::Result<Vec<Index>> {
        let db = self.get_db().await?;
        let mut stream = db.collection::<Document>(col_name).list_indexes().await?;
        let mut ret = Vec::new();
        while let Some(index_model) = stream.next().await {
            let index_model = index_model?;
            let name = index_model.options.as_ref().and_then(|x| x.name.clone());

            ret.push(Index::new(
                name.clone(),
                if name.as_deref().unwrap_or_default() == "_id" {
                    IndexKind::Primary
                } else if index_model
                    .options
                    .as_ref()
                    .map(|x| x.unique.unwrap_or_default())
                    .unwrap_or_default()
                {
                    IndexKind::Unique
                } else {
                    IndexKind::Index
                },
                vec![serde_json::to_string(&index_model.keys)?],
            ))
        }

        anyhow::Ok(ret)
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;

        let db = self.get_db().await?;
        db.collection::<Document>(table_name)
            .delete_many(doc! {})
            .await?;

        anyhow::Ok(())
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        self.show_columns_impl(view_name).await
    }
}

#[async_trait::async_trait]
impl QuerySql for MongoDB {
    async fn query_sql(
        &self, _id: &str, _sql: &str, _emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn execute_sql(
        &self, _query_id: &str, _sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        not_support_feature(self.backend_name())
    }

    async fn execute_sql_no_open_conn(&self, _sql: &str) -> anyhow::Result<Option<usize>> {
        not_support_feature(self.backend_name())
    }

    async fn open_connection(&self, _id: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }
    async fn close_connection(&self, _id: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }
}

impl MongoDB {
    pub(crate) async fn get_db(&self) -> anyhow::Result<mongodb::Database> {
        let client = self.get_conn().await?;
        Ok(client.database(self.db_name.as_ref().option_must_be_some()?))
    }

    pub(crate) async fn get_conn(&self) -> anyhow::Result<mongodb::Client> {
        anyhow::Ok(self.client.clone())
    }
}

// this used in test
#[allow(unused)]
#[cfg(test)]
pub(crate) fn make_test_object() -> MongoDB {
    let x = MongoDB::new(MongoDBConfig {
        conn_timeout: Some(5000.into()),
        host: TEST_HOST.to_string(),
        port: 27017,
        username: None,
        password: None,
        db: Some("test".to_string()),
        read_only: false,

        ssl: Default::default(),
    });
    x.unwrap()
}

#[cfg(test)]
mod tests {

    use std::str::FromStr;
    use std::time::SystemTime;
    use std::vec;

    use bson::oid::ObjectId;
    use bson::spec::BinarySubtype;
    use bson::{Binary, Bson, DateTime, Decimal128, Document, Regex, Timestamp, Uuid, doc};
    use mongodb::bson;
    use mongodb::options::TimeseriesOptions;

    use super::make_test_object;
    use crate::db::{DBTrait, SchemaTrait, ServerMetas};

    #[tokio::test]
    async fn test_multi_db() {
        let multi = make_test_object();
        let _ = multi.create_schema("abc", None).await;
        let dbs = DBTrait::list_schema(&multi).await.unwrap();
        println!("{:?}", dbs);
        for db in dbs.iter() {
            let _ = DBTrait::open_schema(&multi, db.as_str()).await.unwrap();
        }
        multi.drop_schema("abc").await.unwrap();
    }

    #[tokio::test]
    async fn test_mongo_id_types() {
        let db = make_test_object();
        let col = "test_id_types";
        let _ = db.drop_table(col, false).await;

        {
            let mut d = bson::Document::new();
            d.insert("_id", Bson::Double(rand::random::<f64>()));
            d.insert("value", Bson::Int32(1));
            db.query_collection_insert_one(col, d).await.unwrap();
        }
        {
            let mut d = bson::Document::new();
            d.insert("_id", Bson::String("aaaaaaaaaaa".to_string()));
            d.insert("value", Bson::Int32(2));
            db.query_collection_insert_one(col, d).await.unwrap();
        }

        {
            let mut d = bson::Document::new();
            d.insert(
                "_id",
                Bson::Document(bson::doc! { "value" :  Bson::Int32(3)}),
            );
            d.insert("value", Bson::Int32(3));
            db.query_collection_insert_one(col, d).await.unwrap();
        }
        {
            let mut d = bson::Document::new();
            d.insert("_id", Bson::Boolean(true));
            d.insert("value", Bson::Int32(5));
            db.query_collection_insert_one(col, d).await.unwrap();
        }
        {
            let mut d = bson::Document::new();
            d.insert("_id", Bson::JavaScriptCode("xxx".to_string()));
            d.insert("value", Bson::Int32(6));
            db.query_collection_insert_one(col, d).await.unwrap();
        }
        {
            let mut d = bson::Document::new();
            let _id = Bson::JavaScriptCodeWithScope(bson::JavaScriptCodeWithScope {
                code: "xxx".to_string(),
                scope: bson::doc! {"yyy" : Bson::Int32(7) },
            });
            d.insert("_id", _id);

            d.insert("value", Bson::Int32(8));
            db.query_collection_insert_one(col, d).await.unwrap();
        }
        {
            let mut d = bson::Document::new();
            let _id = Bson::Int32(9);
            d.insert("_id", _id);

            d.insert("value", Bson::Int32(10));
            db.query_collection_insert_one(col, d).await.unwrap();
        }
        {
            let mut d = bson::Document::new();
            let _id = Bson::Int64(100);
            d.insert("_id", _id);

            d.insert("value", Bson::Int32(11));
            db.query_collection_insert_one(col, d).await.unwrap();
        }

        {
            let mut d = bson::Document::new();
            let _id = Bson::Timestamp(bson::Timestamp {
                time: 1,
                increment: 1,
            });
            d.insert("_id", _id);

            d.insert("value", Bson::Int32(12));
            db.query_collection_insert_one(col, d).await.unwrap();
        }
        {
            let mut d = bson::Document::new();
            let _id = Bson::Binary(bson::Binary {
                subtype: bson::spec::BinarySubtype::Generic,
                bytes: vec![1, 2, 3],
            });
            d.insert("_id", _id);
            d.insert("value", Bson::Int32(13));
            db.query_collection_insert_one(col, d).await.unwrap();
        }
        {
            let mut d = bson::Document::new();
            let _id = Bson::ObjectId(bson::oid::ObjectId::new());
            d.insert("_id", _id);

            d.insert("value", Bson::Int32(14));
            db.query_collection_insert_one(col, d).await.unwrap();
        }

        {
            let mut d = bson::Document::new();
            let _id = Bson::DateTime(bson::DateTime::now());
            d.insert("_id", _id);

            d.insert("value", Bson::Int32(15));
            db.query_collection_insert_one(col, d).await.unwrap();
        }
        {
            let mut d = bson::Document::new();
            let _id = Bson::Symbol("sssssss".to_owned());
            d.insert("_id", _id);

            d.insert("value", Bson::Int32(16));
            db.query_collection_insert_one(col, d).await.unwrap();
        }
        {
            let mut d = bson::Document::new();
            let _id = Bson::Decimal128("1234".parse().unwrap());
            d.insert("_id", _id);

            d.insert("value", Bson::Int32(17));
            db.query_collection_insert_one(col, d).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_server_metas() {
        let db = make_test_object();
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }
    #[tokio::test]
    async fn test_index() {
        let db = make_test_object();
        let table = "test_index";
        let _ = db.drop_table(table, false).await;

        db.insert(table, doc! {"name" : "haha" , "age" : 2 })
            .await
            .unwrap();
        let indexes = db.show_indexes(table).await.unwrap();
        println!("indexes{:?}", indexes);

        let index_name = "test_index_aaa";

        db.create_index(table, Some(index_name.to_string()), false, doc! {"aaa" : 1})
            .await
            .unwrap();
        let index_name2 = format!("{}_1", index_name);

        db.rename_index(table, index_name, &index_name2)
            .await
            .unwrap();
        db.rename_index(table, &index_name2, index_name)
            .await
            .unwrap();
        db.drop_index(table, index_name).await.unwrap();
    }
    #[tokio::test]
    async fn test_truncate() {
        let db = make_test_object();
        let table = "test_truncate";
        let _ = db.drop_table(table, false).await;

        db.insert(table, doc! {"name" : "haha" , "age" : 2 })
            .await
            .unwrap();
        db.truncate_table(table).await.unwrap();
    }
    #[tokio::test]
    async fn test_export() {
        let db = make_test_object();
        let table = "test_export";
        let _ = db.drop_table(table, false).await;

        db.insert(table, doc! {"name" : "haha" , "age" : 2 })
            .await
            .unwrap();
        db.insert(table, doc! {"name" : "a name" , "age" : 10  })
            .await
            .unwrap();

        let schema_trait = db;
        let json = schema_trait
            .export_table_to_string(table, crate::db::ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
        println!("json:{}", json);

        let json_lines = schema_trait
            .export_table_to_string(table, crate::db::ExportKind::JsonLine)
            .await
            .unwrap();

        println!("json_lines:{}", json_lines);
    }

    #[tokio::test]

    async fn test_rename() {
        let db = make_test_object();

        let table_name = "test_rename";
        let table_name2 = "test_rename2";

        let _ = db.drop_table(table_name, false).await;
        let _ = db.drop_table(table_name2, false).await;
        let database = db.get_db().await.unwrap();
        let c = database.collection::<Document>(table_name);
        let d = bson::doc! {"name" : 123};
        c.insert_one(d).await.unwrap();

        db.rename_table(table_name, table_name2).await.unwrap();
    }
    #[tokio::test]

    async fn test_dup() {
        let db = make_test_object();

        let table_name = "test_dup";
        let table_name2 = "test_dup2";

        let _ = db.drop_table(table_name, false).await;
        let _ = db.drop_table(table_name2, false).await;
        let database = db.get_db().await.unwrap();
        let c = database.collection::<Document>(table_name);
        let d = bson::doc! {"name" : 123};
        c.insert_one(d).await.unwrap();

        db.duplicate_table(table_name, table_name2).await.unwrap();

        let docs = db
            .show_collection_docs(table_name2, bson::doc! {}, bson::doc! {}, 1, 100)
            .await
            .unwrap();
        println!("{:?}", docs);
    }

    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object();
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_large_table() {
        let db = make_test_object();
        let col = "test_large_table";
        let _ = db.drop_table(col, false).await;

        for _ in 0..1590 {
            db.query_collection_insert_one(
                col,
                doc! {
                    "a" : rand::random::<i32 >(),
                    "b" : rand::random::<i32 >(),
                    "c" : rand::random::<i32 >(),
                },
            )
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn test_time_serial() {
        let table = "test_time_serial";
        let db = make_test_object();
        let _ = db.drop_table(table, false).await;

        {
            let builder = TimeseriesOptions::builder();
            let op = builder.time_field("time_field").build();

            let db = db.get_db().await.unwrap();
            db.create_collection("test_time_serial")
                .timeseries(op)
                .await
                .unwrap();
        }
        let time = bson::DateTime::from_millis(1111111111);

        db.insert(
            table,
            doc! {"a" : rand::random::<i32 >() , "time_field" :time },
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_view() {
        let db = make_test_object();

        let view_name = "test_view";
        let _ = db.drop_view(view_name).await;

        let collection_name = "test_mongo";
        // 定义聚合管道
        let pipeline: Vec<Document> = vec![];
        let _ = db
            .get_db()
            .await
            .unwrap()
            .run_command(doc! {
                "create": view_name,
                "viewOn": collection_name,
                "pipeline": pipeline,
            })
            .await
            .unwrap();
        let view_names = db.list_view().await.unwrap();
        println!("view names:{:?}", view_names)
    }
    #[tokio::test]
    async fn test_xxx() {
        let db = make_test_object();
        let view_name = "test_mongo";
        let docs = db
            .show_collection_docs(view_name, doc! {}, doc! {}, 1, 100)
            .await
            .unwrap();

        for d in docs.docs.iter() {
            println!("{:?}", d);
        }
    }

    #[tokio::test]
    async fn test2() {
        let db = make_test_object();
        let table = "test_mongo";
        let _ = db.drop_table(table, false).await;

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64;

        println!("now:{}", now);

        let binary_field = Bson::Binary(Binary::from(bson::binary::Vector::Int8(vec![1, 2])));
        let binary_field2 = Bson::Binary(Binary::from(bson::binary::Vector::Float32(vec![
            1f32, 2f32,
        ])));
        let input = "aGVsbG8=".to_string(); // hello base64
        let binary_field3 = Binary::from_base64(input.clone(), None).unwrap();
        let binary_field4 =
            Binary::from_base64(input.clone(), Some(BinarySubtype::Encrypted)).unwrap();
        let binary_field5 = Binary::from_base64(input, BinarySubtype::Sensitive).unwrap();
        let data = doc! {
            "_id": ObjectId::new(),
            "string_field": "Hello, world!",
            "int32_field": Bson::Int32(123),
            "float_field": 123.45,
            "bool_field": true,
            "date_field": DateTime::from_millis(now * 1_000), // MongoDB uses milliseconds since epoch
            "null_field": Bson::Null,
            "regex_field": Regex { pattern : "haha".to_string(),options :"i".to_string()} ,
            "binary_field":binary_field,
            "binary_field2": binary_field2,
            "binary_field3": binary_field3,
            "binary_field4": binary_field4,
            "binary_field5": binary_field5,
            "uuid_field": Uuid::new(),
            "array_field": vec![
                Bson::Int32(1),
                Bson::Int32(2),
                Bson::Int32(3),
            ],
            "object_field": doc! {"nested_key": "nested_value"},
            "decimal_field": Decimal128::from_str("1").unwrap(),
            "timestamp_field": Timestamp {time: now as u32, increment: 1 },
        };
        db.insert(table, data).await.unwrap();
        let resp = db
            .show_collection_table_list(table, doc! {}, doc! {}, 1, 100)
            .await
            .unwrap();

        println!("resp:{:?}", resp);

        let resp = db
            .show_collection_docs(table, doc! {}, doc! {}, 1, 100)
            .await
            .unwrap();
        for d in resp.docs {
            println!("{:?}", d);
            let json_str = serde_json::to_string_pretty(&d).unwrap();
            println!("json_str:{}", json_str);
        }
        let table_list = db.list_table().await.unwrap();
        println!("{:?}", table_list);
    }
}
