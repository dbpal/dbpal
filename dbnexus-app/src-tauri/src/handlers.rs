use crate::app::{
    self, name_to_parts, parse_bson_doc, CancelTokenMsg, DBConnectionConfig, DBPal,
    DatabaseBackend, ErrorFrontend, MongoShowTablePara, TaskManager,
};
use dashmap::DashMap;
use dbnexus_db::cassandra::CassandraDB;
use dbnexus_db::clickhouse::ClickHouseDB;
use dbnexus_db::couchbase::CouchBaseDB;
use dbnexus_db::db::{
    escape_sql_str, ColumnType, CreateDatabaseExtra, CreateIndexReq, CreateSchemaExtra,
    CreateTableAndIndexSql, CreateTableReq, CreatedTypeDef, DBTrait, DefAndMetas, ExportKind,
    Field, Index, ListTable, MetaValue, NamedStaff, OptionMustBeSomeErr, ProcedureDetail,
    QueryEventEmit, QueryResp, SequenceDetail, ShowTableParameter, SqlEvent, TimeElapsed,
    TriggerDetail, UdfOrProcedureExtraParameter,
};
use dbnexus_db::language_server::{CompletionList, ErrAndLoc};
use dbnexus_db::mongo::MongoDB;
use dbnexus_db::mssql::SqlServerDB;
use dbnexus_db::mysql::MysqlDB;
use dbnexus_db::oracle::OracleDB;
use dbnexus_db::pg::PostGresDB;
use dbnexus_db::redis::{redis_tokenize, RedisDB, RedisListTable};
use dbnexus_db::redis_lang_server::REDIS_COMMANDS_MD_FILES;
use include_dir::{include_dir, Dir};
use mongodb::bson::{self, doc, Document};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, RwLock};
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::State;
use tokio_util::sync::CancellationToken;

pub fn log_level_ref() -> &'static RwLock<Option<log::Level>> {
    static LOG_LEVEL_LOC: RwLock<Option<log::Level>> = RwLock::new(None);
    &LOG_LEVEL_LOC
}

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

#[tauri::command]
pub async fn get_locale() -> Result<String, ErrorFrontend> {
    let r = sys_locale::get_locale().unwrap_or_else(|| String::from("en-US"));
    Ok(r)
}

#[tauri::command]
pub async fn monaco_editor_themes() -> Result<HashMap<&'static str, &'static str>, ErrorFrontend> {
    let mut ret = HashMap::new();
    static FILES: Dir = include_dir!("$CARGO_MANIFEST_DIR/../monaco-themes/themes");
    for en in FILES.entries() {
        match en {
            include_dir::DirEntry::Dir(..) => {}
            include_dir::DirEntry::File(file) => {
                let name = file
                    .path()
                    .file_name()
                    .and_then(|x| x.to_str())
                    .expect("filename convert to str should be ok");
                if name == "themelist.json" {
                    continue;
                }
                let name = name.strip_suffix(".json").expect(" must end with .json");
                ret.insert(name, file.contents_utf8().expect("must be utf8"));
            }
        }
    }
    Ok(ret)
}

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
                                        let name = &name[0..(name.len() - 3)];
                                        if name.contains("-") {
                                            let name =
                                                name.split("-").collect::<Vec<_>>().join(" ");
                                            name
                                        } else {
                                            name.to_string()
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

#[tauri::command]
pub async fn all_font_family() -> Result<Vec<String>, ErrorFrontend> {
    let source = font_kit::source::SystemSource::new();
    let fonts = source.all_families().map_err(ErrorFrontend::from)?;
    Ok(fonts)
}

#[tauri::command]
pub async fn page_init_callback(app: tauri::State<'_, DBPal>) -> Result<(), ()> {
    log::info!("page init call clear all open connection.");
    app.backend.lock().await.clear();
    Ok(())
}

#[tauri::command]
pub async fn drop_database(
    name: String,
    database: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("drop db:{} {}", name, database);
    let backend = app
        .get_backend(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    // first close connections because we want delete it
    let key = format!("{}/{}", name, database);
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
            name
        ))),
    }
}

#[tauri::command]
pub async fn drop_schema(
    name: String,
    schema: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("drop schema:{} {}", name, schema);

    // first close connections because we want delete it
    let key = format!("{}/{}", name, schema);
    app.close_conn_has_prefix(&key).await;
    let backend = app
        .get_backend(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match backend {
        DatabaseBackend::DataBase(x) => {
            app.backend.lock().await.remove(&key);
            x.drop_schema(&schema).await.map_err(ErrorFrontend::from)
        }
        _ => Err(ErrorFrontend::from(format!(
            "'{}' is not a db database",
            name
        ))),
    }
}

#[tauri::command]
pub async fn create_database(
    name: String,
    database: String,
    extra: Option<CreateDatabaseExtra>,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let backend = app
        .get_backend(name.as_str())
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
            name
        ))),
    }
}

