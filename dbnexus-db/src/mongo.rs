use std::borrow::Cow;
use std::collections::HashMap;
use std::time::Duration;

use anyhow::Context;
use futures_util::StreamExt;
use mongodb::bson::{doc, Document};
use mongodb::options::FindOptions;
use mongodb::results::{DeleteResult, InsertOneResult, UpdateResult};

use crate::datasource::{self, MongoDataSource};
use crate::db::{
    CheckConn, DBTrait, ExportKind, Field, FormatSql, Index, IndexKind, ListTable, ListView,
    QueryResp, QuerySql, SqlExecuteRet, TableListResp,
};

#[derive(Clone)]
pub struct Mongo {
    ds: MongoDataSource,
    dbname: String,
    pm: crate::pool::PoolManager,
    read_only: bool,
}

impl Mongo {
    pub fn new(
        pm: crate::pool::PoolManager, ds: MongoDataSource, dbname: String, read_only: bool,
    ) -> Self {
        Self {
            ds,
            dbname,
            pm,
            read_only,
        }
    }
}

fn collect_table_resp_vec(bs: &[bson::Bson]) -> anyhow::Result<String> {
    if bs.len() == 0 {
        return anyhow::Ok("[]".to_string());
    }
    let last = bs.len() - 1;
    let mut ret = "[".to_string();
    for (index, b) in bs.iter().enumerate() {
        let v = bson_value_to_string(b)?.unwrap_or("null".to_string());
        ret.push_str(v.as_str());
        if index != last {
            ret.push(',');
        }
    }
    ret.push(']');

    anyhow::Ok(ret)
}

fn bson_value_to_string(b: &bson::Bson) -> anyhow::Result<Option<String>> {
    let value = match b {
        bson::Bson::Double(x) => Some(x.to_string()),
        bson::Bson::String(x) => Some(x.to_string()),
        bson::Bson::Array(x) => Some(collect_table_resp_vec(&x[..])?),
        bson::Bson::Document(x) => Some(x.to_string()),
        bson::Bson::Boolean(x) => Some(x.to_string()),
        bson::Bson::Null => None,
        bson::Bson::RegularExpression(x) => Some(x.to_string()),
        bson::Bson::JavaScriptCode(x) => Some(x.to_string()),
        bson::Bson::JavaScriptCodeWithScope(x) => Some(x.to_string()),
        bson::Bson::Int32(x) => Some(x.to_string()),
        bson::Bson::Int64(x) => Some(x.to_string()),
        bson::Bson::Timestamp(x) => Some(x.to_string()),
        bson::Bson::Binary(x) => Some(x.to_string()),
        bson::Bson::ObjectId(x) => Some(x.to_string()),
        bson::Bson::DateTime(x) => Some(x.to_string()),
        bson::Bson::Symbol(x) => Some(x.to_string()),
        bson::Bson::Decimal128(x) => Some(x.to_string()),
        bson::Bson::Undefined => None,
        bson::Bson::MaxKey | bson::Bson::MinKey | bson::Bson::DbPointer(_) => {
            anyhow::bail!(format!("should not handle those value,bson:{:?}", b))
        }
    };

    anyhow::Ok(value)
}
fn collect_table_resp(
    fs: &Vec<Field>, docs: &[Document],
) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let mut ret = Vec::with_capacity(docs.len());
    for doc in docs.iter() {
        let mut values = vec![];
        for f in fs.iter() {
            let value = doc
                .get(&f.name)
                .map(|b| bson_value_to_string(b).ok())
                .flatten()
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
                bson::Bson::MaxKey => continue,
                bson::Bson::MinKey => continue,
                // Deprecated
                bson::Bson::DbPointer(_) => continue,
            };
            ret.insert(k.clone(), Field::from_name_type(k.as_str(), ty));
        }
    }

    anyhow::Ok(ret.into_iter().map(|(_, v)| v).collect())
}

#[async_trait::async_trait]
impl CheckConn for Mongo {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        let db = self.get_db().await?;
        let _ping_result = db
            .run_command(bson::doc! {"ping": 1}, None)
            .await
            .context("ping failed")?;
        anyhow::Ok(ins.elapsed())
    }
}

