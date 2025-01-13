use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use bson::{Document, doc};
use dbnexus_db::app::{self, App, DBConnectionConfig, Open, open};
use dbnexus_db::completion::{CompletionItem, SemanticToken};
use dbnexus_db::db::{
    ColumnType, ExportKind, Field, FormatSqlOpts, Index, ListTable, NamedStaff, ProcedureDetail,
    QueryResp, SqlExecuteRet, TableListResp, TriggerDetail, UdfDetail,
    UdfOrPrecedureExtraParameter,
};
use dbnexus_db::meta_value::MetaValue;
use dbnexus_db::mongo::Mongo;
use dbnexus_db::oracle::OracleDB;
use dbnexus_db::pg::PostGresDB;
use dbnexus_db::query_log::QueryLogEvent;
use dbnexus_db::redis::{RedisDB, RedisListTable};
#[tauri::command]
pub async fn page_init_callback(app: tauri::State<'_, App>) -> Result<(), ()> {
    log::info!("page init call clear all open connection.");
    *app.opens.lock().await = Default::default();
    Ok(())
}

#[tauri::command]
pub async fn drop_database(
    name: String, database: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("open db:{}", name);
    let x = app
        .get_opened(name.as_str())
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))?;

    // first close connections because we want delete it

    app.close_conn_has_prefix(&format!("{}/{}", name, database))
        .await;

    match x {
        Open::Catalog(x) => {
            x.drop_database(&database)
                .await
                .map_err(|x| AnyhowErrorFrontend::from(x))?;
            app.opens
                .lock()
                .await
                .remove(&format!("{}:{}", name, database));

            Ok(())
        }

        _ => Err(AnyhowErrorFrontend::from(format!(
            "'{}' is not a db catalog",
            name
        ))),
    }
}

#[tauri::command]
pub async fn drop_schema(
    name: String, schema: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    // first close connections because we want delete it

    app.close_conn_has_prefix(&format!("{}/{}", name, schema))
        .await;
    let x = app
        .get_opened(name.as_str())
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))?;
    match x {
        Open::DataBase(x) => {
            x.drop_schema(&schema)
                .await
                .map_err(|x| AnyhowErrorFrontend::from(x))?;
            app.opens
                .lock()
                .await
                .remove(&format!("{}:{}", name, schema));

            Ok(())
        }
        _ => Err(AnyhowErrorFrontend::from(format!(
            "'{}' is not a db database",
            name
        ))),
    }
}

#[tauri::command]
pub async fn create_database(
    name: String, database: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let x = app
        .get_opened(name.as_str())
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))?;
    match x {
        Open::Catalog(x) => {
            x.create_database(&database)
                .await
                .map_err(|x| AnyhowErrorFrontend::from(x))?;

            let db_name = format!("{}/{}", name, database);

            let db = x
                .open_db(&database)
                .await
                .map_err(|x| AnyhowErrorFrontend::from(x))?;

            app.enter_open_child(&db_name, &Open::from(db.clone()))
                .await
                .map_err(|x| AnyhowErrorFrontend::from(x))?;

            app.opens.lock().await.insert(db_name, db.into());

            Ok(())
        }

        _ => Err(AnyhowErrorFrontend::from(format!(
            "'{}' is not a db catalog",
            name
        ))),
    }
}

#[tauri::command]
pub async fn create_schema(
    name: String, schema: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;

    let x = app
        .get_opened(name.as_str())
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))?;
    match x {
        Open::DataBase(x) => {
            x.create_schema(&schema)
                .await
                .map_err(|x| AnyhowErrorFrontend::from(x))?;
            let db = x
                .open_schema(&schema)
                .await
                .map_err(|x| AnyhowErrorFrontend::from(x))?;
            app.opens
                .lock()
                .await
                .insert(format!("{}/{}", name, schema), db.into());

            Ok(())
        }
        _ => Err(AnyhowErrorFrontend::from(format!(
            "'{}' is not a db database",
            name
        ))),
    }
}

