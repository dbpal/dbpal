use dbnexus_db::db::ListTable;

use crate::storage::{DBConnectionConfig, DBS};

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
