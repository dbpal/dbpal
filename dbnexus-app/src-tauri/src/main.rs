// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod handlers;
use std::backtrace::Backtrace;
use std::io::Write;
use std::ops::DerefMut;
use std::panic;
use std::str::FromStr;
use std::sync::Arc;

use crate::app::generate_random_string;
use crate::handlers::{log_file, truncate_file};
use app::{DBConnectionConfig, DBPal};
use handlers::{
    PASSWORD_KEY, add_table_column, auto_completion, cassandra_drop_aggregate,
    cassandra_invalid_table_or_view_caching, cassandra_list_aggregate, cassandra_show_aggregate,
    check_conn, close_conn, command_panic, config_list_database_or_schema, conn_opened,
    create_database, create_index, create_schema, create_table_and_index_sql, create_view_sql,
    delete_password, disable_trigger, drop_created_type, drop_database, drop_event, drop_index,
    drop_materialized_view, drop_procedure, drop_schema, drop_sequence, drop_table,
    drop_table_column, drop_trigger, drop_udf, drop_view, duplicate_table, enable_trigger,
    execute_sql, execute_sql_no_open_conn, export_query_result_to_file, export_table_or_view,
    extract_table_select_selection, extract_table_select_sort, fonts_all_families,
    get_backend_crash, get_password, has_entered_password, is_dangerous_sql, key_words,
    list_created_types, list_data_types, list_database, list_event, list_materialized_view,
    list_procedure, list_schema, list_sequence, list_sql_current_values, list_table, list_udf,
    list_view, modify_table_column, mongo_create_index, mongo_delete_ids,
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
    mongo_show_collection_table_list, mongo_update_one, open_conn, page_init_callback,
    put_value_in_sql, query_commit, query_get_commit, query_rollback, query_set_autocommit,
    query_sql, query_sql_close_connection, query_sql_open_connection, query_sql_re_connection,
    redis_hash_add, redis_hash_del, redis_hash_replace, redis_list_lpush, redis_list_remove,
    redis_list_rpush, redis_list_set, redis_list_table, redis_set_add, redis_set_del,
    redis_set_replace, redis_string_set, redis_zset_add, redis_zset_del, redis_zset_replace,
    redis_zset_show_table, rename_index, rename_sequence, rename_table, save_entered_password,
    save_password, server_metas, show_indexes, show_procedure, show_table, show_table_columns,
    show_trigger, show_udf, show_view, show_view_columns, sql_parse_error, table_delete,
    table_insert, table_update, test_connection, truncate_log_file, truncate_table, valid_json,
};
use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_store::{Store, StoreExt};

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(4 * 1024 * 1024)
        .build()
        .unwrap();

    runtime.block_on(async {
        main_();
    })
}

// is this macbook computer
#[inline(always)]
fn is_mac() -> bool {
    #[cfg(target_os = "macos")]
    return true;
    #[cfg(not(target_os = "macos"))]
    return false;
}

pub struct AppLogger {
    config: Arc<Store<Wry>>,
}

impl AppLogger {
    fn get_config(&self) -> Option<serde_json::Value> {
        self.config.get("user-settings")
    }
}

#[allow(dead_code)]
fn logger_test() {
    std::thread::spawn(|| {
        loop {
            log::trace!("this is trace log");
            log::debug!("this is debug log");
            log::info!("this is info log");
            log::warn!("this is warn log");
            log::error!("this is error log");
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    });
}

impl AppLogger {
    fn get_level(&self) -> log::Level {
        let x = self.get_config();
        x.as_ref()
            .and_then(|x| match x {
                serde_json::Value::Object(map) => match map.get("loggerLevel") {
                    Some(serde_json::Value::String(level)) => match log::Level::from_str(level) {
                        Ok(level) => Some(level),
                        Err(err) => {
                            println!(
                                "parse level failed,err:{:?} this is a bug from frontend",
                                err
                            );
                            None
                        }
                    },
                    _ => None,
                },
                _ => None,
            })
            .unwrap_or(log::Level::Info)
    }
    fn get_max_size(&self) -> u64 {
        let x = self.get_config();
        x.and_then(|x| match x {
            serde_json::Value::Object(map) => match map.get("loggerMaxSize") {
                Some(serde_json::Value::Number(max)) => {
                    Some(max.as_u64().unwrap_or(5) * 1024 * 1024)
                }
                _ => None,
            },
            _ => None,
        })
        .unwrap_or(5 * 1024 * 1024)
    }
}

impl log::Log for AppLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.get_level()
    }
    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let mut lock = log_file().lock().unwrap();
        let file = match lock.deref_mut() {
            Some(x) => x,
            None => return,
        };
        match file.metadata() {
            Ok(meta) => {
                if meta.len() > self.get_max_size() {
                    match truncate_file(file) {
                        Ok(_) => {}
                        Err(err) => {
                            println!("truncate file failed,err:{}", err);
                            return;
                        }
                    }
                }
            }
            Err(err) => {
                println!("get file metadata failed,err:{}", err);
                return;
            }
        }
        let s = format!(
            "{} {} {}-{}\n",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
            if cfg!(debug_assertions) {
                format!(
                    "{}:{}",
                    record.file().unwrap_or_default(),
                    record.line().unwrap_or_default(),
                )
            } else {
                "".to_string()
            },
            record.level(),
            record.args()
        );
        match file.write_all(s.as_bytes()) {
            Ok(_) => {}
            Err(err) => println!("write file failed,err:{:?}", err),
        }
    }
    fn flush(&self) {
        let mut lock = log_file().lock().unwrap();
        let file = match lock.deref_mut() {
            Some(x) => x,
            None => return,
        };
        if let Err(err) = file.flush() {
            println!("flush file failed,err:{:?}", err)
        }
    }
}

