// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::backtrace::Backtrace;
use std::panic;
mod app;
mod handlers;

use app::{App, DBConnectionConfig};
use handlers::{
    PASSWORD_KEY, add_table_column, auto_completion, cassandra_drop_aggregate,
    cassandra_list_aggregate, cassandra_show_aggregate, check_conn, close_conn, command_panic,
    config_list_database_or_schema, conn_opend, create_database, create_index, create_schema,
    create_table_and_index_sql, create_view_sql, delete_password, disable_trigger, drop_database,
    drop_index, drop_materialized_view, drop_procedure, drop_schema, drop_sequence, drop_table,
    drop_table_column, drop_trigger, drop_udf, drop_view, duplicate_table, enable_trigger,
    execute_sql, export_query_result_to_file, export_table_or_view, format_sql,
    has_entered_password, is_dangerous_sql, list_created_types, list_data_types, list_database,
    list_materialized_view, list_procedure, list_schema, list_sequences, list_table, list_udf,
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
    put_value_in_sql, query_commit, query_get_commit, query_logs, query_rollback,
    query_set_autocommit, query_sql, query_sql_close_connection, query_sql_open_connection,
    query_sql_re_connection, query_tx_execute_counts, redis_hash_add, redis_hash_del,
    redis_hash_replace, redis_list_lpush, redis_list_remove, redis_list_rpush, redis_list_set,
    redis_list_table, redis_set_add, redis_set_del, redis_set_replace, redis_string_set,
    redis_zset_add, redis_zset_del, redis_zset_replace, redis_zset_show_table, rename_index,
    rename_sequnce, rename_table, save_entered_password, save_password, server_metas, show_indexes,
    show_procedure, show_table, show_table_columns, show_trigger, show_udf, show_view,
    show_view_columns, table_delete, table_insert, table_update, test_connection, truncate_table,
    valid_json,
};
use tauri::menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Emitter, Manager};
use tauri_plugin_store::StoreExt;

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

// is this a macbook computer
#[inline(always)]
fn is_mac() -> bool {
    #[cfg(target_os = "macos")]
    return true;
    #[cfg(not(target_os = "macos"))]
    return false;
}

fn main_() {
    {
        use env_logger::{Builder, Env};
        let mut builder = Builder::from_env(Env::default());
        builder.format_file(true);
        builder.format_line_number(true);
        builder.try_init().unwrap();
    }
    let app = App::new();
    let saved_password = app.saved_password.clone();

    let setup = |app: &mut tauri::App| {
        {
            let handler = app.handle().clone();
            let old = panic::take_hook();
            panic::set_hook(Box::new(move |info| {
                let backtrace = Backtrace::capture();

                if let Err(err) = handler.emit("backend-crash", format!("{}\n{}", info, backtrace))
                {
                    log::error!("emit to frontend failed,err:{}", err);
                }
                old(info);
            }));
        }
        #[cfg(debug_assertions)] // only include this code on debug builds
        {
            let window = app.get_webview_window("main").unwrap();
            window.open_devtools();
        }
        let store = app.store("store.json").unwrap();
        *unsafe { App::get_config_ref() } = Some(Box::new(move |name| {
            let configs: Vec<DBConnectionConfig> = serde_json::from_value(
                store
                    .get("connection_config")
                    .unwrap_or(serde_json::Value::Array(vec![])),
            )?;

            for mut c in configs.into_iter() {
                if c.get_name() != name {
                    continue;
                }
                if c.need_password() {
                    if let Some(pass) = saved_password.get(&c.get_name().to_string()) {
                        c.set_password(pass.clone());
                    } else {
                        match keyring::Entry::new(PASSWORD_KEY, name) {
                            Ok(en) => {
                                match en.get_password() {
                                    Ok(pass) => c.set_password(pass),
                                    Err(err) => {
                                        log::error!(
                                            "get password for '{}' failed,err:{}",
                                            name,
                                            err
                                        )
                                    }
                                }
                            }
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
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .manage(app)
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
                            &MenuItem::with_id(
                                handle,
                                "edit_config_file",
                                "edit config file",
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
                    #[cfg(any(target_os = "windows", target_os = "macos"))]
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
                            &PredefinedMenuItem::about(
                                handle,
                                Some("make database management easy."),
                                Some({
                                    let mut meta = AboutMetadata::default();
                                    meta.website = Some("https://dbpal.site".to_string());
                                    meta.name = Some("dbpal".to_string());
                                    meta.authors = Some(vec!["756445638@qq.com".to_string()]);
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
            log::info!("on_menu_event:{:?}", event);
            if event.id.0 == "add_config" {
                handle.emit("add_config", "").unwrap();
            }
            if event.id.0 == "show_settings" {
                handle.emit("show_settings", "").unwrap();
            }
            if event.id.0 == "edit_config_file" {
                handle.emit("edit_config_file", "").unwrap();
            }
            if event.id.0 == "open_dev_tools" {
                let windows = handle.webview_windows();
                for w in windows.values() {
                    w.open_devtools();
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
            show_view,
            query_sql,
            redis_list_table,
            query_sql_close_connection,
            query_sql_open_connection,
            query_sql_re_connection,
            export_table_or_view,
            show_trigger,
            format_sql,
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
            query_logs,
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
            conn_opend,
            create_table_and_index_sql,
            execute_sql,
            save_entered_password,
            has_entered_password,
            query_get_commit,
            query_set_autocommit,
            query_commit,
            query_rollback,
            query_tx_execute_counts,
            valid_json,
            list_sequences,
            drop_sequence,
            rename_sequnce,
            command_panic
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
