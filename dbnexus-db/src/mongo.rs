use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::time::Duration;

use futures_util::StreamExt;
use mongodb::bson::oid::ObjectId;
use mongodb::bson::{doc, Bson, Document};
use mongodb::options::{FindOptions, UpdateOptions};
use serde_json::Value;

use crate::datasource::{self, MongoDataSource};
use crate::db::{
    CheckConn, DBTrait, ExeSqlError, ExportKind, Field, Index, IndexKind, ListTable, ListView,
    QuerySql, SqlExecuteRet, TableListResp,
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
fn infer_schema(v: &serde_json::Value) -> Vec<Field> {
    match v {
        serde_json::Value::Object(x) => {
            let mut ret = Vec::with_capacity(x.len());
            for (k, v) in x.iter() {
                let typ = match v {
                    serde_json::Value::Null => "null",
                    serde_json::Value::Bool(_) => "bool",
                    serde_json::Value::Number(_) => "number",
                    serde_json::Value::String(_) => "string",
                    serde_json::Value::Array(_) => "array",
                    serde_json::Value::Object(_) => "object",
                };
                ret.push(Field::from_name_type(k.as_str(), typ));
            }
            ret
        }

        _ => vec![],
    }
}

fn infer_multi_value_column_names(vs: &[serde_json::Value]) -> anyhow::Result<Vec<String>> {
    let mut ret = Vec::new();
    let mut key_set = HashSet::new();

    for v in vs.iter() {
        match v {
            serde_json::Value::Object(x) => {
                for k in x.keys() {
                    if !key_set.contains(k) {
                        ret.push(k.clone());
                        key_set.insert(k.clone());
                    }
                }
            }
            _ => {
                anyhow::bail!("top value is not a object")
            }
        }
    }
    anyhow::Ok(ret)
}

fn value_to_frontend(v: &Value) -> String {
    match v {
        Value::Null => "null".to_string(),
        Value::Bool(x) => x.to_string(),
        Value::Number(x) => x.to_string(),
        Value::String(x) => x.to_string(),
        Value::Array(_) => v.to_string(),
        Value::Object(_) => v.to_string(),
    }
}

#[async_trait::async_trait]
impl CheckConn for Mongo {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        let _db = self.get_db().await?;
        anyhow::Ok(ins.elapsed())
    }
}
#[async_trait::async_trait]
impl DBTrait for Mongo {
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
        let pretty = match kind {
            ExportKind::Json { pretty } => pretty,
            _ => anyhow::bail!("mongo only support export json."),
        };
        let db = self.get_db().await?;
        let mut stream = db
            .collection::<serde_json::Value>(table_name)
            .find(None, None)
            .await?;
        sender.send(Cow::Borrowed("[")).await?;
        if pretty {
            sender.send(Cow::Borrowed("\n")).await?;
        }
        let mut need_comma = false;

