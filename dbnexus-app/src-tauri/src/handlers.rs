use std::collections::{HashMap, HashSet};
use std::time::Duration;

use bson::{Document, doc};
use dbnexus_db::cassandra::CassandraDB;
use dbnexus_db::clickhouse::ClickHouseDB;
use dbnexus_db::cockroach::CockRoachDB;
use dbnexus_db::db::{
    AutoCompletionQueryUuid, ColumnType, CreateDatabaseExtra, CreateSchemaExtra,
    CreateTableAndIndexSql, CreatedType, DefAndMetas, ExportKind, Field, FormatSqlOpts, Index,
    IndexKind, ListTable, NamedStaff, ProcedureDetail, QueryResp, SequenceDetail, ShowExtraArgs,
    SqlExecuteRet, TriggerDetail, UdfOrProcedureExtraParameter, escape_sql_str,
};
use dbnexus_db::firebird::FireBirdDB;
use dbnexus_db::language_server::CompletionItem;
use dbnexus_db::libsql::LibSqlDB;
use dbnexus_db::meta_value::MetaValue;
use dbnexus_db::mongo::MongoDB;
use dbnexus_db::mssql::MsSqlServerDB;
use dbnexus_db::oracle::OracleDB;
use dbnexus_db::pg::PostGresDB;
use dbnexus_db::query_log::QueryLogEvent;
use dbnexus_db::redis::{RedisDB, RedisListTable};
use dbnexus_db::sqlite::SqliteDB;
use tauri::{AppHandle, Emitter};

use crate::app::{self, App, DBConnectionConfig, Open};
pub const PASSWORD_KEY: &str = "site.dbpal.dev";

#[tauri::command]
pub fn has_entered_password(
    app: tauri::State<'_, App>, name: String,
) -> Result<bool, ErrorFrontend> {
    let has = app.saved_password.get(&name).is_some();
    Ok(has)
}

#[tauri::command]
pub async fn save_entered_password(
    app: tauri::State<'_, App>, name: String, pass: String,
) -> Result<(), ErrorFrontend> {
    let mut conn = unsafe {
        (App::get_config_ref())
            .as_ref()
            .map(|x| x(&name))
            .unwrap()
            .map_err(ErrorFrontend::from)?
    };
    conn.set_password(pass.clone());
    let o = conn.open().await.map_err(ErrorFrontend::from)?;
    match o {
        Open::Catalog(x) => x.check_conn().await,
        Open::DataBase(x) => x.check_conn().await,
        Open::Schema(x) => x.check_conn().await,
    }
    .map_err(ErrorFrontend::from)?;
    app.saved_password.insert(name, pass);
    Ok(())
}
#[tauri::command]
pub fn set_auto_completion_query_id(query_id: uuid::Uuid) {
    AutoCompletionQueryUuid::set(query_id);
}

#[tauri::command]
pub async fn delete_password(name: String) -> Result<(), ErrorFrontend> {
    let entry = keyring::Entry::new(PASSWORD_KEY, &name)?;
    entry.delete_credential()?;
    Ok(())
}
#[tauri::command]
pub async fn save_password(name: String, pass: String) -> Result<(), ErrorFrontend> {
    let entry = keyring::Entry::new(PASSWORD_KEY, &name)?;
    entry.set_password(&pass)?;
    Ok(())
}
#[tauri::command]
pub async fn page_init_callback(app: tauri::State<'_, App>) -> Result<(), ()> {
    log::info!("page init call clear all open connection.");
    app.opens.lock().await.clear();
    Ok(())
}

