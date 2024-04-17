use std::collections::HashMap;

use dbnexus_db::db::{
    ExeSqlError, Field, Index, ListTable, ListView, SqlExecuteRet, TableListResp,
};

use crate::storage::{DBConnectionConfig, UserSettings, DBS};

#[tauri::command]
pub async fn add_config(
    uuid: String, config: DBConnectionConfig, dbs: tauri::State<'_, DBS>,
) -> Result<(), String> {
    log::info!("add config:{} {:?}", uuid, config);
    dbs.add_config(uuid.as_str(), config)
        .await
        .map_err(|x| x.to_string())
}
#[tauri::command]
pub async fn delete_config(uuid: String, dbs: tauri::State<'_, DBS>) -> Result<(), String> {
    log::info!("del config:{}", uuid);
    dbs.delete_config(uuid.as_str())
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn update_config(
    uuid: String, config: DBConnectionConfig, dbs: tauri::State<'_, DBS>,
) -> Result<(), String> {
    log::info!("update config:{} {:?}", uuid, config);
    dbs.update_config(uuid.as_str(), config)
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn open_db(uuid: String, dbs: tauri::State<'_, DBS>) -> Result<(), String> {
    log::info!("open db:{}", uuid);
    dbs.open_db(uuid.as_str())
        .await
        .map_err(|x| x.to_string())
        .map(|_| ())
}

#[tauri::command]
pub async fn list_table(
    uuid: String, dbs: tauri::State<'_, DBS>,
) -> Result<Vec<ListTable>, String> {
    log::info!("show table db:{}", uuid);
    dbs.list_table(uuid.as_str())
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn show_columns(
    uuid: String, table_name: String, dbs: tauri::State<'_, DBS>,
) -> Result<Vec<Field>, String> {
    dbs.show_columns(uuid.as_str(), &table_name)
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn show_table(
    uuid: String, table_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, dbs: tauri::State<'_, DBS>,
) -> Result<TableListResp, String> {
    dbs.show_table(
        uuid.as_str(),
        &table_name,
        where_str,
        order_by,
        page,
        page_size,
    )
    .await
    .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn show_indexes(
    uuid: String, table_name: String, dbs: tauri::State<'_, DBS>,
) -> Result<Vec<Index>, String> {
    dbs.show_indexes(uuid.as_str(), &table_name)
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn insert(
    uuid: String, table_name: String, values: HashMap<String, Option<String>>,
    dbs: tauri::State<'_, DBS>,
) -> Result<SqlExecuteRet, String> {
    dbs.insert(uuid.as_str(), &table_name, values)
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn update(
    uuid: String, table_name: String, values: HashMap<String, Option<String>>,
    wheres: HashMap<String, Option<String>>, dbs: tauri::State<'_, DBS>,
) -> Result<SqlExecuteRet, String> {
    dbs.update(uuid.as_str(), &table_name, values, wheres)
        .await
        .map_err(|x| x.to_string())
}
#[tauri::command]
pub async fn truncate_table(
    uuid: String, table_name: String, dbs: tauri::State<'_, DBS>,
) -> Result<(), String> {
    dbs.truncate_table(uuid.as_str(), &table_name)
        .await
        .map_err(|x| x.to_string())
}
#[tauri::command]
pub async fn delete(
    uuid: String, table_name: String, wheres: HashMap<String, Option<String>>,
    dbs: tauri::State<'_, DBS>,
) -> Result<SqlExecuteRet, String> {
    dbs.delete(uuid.as_str(), &table_name, wheres)
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn list_view(uuid: String, dbs: tauri::State<'_, DBS>) -> Result<Vec<ListView>, String> {
    dbs.list_view(uuid.as_str())
        .await
        .map_err(|x| x.to_string())
}
#[tauri::command]
pub async fn drop_view(
    uuid: String, view_name: String, dbs: tauri::State<'_, DBS>,
) -> Result<(), String> {
    dbs.drop_view(uuid.as_str(), &view_name)
        .await
        .map_err(|x| x.to_string())
}
#[tauri::command]
pub async fn drop_table(
    uuid: String, table_name: String, dbs: tauri::State<'_, DBS>,
) -> Result<(), String> {
    dbs.drop_table(uuid.as_str(), &table_name)
        .await
        .map_err(|x| x.to_string())
}
#[tauri::command]
pub async fn show_view(
    uuid: String, view_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, dbs: tauri::State<'_, DBS>,
) -> Result<TableListResp, String> {
    dbs.show_view(
        uuid.as_str(),
        &view_name,
        where_str,
        order_by,
        page,
        page_size,
    )
    .await
    .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn open_db2(
    uuid: String, db_name: String, dbs: tauri::State<'_, DBS>,
) -> Result<(), String> {
    dbs.open_db2(uuid.as_str(), db_name.as_str())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn query_sql(
    uuid: String, sql: String, dbs: tauri::State<'_, DBS>,
) -> Result<Vec<Result<TableListResp, ExeSqlError>>, String> {
    let sql = sql.as_str();
    let db = dbs
        .open_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;

    let result = match db {
        crate::storage::DB::Multi(x) => x.query_sql(sql).await,
        crate::storage::DB::One(x) => x.query_sql(sql).await,
    };
    result.map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn save_user_settings(
    user_settings: UserSettings, dbs: tauri::State<'_, DBS>,
) -> Result<(), String> {
    dbs.loader
        .save_user_settings(&user_settings)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_user_settings(dbs: tauri::State<'_, DBS>) -> Result<UserSettings, String> {
    dbs.loader
        .get_user_settings_cloned()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_all_config(
    dbs: tauri::State<'_, DBS>,
) -> Result<HashMap<String, HashMap<String, String>>, String> {
    dbs.loader
        .list_all_configs_cloned()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn exit(code: i32) -> Result<(), String> {
    std::process::exit(0);
    Ok(())
}