        while let Some(item) = stream.next().await {
            let item = item?;
            if need_comma {
                sender.send(Cow::Borrowed(",")).await?;
                if pretty {
                    sender.send(Cow::Borrowed("\n")).await?;
                }
            }
            sender
                .send(Cow::Owned(if pretty {
                    serde_json::to_string_pretty(&item)?
                } else {
                    serde_json::to_string(&item)?
                }))
                .await?;
            need_comma = true;
        }
        if pretty {
            sender.send(Cow::Borrowed("\n")).await?;
        }
        sender.send(Cow::Borrowed("]")).await?;
        anyhow::Ok(())
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
        conn.collection::<serde_json::Value>(table)
            .drop(None)
            .await?;

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
        let value: Option<serde_json::Value> =
            db.collection(table_name).find_one(None, None).await?;
        if value.is_none() {
            return anyhow::Ok(vec![]);
        }
        let value = value.as_ref().unwrap();
        anyhow::Ok(infer_schema(&value))
    }

    async fn show_table(
        &self, table_name: &str, filter: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let db = self.get_db().await?;
        let order_by = if let Some(order_by) = order_by {
            let v = serde_json::from_str(order_by)?;
            let v = match v {
                serde_json::Value::Object(x) => x,
                _ => {
                    anyhow::bail!("order str must be an object")
                }
            };
            Some(Document::try_from(v)?)
        } else {
            None
        };
        let options = FindOptions::builder()
            .skip(Some(((page - 1) * page_size) as u64))
            .limit(page_size as i64)
            .sort(order_by)
            .build();

        let filter = if let Some(where_str) = filter {
            let v = serde_json::from_str(where_str)?;
            let v = match v {
                serde_json::Value::Object(x) => x,
                _ => {
                    anyhow::bail!("where str must be an object")
                }
            };
            Some(Document::try_from(v)?)
        } else {
            None
        };

        let count = db
            .collection::<u64>(table_name)
            .count_documents(filter.clone(), None)
            .await?;

        let mut stream = db
            .collection::<serde_json::Value>(table_name)
            .find(filter, Some(options))
            .await?;
        let mut values = Vec::new();
        while let Some(v) = stream.next().await {
            let v = v?;
            values.push(v);
        }
        let cols = infer_multi_value_column_names(&values)?;
        let mut data = Vec::with_capacity(cols.len());

        for v in values.iter() {
            let map = match v {
                serde_json::Value::Object(m) => m,
                _ => anyhow::bail!("document '{}' is not an object", v),
            };
            let mut values = Vec::new();
            for (_index, c) in cols.iter().enumerate() {
                let value = map.get(c).map(|x| {
                    let x = if c == "_id" {
                        x.as_object()
                            .expect("this must be an object")
                            .get("$oid")
                            .expect("must have this field")
                    } else {
                        x
                    };
                    value_to_frontend(x)
                });
                values.push(value);
            }
            data.push(values);
        }
        anyhow::Ok(TableListResp::new(
            cols.into_iter()
                .map(|x| Field::from_name_type(x, ""))
                .collect(),
            count as usize,
            data,
            "",
        ))
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let db = self.get_db().await?;
        let mut stream = db
            .collection::<serde_json::Value>(table_name)
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
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let values = {
            let mut value_json = HashMap::new();
            for (k, v) in check_input_none_string(&values)?.iter() {
                let newv = serde_json::Value::from_str(v.as_str())
                    .ok()
                    .unwrap_or(serde_json::Value::from(v.as_str()));
                log::trace!("str:{:?} as '{:?}'", v.as_str(), newv);
                value_json.insert((*k).clone(), newv);
            }
            value_json
        };
        let db = self.get_db().await?;
        let ret = db.collection(table_name).insert_one(values, None).await?;
        log::trace!("insert id:{}", ret.inserted_id);

        anyhow::Ok(SqlExecuteRet {
            sql: "".to_string(),
            changes: 1,
            last_insert_row_id: None,
            last_insert_id_string: Some(match &ret.inserted_id {
                Bson::ObjectId(x) => x.to_string(),
                _ => unreachable!(),
            }),
        })
    }

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let mut update_by_id = None;

        let wheres = {
            let mut value_json = serde_json::Map::new();
            for (k, v) in check_input_none_string(&wheres)?.iter() {
                let newv = serde_json::from_str(v.as_str())
                    .ok()
                    .unwrap_or(serde_json::Value::from(v.as_str()));
                if k.as_str() == "_id" {
                    update_by_id = Some((*v).clone());
                }
                value_json.insert((*k).clone(), newv);
            }
            value_json
        };

        let values = {
            let mut value_json = serde_json::Map::new();
            for (k, v) in check_input_none_string(&values)?.iter() {
                let newv =
                    serde_json::from_str(v.as_str()).unwrap_or(serde_json::Value::from(v.as_str()));
                value_json.insert((*k).clone(), newv);
            }

            value_json
        };
        let wheres = match update_by_id {
            Some(x) => {
                doc! {"_id" : ObjectId::from_str(x.as_str()).expect(format!("id:{:?}" ,x ).as_str()) }
            }
            _ => Document::try_from(wheres)?,
        };
        let values = {
            let mut m = serde_json::Map::new();
            m.insert("$set".to_string(), serde_json::Value::Object(values));
            m
        };
        let values = Document::try_from(values)?;
        let options = UpdateOptions::default();
        let db = self.get_db().await?;
        let ret = db
            .collection::<Document>(table_name)
            .update_many(wheres, values, options)
            .await?;

        anyhow::Ok(SqlExecuteRet::from(ret.modified_count))
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;

        let db = self.get_db().await?;
        db.collection::<serde_json::Value>(table_name)
            .delete_many(doc! {}, None)
            .await?;

        anyhow::Ok(())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let mut update_by_id = None;
        let wheres = {
            let mut value_json = serde_json::Map::new();
            for (k, v) in check_input_none_string(&wheres)?.iter() {
                let newv = serde_json::from_str(v.as_str())
                    .ok()
                    .unwrap_or(serde_json::Value::from((*v).as_str()));
                if k.as_str() == "_id" {
                    update_by_id = Some((*v).clone());
                    break;
                }
                value_json.insert((*k).clone(), newv);
            }
            value_json
        };
        let db = self.get_db().await?;
        let wheres = match update_by_id {
            Some(x) => {
                doc! {"_id" : ObjectId::from_str(x.as_str()).expect(format!("id:{:?} is not right" ,x ).as_str()) }
            }
            _ => Document::try_from(wheres)?,
        };

        let ret = db
            .collection::<serde_json::Value>(table_name)
            .delete_many(Document::try_from(wheres)?, None)
            .await?;

        anyhow::Ok(SqlExecuteRet::from(ret.deleted_count))
    }
}

