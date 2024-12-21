use std::collections::HashMap;
use std::str::FromStr;

use bson::{Document, doc};
use dbnexus_db::app::{self, App, DBConnectionConfig, Open, OpendKind, UserSettings};
use dbnexus_db::db::{
    ExportKind, Field, FormatSqlOpts, Index, ListTable, NamedStaff, QueryResp, SqlExecuteRet,
    TableListResp, TriggerDetail,
};
use dbnexus_db::mongo::Mongo;
use dbnexus_db::oracle::OracleDB;

use dbnexus_db::redis::{RedisDB, RedisListTable};
use tauri::AppHandle;
use tauri::Emitter;

#[tauri::command]
pub async fn add_config(
    config: DBConnectionConfig, app: tauri::State<'_, App>, handler: AppHandle,
) -> Result<(), AnyhowErrorFrontend> {
    log::info!("add config:{:?}", config);
    if config.get_name().contains(":") {
        return Err(AnyhowErrorFrontend::from(
            "connection name can't contains ':'",
        ));
    }
    app.add_config(config)
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    handler.emit("config_added", "").unwrap();
    Ok(())
}
#[tauri::command]
pub async fn list_all_config(
    app: tauri::State<'_, App>,
) -> Result<Vec<DBConnectionConfig>, AnyhowErrorFrontend> {
    app.loader
        .list_all_configs_cloned()
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn delete_config(
    name: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("del config:{}", name);
    app.delete_config(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
}

// #[tauri::command]
// pub async fn update_config(
//     name: String, config: DBConnectionConfig, app: tauri::State<'_, App>,
// ) -> Result<(), AnyhowErrorFrontend> {
//     random_sleep().await;
//     random_error()?;
//     log::info!("update config:{} {:?}", name, config);
//     app.update_config(name.as_str(), config)
//         .await
//         .map_err(AnyhowErrorFrontend::from)
// }

// #[tauri::command]
// pub async fn open(name: String, app: tauri::State<'_, App>) -> Result<(), AnyhowErrorFrontend> {
//     random_sleep().await;
//     random_error()?;
//     log::info!("open db:{}", name);
//     app.open(name.as_str())
//         .await
//         .map_err(AnyhowErrorFrontend::from)
//         .map(|_| ())
// }

#[tauri::command]
pub async fn list_table(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<ListTable>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("show table db:{}", name);
    app.list_table(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn redis_list_table(
    name: String, app: tauri::State<'_, App>,
) -> Result<RedisListTable, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.redis_list_table(&name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn redis_match_keys(
    name: String, pattern: String, app: tauri::State<'_, App>,
) -> Result<Vec<String>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let keys = app.redis_match_keys(&name, &pattern).await?;
    Ok(keys)
}

#[tauri::command]
pub async fn show_columns(
    name: String, table_name: String, app: tauri::State<'_, App>,
) -> Result<Vec<Field>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.show_columns(name.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn show_table(
    name: String, table_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, app: tauri::State<'_, App>,
) -> Result<MuiDataTableListResp, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let x = app
        .show_table(
            name.as_str(),
            &table_name,
            where_str,
            order_by,
            page,
            page_size,
        )
        .await
        .map_err(AnyhowErrorFrontend::from)?;

    Ok(MuiDataTableListResp::from(x))
}

#[tauri::command]
pub async fn show_indexes(
    name: String, table_name: String, app: tauri::State<'_, App>,
) -> Result<Vec<Index>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.show_indexes(name.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn insert(
    name: String, table_name: String, values: HashMap<String, Option<String>>,
    app: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.insert(name.as_str(), &table_name, values)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn update(
    name: String, table_name: String, values: HashMap<String, Option<String>>,
    wheres: HashMap<String, Option<String>>, app: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.update(name.as_str(), &table_name, values, wheres)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn truncate_table(
    name: String, table_name: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.truncate_table(name.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn delete(
    name: String, table_name: String, wheres: HashMap<String, Option<String>>,
    app: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.delete(name.as_str(), &table_name, wheres)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn list_view(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<NamedStaff>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.list_view(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn drop_view(
    name: String, view_name: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.drop_view(name.as_str(), &view_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn drop_table(
    name: String, table_name: String, cascade: bool, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.drop_table(name.as_str(), &table_name, cascade)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn show_view(
    name: String, view_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, app: tauri::State<'_, App>,
) -> Result<MuiDataTableListResp, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.show_view(
        name.as_str(),
        &view_name,
        where_str,
        order_by,
        page,
        page_size,
    )
    .await
    .map_err(AnyhowErrorFrontend::from)
    .map(MuiDataTableListResp::from)
}

#[tauri::command]
pub async fn opened_kind(
    name: String, app: tauri::State<'_, App>,
) -> Result<OpendKind, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let kind = app
        .get_opened(&name)
        .await
        .map_err(AnyhowErrorFrontend::from)?
        .get_open_kind();
    Ok(kind)
}

#[tauri::command]
pub async fn query_sql(
    name: String, query_id: uuid::Uuid, sql: String, app: tauri::State<'_, App>,
) -> Result<Vec<Result<MuiDataGridData, String>>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let sql = sql.as_str();
    let db = app
        .open(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;

    let result = match db {
        Open::DataBase(x) => x.query_sql(query_id, sql).await,
        Open::Schema(x) => x.query_sql(query_id, sql).await,
        Open::Catalog(x) => x.query_sql(query_id, sql).await,
    };

    result.map_err(AnyhowErrorFrontend::from).map(|x| {
        x.into_iter()
            .map(|x| match x {
                Err(e) => Err(e),
                Ok(x) => {
                    let QueryResp { fields, data } = x;
                    Ok(MuiDataGridData::from((fields, data)))
                }
            })
            .collect::<Vec<_>>()
    })
}
#[tauri::command]
pub async fn query_sql_open_connection(
    name: String, app: tauri::State<'_, App>,
) -> Result<uuid::Uuid, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await.map_err(|e| e.to_string())?;
    let result = match db {
        Open::DataBase(x) => x.open_connection().await,
        Open::Schema(x) => x.open_connection().await,
        Open::Catalog(x) => x.open_connection().await,
    };
    result.map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn query_sql_re_connection(
    name: String, query_id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await.map_err(|e| e.to_string())?;
    let result = match db {
        Open::DataBase(x) => x.re_connect(query_id).await,
        Open::Schema(x) => x.re_connect(query_id).await,
        app::Open::Catalog(x) => x.re_connect(query_id).await,
    };
    result.map_err(|e| AnyhowErrorFrontend::from(e))
}
#[tauri::command]
pub async fn query_sql_close_connection(
    name: String, query_id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await.map_err(|e| e.to_string())?;
    let result = match db {
        Open::DataBase(x) => x.close_connection(query_id).await,
        Open::Schema(x) => x.close_connection(query_id).await,
        Open::Catalog(x) => x.close_connection(query_id).await,
    };
    result.map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn save_user_settings(
    user_settings: UserSettings, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.loader
        .save_user_settings(user_settings)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn get_user_settings(
    app: tauri::State<'_, App>,
) -> Result<UserSettings, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.loader
        .get_user_settings_cloned()
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn export_table(
    name: String, table_name: String, filepath: String, kind: ExportKind,
    app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    db.export_table_to_file(table_name.as_str(), filepath.as_str(), kind)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn format_sql(
    name: String, sql: String, opts: Option<FormatSqlOpts>, app: tauri::State<'_, App>,
) -> Result<String, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_opened(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db {
        Open::DataBase(x) => x.format_sql(sql.as_str(), opts),
        Open::Schema(x) => x.format_sql(sql.as_str(), opts),
        Open::Catalog(x) => x.format_sql(sql.as_str(), opts),
    };
    ret.map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn test_connection(
    config: DBConnectionConfig,
) -> Result<
    u64, // milliseconds
    AnyhowErrorFrontend,
> {
    random_sleep().await;
    random_error()?;
    let db = app::open(&config).await.map_err(|e| e.to_string())?;
    match db {
        app::Open::DataBase(x) => x.check_conn().await,
        app::Open::Schema(x) => x.check_conn().await,
        app::Open::Catalog(x) => x.check_conn().await,
    }
    .map_err(AnyhowErrorFrontend::from)
    .map(|d| d.as_millis() as u64)
}

#[tauri::command]
pub async fn list_database(
    name: String, app: tauri::State<'_, App>,
) -> Result<
    Vec<String>, // milliseconds
    AnyhowErrorFrontend,
> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await?;
    match db {
        app::Open::Catalog(x) => x.list_database().await,
        _ => return Err(AnyhowErrorFrontend::from("not catalog trait")),
    }
    .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn list_schema(
    name: String, app: tauri::State<'_, App>,
) -> Result<
    Vec<String>, // milliseconds
    AnyhowErrorFrontend,
> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await?;
    match db {
        app::Open::DataBase(x) => x.list_schema().await,
        _ => return Err(AnyhowErrorFrontend::from("not database trait")),
    }
    .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn oracle_query_set_autocommit(
    name: String, id: uuid::Uuid, commit: bool, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x
            .query_set_autocommit(id, commit)
            .await
            .map_err(AnyhowErrorFrontend::from),
        None => Err(AnyhowErrorFrontend::from(format!(
            "db '{}' is not oracle",
            name
        ))),
    }
}
#[tauri::command]
pub async fn oracle_query_autocommit(
    name: String, id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<bool, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x
            .query_autocommit(id)
            .await
            .map_err(AnyhowErrorFrontend::from),
        None => Err(AnyhowErrorFrontend::from(format!(
            "db '{}' is not oracle",
            name
        ))),
    }
}
#[tauri::command]
pub async fn oracle_query_commit(
    name: String, id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x
            .query_commit(id)
            .await
            .map_err(|e| AnyhowErrorFrontend::from(e)),
        None => Err(format!("db '{}' is not oracle", name).into()),
    }
}
#[tauri::command]
pub async fn oracle_query_rollback(
    name: String, id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x
            .query_rollback(id)
            .await
            .map_err(|e| AnyhowErrorFrontend::from(e)),
        None => Err(format!("db '{}' is not oracle", name).into()),
    }
}

#[tauri::command]
pub async fn list_trigger(
    name: String, table: String, app: tauri::State<'_, App>,
) -> Result<Vec<NamedStaff>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;

    db.list_trigger(&table)
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn show_trigger(
    name: String, table: String, trigger: String, app: tauri::State<'_, App>,
) -> Result<TriggerDetail, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;

    db.show_trigger(&table, &trigger)
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn rename_table(
    name: String, old: String, new: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;

    db.rename_table(&old, &new)
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn duplicate_table(
    name: String, from: String, to: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;

    db.duplicate_table(&from, &to)
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn mongo_insert(
    name: String, table: String, doc: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<mongodb::results::InsertOneResult, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let doc = parse_bson_doc(doc.as_slice())?;

    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;
    match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.insert(table.as_str(), doc).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(|e| AnyhowErrorFrontend::from(e))
}
#[tauri::command]
pub async fn mongo_update(
    name: String, table: String, update: Vec<u8>, query: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<mongodb::results::UpdateResult, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let update = parse_bson_doc(update.as_slice())?;
    let query = parse_bson_doc(query.as_slice())?;

    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;
    match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.update(table.as_str(), update, query).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn mongo_delete(
    name: String, table: String, query: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<mongodb::results::DeleteResult, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(query.as_slice())?;

    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.delete(table.as_str(), query).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn mongo_show_collection_table_list(
    name: String, table: String, filter: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<MuiDataTableListResp, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let filter = parse_bson_doc(filter.as_slice())?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.show_collection_table_list(table.as_str(), filter).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(|e| AnyhowErrorFrontend::from(e))
    .map(MuiDataTableListResp::from)
}

#[tauri::command]
pub async fn mongo_show_collection_docs(
    name: String, table: String, filter: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<DocsAndCount, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let filter = parse_bson_doc(filter.as_slice())?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let docs = match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.show_collection_docs(table.as_str(), filter).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(|e| AnyhowErrorFrontend::from(e))?;
    DocsAndCount::try_from(docs).map_err(|e| e.into())
}
#[tauri::command]
pub async fn mongo_get_one_document(
    name: String, table: String, id: String, app: tauri::State<'_, App>,
) -> Result<Vec<u8>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let id = bson::oid::ObjectId::from_str(id.as_str()).map_err(AnyhowErrorFrontend::from)?;

    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let doc = match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.get_one_document(table.as_str(), id).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(|e| AnyhowErrorFrontend::from(e))?;
    Ok(encode_bson_doc(&doc).map_err(AnyhowErrorFrontend::from)?)
}

#[tauri::command]
pub async fn redis_list_lpush(
    name: String, key: String, value: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_lpush(key.as_str(), value.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_list_rpush(
    name: String, key: String, value: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_rpush(key.as_str(), value.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_list_remove(
    name: String, key: String, value: String, count: isize, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_remove(key.as_str(), count, value.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_list_set(
    name: String, key: String, value: String, index: isize, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_set(key.as_str(), index, value.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_set_replace(
    name: String, key: String, old: String, new: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let old = hex::decode(old.as_bytes()).map_err(AnyhowErrorFrontend::from)?;
    let new = hex::decode(new.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.set_replace(key.as_str(), old.as_slice(), new.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_set_del(
    name: String, key: String, member: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.set_del(key.as_str(), member.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }

        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_set_add(
    name: String, key: String, member: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.set_add(key.as_str(), member.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_hash_replace(
    name: String, key: String, old_member: String, new_member: String, new_value: String,
    app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let old_member = hex::decode(old_member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let new_member = hex::decode(new_member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;
    let new_value = hex::decode(new_value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.hash_replace(key.as_str(), old_member.as_slice(), &new_member, &new_value)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_hash_del(
    name: String, key: String, member: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.hash_del(key.as_str(), &member)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_hash_add(
    name: String, key: String, member: String, value: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;
    let value = hex::decode(value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.hash_add(key.as_str(), &member, &value)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_zset_replace(
    name: String, key: String, old_member: String, new_member: String, new_value: f64,
    app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let old_member = hex::decode(old_member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;
    let new_member = hex::decode(new_member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.zset_replace(key.as_str(), &old_member, &new_member, new_value)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_zset_del(
    name: String, key: String, member: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.zset_del(key.as_str(), &member)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}
#[tauri::command]
pub async fn redis_zset_add(
    name: String, key: String, member: String, value: f64, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.zset_add(key.as_str(), &member, value)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_string_set(
    name: String, key: String, value: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.string_set(key.as_str(), &value)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
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
        let x = Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
        };
        log::error!("msg:{},details:{}", x.reason, x.detail);
        x
    }
}

impl From<hex::FromHexError> for AnyhowErrorFrontend {
    fn from(value: hex::FromHexError) -> Self {
        let x = Self {
            reason: format!("{}", value),
            detail: "".to_string(),
        };
        log::error!("msg:{},details:{}", x.reason, x.detail);
        x
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
impl From<&str> for AnyhowErrorFrontend {
    fn from(value: &str) -> Self {
        Self {
            reason: value.to_owned(),
            detail: "".to_string(),
        }
    }
}

impl From<bson::oid::Error> for AnyhowErrorFrontend {
    fn from(value: bson::oid::Error) -> Self {
        Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
        }
    }
}

async fn random_sleep() {
    #[cfg(feature = "random-sleep")]
    {
        let duration = {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let millis = rng.gen_range(500..1700);
            let duration = std::time::Duration::from_millis(millis);
            duration
        };
        tokio::time::sleep(duration).await
    }
}

fn random_error() -> Result<(), AnyhowErrorFrontend> {
    #[cfg(not(feature = "random-error"))]
    return Ok(());
    #[cfg(feature = "random-error")]
    {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let x: i8 = rng.gen_range(1..100);
        if x > 50 {
            Err(AnyhowErrorFrontend {
                reason: format!("random error of {}", x),
                detail: "".to_string(),
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, typescript_type_def::TypeDef)]

pub struct MuiDataGridData {
    fields: Vec<Field>,
    data: Vec<HashMap<String, String>>,
}

#[derive(Clone, Debug, serde::Serialize, typescript_type_def::TypeDef)]
pub struct MuiDataTableListResp {
    pub total: usize,
    pub sql: Option<String>,
    data: MuiDataGridData,
}

impl From<TableListResp> for MuiDataTableListResp {
    fn from(value: TableListResp) -> Self {
        let TableListResp {
            fields,
            total,
            sql,
            data,
        } = value;

        Self {
            total,
            sql,
            data: MuiDataGridData::from((fields, data)),
        }
    }
}

impl From<(Vec<Field>, Vec<Vec<Option<String>>>)> for MuiDataGridData {
    fn from(value: (Vec<Field>, Vec<Vec<Option<String>>>)) -> Self {
        let (fields, old_data) = value;
        let data: Vec<_> = old_data
            .into_iter()
            .map(|x| {
                let mut m = HashMap::new();
                fields
                    .iter()
                    .zip(x.into_iter())
                    .for_each(|(f, value)| match value {
                        Some(value) => {
                            m.insert(f.field.clone(), value);
                        }

                        None => {}
                    });

                m
            })
            .collect();
        Self { fields, data }
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

    #[test]
    fn test_multi_query_resp_serde() {
        let xs: Vec<Result<QueryResp, String>> = vec![
            Err("this a error".to_string()),
            Ok(QueryResp {
                fields: vec![],
                data: vec![],
            }),
        ];

        let ret = serde_json::to_string_pretty(&xs).unwrap();
        println!("{}", ret);
    }

    #[test]
    fn test_mui_datagrid() {
        let data = vec![vec![Some("100".to_string()), Some("hello".to_string())]];
        let field = vec![
            Field::from_name_type("a", "int"),
            Field::from_name_type("b", "string"),
        ];

        let x = MuiDataGridData::from((field, data));
        let str = serde_json::to_string_pretty(&x).unwrap();
        println!("{}", str);
    }
}
