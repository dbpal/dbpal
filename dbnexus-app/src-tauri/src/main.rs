// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use dbnexus_db::app::{App, DbConfigLoader, FileJsonSaver};

mod handlers;

use handlers::{
    add_config, delete, delete_config, drop_table, drop_view, duplicate_table, export_table,
    format_sql, get_user_settings, insert, list_all_config, list_table, list_trigger, list_view,
    mongo_delete, mongo_insert, mongo_show_table, mongo_show_table_docs, mongo_update, open,
    opened_kind, oracle_query_autocommit, oracle_query_commit, oracle_query_rollback,
    oracle_query_set_autocommit, query_sql, query_sql_close_connection, query_sql_open_connection,
    query_sql_re_connection, redis_list_table, redis_match_keys, rename_table, save_user_settings,
    show_columns, show_indexes, show_table, show_trigger, show_view, test_connection,
    truncate_table, update, update_config,
};

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
        .invoke_handler(tauri::generate_handler![
            greet,
            add_config,
            update_config,
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
            mongo_show_table,
            mongo_show_table_docs,
            open,
            opened_kind,
            redis_match_keys,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