#[tauri::command]
pub async fn drop_database(
    name: String, database: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("drop db:{}", name);
    let x = app
        .get_opened(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    // first close connections because we want delete it

    let key = format!("{}/{}", name, database);
    app.close_conn_has_prefix(&key).await;
    match x {
        Open::Catalog(x) => {
            app.opens.lock().await.remove(&key);

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
    name: String, schema: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    // first close connections because we want delete it
    let key = format!("{}/{}", name, schema);
    app.close_conn_has_prefix(&key).await;
    let x = app
        .get_opened(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match x {
        Open::DataBase(x) => {
            app.opens.lock().await.remove(&key);
            x.drop_schema(&schema).await.map_err(ErrorFrontend::from)?;
            Ok(())
        }
        _ => Err(ErrorFrontend::from(format!(
            "'{}' is not a db database",
            name
        ))),
    }
}

#[tauri::command]
pub async fn create_database(
    name: String, database: String, extra: Option<CreateDatabaseExtra>, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let x = app
        .get_opened(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match x {
        Open::Catalog(x) => {
            x.create_database(&database, extra)
                .await
                .map_err(ErrorFrontend::from)?;

            let key = format!("{}/{}", name, database);

            let db = x.open_db(&database).await.map_err(ErrorFrontend::from)?;

            app.enter_open_child(&key, &Open::from(db.clone()))
                .await
                .map_err(ErrorFrontend::from)?;

            app.insert_open(key, db.into()).await;

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
    name: String, schema: String, extra: Option<CreateSchemaExtra>, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let x = app
        .get_opened(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match x {
        Open::DataBase(x) => {
            x.create_schema(&schema, extra)
                .await
                .map_err(ErrorFrontend::from)?;
            let key = format!("{}/{}", name, schema);
            let schema = x.open_schema(&schema).await.map_err(ErrorFrontend::from)?;

            app.insert_open(key, schema.into()).await;
            Ok(())
        }
        _ => Err(ErrorFrontend::from(format!(
            "'{}' is not a db database",
            name
        ))),
    }
}

#[tauri::command]
pub async fn open_conn(name: String, app: tauri::State<'_, App>) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("open db:{}", name);
    let secs = 10;
    let name_clone = name.clone();

    tokio::time::timeout(Duration::from_secs(secs), async move {
        app.open(name.as_str())
            .await
            .map_err(ErrorFrontend::from)
            .map(|_| ())
    })
    .await
    .map_err(|_| {
        ErrorFrontend::from(format!(
            "'{name_clone}' open connection not finished in {}s",
            secs
        ))
    })?
}

#[tauri::command]
pub async fn close_conn(name: String, app: tauri::State<'_, App>) -> Result<(), ()> {
    random_sleep().await;
    app.close_conn_has_prefix(name.as_str()).await;
    Ok(())
}

#[tauri::command]
pub async fn list_table(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<ListTable>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("show table_name db:{}", name);
    let db = app.get_schema(&name).await?;
    db.list_table().await.map_err(ErrorFrontend::from)
}
#[tauri::command]
pub async fn mongo_list_time_serial_collection(
    name: String, app: tauri::State<'_, App>,
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
    name: String, filter: Option<String>, max: usize, app: tauri::State<'_, App>,
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
pub async fn redis_zset_show_table(
    name: String, key: String, min: Option<f64>, max: Option<f64>, where_str: Option<&str>,
    app: tauri::State<'_, App>,
) -> Result<MuiDataGridData, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app.get_schema(&name).await.map_err(ErrorFrontend::from)?;

    let r = match schema.as_any().downcast_ref::<RedisDB>() {
        Some(r) => r,
        None => return Err(ErrorFrontend::from(format!("'{}' not redis", name))),
    };
    let x = r
        .zset_show_table(&key, where_str, min, max)
        .await
        .map_err(ErrorFrontend::from)?;

    Ok(MuiDataGridData::from(x))
}

#[tauri::command]
pub async fn show_table_columns(
    name: String, table_name: String, app: tauri::State<'_, App>,
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
    name: String, table_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, extra: Option<ShowExtraArgs>, app: tauri::State<'_, App>,
) -> Result<MuiDataGridData, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    let x = db
        .show_table(&table_name, where_str, order_by, page, page_size, extra)
        .await
        .map_err(ErrorFrontend::from)?;

    Ok(MuiDataGridData::from(x))
}

#[tauri::command]
pub async fn show_indexes(
    name: String, table_name: String, app: tauri::State<'_, App>,
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
    name: String, table_name: String, index_name: String, fields: Vec<String>, index_type: String,
    app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let index_kind = IndexKind::try_from(index_type.as_str()).map_err(ErrorFrontend::from)?;

    schema
        .create_index(&table_name, &index_name, index_kind, &fields[..])
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn mongo_create_index(
    name: String, table_name: String, index_name: Option<String>, keys: Vec<u8>, unique: bool,
    app: tauri::State<'_, App>,
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
            .map_err(ErrorFrontend::from)?,
        None => {
            return Err(ErrorFrontend::from(format!("'{}' not a mongo", name)));
        }
    };
    Ok(())
}
#[tauri::command]
pub async fn rename_index(
    name: String, table_name: String, old_name: String, new_name: String,
    app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
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
    name: String, table_name: String, index: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
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
    name: String, table_name: String, values: HashMap<String, Option<String>>,
    app: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.table_insert(&table_name, values)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn table_update(
    name: String, table_name: String, values: HashMap<String, Option<String>>,
    wheres: HashMap<String, Option<String>>, app: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.table_update(&table_name, values, wheres)
        .await
        .map_err(ErrorFrontend::from)
}
#[tauri::command]
pub async fn truncate_table(
    name: String, table_name: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.truncate_table(&table_name)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn drop_table_column(
    name: String, table_name: String, column_name: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .drop_table_column(&table_name, &column_name)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(())
}

#[tauri::command]
pub async fn modify_table_column(
    name: String, table_name: String, old_name: String, new_name: String, typ: Option<ColumnType>,
    app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .modify_table_column(&table_name, &old_name, &new_name, typ)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(())
}

#[tauri::command]
pub async fn add_table_column(
    name: String, table_name: String, col_name: String, typ: ColumnType, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .add_table_column(&table_name, &col_name, typ)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(())
}

#[tauri::command]
pub async fn list_data_types(
    name: String, app: tauri::State<'_, App>,
) -> Result<&'static [&'static str], ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let ret = schema.list_data_types().map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn table_delete(
    name: String, table_name: String, wheres: HashMap<String, Option<String>>,
    app: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.table_delete(&table_name, wheres)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn list_view(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<NamedStaff>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.list_view().await.map_err(ErrorFrontend::from)
}
#[tauri::command]
pub async fn server_metas(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<MetaValue>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let o = app.get_opened(&name).await.map_err(ErrorFrontend::from)?;
    match o {
        Open::Catalog(x) => x.show_metas().await,
        Open::DataBase(x) => x.show_metas().await,
        Open::Schema(x) => x.show_metas().await,
    }
    .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn list_materialized_view(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<ListTable>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let s = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    s.list_materialized_view()
        .await
        .map_err(ErrorFrontend::from)
}
#[tauri::command]
pub async fn drop_view(
    name: String, view_name: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.drop_view(&view_name).await.map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn drop_materialized_view(
    name: String, view_name: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
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
    name: String, table_name: String, cascade: bool, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.drop_table(&table_name, cascade)
        .await
        .map_err(ErrorFrontend::from)
}
#[tauri::command]
pub async fn show_view(
    name: String, view_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, extra: Option<ShowExtraArgs>, app: tauri::State<'_, App>,
) -> Result<MuiDataGridData, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.get_schema(&name).await?;
    db.show_view(&view_name, where_str, order_by, page, page_size, extra)
        .await
        .map_err(ErrorFrontend::from)
        .map(MuiDataGridData::from)
}
#[tauri::command]
pub async fn show_view_columns(
    name: String, view_name: String, app: tauri::State<'_, App>,
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
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<CreatedType>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app.get_schema(&name).await.map_err(ErrorFrontend::from)?;

    schema
        .list_created_types()
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn export_query_result_to_file(
    name: String, sql: String, kind: ExportKind, file: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let opend = app
        .get_opened(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    match opend {
        Open::Catalog(x) => {
            x.export_query_result_to_file(&sql, kind, &file)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        Open::DataBase(x) => {
            x.export_query_result_to_file(&sql, kind, &file)
                .await
                .map_err(ErrorFrontend::from)?;
        }
        Open::Schema(x) => {
            x.export_query_result_to_file(&sql, kind, &file)
                .await
                .map_err(ErrorFrontend::from)?;
        }
    };

    Ok(())
}
#[tauri::command]
pub async fn is_dangerous_sql(
    name: String, sql: String, app: tauri::State<'_, App>,
) -> Result<bool, ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let db = app
        .get_opened(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let result = match db {
        Open::DataBase(x) => x.is_dangerous_sql(&sql),
        Open::Schema(x) => x.is_dangerous_sql(&sql),
        Open::Catalog(x) => x.is_dangerous_sql(&sql),
    }
    .map_err(ErrorFrontend::from)?;

    Ok(result)
}
#[tauri::command]
pub async fn query_sql(
    name: String, query_id: uuid::Uuid, sql: String, app: tauri::State<'_, App>,
) -> Result<Vec<Result<MuiDataGridData, String>>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let db = app.open(name.as_str()).await.map_err(ErrorFrontend::from)?;

    let result = match db {
        Open::DataBase(x) => x.query_sql(query_id, &sql).await,
        Open::Schema(x) => x.query_sql(query_id, &sql).await,
        Open::Catalog(x) => x.query_sql(query_id, &sql).await,
    };

    result.map_err(ErrorFrontend::from).map(|x| {
        x.into_iter()
            .map(|x| match x {
                Err(e) => Err(e),
                Ok(x) => Ok(MuiDataGridData::from(x)),
            })
            .collect::<Vec<_>>()
    })
}

#[tauri::command]
pub async fn execute_sql(
    name: String, query_id: uuid::Uuid, sql: String, app: tauri::State<'_, App>,
) -> Result<Result<MuiDataGridData, String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let db = app.open(name.as_str()).await.map_err(ErrorFrontend::from)?;

    let result = match db {
        Open::DataBase(x) => x.execute_sql(query_id, &sql).await,
        Open::Schema(x) => x.execute_sql(query_id, &sql).await,
        Open::Catalog(x) => x.execute_sql(query_id, &sql).await,
    }
    .map_err(ErrorFrontend::from)?;
    Ok(result.map(MuiDataGridData::from))
}

#[tauri::command]
pub async fn query_logs(
    name: String, query_id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<Vec<QueryLogEvent>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await.map_err(ErrorFrontend::from)?;

    match db {
        Open::DataBase(x) => x.get_query_log(query_id),
        Open::Schema(x) => x.get_query_log(query_id),
        Open::Catalog(x) => x.get_query_log(query_id),
    }
    .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn query_sql_open_connection(
    name: String, app: tauri::State<'_, App>,
) -> Result<uuid::Uuid, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await.map_err(|e| e.to_string())?;
    let result = match db {
        Open::DataBase(x) => x.open_connection().await,
        Open::Schema(x) => x.open_connection().await,
        Open::Catalog(x) => x.open_connection().await,
    };
    result.map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn query_sql_re_connection(
    name: String, query_id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await.map_err(|e| e.to_string())?;
    let result = match db {
        Open::DataBase(x) => x.re_connect(query_id).await,
        Open::Schema(x) => x.re_connect(query_id).await,
        Open::Catalog(x) => x.re_connect(query_id).await,
    };
    result.map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn query_sql_close_connection(
    name: String, query_id: uuid::Uuid, app: tauri::State<'_, App>, handle: AppHandle,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let db = app.open(name.as_str()).await.map_err(|e| e.to_string())?;
    let commit = match &db {
        Open::Catalog(x) => x.get_auto_commit(query_id).await,
        Open::DataBase(x) => x.get_auto_commit(query_id).await,
        Open::Schema(x) => x.get_auto_commit(query_id).await,
    }
    .ok()
    .unwrap_or(true);
    let tx_count = match &db {
        Open::Catalog(x) => x.tx_execute_counts(query_id),
        Open::DataBase(x) => x.tx_execute_counts(query_id),
        Open::Schema(x) => x.tx_execute_counts(query_id),
    }
    .ok()
    .unwrap_or(0);

    if !commit && tx_count > 0 {
        let _ = handle.emit("close_connection_has_uncommit_tx", "");
    }

    let result = match db {
        Open::DataBase(x) => x.close_connection(query_id).await,
        Open::Schema(x) => x.close_connection(query_id).await,
        Open::Catalog(x) => x.close_connection(query_id).await,
    };
    result.map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn export_table_or_view(
    name: String, table_or_view: String, filepath: String, kind: ExportKind,
    app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    db.export_table_to_file(table_or_view.as_str(), filepath.as_str(), kind)
        .await
        .map_err(ErrorFrontend::from)
}
#[tauri::command]
pub async fn format_sql(
    name: String, sql: String, opts: Option<FormatSqlOpts>, app: tauri::State<'_, App>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_opened(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db {
        Open::DataBase(x) => x.format_sql(sql.as_str(), opts),
        Open::Schema(x) => x.format_sql(sql.as_str(), opts),
        Open::Catalog(x) => x.format_sql(sql.as_str(), opts),
    };
    ret.map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn check_conn(
    name: String, app: tauri::State<'_, App>,
) -> Result<
    String, // human readable time
    ErrorFrontend,
> {
    random_sleep().await;
    random_error()?;
    let t = 5;
    tokio::time::timeout(Duration::from_secs(t), async move {
        let db = app.open(&name).await?;
        match db {
            app::Open::DataBase(x) => x.check_conn().await,
            app::Open::Schema(x) => x.check_conn().await,
            app::Open::Catalog(x) => x.check_conn().await,
        }
        .map_err(ErrorFrontend::from)
        .map(|d| {
            //
            humantime::format_duration(Duration::from_millis(
                d.as_millis() as u64, // 最小单位毫秒
            ))
            .to_string()
        })
    })
    .await
    .map_err(|_| ErrorFrontend::from(format!("check connection status not finished in {}s", t)))?
}

#[tauri::command]
pub async fn test_connection(
    config: DBConnectionConfig,
) -> Result<
    u64, // milliseconds
    ErrorFrontend,
> {
    random_error()?;

    let t = 5;
    tokio::time::timeout(Duration::from_secs(5), async {
        let db = config.open().await.map_err(|e| e.to_string())?;
        match db {
            app::Open::DataBase(x) => x.check_conn().await,
            app::Open::Schema(x) => x.check_conn().await,
            app::Open::Catalog(x) => x.check_conn().await,
        }
        .map_err(ErrorFrontend::from)
        .map(|d| d.as_millis() as u64)
    })
    .await
    .map_err(|_| ErrorFrontend::from(format!("test connection not finished in {}s", t)))?
}

#[tauri::command]
pub async fn config_list_database_or_schema(
    conn: DBConnectionConfig,
) -> Result<Vec<String>, ErrorFrontend> {
    let x = conn.open().await?;
    match x {
        Open::Catalog(catalog_trait) => catalog_trait
            .list_database()
            .await
            .map_err(ErrorFrontend::from),
        Open::DataBase(dbtrait) => dbtrait.list_schema().await.map_err(ErrorFrontend::from),
        Open::Schema(_) => Ok(vec![]),
    }
}

#[tauri::command]
pub async fn conn_opend(name: String, app: tauri::State<'_, App>) -> Result<bool, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let opend = app.get_opened(name.as_str()).await.is_ok();
    Ok(opend)
}

#[tauri::command]
pub async fn list_database(
    name: String, app: tauri::State<'_, App>,
) -> Result<
    Vec<String>, // milliseconds
    ErrorFrontend,
> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await?;
    app.enter_open_child(name.as_str(), &db).await?;
    match db {
        app::Open::Catalog(x) => x.list_database().await,
        _ => {
            return Err(ErrorFrontend::from(format!(
                "not catalog trait  of  {}",
                name
            )));
        }
    }
    .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn list_schema(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<String>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await?;
    app.enter_open_child(name.as_str(), &db).await?;
    match db {
        app::Open::DataBase(x) => x.list_schema().await,
        app::Open::Catalog(_) => {
            return Err(ErrorFrontend::from(format!(
                "not database trait but a catalog of {}",
                name
            )));
        }
        app::Open::Schema(_) => {
            return Err(ErrorFrontend::from(format!(
                "not database trait but a schema of {}",
                name
            )));
        }
    }
    .map_err(ErrorFrontend::from)
}
#[tauri::command]
pub async fn auto_completion(
    name: String, sql: String, line: u32, col: u32, app: tauri::State<'_, App>,
) -> Result<Vec<CompletionItem>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let use_lexer = true;
    let db = app.get_opened(name.as_str()).await?;
    let ret = match db {
        Open::Catalog(x) => x.auto_completion(&sql, line, col, use_lexer).await,
        Open::DataBase(x) => x.auto_completion(&sql, line, col, use_lexer).await,
        Open::Schema(x) => x.auto_completion(&sql, line, col, use_lexer).await,
    }
    .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn query_set_autocommit(
    name: String, query_id: uuid::Uuid, commit: bool, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let open = app
        .get_opened(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match open {
        Open::Catalog(x) => x.set_auto_commit(query_id, commit).await,
        Open::DataBase(x) => x.set_auto_commit(query_id, commit).await,
        Open::Schema(x) => x.set_auto_commit(query_id, commit).await,
    }
    .map_err(ErrorFrontend::from)?;
    Ok(())
}

#[tauri::command]
pub async fn query_get_commit(
    name: String, query_id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<bool, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let open = app
        .get_opened(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match open {
        Open::Catalog(x) => x.get_auto_commit(query_id).await,
        Open::DataBase(x) => x.get_auto_commit(query_id).await,
        Open::Schema(x) => x.get_auto_commit(query_id).await,
    }
    .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn query_commit(
    name: String, query_id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let open = app
        .get_opened(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match open {
        Open::Catalog(x) => x.commit(query_id).await,
        Open::DataBase(x) => x.commit(query_id).await,
        Open::Schema(x) => x.commit(query_id).await,
    }
    .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn query_tx_execute_counts(
    name: String, query_id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<usize, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let open = app
        .get_opened(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let size = match open {
        Open::Catalog(x) => x.tx_execute_counts(query_id),
        Open::DataBase(x) => x.tx_execute_counts(query_id),
        Open::Schema(x) => x.tx_execute_counts(query_id),
    }
    .map_err(ErrorFrontend::from)?;
    Ok(size)
}

#[tauri::command]
pub async fn query_rollback(
    name: String, query_id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_opened(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db {
        Open::Catalog(x) => x.rollback(query_id).await,
        Open::DataBase(x) => x.rollback(query_id).await,
        Open::Schema(x) => x.rollback(query_id).await,
    }
    .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn drop_trigger(
    name: String, table_name: String, trigger: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
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
    name: String, table_name: String, trigger: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
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
    name: String, table_name: String, trigger: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
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
    name: String, table_name: String, app: tauri::State<'_, App>,
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
    name: String, old: String, new: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
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
pub async fn create_table_and_index_sql(
    name: String, table_name: String, app: tauri::State<'_, App>,
) -> Result<CreateTableAndIndexSql, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let ret = db
        .create_table_and_index_sql(&table_name)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn create_view_sql(
    name: String, view_name: String, materialized: bool, app: tauri::State<'_, App>,
) -> Result<String, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let ret = db
        .create_view_sql(&view_name, materialized)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn duplicate_table(
    name: String, from: String, to: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    db.duplicate_table(&from, &to)
        .await
        .map_err(ErrorFrontend::from)
}

#[tauri::command]
pub async fn mongo_insert(
    name: String, table_name: String, doc: Vec<u8>, app: tauri::State<'_, App>,
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
    name: String, table_name: String, doc: Vec<u8>, app: tauri::State<'_, App>,
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
    name: String, table_name: String, ids: String, app: tauri::State<'_, App>,
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
    name: String, table_name: String, filter: Vec<u8>, page: u64, page_size: u64, sort: Vec<u8>,
    app: tauri::State<'_, App>,
) -> Result<MuiDataGridData, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let filter = parse_bson_doc(filter.as_slice())?;
    let sort = parse_bson_doc(sort.as_slice())?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.show_collection_table_list(table_name.as_str(), filter, sort, page, page_size)
                .await
        }
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)
    .map(MuiDataGridData::from)
}

#[tauri::command]
pub async fn mongo_show_collection_docs(
    name: String, table_name: String, filter: Vec<u8>, sort: Vec<u8>, app: tauri::State<'_, App>,
    page: u64, page_size: u64,
) -> Result<DocsAndCount, ErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let filter = parse_bson_doc(filter.as_slice())?;
    let sort = parse_bson_doc(sort.as_slice())?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let docs = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.show_collection_docs(table_name.as_str(), filter, sort, page, page_size)
                .await
        }
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(ErrorFrontend::from)?;
    DocsAndCount::try_from(docs).map_err(|e| e.into())
}
#[tauri::command]
pub async fn mongo_get_one_document_by_id(
    name: String, table_name: String, id: String, app: tauri::State<'_, App>,
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
    name: String, app: tauri::State<'_, App>,
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
    name: String, app: tauri::State<'_, App>,
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
    name: String, command: Vec<u8>, app: tauri::State<'_, App>,
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
    name: String, col: String, app: tauri::State<'_, App>,
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
    name: String, col: String, index: Vec<u8>, app: tauri::State<'_, App>,
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
    name: String, col: String, index: Vec<u8>, index_name: Option<String>,
    app: tauri::State<'_, App>,
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
    name: String, col: String, query: Vec<u8>, app: tauri::State<'_, App>,
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
    name: String, col: String, query: Vec<u8>, app: tauri::State<'_, App>,
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
    name: String, col: String, query: Vec<u8>, field: String, app: tauri::State<'_, App>,
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
    name: String, col: String, app: tauri::State<'_, App>,
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
    name: String, col: String, index: String, app: tauri::State<'_, App>,
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
    name: String, col: String, app: tauri::State<'_, App>,
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
    name: String, col: String, index: String, app: tauri::State<'_, App>,
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
    name: String, col: String, query: Vec<u8>, projection: Option<Vec<u8>>, sort: Option<Vec<u8>>,
    min: Option<Vec<u8>>, max: Option<Vec<u8>>, limit: Option<i64>, skip: Option<u64>,
    app: tauri::State<'_, App>,
) -> Result<Vec<Vec<u8>>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let max = if let Some(max) = max.as_ref() {
        Some(parse_bson_doc(max)?)
    } else {
        None
    };
    let min = if let Some(min) = min.as_ref() {
        Some(parse_bson_doc(min)?)
    } else {
        None
    };

    let sort = if let Some(sort) = sort.as_ref() {
        Some(parse_bson_doc(sort)?)
    } else {
        None
    };
    let projection = if let Some(projection) = projection.as_ref() {
        Some(parse_bson_doc(projection)?)
    } else {
        None
    };
    let filter = parse_bson_doc(&query)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let docs = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.query_collection_find(&col, projection, filter, sort, max, min, limit, skip)
                .await
        }
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
    name: String, col: String, query: Vec<u8>, projection: Option<Vec<u8>>, sort: Option<Vec<u8>>,
    min: Option<Vec<u8>>, max: Option<Vec<u8>>, skip: Option<u64>, app: tauri::State<'_, App>,
) -> Result<Option<Vec<u8>>, ErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let max = if let Some(max) = max.as_ref() {
        Some(parse_bson_doc(max)?)
    } else {
        None
    };
    let min = if let Some(min) = min.as_ref() {
        Some(parse_bson_doc(min)?)
    } else {
        None
    };

    let sort = if let Some(sort) = sort.as_ref() {
        Some(parse_bson_doc(sort)?)
    } else {
        None
    };
    let projection = if let Some(projection) = projection.as_ref() {
        Some(parse_bson_doc(projection)?)
    } else {
        None
    };
    let filter = parse_bson_doc(&query)?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let ret = match db.as_any().downcast_ref::<MongoDB>() {
        Some(x) => {
            x.query_collection_find_one(&col, projection, filter, sort, max, min, skip)
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
pub async fn mongo_query_collection_find_one_and_delete(
    name: String, col: String, query: Vec<u8>, app: tauri::State<'_, App>,
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
    name: String, col: String, query: Vec<u8>, replacement: Vec<u8>, app: tauri::State<'_, App>,
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
    name: String, col: String, data: Vec<Vec<u8>>, app: tauri::State<'_, App>,
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
    name: String, col: String, doc: Vec<u8>, app: tauri::State<'_, App>,
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
    name: String, col: String, app: tauri::State<'_, App>,
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
    name: String, col: String, app: tauri::State<'_, App>,
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
    name: String, col: String, app: tauri::State<'_, App>,
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
    name: String, col: String, query: Vec<u8>, replacement: Vec<u8>, app: tauri::State<'_, App>,
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
    name: String, col: String, query: Vec<u8>, replacement: Vec<u8>, app: tauri::State<'_, App>,
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
    name: String, col: String, query: Vec<u8>, replacement: Vec<u8>, app: tauri::State<'_, App>,
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
    name: String, key: String, value: String, app: tauri::State<'_, App>,
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
    name: String, key: String, value: String, app: tauri::State<'_, App>,
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
    name: String, key: String, value: String, count: isize, app: tauri::State<'_, App>,
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
    name: String, key: String, value: String, index: isize, app: tauri::State<'_, App>,
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
    name: String, key: String, old: String, new: String, app: tauri::State<'_, App>,
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
    name: String, key: String, member: String, app: tauri::State<'_, App>,
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
    name: String, key: String, member: String, app: tauri::State<'_, App>,
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
    name: String, key: String, old_member: String, new_member: String, new_value: String,
    app: tauri::State<'_, App>,
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
    name: String, key: String, member: String, app: tauri::State<'_, App>,
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
    name: String, key: String, member: String, value: String, app: tauri::State<'_, App>,
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
    name: String, key: String, old_member: String, new_member: String, new_value: f64,
    app: tauri::State<'_, App>,
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
    name: String, key: String, member: String, app: tauri::State<'_, App>,
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
    name: String, key: String, member: String, value: f64, app: tauri::State<'_, App>,
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
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<NamedStaff>, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let sx = schema.list_procedure().await.map_err(ErrorFrontend::from)?;
    check_named_staff_unique(&sx[..])?;
    Ok(sx)
}

#[tauri::command]
pub async fn show_procedure(
    name: String, procedure: String, arg: Option<UdfOrProcedureExtraParameter>,
    app: tauri::State<'_, App>,
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
    name: String, procedure: String, arg: Option<UdfOrProcedureExtraParameter>,
    app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
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
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<NamedStaff>, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let sx = schema.list_udf().await.map_err(ErrorFrontend::from)?;
    // TODO why cockroach database  build in udf  `pg_get_function_arguments` return
    // null
    check_named_staff_unique(&sx[..])?;
    Ok(sx)
}

#[tauri::command]
pub async fn list_sequences(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<SequenceDetail>, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let sx = schema.list_sequences().await.map_err(ErrorFrontend::from)?;

    Ok(sx)
}

#[tauri::command]
pub async fn drop_sequence(
    name: String, app: tauri::State<'_, App>, seq: String, cascade: bool,
) -> Result<(), ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .drop_sequence(&seq, cascade)
        .await
        .map_err(ErrorFrontend::from)?;
    Ok(())
}

#[tauri::command]
pub async fn rename_sequnce(
    name: String, app: tauri::State<'_, App>, old: String, new: String,
) -> Result<(), ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    schema
        .rename_sequnce(&old, &new)
        .await
        .map_err(ErrorFrontend::from)?;

    Ok(())
}

fn check_named_staff_unique(named: &[NamedStaff]) -> Result<(), ErrorFrontend> {
    #[derive(Clone, Hash, PartialEq, Eq)]
    struct Key<'a> {
        name: &'a String,
        parameters: &'a Option<String>,
        cassandra_parameters: &'a Option<Vec<String>>,
    }

    impl<'a> std::fmt::Debug for Key<'a> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.name)?;
            if let Some(x) = self.parameters {
                write!(f, "({})", x)?;
            }
            if let Some(x) = self.cassandra_parameters {
                write!(f, "({:?})", x)?;
            }
            Ok(())
        }
    }

    impl<'a> From<&'a NamedStaff> for Key<'a> {
        fn from(value: &'a NamedStaff) -> Self {
            Key {
                name: &value.name,
                parameters: &value.parameters,
                cassandra_parameters: &value.cassandra_parameters,
            }
        }
    }
    let mut set = HashSet::new();
    for n in named.iter() {
        let k = Key::from(n);
        if set.contains(&k) {
            return Err(ErrorFrontend::from(format!(
                "fn named '{:?}' duplicated this will make fontend not working",
                k
            )));
        }
        set.insert(k);
    }
    Ok(())
}

#[tauri::command]
pub async fn show_udf(
    name: String, udf: String, arg: Option<UdfOrProcedureExtraParameter>,
    app: tauri::State<'_, App>,
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
    name: String, udf: String, arg: Option<UdfOrProcedureExtraParameter>,
    app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
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
    name: String, key: String, value: String, app: tauri::State<'_, App>,
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
    name: String, typ: String, value: Option<String>, app: tauri::State<'_, App>,
) -> Result<String, ErrorFrontend> {
    use dbnexus_db::db::SqlBuilder;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;
    let a = schema.as_any();
    #[inline]
    fn xxx<T: SqlBuilder + 'static>(
        a: &dyn std::any::Any, typ: &String, value: Option<&str>,
    ) -> Result<String, ErrorFrontend> {
        if let Some(x) = a.downcast_ref::<T>() {
            return x
                .put_value_in_sql(typ, None, value)
                .map_err(ErrorFrontend::from);
        }
        Err(ErrorFrontend::from("type not match"))
    }
    // due to this error mysql sometime confuse about the real type.
    // special case
    // https://github.com/launchbadge/sqlx/issues/3390
    {
        let x = schema.backend_name();

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
    let value = value.as_deref();
    if let Ok(e) = xxx::<SqliteDB>(a, &typ, value) {
        return Ok(e);
    }
    if let Ok(e) = xxx::<LibSqlDB>(a, &typ, value) {
        return Ok(e);
    }
    if let Ok(e) = xxx::<PostGresDB>(a, &typ, value) {
        return Ok(e);
    }
    if let Ok(e) = xxx::<CockRoachDB>(a, &typ, value) {
        return Ok(e);
    }
    if let Ok(e) = xxx::<OracleDB>(a, &typ, value) {
        return Ok(e);
    }
    if let Ok(e) = xxx::<ClickHouseDB>(a, &typ, value) {
        return Ok(e);
    }
    if let Ok(e) = xxx::<CassandraDB>(a, &typ, value) {
        return Ok(e);
    }
    if let Ok(e) = xxx::<FireBirdDB>(a, &typ, value) {
        return Ok(e);
    }
    if let Ok(e) = xxx::<MsSqlServerDB>(a, &typ, value) {
        return Ok(e);
    }
    #[cfg(feature = "duckdb")]
    {
        use dbnexus_db::duckdb::DuckDB;
        if let Ok(e) = xxx::<DuckDB>(a, &typ, value) {
            return Ok(e);
        }
    }
    Err(ErrorFrontend::from(
        "not valid schema to put value in sql.maybe a internal error",
    ))
}

#[tauri::command]
pub async fn cassandra_invalid_table_or_view_caching(
    name: String, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    let open = app.get_schema(&name).await.map_err(ErrorFrontend::from)?;
    match open.as_any().downcast_ref::<CassandraDB>() {
        Some(x) => {
            x.invalid_table_or_view_caching(None);
            Ok(())
        }
        None => Err(ErrorFrontend::from("this is not cassandra")),
    }
}

#[tauri::command]
pub async fn cassandra_list_aggregate(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<NamedStaff>, ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    let ret = match schema.as_any().downcast_ref::<CassandraDB>() {
        Some(x) => x.list_aggregate().await.map_err(ErrorFrontend::from)?,
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
pub async fn cassandra_show_aggregate(
    name: String, agg_name: String, parameters: Vec<String>, app: tauri::State<'_, App>,
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
    name: String, agg_name: String, parameters: Vec<String>, app: tauri::State<'_, App>,
) -> Result<(), ErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(ErrorFrontend::from)?;

    match schema.as_any().downcast_ref::<CassandraDB>() {
        Some(x) => x
            .drop_aggregate(&agg_name, parameters)
            .await
            .map_err(ErrorFrontend::from)?,
        None => {
            return Err(ErrorFrontend::from(format!(
                "'{}' is not a cassandra",
                name
            )));
        }
    };
    Ok(())
}

#[tauri::command]
pub async fn command_panic() -> Result<(), ErrorFrontend> {
    todo!("this is a panic.")
}

#[tauri::command]
pub fn valid_json(json_str: String) -> Result<(), String> {
    use serde_json::Value;
    match serde_json::from_str::<Value>(&json_str) {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("{}", e)),
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

fn parse_bson_doc(bs: &[u8]) -> Result<Document, String> {
    let doc = bson::Document::from_reader(std::io::Cursor::new(bs)).map_err(|e| e.to_string())?;
    Ok(doc)
}

#[derive(serde::Serialize)]
pub struct ErrorFrontend {
    // short message on why
    reason: String,
    // long message about why and backtrace
    detail: String,
}

impl From<tauri::Error> for ErrorFrontend {
    fn from(value: tauri::Error) -> Self {
        let x = Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
        };
        log::error!("msg:{}", x.reason);
        x
    }
}
impl From<anyhow::Error> for ErrorFrontend {
    fn from(value: anyhow::Error) -> Self {
        let x = Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
        };
        log::error!("msg:{}", x.reason);
        x
    }
}

impl From<hex::FromHexError> for ErrorFrontend {
    fn from(value: hex::FromHexError) -> Self {
        let x = Self {
            reason: format!("{}", value),
            detail: "".to_string(),
        };
        log::error!("msg:{}", x.reason);
        x
    }
}
impl From<String> for ErrorFrontend {
    fn from(value: String) -> Self {
        Self {
            reason: value,
            detail: "".to_string(),
        }
    }
}
impl From<&str> for ErrorFrontend {
    fn from(value: &str) -> Self {
        Self {
            reason: value.to_owned(),
            detail: "".to_string(),
        }
    }
}

impl From<bson::oid::Error> for ErrorFrontend {
    fn from(value: bson::oid::Error) -> Self {
        Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
        }
    }
}
impl From<serde_json::Error> for ErrorFrontend {
    fn from(value: serde_json::Error) -> Self {
        Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
        }
    }
}

impl From<keyring::Error> for ErrorFrontend {
    fn from(value: keyring::Error) -> Self {
        Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
        }
    }
}

async fn random_sleep() {
    #[cfg(feature = "random-sleep")]
    {
        let duration = {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            let millis = rng.gen_range(500..1700);
            let duration = std::time::Duration::from_millis(millis);
            duration
        };
        tokio::time::sleep(duration).await
    }
}

fn random_error() -> Result<(), ErrorFrontend> {
    #[cfg(not(feature = "random-error"))]
    return Ok(());
    #[cfg(feature = "random-error")]
    {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let x: i8 = rng.gen_range(1..100);
        if x > 50 {
            Err(ErrorFrontend {
                reason: format!("random error of {}", x),
                detail: "".to_string(),
            })
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, typescript_type_def::TypeDef)]
pub struct MuiDataGridData {
    fields: Vec<Field>,
    data: Vec<HashMap<String, String>>,
    sql: Option<String>,
    total: usize,
    cache_time: Option<u64>,
}

impl From<QueryResp> for MuiDataGridData {
    fn from(value: QueryResp) -> Self {
        let QueryResp {
            fields,
            data,
            total,
            sql,
            cache_time,
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
            cache_time,
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
    fn test_multi_query_resp_serde() {
        let xs: Vec<Result<QueryResp, String>> = vec![
            Err("this a error".to_string()),
            Ok(QueryResp {
                fields: vec![],
                data: vec![],
                sql: None,
                total: 0,
                cache_time: None,
            }),
        ];

        let ret = serde_json::to_string_pretty(&xs).unwrap();
        println!("{}", ret);
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
