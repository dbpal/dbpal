use std::collections::HashMap;

use bson::{doc, Document};
use dbnexus_db::app::{self, App, DBConnectionConfig, Open, OpendKind, UserSettings};
use dbnexus_db::db::{
    ExportKind, Field, FormatSqlOpts, Index, ListTable, NamedStaff, QueryResp, SqlExecuteRet,
    TableListResp, TriggerDetail,
};
use dbnexus_db::mongo::Mongo;
use dbnexus_db::oracle::OracleDB;
use dbnexus_db::redis::{RedisDB, RedisListTable};

#[tauri::command]
pub async fn add_config(
    config: DBConnectionConfig, app: tauri::State<'_, App>,
) -> Result<String, AnyhowErrorFrontend> {
    let uuid = uuid::Uuid::new_v4().to_string();

    log::info!("add config:{} {:?}", uuid, config);
    app.add_config(uuid.as_str(), config)
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    Ok(uuid)
}
#[tauri::command]
pub async fn delete_config(
    uuid: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    log::info!("del config:{}", uuid);
    app.delete_config(uuid.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn update_config(
    uuid: String, config: DBConnectionConfig, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    log::info!("update config:{} {:?}", uuid, config);
    app.update_config(uuid.as_str(), config)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn open(uuid: String, app: tauri::State<'_, App>) -> Result<(), AnyhowErrorFrontend> {
    log::info!("open db:{}", uuid);
    app.open(uuid.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
        .map(|_| ())
}

#[tauri::command]
pub async fn list_table(
    uuid: String, app: tauri::State<'_, App>,
) -> Result<Vec<ListTable>, AnyhowErrorFrontend> {
    log::info!("show table db:{}", uuid);
    app.list_table(uuid.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn redis_list_table(
    uuid: String, app: tauri::State<'_, App>,
) -> Result<RedisListTable, AnyhowErrorFrontend> {
    let db = app
        .get_schema(uuid.as_str())
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
pub async fn redis_match_keys(
    uuid: String, pattern: String, app: tauri::State<'_, App>,
) -> Result<Vec<String>, AnyhowErrorFrontend> {
    let db = app
        .get_schema(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<RedisDB>() {
        Some(x) => x
            .redis_match_keys(pattern.as_str())
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
    uuid: String, table_name: String, app: tauri::State<'_, App>,
) -> Result<Vec<Field>, AnyhowErrorFrontend> {
    app.show_columns(uuid.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn show_table(
    uuid: String, table_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, app: tauri::State<'_, App>,
) -> Result<TableListResp, AnyhowErrorFrontend> {
    app.show_table(
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
    uuid: String, table_name: String, app: tauri::State<'_, App>,
) -> Result<Vec<Index>, AnyhowErrorFrontend> {
    app.show_indexes(uuid.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn insert(
    uuid: String, table_name: String, values: HashMap<String, Option<String>>,
    app: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, AnyhowErrorFrontend> {
    app.insert(uuid.as_str(), &table_name, values)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn update(
    uuid: String, table_name: String, values: HashMap<String, Option<String>>,
    wheres: HashMap<String, Option<String>>, app: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, AnyhowErrorFrontend> {
    app.update(uuid.as_str(), &table_name, values, wheres)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn truncate_table(
    uuid: String, table_name: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    app.truncate_table(uuid.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn delete(
    uuid: String, table_name: String, wheres: HashMap<String, Option<String>>,
    app: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, AnyhowErrorFrontend> {
    app.delete(uuid.as_str(), &table_name, wheres)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn list_view(
    uuid: String, app: tauri::State<'_, App>,
) -> Result<Vec<NamedStaff>, AnyhowErrorFrontend> {
    app.list_view(uuid.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn drop_view(
    uuid: String, view_name: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    app.drop_view(uuid.as_str(), &view_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn drop_table(
    uuid: String, table_name: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    app.drop_table(uuid.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn show_view(
    uuid: String, view_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, app: tauri::State<'_, App>,
) -> Result<TableListResp, AnyhowErrorFrontend> {
    app.show_view(
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
pub async fn open_schema(
    uuid: String, db_name: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    app.open_schema(uuid.as_str(), db_name.as_str())
        .await
        .map(|_| ())
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn opened_kind(
    uuid: String, app: tauri::State<'_, App>,
) -> Result<OpendKind, AnyhowErrorFrontend> {
    let kind = app
        .get_opened(uuid.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?
        .get_open_kind();
    Ok(kind)
}

#[tauri::command]
pub async fn open_db(
    uuid: String, db_name: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    app.open_db(uuid.as_str(), db_name.as_str())
        .await
        .map(|_| ())
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn query_sql(
    uuid: String, id: uuid::Uuid, sql: String, app: tauri::State<'_, App>,
) -> Result<Vec<Result<QueryResp, String>>, AnyhowErrorFrontend> {
    let sql = sql.as_str();
    let db = app
        .open(uuid.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;

    let result = match db {
        Open::DataBase(x) => x.query_sql(id, sql).await,
        Open::Schema(x) => x.query_sql(id, sql).await,
        Open::Catelog(x) => x.query_sql(id, sql).await,
    };

    result.map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn query_sql_open_connection(
    uuid: String, app: tauri::State<'_, App>,
) -> Result<uuid::Uuid, AnyhowErrorFrontend> {
    let db = app.open(uuid.as_str()).await.map_err(|e| e.to_string())?;
    let result = match db {
        Open::DataBase(x) => x.open_connection().await,
        Open::Schema(x) => x.open_connection().await,
        Open::Catelog(x) => x.open_connection().await,
    };
    result.map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn query_sql_re_connection(
    uuid: String, id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), String> {
    let db = app.open(uuid.as_str()).await.map_err(|e| e.to_string())?;
    let result = match db {
        Open::DataBase(x) => x.re_connect(id).await,
        Open::Schema(x) => x.re_connect(id).await,
        app::Open::Catelog(x) => x.re_connect(id).await,
    };
    result.map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn query_sql_close_connection(
    uuid: String, id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let db = app.open(uuid.as_str()).await.map_err(|e| e.to_string())?;
    let result = match db {
        Open::DataBase(x) => x.close_connection(id).await,
        Open::Schema(x) => x.close_connection(id).await,
        Open::Catelog(x) => x.close_connection(id).await,
    };
    result.map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn save_user_settings(
    user_settings: UserSettings, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    app.loader
        .save_user_settings(user_settings)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn get_user_settings(
    app: tauri::State<'_, App>,
) -> Result<UserSettings, AnyhowErrorFrontend> {
    app.loader
        .get_user_settings_cloned()
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn list_all_config(
    app: tauri::State<'_, App>,
) -> Result<HashMap<String /* uuid */, DBConnectionConfig>, AnyhowErrorFrontend> {
    app.loader
        .list_all_configs_cloned()
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn export_table(
    uuid: String, table_name: String, filepath: String, kind: ExportKind,
    app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let db = app
        .get_schema(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    db.export_table_to_file(table_name.as_str(), filepath.as_str(), kind)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn format_sql(
    uuid: String, sql: String, opts: Option<FormatSqlOpts>, app: tauri::State<'_, App>,
) -> Result<String, String> {
    let db = app
        .get_opened(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db {
        Open::DataBase(x) => x.format_sql(sql.as_str(), opts),
        Open::Schema(x) => x.format_sql(sql.as_str(), opts),
        Open::Catelog(x) => x.format_sql(sql.as_str(), opts),
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
    let db = app::open(&config).await.map_err(|e| e.to_string())?;
    match db {
        app::Open::DataBase(x) => x.check_conn().await,
        app::Open::Schema(x) => x.check_conn().await,
        app::Open::Catelog(x) => x.check_conn().await,
    }
    .map_err(AnyhowErrorFrontend::from)
    .map(|d| d.as_millis() as u64)
}

#[tauri::command]
pub async fn oracle_query_set_autocommit(
    uuid: String, id: uuid::Uuid, commit: bool, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let db = app
        .get_schema(uuid.as_str())
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
    uuid: String, id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<bool, AnyhowErrorFrontend> {
    let db = app
        .get_schema(uuid.as_str())
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
    uuid: String, id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), String> {
    let db = app
        .get_schema(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x.query_commit(id).await.map_err(|e| e.to_string()),
        None => Err(format!("db '{}' is not oracle", uuid)),
    }
}
#[tauri::command]
pub async fn oracle_query_rollback(
    uuid: String, id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), String> {
    let db = app
        .get_schema(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x.query_rollback(id).await.map_err(|e| e.to_string()),
        None => Err(format!("db '{}' is not oracle", uuid)),
    }
}

#[tauri::command]
pub async fn list_trigger(
    uuid: String, table: String, app: tauri::State<'_, App>,
) -> Result<Vec<NamedStaff>, String> {
    let db = app
        .get_schema(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;

    db.list_trigger(&table).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn show_trigger(
    uuid: String, table: String, name: String, app: tauri::State<'_, App>,
) -> Result<TriggerDetail, String> {
    let db = app
        .get_schema(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;

    db.show_trigger(&table, &name)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn rename_table(
    uuid: String, old: String, new: String, app: tauri::State<'_, App>,
) -> Result<(), String> {
    let db = app
        .get_schema(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;

    db.rename_table(&old, &new).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn duplicate_table(
    uuid: String, from: String, to: String, app: tauri::State<'_, App>,
) -> Result<(), String> {
    let db = app
        .get_schema(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;

    db.duplicate_table(&from, &to)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn mongo_insert(
    uuid: String, table: String, doc: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<mongodb::results::InsertOneResult, String> {
    let doc = parse_bson_doc(doc.as_slice())?;

    let db = app
        .get_schema(uuid.as_str())
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
    uuid: String, table: String, update: Vec<u8>, query: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<mongodb::results::UpdateResult, String> {
    let update = parse_bson_doc(update.as_slice())?;
    let query = parse_bson_doc(query.as_slice())?;

    let db = app
        .get_schema(uuid.as_str())
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
    uuid: String, table: String, query: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<mongodb::results::DeleteResult, String> {
    let query = parse_bson_doc(query.as_slice())?;

    let db = app
        .get_schema(uuid.as_str())
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
    uuid: String, table: String, filter: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<TableListResp, String> {
    let filter = parse_bson_doc(filter.as_slice())?;
    let db = app
        .get_schema(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.show_collection(table.as_str(), filter).await,
        None => return Err(format!("db '{}' not mongo", uuid)),
    }
    .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn mongo_show_table_docs(
    uuid: String, table: String, filter: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<DocsAndCount, String> {
    let filter = parse_bson_doc(filter.as_slice())?;
    let db = app
        .get_schema(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let docs = match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.show_collection_docs(table.as_str(), filter).await,
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