#[tauri::command]
pub async fn open_conn(
    name: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("open db:{}", name);
    app.open(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
        .map(|_| ())
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
) -> Result<Vec<ListTable>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    log::info!("show table_name db:{}", name);
    app.list_table(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn redis_list_table(
    name: String, filter: Option<String>, app: tauri::State<'_, App>,
) -> Result<RedisListTable, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.redis_list_table(&name, filter.as_ref().map(|x| x.as_str()))
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn redis_zset_show_table(
    name: String, key: String, min: Option<f64>, max: Option<f64>, where_str: Option<&str>,
    app: tauri::State<'_, App>,
) -> Result<MuiDataTableListResp, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(&name)
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))?;

    let r = match schema.as_any().downcast_ref::<RedisDB>() {
        Some(r) => r,
        None => return Err(AnyhowErrorFrontend::from(format!("'{}' not redis", name))),
    };
    let x = r
        .zset_show_table(&key, where_str, min, max)
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))?;

    Ok(MuiDataTableListResp::from(x))
}

#[tauri::command]
pub async fn show_columns(
    name: String, table_name: String, app: tauri::State<'_, App>,
) -> Result<Vec<Field>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.show_columns(name.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn show_table(
    name: String, table_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, app: tauri::State<'_, App>,
) -> Result<MuiDataTableListResp, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let x = app
        .show_table(
            name.as_str(),
            &table_name,
            where_str,
            order_by,
            page,
            page_size,
        )
        .await
        .map_err(AnyhowErrorFrontend::from)?;

    Ok(MuiDataTableListResp::from(x))
}

#[tauri::command]
pub async fn show_indexes(
    name: String, table_name: String, app: tauri::State<'_, App>,
) -> Result<Vec<Index>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.show_indexes(name.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn rename_index(
    name: String, table_name: String, old_name: String, new_name: String,
    app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;

    schema
        .rename_index(&table_name, &old_name, &new_name)
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))
}

#[tauri::command]
pub async fn drop_index(
    name: String, table_name: String, index: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;

    schema
        .drop_index(&table_name, &index)
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))
}

