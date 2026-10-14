#![warn(clippy::unwrap_used)]
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod app;
mod handlers;

use app::DBPal;
use app::SetZoom;

use handlers::{
    add_table_column, ai_text_completion, all_font_family, auto_completion, cancel_task,
    cassandra_drop_aggregate, cassandra_list_aggregate, cassandra_show_aggregate, check_conn,
    close_conn, command_panic, create_database, create_index, create_schema, create_table,
    create_table_req_sql, create_view_sql, drop_created_type, drop_database, drop_event,
    drop_foreign_key, drop_index, drop_materialized_view, drop_procedure, drop_schema,
    drop_sequence, drop_table, drop_table_column, drop_trigger, drop_udf, drop_view,
    duplicate_table, enable_trigger, establish_conn, execute_sql, execute_tx_multi_sql,
    export_query_result_to_file, export_table_or_view, extract_table_select_selection,
    extract_table_select_sort, get_redis_doc, get_update_sql_from_result_set, is_dangerous_sql,
    list_created_types, list_data_types, list_database, list_event, list_foreign_key,
    list_materialized_view, list_procedure, list_schema, list_sequence, list_sql_predefine_values,
    list_table, list_udf, list_view, modify_table_column, mongo_create_index, mongo_delete_ids,
    mongo_get_one_document_by_id, mongo_insert, mongo_query_collection_count_docs,
    mongo_query_collection_create_index, mongo_query_collection_create_search_index,
    mongo_query_collection_delete_many, mongo_query_collection_delete_one,
    mongo_query_collection_distinct, mongo_query_collection_drop,
    mongo_query_collection_drop_index, mongo_query_collection_drop_indexes,
    mongo_query_collection_drop_search_index, mongo_query_collection_find,
    mongo_query_collection_find_one, mongo_query_collection_find_one_and_delete,
    mongo_query_collection_find_one_and_replace, mongo_query_collection_insert_many,
    mongo_query_collection_insert_one, mongo_query_collection_list_index_names,
    mongo_query_collection_list_indexes, mongo_query_collection_list_search_indexes,
    mongo_query_collection_replace_one, mongo_query_collection_update_many,
    mongo_query_collection_update_one, mongo_query_db_drop, mongo_query_db_list_collection_names,
    mongo_query_db_run_command, mongo_show_collection_docs, mongo_show_collection_table_list,
    mongo_update_one, page_init_callback, query_commit, query_get_commit, query_rollback,
    query_set_autocommit, query_sql, query_sql_close_connection, query_sql_open_connection,
    read_excel_file_preview, redis_hash_add, redis_hash_del, redis_hash_replace, redis_list_key,
    redis_list_lpush, redis_list_remove, redis_list_rpush, redis_list_set, redis_set_add,
    redis_set_del, redis_set_replace, redis_string_set, redis_zset_add, redis_zset_del,
    redis_zset_replace, rename_index, rename_sequence, rename_table, server_metas, set_env,
    set_log_level, show_create_table_sql, show_indexes, show_procedure, show_table,
    show_table_columns, show_trigger, show_udf, show_view_columns, simple_execute_sql,
    sql_encode_value, sql_encode_values, sql_infer_fields_from_file, table_delete,
    table_delete_sql, table_import_data_from_file_insert_samples, table_import_from_file,
    table_insert, table_insert_sql, table_update, table_update_sql, test_connection,
    truncate_table, try_list_database_schema_candidates, write_xlsx,
};

use ai::openai_invoke;
use ai::openai_stream;

use mimalloc::MiMalloc;
use std::fs::OpenOptions;
use std::io::Write;
use std::panic;
use std::path::PathBuf;
use std::sync::Once;
use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Emitter, Manager};
#[allow(unused_imports)]
use tauri_plugin_log::{Target, TargetKind};

use crate::app::TaskManager;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

static HOOK_SET: Once = Once::new();

#[allow(unused)]
fn setup_panic_hook(dir: PathBuf) {
    HOOK_SET.call_once(|| {
        panic::set_hook(Box::new(move |panic_info| {
            eprintln!("{}", panic_info);
            eprintln!("{}", std::backtrace::Backtrace::capture());
            // 1. Extract the information defensively; any step may fail.
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
                    //   Ignore all errors to avoid a second panic.
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
            // 4. Exit immediately without attempting any UI operations.
            std::process::exit(101); // 101 is the conventional panic exit code.
        }));
    });
}

// is this macbook computer
#[inline(always)]
fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

#[allow(dead_code)]
fn get_logger_level() -> Option<log::Level> {
    *handlers::log_level_ref().read().expect("lock failed")
}

#[allow(unused)]
fn logger_test() {
    #[cfg(debug_assertions)]
    std::thread::spawn(|| loop {
        use std::time::Duration;
        log::trace!("this is trace log");
        log::debug!("this is debug log");
        log::info!("this is info log");
        log::warn!("this is warn log");
        log::error!("this is error log");
        std::thread::sleep(std::time::Duration::from_secs(1));
    });
}