impl FormatSql for Mongo {
    fn format_sql(&self, _sql: &str) -> anyhow::Result<String> {
        anyhow::bail!(format!(
            "this backend '{}' don't support format sql",
            self.backend_name()
        ))
    }
}

impl Mongo {
    /// normal `insert` , `update` ,`delete` works very bad with mongodb
    /// So i just impl those method on Document
    pub async fn insert(&self, table: &str, doc: Document) -> anyhow::Result<InsertOneResult> {
        let db = self.get_db().await?;
        let ret = db
            .collection::<Document>(table)
            .insert_one(doc, None)
            .await
            .context("insert failed")?;
        anyhow::Ok(ret)
    }

    pub async fn update(
        &self, table: &str, update: Document, query: Document,
    ) -> anyhow::Result<UpdateResult> {
        self.can_write()?;
        let db = self.get_db().await?;
        let ret = db
            .collection::<Document>(table)
            .update_many(query, update, None)
            .await
            .context("update failed")?;
        anyhow::Ok(ret)
    }

    pub async fn delete(&self, table: &str, query: Document) -> anyhow::Result<DeleteResult> {
        self.can_write()?;
        let db = self.get_db().await?;
        let ret = db
            .collection::<Document>(table)
            .delete_many(query, None)
            .await
            .context("delete failed")?;
        anyhow::Ok(ret)
    }

    pub async fn show_table(
        &self, table: &str, filter: Document, order_by: Document, page: usize, page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let options = FindOptions::builder()
            .skip(Some(((page - 1) * page_size) as u64))
            .limit(page_size as i64)
            .sort(order_by)
            .build();
        let db = self.get_db().await?;
        let count = db
            .collection::<u64>(table)
            .count_documents(filter.clone(), None)
            .await
            .context("query total count failed")?;
        let mut stream = db
            .collection::<Document>(table)
            .find(filter, options)
            .await
            .context("get query stream")?;

        let mut docs = Vec::with_capacity(page_size);
        while let Some(item) = stream.next().await {
            let item = item.context("list doc failed")?;
            docs.push(item);
        }

        let cols = infer_multi_value_column_names(&docs[..]).context("infer columns failed")?;
        let data = collect_table_resp(&cols, docs.as_slice()).context("make response failed")?;

        anyhow::Ok(TableListResp::new(cols, count as usize, data, None))
    }