#[tauri::command]
pub async fn create_schema(
    name: String,
    schema: String,
    extra: Option<CreateSchemaExtra>,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let backend = app
        .get_backend(name.as_str())
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
            name
        ))),
    }
}

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

#[tauri::command]
pub async fn close_conn(name: String, app: tauri::State<'_, DBPal>) -> Result<(), ()> {
    random_sleep().await;
    app.close_conn_has_prefix(name.as_str()).await;
    Ok(())
}

#[tauri::command]
pub async fn list_table(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<ListTable>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.list_table().await.map_err(ErrorFrontend::from)
}
#[tauri::command]
pub async fn mongo_list_time_serial_collection(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<ListTable>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app.get_schema(&name).await.map_err(ErrorFrontend::from)?;
    let ret = match schema.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x
            .list_time_serial_collection()
            .await
            .map_err(ErrorFrontend::from)?,
        None => {
            return Err(ErrorFrontend::from(format!("'{}' is not a mongo", name)));
        }
    };

    Ok(ret)
}

#[tauri::command]
pub async fn redis_list_table(
    name: String,
    filter: Option<String>,
    max: usize,
    app: tauri::State<'_, DBPal>,
) -> Result<RedisListTable, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await.map_err(ErrorFrontend::from)?;
    match db.as_any().downcast_ref::<RedisDB>() {
        Some(x) => x.redis_list_table(filter.as_deref(), max).await,
        None => return Err(ErrorFrontend::from(format!("'{}' not a redis", name))),
    }
    .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn show_table_columns(
    name: String,
    table_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<Field>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.show_table_columns(&table_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn show_table(
    name: String,
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
    let db = app.get_schema(&name).await?;
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

#[tauri::command]
pub async fn show_indexes(
    name: String,
    table_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<Index>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.show_indexes(&table_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn create_index(
    name: String,
    table_name: String,
    req: CreateIndexReq,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    schema
        .create_index(&table_name, &req)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn mongo_create_index(
    name: String,
    table_name: String,
    index_name: Option<String>,
    keys: Vec<u8>,
    unique: bool,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let keys = parse_bson_doc(&keys)?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x
            .create_index(&table_name, index_name, unique, keys)
            .await
            .map_err(ErrorFrontend::from),
        None => Err(ErrorFrontend::from(format!("'{}' not a mongo", name))),
    }
}
#[tauri::command]
pub async fn rename_index(
    name: String,
    table_name: String,
    old_name: String,
    new_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .rename_index(&table_name, &old_name, &new_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn drop_index(
    name: String,
    table_name: String,
    index: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    schema
        .drop_index(&table_name, &index)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn table_insert(
    name: String,
    table_name: String,
    values: HashMap<String, Option<String>>,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<usize>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.table_insert(&table_name, values)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn table_update(
    name: String,
    table_name: String,
    values: HashMap<String, Option<String>>,
    wheres: HashMap<String, Option<String>>,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<usize>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.table_update(&table_name, values, wheres)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn table_update_sql(
    name: String,
    table_name: String,
    values: HashMap<String, Option<String>>,
    wheres: HashMap<String, Option<String>>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.table_update_sql(&table_name, values, wheres)
        .await
        .map_err(ErrorFrontend::from)
}
#[tauri::command]
pub async fn truncate_table(
    name: String,
    table_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.truncate_table(&table_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn drop_table_column(
    name: String,
    table_name: String,
    column_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .drop_table_column(&table_name, &column_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn modify_table_column(
    name: String,
    table_name: String,
    old_name: String,
    new_name: String,
    typ: Option<ColumnType>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .modify_table_column(&table_name, &old_name, &new_name, typ)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn add_table_column(
    name: String,
    table_name: String,
    col_name: String,
    typ: ColumnType,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .add_table_column(&table_name, &col_name, typ)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn list_data_types(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let ret = schema
        .list_data_types()
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn list_sql_current_values(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<&'static [&'static str], ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let ret = schema.list_sql_current_values();
    Ok(ret)
}

#[tauri::command]
pub async fn table_delete_sql(
    name: String,
    table_name: String,
    wheres: HashMap<String, Option<String>>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.table_delete_sql(&table_name, wheres)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn table_delete(
    name: String,
    table_name: String,
    wheres: HashMap<String, Option<String>>,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<usize>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.table_delete(&table_name, wheres)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn list_view(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<ListTable>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.list_view().await.map_err(ErrorFrontend::from)
}
#[tauri::command]
pub async fn server_metas(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<MetaValue>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let backend = app.get_backend(&name).await.map_err(ErrorFrontend::from)?;
    backend
        .as_server_mates()
        .show_metas()
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn get_redis_type_hint(
    name: String,
    text: String,
    query_id: &str,
    app: tauri::State<'_, DBPal>,
) -> Result<HashMap<String, String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => x.get_redis_type_hint(&text, query_id).await,
        None => return Err(ErrorFrontend::from("this is not redis db")),
    }
    .map_err(ErrorFrontend::from)
}

impl DatabaseBackend {
    fn db_trait(&self) -> Option<&dyn DBTrait> {
        match self {
            DatabaseBackend::Catalog(_x) => None,
            DatabaseBackend::DataBase(x) => Some(x.as_ref()),
            DatabaseBackend::Schema(x) => {
                let a = x.as_any();
                if let Some(x) = a.downcast_ref::<MysqlDB>() {
                    Some(x as &dyn DBTrait)
                } else if let Some(x) = a.downcast_ref::<PostGresDB>() {
                    Some(x as &dyn DBTrait)
                } else if let Some(x) = a.downcast_ref::<SqlServerDB>() {
                    Some(x as &dyn DBTrait)
                } else if let Some(x) = a.downcast_ref::<OracleDB>() {
                    Some(x as &dyn DBTrait)
                } else if let Some(x) = a.downcast_ref::<CassandraDB>() {
                    Some(x as &dyn DBTrait)
                } else if let Some(x) = a.downcast_ref::<ClickHouseDB>() {
                    Some(x as &dyn DBTrait)
                } else if let Some(x) = a.downcast_ref::<CouchBaseDB>() {
                    Some(x as &dyn DBTrait)
                } else {
                    None
                }
            }
        }
    }
}
impl DBPal {
    async fn ai_get_search_path(
        &self,
        backend: DatabaseBackend,
        name: &str,
        query_id: &str,
        text: &str,
    ) -> anyhow::Result<Vec<String>> {
        match &backend {
            DatabaseBackend::Catalog(..) => {
                anyhow::bail!("this is a interval error,we should never hit here")
            }
            DatabaseBackend::DataBase(x) => {
                // a
                let text = text.to_lowercase();
                let mut r = HashSet::<String>::new();
                let databasese = x.list_schema().await?;
                for d in databasese {
                    if text.contains(d.to_lowercase().as_str()) {
                        r.insert(d);
                    }
                }
                r.extend(x.get_search_path(query_id).await?.into_iter());
                Ok(r.into_iter().map(|x| format!("{}/{}", name, x)).collect())
            }
            DatabaseBackend::Schema(..) => {
                let names = name_to_parts(name);
                let db_trait = backend.db_trait();
                match db_trait {
                    Some(x) if names.len() == 2 => {
                        let text = text.to_lowercase();
                        let mut r = HashSet::<String>::new();
                        let databasese = x.list_schema().await?;
                        for d in databasese {
                            if text.contains(d.to_lowercase().as_str()) {
                                r.insert(d);
                            }
                        }
                        r.extend(x.get_search_path(query_id).await?.into_iter());
                        Ok(r.into_iter()
                            .map(|x| format!("{}/{}", names[0], x))
                            .collect())
                    }
                    _ => anyhow::Ok(vec![name.to_string()]),
                }
            }
        }
    }
}

#[tauri::command]
pub async fn ai_get_search_path(
    name: String,
    query_id: &str,
    text: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<String>, ErrorFrontend> {
    let backend = app.get_backend(&name).await.map_err(ErrorFrontend::from)?;
    app.ai_get_search_path(backend, &name, query_id, &text)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn list_materialized_view(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<ListTable>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .list_materialized_view()
        .await
        .map_err(ErrorFrontend::from)
}
#[tauri::command]
pub async fn drop_view(
    name: String,
    view_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.drop_view(&view_name).await.map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn drop_materialized_view(
    name: String,
    view_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app.get_schema(&name).await.map_err(ErrorFrontend::from)?;
    schema
        .drop_materialized_view(&view_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn drop_table(
    name: String,
    table_name: String,
    cascade: bool,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.drop_table(&table_name, cascade)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn show_view_columns(
    name: String,
    view_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<Field>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app.get_schema(&name).await.map_err(ErrorFrontend::from)?;
    schema
        .show_view_columns(&view_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn list_created_types(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<CreatedTypeDef>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app.get_schema(&name).await.map_err(ErrorFrontend::from)?;
    schema
        .list_created_types()
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn list_event(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<SqlEvent>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app.get_schema(&name).await.map_err(ErrorFrontend::from)?;
    schema.list_event().await.map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn drop_event(
    name: String,
    event_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app.get_schema(&name).await.map_err(ErrorFrontend::from)?;
    schema
        .drop_event(&event_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn export_query_result_to_file(
    name: String,
    task_id: String,
    sql: String,
    query_id: String,
    kind: ExportKind,
    file: String,
    size_emitter: Channel<usize>,
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
        .get_backend(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let size_emitter = Arc::new(move |size| {
        if let Err(err) = size_emitter.send(size) {
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

#[tauri::command]
pub async fn is_dangerous_sql(
    name: String,
    sql: String,
    app: tauri::State<'_, DBPal>,
) -> Result<bool, ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let db = app
        .get_backend(name.as_str())
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
    limit: Option<usize>,
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
    fn new(channel: Channel<QueryEvent>, cancel_msg: CancelTokenMsg, limit: Option<usize>) -> Self {
        Self {
            channel,
            cancel_msg,
            limit,
            emitted_size: AtomicUsize::new(0),
            cache: Default::default(),
            time_elapsed: Arc::new(Mutex::new(TimeElapsed::new())),
        }
    }
    fn get_limit(&self) -> usize {
        self.limit.unwrap_or(usize::MAX)
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
        if total > self.get_limit() {
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

#[tauri::command]
pub async fn query_sql(
    name: String,
    query_id: &str,
    max_row: usize,
    sql: String,
    app: tauri::State<'_, DBPal>,
    emit: Channel<QueryEvent>,
    task_manager: tauri::State<'_, TaskManager>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_backend(name.as_str())
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

#[derive(Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum QueryEvent {
    Err { err: String },
    Status { status: String },
    Data { data: Vec<Vec<Option<String>>> },
    Structure { resp: MuiDataGridData },
    End,
}

#[tauri::command]
pub async fn simple_execute_sql(
    name: String,
    sql: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<usize>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let db = app
        .get_backend(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let result = db
        .as_query_sql()
        .simple_execute_sql(&sql)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(result)
}

#[tauri::command]
pub async fn execute_sql(
    name: String,
    query_id: &str,
    sql: String,
    app: tauri::State<'_, DBPal>,
    task_manager: tauri::State<'_, TaskManager>,
) -> Result<Result<MuiDataGridData, String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_backend(&name).await.map_err(ErrorFrontend::from)?;
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

#[tauri::command]
pub async fn query_sql_open_connection(
    name: String,
    id: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_backend(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        db.as_query_sql().open_connection(&id).await
    })
    .await
    .map_err(|_e| ErrorFrontend::from("open connection not finished in reasonable time"))?;
    result.map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn query_sql_close_connection(
    name: String,
    query_id: &str,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("close conn:{} {}", name, query_id);
    let db = app
        .get_backend(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let result = db.as_query_sql().close_connection(query_id).await;
    result.map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn export_table_or_view(
    name: String,
    task_id: String,
    task_manager: tauri::State<'_, TaskManager>,
    size_emitter: Channel<usize>,
    table_or_view: String,
    para: Option<ShowTableParameter>,
    filepath: String,
    kind: ExportKind,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
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
                if let Err(err) = size_emitter.send(size) {
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

#[tauri::command]
pub async fn check_conn(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<
    u128, // human readable time
    ErrorFrontend,
> {
    random_sleep().await;
    random_error()?;
    let timeout = 5;
    tokio::time::timeout(Duration::from_secs(timeout), async move {
        let db = app.get_backend(&name).await?;
        db.check_conn()
            .await
            .map_err(ErrorFrontend::from)
            .map(|d| d.as_millis())
    })
    .await
    .map_err(|_| {
        ErrorFrontend::from(format!(
            "check connection status not finished in {}s",
            timeout
        ))
    })?
}

#[tauri::command]
pub async fn test_connection(
    config: DBConnectionConfig,
) -> Result<
    u64, // milliseconds
    ErrorFrontend,
> {
    random_error()?;
    let timeout = 5;
    tokio::time::timeout(Duration::from_secs(5), async {
        let db = config.open().await.map_err(|e| e.to_string())?;
        db.check_conn()
            .await
            .map_err(ErrorFrontend::from)
            .map(|d| d.as_millis() as u64)
    })
    .await
    .map_err(|_| ErrorFrontend::from(format!("test connection not finished in {}s", timeout)))?
}

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

#[tauri::command]
pub async fn conn_opened(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<bool, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let opened = app.conn_opened(&name).await;
    Ok(opened)
}

#[tauri::command]
pub async fn list_database(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<
    Vec<String>, // milliseconds
    ErrorFrontend,
> {
    random_sleep().await;
    random_error()?;
    let db = app.get_backend(name.as_str()).await?;
    // app.enter_open_child(name.as_str(), &db).await?;
    match db {
        app::DatabaseBackend::Catalog(x) => x.list_database().await,
        _ => {
            return Err(ErrorFrontend::from(format!(
                "not catalog trait but:{}",
                name
            )));
        }
    }
    .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn list_schema(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_backend(name.as_str()).await?;
    match db {
        app::DatabaseBackend::DataBase(x) => x.list_schema().await,
        app::DatabaseBackend::Catalog(_) => {
            return Err(ErrorFrontend::from(format!(
                "not database trait but a catalog of {}",
                name
            )));
        }
        app::DatabaseBackend::Schema(_) => {
            return Err(ErrorFrontend::from(format!(
                "not database trait but a schema of {}",
                name
            )));
        }
    }
    .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn extract_table_select_selection(
    name: String,
    sql: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_backend(name.as_str()).await?;
    let ret = db
        .as_language_server()
        .extract_table_select_selection(&sql)
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn extract_table_select_sort(
    name: String,
    sql: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_backend(name.as_str()).await?;
    let ret = db
        .as_language_server()
        .extract_table_select_sort(&sql)
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

fn auto_completion_mutex(name: &str) -> Arc<tokio::sync::Mutex<()>> {
    static LOCKS: LazyLock<Arc<DashMap<String, Arc<tokio::sync::Mutex<()>>>>> =
        LazyLock::new(|| Arc::new(DashMap::new()));
    let name = name.to_owned();
    match LOCKS.get(&name) {
        Some(x) => x.clone(),
        None => {
            let x = Arc::new(tokio::sync::Mutex::new(()));
            LOCKS.insert(name, x.clone());
            x.clone()
        }
    }
}

#[tauri::command]
pub async fn auto_completion(
    name: String,
    query_id: Option<String>,
    sql: String,
    line: u32,
    col: u32,
    app: tauri::State<'_, DBPal>,
) -> Result<CompletionList, ErrorFrontend> {
    let use_lexer = true;
    // 避免多个线程同时访问
    // 主要是后端实现更简单了比如可以把 completion——id缓存起来
    let lock = auto_completion_mutex(name_to_parts(&name)[0]);
    let _guard = lock.lock().await;

    let db = app.get_backend(name.as_str()).await?;
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

#[tauri::command]
pub async fn ai_text_completion(
    name: String,
    text: String,
    query_id: Option<&str>,
    line: u32,
    col: u32,
    app: tauri::State<'_, DBPal>,
) -> Result<CompletionList, ErrorFrontend> {
    let db = app.get_backend(name.as_str()).await?;
    let uuids = if let Some(query_id) = query_id {
        app.ai_get_search_path(db, &name, query_id, &text).await?
    } else {
        vec![name.to_string()]
    };
    let mut ret = vec![];
    for uuid in uuids {
        let db = app.get_backend(&uuid).await?;
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
        ret.extend(list.items);
    }
    Ok(ret.into())
}

#[tauri::command]
pub async fn check_sql(
    name: String,
    sql: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<ErrAndLoc>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_backend(name.as_str()).await?;
    let ret = db
        .as_language_server()
        .check_sql(&sql)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn try_get_max_id(
    name: String,
    table: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_backend(name.as_str()).await?;
    let ret = db
        .as_language_server()
        .try_get_max_id(&table)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn query_set_autocommit(
    name: String,
    query_id: &str,
    commit: bool,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let backend = app
        .get_backend(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    backend
        .as_query_sql()
        .set_auto_commit(query_id, commit)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn query_get_commit(
    name: String,
    query_id: &str,
    app: tauri::State<'_, DBPal>,
) -> Result<bool, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let backend = app
        .get_backend(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    backend
        .as_query_sql()
        .get_auto_commit(query_id)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn query_commit(
    name: String,
    query_id: &str,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let backend = app
        .get_backend(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    backend
        .as_query_sql()
        .commit(query_id)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn query_rollback(
    name: String,
    query_id: &str,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_backend(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    db.as_query_sql()
        .rollback(query_id)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn drop_trigger(
    name: String,
    table_name: String,
    trigger: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    db.drop_trigger(&table_name, &trigger)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn enable_trigger(
    name: String,
    table_name: String,
    trigger: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    db.enable_trigger(&table_name, &trigger)
        .await
        .map_err(ErrorFrontend::from)
}
#[tauri::command]
pub async fn disable_trigger(
    name: String,
    table_name: String,
    trigger: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    db.disable_trigger(&table_name, &trigger)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn show_trigger(
    name: String,
    table_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<TriggerDetail>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
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

#[tauri::command]
pub async fn rename_table(
    name: String,
    old: String,
    new: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    db.rename_table(&old, &new)
        .await
        .map_err(ErrorFrontend::from)
}
#[tauri::command]
pub async fn create_table_req_sql(
    name: String,
    req: CreateTableReq,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let ret = db
        .create_table_sql_string(&req)
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]

pub async fn create_table(
    name: String,
    req: CreateTableReq,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    db.create_table(&req).await.map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn show_create_table_sql(
    name: String,
    table_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<CreateTableAndIndexSql, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let ret = db
        .show_create_table_sql(&table_name)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn create_view_sql(
    name: String,
    view_name: String,
    materialized: bool,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let ret = db
        .show_view_sql(&view_name, materialized)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn duplicate_table(
    name: String,
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
        .get_schema(name.as_str())
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

#[tauri::command]
pub async fn mongo_insert(
    name: String,
    table_name: String,
    doc: Vec<u8>,
    app: tauri::State<'_, DBPal>,
) -> Result<mongodb::results::InsertOneResult, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let doc = parse_bson_doc(doc.as_slice())?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.insert(table_name.as_str(), doc).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn mongo_update_one(
    name: String,
    table_name: String,
    doc: Vec<u8>,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let doc = parse_bson_doc(doc.as_slice())?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.update_one(table_name.as_str(), doc).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    if ret.modified_count != 1 {
        return Err(ErrorFrontend::from("modified count is not 1"));
    }
    Ok(())
}

#[tauri::command]
pub async fn mongo_delete_ids(
    name: String,
    table_name: String,
    ids: String,
    app: tauri::State<'_, DBPal>,
) -> Result<mongodb::results::DeleteResult, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let ids: Vec<bson::Bson> = serde_json::from_str(&ids)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.delete_by_ids(table_name.as_str(), ids).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn mongo_show_collection_table_list(
    name: String,
    table_name: String,
    para: MongoShowTablePara,
    app: tauri::State<'_, DBPal>,
) -> Result<MuiDataGridData, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.show_collection_table_list(table_name.as_str(), para.try_into()?)
                .await
        }
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)
    .map(MuiDataGridData::from)
}

#[tauri::command]
pub async fn mongo_show_collection_docs(
    name: String,
    table_name: String,
    app: tauri::State<'_, DBPal>,
    para: MongoShowTablePara,
) -> Result<DocsAndCount, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let docs = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.show_collection_docs(table_name.as_str(), para.try_into()?)
                .await
        }
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    DocsAndCount::try_from(docs).map_err(|e| e.into())
}
#[tauri::command]
pub async fn mongo_get_one_document_by_id(
    name: String,
    table_name: String,
    id: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<u8>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let id: bson::Bson = serde_json::from_str(&id)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let doc = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.get_one_document_by_id(table_name.as_str(), id).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    encode_bson_doc(&doc).map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn mongo_query_db_drop(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_db_drop().await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(())
}
#[tauri::command]
pub async fn mongo_query_db_list_collection_names(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_db_list_collection_names().await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(ret)
}
#[tauri::command]
pub async fn mongo_query_db_run_command(
    name: String,
    command: Vec<u8>,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<u8>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let command = parse_bson_doc(&command)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_db_run_command(command).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(encode_bson_doc(&ret)?)
}

#[tauri::command]
pub async fn mongo_query_collection_count_docs(
    name: String,
    col: String,
    app: tauri::State<'_, DBPal>,
) -> Result<u64, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_count_docs(&col).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn mongo_query_collection_create_index(
    name: String,
    col: String,
    index: Vec<u8>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let index = parse_bson_doc(&index)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_create_index(&col, index).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(ret.index_name)
}
#[tauri::command]
pub async fn mongo_query_collection_create_search_index(
    name: String,
    col: String,
    index: Vec<u8>,
    index_name: Option<String>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let index = parse_bson_doc(&index)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.query_collection_create_search_index(&col, index, index_name)
                .await
        }
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(ret)
}
#[tauri::command]
pub async fn mongo_query_collection_delete_many(
    name: String,
    col: String,
    query: Vec<u8>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(&query)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_delete_many(&col, query).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(serde_json::to_string(&ret)?)
}
#[tauri::command]
pub async fn mongo_query_collection_delete_one(
    name: String,
    col: String,
    query: Vec<u8>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(&query)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_delete_one(&col, query).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(serde_json::to_string(&ret)?)
}

#[tauri::command]
pub async fn mongo_query_collection_distinct(
    name: String,
    col: String,
    query: Vec<u8>,
    field: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(&query)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let values = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_distinct(&col, query, &field).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    serde_json::to_string(&values).map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn mongo_query_collection_drop(
    name: String,
    col: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_drop(&col).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(())
}

#[tauri::command]
pub async fn mongo_query_collection_drop_index(
    name: String,
    col: String,
    index: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_drop_index(&col, &index).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(())
}

#[tauri::command]
pub async fn mongo_query_collection_drop_indexes(
    name: String,
    col: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_drop_indexes(&col).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(())
}
#[tauri::command]
pub async fn mongo_query_collection_drop_search_index(
    name: String,
    col: String,
    index: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_drop_search_index(&col, &index).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(())
}

#[tauri::command]
pub async fn mongo_query_collection_find(
    name: String,
    col: String,
    para: app::MongoFindParameter,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<Vec<u8>>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let docs = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_find(&col, para.try_into()?).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    let mut ret = Vec::with_capacity(docs.len());
    for d in docs.iter() {
        ret.push(encode_bson_doc(d)?);
    }
    Ok(ret)
}

#[tauri::command]
pub async fn mongo_query_collection_find_one(
    name: String,
    col: String,
    para: app::MongoFindParameter,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<Vec<u8>>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_find_one(&col, para.try_into()?).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(match ret {
        Some(x) => Some(encode_bson_doc(&x)?),
        None => None,
    })
}

#[tauri::command]
pub async fn mongo_query_collection_find_one_and_delete(
    name: String,
    col: String,
    query: Vec<u8>,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<Vec<u8>>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(&query)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_find_one_and_delete(&col, query).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;

    Ok(match ret {
        Some(x) => Some(encode_bson_doc(&x)?),
        None => None,
    })
}
#[tauri::command]
pub async fn mongo_query_collection_find_one_and_replace(
    name: String,
    col: String,
    query: Vec<u8>,
    replacement: Vec<u8>,
    app: tauri::State<'_, DBPal>,
) -> Result<Option<Vec<u8>>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(&query)?;
    let rep = parse_bson_doc(&replacement)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.query_collection_find_one_and_replace(&col, query, rep)
                .await
        }
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;

    Ok(match ret {
        Some(x) => Some(encode_bson_doc(&x)?),
        None => None,
    })
}

#[tauri::command]
pub async fn mongo_query_collection_insert_many(
    name: String,
    col: String,
    data: Vec<Vec<u8>>,
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
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_insert_many(&col, docs).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(serde_json::to_string(&ret)?)
}

#[tauri::command]
pub async fn mongo_query_collection_insert_one(
    name: String,
    col: String,
    doc: Vec<u8>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let doc = parse_bson_doc(&doc)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_insert_one(&col, doc).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(serde_json::to_string(&ret)?)
}

#[tauri::command]
pub async fn mongo_query_collection_list_index_names(
    name: String,
    col: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_list_index_names(&col).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(ret)
}
#[tauri::command]
pub async fn mongo_query_collection_list_indexes(
    name: String,
    col: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let indexes = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_list_indexes(&col).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(serde_json::to_string(&indexes)?)
}

#[tauri::command]
pub async fn mongo_query_collection_list_search_indexes(
    name: String,
    col: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<Vec<u8>>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let indexes = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => x.query_collection_list_search_indexes(&col).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    let mut ret = Vec::with_capacity(indexes.len());
    for i in indexes.iter() {
        ret.push(encode_bson_doc(i)?);
    }
    Ok(ret)
}

#[tauri::command]
pub async fn mongo_query_collection_replace_one(
    name: String,
    col: String,
    query: Vec<u8>,
    replacement: Vec<u8>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(&query)?;
    let replacement = parse_bson_doc(&replacement)?;

    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.query_collection_replace_one(&col, query, replacement)
                .await
        }
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;

    Ok(serde_json::to_string(&ret)?)
}
#[tauri::command]
pub async fn mongo_query_collection_update_many(
    name: String,
    col: String,
    query: Vec<u8>,
    replacement: Vec<u8>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(&query)?;
    let replacement = parse_bson_doc(&replacement)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.query_collection_update_many(&col, query, replacement)
                .await
        }
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;

    Ok(serde_json::to_string(&ret)?)
}

#[tauri::command]
pub async fn mongo_query_collection_update_one(
    name: String,
    col: String,
    query: Vec<u8>,
    replacement: Vec<u8>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(&query)?;
    let replacement = parse_bson_doc(&replacement)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.query_collection_update_one(&col, query, replacement)
                .await
        }
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;

    Ok(serde_json::to_string(&ret)?)
}

#[tauri::command]
pub async fn redis_list_lpush(
    name: String,
    key: String,
    value: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_lpush(key.as_str(), value.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn redis_list_rpush(
    name: String,
    key: String,
    value: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_rpush(key.as_str(), value.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn redis_list_remove(
    name: String,
    key: String,
    value: String,
    count: isize,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_remove(key.as_str(), count, value.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn redis_list_set(
    name: String,
    key: String,
    value: String,
    index: isize,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_set(key.as_str(), index, value.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn redis_set_replace(
    name: String,
    key: String,
    old: String,
    new: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let old = hex::decode(old.as_bytes()).map_err(ErrorFrontend::from)?;
    let new = hex::decode(new.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.set_replace(key.as_str(), old.as_slice(), new.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn redis_set_del(
    name: String,
    key: String,
    member: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(ErrorFrontend::from)?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.set_del(key.as_str(), member.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn redis_set_add(
    name: String,
    key: String,
    member: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(ErrorFrontend::from)?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.set_add(key.as_str(), member.as_slice())
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn redis_hash_replace(
    name: String,
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
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.hash_replace(key.as_str(), old_member.as_slice(), &new_member, &new_value)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn redis_hash_del(
    name: String,
    key: String,
    member: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.hash_del(key.as_str(), &member)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn redis_hash_add(
    name: String,
    key: String,
    member: String,
    value: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(ErrorFrontend::from)?;
    let value = hex::decode(value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.hash_add(key.as_str(), &member, &value)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn redis_zset_replace(
    name: String,
    key: String,
    old_member: String,
    new_member: String,
    new_value: f64,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let old_member = hex::decode(old_member.as_bytes()).map_err(ErrorFrontend::from)?;
    let new_member = hex::decode(new_member.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.zset_replace(key.as_str(), &old_member, &new_member, new_value)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn redis_zset_del(
    name: String,
    key: String,
    member: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.zset_del(key.as_str(), &member)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}
#[tauri::command]
pub async fn redis_zset_add(
    name: String,
    key: String,
    member: String,
    value: f64,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.zset_add(key.as_str(), &member, value)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn list_procedure(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<NamedStaff>, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let sx = schema.list_procedure().await.map_err(ErrorFrontend::from)?;
    Ok(sx)
}

#[tauri::command]
pub async fn show_procedure(
    name: String,
    procedure: String,
    arg: Option<UdfOrProcedureExtraParameter>,
    app: tauri::State<'_, DBPal>,
) -> Result<ProcedureDetail, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .show_procedure(&procedure, arg)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn drop_procedure(
    name: String,
    procedure: String,
    arg: Option<UdfOrProcedureExtraParameter>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .drop_procedure(&procedure, arg)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn list_udf(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<NamedStaff>, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let ret = schema.list_udf().await.map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn list_sequence(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<SequenceDetail>, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let ret = schema.list_sequence().await.map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn drop_sequence(
    name: String,
    app: tauri::State<'_, DBPal>,
    seq: String,
    cascade: bool,
) -> Result<String, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .drop_sequence(&seq, cascade)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn rename_sequence(
    name: String,
    app: tauri::State<'_, DBPal>,
    old: String,
    new: String,
) -> Result<String, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .rename_sequence(&old, &new)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn show_udf(
    name: String,
    udf: String,
    arg: Option<UdfOrProcedureExtraParameter>,
    app: tauri::State<'_, DBPal>,
) -> Result<DefAndMetas, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .show_udf(&udf, arg)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn drop_udf(
    name: String,
    udf: String,
    arg: Option<UdfOrProcedureExtraParameter>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .drop_udf(&udf, arg)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn redis_string_set(
    name: String,
    key: String,
    value: String,
    app: tauri::State<'_, DBPal>,
) -> Result<(), ErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(ErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.string_set(key.as_str(), &value)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    Ok(())
}

#[tauri::command]
pub async fn put_value_in_sql(
    name: String,
    typ: String,
    value: Option<String>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    // due to this error mysql sometime confuse about the real type.
    // special case
    // https://github.com/launchbadge/sqlx/issues/3390
    {
        let x = schema.backend_kind();
        if x.mysql_like() {
            return Ok(match value {
                Some(x) => {
                    if x.starts_with("0x") {
                        x
                    } else {
                        format!("'{}'", escape_sql_str(&x))
                    }
                }
                None => "null".to_string(),
            });
        }
    }
    schema
        .put_value_in_sql(&typ, value.as_deref())
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn cassandra_list_aggregate(
    name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<Vec<NamedStaff>, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let ret = match schema.as_any().downcast_ref::<CassandraDB>() {
        Some(x) => x.list_aggregate().await.map_err(ErrorFrontend::from)?,
        None => {
            return Err(ErrorFrontend::from(format!(
                "'{}' is not a cassandra backend",
                name
            )));
        }
    };
    Ok(ret)
}

#[tauri::command]
pub async fn cassandra_show_aggregate(
    name: String,
    agg_name: String,
    parameters: Vec<String>,
    app: tauri::State<'_, DBPal>,
) -> Result<DefAndMetas, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
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
                name
            )));
        }
    };
    Ok(ret)
}

#[tauri::command]
pub async fn cassandra_drop_aggregate(
    name: String,
    agg_name: String,
    parameters: Vec<String>,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    match schema.as_any().downcast_ref::<CassandraDB>() {
        Some(x) => x
            .drop_aggregate(&agg_name, parameters)
            .await
            .map_err(ErrorFrontend::from),
        None => Err(ErrorFrontend::from(format!(
            "'{}' is not a cassandra",
            name
        ))),
    }
}

#[tauri::command]
pub async fn drop_created_type(
    name: String,
    type_name: String,
    app: tauri::State<'_, DBPal>,
) -> Result<String, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    schema
        .drop_created_type(&type_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn command_panic() -> Result<(), ErrorFrontend> {
    panic!("this is a panic for testing.")
}

#[tauri::command]
pub fn valid_json(json_str: String) -> Result<Option<ErrAndLoc>, ErrorFrontend> {
    use serde_json::Value;
    match serde_json::from_str::<Value>(&json_str) {
        Ok(_) => Ok(None),
        Err(e) => Ok(Some(ErrAndLoc {
            msg: format!("{}", e),
            line: Some(e.line() as u32),
            column: Some(e.column() as u32),
        })),
    }
}

#[derive(serde::Serialize)]
pub struct DocsAndCount {
    count: u64,
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
            count: value.count,
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
            use rand::Rng;
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
            use rand::Rng;
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
            use rand::Rng;
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

#[derive(Clone, Debug, serde::Serialize, typescript_type_def::TypeDef)]
pub struct MuiDataGridData {
    fields: Vec<Field>,
    data: Vec<HashMap<String, String>>,
    sql: Option<String>,
    total: usize,
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
            fn get_index(&mut self, name: &String) -> Option<usize> {
                match self.indexes.get_mut(name) {
                    Some(x) => {
                        *x += 1;
                        Some(*x)
                    }
                    None => {
                        self.indexes.insert(name.clone(), 1);
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
                    if let Some(value) = value {
                        values.insert(f.field.clone(), value);
                    }
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
        "name": "Rust"
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
