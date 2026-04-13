pub mod ai;
pub mod app;
pub mod handlers;

#[cfg(test)]
mod tests {

    use dbnexus_db::redis_lang_server::get_commands_from_docs;
    use std::io::Write;
    use std::path::Path;
    use tauri_specta::ErrorHandlingMode;

    use super::*;

    #[test]
    fn generate_redis_command() {
        let cmds = get_commands_from_docs();
        let dir = env!("CARGO_MANIFEST_DIR");
        let filepath = Path::new(&dir)
            .to_path_buf()
            .join("../src/comp/redisCmd.ts");

        let mut ops = std::fs::OpenOptions::new();
        ops.write(true);
        ops.truncate(true);
        ops.create(true);

        let mut file = ops.open(filepath.as_path()).unwrap();

        file.write("export const redisCommands = [\n".as_bytes())
            .unwrap();

        let last = cmds.len() - 1;

        for (i, x) in cmds.iter().enumerate() {
            file.write(format!(r#"  "{}""#, x).as_bytes()).unwrap();
            if i != last {
                file.write(",\n".as_bytes()).unwrap();
            } else {
                file.write("\n".as_bytes()).unwrap();
            }
        }
        file.write("];".as_bytes()).unwrap();
    }
    #[test]

    fn gen_bindings() {
        use handlers::{
            add_table_column, ai_text_completion, all_font_family, auto_completion, cancel_task,
            cassandra_drop_aggregate, cassandra_list_aggregate, cassandra_show_aggregate,
            check_conn, check_sql, close_conn, command_panic, create_database, create_index,
            create_schema, create_table, create_table_req_sql, create_view_sql, disable_trigger,
            drop_created_type, drop_database, drop_event, drop_index, drop_materialized_view,
            drop_procedure, drop_schema, drop_sequence, drop_table, drop_table_column,
            drop_trigger, drop_udf, drop_view, duplicate_table, enable_trigger, establish_conn,
            execute_sql, export_query_result_to_file, export_table_or_view,
            extract_table_select_selection, extract_table_select_sort, get_redis_doc,
            is_dangerous_sql, list_created_types, list_data_types, list_database, list_event,
            list_materialized_view, list_procedure, list_schema, list_sequence,
            list_sql_current_values, list_table, list_udf, list_view, modify_table_column,
            monaco_editor_themes, mongo_create_index, mongo_delete_ids,
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
            mongo_query_collection_update_one, mongo_query_db_drop,
            mongo_query_db_list_collection_names, mongo_query_db_run_command,
            mongo_show_collection_docs, mongo_show_collection_table_list, mongo_update_one,
            page_init_callback, put_value_in_sql, query_commit, query_get_commit, query_rollback,
            query_set_autocommit, query_sql, query_sql_close_connection, query_sql_open_connection,
            redis_hash_add, redis_hash_del, redis_hash_replace, redis_list_key, redis_list_lpush,
            redis_list_remove, redis_list_rpush, redis_list_set, redis_set_add, redis_set_del,
            redis_set_replace, redis_string_set, redis_zset_add, redis_zset_del,
            redis_zset_replace, rename_index, rename_sequence, rename_table, server_metas, set_env,
            set_log_level, show_create_table_sql, show_indexes, show_procedure, show_table,
            show_table_columns, show_trigger, show_udf, show_view_columns, simple_execute_sql,
            table_delete, table_delete_sql, table_insert, table_insert_sql, table_update,
            table_update_sql, test_connection, truncate_table, try_list_database_schema_candidates,
            valid_json,
        };

        use ai::openai_invoke;
        use ai::openai_stream;
        tauri_specta::Builder::<tauri::Wry>::new()
            // 注册所有带有 #[specta::specta] 标记的命令
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
                mongo_create_index,
                create_index,
                put_value_in_sql,
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
                valid_json,
                list_sequence,
                drop_sequence,
                rename_sequence,
                command_panic,
                all_font_family,
                list_sql_current_values,
                check_sql,
                simple_execute_sql,
                drop_created_type,
                list_event,
                drop_event,
                extract_table_select_selection,
                extract_table_select_sort,
                get_redis_doc,
                monaco_editor_themes,
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
                table_insert_sql
            ])
            .error_handling(ErrorHandlingMode::Throw)
            // 导出到前端目录
            .export(specta_typescript::Typescript::default(), {
                let dir = env!("CARGO_MANIFEST_DIR");
                let filepath = std::path::Path::new(&dir)
                    .to_path_buf()
                    .join("../src/comp/bindings.ts");
                filepath
            })
            .unwrap();
    }
}
