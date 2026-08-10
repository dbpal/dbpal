use crate::app::{
    self, name_to_parts, parse_bson_doc, CancelTokenMsg, DBConnectionConfig, DBPal,
    DatabaseBackend, ErrorFrontend, MongoShowTablePara, TaskManager,
};
use calamine::Reader;
use dashmap::DashMap;
use dbnexus_db::cassandra::CassandraDB;

use dbnexus_db::db::{
    quote_sql_str, ColumnType, CreateDatabaseExtra, CreateIndexReq, CreateSchemaExtra,
    CreateTableAndIndexSql, CreateTableReq, CreatedTypeDef, DefAndMetas, ExportKind, Field, Index,
    JsUint8Array, ListTable, MetaValue, NamedStaff, OptionMustBeSomeErr, ProcedureDetail,
    QueryEventEmit, QueryResp, SequenceDetail, ShowTableParameter, SqlEvent, TableImportPara,
    TimeElapsed, TriggerDetail, XlsxEdit,
};
use dbnexus_db::language_server::CompletionList;
use dbnexus_db::mongo::MongoDB;

use dbnexus_db::redis::{redis_tokenize, RedisDB, RedisListTableRet};
use dbnexus_db::redis_lang_server::REDIS_COMMANDS_MD_FILES;
use mongodb::bson::{self, doc, Document};
use rand::RngExt;
use std::collections::HashMap;
use std::env;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::Duration;
use tauri::async_runtime::spawn_blocking;
use tauri::ipc::Channel;
use tauri::State;
use tokio_util::sync::CancellationToken;

pub fn log_level_ref() -> &'static RwLock<Option<log::Level>> {
    static LOG_LEVEL_LOC: RwLock<Option<log::Level>> = RwLock::new(None);
    &LOG_LEVEL_LOC
}

#[specta::specta]
#[tauri::command]
pub fn set_log_level(level: String) {
    let level = match level.as_str() {
        "off" => None,
        _ => log::Level::from_str(&level)
            .map_err(|e| println!("frontend give wrong kind of log level,{} {}", e, level))
            .ok(),
    };
    *log_level_ref().write().expect("lock failed") = level;
}
#[specta::specta]
#[tauri::command]
pub fn set_env(env: HashMap<String, String>) {
    for (key, value) in env {
        if value.is_empty() {
            // 如果值为空，则从当前进程中移除该环境变量
            env::remove_var(&key);
        } else {
            // 否则正常设置
            env::set_var(&key, &value);
        }
    }
}

