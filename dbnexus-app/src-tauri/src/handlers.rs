use std::collections::HashMap;

use bson::Document;
use dbnexus_db::db::{
    ExportKind, Field, Index, ListTable, ListTrigger, ListView, QueryResp, SqlExecuteRet,
    TableListResp, TriggerDetail,
};
use dbnexus_db::mongo::Mongo;
use dbnexus_db::oracle::OracleDB;
use dbnexus_db::redis::{RedisDB, RedisListTable};

use crate::storage::{self, App, DBConnectionConfig, UserSettings};

#[tauri::command]
pub async fn add_config(
    config: DBConnectionConfig, dbs: tauri::State<'_, App>,
) -> Result<String, AnyhowErrorFrontend> {
    let uuid = uuid::Uuid::new_v4().to_string();

    log::info!("add config:{} {:?}", uuid, config);
    dbs.add_config(uuid.as_str(), config)
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    Ok(uuid)
}
#[tauri::command]
pub async fn delete_config(
    uuid: String, dbs: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    log::info!("del config:{}", uuid);
    dbs.delete_config(uuid.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn update_config(
    uuid: String, config: DBConnectionConfig, dbs: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    log::info!("update config:{} {:?}", uuid, config);
    dbs.update_config(uuid.as_str(), config)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn open_db(uuid: String, dbs: tauri::State<'_, App>) -> Result<(), AnyhowErrorFrontend> {
    log::info!("open db:{}", uuid);
    dbs.open_db(uuid.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
        .map(|_| ())
}

#[tauri::command]
pub async fn list_table(
    uuid: String, dbs: tauri::State<'_, App>,
) -> Result<Vec<ListTable>, AnyhowErrorFrontend> {
    log::info!("show table db:{}", uuid);
    dbs.list_table(uuid.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn redis_list_table(
    uuid: String, dbs: tauri::State<'_, App>,
) -> Result<RedisListTable, AnyhowErrorFrontend> {
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<RedisDB>() {
        Some(x) => x
            .redis_list_table()
            .await
            .map_err(AnyhowErrorFrontend::from),
        None => Err(AnyhowErrorFrontend::from(anyhow::Error::msg(format!(
            "'{}' not a redis",
            uuid
        )))),
    }
}

#[tauri::command]
pub async fn show_columns(
    uuid: String, table_name: String, dbs: tauri::State<'_, App>,
) -> Result<Vec<Field>, AnyhowErrorFrontend> {
    dbs.show_columns(uuid.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn show_table(
    uuid: String, table_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, dbs: tauri::State<'_, App>,
) -> Result<TableListResp, AnyhowErrorFrontend> {
    dbs.show_table(
        uuid.as_str(),
        &table_name,
        where_str,
        order_by,
        page,
        page_size,
    )
    .await
    .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn show_indexes(
    uuid: String, table_name: String, dbs: tauri::State<'_, App>,
) -> Result<Vec<Index>, AnyhowErrorFrontend> {
    dbs.show_indexes(uuid.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn insert(
    uuid: String, table_name: String, values: HashMap<String, Option<String>>,
    dbs: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, AnyhowErrorFrontend> {
    dbs.insert(uuid.as_str(), &table_name, values)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn update(
    uuid: String, table_name: String, values: HashMap<String, Option<String>>,
    wheres: HashMap<String, Option<String>>, dbs: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, AnyhowErrorFrontend> {
    dbs.update(uuid.as_str(), &table_name, values, wheres)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn truncate_table(
    uuid: String, table_name: String, dbs: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    dbs.truncate_table(uuid.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn delete(
    uuid: String, table_name: String, wheres: HashMap<String, Option<String>>,
    dbs: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, AnyhowErrorFrontend> {
    dbs.delete(uuid.as_str(), &table_name, wheres)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn list_view(
    uuid: String, dbs: tauri::State<'_, App>,
) -> Result<Vec<ListView>, AnyhowErrorFrontend> {
    dbs.list_view(uuid.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn drop_view(
    uuid: String, view_name: String, dbs: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    dbs.drop_view(uuid.as_str(), &view_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn drop_table(
    uuid: String, table_name: String, dbs: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    dbs.drop_table(uuid.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn show_view(
    uuid: String, view_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, dbs: tauri::State<'_, App>,
) -> Result<TableListResp, AnyhowErrorFrontend> {
    dbs.show_view(
        uuid.as_str(),
        &view_name,
        where_str,
        order_by,
        page,
        page_size,
    )
    .await
    .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn open_db2(
    uuid: String, db_name: String, dbs: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    dbs.open_db2(uuid.as_str(), db_name.as_str())
        .await
        .map(|_| ())
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn query_sql(
    uuid: String, id: uuid::Uuid, sql: String, dbs: tauri::State<'_, App>,
) -> Result<Vec<Result<QueryResp, String>>, AnyhowErrorFrontend> {
    let sql = sql.as_str();
    let db = dbs
        .open_db(uuid.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;

    let result = match db {
        crate::storage::DB::Multi(x) => x.query_sql(id, sql).await,
        crate::storage::DB::One(x) => x.query_sql(id, sql).await,
    };

    result.map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn query_sql_open_connection(
    uuid: String, dbs: tauri::State<'_, App>,
) -> Result<uuid::Uuid, AnyhowErrorFrontend> {
    let db = dbs
        .open_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let result = match db {
        crate::storage::DB::Multi(x) => x.open_connection().await,
        crate::storage::DB::One(x) => x.open_connection().await,
    };
    result.map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn query_sql_re_connection(
    uuid: String, id: uuid::Uuid, dbs: tauri::State<'_, App>,
) -> Result<(), String> {
    let db = dbs
        .open_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let result = match db {
        crate::storage::DB::Multi(x) => x.re_connect(id).await,
        crate::storage::DB::One(x) => x.re_connect(id).await,
    };
    result.map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn query_sql_close_connection(
    uuid: String, id: uuid::Uuid, dbs: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let db = dbs
        .open_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let result = match db {
        crate::storage::DB::Multi(x) => x.close_connection(id).await,
        crate::storage::DB::One(x) => x.close_connection(id).await,
    };
    result.map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn save_user_settings(
    user_settings: UserSettings, dbs: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    dbs.loader
        .save_user_settings(user_settings)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn get_user_settings(
    dbs: tauri::State<'_, App>,
) -> Result<UserSettings, AnyhowErrorFrontend> {
    dbs.loader
        .get_user_settings_cloned()
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn list_all_config(
    dbs: tauri::State<'_, App>,
) -> Result<HashMap<String /* uuid */, DBConnectionConfig>, AnyhowErrorFrontend> {
    dbs.loader
        .list_all_configs_cloned()
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn export_table(
    uuid: String, table_name: String, filepath: String, kind: ExportKind,
    dbs: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    db.export_table_to_file(table_name.as_str(), filepath.as_str(), kind)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn format_sql(
    uuid: String, sql: String, dbs: tauri::State<'_, App>,
) -> Result<String, String> {
    let db = dbs.get_db(uuid.as_str()).await.map_err(|e| e.to_string())?;
    let ret = match db {
        crate::storage::DB::Multi(x) => x.format_sql(sql.as_str()),
        crate::storage::DB::One(x) => x.format_sql(sql.as_str()),
    };
    ret.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_connection(
    config: DBConnectionConfig,
) -> Result<
    u64, // milliseconds
    AnyhowErrorFrontend,
> {
    let db = storage::open_db(&config, Default::default())
        .await
        .map_err(|e| e.to_string())?;
    match db {
        storage::DB::Multi(x) => x.check_conn().await,
        storage::DB::One(x) => x.check_conn().await,
    }
    .map_err(AnyhowErrorFrontend::from)
    .map(|d| d.as_millis() as u64)
}

#[tauri::command]
pub async fn oracle_query_set_autocommit(
    uuid: String, id: uuid::Uuid, commit: bool, dbs: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x
            .query_set_autocommit(id, commit)
            .await
            .map_err(AnyhowErrorFrontend::from),
        None => Err(AnyhowErrorFrontend::from(format!(
            "db '{}' is not oracle",
            uuid
        ))),
    }
}
#[tauri::command]
pub async fn oracle_query_autocommit(
    uuid: String, id: uuid::Uuid, dbs: tauri::State<'_, App>,
) -> Result<bool, AnyhowErrorFrontend> {
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x
            .query_autocommit(id)
            .await
            .map_err(AnyhowErrorFrontend::from),
        None => Err(AnyhowErrorFrontend::from(format!(
            "db '{}' is not oracle",
            uuid
        ))),
    }
}
#[tauri::command]
pub async fn oracle_query_commit(
    uuid: String, id: uuid::Uuid, dbs: tauri::State<'_, App>,
) -> Result<(), String> {
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x.query_commit(id).await.map_err(|e| e.to_string()),
        None => Err(format!("db '{}' is not oracle", uuid)),
    }
}
#[tauri::command]
pub async fn oracle_query_rollback(
    uuid: String, id: uuid::Uuid, dbs: tauri::State<'_, App>,
) -> Result<(), String> {
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x.query_rollback(id).await.map_err(|e| e.to_string()),
        None => Err(format!("db '{}' is not oracle", uuid)),
    }
}

#[tauri::command]
pub async fn list_trigger(
    uuid: String, table: String, dbs: tauri::State<'_, App>,
) -> Result<Vec<ListTrigger>, String> {
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;

    db.list_trigger(&table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn show_trigger(
    uuid: String, table: String, name: String, dbs: tauri::State<'_, App>,
) -> Result<TriggerDetail, String> {
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;

    db.show_trigger(&table, &name)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn rename_table(
    uuid: String, old: String, new: String, dbs: tauri::State<'_, App>,
) -> Result<(), String> {
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;

    db.rename_table(&old, &new).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn duplicate_table(
    uuid: String, from: String, to: String, dbs: tauri::State<'_, App>,
) -> Result<(), String> {
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;

    db.duplicate_table(&from, &to)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mongo_insert(
    uuid: String, table: String, doc: Vec<u8>, dbs: tauri::State<'_, App>,
) -> Result<mongodb::results::InsertOneResult, String> {
    let doc = parse_bson_doc(doc.as_slice())?;

    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.insert(table.as_str(), doc).await,
        None => return Err(format!("db '{}' not mongo", uuid)),
    }
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn mongo_update(
    uuid: String, table: String, update: Vec<u8>, query: Vec<u8>, dbs: tauri::State<'_, App>,
) -> Result<mongodb::results::UpdateResult, String> {
    let update = parse_bson_doc(update.as_slice())?;
    let query = parse_bson_doc(query.as_slice())?;

    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.update(table.as_str(), update, query).await,
        None => return Err(format!("db '{}' not mongo", uuid)),
    }
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn mongo_delete(
    uuid: String, table: String, query: Vec<u8>, dbs: tauri::State<'_, App>,
) -> Result<mongodb::results::DeleteResult, String> {
    let query = parse_bson_doc(query.as_slice())?;

    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.delete(table.as_str(), query).await,
        None => return Err(format!("db '{}' not mongo", uuid)),
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mongo_show_table(
    uuid: String, table: String, filter: Vec<u8>, order_by: Vec<u8>, page: usize, page_size: usize,
    dbs: tauri::State<'_, App>,
) -> Result<TableListResp, String> {
    let filter = parse_bson_doc(filter.as_slice())?;
    let order_by = parse_bson_doc(order_by.as_slice())?;
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => {
            x.show_collection(table.as_str(), filter, order_by, page, page_size)
                .await
        }
        None => return Err(format!("db '{}' not mongo", uuid)),
    }
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn mongo_show_table_docs(
    uuid: String, table: String, filter: Vec<u8>, order_by: Vec<u8>, page: usize, page_size: usize,
    dbs: tauri::State<'_, App>,
) -> Result<DocsAndCount, String> {
    let filter = parse_bson_doc(filter.as_slice())?;
    let order_by = parse_bson_doc(order_by.as_slice())?;
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let docs = match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => {
            x.show_collection_docs(table.as_str(), filter, order_by, page, page_size)
                .await
        }
        None => return Err(format!("db '{}' not mongo", uuid)),
    }
    .map_err(|e| e.to_string())?;
    DocsAndCount::try_from(docs)
}

#[derive(serde::Serialize)]
pub struct DocsAndCount {
    count: u64,
    /// list of docs in bson format.
    /// use this library to parse.
    /// https://github.com/mongodb/js-bson
    docs: Vec<Vec<u8>>,
}

impl TryFrom<dbnexus_db::mongo::DocsAndCount> for DocsAndCount {
    type Error = String;

    fn try_from(value: dbnexus_db::mongo::DocsAndCount) -> Result<Self, Self::Error> {
        let mut vec = Vec::with_capacity(value.docs.len());
        for index in 0..value.docs.len() {
            let bs = encode_bson_doc(&value.docs[index])?;
            vec.push(bs);
        }
        Ok(Self {
            count: value.count,
            docs: vec,
        })
    }
}
fn encode_bson_doc(doc: &bson::Document) -> Result<Vec<u8>, String> {
    let bs = bson::to_vec(&doc).map_err(|e| e.to_string())?;
    Ok(bs)
}

fn parse_bson_doc(bs: &[u8]) -> Result<Document, String> {
    let doc = bson::Document::from_reader(std::io::Cursor::new(bs)).map_err(|e| e.to_string())?;
    Ok(doc)
}

#[derive(serde::Serialize)]
pub struct AnyhowErrorFrontend {
    // short message on why
    reason: String,
    // long message about why and backtrace
    detail: String,
}

impl From<anyhow::Error> for AnyhowErrorFrontend {
    fn from(value: anyhow::Error) -> Self {
        Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
        }
    }
}
impl From<String> for AnyhowErrorFrontend {
    fn from(value: String) -> Self {
        Self {
            reason: value,
            detail: "".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bson_serial() {
        let d = bson::doc! {"x" : 1 ,"b" : 2};
        let d2 = parse_bson_doc(&encode_bson_doc(&d).unwrap()).unwrap();
        assert_eq!(format!("{:?}", d), format!("{:?}", d2));
    }
    #[test]
    fn test_anyhow_display_and_debug() {
        let err = anyhow::Error::msg("aaaa").context("bbbb");
        println!("{}", err);
        println!("#####################################");
        println!("{:?}", err);
    }
}
