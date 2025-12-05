#![warn(clippy::unwrap_used)]
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod handlers;
use crate::app::generate_random_string;
use crate::handlers::TaskManager;
use app::DBPal;
use handlers::{
    add_table_column, ai_get_search_path, ai_text_completion, all_font_family, auto_completion,
    cancel_task, cassandra_drop_aggregate, cassandra_list_aggregate, cassandra_show_aggregate,
    check_conn, check_sql, close_conn, command_panic, conn_opened, create_database, create_index,
    create_schema, create_table_and_index_sql, create_view_sql, disable_trigger, drop_created_type,
    drop_database, drop_event, drop_index, drop_materialized_view, drop_procedure, drop_schema,
    drop_sequence, drop_table, drop_table_column, drop_trigger, drop_udf, drop_view,
    duplicate_table, enable_trigger, establish_conn, execute_sql, execute_sql_no_open_conn,
    export_query_result_to_file, export_table_or_view, extract_table_select_selection,
    extract_table_select_sort, get_locale, get_redis_doc, get_redis_type_hint, is_dangerous_sql,
    list_created_types, list_data_types, list_database, list_event, list_materialized_view,
    list_procedure, list_schema, list_sequence, list_sql_current_values, list_table, list_udf,
    list_view, modify_table_column, monaco_editor_themes, mongo_create_index, mongo_delete_ids,
    mongo_get_one_document_by_id, mongo_insert, mongo_list_time_serial_collection,
    mongo_query_collection_count_docs, mongo_query_collection_create_index,
    mongo_query_collection_create_search_index, mongo_query_collection_delete_many,
    mongo_query_collection_delete_one, mongo_query_collection_distinct,
    mongo_query_collection_drop, mongo_query_collection_drop_index,
    mongo_query_collection_drop_indexes, mongo_query_collection_drop_search_index,
    mongo_query_collection_find, mongo_query_collection_find_one,
    mongo_query_collection_find_one_and_delete, mongo_query_collection_find_one_and_replace,
    mongo_query_collection_insert_many, mongo_query_collection_insert_one,
    mongo_query_collection_list_index_names, mongo_query_collection_list_indexes,
    mongo_query_collection_list_search_indexes, mongo_query_collection_replace_one,
    mongo_query_collection_update_many, mongo_query_collection_update_one, mongo_query_db_drop,
    mongo_query_db_list_collection_names, mongo_query_db_run_command, mongo_show_collection_docs,
    mongo_show_collection_table_list, mongo_update_one, page_init_callback, put_value_in_sql,
    query_commit, query_get_commit, query_rollback, query_set_autocommit, query_sql,
    query_sql_close_connection, query_sql_open_connection, redis_hash_add, redis_hash_del,
    redis_hash_replace, redis_list_lpush, redis_list_remove, redis_list_rpush, redis_list_set,
    redis_list_table, redis_set_add, redis_set_del, redis_set_replace, redis_string_set,
    redis_zset_add, redis_zset_del, redis_zset_replace, rename_index, rename_sequence,
    rename_table, server_metas, show_indexes, show_procedure, show_table, show_table_columns,
    show_trigger, show_udf, show_view_columns, table_delete, table_insert, table_update,
    test_connection, truncate_table, try_get_max_id, try_list_database_schema_candidates,
    valid_json,
};
use std::io::Write;
use std::panic;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::OnceLock;
use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_log::{Target, TargetKind};
use tauri_plugin_store::StoreExt;
mod migration;
use rustls::crypto::{CryptoProvider, aws_lc_rs};
use std::fs::OpenOptions;
use std::sync::Once;

static HOOK_SET: Once = Once::new();