#[specta::specta]
#[tauri::command]
pub async fn cancel_task(
    task_id: String,
    state: State<'_, TaskManager>,
) -> Result<(), ErrorFrontend> {
    // 通知取消
    if let Some(token) = state.tokens.get(&task_id) {
        token.cancel(); // 触发取消
    } else {
        return Err(ErrorFrontend::from(format!(
            "task id not found '{}'",
            task_id
        )));
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn get_redis_doc(line: &str) -> Result<String, ErrorFrontend> {
    let tokens = redis_tokenize(line).map_err(ErrorFrontend::from)?;
    if tokens.is_empty() {
        return Err(ErrorFrontend::from("no tokens found,empty line??"));
    }

    let command = tokens[0].token.to_lowercase();
    if tokens.len() >= 2 {
        let file = REDIS_COMMANDS_MD_FILES
            .get_file(format!("{}-{}.md", command, tokens[1].token.to_lowercase()).as_str());
        if let Some(file) = file {
            let mut ret = format!("## {} {}\n", tokens[0].token, tokens[1].token);
            ret.push_str(file.contents_utf8().option_must_be_some()?);
            return Ok(ret);
        }
    }
    let exact_file = format!("{}.md", command);
    match REDIS_COMMANDS_MD_FILES.get_file(&exact_file) {
        Some(file) => {
            let sub_match = format!("{}-", command);
            let mut ret = format!("## {}\n", tokens[0].token);
            ret.push_str(file.contents_utf8().option_must_be_some()?);
            // try match sub markdown
            for en in REDIS_COMMANDS_MD_FILES.entries() {
                match en {
                    include_dir::DirEntry::Dir(..) => {}
                    include_dir::DirEntry::File(file) => {
                        let str_path = file.path().to_str();
                        if str_path
                            .map(|path| path.starts_with(&sub_match) && path.ends_with(".md"))
                            .unwrap_or_default()
                        {
                            ret.push_str("  \n");
                            ret.push_str("---\n");
                            ret.push_str(
                                format!("### {}\n", {
                                    let name = file
                                        .path()
                                        .file_name()
                                        .and_then(|x| x.to_str())
                                        .unwrap_or_default();
                                    if name.len() > 3 {
                                        let db_name = &name[0..(name.len() - 3)];
                                        if db_name.contains("-") {
                                            let db_name =
                                                db_name.split("-").collect::<Vec<_>>().join(" ");
                                            db_name
                                        } else {
                                            db_name.to_string()
                                        }
                                    } else {
                                        name.to_string()
                                    }
                                })
                                .as_str(),
                            );
                            ret.push_str(file.contents_utf8().option_must_be_some()?);
                        }
                    }
                }
            }
            Ok(ret)
        }
        None => Err(ErrorFrontend::from(format!(
            "no markdown file found for command '{}'",
            tokens[0].token
        ))),
    }
}

#[specta::specta]
#[tauri::command]

pub async fn read_excel_file_preview(
    path: String,
    max_row: u32,
) -> Result<MuiDataGridData, ErrorFrontend> {
    MuiDataGridData::preview_excel(&path, max_row)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn all_font_family() -> Result<Vec<String>, ErrorFrontend> {
    let source = font_kit::source::SystemSource::new();
    let fonts = source.all_families().map_err(ErrorFrontend::from)?;
    Ok(fonts)
}

#[specta::specta]
#[tauri::command]
pub async fn page_init_callback(app: tauri::State<'_, DBPal>) -> Result<(), ()> {
    log::info!("page init call clear all open connection.");
    app.backend.lock().await.clear();
    Ok(())
}

#[specta::specta]
#[tauri::command]

pub async fn sql_infer_fields_from_file(
    db_name: String,
    app: tauri::State<'_, DBPal>,
    path: String,
    csv_delimiter: Option<char>,
) -> Result<Vec<Field>, ErrorFrontend> {
    let backend = app
        .get_backend(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let kind = match backend {
        DatabaseBackend::Catalog(catalog_trait) => catalog_trait.backend_kind(),
        DatabaseBackend::DataBase(dbtrait) => dbtrait.backend_kind(),
        DatabaseBackend::Schema(schema_trait) => schema_trait.backend_kind(),
    };
    let fs = kind
        .infer_fields(&path, csv_delimiter)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(fs)
}

#[specta::specta]
#[tauri::command]
pub async fn drop_database(
    db_name: String,
    database: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("drop db:{} {}", db_name, database);
    let backend = app
        .get_backend(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    // first close connections because we want delete it
    let key = format!("{}/{}", db_name, database);
    app.close_conn_has_prefix(&key).await;
    match backend {
        DatabaseBackend::Catalog(x) => {
            app.backend.lock().await.remove(&key);

            x.drop_database(&database)
                .await
                .map_err(ErrorFrontend::from)?;

            Ok(())
        }

        _ => Err(ErrorFrontend::from(format!(
            "'{}' is not a db catalog",
            db_name
        ))),
    }
}

#[specta::specta]
#[tauri::command]
pub async fn drop_schema(
    db_name: String,
    schema: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("drop schema:{} {}", db_name, schema);

    // first close connections because we want delete it
    let key = format!("{}/{}", db_name, schema);
    app.close_conn_has_prefix(&key).await;
    let backend = app
        .get_backend(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match backend {
        DatabaseBackend::DataBase(x) => {
            app.backend.lock().await.remove(&key);
            x.drop_schema(&schema).await.map_err(ErrorFrontend::from)
        }
        _ => Err(ErrorFrontend::from(format!(
            "'{}' is not a db database",
            db_name
        ))),
    }
}

#[specta::specta]
#[tauri::command]
pub async fn create_database(
    db_name: String,
    database: String,
    extra: Option<CreateDatabaseExtra>,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let backend = app
        .get_backend(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match backend {
        DatabaseBackend::Catalog(x) => {
            x.create_database(&database, extra)
                .await
                .map_err(ErrorFrontend::from)?;
            Ok(())
        }
        _ => Err(ErrorFrontend::from(format!(
            "'{}' is not a db catalog",
            db_name
        ))),
    }
}

#[specta::specta]
#[tauri::command]
pub async fn create_schema(
    db_name: String,
    schema: String,
    extra: Option<CreateSchemaExtra>,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let backend = app
        .get_backend(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match backend {
        DatabaseBackend::DataBase(x) => {
            x.create_schema(&schema, extra)
                .await
                .map_err(ErrorFrontend::from)?;
            Ok(())
        }
        _ => Err(ErrorFrontend::from(format!(
            "'{}' is not a db database",
            db_name
        ))),
    }
}

#[specta::specta]
#[tauri::command]
pub async fn establish_conn(
    config: DBConnectionConfig,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    #[cfg(debug_assertions)]
    println!("open connection:{:?}", config);
    let secs = 10;
    tokio::time::timeout(Duration::from_secs(secs), async {
        app.establish_conn(&config)
            .await
            .map_err(ErrorFrontend::from)
            .map(|_| ())
    })
    .await
    .map_err(|_| {
        ErrorFrontend::from(format!(
            "'{}' open connection not finished in {}s",
            config.get_name(),
            secs
        ))
    })?
}

#[specta::specta]
#[tauri::command]
pub async fn close_conn(db_name: String, app: tauri::State<'_, DBPal>) -> Result<(), ()> {
    random_sleep().await;
    app.close_conn_has_prefix(db_name.as_str()).await;
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn list_table(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<ListTable>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&db_name).await?;
    db.list_table().await.map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn redis_list_key(
    db_name: String,
    filter: Option<String>,
    max: u32,
    app: tauri::State<'_, DBPal>,
) -> Result<RedisListTableRet, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(&db_name)
        .await
        .map_err(ErrorFrontend::from)?;
    match db.as_any().downcast_ref::<RedisDB>() {
        Some(x) => x.redis_list_key(filter.as_deref(), max as usize).await,
        None => return Err(ErrorFrontend::from(format!("'{}' not a redis", db_name))),
    }
    .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn show_table_columns(
    db_name: String,
    table_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<Field>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&db_name).await?;
    db.show_table_columns(&table_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn show_table(
    db_name: String,
    is_view: bool,
    table_name: String,
    para: ShowTableParameter,
    emit: Channel<QueryEvent>,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let token = CancellationToken::new();
    let token_msg = CancelTokenMsg::new(token.clone());
    let token_msg_clone = token_msg.clone();
    let sender = Arc::new(QueryEventChannelEmit::new(
        emit,
        token_msg_clone,
        para.limit,
    ));
    let db = app.get_schema(&db_name).await?;
    let fur = async {
        if is_view {
            db.show_view(&table_name, para, sender)
                .await
                .map_err(ErrorFrontend::from)
        } else {
            db.show_table(&table_name, para, sender)
                .await
                .map_err(ErrorFrontend::from)
        }
    };
    tokio::select! {
        biased;
        _ = token.cancelled() => Err(ErrorFrontend::cancelled(token_msg.get_msg())),
        res = fur => res,

    }
}

#[specta::specta]
#[tauri::command]
pub async fn show_indexes(
    db_name: String,
    table_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<Index>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&db_name).await?;
    db.show_indexes(&table_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn create_index(
    db_name: String,
    table_name: String,
    req: CreateIndexReq,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    schema
        .create_index(&table_name, &req)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_create_index(
    db_name: String,
    table_name: String,
    index_name: Option<String>,
    keys: JsUint8Array,
    unique: bool,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let keys = parse_bson_doc(&keys)?;
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x
            .create_index(&table_name, index_name, unique, keys)
            .await
            .map_err(ErrorFrontend::from),
        None => Err(ErrorFrontend::from(format!("'{}' not a mongo", db_name))),
    }
}
#[specta::specta]
#[tauri::command]
pub async fn rename_index(
    db_name: String,
    table_name: String,
    old_name: String,
    new_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .rename_index(&table_name, &old_name, &new_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn drop_index(
    db_name: String,
    table_name: String,
    index: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    schema
        .drop_index(&table_name, &index)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn table_insert(
    db_name: String,
    table_name: String,
    values: HashMap<String, Option<String>>,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<f64>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&db_name).await?;
    let r = db
        .table_insert(&table_name, values)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(r.map(|x| x as f64))
}

#[specta::specta]
#[tauri::command]
pub async fn table_insert_sql(
    db_name: String,
    table_name: String,
    values: HashMap<String, Option<String>>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&db_name).await?;
    db.table_insert_sql(&table_name, values)
        .await
        .map_err(ErrorFrontend::from)
}
#[specta::specta]
#[tauri::command]
pub async fn table_update(
    db_name: String,
    table_name: String,
    values: HashMap<String, Option<String>>,
    wheres: HashMap<String, Option<String>>,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<f64>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&db_name).await?;
    let r = db
        .table_update(&table_name, values, wheres)
        .await
        .map_err(ErrorFrontend::from)?;

    Ok(r.map(|x| x as f64))
}

#[specta::specta]
#[tauri::command]
pub async fn table_update_sql(
    db_name: String,
    table_name: String,
    values: HashMap<String, Option<String>>,
    wheres: HashMap<String, Option<String>>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&db_name).await?;
    db.table_update_sql(&table_name, values, wheres)
        .await
        .map_err(ErrorFrontend::from)
}
#[specta::specta]
#[tauri::command]
pub async fn truncate_table(
    db_name: String,
    table_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&db_name).await?;
    db.truncate_table(&table_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn drop_table_column(
    db_name: String,
    table_name: String,
    column_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .drop_table_column(&table_name, &column_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn modify_table_column(
    db_name: String,
    table_name: String,
    old_name: String,
    new_name: String,
    typ: Option<ColumnType>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .modify_table_column(&table_name, &old_name, &new_name, typ)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn add_table_column(
    db_name: String,
    table_name: String,
    col_name: String,
    typ: ColumnType,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .add_table_column(&table_name, &col_name, typ)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn list_data_types(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let ret = schema
        .list_data_types()
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[specta::specta]
#[tauri::command]
pub async fn list_sql_current_values(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<&'static [&'static str], ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let ret = schema.list_sql_current_values();
    Ok(ret)
}

#[specta::specta]
#[tauri::command]
pub async fn table_delete_sql(
    db_name: String,
    table_name: String,
    wheres: HashMap<String, Option<String>>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&db_name).await?;
    db.table_delete_sql(&table_name, wheres)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn table_delete(
    db_name: String,
    table_name: String,
    wheres: HashMap<String, Option<String>>,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<f64>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&db_name).await?;
    let r = db
        .table_delete(&table_name, wheres)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(r.map(|x| x as f64))
}

#[specta::specta]
#[tauri::command]
pub async fn list_view(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<ListTable>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&db_name).await?;
    db.list_view().await.map_err(ErrorFrontend::from)
}
#[specta::specta]
#[tauri::command]
pub async fn server_metas(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<MetaValue>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let backend = app
        .get_backend(&db_name)
        .await
        .map_err(ErrorFrontend::from)?;
    backend
        .as_server_mates()
        .show_metas()
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn list_materialized_view(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<ListTable>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .list_materialized_view()
        .await
        .map_err(ErrorFrontend::from)
}
#[specta::specta]
#[tauri::command]
pub async fn drop_view(
    db_name: String,
    view_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&db_name).await?;
    db.drop_view(&view_name).await.map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn drop_materialized_view(
    db_name: String,
    view_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(&db_name)
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .drop_materialized_view(&view_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn drop_table(
    db_name: String,
    table_name: String,
    cascade: bool,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&db_name).await?;
    db.drop_table(&table_name, cascade)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn show_view_columns(
    db_name: String,
    view_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<Field>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(&db_name)
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .show_view_columns(&view_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn list_created_types(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<CreatedTypeDef>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(&db_name)
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .list_created_types()
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn list_event(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<SqlEvent>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(&db_name)
        .await
        .map_err(ErrorFrontend::from)?;
    schema.list_event().await.map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn drop_event(
    db_name: String,
    event_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(&db_name)
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .drop_event(&event_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn export_query_result_to_file(
    db_name: String,
    task_id: String,
    sql: String,
    query_id: String,
    kind: ExportKind,
    file: String,
    size_emitter: Channel<f64>,
    task_manager: tauri::State<'_, TaskManager>,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!(
        "export_query_result_to_file sql{} query_id:{}",
        sql,
        query_id
    );
    let (token, _guard) = task_manager.add(task_id);
    let opened = app
        .get_backend(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let size_emitter = Arc::new(move |size: usize| {
        if let Err(err) = size_emitter.send(size as f64) {
            log::error!("Error sending size to frontend: {}", err);
        }
    });
    let fur = async {
        opened
            .as_export_query_result()
            .export_query_result_to_file(&query_id, &sql, kind, &file, size_emitter.clone())
            .await
            .map_err(ErrorFrontend::from)
    };
    let remove_file = async {
        if let Err(e) = tokio::fs::remove_file(&file).await {
            log::error!("remove file failed:{}", e);
        }
    };
    let r = tokio::select! {
        biased;
        res =  fur  => res,
        _ = token.cancelled() => {
            remove_file.await;
            Err(ErrorFrontend::cancelled(None))
        }
    };

    r
}

#[specta::specta]
#[tauri::command]
pub async fn is_dangerous_sql(
    db_name: String,
    sql: String,
    app: tauri::State<'_, DBPal>,
) -> Result<bool, ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let db = app
        .get_backend(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let result = db
        .as_query_sql()
        .is_dangerous_sql(&sql)
        .map_err(ErrorFrontend::from)?;

    Ok(result)
}

struct QueryEventChannelEmit {
    channel: Channel<QueryEvent>,
    cancel_msg: CancelTokenMsg,
    limit: Option<u32>,
    emitted_size: AtomicUsize,
    cache: Arc<Mutex<Vec<Vec<Option<String>>>>>,
    time_elapsed: Arc<Mutex<TimeElapsed>>,
}

impl QueryEventChannelEmit {
    fn elapsed(&self) -> bool {
        self.time_elapsed.lock().expect("lock failed").elapsed()
    }
}

impl QueryEventChannelEmit {
    fn new(channel: Channel<QueryEvent>, cancel_msg: CancelTokenMsg, limit: Option<u32>) -> Self {
        Self {
            channel,
            cancel_msg,
            limit,
            emitted_size: AtomicUsize::new(0),
            cache: Default::default(),
            time_elapsed: Arc::new(Mutex::new(TimeElapsed::new(Duration::from_secs(1)))),
        }
    }
    fn get_limit(&self) -> u32 {
        self.limit.unwrap_or(u32::MAX)
    }
}

impl Drop for QueryEventChannelEmit {
    fn drop(&mut self) {
        if let Err(err) = self.take_data_and_send() {
            println!("send data to frontend failed,err:{}", err);
        }
        if let Err(err) = self.channel.send(QueryEvent::End) {
            println!("send data to frontend failed,err:{}", err);
        }
    }
}

impl QueryEventChannelEmit {
    fn take_data_and_send(&self) -> anyhow::Result<()> {
        let mut data = self.cache.lock().expect("lock failed");
        let r = std::mem::take(&mut *data);
        if !r.is_empty() {
            self.channel.send(QueryEvent::Data { data: r })?;
        }
        anyhow::Ok(())
    }
}

impl QueryEventEmit for QueryEventChannelEmit {
    fn send(&self, event: dbnexus_db::db::QueryEvent) -> anyhow::Result<()> {
        block_random_sleep();
        let total = self
            .emitted_size
            .fetch_add(event.data_len(), Ordering::Relaxed);
        if total > (self.get_limit() as usize) {
            let err_msg = format!(
                "Query result is too large({}), limit:{}",
                total,
                self.get_limit(),
            );
            self.cancel_msg.cancel(err_msg.clone());
            return Err(anyhow::anyhow!(err_msg));
        }
        let cache_size = if cfg!(debug_assertions) { 177 } else { 1000 };
        match event {
            dbnexus_db::db::QueryEvent::Resp(query_resp) => {
                self.take_data_and_send()?;
                self.channel.send(QueryEvent::Structure {
                    resp: MuiDataGridData::from(query_resp),
                })?;
            }
            dbnexus_db::db::QueryEvent::Data(items) => {
                let mut data = self.cache.lock().expect("lock failed");
                data.extend(items);
                if data.len() > cache_size || self.elapsed() {
                    drop(data);
                    self.take_data_and_send()?;
                }
            }
            dbnexus_db::db::QueryEvent::Status(s) => {
                self.channel.send(QueryEvent::Status { status: s })?;
            }
            dbnexus_db::db::QueryEvent::Err(err) => {
                self.channel.send(QueryEvent::Err { err })?;
            }
            dbnexus_db::db::QueryEvent::Row(x) => {
                let mut data = self.cache.lock().expect("lock failed");
                data.push(x);
                if data.len() > cache_size || self.elapsed() {
                    drop(data);
                    self.take_data_and_send()?;
                }
            }
        }

        anyhow::Ok(())
    }
}

#[specta::specta]
#[tauri::command]
pub async fn query_sql(
    db_name: String,
    query_id: &str,
    max_row: u32,
    sql: String,
    app: tauri::State<'_, DBPal>,
    emit: Channel<QueryEvent>,
    task_manager: tauri::State<'_, TaskManager>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_backend(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let (token, _guard) = task_manager.add(query_id.to_string());
    let cancel_msg: CancelTokenMsg = CancelTokenMsg::new(token.clone());
    let sender = Arc::new(QueryEventChannelEmit::new(
        emit,
        cancel_msg.clone(),
        Some(max_row),
    ));
    let fur = async move {
        random_sleep().await;
        db.as_query_sql()
            .query_sql(query_id, &sql, sender)
            .await
            .map_err(ErrorFrontend::from)
    };
    tokio::select! {
        biased;
        _ = token.cancelled() => Err(ErrorFrontend::cancelled(cancel_msg.get_msg())),
        res = fur => res,
    }
}

#[derive(Debug, serde::Serialize, specta::Type)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum QueryEvent {
    Err { err: String },
    Status { status: String },
    Data { data: Vec<Vec<Option<String>>> },
    Structure { resp: MuiDataGridData },
    End,
}

#[specta::specta]
#[tauri::command]
pub async fn simple_execute_sql(
    db_name: String,
    sql: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<f64>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let db = app
        .get_backend(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let result = db
        .as_query_sql()
        .simple_execute_sql(&sql)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(result.map(|x| x as f64))
}

#[specta::specta]
#[tauri::command]
pub async fn execute_sql(
    db_name: String,
    query_id: &str,
    sql: String,
    app: tauri::State<'_, DBPal>,
    task_manager: tauri::State<'_, TaskManager>,
) -> Result<Result<MuiDataGridData, String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_backend(&db_name)
        .await
        .map_err(ErrorFrontend::from)?;
    let (token, _guard) = task_manager.add(query_id.to_string());
    let fur = async {
        #[cfg(feature = "random-sleep")]
        tokio::time::sleep(Duration::from_secs(10)).await;
        db.as_query_sql()
            .execute_sql(query_id, &sql)
            .await
            .map_err(ErrorFrontend::from)
    };
    let result = tokio::select! {
        biased;
        _ = token.cancelled() => {
            return Err(ErrorFrontend::cancelled(None));
        }
        res = fur => res,
    };
    let result = result?;
    let result = result.map(MuiDataGridData::from);
    Ok(result)
}

#[specta::specta]
#[tauri::command]
pub async fn query_sql_open_connection(
    db_name: String,
    id: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_backend(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        db.as_query_sql().open_connection(&id).await
    })
    .await
    .map_err(|_e| ErrorFrontend::from("open connection not finished in reasonable time"))?;
    result.map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn query_sql_close_connection(
    db_name: String,
    query_id: &str,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("close conn:{} {}", db_name, query_id);
    let db = app
        .get_backend(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let result = db.as_query_sql().close_connection(query_id).await;
    result.map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn export_table_or_view(
    db_name: String,
    task_id: String,
    task_manager: tauri::State<'_, TaskManager>,
    size_emitter: Channel<f64>,
    table_or_view: String,
    para: Option<ShowTableParameter>,
    filepath: String,
    kind: ExportKind,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let (token, _guard) = task_manager.add(task_id);
    let filepath2 = filepath.clone();
    let fur = async move {
        #[cfg(feature = "random-sleep")]
        tokio::time::sleep(Duration::from_secs(10)).await;
        db.export_table_or_view_to_file(
            &table_or_view,
            para,
            &filepath2,
            kind,
            Arc::new(move |size| {
                if let Err(err) = size_emitter.send(size as f64) {
                    log::error!("Error sending size: {}", err);
                }
            }),
        )
        .await
        .map_err(ErrorFrontend::from)
    };
    let remove_file = async {
        if let Err(e) = tokio::fs::remove_file(&filepath).await {
            log::error!("remove file failed:{}", e);
        }
    };
    let r = tokio::select! {
        biased;
        _ = token.cancelled() => {
            remove_file.await;
            Err(ErrorFrontend::cancelled(None))
        }
        res = fur => res,
    };

    r
}

#[specta::specta]
#[tauri::command]
pub async fn check_conn(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<
    f64, // human readable time
    ErrorFrontend,
> {
    random_sleep().await;
    random_error()?;
    let timeout = 5;
    tokio::time::timeout(Duration::from_secs(timeout), async move {
        let db = app.get_backend(&db_name).await?;
        db.check_conn()
            .await
            .map_err(ErrorFrontend::from)
            .map(|d| d.as_millis() as f64)
    })
    .await
    .map_err(|_| {
        ErrorFrontend::from(format!(
            "check connection status not finished in {}s",
            timeout
        ))
    })?
}

#[specta::specta]
#[tauri::command]
pub async fn test_connection(
    config: DBConnectionConfig,
) -> Result<
    u32, // milliseconds
    ErrorFrontend,
> {
    random_error()?;
    let timeout = 5;
    tokio::time::timeout(Duration::from_secs(5), async {
        let db = config.open().await.map_err(|e| e.to_string())?;
        db.check_conn()
            .await
            .map_err(ErrorFrontend::from)
            .map(|d| d.as_millis() as u32)
    })
    .await
    .map_err(|_| ErrorFrontend::from(format!("test connection not finished in {}s", timeout)))?
}

#[specta::specta]
#[tauri::command]
pub async fn try_list_database_schema_candidates(
    conn: DBConnectionConfig,
) -> Result<Vec<String>, ErrorFrontend> {
    tokio::time::timeout(Duration::from_secs(5), async {
        let x = conn.open().await?;
        match x {
            DatabaseBackend::Catalog(catalog_trait) => catalog_trait
                .list_database()
                .await
                .map_err(ErrorFrontend::from),
            DatabaseBackend::DataBase(db_trait) => {
                db_trait.list_schema().await.map_err(ErrorFrontend::from)
            }
            DatabaseBackend::Schema(_) => Ok(vec![]),
        }
    })
    .await
    .map_err(ErrorFrontend::from)?
}

#[specta::specta]
#[tauri::command]
pub async fn list_database(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<
    Vec<String>, // milliseconds
    ErrorFrontend,
> {
    random_sleep().await;
    random_error()?;
    let db = app.get_backend(db_name.as_str()).await?;
    // app.enter_open_child(db_name.as_str(), &db).await?;
    match db {
        app::DatabaseBackend::Catalog(x) => x.list_database().await,
        _ => {
            return Err(ErrorFrontend::from(format!(
                "not catalog trait but:{}",
                db_name
            )));
        }
    }
    .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn list_schema(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_backend(db_name.as_str()).await?;
    match db {
        app::DatabaseBackend::DataBase(x) => x.list_schema().await,
        app::DatabaseBackend::Catalog(_) => {
            return Err(ErrorFrontend::from(format!(
                "not database trait but a catalog of {}",
                db_name
            )));
        }
        app::DatabaseBackend::Schema(_) => {
            return Err(ErrorFrontend::from(format!(
                "not database trait but a schema of {}",
                db_name
            )));
        }
    }
    .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn extract_table_select_selection(
    db_name: String,
    sql: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_backend(db_name.as_str()).await?;
    let ret = db
        .as_language_server()
        .extract_table_select_selection(&sql)
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[specta::specta]
#[tauri::command]
pub async fn extract_table_select_sort(
    db_name: String,
    sql: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_backend(db_name.as_str()).await?;
    let ret = db
        .as_language_server()
        .extract_table_select_sort(&sql)
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

fn auto_completion_mutex(db_name: &str) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: LazyLock<Arc<DashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
        LazyLock::new(|| Arc::new(DashMap::new()));
    let db_name = db_name.to_owned();
    match LOCKS.get(&db_name) {
        Some(x) => x.clone(),
        None => {
            let x = Arc::new(tokio::sync::Mutex::new(()));
            LOCKS.insert(db_name, x.clone());
            x.clone()
        }
    }
}

#[specta::specta]
#[tauri::command]
pub async fn auto_completion(
    db_name: String,
    query_id: Option<String>,
    sql: String,
    line: u32,
    col: u32,
    app: tauri::State<'_, DBPal>,
) -> Result<CompletionList, ErrorFrontend> {
    let use_lexer = true;
    // 避免多个线程同时访问
    // 主要是后端实现更简单了比如可以把 completion——id缓存起来
    let lock = auto_completion_mutex(name_to_parts(&db_name)[0]);
    let _guard = lock.lock().await;

    let db = app.get_backend(db_name.as_str()).await?;
    let ret = tokio::time::timeout(std::time::Duration::from_secs(4), async {
        db.as_language_server()
            .auto_completion(query_id, &sql, line, col, use_lexer)
            .await
    })
    .await
    .map_err(|_| ErrorFrontend::from("auto completion timeout"))?
    .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[specta::specta]
#[tauri::command]
pub async fn ai_text_completion(
    db_name: String,
    text: String,
    line: u32,
    col: u32,
    app: tauri::State<'_, DBPal>,
) -> Result<CompletionList, ErrorFrontend> {
    let db = app.get_backend(&db_name).await?;
    let list = tokio::time::timeout(std::time::Duration::from_secs(4), async {
        db.as_language_server()
            .ai_text_completion(&text, line, col)
            .await
    })
    .await
    .map_err(|_| ErrorFrontend::from("ai auto completion timeout"))?
    .map_err(|e| log::error!("completion failed,err:{}", e))
    .unwrap_or_default();
    if list.from_dot {
        return Ok(list);
    }
    Ok(list)
}

#[specta::specta]
#[tauri::command]
pub async fn query_set_autocommit(
    db_name: String,
    query_id: &str,
    commit: bool,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let backend = app
        .get_backend(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    backend
        .as_query_sql()
        .set_auto_commit(query_id, commit)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn query_get_commit(
    db_name: String,
    query_id: &str,
    app: tauri::State<'_, DBPal>,
) -> Result<bool, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let backend = app
        .get_backend(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    backend
        .as_query_sql()
        .get_auto_commit(query_id)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn query_commit(
    db_name: String,
    query_id: &str,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let backend = app
        .get_backend(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    backend
        .as_query_sql()
        .commit(query_id)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn query_rollback(
    db_name: String,
    query_id: &str,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_backend(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    db.as_query_sql()
        .rollback(query_id)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn drop_trigger(
    db_name: String,
    table_name: String,
    trigger: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    db.drop_trigger(&table_name, &trigger)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn enable_trigger(
    db_name: String,
    table_name: String,
    trigger: String,
    enable: bool,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    db.enable_trigger(&table_name, &trigger, enable)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn show_trigger(
    db_name: String,
    table_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<TriggerDetail>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let x = db
        .list_trigger(&table_name)
        .await
        .map_err(ErrorFrontend::from)?;

    let mut ret = Vec::with_capacity(x.len());

    for tri in x.iter() {
        let d = db
            .show_trigger(&table_name, &tri.name)
            .await
            .map_err(ErrorFrontend::from)?;
        ret.push(d);
    }
    Ok(ret)
}

#[specta::specta]
#[tauri::command]
pub async fn rename_table(
    db_name: String,
    old: String,
    new_table: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    db.rename_table(&old, &new_table)
        .await
        .map_err(ErrorFrontend::from)
}
#[specta::specta]
#[tauri::command]
pub async fn create_table_req_sql(
    db_name: String,
    req: CreateTableReq,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let ret = db
        .create_table_sql_string(&req)
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[specta::specta]
#[tauri::command]

pub async fn create_table(
    db_name: String,
    req: CreateTableReq,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    db.create_table(&req).await.map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn show_create_table_sql(
    db_name: String,
    table_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<CreateTableAndIndexSql, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let ret = db
        .show_create_table_sql(&table_name)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[specta::specta]
#[tauri::command]
pub async fn create_view_sql(
    db_name: String,
    view_name: String,
    materialized: bool,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let ret = db
        .show_view_sql(&view_name, materialized)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[specta::specta]
#[tauri::command]
pub async fn duplicate_table(
    db_name: String,
    from: String,
    task_id: String,
    task_manager: tauri::State<'_, TaskManager>,
    to: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let (token, _guard) = task_manager.add(task_id);

    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let fur = async {
        #[cfg(feature = "random-sleep")]
        tokio::time::sleep(Duration::from_secs(10)).await;
        db.duplicate_table(&from, &to)
            .await
            .map_err(ErrorFrontend::from)
    };
    let result = tokio::select! {
        r = fur => r,
        _ = token.cancelled() => {
            Err(ErrorFrontend::cancelled(None))
        }
    };
    result
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_insert(
    db_name: String,
    table_name: String,
    doc: JsUint8Array,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let doc = parse_bson_doc(doc.deref())?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.insert(table_name.as_str(), doc).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)
    .map(|x| x.inserted_id.to_string())
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_update_one(
    db_name: String,
    table_name: String,
    doc: JsUint8Array,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let doc = parse_bson_doc(doc.deref())?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.update_one(table_name.as_str(), doc).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    if ret.modified_count != 1 {
        return Err(ErrorFrontend::from("modified count is not 1"));
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_delete_ids(
    db_name: String,
    table_name: String,
    ids: String,
    app: tauri::State<'_, DBPal>,
) -> Result<f64, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let ids: Vec<bson::Bson> = serde_json::from_str(&ids)?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.delete_by_ids(table_name.as_str(), ids).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)
    .map(|x| x.deleted_count as f64)
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_show_collection_table_list(
    db_name: String,
    table_name: String,
    para: MongoShowTablePara,
    app: tauri::State<'_, DBPal>,
) -> Result<MuiDataGridData, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.show_collection_table_list(table_name.as_str(), para.try_into()?)
                .await
        }
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)
    .map(MuiDataGridData::from)
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_show_collection_docs(
    db_name: String,
    table_name: String,
    app: tauri::State<'_, DBPal>,
    para: MongoShowTablePara,
) -> Result<DocsAndCount, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let docs = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.show_collection_docs(table_name.as_str(), para.try_into()?)
                .await
        }
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    DocsAndCount::try_from(docs).map_err(|e| e.into())
}
#[specta::specta]
#[tauri::command]
pub async fn mongo_get_one_document_by_id(
    db_name: String,
    table_name: String,
    id: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<u8>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let id: bson::Bson = serde_json::from_str(&id)?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let doc = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.get_one_document_by_id(table_name.as_str(), id).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    encode_bson_doc(&doc).map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_db_drop(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_db_drop().await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(())
}
#[specta::specta]
#[tauri::command]
pub async fn mongo_query_db_list_collection_names(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_db_list_collection_names().await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(ret)
}
#[specta::specta]
#[tauri::command]
pub async fn mongo_query_db_run_command(
    db_name: String,
    command: JsUint8Array,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<u8>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let command = parse_bson_doc(command.deref())?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_db_run_command(command).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(encode_bson_doc(&ret)?)
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_count_docs(
    db_name: String,
    col: String,
    filter: Option<JsUint8Array>,
    app: tauri::State<'_, DBPal>,
) -> Result<f64, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let filter = if let Some(filter) = filter {
        Some(parse_bson_doc(&filter)?)
    } else {
        None
    };
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_count_docs(&col, filter).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(ret as f64)
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_create_index(
    db_name: String,
    col: String,
    index: JsUint8Array,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let index = parse_bson_doc(&index)?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_create_index(&col, index).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(ret.index_name)
}
#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_create_search_index(
    db_name: String,
    col: String,
    index: JsUint8Array,
    index_name: Option<String>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let index = parse_bson_doc(index.deref())?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.query_collection_create_search_index(&col, index, index_name)
                .await
        }
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(ret)
}
#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_delete_many(
    db_name: String,
    col: String,
    query: JsUint8Array,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(query.deref())?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_delete_many(&col, query).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(serde_json::to_string(&ret)?)
}
#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_delete_one(
    db_name: String,
    col: String,
    query: JsUint8Array,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(query.deref())?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_delete_one(&col, query).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(serde_json::to_string(&ret)?)
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_distinct(
    db_name: String,
    col: String,
    query: JsUint8Array,
    field: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(&query)?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let values = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_distinct(&col, query, &field).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    serde_json::to_string(&values).map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_drop(
    db_name: String,
    col: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_drop(&col).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_drop_index(
    db_name: String,
    col: String,
    index: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_drop_index(&col, &index).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_drop_indexes(
    db_name: String,
    col: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_drop_indexes(&col).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(())
}
#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_drop_search_index(
    db_name: String,
    col: String,
    index: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_drop_search_index(&col, &index).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_find(
    db_name: String,
    col: String,
    para: app::MongoFindParameter,
    emit: Channel<Vec<u8>>,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;

    let f = Arc::new(move |x| -> anyhow::Result<()> {
        let d = encode_bson_doc(&x).map_err(|e| anyhow::Error::msg(e.to_string()))?;
        emit.send(d)?;
        anyhow::Ok(())
    });

    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_find(&col, para.try_into()?, f).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;

    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_find_one(
    db_name: String,
    col: String,
    para: app::MongoFindParameter,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<Vec<u8>>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_find_one(&col, para.try_into()?).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(match ret {
        Some(x) => Some(encode_bson_doc(&x)?),
        None => None,
    })
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_find_one_and_delete(
    db_name: String,
    col: String,
    query: JsUint8Array,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<Vec<u8>>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(query.deref())?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_find_one_and_delete(&col, query).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;

    Ok(match ret {
        Some(x) => Some(encode_bson_doc(&x)?),
        None => None,
    })
}
#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_find_one_and_replace(
    db_name: String,
    col: String,
    query: JsUint8Array,
    replacement: JsUint8Array,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<Vec<u8>>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(&query)?;
    let rep = parse_bson_doc(&replacement)?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.query_collection_find_one_and_replace(&col, query, rep)
                .await
        }
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;

    Ok(match ret {
        Some(x) => Some(encode_bson_doc(&x)?),
        None => None,
    })
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_insert_many(
    db_name: String,
    col: String,
    data: Vec<JsUint8Array>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let mut docs = Vec::with_capacity(data.len());
    for d in data.iter() {
        let d = parse_bson_doc(d)?;
        docs.push(d);
    }
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_insert_many(&col, docs).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(serde_json::to_string(&ret)?)
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_insert_one(
    db_name: String,
    col: String,
    doc: JsUint8Array,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let doc = parse_bson_doc(doc.deref())?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_insert_one(&col, doc).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(serde_json::to_string(&ret)?)
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_list_index_names(
    db_name: String,
    col: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_list_index_names(&col).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(ret)
}
#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_list_indexes(
    db_name: String,
    col: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let indexes = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_list_indexes(&col).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(serde_json::to_string(&indexes)?)
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_list_search_indexes(
    db_name: String,
    col: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<Vec<u8>>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let indexes = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_list_search_indexes(&col).await,
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    let mut ret = Vec::with_capacity(indexes.len());
    for i in indexes.iter() {
        ret.push(encode_bson_doc(i)?);
    }
    Ok(ret)
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_replace_one(
    db_name: String,
    col: String,
    query: JsUint8Array,
    replacement: JsUint8Array,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(query.deref())?;
    let replacement = parse_bson_doc(replacement.deref())?;

    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.query_collection_replace_one(&col, query, replacement)
                .await
        }
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;

    Ok(serde_json::to_string(&ret)?)
}
#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_update_many(
    db_name: String,
    col: String,
    query: JsUint8Array,
    replacement: JsUint8Array,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(query.deref())?;
    let replacement = parse_bson_doc(replacement.deref())?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.query_collection_update_many(&col, query, replacement)
                .await
        }
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;

    Ok(serde_json::to_string(&ret)?)
}

#[specta::specta]
#[tauri::command]
pub async fn mongo_query_collection_update_one(
    db_name: String,
    col: String,
    query: JsUint8Array,
    replacement: JsUint8Array,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(query.deref())?;
    let replacement = parse_bson_doc(replacement.deref())?;
    let db = app
        .get_schema(db_name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.query_collection_update_one(&col, query, replacement)
                .await
        }
        None => return Err(format!("db '{}' not mongo", db_name).into()),
    }
    .map_err(ErrorFrontend::from)?;

    Ok(serde_json::to_string(&ret)?)
}

#[specta::specta]
#[tauri::command]
pub async fn redis_list_lpush(
    db_name: String,
    key: String,
    value: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_lpush(key.as_str(), value.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn redis_list_rpush(
    db_name: String,
    key: String,
    value: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_rpush(key.as_str(), value.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn redis_list_remove(
    db_name: String,
    key: String,
    value: String,
    count: f64,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_remove(key.as_str(), count as isize, value.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn redis_list_set(
    db_name: String,
    key: String,
    value: String,
    index: f64,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_set(key.as_str(), index as isize, value.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn redis_set_replace(
    db_name: String,
    key: String,
    old: String,
    new_value: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let old = hex::decode(old.as_bytes()).map_err(ErrorFrontend::from)?;
    let new = hex::decode(new_value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.set_replace(key.as_str(), old.as_slice(), new.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn redis_set_del(
    db_name: String,
    key: String,
    member: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(ErrorFrontend::from)?;
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.set_del(key.as_str(), member.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn redis_set_add(
    db_name: String,
    key: String,
    member: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(ErrorFrontend::from)?;
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.set_add(key.as_str(), member.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn redis_hash_replace(
    db_name: String,
    key: String,
    old_member: String,
    new_member: String,
    new_value: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let old_member = hex::decode(old_member.as_bytes()).map_err(ErrorFrontend::from)?;

    let new_member = hex::decode(new_member.as_bytes()).map_err(ErrorFrontend::from)?;
    let new_value = hex::decode(new_value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.hash_replace(key.as_str(), old_member.as_slice(), &new_member, &new_value)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn redis_hash_del(
    db_name: String,
    key: String,
    member: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.hash_del(key.as_str(), &member)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn redis_hash_add(
    db_name: String,
    key: String,
    member: String,
    value: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(ErrorFrontend::from)?;
    let value = hex::decode(value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.hash_add(key.as_str(), &member, &value)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn redis_zset_replace(
    db_name: String,
    key: String,
    old_member: String,
    new_member: String,
    new_value: f64,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let old_member = hex::decode(old_member.as_bytes()).map_err(ErrorFrontend::from)?;
    let new_member = hex::decode(new_member.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.zset_replace(key.as_str(), &old_member, &new_member, new_value)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn redis_zset_del(
    db_name: String,
    key: String,
    member: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.zset_del(key.as_str(), &member)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}
#[specta::specta]
#[tauri::command]
pub async fn redis_zset_add(
    db_name: String,
    key: String,
    member: String,
    value: f64,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.zset_add(key.as_str(), &member, value)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]
pub async fn list_procedure(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<NamedStaff>, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let sx = schema.list_procedure().await.map_err(ErrorFrontend::from)?;
    Ok(sx)
}

#[specta::specta]
#[tauri::command]
pub async fn show_procedure(
    db_name: String,
    procedure: NamedStaff,

    app: tauri::State<'_, DBPal>,
) -> Result<ProcedureDetail, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .show_procedure(&procedure)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn drop_procedure(
    db_name: String,
    procedure: NamedStaff,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .drop_procedure(&procedure)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn list_udf(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<NamedStaff>, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let ret = schema.list_udf().await.map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[specta::specta]
#[tauri::command]
pub async fn list_sequence(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<SequenceDetail>, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let ret = schema.list_sequence().await.map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[specta::specta]
#[tauri::command]
pub async fn drop_sequence(
    db_name: String,
    app: tauri::State<'_, DBPal>,
    seq: String,
    cascade: bool,
) -> Result<String, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .drop_sequence(&seq, cascade)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn rename_sequence(
    db_name: String,
    app: tauri::State<'_, DBPal>,
    old: String,
    new_name: String,
) -> Result<String, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .rename_sequence(&old, &new_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn show_udf(
    db_name: String,
    udf: NamedStaff,
    app: tauri::State<'_, DBPal>,
) -> Result<DefAndMetas, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema.show_udf(&udf).await.map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn drop_udf(
    db_name: String,
    udf: NamedStaff,

    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema.drop_udf(&udf).await.map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn redis_string_set(
    db_name: String,
    key: String,
    value: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.string_set(key.as_str(), &value)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", db_name).into()),
    }
    Ok(())
}

#[specta::specta]
#[tauri::command]

pub async fn write_xlsx(path: String, data: Vec<XlsxEdit>) -> Result<(), ErrorFrontend> {
    spawn_blocking(move || {
        let mut workbook = rust_xlsxwriter::workbook::Workbook::new();
        // Add a worksheet to the workbook.
        let worksheet = workbook.add_worksheet_with_constant_memory();

        for d in data {
            worksheet.write_string(d.row, d.column, d.content)?;
        }
        workbook.save(path)?;
        Ok(())
    })
    .await?
}

#[specta::specta]
#[tauri::command]
pub async fn put_value_in_sql(
    db_name: String,
    typ: String,
    value: Option<String>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    let value = match value {
        Some(value) => value,
        None => return Ok("NULL".to_string()),
    };
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    // due to this error mysql sometime confuse about the real type.
    // special case
    // https://github.com/launchbadge/sqlx/issues/3390
    {
        let x = schema.backend_kind();
        if x.mysql_like() {
            return Ok(if value.starts_with("0x") {
                value
            } else {
                quote_sql_str(&value)
            });
        }
    }
    schema
        .put_value_in_sql(&typ, &value)
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn put_values_in_sql(
    db_name: String,
    typ: String,
    values: Vec<Option<String>>,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<String>, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let mut ret = vec![];

    for value in values {
        let r = match value {
            Some(value) => schema
                .put_value_in_sql(&typ, &value)
                .map_err(ErrorFrontend::from)?,
            None => "null".to_string(),
        };
        ret.push(r);
    }
    Ok(ret)
}

#[specta::specta]
#[tauri::command]
pub async fn cassandra_list_aggregate(
    db_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<NamedStaff>, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let ret = match schema.as_any().downcast_ref::<CassandraDB>() {
        Some(x) => x.list_aggregate().await.map_err(ErrorFrontend::from)?,
        None => {
            return Err(ErrorFrontend::from(format!(
                "'{}' is not a cassandra backend",
                db_name
            )));
        }
    };
    Ok(ret)
}

#[specta::specta]
#[tauri::command]
pub async fn cassandra_show_aggregate(
    db_name: String,
    agg_name: String,
    parameters: Vec<String>,
    app: tauri::State<'_, DBPal>,
) -> Result<DefAndMetas, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let ret = match schema.as_any().downcast_ref::<CassandraDB>() {
        Some(x) => x
            .show_aggregate(&agg_name, parameters)
            .await
            .map_err(ErrorFrontend::from)?,
        None => {
            return Err(ErrorFrontend::from(format!(
                "'{}' is not a cassandra",
                db_name
            )));
        }
    };
    Ok(ret)
}

#[specta::specta]
#[tauri::command]
pub async fn cassandra_drop_aggregate(
    db_name: String,
    agg_name: String,
    parameters: Vec<String>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    match schema.as_any().downcast_ref::<CassandraDB>() {
        Some(x) => x
            .drop_aggregate(&agg_name, parameters)
            .await
            .map_err(ErrorFrontend::from),
        None => Err(ErrorFrontend::from(format!(
            "'{}' is not a cassandra",
            db_name
        ))),
    }
}

#[specta::specta]
#[tauri::command]
pub async fn drop_created_type(
    db_name: String,
    type_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    let schema = app
        .get_schema(db_name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    schema
        .drop_created_type(&type_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[specta::specta]
#[tauri::command]
pub async fn command_panic() -> Result<(), ErrorFrontend> {
    panic!("this is a panic for testing.")
}

#[specta::specta]
#[tauri::command]
pub async fn table_import_from_file(
    db_name: String,
    table_name: String,
    kind: TableImportPara,
    dbpal: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = dbpal.get_schema(&db_name).await?;
    schema
        .table_import_data_from_file(&table_name, kind)
        .await
        .map_err(ErrorFrontend::from)
}

#[derive(serde::Serialize, specta::Type)]
pub struct DocsAndCount {
    count: u32,
    /// list of docs in bson format.
    /// use this library to parse.
    /// https://github.com/mongodb/js-bson
    docs: Vec<Vec<u8>>,
}

impl TryFrom<dbnexus_db::mongo::DocsAndCount> for DocsAndCount {
    type Error = String;

    fn try_from(value: dbnexus_db::mongo::DocsAndCount) -> Result<Self, Self::Error> {
        let mut vec = Vec::with_capacity(value.docs.len());
        for index in 0..value.docs.len() {
            let bs = encode_bson_doc(&value.docs[index])?;
            vec.push(bs);
        }
        Ok(Self {
            count: value.count as u32,
            docs: vec,
        })
    }
}
fn encode_bson_doc(doc: &Document) -> Result<Vec<u8>, String> {
    let bs = bson::to_vec(&doc).map_err(|e| e.to_string())?;
    Ok(bs)
}

fn block_random_sleep() {
    if cfg!(feature = "random-sleep") {
        let duration = {
            let mut rng = rand::rng();
            let millis = rng.random_range(500..1700);
            std::time::Duration::from_millis(millis)
        };
        tokio::task::block_in_place(|| std::thread::sleep(duration));
    }
}
async fn random_sleep() {
    if cfg!(feature = "random-sleep") {
        let duration = {
            let mut rng = rand::rng();
            let millis = rng.random_range(500..1700);

            std::time::Duration::from_millis(millis)
        };
        tokio::time::sleep(duration).await
    }
}

fn random_error() -> Result<(), ErrorFrontend> {
    if cfg!(feature = "random-error") {
        {
            let mut rng = rand::rng();
            let x: i8 = rng.random_range(1..100);
            if x > 50 {
                Err(ErrorFrontend {
                    reason: format!("random error of {}", x),
                    detail: "".to_string(),
                    cancelled: false,
                })
            } else {
                Ok(())
            }
        }
    } else {
        Ok(())
    }
}

#[derive(Clone, Debug, serde::Serialize, specta::Type)]
pub struct MuiDataGridData {
    fields: Vec<Field>,
    data: Vec<HashMap<String, Option<String>>>,
    sql: Option<String>,
    total: f64,
}

impl MuiDataGridData {
    pub async fn preview_excel(path: &str, max_row: u32) -> anyhow::Result<Self> {
        let max_row = max_row as usize;
        let path = path.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut wb: calamine::Xlsx<std::io::BufReader<std::fs::File>> =
                calamine::open_workbook(path)?;
            if wb.sheet_names().len() == 0 {
                anyhow::bail!("this excel file don't contains any sheet")
            }
            let sheet_name = &wb.sheet_names()[0];

            let range = wb.worksheet_range(sheet_name).unwrap();

            let mut headers = vec![];
            let mut data = vec![];

            for (row_idx, row) in range.rows().enumerate() {
                if row_idx == 0 {
                    headers = row
                        .iter()
                        .map(|x| Field::from_name_type(x.to_string(), ""))
                        .collect();
                    continue;
                }

                let mut values = HashMap::new();

                for (h, v) in headers.iter().zip(row.iter().map(|x| x.to_string())) {
                    values.insert(h.field.clone(), Some(v));
                }
                data.push(values);
                if data.len() >= max_row {
                    break;
                }
            }
            anyhow::Ok(Self {
                fields: headers,
                data,
                sql: None,
                total: 0f64,
            })
        })
        .await?
    }
}

impl From<QueryResp> for MuiDataGridData {
    fn from(value: QueryResp) -> Self {
        let QueryResp {
            fields,
            data,
            total,
            sql,
        } = value;
        #[derive(Default)]
        struct NameIndex {
            indexes: HashMap<String, usize>,
        }
        impl NameIndex {
            fn get_index(&mut self, db_name: &String) -> Option<usize> {
                match self.indexes.get_mut(db_name) {
                    Some(x) => {
                        *x += 1;
                        Some(*x)
                    }
                    None => {
                        self.indexes.insert(db_name.clone(), 1);
                        None
                    }
                }
            }
        }

        let fields_renamed = {
            let mut indexes = NameIndex::default();
            fields
                .into_iter()
                .map(|mut x| match indexes.get_index(&x.field) {
                    Some(index) => {
                        x.field = format!("{}#{}", x.field, index);
                        x
                    }
                    None => x,
                })
                .collect::<Vec<_>>()
        };

        let data: Vec<_> = data
            .into_iter()
            .map(|x| {
                let mut values = HashMap::new();
                fields_renamed.iter().zip(x).for_each(|(f, value)| {
                    values.insert(f.field.clone(), value);
                });
                values
            })
            .collect();

        Self {
            fields: fields_renamed,
            data,
            sql,
            total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bson_serial() {
        let d = bson::doc! {"x" : 1 ,"b" : 2};
        let d2 = parse_bson_doc(&encode_bson_doc(&d).unwrap()).unwrap();
        assert_eq!(format!("{:?}", d), format!("{:?}", d2));
    }
    #[test]
    fn test_anyhow_display_and_debug() {
        let err = anyhow::Error::msg("aaaa");
        println!("{}", err);
        println!("#####################################");
        println!("{:?}", err);
    }

    #[test]
    fn test_bson_array() {
        let x = vec![
            bson::Bson::Int64(1),
            bson::Bson::Int64(10),
            bson::Bson::Int64(100),
            bson::Bson::JavaScriptCode(
                r#"
                function(){}
            "#
                .to_owned(),
            ),
            bson::Bson::ObjectId(bson::oid::ObjectId::new()),
        ];
        let x = serde_json::to_string_pretty(&x).unwrap();
        println!("{}", x)
    }

    #[test]
    fn test_json_valid_error_has_line() {
        use serde_json::Value;

        let bad = r#"{
        "db_name": "Rust"
        "age": 12    // ← 少逗号
    }"#;

        match serde_json::from_str::<Value>(bad) {
            Ok(_) => println!("合法 JSON"),
            Err(e) => {
                println!("{}:{}", e.line(), e.column());
            }
        }
    }
}