fn main() {
    // logger_test();

    #[cfg(debug_assertions)]
    gen_bindings();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("should build runtime");

    tauri::async_runtime::set(runtime.handle().clone());

    let dbpal = DBPal::new();
    let task_manager = TaskManager::default();
    let setup = move |app: &mut tauri::App| {
        setup_panic_hook(
            app.path()
                .app_log_dir()
                .expect("should be able to get app data dir"),
        );
        #[cfg(debug_assertions)] // only include this code on debug builds
        {
            let window = app
                .get_webview_window("main")
                .expect("should be able to get main window");
            window.open_devtools();
        }
        Ok(())
    };

    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_window_state::Builder::new().build());

    #[cfg(feature = "devtools")]
    {
        let _devtools = tauri_plugin_devtools::init();
        builder = builder.plugin(_devtools);
    }

    #[cfg(not(feature = "devtools"))]
    {
        builder = builder.plugin(
            tauri_plugin_log::Builder::new()
                .filter(|meta| {
                    let level = get_logger_level();
                    match level {
                        Some(level) => level >= meta.level(),
                        None => false,
                    }
                })
                .targets(vec![
                    Target::new(TargetKind::LogDir { file_name: None }),
                    Target::new(TargetKind::Stdout),
                ])
                .max_file_size(10 * 1024 * 1024)
                .build(),
        );
    }

    builder
        .plugin(tauri_plugin_autostart::Builder::new().build())
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
            let about_meta = AboutMetadata {
                website: Some("https://dbpal.site".to_string()),
                name: Some("dbpal".to_string()),
                authors: Some(vec![
                    "756445638@qq.com".to_string(),
                    "yuyangxxxmail@gmail.com".to_string(),
                ]),
                version: Some(handle.package_info().version.to_string()),
                copyright: Some("Copyright © yuyang 2025 All rights reserved.".to_string()),
                ..AboutMetadata::default()
            };

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
                                "zoom_out",
                                "zoom out",
                                true,
                                Some(if is_macos() { "command+-" } else { "ctrl+-" }),
                            )?,
                            &MenuItem::with_id(
                                handle,
                                "zoom_in",
                                "zoom in",
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
                        &[
                            &MenuItem::with_id(
                                handle,
                                "show_shortcut",
                                "show shortcut",
                                true,
                                None as Option<&'static str>,
                            )?,
                            &PredefinedMenuItem::about(
                                handle,
                                Some("make database management easy."),
                                Some(about_meta),
                            )?,
                        ],
                    )?,
                ],
            )
        })
        .on_menu_event(move |handle, event| {
            log::info!("on menu event:{:?}", event);

            match event.id.0.as_str() {
                "add_config" => {
                    if let Err(err) = handle.emit("add_config", "") {
                        log::error!("emit add_config failed:{}", err);
                    }
                }
                "show_setting" => {
                    if let Err(err) = handle.emit("show_setting", "") {
                        println!("emit show_setting failed:{}", err)
                    }
                }
                "relauch" => {
                    handle.restart();
                }
                "zoom_out" => {
                    if let Err(err) = handle.emit("set_zoom", SetZoom::Out) {
                        log::error!("emit zoom failed,err:{}", err)
                    }
                }
                "zoom_in" => {
                    if let Err(err) = handle.emit("set_zoom", SetZoom::In) {
                        log::error!("emit zoom failed,err:{}", err)
                    }
                }
                "zoom_reset" => {
                    if let Err(err) = handle.emit("set_zoom", SetZoom::Reset) {
                        log::error!("emit zoom failed,err:{}", err)
                    }
                }
                "show_shortcut" => {
                    if let Err(err) = handle.emit(event.id.0.as_str(), "") {
                        log::error!("emit failed:{}", err);
                    }
                }
                _ => {
                    log::error!("unhandled menu event:{:?}", event);
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
            table_update_sql,
            truncate_table,
            table_delete_sql,
            list_view,
            drop_view,
            drop_table,
            query_sql,
            redis_list_key,
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
            mongo_create_index,
            create_index,
            sql_encode_values,
            cassandra_drop_aggregate,
            cassandra_list_aggregate,
            cassandra_show_aggregate,
            list_created_types,
            show_create_table_sql,
            execute_sql,
            query_get_commit,
            query_set_autocommit,
            query_commit,
            query_rollback,
            list_sequence,
            drop_sequence,
            rename_sequence,
            command_panic,
            all_font_family,
            list_sql_predefine_values,
            simple_execute_sql,
            drop_created_type,
            list_event,
            drop_event,
            extract_table_select_selection,
            extract_table_select_sort,
            get_redis_doc,
            ai_text_completion,
            cancel_task,
            set_log_level,
            create_table_req_sql,
            create_table,
            table_update,
            table_delete,
            openai_stream,
            openai_invoke,
            set_env,
            table_insert_sql,
            sql_encode_values,
            write_xlsx,
            table_import_from_file,
            sql_infer_fields_from_file,
            read_excel_file_preview,
            table_import_data_from_file_insert_samples,
            get_update_sql_from_result_set,
            execute_tx_multi_sql,
            drop_foreign_key,
            list_foreign_key,
            sql_encode_value,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

use dbnexus_db::redis_lang_server::get_commands_from_docs;
use tauri_specta::ErrorHandlingMode;

fn gen_bindings() {
    use handlers::{
        add_table_column, ai_text_completion, all_font_family, auto_completion, cancel_task,
        cassandra_drop_aggregate, cassandra_list_aggregate, cassandra_show_aggregate, check_conn,
        close_conn, command_panic, create_database, create_index, create_schema, create_table,
        create_table_req_sql, create_view_sql, drop_created_type, drop_database, drop_event,
        drop_foreign_key, drop_index, drop_materialized_view, drop_procedure, drop_schema,
        drop_sequence, drop_table, drop_table_column, drop_trigger, drop_udf, drop_view,
        duplicate_table, enable_trigger, establish_conn, execute_sql, execute_tx_multi_sql,
        export_query_result_to_file, export_table_or_view, extract_table_select_selection,
        extract_table_select_sort, get_redis_doc, get_update_sql_from_result_set, is_dangerous_sql,
        list_created_types, list_data_types, list_database, list_event, list_foreign_key,
        list_materialized_view, list_procedure, list_schema, list_sequence,
        list_sql_predefine_values, list_table, list_udf, list_view, modify_table_column,
        mongo_create_index, mongo_delete_ids, mongo_get_one_document_by_id, mongo_insert,
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
        mongo_query_db_list_collection_names, mongo_query_db_run_command,
        mongo_show_collection_docs, mongo_show_collection_table_list, mongo_update_one,
        page_init_callback, query_commit, query_get_commit, query_rollback, query_set_autocommit,
        query_sql, query_sql_close_connection, query_sql_open_connection, read_excel_file_preview,
        redis_hash_add, redis_hash_del, redis_hash_replace, redis_list_key, redis_list_lpush,
        redis_list_remove, redis_list_rpush, redis_list_set, redis_set_add, redis_set_del,
        redis_set_replace, redis_string_set, redis_zset_add, redis_zset_del, redis_zset_replace,
        rename_index, rename_sequence, rename_table, server_metas, set_env, set_log_level,
        show_create_table_sql, show_indexes, show_procedure, show_table, show_table_columns,
        show_trigger, show_udf, show_view_columns, simple_execute_sql, sql_encode_value,
        sql_encode_values, sql_infer_fields_from_file, table_delete, table_delete_sql,
        table_import_data_from_file_insert_samples, table_import_from_file, table_insert,
        table_insert_sql, table_update, table_update_sql, test_connection, truncate_table,
        try_list_database_schema_candidates, write_xlsx,
    };

    use ai::openai_invoke;
    use ai::openai_stream;
    tauri_specta::Builder::<tauri::Wry>::new()
        // Register all commands annotated with #[specta::specta].
        .commands(tauri_specta::collect_commands![
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
            table_update_sql,
            truncate_table,
            table_delete_sql,
            list_view,
            drop_view,
            drop_table,
            query_sql,
            redis_list_key,
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
            mongo_create_index,
            create_index,
            cassandra_drop_aggregate,
            cassandra_list_aggregate,
            cassandra_show_aggregate,
            list_created_types,
            show_create_table_sql,
            execute_sql,
            query_get_commit,
            query_set_autocommit,
            query_commit,
            query_rollback,
            list_sequence,
            drop_sequence,
            rename_sequence,
            command_panic,
            all_font_family,
            list_sql_predefine_values,
            simple_execute_sql,
            drop_created_type,
            list_event,
            drop_event,
            extract_table_select_selection,
            extract_table_select_sort,
            get_redis_doc,
            ai_text_completion,
            cancel_task,
            set_log_level,
            create_table_req_sql,
            create_table,
            table_update,
            table_delete,
            openai_stream,
            openai_invoke,
            set_env,
            table_insert_sql,
            sql_encode_values,
            write_xlsx,
            table_import_from_file,
            sql_infer_fields_from_file,
            read_excel_file_preview,
            table_import_data_from_file_insert_samples,
            get_update_sql_from_result_set,
            execute_tx_multi_sql,
            drop_foreign_key,
            list_foreign_key,
            sql_encode_value,
        ])
        .error_handling(ErrorHandlingMode::Throw)
        .constant("redisCommands", get_commands_from_docs())
        // Export to the frontend directory.
        .export(specta_typescript::Typescript::default(), {
            let dir = env!("CARGO_MANIFEST_DIR");
            let filepath = std::path::Path::new(&dir)
                .to_path_buf()
                .join("../src/comp/bindings.ts");
            filepath
        })
        .unwrap();
}