const STORE_FILE: &str = "store.json";

fn get_configured_zoom(handle: &AppHandle) -> anyhow::Result<f64> {
    let x = handle
        .get_store(STORE_FILE)
        .map(|store| store.get("zoom").and_then(|x| x.as_f64()));
    Ok(x.flatten().unwrap_or(1.0))
}

fn config_zoom(handle: &AppHandle, zoom: f64) -> anyhow::Result<()> {
    let store = handle.get_store(STORE_FILE);
    if let Some(store) = store {
        store.set("zoom", zoom);
    }
    Ok(())
}

fn set_webview_zoom(handle: &AppHandle, zoom: f64) {
    let views = handle.webview_windows();
    for view in views.values() {
        if let Err(err) = view.set_zoom(zoom) {
            log::error!("set view  zoom failed:{}", err);
        }
    }
}
fn main_() {
    // logger_test();
    let dbpal = DBPal::new();
    let dbpal_clone = dbpal.clone();

    let setup = move |app: &mut tauri::App| {
        {
            let dbpal_clone = dbpal_clone.clone();
            let handler = app.handle().clone();
            let old = panic::take_hook();
            panic::set_hook(Box::new(move |info| {
                let backtrace = Backtrace::capture();
                let id = dbpal_clone.set_backend_crash_msg(format!("{}\n{}", info, backtrace));
                if let Err(err) = WebviewWindowBuilder::new(
                    &handler,
                    generate_random_string(10),
                    WebviewUrl::App(format!("backend-crash.html?id={id}").into()),
                )
                .title("Backend Crash")
                .inner_size(800f64, 600f64)
                .maximized(true)
                .build()
                {
                    log::error!("create backend crash window failed:{}", err);
                }
                old(info);
            }));
        }
        #[cfg(debug_assertions)] // only include this code on debug builds
        {
            let window = app.get_webview_window("main").unwrap();
            window.open_devtools();
        }
        let store = app.store(STORE_FILE).unwrap();
        match app.path().app_data_dir() {
            Ok(mut f_path) => {
                f_path.push("dbpal.log");
                let mut ops = std::fs::OpenOptions::new();
                ops.write(true);
                ops.create(true);
                match ops.open(f_path) {
                    Ok(file) => {
                        let logger = AppLogger {
                            config: store.clone(),
                        };
                        *log_file().lock().unwrap() = Some(file);
                        log::set_boxed_logger(Box::new(logger)).unwrap();
                        log::set_max_level(log::LevelFilter::Trace);
                    }
                    Err(err) => println!("can't open file for write,err:{}", err),
                }
            }
            Err(err) => println!("can't get app data dir,err:{}", err),
        }

        *unsafe { DBPal::get_config_ref() } = Some(Box::new(move |name| {
            let configs: Vec<DBConnectionConfig> = serde_json::from_value(
                store
                    .get("connection-config")
                    .unwrap_or(serde_json::Value::Array(vec![])),
            )?;

            for mut c in configs.into_iter() {
                if c.get_name() != name {
                    continue;
                }
                if c.need_password() {
                    if let Some(pass) = dbpal_clone.saved_password.get(&c.get_name().to_string()) {
                        c.set_password(pass.clone());
                    } else {
                        match keyring::Entry::new(PASSWORD_KEY, name) {
                            Ok(en) => match en.get_password() {
                                Ok(pass) => c.set_password(pass),
                                Err(err) => {
                                    log::error!("get password for '{}' failed,err:{}", name, err)
                                }
                            },
                            Err(err) => {
                                log::error!(
                                    "get password entry failed:{} maybe user don't set a password should be fine",
                                    err
                                )
                            }
                        };
                    }
                }
                return anyhow::Ok(c);
            }
            anyhow::bail!(format!("config named '{}' not found", name))
        }));
        Ok(())
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_sql::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .manage(dbpal)
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
                                None as Option<&'static str>,
                            )?,
                            &MenuItem::with_id(
                                handle,
                                "show_settings",
                                "settings",
                                true,
                                Some(if is_mac() { "command+," } else { "control+," }),
                            )?,
                            #[cfg(debug_assertions)]
                            &MenuItem::with_id(
                                handle,
                                "edit_config_file",
                                "edit config file",
                                true,
                                None as Option<&'static str>,
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
                            #[cfg(any(target_os = "macos"))]
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
                                Some(if is_mac() { "command+-" } else { "ctrl+-" }),
                            )?,
                            &MenuItem::with_id(
                                handle,
                                "zoom_out",
                                "zoom out",
                                true,
                                Some(if is_mac() { "command+=" } else { "ctrl+=" }),
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
                        &[
                            &MenuItem::with_id(
                                handle,
                                "open_dev_tools",
                                "open dev tools",
                                true,
                                None as Option<&'static str>,
                            )?,
                            &MenuItem::with_id(
                                handle,
                                "open_log_file",
                                "open log file",
                                true,
                                None as Option<&'static str>,
                            )?,
                            &PredefinedMenuItem::about(
                                handle,
                                Some("make database management easy."),
                                Some({
                                    let mut meta = AboutMetadata::default();
                                    meta.website = Some("https://dbpal.site".to_string());
                                    meta.name = Some("dbpal".to_string());
                                    meta.authors = Some(vec![
                                        "756445638@qq.com".to_string(),
                                        "yuyangxxxmail@gmail.com".to_string(),
                                    ]);
                                    meta.copyright = Some(
                                        "Copyright © yuyang 2025 All rights reserved.".to_string(),
                                    );
                                    meta
                                }),
                            )?,
                        ],
                    )?,
                ],
            )
        })
        .on_menu_event(|handle, event| {
            log::info!("on menu event:{:?}", event);
            if event.id.0 == "add_config" {
                handle.emit("add_config", "").unwrap();
            }
            if event.id.0 == "show_settings" {
                handle.emit("show_settings", "").unwrap();
            }
            if event.id.0 == "edit_config_file" {
                handle.emit("edit_config_file", "").unwrap();
            }
            if event.id.0 == "relauch" {
                handle.restart();
            }
            if event.id.0 == "open_dev_tools" {
                let views = handle.webview_windows();
                for view in views.values() {
                    if view.is_focused().unwrap_or_default() {
                        view.open_devtools();
                    }
                }
            }

            if event.id.0 == "zoom_in" {
                match get_configured_zoom(handle) {
                    Ok(x) => {
                        let mut zoom = x - 0.1;
                        let min = 0.3;
                        if zoom < min {
                            zoom = min;
                        }
                        set_webview_zoom(handle, zoom);
                        if let Err(err) = config_zoom(handle, zoom) {
                            log::error!("set zoom failed:{}", err);
                        }
                    }
                    Err(err) => {
                        log::error!("get zoom failed,err{}", err);
                    }
                }
            }
            if event.id.0 == "zoom_out" {
                match get_configured_zoom(handle) {
                    Ok(x) => {
                        let mut zoom = x + 0.1;
                        let max = 2f64;
                        if zoom > max {
                            zoom = max;
                        }
                        set_webview_zoom(handle, zoom);
                        if let Err(err) = config_zoom(handle, zoom) {
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
                set_webview_zoom(handle, zoom);
                if let Err(err) = config_zoom(handle, zoom) {
                    log::error!("set zoom failed:{}", err);
                }
            }
            if event.id.0 == "open_log_file" {
                handle.emit("open_log_file", "").unwrap();
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
            show_view,
            query_sql,
            redis_list_table,
            query_sql_close_connection,
            query_sql_open_connection,
            query_sql_re_connection,
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
            config_list_database_or_schema,
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
            open_conn,
            drop_materialized_view,
            server_metas,
            auto_completion,
            is_dangerous_sql,
            export_query_result_to_file,
            redis_zset_show_table,
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
            save_password,
            delete_password,
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
            save_entered_password,
            has_entered_password,
            query_get_commit,
            query_set_autocommit,
            query_commit,
            query_rollback,
            valid_json,
            list_sequence,
            drop_sequence,
            rename_sequence,
            command_panic,
            cassandra_invalid_table_or_view_caching,
            fonts_all_families,
            list_sql_current_values,
            sql_parse_error,
            execute_sql_no_open_conn,
            drop_created_type,
            list_event,
            drop_event,
            truncate_log_file,
            key_words,
            extract_table_select_selection,
            extract_table_select_sort,
            get_password,
            get_backend_crash
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