#[async_trait::async_trait]
impl QuerySql for Mongo {
    async fn query_sql(
        &self, _id: uuid::Uuid, _sql: &str,
    ) -> anyhow::Result<Vec<Result<TableListResp, ExeSqlError>>> {
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
        let client = self
            .pm
            .get(&datasource::DBPoolKey::MongoDB(self.ds.clone()))
            .await?;
        let client = match client {
            crate::pool::NexusPool::MongoDB(x) => x.clone(),
            _ => unreachable!(),
        };

        Ok(client.database(self.dbname.as_str()))
    }
}

// our parameter accept a value as `HashMap<String, Option<String>>`
// as match for sql `null` , but mongodb have no null type
fn check_input_none_string(
    hashmap: &HashMap<String, Option<String>>,
) -> anyhow::Result<HashMap<&String, &String>> {
    let mut m = HashMap::new();
    for (k, v) in hashmap.iter() {
        if v.is_none() {
            anyhow::bail!("k '{}' is none", k);
        }
        m.insert(k, v.as_ref().unwrap());
    }
    anyhow::Ok(m)
}

#[cfg(test)]

mod tests {
    use std::collections::HashMap;

    use super::Mongo;
    use crate::datasource::MongoDataSource;
    use crate::pool::PoolManager;
    use crate::{init_log, TEST_HOST};

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
    async fn test() {
        let oj = make_test_object();
        let handle = &oj as &dyn crate::db::DBTrait;
        let tables = handle.list_table().await.unwrap();
        for t in tables.iter() {
            let cols = handle.show_columns(t.table.as_str()).await.unwrap();
            println!("cols:{:?}", cols);
            let list = handle
                .show_table(t.table.as_str(), None, None, 1, 100)
                .await
                .unwrap();
            println!("list:{:?}", list);
            let indexes = handle.show_indexes(t.table.as_str()).await.unwrap();
            println!("list:{:?}", indexes);
        }
    }
    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object();
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test2() {
        init_log();

        let table_name = "users";
        let oj = make_test_object();
        {
            let db = oj.get_db().await.unwrap();
            let _ = db
                .collection::<serde_json::Value>(table_name)
                .drop(None)
                .await;
        }
        let db = &oj as &dyn crate::db::DBTrait;

        {
            let mut values = HashMap::new();

            values.insert("email".to_string(), Some("a@qq.com".to_string()));
            values.insert(
                "password".to_string(),
                Some(
                    r#"
                ["111", "222"]
            "#
                    .to_string(),
                ),
            );

            db.insert(table_name, values).await.unwrap();
        }

        {
            let mut values = HashMap::new();

            values.insert("email".to_string(), Some("b@qq.com".to_string()));
            values.insert(
                "password".to_string(),
                Some(
                    r#"
                {"a":123,"b":456}
            "#
                    .to_string(),
                ),
            );
            let ret = db.insert(table_name, values.clone()).await.unwrap();
            println!("ret{:?}", ret);

            values.insert("email".to_string(), Some("c@qq.com".to_string()));
            values.remove(&"password".to_string());
            let changes = db
                .update(table_name, values, {
                    let mut wheres = HashMap::new();
                    wheres.insert(
                        "_id".to_string(),
                        Some(ret.last_insert_id_string.clone().unwrap()),
                    );

                    wheres
                })
                .await
                .unwrap();
            assert_eq!(changes.changes, 1);
            db.delete(table_name, {
                let mut wheres = HashMap::new();
                wheres.insert("_id".to_string(), Some(ret.last_insert_id_string.unwrap()));
                wheres
            })
            .await
            .unwrap();
            assert_eq!(changes.changes, 1);
        }

        let indexes = db.show_indexes(table_name).await.unwrap();
        println!("indexes:{:?}", indexes);
        let list = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("indexes:{:?}", list);

        let export_str = db
            .export_table_to_string(table_name, crate::db::ExportKind::Json { pretty: true })
            .await
            .unwrap();
        println!("export_str:{}", export_str);
        let export_str = db
            .export_table_to_string(table_name, crate::db::ExportKind::Json { pretty: false })
            .await
            .unwrap();
        println!("export_str2:{}", export_str);

        db.drop_table(table_name).await.unwrap();
    }
}