fn setup_panic_hook(dir: PathBuf) {
    HOOK_SET.call_once(|| {
        panic::set_hook(Box::new(move |panic_info| {
            eprintln!("{}", std::backtrace::Backtrace::capture());
            // 1. 提取信息（防御式，每一步都可能失败）
            let payload = panic_info
                .payload()
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| panic_info.payload().downcast_ref::<&str>().copied())
                .unwrap_or("Unknown panic");

            let location = panic_info
                .location()
                .map(|loc| format!("{}:{}:{}", loc.file(), loc.line(), loc.column()))
                .unwrap_or_else(|| "Unknown location".to_string());
            let current_thread = std::thread::current();
            let err = std::fs::create_dir_all(&dir);
            if let Err(err) = err {
                eprintln!("create dir failed,err:{:?}", err);
            }
            let thread = current_thread.name().unwrap_or("unnamed");
            let mut filepath = dir.clone();
            filepath = filepath.join("dbpal.panic");
            match OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(&filepath)
            {
                Ok(mut file) => {
                    //   忽略所有错误，避免二次 panic
                    let err = writeln!(
                        file,
                        "[{}] Thread '{}' panicked at '{}': {}",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                        thread,
                        location,
                        payload
                    );
                    if let Err(err) = err {
                        eprintln!("write backtrace failed,err:{:?}", err);
                    }
                    let err = writeln!(file, "{}", std::backtrace::Backtrace::capture());
                    if let Err(err) = err {
                        eprintln!("write backtrace failed,err:{:?}", err);
                    }
                }
                Err(err) => {
                    eprintln!("create file failed,err:{:?}", err);
                }
            }
            // 4. 立即退出（不尝试任何 UI 操作）
            std::process::exit(101); // 101 是 panic 退出码惯例
        }));
    });
}

// is this macbook computer
#[inline(always)]
fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

static APP_HANDLE: OnceLock<AppHandle<Wry>> = OnceLock::new();

fn get_logger_level() -> Option<log::Level> {
    match APP_HANDLE.get() {
        None => None,
        Some(handle) => match handle.store(STORE_FILE) {
            Ok(x) => {
                let x = x.get("logger-level");
                let r = x.and_then(|x| match x {
                    serde_json::Value::String(level) => match level.as_str() {
                        "none" => None,
                        _ => match log::Level::from_str(&level) {
                            Ok(level) => Some(level),
                            Err(err) => {
                                println!(
                                    "parse level failed,err:{:?} this is a bug from frontend",
                                    err
                                );
                                None
                            }
                        },
                    },
                    _ => None,
                });
                r
            }
            Err(err) => {
                eprint!("get store failed,err:{:?}", err);
                None
            }
        },
    }
}

const STORE_FILE: &str = "store.json";

mod zoom {
    use crate::STORE_FILE;
    use tauri::{AppHandle, Manager};
    use tauri_plugin_store::StoreExt;
    pub(super) const STEP: f64 = 0.03;

    pub(crate) fn get_configured_zoom(handle: &AppHandle) -> anyhow::Result<f64> {
        let x = handle
            .get_store(STORE_FILE)
            .map(|store| store.get("zoom").and_then(|x| x.as_f64()));
        Ok(x.flatten().unwrap_or(1.0))
    }

    pub(crate) fn configure_zoom(handle: &AppHandle, zoom: f64) -> anyhow::Result<()> {
        let store = handle.get_store(STORE_FILE);
        if let Some(store) = store {
            store.set("zoom", zoom);
        }
        Ok(())
    }
    pub(crate) fn set_webview_zoom(handle: &AppHandle, zoom: f64) {
        let views = handle.webview_windows();
        for view in views.values() {
            if let Err(err) = view.set_zoom(zoom) {
                log::error!("set view zoom failed:{}", err);
            }
        }
    }
}

