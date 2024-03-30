// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use dbnexus_db::init_log;
use storage::{DbConfigLoader, FileJsonSaver, DBS};

mod handlers;
mod storage;
use handlers::{
    add_config, delete, delete_config, drop_table, drop_view, insert, list_table, list_view,
    open_db, show_columns, show_indexes, show_table, show_view, truncate_table, update,
    update_config,
};

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
async fn greet(name: String) -> Result<String, String> {
    Ok(format!("Hello, {}! You've been greeted from Rust!", name))
}

#[tokio::main]
async fn main() {
    init_log();

    let loader: Arc<dyn DbConfigLoader> = Arc::new(FileJsonSaver::new("dbnexus.json").unwrap());
    let dbs = DBS::new(loader);

    tauri::Builder::default()
        .manage(dbs)
        .invoke_handler(tauri::generate_handler![
            greet,
            add_config,
            update_config,
            delete_config,
            open_db,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