    pub async fn show_table_docs(
        &self, table: &str, filter: Document, order_by: Document, page: usize, page_size: usize,
    ) -> anyhow::Result<DocsAndCount> {
        let options = FindOptions::builder()
            .skip(Some(((page - 1) * page_size) as u64))
            .limit(page_size as i64)
            .sort(order_by)
            .build();
        let db = self.get_db().await?;
        let count = db
            .collection::<u64>(table)
            .count_documents(filter.clone(), None)
            .await
            .context("query docs count failed")?;
        let mut stream = db
            .collection::<Document>(table)
            .find(filter, options)
            .await
            .context("get query result")?;
        let mut docs = Vec::with_capacity(page_size);
        while let Some(item) = stream.next().await {
            let item = item.context("list doc failed")?;
            docs.push(item);
        }
        anyhow::Ok(DocsAndCount { docs, count })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DocsAndCount {
    pub docs: Vec<Document>,
    pub count: u64,
}

impl Mongo {
    async fn duplicate_table_impl(&self, old_table: &str, dup: &str) -> anyhow::Result<()> {
        let db = self.get_db().await?;
        let old_table = db.collection::<Document>(old_table);
        let new_table = db.collection::<Document>(dup);
        let mut stream = old_table
            .find(None, None)
            .await
            .context("query duplicate table failed")?;
        while let Some(doc) = stream.next().await {
            let d = doc.context("get doc failed")?;
            new_table
                .insert_one(d, None)
                .await
                .context("insert to new table failed")?;
        }
        anyhow::Ok(())
    }
    async fn delete_collection(&self, collection_name: &str) -> anyhow::Result<()> {
        let db = self.get_db().await?;
        db.collection::<bson::Document>(collection_name)
            .drop(None)
            .await
            .context("drop collection failed")?;
        anyhow::Ok(())
    }
}

impl Mongo {
    async fn export_stream_as_json_line(
        mut stream: mongodb::Cursor<Document>, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        while let Some(item) = stream.next().await {
            let item = item.context("stream item failed")?;
            sender
                .send(Cow::Owned(
                    serde_json::to_string(&item).context("encode json failed")?,
                ))
                .await?;
            sender.send(Cow::Borrowed("\n")).await?;
        }
        anyhow::Ok(())
    }

    async fn export_stream_as_json(
        mut stream: mongodb::Cursor<Document>, pretty: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        sender.send(Cow::Borrowed("[")).await?;
        let mut first = true;
        while let Some(item) = stream.next().await {
            let item = item.context("stream doc failed")?;
            if !first {
                sender.send(Cow::Borrowed(",")).await?;
            }
            if pretty {
                sender.send(Cow::Borrowed("\n")).await?;
            }
            sender
                .send(Cow::Owned(if pretty {
                    serde_json::to_string_pretty(&item).context("encode json failed")?
                } else {
                    serde_json::to_string(&item).context("encode json failed")?
                }))
                .await?;

            first = false;
        }
        if pretty {
            sender.send(Cow::Borrowed("\n")).await?;
        }
        sender.send(Cow::Borrowed("]")).await?;
        anyhow::Ok(())
    }

    async fn export_table_as_json(
        &self, table_name: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let db = self.get_db().await?;
        let stream = db
            .collection::<Document>(table_name)
            .find(None, None)
            .await
            .context("execute query stream failed")?;
        Self::export_stream_as_json(stream, pretty, sender).await
    }
    async fn export_table_as_json_line(
        &self, table_name: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let db = self.get_db().await?;
        let stream = db
            .collection::<Document>(table_name)
            .find(None, None)
            .await?;
        Self::export_stream_as_json_line(stream, sender).await
    }
}

#[async_trait::async_trait]
impl DBTrait for Mongo {
    async fn duplicate_table(&self, old_table: &str, dup: &str) -> anyhow::Result<()> {
        if let Err(err) = self.duplicate_table_impl(&old_table, dup).await {
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
        let old = format!("{}.{}", self.dbname, old);
        let new = format!("{}.{}", self.dbname, new);
        let cmd = bson::doc! { "renameCollection":  old , "to": new};
        let client = self.get_conn().await?;
        let db = client.database("admin");

        db.run_command(cmd, None)
            .await
            .context("run rename command failed")?;
        anyhow::Ok(())
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    fn backend_name(&self) -> &str {
        "mongodb"
    }

    fn clone(&self) -> Box<dyn DBTrait> {
        Box::new(Clone::clone(self))
    }

    async fn export_table(
        &self, table_name: &str, kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::Json { pretty } => {
                self.export_table_as_json(table_name, pretty, sender).await
            }
            ExportKind::JsonLine => self.export_table_as_json_line(table_name, sender).await,
            _ => anyhow::bail!("mongo only support export json."),
        }
    }

    async fn show_view(
        &self, _view_name: &str, _where_str: Option<&str>, _order_by: Option<&str>, _page: usize,
        _page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        anyhow::bail!("driver don't have view functions")
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn drop_view(&self, _view_name: &str) -> anyhow::Result<()> {
        anyhow::bail!("driver don't have view functions")
    }

    async fn drop_table(&self, table: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let conn = self.get_db().await?;
        conn.collection::<Document>(table).drop(None).await?;
        anyhow::Ok(())
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListView>> {
        anyhow::bail!("driver don't have view functions")
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let db = self.get_db().await?;
        let tables = db.list_collection_names(None).await?;
        anyhow::Ok(tables.into_iter().map(ListTable::from).collect())
    }

    async fn show_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let db = self.get_db().await?;
        let value: Option<Document> = db.collection(table_name).find_one(None, None).await?;
        if value.is_none() {
            return anyhow::Ok(vec![]);
        }
        let value = value.unwrap();
        let cols = infer_multi_value_column_names(&vec![value][..])?;

        anyhow::Ok(cols)
    }

    async fn show_table(
        &self, _table_name: &str, _filter: Option<&str>, _order_by: Option<&str>, _page: usize,
        _page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        anyhow::bail!("use mongo show_tablee")
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let db = self.get_db().await?;
        let mut stream = db
            .collection::<Document>(table_name)
            .list_indexes(None)
            .await?;
        let mut ret = Vec::new();
        while let Some(index_model) = stream.next().await {
            let index_model = index_model?;
            ret.push(Index::new(
                index_model.keys.to_string(),
                if index_model
                    .options
                    .as_ref()
                    .map(|x| x.unique.unwrap_or_default())
                    .unwrap_or_default()
                {
                    IndexKind::Unique
                } else {
                    IndexKind::Normal
                },
                vec![],
            ))
        }

        anyhow::Ok(ret)
    }

    async fn insert(
        &self, _table_name: &str, _values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        anyhow::bail!("use mongo insert")
    }

    async fn update(
        &self, _table_name: &str, _values: HashMap<String, Option<String>>,
        _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        anyhow::bail!("use mongo update")
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;

        let db = self.get_db().await?;
        db.collection::<Document>(table_name)
            .delete_many(doc! {}, None)
            .await?;

        anyhow::Ok(())
    }

    async fn delete(
        &self, _table_name: &str, _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        anyhow::bail!("use mongo delete")
    }
}

#[async_trait::async_trait]
impl QuerySql for Mongo {
    async fn query_sql(
        &self, _id: uuid::Uuid, _sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        anyhow::bail!("mongodb query sql is not supported right now.")
    }

    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        anyhow::bail!("mongodb doesn't support query sql")
    }

    async fn re_connect(&self, _id: uuid::Uuid) -> anyhow::Result<()> {
        anyhow::bail!("mongodb doesn't support query sql")
    }

    async fn close_connection(&self, _id: uuid::Uuid) -> anyhow::Result<()> {
        anyhow::bail!("mongodb doesn't support query sql")
    }
}

impl Mongo {
    async fn get_db(&self) -> anyhow::Result<mongodb::Database> {
        let client = self.get_conn().await.context("get conn failed")?;
        Ok(client.database(self.dbname.as_str()))
    }

    async fn get_conn(&self) -> anyhow::Result<mongodb::Client> {
        let client = self
            .pm
            .get(&datasource::DBPoolKey::MongoDB(self.ds.clone()))
            .await?;
        let client = match client {
            crate::pool::NexusPool::MongoDB(x) => x.clone(),
            _ => unreachable!(),
        };
        anyhow::Ok(client)
    }
}

#[cfg(test)]

mod tests {

    use std::str::FromStr;
    use std::time::SystemTime;

    use bson::oid::ObjectId;
    use bson::{doc, Bson, DateTime, Decimal128, Document, Regex, Timestamp, Uuid};

    use super::Mongo;
    use crate::datasource::MongoDataSource;
    use crate::db::DBTrait;
    use crate::pool::PoolManager;
    use crate::TEST_HOST;

    fn make_test_object() -> Mongo {
        let x = Mongo {
            ds: MongoDataSource {
                host: TEST_HOST.to_string(),
                port: 27017,
                username_and_password: None,
            },
            read_only: false,
            dbname: "test".to_string(),
            pm: PoolManager::default(),
        };
        x
    }
    #[tokio::test]
    async fn test_show_index() {
        let db = make_test_object();
        let table = "test_index";
        let _ = db.drop_table(&table).await;

        db.insert(&table, doc! {"name" : "haha" , "age" : 2 })
            .await
            .unwrap();
        let indexes = db.show_indexes(&table).await.unwrap();
        println!("indexes{:?}", indexes);
    }
    #[tokio::test]
    async fn test_truncat() {
        let db = make_test_object();
        let table = "test_truncat";
        let _ = db.drop_table(&table).await;

        db.insert(&table, doc! {"name" : "haha" , "age" : 2 })
            .await
            .unwrap();
        db.truncate_table(&table).await.unwrap();
    }
    #[tokio::test]
    async fn test_export() {
        let db = make_test_object();
        let table = "test_export";
        let _ = db.drop_table(&table).await;

        db.insert(&table, doc! {"name" : "haha" , "age" : 2 })
            .await
            .unwrap();
        db.insert(&table, doc! {"name" : "a name" , "age" : 10  })
            .await
            .unwrap();

        let db_trait = &db as &dyn DBTrait;
        let json = db_trait
            .export_table_to_string(&table, crate::db::ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
        println!("json:{}", json);

        let json_lines = db_trait
            .export_table_to_string(&table, crate::db::ExportKind::JsonLine)
            .await
            .unwrap();

        println!("json_lines:{}", json_lines);
    }

    #[tokio::test]

    async fn test_rename() {
        let db = make_test_object();
        let db_trait = &db as &dyn DBTrait;
        let table_name = "test_rename";
        let table_name2 = "test_rename2";

        let _ = db_trait.drop_table(&table_name).await;
        let _ = db_trait.drop_table(&table_name2).await;
        let database = db.get_db().await.unwrap();
        let c = database.collection::<Document>(&table_name);
        let d = bson::doc! {"name" : 123};
        c.insert_one(d, None).await.unwrap();

        db_trait
            .rename_table(table_name, table_name2)
            .await
            .unwrap();
    }
    #[tokio::test]

    async fn test_dup() {
        let db = make_test_object();
        let db_trait = &db as &dyn DBTrait;
        let table_name = "test_dup";
        let table_name2 = "test_dup2";

        let _ = db_trait.drop_table(&table_name).await;
        let _ = db_trait.drop_table(&table_name2).await;
        let database = db.get_db().await.unwrap();
        let c = database.collection::<Document>(&table_name);
        let d = bson::doc! {"name" : 123};
        c.insert_one(d, None).await.unwrap();

        db_trait
            .duplicate_table(table_name, table_name2)
            .await
            .unwrap();

        let docs = db
            .show_table_docs(table_name2, bson::doc! {}, bson::doc! {}, 1, 100)
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
    async fn test2() {
        let db = make_test_object();
        let table = "test_mongo";
        let _ = db.drop_table(&table).await;
        db.insert(table, doc! {"name" : "xxx" , "age" : 22})
            .await
            .unwrap();
        db.insert(table, doc! {"name" : "yyy" , "age" : 22})
            .await
            .unwrap();

        db.update(table, doc! {"$set": {"age" : 100}}, doc! {"name" : "yyy"})
            .await
            .unwrap();
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs() as i64;
        let data = doc! {
            "_id": ObjectId::new(),
            "string_field": "Hello, world!",
            "int_field": 123,
            "float_field": 123.45,
            "bool_field": true,
            "date_field": DateTime::from_millis(now * 1_000), // MongoDB uses milliseconds since epoch
            "null_field": Bson::Null,
            "regex_field": Regex { pattern : "haha".to_string(),options :"i".to_string()} ,
            "binary_field": vec![0x01, 0x02, 0x03],
            "uuid_field": Uuid::new(),
            "array_field": vec![
                Bson::Int32(1),
                Bson::Int32(2),
                Bson::Int32(3),
            ],
            "object_field": doc! {"nested_key": "nested_value"},
            "decimal_field": Decimal128::from_str("1").unwrap(),
            "timestamp_field": Timestamp {time: now as u32, increment: 1},
        };

        db.insert(&table, data).await.unwrap();
        assert_eq!(
            db.delete(&table, doc! {"xxx" : 1})
                .await
                .unwrap()
                .deleted_count,
            0
        );

        let resp = db
            .show_table(table, doc! {}, doc! {}, 1, 100)
            .await
            .unwrap();
        println!("resp:{:?}", resp);

        let table_list = db.list_table().await.unwrap();
        println!("{:?}", table_list);
    }
}
