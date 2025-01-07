// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use dbnexus_db::app::{App, DBConnectionConfig};
mod handlers;

use handlers::{
    add_table_column, check_conn, close_conn, config_list_database_or_schema, create_database,
    create_schema, delete, disable_trigger, drop_database, drop_index, drop_materialized_view,
    drop_procedure, drop_schema, drop_table, drop_table_column, drop_trigger, drop_udf, drop_view,
    duplicate_table, enable_trigger, export_table_or_view, format_sql, insert, list_data_types,
    list_database, list_materialized_view, list_procedure, list_schema, list_table, list_trigger,
    list_udf, list_view, modify_table_column, mongo_delete, mongo_get_one_document, mongo_insert,
    mongo_show_collection_docs, mongo_show_collection_table_list, mongo_update, open_conn,
    oracle_query_autocommit, oracle_query_commit, oracle_query_rollback,
    oracle_query_set_autocommit, query_sql, query_sql_close_connection, query_sql_open_connection,
    query_sql_re_connection, redis_hash_add, redis_hash_del, redis_hash_replace, redis_list_lpush,
    redis_list_remove, redis_list_rpush, redis_list_set, redis_list_table, redis_match_keys,
    redis_set_add, redis_set_del, redis_set_replace, redis_string_set, redis_zset_add,
    redis_zset_del, redis_zset_replace, rename_index, rename_table, server_metas, show_columns,
    show_indexes, show_procedure, show_table, show_trigger, show_udf, show_view, show_view_coumns,
    test_connection, truncate_table, update,
};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Emitter, Manager};
use tauri_plugin_store::StoreExt;

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
async fn greet(name: String) -> Result<String, String> {
    Ok(format!("Hello, {}! You've been greeted from Rust!", name))
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let app = App::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(app)
        .setup(|app| {
            #[cfg(debug_assertions)] // only include this code on debug builds
            {
                let window = app.get_webview_window("main").unwrap();
                window.open_devtools();
                // window.close_devtools();
            }
            Ok(())
        })
        .setup(|app| {
            let store = app.store("store.json").unwrap();
            *unsafe { App::get_config_ref() } = Some(Box::new(move |name| {
                let configs = store.get("connection_config");
                let json_str = match configs {
                    Some(x) => serde_json::to_string(&x).unwrap(),
                    None => "[]".to_string(),
                };
                let configs: Vec<DBConnectionConfig> = serde_json::from_str(&json_str)?;
                for c in configs.into_iter() {
                    if c.get_name() == name {
                        return anyhow::Ok(c);
                    }
                }
                anyhow::bail!(format!("config named '{}' not found", name))
            }));
            Ok(())
        })
        .menu(|handle| {
            Menu::with_items(handle, &[
                &Submenu::with_items(handle, "file", true, &[
                    &MenuItem::with_id(
                        handle,
                        "add_config",
                        "Add Connection",
                        true,
                        None as Option<&'static str>,
                    )?,
                    &MenuItem::with_id(
                        handle,
                        "show_settings",
                        "settings",
                        true,
                        None as Option<&'static str>,
                    )?,
                    #[cfg(any(target_os = "windows", target_os = "macos"))]
                    &PredefinedMenuItem::close_window(handle, None)?,
                    #[cfg(any(target_os = "windows", target_os = "macos"))]
                    &PredefinedMenuItem::quit(handle, None)?,
                ])?,
                &Submenu::with_items(handle, "Edit", true, &[
                    &PredefinedMenuItem::copy(handle, None)?,
                    &PredefinedMenuItem::paste(handle, None)?,
                    &PredefinedMenuItem::cut(handle, None)?,
                    &PredefinedMenuItem::select_all(handle, None)?,
                    &PredefinedMenuItem::undo(handle, None)?,
                    &PredefinedMenuItem::redo(handle, None)?,
                ])?,
                #[cfg(any(target_os = "windows", target_os = "macos"))]
                &Submenu::with_items(handle, "window", true, &[
                    &PredefinedMenuItem::maximize(handle, None)?,
                    &PredefinedMenuItem::minimize(handle, None)?,
                    #[cfg(any(target_os = "macos"))]
                    &PredefinedMenuItem::fullscreen(handle, None)?,
                ])?,
                &Submenu::with_items(handle, "about", true, &[&PredefinedMenuItem::about(
                    handle, None, None,
                )?])?,
            ])
        })
        .on_menu_event(|handle, event| {
            log::info!("on_menu_event:{:?}", event);
            if event.id.0 == "add_config" {
                handle.emit("add_connection", "").unwrap();
            }
            if event.id.0 == "show_settings" {
                handle.emit("show_settings", "").unwrap();
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            create_database,
            create_schema,
            drop_database,
            drop_schema,
            list_database,
            greet,
            list_table,
            rename_index,
            show_columns,
            show_table,
            insert,
            update,
            truncate_table,
            delete,
            list_view,
            drop_view,
            drop_table,
            show_view,
            query_sql,
            redis_list_table,
            query_sql_close_connection,
            query_sql_open_connection,
            query_sql_re_connection,
            export_table_or_view,
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
            disable_trigger,
            list_procedure,
            show_procedure,
            drop_procedure,
            list_udf,
            show_udf,
            drop_udf,
            add_table_column,
            check_conn,
            close_conn,
            show_indexes,
            drop_index,
            list_materialized_view,
            open_conn,
            drop_materialized_view,
            server_metas,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