// api key sk-2zegNGK2WhxZ9jToc17JBp260Z5vD5Cdd4zXG4s4WISRyczO
// https://api.moonshot.cn/v1

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("should build runtime");

    tauri::async_runtime::set(runtime.handle().clone());

    // https://github.com/tauri-apps/tauri/issues/4788
    let _ = fix_path_env::fix();
    CryptoProvider::install_default(aws_lc_rs::default_provider())
        .expect("Failed to install default crypto provider");
    let dbpal = DBPal::new();
    let task_manager = TaskManager::default();
    let setup = move |app: &mut tauri::App| {
        setup_panic_hook(
            app.path()
                .app_log_dir()
                .expect("should be able to get app data dir"),
        );
        APP_HANDLE.set(app.handle().clone()).unwrap();
        #[cfg(debug_assertions)] // only include this code on debug builds
        {
            let window = app
                .get_webview_window("main")
                .expect("should be able to get main window");
            window.open_devtools();
        }
        Ok(())
    };
    let setting_window_id = generate_random_string(10);
    let builder = tauri::Builder::default().plugin(
        tauri_plugin_log::Builder::new()
            .level(tauri_plugin_log::log::LevelFilter::Info)
            .build(),
    );
    let about_meta = AboutMetadata {
        website: Some("https://dbpal.site".to_string()),
        name: Some("dbpal".to_string()),
        authors: Some(vec![
            "756445638@qq.com".to_string(),
            "yuyangxxxmail@gmail.com".to_string(),
        ]),
        copyright: Some("Copyright © yuyang 2025 All rights reserved.".to_string()),
        ..AboutMetadata::default()
    };
    builder
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations("sqlite:dbpal.sqlite.db", migration::migrations())
                .build(),
        )
        .plugin(
            tauri_plugin_log::Builder::new()
                .filter(|meta| {
                    let level = dbg!(get_logger_level());
                    match level {
                        Some(level) => dbg!(level >= meta.level()),
                        None => false,
                    }
                })
                .targets(vec![
                    Target::new(TargetKind::LogDir { file_name: None }),
                    Target::new(TargetKind::Stdout),
                ])
                .build(),
        )
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_keyring::init())
        .plugin(tauri_plugin_http::init())
        .manage(dbpal)
        .manage(task_manager)
        .setup(setup)
        .menu(|handle| {
            Menu::with_items(
                handle,
                &[
                    &Submenu::with_items(
                        handle,
                        "file",
                        true,
                        &[
                            &MenuItem::with_id(
                                handle,
                                "add_config",
                                "Add Connection",
                                true,
                                Some(if is_macos() { "command+n" } else { "control+n" }),
                            )?,
                            &MenuItem::with_id(
                                handle,
                                "show_setting",
                                "settings",
                                true,
                                Some(if is_macos() { "command+," } else { "control+," }),
                            )?,
                            &MenuItem::with_id(
                                handle,
                                "relauch",
                                "relauch",
                                true,
                                None as Option<&'static str>,
                            )?,
                            #[cfg(any(target_os = "windows", target_os = "macos"))]
                            &PredefinedMenuItem::close_window(handle, None)?,
                            #[cfg(any(target_os = "windows", target_os = "macos"))]
                            &PredefinedMenuItem::quit(handle, None)?,
                        ],
                    )?,
                    &Submenu::with_items(
                        handle,
                        "Edit",
                        true,
                        &[
                            &PredefinedMenuItem::copy(handle, None)?,
                            &PredefinedMenuItem::paste(handle, None)?,
                            &PredefinedMenuItem::cut(handle, None)?,
                            &PredefinedMenuItem::select_all(handle, None)?,
                            &PredefinedMenuItem::undo(handle, None)?,
                            &PredefinedMenuItem::redo(handle, None)?,
                        ],
                    )?,
                    &Submenu::with_items(
                        handle,
                        "window",
                        true,
                        &[
                            #[cfg(any(target_os = "windows", target_os = "macos"))]
                            &PredefinedMenuItem::maximize(handle, None)?,
                            #[cfg(any(target_os = "windows", target_os = "macos"))]
                            &PredefinedMenuItem::minimize(handle, None)?,
                            #[cfg(target_os = "macos")]
                            &PredefinedMenuItem::fullscreen(handle, None)?,
                            // #[cfg(any(target_os = "macos"))]
                            // &PredefinedMenuItem::show_all(handle, None)?,
                            #[cfg(any(target_os = "macos", target_os = "windows",))]
                            &PredefinedMenuItem::hide(handle, None)?,
                            &MenuItem::with_id(
                                handle,
                                "zoom_in",
                                "zoom in",
                                true,
                                Some(if is_macos() { "command+-" } else { "ctrl+-" }),
                            )?,
                            &MenuItem::with_id(
                                handle,
                                "zoom_out",
                                "zoom out",
                                true,
                                Some(if is_macos() { "command+=" } else { "ctrl+=" }),
                            )?,
                            &MenuItem::with_id(
                                handle,
                                "zoom_reset",
                                "reset zoom",
                                true,
                                None as Option<&'static str>,
                            )?,
                        ],
                    )?,
                    &Submenu::with_items(
                        handle,
                        "about",
                        true,
                        &[&PredefinedMenuItem::about(
                            handle,
                            Some("make database management easy."),
                            Some(about_meta),
                        )?],
                    )?,
                ],
            )
        })
        .on_menu_event(move |handle, event| {
            log::info!("on menu event:{:?}", event);
            if event.id.0 == "add_config" {
                if let Err(err) = handle.emit("add_config", "") {
                    log::error!("emit add_config failed:{}", err);
                }
            }
            if event.id.0 == "show_setting" {
                if let Some(window) = handle.get_webview_window(&setting_window_id) {
                    if let Err(err) = window.set_focus() {
                        log::error!("set focus failed:{}", err);
                    }
                } else if let Err(err) = WebviewWindowBuilder::new(
                    handle,
                    setting_window_id.clone(),
                    WebviewUrl::App("setting.html".to_string().into()),
                )
                .title("setting")
                .inner_size(800f64, 600f64)
                .maximized(true)
                .build()
                {
                    log::error!("create setting window failed:{}", err);
                }
            }
            if event.id.0 == "relauch" {
                handle.restart();
            }
            if event.id.0 == "zoom_in" {
                match zoom::get_configured_zoom(handle) {
                    Ok(x) => {
                        let mut zoom = x - zoom::STEP;
                        let min = 0.3;
                        if zoom < min {
                            zoom = min;
                        }
                        zoom::set_webview_zoom(handle, zoom);
                        if let Err(err) = zoom::configure_zoom(handle, zoom) {
                            log::error!("set zoom failed:{}", err);
                        }
                    }
                    Err(err) => {
                        log::error!("get zoom failed,err{}", err);
                    }
                }
            }
            if event.id.0 == "zoom_out" {
                match zoom::get_configured_zoom(handle) {
                    Ok(x) => {
                        let mut zoom = x + zoom::STEP;
                        let max = 2f64;
                        if zoom > max {
                            zoom = max;
                        }
                        zoom::set_webview_zoom(handle, zoom);
                        if let Err(err) = zoom::configure_zoom(handle, zoom) {
                            log::error!("set zoom failed:{}", err);
                        }
                    }
                    Err(err) => {
                        log::error!("get zoom failed,err{}", err);
                    }
                }
            }
            if event.id.0 == "zoom_reset" {
                let zoom = 1f64;
                zoom::set_webview_zoom(handle, zoom);
                if let Err(err) = zoom::configure_zoom(handle, zoom) {
                    log::error!("set zoom failed:{}", err);
                }
            }
        })
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            create_view_sql,
            create_database,
            create_schema,
            drop_database,
            drop_schema,
            list_database,
            list_table,
            rename_index,
            show_table_columns,
            show_table,
            table_insert,
            table_update,
            truncate_table,
            table_delete,
            list_view,
            drop_view,
            drop_table,
            query_sql,
            redis_list_table,
            query_sql_close_connection,
            query_sql_open_connection,
            export_table_or_view,
            show_trigger,
            test_connection,
            rename_table,
            duplicate_table,
            mongo_delete_ids,
            mongo_insert,
            mongo_update_one,
            mongo_show_collection_docs,
            mongo_show_collection_table_list,
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
            mongo_get_one_document_by_id,
            try_list_database_schema_candidates,
            list_schema,
            drop_table_column,
            show_view_columns,
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
            establish_conn,
            drop_materialized_view,
            server_metas,
            auto_completion,
            is_dangerous_sql,
            export_query_result_to_file,
            page_init_callback,
            mongo_query_collection_count_docs,
            mongo_query_collection_create_index,
            mongo_query_collection_create_search_index,
            mongo_query_collection_delete_many,
            mongo_query_collection_delete_one,
            mongo_query_collection_distinct,
            mongo_query_collection_drop,
            mongo_query_collection_drop_index,
            mongo_query_collection_drop_indexes,
            mongo_query_collection_drop_search_index,
            mongo_query_db_drop,
            mongo_query_db_list_collection_names,
            mongo_query_db_run_command,
            mongo_query_collection_find,
            mongo_query_collection_find_one,
            mongo_query_collection_find_one_and_delete,
            mongo_query_collection_find_one_and_replace,
            mongo_query_collection_insert_many,
            mongo_query_collection_insert_one,
            mongo_query_collection_list_index_names,
            mongo_query_collection_list_indexes,
            mongo_query_collection_list_search_indexes,
            mongo_query_collection_replace_one,
            mongo_query_collection_update_many,
            mongo_query_collection_update_one,
            create_index,
            mongo_list_time_serial_collection,
            mongo_create_index,
            put_value_in_sql,
            cassandra_drop_aggregate,
            cassandra_list_aggregate,
            cassandra_show_aggregate,
            list_created_types,
            conn_opened,
            create_table_and_index_sql,
            execute_sql,
            query_get_commit,
            query_set_autocommit,
            query_commit,
            query_rollback,
            valid_json,
            list_sequence,
            drop_sequence,
            rename_sequence,
            command_panic,
            all_font_family,
            list_sql_current_values,
            check_sql,
            execute_sql_no_open_conn,
            drop_created_type,
            list_event,
            drop_event,
            extract_table_select_selection,
            extract_table_select_sort,
            get_redis_doc,
            monaco_editor_themes,
            ai_text_completion,
            get_redis_type_hint,
            get_locale,
            try_get_max_id,
            ai_get_search_path,
            cancel_task,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