#[tauri::command]
pub async fn insert(
    name: String, table_name: String, values: HashMap<String, Option<String>>,
    app: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.insert(name.as_str(), &table_name, values)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn update(
    name: String, table_name: String, values: HashMap<String, Option<String>>,
    wheres: HashMap<String, Option<String>>, app: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.update(name.as_str(), &table_name, values, wheres)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn truncate_table(
    name: String, table_name: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.truncate_table(name.as_str(), &table_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn drop_table_column(
    name: String, table_name: String, column_name: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    schema
        .drop_table_column(&table_name, &column_name)
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    Ok(())
}

#[tauri::command]
pub async fn modify_table_column(
    name: String, table_name: String, old_name: String, new_name: String, typ: Option<ColumnType>,
    app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    schema
        .modify_table_column(&table_name, &old_name, &new_name, typ)
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    Ok(())
}

#[tauri::command]
pub async fn add_table_column(
    name: String, table_name: String, col_name: String, typ: ColumnType, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    schema
        .add_table_column(&table_name, &col_name, typ)
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    Ok(())
}

#[tauri::command]
pub async fn list_data_types(
    name: String, app: tauri::State<'_, App>,
) -> Result<&'static [&'static str], AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    let ret = schema
        .list_data_types()
        .map_err(AnyhowErrorFrontend::from)?;
    Ok(ret)
}

#[tauri::command]
pub async fn delete(
    name: String, table_name: String, wheres: HashMap<String, Option<String>>,
    app: tauri::State<'_, App>,
) -> Result<SqlExecuteRet, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.delete(name.as_str(), &table_name, wheres)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn list_view(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<NamedStaff>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.list_view(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn server_metas(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<MetaValue>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let o = app
        .get_opened(&name)
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))?;
    match o {
        Open::Catalog(x) => x.show_metas().await,
        Open::DataBase(x) => x.show_metas().await,
        Open::Schema(x) => x.show_metas().await,
    }
    .map_err(|x| AnyhowErrorFrontend::from(x))
}

#[tauri::command]
pub async fn list_materialized_view(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<ListTable>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let s = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match s.as_any().downcast_ref::<PostGresDB>() {
        Some(x) => x
            .list_materialized_view()
            .await
            .map_err(|e| AnyhowErrorFrontend::from(e)),
        None => {
            return Err(AnyhowErrorFrontend::from(format!(
                "open named '{}' is not postgres",
                name
            )));
        }
    }
}
#[tauri::command]
pub async fn drop_view(
    name: String, view_name: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.drop_view(name.as_str(), &view_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn drop_materialized_view(
    name: String, view_name: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(&name)
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))?;

    match schema.as_any().downcast_ref::<PostGresDB>() {
        Some(x) => {
            x.drop_materialized_view(&view_name)
                .await
                .map_err(|x| AnyhowErrorFrontend::from(x))?;
            Ok(())
        }

        None => Err(AnyhowErrorFrontend::from(format!(
            "'{}' is not a postgres",
            name
        ))),
    }
}

#[tauri::command]
pub async fn drop_table(
    name: String, table_name: String, cascade: bool, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.drop_table(name.as_str(), &table_name, cascade)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn show_view(
    name: String, view_name: String, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize, app: tauri::State<'_, App>,
) -> Result<MuiDataTableListResp, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    app.show_view(
        name.as_str(),
        &view_name,
        where_str,
        order_by,
        page,
        page_size,
    )
    .await
    .map_err(AnyhowErrorFrontend::from)
    .map(MuiDataTableListResp::from)
}
#[tauri::command]
pub async fn show_view_coumns(
    name: String, view_name: String, app: tauri::State<'_, App>,
) -> Result<Vec<Field>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let schema = app
        .get_schema(&name)
        .await
        .map_err(AnyhowErrorFrontend::from)?;

    schema
        .show_view_columns(&view_name)
        .await
        .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn export_query_result_to_file(
    name: String, sql: String, kind: ExportKind, file: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let sql = sql.as_str();
    app.get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?
        .export_query_result_to_file(sql, kind, &file)
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))?;
    Ok(())
}
#[tauri::command]
pub async fn is_dangerous_sql(
    name: String, sql: String, app: tauri::State<'_, App>,
) -> Result<bool, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let sql = sql.as_str();
    let db = app
        .get_opened(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;

    let result = match db {
        Open::DataBase(x) => x.is_dangerous_sql(sql),
        Open::Schema(x) => x.is_dangerous_sql(sql),
        Open::Catalog(x) => x.is_dangerous_sql(sql),
    }
    .map_err(|x| AnyhowErrorFrontend::from(x))?;

    Ok(result)
}
#[tauri::command]
pub async fn query_sql(
    name: String, query_id: uuid::Uuid, sql: String, app: tauri::State<'_, App>,
) -> Result<Vec<Result<MuiDataGridData, String>>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let sql = sql.as_str();
    let db = app
        .open(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;

    let result = match db {
        Open::DataBase(x) => x.query_sql(query_id, sql).await,
        Open::Schema(x) => x.query_sql(query_id, sql).await,
        Open::Catalog(x) => x.query_sql(query_id, sql).await,
    };

    result.map_err(AnyhowErrorFrontend::from).map(|x| {
        x.into_iter()
            .map(|x| match x {
                Err(e) => Err(e),
                Ok(x) => {
                    let QueryResp { fields, data } = x;
                    Ok(MuiDataGridData::from((fields, data)))
                }
            })
            .collect::<Vec<_>>()
    })
}

