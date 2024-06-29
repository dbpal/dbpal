use std::collections::HashMap;

use dbnexus_db::db::{
    ExportKind, Field, Index, ListTable, ListTrigger, ListView, QueryResp, RedisListTable,
    SqlExecuteRet, TableListResp, TriggerDetail,
};
use dbnexus_db::oracle::OracleDB;

use crate::storage::{self, App, DBConnectionConfig, UserSettings};

#[tauri::command]
pub async fn add_config(
    config: DBConnectionConfig, dbs: tauri::State<'_, App>,
) -> Result<String, String> {
    let uuid = uuid::Uuid::new_v4().to_string();

    log::info!("add config:{} {:?}", uuid, config);
    dbs.add_config(uuid.as_str(), config)
        .await
        .map_err(|x| x.to_string())?;
    Ok(uuid)
}
#[tauri::command]
pub async fn delete_config(uuid: String, dbs: tauri::State<'_, App>) -> Result<(), String> {
    log::info!("del config:{}", uuid);
    dbs.delete_config(uuid.as_str())
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn update_config(
    uuid: String, config: DBConnectionConfig, dbs: tauri::State<'_, App>,
) -> Result<(), String> {
    log::info!("update config:{} {:?}", uuid, config);
    dbs.update_config(uuid.as_str(), config)
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn open_db(uuid: String, dbs: tauri::State<'_, App>) -> Result<(), String> {
    log::info!("open db:{}", uuid);
    dbs.open_db(uuid.as_str())
        .await
        .map_err(|x| x.to_string())
        .map(|_| ())
}

#[tauri::command]
pub async fn list_table(
    uuid: String, dbs: tauri::State<'_, App>,
) -> Result<Vec<ListTable>, String> {
    log::info!("show table db:{}", uuid);
    dbs.list_table(uuid.as_str())
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn redis_list_table(
    uuid: String, dbs: tauri::State<'_, App>,
) -> Result<RedisListTable, String> {
    dbs.get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?
        .redis_list_table()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn show_columns(
    uuid: String, table_name: String, dbs: tauri::State<'_, App>,
) -> Result<Vec<Field>, String> {
    dbs.show_columns(uuid.as_str(), &table_name)
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn show_table(
    uuid: String, table_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, dbs: tauri::State<'_, App>,
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
    uuid: String, table_name: String, dbs: tauri::State<'_, App>,
) -> Result<Vec<Index>, String> {
    dbs.show_indexes(uuid.as_str(), &table_name)
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn insert(
    uuid: String, table_name: String, values: HashMap<String, Option<String>>,
    dbs: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, String> {
    dbs.insert(uuid.as_str(), &table_name, values)
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn update(
    uuid: String, table_name: String, values: HashMap<String, Option<String>>,
    wheres: HashMap<String, Option<String>>, dbs: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, String> {
    dbs.update(uuid.as_str(), &table_name, values, wheres)
        .await
        .map_err(|x| x.to_string())
}
#[tauri::command]
pub async fn truncate_table(
    uuid: String, table_name: String, dbs: tauri::State<'_, App>,
) -> Result<(), String> {
    dbs.truncate_table(uuid.as_str(), &table_name)
        .await
        .map_err(|x| x.to_string())
}
#[tauri::command]
pub async fn delete(
    uuid: String, table_name: String, wheres: HashMap<String, Option<String>>,
    dbs: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, String> {
    dbs.delete(uuid.as_str(), &table_name, wheres)
        .await
        .map_err(|x| x.to_string())
}

#[tauri::command]
pub async fn list_view(uuid: String, dbs: tauri::State<'_, App>) -> Result<Vec<ListView>, String> {
    dbs.list_view(uuid.as_str())
        .await
        .map_err(|x| x.to_string())
}
#[tauri::command]
pub async fn drop_view(
    uuid: String, view_name: String, dbs: tauri::State<'_, App>,
) -> Result<(), String> {
    dbs.drop_view(uuid.as_str(), &view_name)
        .await
        .map_err(|x| x.to_string())
}
#[tauri::command]
pub async fn drop_table(
    uuid: String, table_name: String, dbs: tauri::State<'_, App>,
) -> Result<(), String> {
    dbs.drop_table(uuid.as_str(), &table_name)
        .await
        .map_err(|x| x.to_string())
}
#[tauri::command]
pub async fn show_view(
    uuid: String, view_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, dbs: tauri::State<'_, App>,
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
    uuid: String, db_name: String, dbs: tauri::State<'_, App>,
) -> Result<(), String> {
    dbs.open_db2(uuid.as_str(), db_name.as_str())
        .await
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn query_sql(
    uuid: String, id: uuid::Uuid, sql: String, dbs: tauri::State<'_, App>,
) -> Result<Vec<Result<QueryResp, String>>, String> {
    let sql = sql.as_str();
    let db = dbs
        .open_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;

    let result = match db {
        crate::storage::DB::Multi(x) => x.query_sql(id, sql).await,
        crate::storage::DB::One(x) => x.query_sql(id, sql).await,
    };
    result.map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn query_sql_open_connection(
    uuid: String, dbs: tauri::State<'_, App>,
) -> Result<uuid::Uuid, String> {
    let db = dbs
        .open_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let result = match db {
        crate::storage::DB::Multi(x) => x.open_connection().await,
        crate::storage::DB::One(x) => x.open_connection().await,
    };
    result.map_err(|e| e.to_string())
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
) -> Result<(), String> {
    let db = dbs
        .open_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let result = match db {
        crate::storage::DB::Multi(x) => x.close_connection(id).await,
        crate::storage::DB::One(x) => x.close_connection(id).await,
    };
    result.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_user_settings(
    user_settings: UserSettings, dbs: tauri::State<'_, App>,
) -> Result<(), String> {
    dbs.loader
        .save_user_settings(user_settings)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_user_settings(dbs: tauri::State<'_, App>) -> Result<UserSettings, String> {
    dbs.loader
        .get_user_settings_cloned()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_all_config(
    dbs: tauri::State<'_, App>,
) -> Result<HashMap<String /* uuid */, DBConnectionConfig>, String> {
    dbs.loader
        .list_all_configs_cloned()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn export_table(
    uuid: String, table_name: String, filepath: String, kind: ExportKind,
    dbs: tauri::State<'_, App>,
) -> Result<(), String> {
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    db.export_table_to_file(table_name.as_str(), filepath.as_str(), kind)
        .await
        .map_err(|e| e.to_string())
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
    String,
> {
    let db = storage::open_db(&config, Default::default())
        .await
        .map_err(|e| e.to_string())?;
    match db {
        storage::DB::Multi(x) => x.check_conn().await,
        storage::DB::One(x) => x.check_conn().await,
    }
    .map_err(|e| e.to_string())
    .map(|d| d.as_millis() as u64)
}

#[tauri::command]
pub async fn oracle_query_set_autocommit(
    uuid: String, id: uuid::Uuid, commit: bool, dbs: tauri::State<'_, App>,
) -> Result<(), String> {
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => {
            x.query_set_autocommit(id, commit)
                .await
                .map_err(|e| e.to_string())
        }
        None => Err(format!("db '{}' is not oracle", uuid)),
    }
}
#[tauri::command]
pub async fn oracle_query_autocommit(
    uuid: String, id: uuid::Uuid, dbs: tauri::State<'_, App>,
) -> Result<bool, String> {
    let db = dbs
        .get_one_db(uuid.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x.query_autocommit(id).await.map_err(|e| e.to_string()),
        None => Err(format!("db '{}' is not oracle", uuid)),
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
