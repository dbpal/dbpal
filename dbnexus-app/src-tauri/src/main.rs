// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use dbnexus_db::app::{App, DbConfigLoader, FileJsonSaver};

mod handlers;

use handlers::{
    add_config, config_list_database_or_schema, delete, delete_config, disable_trigger, drop_table,
    drop_table_column, drop_trigger, drop_view, duplicate_table, enable_trigger, export_table,
    format_sql, get_user_settings, insert, list_all_config, list_data_types, list_schema,
    list_table, list_trigger, list_view, modify_table_column, mongo_delete, mongo_get_one_document,
    mongo_insert, mongo_show_collection_docs, mongo_show_collection_table_list, mongo_update,
    oracle_query_autocommit, oracle_query_commit, oracle_query_rollback,
    oracle_query_set_autocommit, query_sql, query_sql_close_connection, query_sql_open_connection,
    query_sql_re_connection, redis_hash_add, redis_hash_del, redis_hash_replace, redis_list_lpush,
    redis_list_remove, redis_list_rpush, redis_list_set, redis_list_table, redis_match_keys,
    redis_set_add, redis_set_del, redis_set_replace, redis_string_set, redis_zset_add,
    redis_zset_del, redis_zset_replace, rename_table, save_user_settings, show_columns,
    show_indexes, show_table, show_trigger, show_view, show_view_coumns, test_connection,
    truncate_table, update,
};
use tauri::Manager;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
async fn greet(name: String) -> Result<String, String> {
    Ok(format!("Hello, {}! You've been greeted from Rust!", name))
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let loader: Arc<dyn DbConfigLoader> = Arc::new(FileJsonSaver::new("dbnexus.json").unwrap());
    let app = App::new(loader);

    tauri::Builder::default()
        .manage(app)
        .setup(|app| {
            #[cfg(debug_assertions)] // only include this code on debug builds
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
                window.close_devtools();
            }
            Ok(())
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            add_config,
            delete_config,
            list_table,
            show_columns,
            show_table,
            show_indexes,
            insert,
            update,
            truncate_table,
            delete,
            list_view,
            drop_view,
            drop_table,
            show_view,
            query_sql,
            get_user_settings,
            save_user_settings,
            list_all_config,
            redis_list_table,
            query_sql_close_connection,
            query_sql_open_connection,
            query_sql_re_connection,
            export_table,
            oracle_query_autocommit,
            oracle_query_commit,
            oracle_query_rollback,
            oracle_query_set_autocommit,
            list_trigger,
            show_trigger,
            format_sql,
            test_connection,
            rename_table,
            duplicate_table,
            mongo_delete,
            mongo_insert,
            mongo_update,
            mongo_show_collection_docs,
            mongo_show_collection_table_list,
            redis_match_keys,
            redis_hash_add,
            redis_hash_del,
            redis_hash_replace,
            redis_list_lpush,
            redis_list_remove,
            redis_list_rpush,
            redis_list_set,
            redis_set_replace,
            redis_set_del,
            redis_set_add,
            redis_zset_add,
            redis_zset_del,
            redis_zset_replace,
            redis_string_set,
            mongo_get_one_document,
            config_list_database_or_schema,
            list_schema,
            drop_table_column,
            show_view_coumns,
            list_data_types,
            modify_table_column,
            drop_trigger,
            enable_trigger,
            disable_trigger
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