#[tauri::command]
pub async fn query_logs(
    name: String, query_id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<Vec<QueryLogEvent>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .open(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;

    match db {
        Open::DataBase(x) => x.get_query_log(query_id).await,
        Open::Schema(x) => x.get_query_log(query_id).await,
        Open::Catalog(x) => x.get_query_log(query_id).await,
    }
    .map_err(|x| AnyhowErrorFrontend::from(x))
}

#[tauri::command]
pub async fn query_sql_open_connection(
    name: String, app: tauri::State<'_, App>,
) -> Result<uuid::Uuid, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await.map_err(|e| e.to_string())?;
    let result = match db {
        Open::DataBase(x) => x.open_connection().await,
        Open::Schema(x) => x.open_connection().await,
        Open::Catalog(x) => x.open_connection().await,
    };
    result.map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn query_sql_re_connection(
    name: String, query_id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await.map_err(|e| e.to_string())?;
    let result = match db {
        Open::DataBase(x) => x.re_connect(query_id).await,
        Open::Schema(x) => x.re_connect(query_id).await,
        Open::Catalog(x) => x.re_connect(query_id).await,
    };
    result.map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn query_sql_close_connection(
    name: String, query_id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await.map_err(|e| e.to_string())?;
    let result = match db {
        Open::DataBase(x) => x.close_connection(query_id).await,
        Open::Schema(x) => x.close_connection(query_id).await,
        Open::Catalog(x) => x.close_connection(query_id).await,
    };
    result.map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn export_table_or_view(
    name: String, table_or_view: String, filepath: String, kind: ExportKind,
    app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    db.export_table_to_file(table_or_view.as_str(), filepath.as_str(), kind)
        .await
        .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn format_sql(
    name: String, sql: String, opts: Option<FormatSqlOpts>, app: tauri::State<'_, App>,
) -> Result<String, AnyhowErrorFrontend> {
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
    ret.map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn check_conn(
    name: String, app: tauri::State<'_, App>,
) -> Result<
    String, // human readable time
    AnyhowErrorFrontend,
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
        .map_err(AnyhowErrorFrontend::from)
        .map(|d| {
            //
            humantime::format_duration(Duration::from_millis(
                d.as_millis() as u64, //最小单位毫秒
            ))
            .to_string()
        })
    })
    .await
    .map_err(|_| {
        AnyhowErrorFrontend::from(format!("check connection status not finished in {}s", t))
    })?
}

#[tauri::command]
pub async fn test_connection(
    config: DBConnectionConfig,
) -> Result<
    u64, // milliseconds
    AnyhowErrorFrontend,
> {
    random_error()?;

    let t = 5;
    tokio::time::timeout(Duration::from_secs(5), async {
        let db = app::open(&config).await.map_err(|e| e.to_string())?;
        match db {
            app::Open::DataBase(x) => x.check_conn().await,
            app::Open::Schema(x) => x.check_conn().await,
            app::Open::Catalog(x) => x.check_conn().await,
        }
        .map_err(AnyhowErrorFrontend::from)
        .map(|d| d.as_millis() as u64)
    })
    .await
    .map_err(|_| AnyhowErrorFrontend::from(format!("test connection not finished in {}s", t)))?
}

#[tauri::command]
pub async fn config_list_database_or_schema(
    conn: DBConnectionConfig,
) -> Result<Vec<String>, AnyhowErrorFrontend> {
    let x = open(&conn).await?;
    match x {
        Open::Catalog(catalog_trait) => catalog_trait
            .list_database()
            .await
            .map_err(AnyhowErrorFrontend::from),
        Open::DataBase(dbtrait) => dbtrait
            .list_schema()
            .await
            .map_err(AnyhowErrorFrontend::from),
        Open::Schema(_) => Ok(vec![]),
    }
}

#[tauri::command]
pub async fn list_database(
    name: String, app: tauri::State<'_, App>,
) -> Result<
    Vec<String>, // milliseconds
    AnyhowErrorFrontend,
> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await?;
    match db {
        app::Open::Catalog(x) => x.list_database().await,
        _ => return Err(AnyhowErrorFrontend::from("not catalog trait")),
    }
    .map_err(AnyhowErrorFrontend::from)
}

#[tauri::command]
pub async fn list_schema(
    name: String, app: tauri::State<'_, App>,
) -> Result<
    Vec<String>, // milliseconds
    AnyhowErrorFrontend,
> {
    random_sleep().await;
    random_error()?;
    let db = app.open(name.as_str()).await?;
    match db {
        app::Open::DataBase(x) => x.list_schema().await,
        _ => return Err(AnyhowErrorFrontend::from("not database trait")),
    }
    .map_err(AnyhowErrorFrontend::from)
}
#[tauri::command]
pub async fn auto_completion(
    name: String, sql: String, line: u32, col: u32, app: tauri::State<'_, App>,
) -> Result<
    Vec<CompletionItem>, // milliseconds
    AnyhowErrorFrontend,
> {
    random_sleep().await;
    random_error()?;
    // TODO front is very hard to pass this parameter to the server
    // because of manoca language is global registerd.
    let query_id = uuid::Uuid::new_v4();
    let use_lexer = true;
    let db = app.get_opened(name.as_str()).await?;
    let ret = match db {
        Open::Catalog(x) => {
            x.auto_completion(query_id, &sql, line, col, use_lexer)
                .await
        }
        Open::DataBase(x) => {
            x.auto_completion(query_id, &sql, line, col, use_lexer)
                .await
        }
        Open::Schema(x) => {
            x.auto_completion(query_id, &sql, line, col, use_lexer)
                .await
        }
    }
    .map_err(|x| AnyhowErrorFrontend::from(x))?;
    Ok(ret)
}

#[tauri::command]
pub async fn semantic_tokens(
    name: String, sql: String, app: tauri::State<'_, App>,
) -> Result<
    Vec<SemanticToken>, // milliseconds
    AnyhowErrorFrontend,
> {
    random_sleep().await;
    random_error()?;

    let db = app.get_opened(name.as_str()).await?;
    let ret = match db {
        Open::Catalog(x) => x.senmatic_tokens(&sql).await,
        Open::DataBase(x) => x.senmatic_tokens(&sql).await,
        Open::Schema(x) => x.senmatic_tokens(&sql).await,
    }
    .map_err(|x| AnyhowErrorFrontend::from(x))?;

    Ok(ret)
}

#[tauri::command]
pub async fn oracle_query_set_autocommit(
    name: String, id: uuid::Uuid, commit: bool, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x
            .query_set_autocommit(id, commit)
            .await
            .map_err(AnyhowErrorFrontend::from),
        None => Err(AnyhowErrorFrontend::from(format!(
            "db '{}' is not oracle",
            name
        ))),
    }
}
#[tauri::command]
pub async fn oracle_query_autocommit(
    name: String, id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<bool, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x
            .query_autocommit(id)
            .await
            .map_err(AnyhowErrorFrontend::from),
        None => Err(AnyhowErrorFrontend::from(format!(
            "db '{}' is not oracle",
            name
        ))),
    }
}
#[tauri::command]
pub async fn oracle_query_commit(
    name: String, id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x
            .query_commit(id)
            .await
            .map_err(|e| AnyhowErrorFrontend::from(e)),
        None => Err(format!("db '{}' is not oracle", name).into()),
    }
}
#[tauri::command]
pub async fn oracle_query_rollback(
    name: String, id: uuid::Uuid, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<OracleDB>() {
        Some(x) => x
            .query_rollback(id)
            .await
            .map_err(|e| AnyhowErrorFrontend::from(e)),
        None => Err(format!("db '{}' is not oracle", name).into()),
    }
}

