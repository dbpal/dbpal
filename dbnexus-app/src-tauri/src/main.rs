// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;

use dbnexus_db::init_log;
use storage::{DbConfigLoader, FileJsonSaver, DBS};

mod handlers;
mod storage;
use handlers::{add_config, list_table, open_db};

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
            greet, add_config, open_db, list_table
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