#[tauri::command]
pub async fn list_trigger(
    name: String, table_name: String, app: tauri::State<'_, App>,
) -> Result<Vec<NamedStaff>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;

    db.list_trigger(&table_name)
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn drop_trigger(
    name: String, table_name: String, trigger: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;

    db.drop_trigger(&table_name, &trigger)
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn enable_trigger(
    name: String, table_name: String, trigger: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;

    db.enable_trigger(&table_name, &trigger)
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))
}
#[tauri::command]
pub async fn disable_trigger(
    name: String, table_name: String, trigger: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;

    db.disable_trigger(&table_name, &trigger)
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn show_trigger(
    name: String, table_name: String, trigger: String, app: tauri::State<'_, App>,
) -> Result<TriggerDetail, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;

    db.show_trigger(&table_name, &trigger)
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn rename_table(
    name: String, old: String, new: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;

    db.rename_table(&old, &new)
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn duplicate_table(
    name: String, from: String, to: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;

    db.duplicate_table(&from, &to)
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn mongo_insert(
    name: String, table_name: String, doc: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<mongodb::results::InsertOneResult, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let doc = parse_bson_doc(doc.as_slice())?;

    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;
    match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.insert(table_name.as_str(), doc).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(|e| AnyhowErrorFrontend::from(e))
}
#[tauri::command]
pub async fn mongo_update(
    name: String, table_name: String, update: Vec<u8>, query: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<mongodb::results::UpdateResult, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let update = parse_bson_doc(update.as_slice())?;
    let query = parse_bson_doc(query.as_slice())?;

    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| AnyhowErrorFrontend::from(e))?;
    match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.update(table_name.as_str(), update, query).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn mongo_delete(
    name: String, table_name: String, query: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<mongodb::results::DeleteResult, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let query = parse_bson_doc(query.as_slice())?;

    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.delete(table_name.as_str(), query).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(|e| AnyhowErrorFrontend::from(e))
}

#[tauri::command]
pub async fn mongo_show_collection_table_list(
    name: String, table_name: String, filter: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<MuiDataTableListResp, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let filter = parse_bson_doc(filter.as_slice())?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => {
            x.show_collection_table_list(table_name.as_str(), filter)
                .await
        }
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(|e| AnyhowErrorFrontend::from(e))
    .map(MuiDataTableListResp::from)
}

#[tauri::command]
pub async fn mongo_show_collection_docs(
    name: String, table_name: String, filter: Vec<u8>, app: tauri::State<'_, App>,
) -> Result<DocsAndCount, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let filter = parse_bson_doc(filter.as_slice())?;
    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let docs = match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.show_collection_docs(table_name.as_str(), filter).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(|e| AnyhowErrorFrontend::from(e))?;
    DocsAndCount::try_from(docs).map_err(|e| e.into())
}
#[tauri::command]
pub async fn mongo_get_one_document(
    name: String, table_name: String, id: String, app: tauri::State<'_, App>,
) -> Result<Vec<u8>, AnyhowErrorFrontend> {
    random_sleep().await;
    random_error()?;
    let id = bson::oid::ObjectId::from_str(id.as_str()).map_err(AnyhowErrorFrontend::from)?;

    let db = app
        .get_schema(name.as_str())
        .await
        .map_err(|e| e.to_string())?;
    let doc = match db.as_any().downcast_ref::<Mongo>() {
        Some(x) => x.get_one_document(table_name.as_str(), id).await,
        None => return Err(format!("db '{}' not mongo", name).into()),
    }
    .map_err(|e| AnyhowErrorFrontend::from(e))?;
    Ok(encode_bson_doc(&doc).map_err(AnyhowErrorFrontend::from)?)
}

#[tauri::command]
pub async fn redis_list_lpush(
    name: String, key: String, value: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_lpush(key.as_str(), value.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_list_rpush(
    name: String, key: String, value: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_rpush(key.as_str(), value.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_list_remove(
    name: String, key: String, value: String, count: isize, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_remove(key.as_str(), count, value.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_list_set(
    name: String, key: String, value: String, index: isize, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.list_set(key.as_str(), index, value.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_set_replace(
    name: String, key: String, old: String, new: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let old = hex::decode(old.as_bytes()).map_err(AnyhowErrorFrontend::from)?;
    let new = hex::decode(new.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.set_replace(key.as_str(), old.as_slice(), new.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_set_del(
    name: String, key: String, member: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.set_del(key.as_str(), member.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_set_add(
    name: String, key: String, member: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.set_add(key.as_str(), member.as_slice())
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_hash_replace(
    name: String, key: String, old_member: String, new_member: String, new_value: String,
    app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let old_member = hex::decode(old_member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let new_member = hex::decode(new_member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;
    let new_value = hex::decode(new_value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.hash_replace(key.as_str(), old_member.as_slice(), &new_member, &new_value)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_hash_del(
    name: String, key: String, member: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.hash_del(key.as_str(), &member)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_hash_add(
    name: String, key: String, member: String, value: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;
    let value = hex::decode(value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.hash_add(key.as_str(), &member, &value)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_zset_replace(
    name: String, key: String, old_member: String, new_member: String, new_value: f64,
    app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let old_member = hex::decode(old_member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;
    let new_member = hex::decode(new_member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.zset_replace(key.as_str(), &old_member, &new_member, new_value)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn redis_zset_del(
    name: String, key: String, member: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.zset_del(key.as_str(), &member)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}
#[tauri::command]
pub async fn redis_zset_add(
    name: String, key: String, member: String, value: f64, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let member = hex::decode(member.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.zset_add(key.as_str(), &member, value)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
}

#[tauri::command]
pub async fn list_procedure(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<NamedStaff>, AnyhowErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    schema
        .list_procedure()
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))
}

#[tauri::command]
pub async fn show_procedure(
    name: String, procedure: String, arg: Option<UdfOrPrecedureExtraParameter>,
    app: tauri::State<'_, App>,
) -> Result<ProcedureDetail, AnyhowErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    schema
        .show_procedure(&procedure, arg)
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))
}

#[tauri::command]
pub async fn drop_procedure(
    name: String, procedure: String, arg: Option<UdfOrPrecedureExtraParameter>,
    app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    schema
        .drop_procedure(&procedure, arg)
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))
}

#[tauri::command]
pub async fn list_udf(
    name: String, app: tauri::State<'_, App>,
) -> Result<Vec<NamedStaff>, AnyhowErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    schema
        .list_udf()
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))
}

#[tauri::command]
pub async fn show_udf(
    name: String, udf: String, arg: Option<UdfOrPrecedureExtraParameter>,
    app: tauri::State<'_, App>,
) -> Result<UdfDetail, AnyhowErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    schema
        .show_udf(&udf, arg)
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))
}

#[tauri::command]
pub async fn drop_udf(
    name: String, udf: String, arg: Option<UdfOrPrecedureExtraParameter>,
    app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    schema
        .drop_udf(&udf, arg)
        .await
        .map_err(|x| AnyhowErrorFrontend::from(x))
}

#[tauri::command]
pub async fn redis_string_set(
    name: String, key: String, value: String, app: tauri::State<'_, App>,
) -> Result<(), AnyhowErrorFrontend> {
    let value = hex::decode(value.as_bytes()).map_err(AnyhowErrorFrontend::from)?;

    let schema = app
        .get_schema(name.as_str())
        .await
        .map_err(AnyhowErrorFrontend::from)?;
    match schema.as_any().downcast_ref::<RedisDB>() {
        Some(x) => {
            x.string_set(key.as_str(), &value)
                .await
                .map_err(AnyhowErrorFrontend::from)?;
        }
        None => return Err(format!("uuid {} is not redis", name).into()),
    }
    return Ok(());
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
fn encode_bson_doc(doc: &bson::Document) -> Result<Vec<u8>, String> {
    let bs = bson::to_vec(&doc).map_err(|e| e.to_string())?;
    Ok(bs)
}

fn parse_bson_doc(bs: &[u8]) -> Result<Document, String> {
    let doc = bson::Document::from_reader(std::io::Cursor::new(bs)).map_err(|e| e.to_string())?;
    Ok(doc)
}

#[derive(serde::Serialize)]
pub struct AnyhowErrorFrontend {
    // short message on why
    reason: String,
    // long message about why and backtrace
    detail: String,
}

impl From<tauri::Error> for AnyhowErrorFrontend {
    fn from(value: tauri::Error) -> Self {
        let x = Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
        };
        log::error!("msg:{},details:{}", x.reason, x.detail);
        x
    }
}
impl From<anyhow::Error> for AnyhowErrorFrontend {
    fn from(value: anyhow::Error) -> Self {
        let x = Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
        };
        log::error!("msg:{},details:{}", x.reason, x.detail);
        x
    }
}

impl From<hex::FromHexError> for AnyhowErrorFrontend {
    fn from(value: hex::FromHexError) -> Self {
        let x = Self {
            reason: format!("{}", value),
            detail: "".to_string(),
        };
        log::error!("msg:{},details:{}", x.reason, x.detail);
        x
    }
}
impl From<String> for AnyhowErrorFrontend {
    fn from(value: String) -> Self {
        Self {
            reason: value,
            detail: "".to_string(),
        }
    }
}
impl From<&str> for AnyhowErrorFrontend {
    fn from(value: &str) -> Self {
        Self {
            reason: value.to_owned(),
            detail: "".to_string(),
        }
    }
}

impl From<bson::oid::Error> for AnyhowErrorFrontend {
    fn from(value: bson::oid::Error) -> Self {
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

fn random_error() -> Result<(), AnyhowErrorFrontend> {
    #[cfg(not(feature = "random-error"))]
    return Ok(());
    #[cfg(feature = "random-error")]
    {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let x: i8 = rng.gen_range(1..100);
        if x > 50 {
            Err(AnyhowErrorFrontend {
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
}

#[derive(Clone, Debug, serde::Serialize, typescript_type_def::TypeDef)]
pub struct MuiDataTableListResp {
    pub total: usize,
    pub sql: Option<String>,
    data: MuiDataGridData,
}

impl From<TableListResp> for MuiDataTableListResp {
    fn from(value: TableListResp) -> Self {
        let TableListResp {
            fields,
            total,
            sql,
            data,
        } = value;

        Self {
            total,
            sql,
            data: MuiDataGridData::from((fields, data)),
        }
    }
}

impl From<(Vec<Field>, Vec<Vec<Option<String>>>)> for MuiDataGridData {
    fn from(value: (Vec<Field>, Vec<Vec<Option<String>>>)) -> Self {
        let (fields, old_data) = value;
        let data: Vec<_> = old_data
            .into_iter()
            .map(|x| {
                let mut m = HashMap::new();
                fields
                    .iter()
                    .zip(x.into_iter())
                    .for_each(|(f, value)| match value {
                        Some(value) => {
                            m.insert(f.field.clone(), value);
                        }

                        None => {}
                    });

                m
            })
            .collect();
        Self { fields, data }
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
            }),
        ];

        let ret = serde_json::to_string_pretty(&xs).unwrap();
        println!("{}", ret);
    }

    #[test]
    fn test_mui_datagrid() {
        let data = vec![vec![Some("100".to_string()), Some("hello".to_string())]];
        let field = vec![
            Field::from_name_type("a", "int"),
            Field::from_name_type("b", "string"),
        ];

        let x = MuiDataGridData::from((field, data));
        let str = serde_json::to_string_pretty(&x).unwrap();
        println!("{}", str);
    }
}
