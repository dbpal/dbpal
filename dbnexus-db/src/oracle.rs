use dashmap::DashMap;
use oracle::pool::Pool;
use oracle::{Connection, Row, Statement};
/// notice oracle sql 不能带有分号
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::vec;
use tokio::task::spawn_blocking;

use crate::datasource_config::OracleDBConfig;
use crate::db::{
    BackendKind, CheckConn, ColumnType, CreateIndexReq, CreateSchemaExtra, CreateTableAndIndexSql,
    DBTrait, DefAndMetas, ExportFrame, ExportKind, ExportQueryResult, Field, GetBackendKind, Index,
    IndexKind, ListTable, MetaValue, NamedStaff, OptionMustBeSomeErr, POOL_MAX_CONNECTIONS,
    ProcedureDetail, QueryEvent, QueryEventEmitter, QueryResp, QuerySql, ReadOnlyTrait,
    SchemaTrait, ServerMetas, ShowTableParameter, SqlBuilder, TriggerDetail, TriggerEnable,
    TriggerEvent, TriggerTiming, UdfOrProcedureExtraParameter, XlsxEdit, create_html_table_data,
    create_html_table_header, create_markdown_data, create_markdown_header, escape_sql_str,
    export_query_as_xlsx_send_blocking_data,
};

use crate::helper::{create_csv, run_with_timeout};
use crate::language_server::LexerCompletionTrait;

#[derive(Clone)]
pub struct OracleDB {
    pool: Arc<Pool>,
    query_connection: Arc<DashMap<String, Arc<Mutex<Connection>>>>,
    pub(crate) config: Arc<OracleDBConfig>,
    pub(crate) key_words_cache: Arc<tokio::sync::Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl ServerMetas for OracleDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let row = self.query_row("SELECT * FROM v$version").await?;
        let mut ret = Vec::new();
        let banner: String = row.get(0)?;
        let banner_full: String = row.get(1)?;
        let banner_legacy: String = row.get(2)?;
        ret.push(MetaValue::new("BANNER", banner));
        ret.push(MetaValue::new("BANNER_FULL", banner_full));
        ret.push(MetaValue::new("BANNER_LEGACY", banner_legacy));
        Ok(ret)
    }
}
impl std::fmt::Debug for OracleDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "oracle@{:?}", self.config)
    }
}

impl OracleDB {
    pub fn new(config: OracleDBConfig, min_conn: Option<u32>) -> anyhow::Result<Self> {
        let mut builder = oracle::pool::PoolBuilder::new(
            &config.username,
            &config.password,
            format!("//{}:{}/{}", config.host, config.port, config.db),
        );
        builder
            .min_connections(min_conn.unwrap_or(10))
            .max_connections(POOL_MAX_CONNECTIONS);

        let pool = builder.build()?;
        anyhow::Ok(Self {
            pool: Arc::new(pool),
            query_connection: Default::default(),
            config: Arc::new(config),
            key_words_cache: Default::default(),
        })
    }
}

const QUERY_AUTO_COMMIT: bool = true;
#[async_trait::async_trait]
impl ExportQueryResult for OracleDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self)) as Box<dyn ExportQueryResult>
    }

    async fn export_query_result(
        &self, query_id: Option<&str>, sql: &str, kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let sql = sql.to_owned();
        let query_id = query_id.map(|s| s.to_owned());
        spawn_blocking(move || {
            let query_id = query_id.as_deref();
            let conn = this.get_conn_by_query_id(query_id)?;
            let conn = conn.lock().expect("lock failed");
            let mut first = true;
            let rows = conn.query(&sql, &[])?;
            let mut row_num = 0;
            for row in rows {
                let row = row?;
                match kind {
                    ExportKind::Csv {
                        include_header,
                        delimiter,
                    } => {
                        if include_header && first {
                            let fs: Vec<_> = row
                                .column_info()
                                .iter()
                                .map(|x| Some(x.name().to_owned()))
                                .collect();
                            sender.blocking_send(create_csv(&fs, delimiter)?.into())?;
                        }
                        let data = encode_row(row)?;
                        sender.blocking_send(create_csv(&data, delimiter)?.into())?;
                    }
                    ExportKind::Json { .. } | ExportKind::JsonLine => {
                        let value = encode_row_as_json(&row)?;
                        sender.blocking_send(value.into())?;
                    }
                    ExportKind::Sql { .. } => {
                        anyhow::bail!("can't export query result to sql")
                    }
                    ExportKind::Xlsx { include_header } => {
                        if include_header && first {
                            for (index, col) in row.column_info().iter().enumerate() {
                                sender.blocking_send(
                                    XlsxEdit {
                                        row: row_num,
                                        column: index as rust_xlsxwriter::ColNum,
                                        content: col.name().to_owned(),
                                    }
                                    .into(),
                                )?;
                            }

                            row_num += 1;
                        }
                        let data = encode_row(row)?;
                        export_query_as_xlsx_send_blocking_data(
                            &mut row_num,
                            data,
                            sender.clone(),
                        )?;
                    }
                    ExportKind::Html { include_header } => {
                        if include_header && first {
                            let fs: Vec<_> = row
                                .column_info()
                                .iter()
                                .map(|x| x.name().to_owned())
                                .collect();
                            sender.blocking_send(create_html_table_header(&fs).into())?;
                        }
                        let data = encode_row(row)?;
                        sender.blocking_send(create_html_table_data(&[data]).into())?;
                    }
                    ExportKind::MarkDown => {
                        if first {
                            first = false;
                            let fs: Vec<_> = row
                                .column_info()
                                .iter()
                                .map(|x| x.name().to_owned())
                                .collect();
                            sender.blocking_send(create_markdown_header(&fs).into())?;
                        }
                        let data = encode_row(row)?;
                        sender.blocking_send(create_markdown_data(&data).into())?;
                    }
                }
            }
            anyhow::Ok(())
        })
        .await?
    }
}

#[async_trait::async_trait]
impl QuerySql for OracleDB {
    async fn get_auto_commit(&self, query_id: &str) -> anyhow::Result<bool> {
        let this = Clone::clone(self);
        let query_id = query_id.to_owned();
        spawn_blocking(move || {
            let conn = match this.query_connection.get(&query_id) {
                Some(x) => x,
                None => anyhow::bail!("query_id not found"),
            };
            let conn = conn.lock().expect("lock failed");
            anyhow::Ok(conn.autocommit())
        })
        .await?
    }

    async fn set_auto_commit(&self, query_id: &str, commit: bool) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let query_id = query_id.to_owned();
        spawn_blocking(move || {
            let conn = match this.query_connection.get(&query_id) {
                Some(x) => x,
                None => anyhow::bail!("query_id not found"),
            };
            let mut conn = conn.lock().expect("lock failed");
            conn.set_autocommit(commit);
            anyhow::Ok(())
        })
        .await?
    }

    async fn commit(&self, query_id: &str) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let query_id = query_id.to_owned();
        spawn_blocking(move || {
            let conn = match this.query_connection.get(&query_id) {
                Some(x) => x,
                None => anyhow::bail!("query_id not found"),
            };
            let conn = conn.lock().expect("lock failed");
            conn.commit()?;
            anyhow::Ok(())
        })
        .await?
    }

    async fn rollback(&self, query_id: &str) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let query_id = query_id.to_owned();
        spawn_blocking(move || {
            let conn = match this.query_connection.get(&query_id) {
                Some(x) => x,
                None => anyhow::bail!("query_id not found"),
            };
            let conn = conn.lock().expect("lock failed");
            conn.rollback()?;
            anyhow::Ok(())
        })
        .await?
    }

    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        let id = id.to_owned();
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let mut conn = this.get_conn()?;
            conn.set_autocommit(QUERY_AUTO_COMMIT);
            this.query_connection.insert(id, Arc::new(Mutex::new(conn)));
            anyhow::Ok(())
        })
        .await?
    }

    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        if let Some((_, x)) = self.query_connection.remove(&id.to_owned()) {
            tokio::spawn(async move { x.lock().expect("lock failed").close() });
        }
        anyhow::Ok(())
    }

    async fn execute_sql_no_open_conn(&self, sql: &str) -> anyhow::Result<Option<usize>> {
        let ret = self.execute(sql).await?;
        anyhow::Ok(Some(ret.row_count()? as usize))
    }

    async fn execute_sql(
        &self, query_id: &str, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let this = Clone::clone(self);
        let query_id = query_id.to_owned();
        let sql = sql.to_owned();
        spawn_blocking(move || {
            let conn = match this.query_connection.get(&query_id.to_string()) {
                Some(x) => x.clone(),
                None => anyhow::bail!("connection closed???"),
            };
            let mut lock = conn.lock().expect("lock failed");
            let conn = lock.deref_mut();
            let ret = match conn.execute(&sql, &[]) {
                Ok(_) => Ok(QueryResp::execute_ok(sql.to_owned())),
                Err(err) => Err(err.to_string()),
            };
            anyhow::Ok(ret)
        })
        .await?
    }

    async fn query_sql(
        &self, query_id: &str, sql: &str, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let query_id = query_id.to_owned();
        let sql = sql.to_owned();
        spawn_blocking(move || {
            emit.send(QueryEvent::new_status("trying to lock connection."))?;
            let conn = match this.query_connection.get(&query_id.to_string()) {
                Some(x) => x.clone(),
                None => anyhow::bail!("connection closed???"),
            };
            let mut lock = conn.lock().expect("lock failed");
            let conn = lock.deref_mut();
            emit.send(QueryEvent::new_status("locked connection."))?;
            match conn.query(&sql, &[]) {
                Ok(rows) => {
                    let mut first = true;
                    for row in rows {
                        let row = row?;
                        if first {
                            first = false;
                            let fields = infer_schema_by_row(&row);
                            emit.send(
                                QueryResp {
                                    fields,
                                    ..Default::default()
                                }
                                .into(),
                            )?;
                        }
                        let r = encode_row(row)?;
                        emit.send(r.into())?;
                    }
                }
                Err(err) => {
                    emit.send(QueryEvent::Err(err.to_string()))?;
                }
            }
            anyhow::Ok(())
        })
        .await?
    }
}

fn gen_show_sql(
    table_name: &str, fields: Option<String>, where_str: Option<&str>, order_by: Option<&str>,
    page: usize, page_size: usize,
) -> String {
    let mut sql = format!(
        r#"SELECT {} FROM "{table_name}""#,
        if let Some(fields) = fields {
            fields
        } else {
            "*".to_owned()
        }
    );
    if let Some(where_str) = where_str {
        sql.push_str(format!(" WHERE {}", where_str).as_str());
    }
    if let Some(order_by) = order_by {
        sql.push_str(format!(" ORDER BY {}", order_by).as_str());
    }

    format!(
        "{} OFFSET {} ROWS FETCH NEXT {} ROWS ONLY",
        sql,
        (page - 1) * page_size,
        page_size
    )
}

impl OracleDB {
    async fn query_count(&self, table: &str, where_str: Option<&str>) -> anyhow::Result<i64> {
        let sql = format!(
            r#"SELECT count(*) FROM "{table}" {}"#,
            match where_str {
                Some(x) => format!("  WHERE {} ", x),
                None => "".to_string(),
            }
        );
        let row = self.query_row(sql).await?;
        let value: i64 = row.get(0)?;
        anyhow::Ok(value)
    }
}

fn infer_schema_by_row(x: &oracle::Row) -> Vec<Field> {
    let mut ret = Vec::with_capacity(x.column_info().len());
    for c in x.column_info().iter() {
        ret.push(Field::from_name_type(
            c.name().to_string(),
            c.oracle_type().to_string(),
        ))
    }
    ret
}

fn encode_row(row: Row) -> anyhow::Result<Vec<Option<String>>> {
    let mut values = vec![];
    for c in 0..row.sql_values().len() {
        let col_info = &row.column_info()[c];
        let type_not_support = Err(anyhow::Error::msg(format!(
            "type '{}' not supported",
            col_info.oracle_type()
        ))) as anyhow::Result<Vec<Option<String>>>;

        let v = match col_info.oracle_type() {
            oracle::sql_type::OracleType::Varchar2(_) => {
                let x: Option<String> = row.get(c)?;
                x
            }
            oracle::sql_type::OracleType::NVarchar2(_) => {
                let x: Option<String> = row.get(c)?;
                x
            }
            oracle::sql_type::OracleType::Char(_) => {
                let x: Option<String> = row.get(c)?;
                x
            }
            oracle::sql_type::OracleType::NChar(_) => {
                let x: Option<String> = row.get(c)?;
                x
            }
            oracle::sql_type::OracleType::Rowid => {
                let x: Option<String> = row.get(c)?;
                x
            }
            oracle::sql_type::OracleType::Raw(_) => {
                let x: Option<String> = row.get(c)?;
                x
            }
            oracle::sql_type::OracleType::BinaryFloat => {
                let x: Option<f32> = row.get(c)?;
                x.map(|x| x.to_string())
            }

            oracle::sql_type::OracleType::BinaryDouble => {
                let x: Option<f64> = row.get(c)?;
                x.map(|x| x.to_string())
            }
            oracle::sql_type::OracleType::Number(..) => {
                let x: Option<f64> = row.get(c)?;
                x.map(|x| x.to_string())
            }
            oracle::sql_type::OracleType::Float(_) => {
                let x: Option<f64> = row.get(c)?;
                x.map(|x| x.to_string())
            }
            oracle::sql_type::OracleType::Date => {
                let x: Option<chrono::NaiveDate> = row.get(c)?;
                x.map(|x| x.to_string())
            }
            oracle::sql_type::OracleType::Timestamp(_) => {
                let x: Option<chrono::NaiveDateTime> = row.get(c)?;
                x.map(|x| x.to_string())
            }
            oracle::sql_type::OracleType::TimestampTZ(_) => return type_not_support,
            oracle::sql_type::OracleType::TimestampLTZ(_) => return type_not_support,
            oracle::sql_type::OracleType::IntervalDS(..) => return type_not_support,
            oracle::sql_type::OracleType::IntervalYM(_) => return type_not_support,
            oracle::sql_type::OracleType::CLOB => {
                let x: Option<String> = row.get(c)?;
                x
            }
            oracle::sql_type::OracleType::NCLOB => {
                let x: Option<String> = row.get(c)?;
                x
            }
            oracle::sql_type::OracleType::BLOB => {
                let x: Option<String> = row.get(c)?;
                x
            }
            oracle::sql_type::OracleType::BFILE => return type_not_support,
            oracle::sql_type::OracleType::Boolean => {
                let x: Option<bool> = row.get(c)?;
                x.map(|x| x.to_string())
            }
            oracle::sql_type::OracleType::RefCursor => return type_not_support,
            oracle::sql_type::OracleType::Object(_) => return type_not_support,
            oracle::sql_type::OracleType::Long => {
                let x: Option<String> = row.get(c)?;
                x
            }
            oracle::sql_type::OracleType::LongRaw => {
                let x: Option<String> = row.get(c)?;
                x
            }
            oracle::sql_type::OracleType::Json => return type_not_support,
            oracle::sql_type::OracleType::Xml => return type_not_support,
            oracle::sql_type::OracleType::Int64 => {
                let x: Option<i64> = row.get(c)?;
                x.map(|x| x.to_string())
            }
            oracle::sql_type::OracleType::UInt64 => {
                let x: Option<u64> = row.get(c)?;
                x.map(|x| x.to_string())
            }
        };
        values.push(v);
    }
    anyhow::Ok(values)
}

fn put_value_in_sql(typ: &str, value: Option<&str>) -> anyhow::Result<String> {
    let null_string = "NULL".to_string();
    let typ = typ.to_string().to_uppercase();

    match typ.as_str() {
        "VARCHAR2" => anyhow::Ok(match value {
            Some(x) => format!("'{}'", escape_sql_str(x)),
            None => null_string,
        }),
        _ if typ.starts_with("VARCHAR2") => anyhow::Ok(match value {
            Some(x) => format!("'{}'", escape_sql_str(x)),
            None => null_string,
        }),
        "NVARCHAR2" => anyhow::Ok(match value {
            Some(x) => format!("N'{}'", escape_sql_str(x)),
            None => null_string,
        }),
        _ if typ.starts_with("NVARCHAR2") => anyhow::Ok(match value {
            Some(x) => format!("N'{}'", escape_sql_str(x)),
            None => null_string,
        }),

        "CHAR" => anyhow::Ok(match value {
            Some(x) => format!("'{}'", x),
            None => null_string,
        }),
        _ if typ.starts_with("CHAR") => anyhow::Ok(match value {
            Some(x) => format!("'{}'", escape_sql_str(x)),
            None => null_string,
        }),
        "NCHAR" => anyhow::Ok(match value {
            Some(x) => format!("N'{}'", escape_sql_str(x)),
            None => null_string,
        }),
        _ if typ.starts_with("NCHAR") => anyhow::Ok(match value {
            Some(x) => format!("N'{}'", escape_sql_str(x)),
            None => null_string,
        }),
        "RAW" => anyhow::Ok(match value {
            Some(x) => format!("'{}'", x),
            None => null_string,
        }),
        _ if typ.starts_with("RAW") => anyhow::Ok(match value {
            Some(x) => format!("'{}'", x),
            None => null_string,
        }),

        "BINARY_FLOAT" | "BINARY_DOUBLE" | "NUMBER" | "FLOAT" => anyhow::Ok(match value {
            Some(x) => x.to_string(),
            None => null_string,
        }),

        _ if typ.starts_with("FLOAT") || typ.starts_with("NUMBER") => anyhow::Ok(match value {
            Some(x) => x.to_string(),
            None => null_string,
        }),

        "CLOB" => anyhow::Ok(match value {
            Some(x) => format!("'{}'", x),
            None => null_string,
        }),
        "NCLOB" => anyhow::Ok(match value {
            Some(x) => format!("N'{}'", x),
            None => null_string,
        }),
        "BLOB" => anyhow::Ok(match value {
            Some(x) => format!("'{}'", x),
            None => null_string,
        }),
        "LONG" => anyhow::Ok(match value {
            Some(x) => format!("'{}'", x),
            None => null_string,
        }),
        "TIMESTAMP" => anyhow::Ok(match value {
            Some(x) => format!("'{}'", x),
            None => null_string,
        }),
        _ if typ.starts_with("TIMESTAMP") => anyhow::Ok(match value {
            Some(x) => format!("'{}'", x),
            None => null_string,
        }),
        // "BFILE" => {
        //     anyhow::Ok(match value {
        //         Some(x) => format!("'{}'", x),
        //         None => null_string,
        //     })
        // }
        _ => anyhow::bail!(format!("type '{}' not handled", typ)),
    }
}

fn encode_row_as_json(r: &Row) -> anyhow::Result<serde_json::map::Map<String, serde_json::Value>> {
    let mut values = serde_json::map::Map::new();
    for c in 0..r.sql_values().len() {
        let col_info = &r.column_info()[c];
        let type_not_support = Err(anyhow::Error::msg(format!(
            "type '{}' not supported",
            col_info.oracle_type()
        ))) as anyhow::Result<_>;
        let v = match col_info.oracle_type() {
            oracle::sql_type::OracleType::Varchar2(_) => {
                let x: Option<String> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::NVarchar2(_) => {
                let x: Option<String> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::Char(_) => {
                let x: Option<String> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::NChar(_) => {
                let x: Option<String> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::Rowid => {
                let x: Option<String> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::Raw(_) => {
                let x: Option<String> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::BinaryFloat => {
                let x: Option<f32> = r.get(c)?;
                x.map(serde_json::Value::from)
            }

            oracle::sql_type::OracleType::BinaryDouble => {
                let x: Option<f64> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::Number(..) => {
                let x: Option<f64> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::Float(_) => {
                let x: Option<f64> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::Date => {
                let x: Option<chrono::NaiveDate> = r.get(c)?;
                x.map(|x| x.to_string()).map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::Timestamp(_) => {
                let x: Option<chrono::NaiveDateTime> = r.get(c)?;
                x.map(|x| x.to_string()).map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::TimestampTZ(_) => {
                return type_not_support;
            }
            oracle::sql_type::OracleType::TimestampLTZ(_) => return type_not_support,
            oracle::sql_type::OracleType::IntervalDS(..) => return type_not_support,
            oracle::sql_type::OracleType::IntervalYM(_) => return type_not_support,
            oracle::sql_type::OracleType::CLOB => {
                let x: Option<String> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::NCLOB => {
                let x: Option<String> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::BLOB => {
                let x: Option<String> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::BFILE => return type_not_support,
            oracle::sql_type::OracleType::Boolean => {
                let x: Option<bool> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::RefCursor => return type_not_support,
            oracle::sql_type::OracleType::Object(_) => return type_not_support,
            oracle::sql_type::OracleType::Long => {
                let x: Option<String> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::LongRaw => {
                let x: Option<String> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::Json => return type_not_support,
            oracle::sql_type::OracleType::Xml => return type_not_support,
            oracle::sql_type::OracleType::Int64 => {
                let x: Option<i64> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
            oracle::sql_type::OracleType::UInt64 => {
                let x: Option<u64> = r.get(c)?;
                x.map(serde_json::Value::from)
            }
        };
        values.insert(
            col_info.name().to_string(),
            v.unwrap_or(serde_json::Value::Null),
        );
    }
    anyhow::Ok(values)
}

impl OracleDB {
    pub(crate) async fn create_view_sql_impl(
        &self, view_name: &str, materialized: bool,
    ) -> anyhow::Result<String> {
        let sql = if materialized {
            format!(
                "SELECT DBMS_METADATA.GET_DDL('MATERIALIZED_VIEW', '{view_name}') AS create_statement FROM dual"
            )
        } else {
            format!("SELECT text FROM user_views WHERE view_name = '{view_name}'")
        };
        log::info!("sql:{}", sql);
        let row = self.query_row(&sql).await?;
        let s: String = row.get(0)?;
        anyhow::Ok(s)
    }
    pub(crate) async fn create_table_sql_impl(&self, table_name: &str) -> anyhow::Result<String> {
        let sql = format!("SELECT DBMS_METADATA.GET_DDL('TABLE', '{table_name}')  FROM dual");
        log::info!("sql:{}", sql);
        let row = self.query_row(&sql).await?;
        let s: String = row.get(0)?;
        anyhow::Ok(s)
    }
}

impl OracleDB {
    pub(crate) async fn create_index_sql(
        &self, name: &str, schema: &str,
    ) -> anyhow::Result<String> {
        let sql = format!("SELECT DBMS_METADATA.GET_DDL('INDEX', '{name}', '{schema}') FROM dual;");
        log::info!("sql:{}", sql);
        let row = self.query_row(&sql).await?;
        let s: String = row.get(0)?;
        anyhow::Ok(s)
    }

    async fn query_index_unique(&self, name: &str, table_name: &str) -> anyhow::Result<bool> {
        let sql = format!(
            r#"
        SELECT
            i.uniqueness
        FROM
            user_indexes i
            JOIN user_ind_columns ic ON i.index_name = ic.index_name
        WHERE
            ic.table_name = '{table_name}'
            AND i.index_name = '{name}'
        GROUP BY
            i.index_name,
            i.uniqueness
    "#
        );
        log::info!("sql:{}", sql);
        let row = self.query_row(&sql).await?;
        let u: String = row.get(0)?;
        anyhow::Ok(u == "UNIQUE")
    }
}

impl OracleDB {
    async fn query_index_primary(&self, name: &str, table_name: &str) -> anyhow::Result<bool> {
        let sql = format!(
            r#"
        SELECT
            count(*)
        FROM
            user_constraints
        WHERE
            TABLE_NAME = '{table_name}'
            AND CONSTRAINT_NAME = '{name}'
            AND constraint_type = 'P'
    "#
        );
        log::info!("sql:{}", sql);
        let row = self.query_row(&sql).await?;
        let u: i64 = row.get(0)?;
        anyhow::Ok(u > 0)
    }
}

impl OracleDB {
    async fn export_table_as_sql(
        &self, table_name: &str, para: Option<ShowTableParameter>, include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let cols = SchemaTrait::show_table_columns(self, table_name).await?;
        let ss = SchemaTrait::show_create_table_sql(self, table_name).await?;
        let table_name = table_name.to_owned();
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let sql = if let Some(para) = para {
                let ShowTableParameter {
                    where_str,
                    order_by,
                    page,
                    page_size,
                    ..
                } = &para;
                gen_show_sql(
                    &table_name,
                    this.backend_name().format_fields(Some(&cols)).into(),
                    where_str.as_deref(),
                    order_by.as_deref(),
                    *page,
                    *page_size,
                )
            } else {
                format!(r#"SELECT * from "{}"."{}""#, this.schema_str(), table_name)
            };
            log::info!("sql:{}", sql);
            if include_table_schema {
                sender.blocking_send(ss.create_table.clone().into())?;
                sender.blocking_send(";\n".into())?;
                for index in ss.indexes.into_iter() {
                    sender.blocking_send(index.into())?;
                    sender.blocking_send(";\n".into())?;
                }
            }
            let rows = conn.query(sql.as_str(), &[])?;
            for row in rows {
                let row = row?;
                let data = encode_row(row)?;
                let mut m = HashMap::new();
                for (d, f) in data.into_iter().zip(cols.iter()) {
                    m.insert(f.field.clone(), d);
                }
                let insert_sql = this.insert_sql(&table_name, &cols, &m)?;
                sender.blocking_send(insert_sql.into())?;
                sender.blocking_send(";\n".into())?;
            }
            anyhow::Ok(())
        })
        .await?
    }

    fn get_conn_by_query_id(
        &self, query_id: Option<&str>,
    ) -> anyhow::Result<Arc<Mutex<oracle::Connection>>> {
        match query_id {
            Some(query_id) => match self.query_connection.get(&query_id.to_string()) {
                Some(x) => anyhow::Ok(x.clone()),
                None => anyhow::bail!("query_id not found"),
            },
            None => anyhow::Ok(Arc::new(Mutex::new(self.get_conn()?))),
        }
    }
}

impl OracleDB {
    fn schema_str(&self) -> &str {
        self.config.schema.as_ref().expect("must be some").as_str()
    }
}

#[async_trait::async_trait]
impl CheckConn for OracleDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        let sql = "SELECT 1 FROM DUAL";
        let _ = self.query_row(sql).await?;
        anyhow::Ok(ins.elapsed())
    }
}

impl OracleDB {
    pub(crate) async fn query_size(
        &self, names: &[String],
    ) -> anyhow::Result<HashMap<String, Option<i64>>> {
        let sql = format!(
            r#"
            SELECT
            segment_name,
            bytes
        FROM
            user_segments
        WHERE
            segment_name in ('{}') -- 替换为你的表名
            
        "#,
            names.join("','")
        );
        log::info!("sql:{}", sql);
        let rows = self.query(sql).await?;
        let mut ret = HashMap::new();
        for row in rows {
            let name: String = row.get(0)?;
            let size: Option<i64> = row.get(1)?;
            ret.insert(name, size);
        }
        anyhow::Ok(ret)
    }

    pub(crate) async fn list_table_impl(&self, schema: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"SELECT TABLE_NAME FROM DBA_TABLES WHERE OWNER = '{}'  ORDER BY TABLE_NAME "#,
            schema
        );
        log::info!("sql:{}", sql);
        let result = self.query(sql).await?;
        let mut ret = Vec::new();
        for r in result {
            let name: String = r.get(0)?;
            ret.push(name);
        }
        let sizes = self.query_size(&ret[..]).await?;

        anyhow::Ok(
            ret.into_iter()
                .map(|x| ListTable {
                    size: sizes.get(&x).and_then(|x| (*x).map(|x| x as u64)),
                    name: x,
                })
                .collect(),
        )
    }

    pub(crate) async fn list_view_impl(&self, schema: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"
        SELECT VIEW_NAME   FROM all_views WHERE OWNER  = '{}'
            "#,
            schema
        );
        log::info!("sql:{}", sql);
        let rows = self.query(sql).await?;
        let mut ret = Vec::new();
        for r in rows {
            let name: String = r.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }
}
impl ReadOnlyTrait for OracleDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

impl OracleDB {
    pub(crate) async fn execute(&self, sql: impl Into<String>) -> anyhow::Result<Statement> {
        let sql = sql.into();
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let r = conn.execute(&sql, &[])?;
            anyhow::Ok(r)
        })
        .await?
    }

    pub(crate) async fn query(&self, sql: impl Into<String>) -> anyhow::Result<Vec<Row>> {
        let sql = sql.into();
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let rows = conn.query(&sql, &[])?;
            let mut r = vec![];
            for row in rows {
                let row = row?;
                r.push(row);
            }
            anyhow::Ok(r)
        })
        .await?
    }
    pub(crate) async fn query_row(&self, sql: impl Into<String>) -> anyhow::Result<Row> {
        let sql = sql.into();
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let r = conn.query_row(&sql, &[])?;
            anyhow::Ok(r)
        })
        .await?
    }
}

#[async_trait::async_trait]
impl SchemaTrait for OracleDB {
    fn list_sql_current_values(&self) -> &'static [&'static str] {
        <Self as SqlBuilder>::get_current_values(self)
    }

    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            "SELECT MVIEW_NAME FROM USER_MVIEWS  WHERE OWNER='{}' ",
            self.schema_str()
        );
        log::info!("sql:{}", sql);
        let rows = self.query(sql).await?;
        let mut ret = Vec::new();
        for row in rows {
            let name: String = row.get(0)?;
            ret.push(name);
        }
        let sizes = self.query_size(&ret[..]).await?;
        anyhow::Ok(
            ret.into_iter()
                .map(|x| ListTable {
                    size: sizes.get(&x).and_then(|x| (*x).map(|x| x as u64)),
                    name: x,
                })
                .collect(),
        )
    }

    async fn drop_materialized_view(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP MATERIALIZED VIEW "{name}""#);
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        let sql = format!(
            r#"
        SELECT column_name, data_type
            FROM all_tab_columns
            WHERE table_name = '{view_name}'  AND OWNER = '{}'  ORDER BY column_name 
        "#,
            self.schema_str()
        );
        log::info!("sql:{}", sql);
        let rows = self.query(sql).await?;

        let mut ret = Vec::new();

        for row in rows {
            let name: String = row.get(0)?;
            let ty: String = row.get(1)?;
            ret.push(Field::from_name_type(name, ty));
        }
        anyhow::Ok(ret)
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        if self.table_exists(dup).await? {
            anyhow::bail!(format!("table '{}' already exists", dup));
        }
        let sql = format!(r#"CREATE TABLE "{dup}" AS SELECT * FROM "{table}""#);
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let sql = format!(r#"RENAME "{old}" TO "{new}""#);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn show_create_table_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let create_table = self.create_table_sql_impl(table_name).await?;
        let indexes = self.show_indexes(table_name).await?;
        let mut indexes_sql = Vec::new();
        for i in indexes.iter() {
            let s = self
                .create_index_sql(i.name.as_ref().option_must_be_some()?, self.schema_str())
                .await?;
            indexes_sql.push(s);
        }
        anyhow::Ok(CreateTableAndIndexSql {
            create_table,
            indexes: indexes_sql,
        })
    }

    async fn show_view_sql(&self, view_name: &str, materialized: bool) -> anyhow::Result<String> {
        anyhow::Ok(self.create_view_sql_impl(view_name, materialized).await?)
    }

    async fn export_table_or_view(
        &self, name: &str, para: Option<ShowTableParameter>, kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::Sql {
                include_table_schema,
            } => {
                self.export_table_as_sql(name, para, include_table_schema, sender)
                    .await
            }
            _ => {
                let sql = if let Some(para) = para {
                    let ShowTableParameter {
                        where_str,
                        order_by,
                        page,
                        page_size,
                        ..
                    } = &para;
                    gen_show_sql(
                        name,
                        None,
                        where_str.as_deref(),
                        order_by.as_deref(),
                        *page,
                        *page_size,
                    )
                } else {
                    format!("SELECT * from {}", name)
                };
                self.export_query_result(None, &sql, kind, sender).await
            }
        }
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let sql = format!(
            "SELECT COLUMN_NAME, DATA_TYPE , NULLABLE ,DATA_DEFAULT FROM DBA_TAB_COLUMNS WHERE OWNER = '{}' AND TABLE_NAME = '{table_name}'",
            self.schema_str(),
        );
        log::info!("sql:{}", sql);
        let mut ret = Vec::new();
        let rows = self.query(sql).await?;
        for r in rows {
            let name: String = r.get(0)?;
            let data_type: String = r.get(1)?;
            let nullable: String = r.get(2)?;
            let value_default: Option<String> = r.get(3)?;
            let nullable = nullable == "Y";
            ret.push(Field::new(
                name,
                data_type,
                nullable,
                value_default,
                false,
                false,
            ));
        }
        let names = {
            let sql = format!(
                "SELECT COLUMN_NAME FROM user_cons_columns where  OWNER='{}' and TABLE_NAME='{}' and CONSTRAINT_NAME='{}_PK'",
                self.schema_str(),
                table_name,
                table_name
            );
            log::info!("sql:{}", sql);
            let rows = self.query(sql).await?;
            let mut names = HashSet::new();
            for r in rows {
                let name: String = r.get(0)?;
                names.insert(name);
            }
            names
        };
        for x in ret.iter_mut() {
            if names.contains(&x.field) {
                x.is_primary = true;
            }
        }
        anyhow::Ok(ret)
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self)) as Box<dyn SchemaTrait>
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.schema_str()).await
    }

    async fn create_index(&self, table: &str, req: &CreateIndexReq) -> anyhow::Result<()> {
        let sql = req.common_sql(table, true, self.backend_name().quote_style())?;
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn rename_index(
        &self, _table_name: &str, oldname: &str, newname: &str,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"ALTER INDEX "{oldname}" RENAME TO "{newname}""#);
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<()> {
        let sql = format!("DROP INDEX {index}");
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        let sql = format!(r#"ALTER TABLE "{table}" DROP COLUMN "{col}""#);
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        LexerCompletionTrait::get_types(self).await
    }

    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        if old_name != new_name {
            let sql =
                format!(r#"ALTER TABLE "{table}" RENAME COLUMN "{old_name}" TO "{new_name}""#);
            log::info!("sql:{}", sql);
            self.execute(sql).await?;
        }
        if let Some(typ) = typ {
            let sql = format!(r#"ALTER TABLE {table} MODIFY "{new_name}" {}"#, typ.typ);
            self.execute(sql).await?;

            if let Some(null) = typ.nullable.as_ref() {
                match null {
                    crate::db::SqlNullAble::Null | crate::db::SqlNullAble::NotNull => {
                        let sql = format!(r#"ALTER TABLE "{table}" MODIFY "{new_name}" {}"#, null);
                        log::info!("sql:{}", sql);
                        self.execute(sql).await?;
                    }
                    _ => {}
                }
            }

            if let Some(dv) = typ.default_value.as_ref() {
                let sql = format!(
                    r#"ALTER TABLE "{table}" MODIFY "{new_name}" DEFAULT {}"#,
                    self.put_value_in_sql(&typ.typ, Some(dv.as_str()))?
                );
                log::info!("sql:{}", sql);
                self.execute(sql).await?;
            }
        }
        anyhow::Ok(())
    }

    async fn add_table_column(
        &self, table: &str, name: &str, typ: ColumnType,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"ALTER TABLE "{table}" ADD "{name}"  {}  {} {}"#,
            typ.typ,
            typ.nullable.unwrap_or_default(),
            if let Some(dv) = typ.default_value.as_ref() {
                format!(
                    "DEFAULT {}",
                    self.put_value_in_sql(&typ.typ, Some(dv.as_str()))?
                )
            } else {
                "".to_owned()
            }
        );
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn show_table(
        &self, table_name: &str, para: ShowTableParameter, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let ShowTableParameter {
            where_str,
            order_by,
            page,
            page_size,
            ..
        } = &para;
        let cols = self.show_table_columns(table_name).await?;
        let sql = gen_show_sql(
            table_name,
            Some(self.backend_name().format_fields(Some(&cols))),
            where_str.as_deref(),
            order_by.as_deref(),
            *page,
            *page_size,
        );
        log::info!("query sql:{}", sql);
        emit.send("trying to query total amount.".into())?;
        let total = self.query_count(table_name, where_str.as_deref()).await?;
        emit.send(
            QueryResp {
                total: total as usize,
                sql: sql.clone().into(),
                fields: cols,
                ..Default::default()
            }
            .into(),
        )?;
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let stream = conn.query(&sql, &[])?;
            for row in stream {
                let row = row?;
                let d = encode_row(row)?;
                emit.send(d.into())?;
            }
            anyhow::Ok(())
        })
        .await?
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let sql = format!(
            r#"
    SELECT 
        index_name,
        column_name
    FROM 
        user_ind_columns
    WHERE 
        table_name = '{table_name}'
        "#
        );
        log::info!("sql:{}", sql);
        let rows = self.query(sql).await?;
        let mut ret: HashMap<String, Index> = HashMap::new();
        for r in rows {
            let name: String = r.get(0)?;
            let col_name = r.get(1)?;
            if let Some(index) = ret.get_mut(&name) {
                index.columns.push(col_name);
            } else {
                let primary = self.query_index_primary(&name, table_name).await?;
                let unique = if primary {
                    false
                } else {
                    self.query_index_unique(&name, table_name).await?
                };
                ret.insert(
                    name.clone(),
                    Index {
                        name: Some(name),
                        kind: if primary {
                            IndexKind::Primary
                        } else if unique {
                            IndexKind::Unique
                        } else {
                            IndexKind::Index
                        },
                        columns: vec![col_name],
                    },
                );
            }
        }
        anyhow::Ok(ret.into_values().collect())
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"TRUNCATE TABLE  "{table_name}""#);
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_view_impl(self.schema_str()).await
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP VIEW "{view_name}""#);
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        let sql = if cascade {
            format!(r#"DROP TABLE "{table}" CASCADE CONSTRAINTS"#)
        } else {
            format!(r#"DROP TABLE "{table}""#)
        };
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn show_view(
        &self, view_name: &str, para: ShowTableParameter, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let ShowTableParameter {
            where_str,
            order_by,
            page,
            page_size,
            ..
        } = &para;
        let sql = gen_show_sql(
            view_name,
            None,
            where_str.as_deref(),
            order_by.as_deref(),
            *page,
            *page_size,
        );
        log::info!("sql:{}", sql);
        emit.send("trying to query total amount.".into())?;
        let total = self.query_count(view_name, where_str.as_deref()).await?;
        log::info!("query sql:{}", sql);
        emit.send("trying to query data.".into())?;
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let stream = conn.query(&sql, &[])?;
            let mut first = true;
            for row in stream {
                let row = row?;
                if first {
                    first = false;
                    emit.send(
                        QueryResp {
                            fields: infer_schema_by_row(&row),
                            total: total as usize,
                            sql: sql.clone().into(),
                            ..Default::default()
                        }
                        .into(),
                    )?;
                }
                let d = encode_row(row)?;
                emit.send(d.into())?;
            }
            Ok(())
        })
        .await?
    }

    async fn list_trigger(&self, table_name: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
            SELECT trigger_name
                FROM USER_TRIGGERS
            WHERE table_name = '{table_name}'
        "#
        );
        log::info!("sql:{}", sql);
        let mut ret = Vec::new();
        let rows = self.query(sql).await?;
        for row in rows {
            let x: String = row.get(0)?;
            ret.push(NamedStaff::from(x));
        }
        anyhow::Ok(ret)
    }

    async fn show_trigger(&self, table: &str, name: &str) -> anyhow::Result<TriggerDetail> {
        let sql = format!(
            r#"
        SELECT trigger_type, triggering_event, status ,
            dbms_metadata.get_ddl('TRIGGER', trigger_name) AS trigger_ddl
            FROM USER_TRIGGERS
            WHERE table_name = '{table}' and TRIGGER_NAME='{name}'
        "#
        );
        log::info!("sql:{}", sql);
        let row = self.query_row(&sql).await?;
        let trigger_type: String = row.get(0)?;
        let event: String = row.get(1)?;
        let status: String = row.get(2)?;
        let ddl: String = row.get(3)?;

        anyhow::Ok(TriggerDetail {
            name: name.to_owned(),
            timing: TriggerTiming::String(trigger_type),
            event: TriggerEvent::try_from(event.as_str())?,
            enable: Some(TriggerEnable::Bool {
                enable: status == "ENABLED",
            }),
            sql: ddl,
            metas: vec![],
        })
    }

    async fn drop_trigger(&self, _table: &str, name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP TRIGGER "{name}""#);
        log::info!("sql:{}", sql);

        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn disable_trigger(&self, _table: &str, name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"ALTER TRIGGER "{name}" DISABLE"#);
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn enable_trigger(&self, _table: &str, name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"ALTER TRIGGER "{name}" ENABLE"#);
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
            SELECT object_name 
            FROM all_objects 
            WHERE object_type = 'FUNCTION' 
            AND owner = '{}'
        "#,
            self.schema_str()
        );
        log::info!("sql:{}", sql);
        let mut ret = Vec::new();
        for row in self.query(sql).await? {
            let name: String = row.get(0)?;
            ret.push(NamedStaff::from(name));
        }
        anyhow::Ok(ret)
    }

    async fn show_udf(
        &self, name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<DefAndMetas> {
        let sql = self.get_user_source(name).await?;
        log::info!("sql:{}", sql);
        anyhow::Ok(DefAndMetas {
            def: sql,
            metas: vec![],
        })
    }

    async fn drop_udf(
        &self, name: &str, _extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"DROP FUNCTION "{name}""#);
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_procedure_impl(self.schema_str()).await
    }

    async fn show_procedure(
        &self, name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<ProcedureDetail> {
        let sql = self.get_user_source(name).await?;
        log::info!("sql:{}", sql);
        anyhow::Ok(ProcedureDetail {
            def: sql,
            metas: vec![],
        })
    }

    async fn drop_procedure(
        &self, name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"DROP PROCEDURE "{name}""#);
        log::info!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }
}

impl OracleDB {
    pub(crate) async fn list_procedure_impl(
        &self, schema: &str,
    ) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
                SELECT object_name 
                FROM all_objects 
                WHERE object_type = 'PROCEDURE' 
                AND owner = '{}'
        "#,
            schema
        );
        log::info!("sql:{}", sql);
        let rows = self.query(sql).await?;
        let mut ret = Vec::new();
        for row in rows {
            let name: String = row.get(0)?;
            ret.push(NamedStaff {
                name,
                metas: vec![],
                parameters: None,
                cassandra_parameters: None,
            });
        }
        anyhow::Ok(ret)
    }
}
/// Oracle 里可直接写在 SQL 语句中的 CURRENT_* 无括号字面量
pub const ORACLE_CURRENT_LITERALS: &[&str] = &[
    "CURRENT_DATE", // 2025-08-25 14:36:07   （会话时区）
    "CURRENT_TIMESTAMP", /*  2025-08-25 14:36:07.123456   （会话时区，带微秒）
                     *   "CURRENT_TIMESTAMP(n)", //  同上，精度可调；裸写 CURRENT_TIMESTAMP
                     * 默认为 6 */
];

impl SqlBuilder for OracleDB {
    fn get_schema(&self) -> Option<&str> {
        self.config.schema.as_deref()
    }

    fn get_current_values(&self) -> &'static [&'static str] {
        ORACLE_CURRENT_LITERALS
    }

    /// format a value make it suitable for insert into sql.
    /// this method current use in `insert_sql` etc.
    fn put_value_in_sql(&self, typ: &str, value: Option<&str>) -> anyhow::Result<String> {
        if self.hit_current_value(value) {
            return anyhow::Result::Ok(value.option_must_be_some()?.to_string());
        }
        put_value_in_sql(typ, value)
    }
}
impl OracleDB {
    async fn get_user_source(&self, name: &str) -> anyhow::Result<String> {
        let sql = format!(
            r#"
            SELECT text
                FROM user_source
                WHERE name = '{name}'
                ORDER BY line
        "#
        );
        log::info!("sql:{}", sql);
        let rows = self.query(sql).await?;

        let mut sql = "".to_owned();
        for row in rows {
            let t: String = row.get(0)?;
            sql.push_str(t.as_str());
            sql.push('\n');
        }
        anyhow::Ok(sql)
    }
}

impl GetBackendKind for OracleDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::Oracle
    }
}

#[async_trait::async_trait]
impl DBTrait for OracleDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let sql = r#"SELECT USERNAME FROM DBA_USERS"#;
        let rows = self.query(sql).await?;
        let mut ret = Vec::new();
        for r in rows {
            let db: String = r.get(0)?;
            ret.push(db);
        }
        ret.sort();
        anyhow::Ok(ret)
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut x = Clone::clone(self);
        x.config = {
            let mut config = Clone::clone(self.config.as_ref());
            config.schema = Some(name.to_string());
            Arc::new(config)
        };
        x.query_connection = Default::default();
        anyhow::Ok(Arc::new(x))
    }

    async fn create_schema(
        &self, _name: &str, _extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.backend_name().not_support_feature()
    }

    async fn drop_schema(&self, _name: &str) -> anyhow::Result<()> {
        self.backend_name().not_support_feature()
    }
}

impl OracleDB {
    pub(crate) fn get_conn(&self) -> anyhow::Result<oracle::Connection> {
        let mut conn: oracle::Connection = match self.config.conn_timeout {
            Some(timeout) => {
                let this = Clone::clone(self);
                run_with_timeout(move || this.pool.get(), Duration::from(timeout))??
            }
            None => self.pool.get()?,
        };
        if let Some(db_name) = &self.config.schema {
            let sql = format!("ALTER SESSION SET CURRENT_SCHEMA={db_name}");
            log::info!("sql:{}", sql);
            conn.execute(&sql, &[])?;
        }
        conn.set_autocommit(QUERY_AUTO_COMMIT);
        anyhow::Ok(conn)
    }
}

// docker container create    -it    --name oracle-test  -p 1521:1521   -e
// ORACLE_PWD=welcome123   container-registry.oracle.com/database/express:latest
#[cfg(test)]
pub(crate) fn make_test_object(schema: Option<String>) -> OracleDB {
    OracleDB::new(
        OracleDBConfig {
            conn_timeout: Some(5000.into()),
            username: "system".to_string(),
            password: "welcome123".to_string(),
            schema,
            read_only: false,
            host: crate::TEST_HOST.to_string(),
            port: 1521,
            db: "XE".to_string(),
        },
        Some(2),
    )
    .unwrap()
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        db::{CreateIndexColumn, CreateTableReq, CreateTableReqColumn, SqlNullAble},
        enable_test_log,
    };
    use tokio::sync::Mutex;

    // when I test via `cargo test`
    // sometimes I have this error
    // ERROR - Transaction (Process ID 76) was deadlocked on lock resources with
    // another process and has been chosen as the deadlock victim. Rerun the
    // transaction. code=1205 maybe I need mutex to run one test a time.
    fn test_lock() -> &'static Mutex<()> {
        static MUTEX: Mutex<()> = Mutex::const_new(());
        &MUTEX
    }

    #[tokio::test]
    async fn test_server_metas() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(None);
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }
    #[tokio::test]
    async fn test_procedure() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let udf_name = "DO_NOTHING_P";
        let _ = db.drop_procedure(udf_name, None).await;

        db.execute_sql(
            query_id,
            r#"
            CREATE OR REPLACE PROCEDURE DO_NOTHING_P AS
            BEGIN
                -- 这里什么也不做
                NULL; -- NULL 是一个空操作，表示不做任何事情
            END;
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let udfs = db.list_procedure().await.unwrap();
        for udf in udfs.iter() {
            if udf.name != udf_name {
                continue;
            }
            let details = db.show_procedure(&udf.name, None).await.unwrap();
            println!("detail:{:?}", details)
        }
    }

    #[tokio::test]
    async fn test_udf() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let udf_name = "DO_NOTHING";
        let _ = db.drop_udf(udf_name, None).await;
        db.execute_sql(
            query_id,
            r#"
            CREATE OR REPLACE FUNCTION DO_NOTHING(input_string VARCHAR2) RETURN VARCHAR2 IS
            BEGIN
                NULL; -- 什么都不做
                RETURN 'Nothing done'; -- 可选返回值
            END do_nothing;
            /
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
            CREATE OR REPLACE FUNCTION DO_NOTHING RETURN VARCHAR2 IS
            BEGIN
                NULL; -- 什么都不做
                RETURN 'Nothing done'; -- 可选返回值
            END do_nothing;
            /
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let udfs = db.list_udf().await.unwrap();
        for udf in udfs.iter() {
            if udf.name != udf_name {
                continue;
            }
            let details = db.show_udf(&udf.name, None).await.unwrap();
            println!("detail:{:?}", details)
        }
    }
    // cargo test --package dbnexus-db --lib -- oracle::tests::test_trigger --exact
    // --show-output --nocapture
    #[tokio::test]
    async fn test_trigger() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let table_name = "TEST_TRIGGER";
        let _ = db.drop_table(table_name, false).await;

        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE TEST_TRIGGER (
                    id NUMBER GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
                    name VARCHAR2(100),
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )"#,
        )
        .await
        .unwrap()
        .unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE OR REPLACE TRIGGER  TRG_NO_ACTION
            AFTER INSERT
            ON test_trigger
            FOR EACH ROW
            BEGIN
                NULL; -- 什么都不做
            END;
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let triggers = db.list_trigger(table_name).await.unwrap();
        for tri in triggers.iter() {
            let show_trigger = db.show_trigger(table_name, &tri.name).await.unwrap();
            println!("trigger detail:{:?}", show_trigger);
            db.disable_trigger(table_name, &tri.name).await.unwrap();
            db.enable_trigger(table_name, &tri.name).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_rename() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("TEST_RENAME", false).await;
        let _ = db.drop_table("TEST_RENAME2", false).await;
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE TEST_RENAME (
                    ID NUMBER PRIMARY KEY,
                    NAME VARCHAR2(255) NOT NULL,
                    CREATED_AT TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )
                "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.rename_table("TEST_RENAME", "TEST_RENAME2")
            .await
            .unwrap();

        db.show_table_collect(
            "TEST_RENAME2",
            ShowTableParameter::new_stupid_test("id asc"),
        )
        .await
        .unwrap();
    }

    #[tokio::test]

    async fn test_duplicate() {
        let _guard = test_lock().lock().await;
        enable_test_log();

        let db = make_test_object(Some("SYSTEM".to_string()));

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("TEST_DUPLICATE", false).await;
        let _ = db.drop_table("TEST_DUPLICATE2", false).await;

        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE TEST_DUPLICATE (
                    ID NUMBER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                    NAME VARCHAR2(100) NOT NULL
                )


                "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
    
                INSERT INTO TEST_DUPLICATE (NAME) VALUES ('xxxyyy1')
            "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.duplicate_table("TEST_DUPLICATE", "TEST_DUPLICATE2")
            .await
            .unwrap();

        let resp = db
            .show_table_collect(
                "TEST_DUPLICATE2",
                ShowTableParameter::new_stupid_test("id asc"),
            )
            .await
            .unwrap();
        assert_eq!(resp.data.len(), 1);
    }
    #[tokio::test]
    async fn test_check_conn() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(None);
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }
    #[tokio::test]
    async fn test_view() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let db = db;
        let _ = db.drop_table("TEST_VIEW_TABLE", false).await;
        let _ = db.drop_view("TEST_VIEW").await;

        let ss = [
            r#"
        CREATE TABLE TEST_VIEW_TABLE (
            product_id   NUMBER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            product_name VARCHAR2(50) NOT NULL,
            category     VARCHAR2(30) NOT NULL,
            price        NUMBER(8,2)  NOT NULL
        )"#,
            "INSERT INTO TEST_VIEW_TABLE (product_name, category, price)
VALUES ('iPhone 13', 'Electronics', 799.99)",
            "INSERT INTO
TEST_VIEW_TABLE (product_name, category, price) VALUES ('Samsung Galaxy S21',
'Electronics', 699.99)",
            r#"
            CREATE VIEW TEST_VIEW AS
            SELECT product_id, product_name, category, price
            FROM  TEST_VIEW_TABLE"#,
        ];
        for s in ss.iter() {
            db.execute_sql(query_id, s).await.unwrap().unwrap();
        }
        let views = db.list_view().await.unwrap();
        println!("{:?}", views);

        let resp = db
            .show_view_collect("TEST_VIEW", ShowTableParameter::new_stupid_test("id asc"))
            .await;
        println!("show view:{:?}", resp);
        let sql = db.show_view_sql("TEST_VIEW", false).await.unwrap();
        println!("create view sql:{}", sql);
    }

    #[tokio::test]
    async fn test_materialized_view() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let db = db;
        let _ = db.drop_table("TEST_MATERIALIZED_VIEW_TABLE", false).await;
        let _ = db.drop_materialized_view("TEST_MATERIALIZED_VIEW").await;

        let ss = [
            r#"
        CREATE TABLE  TEST_MATERIALIZED_VIEW_TABLE (
            product_id   NUMBER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            product_name VARCHAR2(50) NOT NULL,
            category     VARCHAR2(30) NOT NULL,
            price        NUMBER(8,2)  NOT NULL
        )"#,
            "INSERT INTO TEST_MATERIALIZED_VIEW_TABLE (product_name, category, price)
VALUES ('iPhone 13', 'Electronics', 799.99)",
            "INSERT INTO
TEST_MATERIALIZED_VIEW_TABLE (product_name, category, price) VALUES ('Samsung Galaxy S21',
'Electronics', 699.99)",
            r#"
            CREATE MATERIALIZED VIEW TEST_MATERIALIZED_VIEW AS
            SELECT product_id, product_name, category, price
            FROM  TEST_MATERIALIZED_VIEW_TABLE"#,
        ];
        for s in ss.iter() {
            db.execute_sql(query_id, s).await.unwrap().unwrap();
        }
        let views = db.list_materialized_view().await.unwrap();
        println!("list_materialized_view:{:?}", views);

        let resp = db
            .show_view_collect(
                "TEST_MATERIALIZED_VIEW",
                ShowTableParameter::new_stupid_test("id asc"),
            )
            .await;
        println!("show view:{:?}", resp);
        let sql = db
            .show_view_sql("TEST_MATERIALIZED_VIEW", true)
            .await
            .unwrap();
        println!("create view sql:{}", sql);
    }

    #[tokio::test]
    async fn test_index() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let table_name = "TEST_INDEXES";

        let _ = db.drop_table(table_name, false).await;
        let req = CreateTableReq {
            name: table_name.to_string(),
            columns: vec![
                CreateTableReqColumn {
                    name: "ORDER_ID".to_string(),
                    data_type: "NUMBER(10)".into(),
                    auto_increment: false,
                    primary_key: true,
                    default_value: None,
                    nullable: SqlNullAble::NotNull,
                },
                CreateTableReqColumn {
                    name: "CUSTOMER_ID".to_string(),
                    data_type: "NUMBER(10)".into(),
                    auto_increment: false,
                    ..Default::default()
                },
                CreateTableReqColumn {
                    name: "ORDER_DATE".to_string(),
                    data_type: "DATE".into(),
                    auto_increment: false,
                    ..Default::default()
                },
            ],
            indexes: vec![],
        };

        db.create_table(&req).await.unwrap();
        {
            let sql = db.create_table_sql_string(&req).unwrap();
            println!("create table sql:{}", sql);
        }
        let index_name = format!("{}_CUSTOMER_ID_ORDER_DATE", table_name,);
        let index_name2 = format!("{}2", index_name);

        db.create_index(
            table_name,
            &CreateIndexReq {
                name: index_name.clone(),
                columns: vec![
                    CreateIndexColumn {
                        col: "CUSTOMER_ID".into(),
                        asc: Some(true),
                    },
                    CreateIndexColumn {
                        col: "ORDER_DATE".into(),
                        asc: None,
                    },
                ],
                kind: IndexKind::Unique,
            },
        )
        .await
        .unwrap();
        db.rename_index(table_name, &index_name, &index_name2)
            .await
            .unwrap();
        db.rename_index(table_name, &index_name2, &index_name)
            .await
            .unwrap();
        let indexes = db.show_indexes(table_name).await.unwrap();
        for index in indexes.iter() {
            println!("index:{:?}", index);
        }
    }

    #[tokio::test]
    async fn test_table_column() {
        enable_test_log();
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));

        let table_name = "TEST_COLUMN";
        let _ = db.drop_table(table_name, false).await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE TEST_COLUMN (
                    id NUMBER GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY  -- 主键，自增
                    
                )
                "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.add_table_column(
            table_name,
            "NAME",
            ColumnType {
                typ: "CHAR".to_string(),
                nullable: SqlNullAble::NotNull.into(),
                after: None,

                default_value: None,
            },
        )
        .await
        .unwrap();

        db.modify_table_column(
            table_name,
            "NAME",
            "NAME2",
            Some(ColumnType {
                typ: "NCHAR(100)".to_string(),
                nullable: SqlNullAble::Null.into(),
                after: None,
                default_value: Some("abc".to_string()),
            }),
        )
        .await
        .unwrap();

        db.drop_table_column(table_name, "NAME2").await.unwrap();
    }

    #[tokio::test]
    async fn test_multi_db_trait() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(None);
        let db = &db as &dyn DBTrait;
        let dbs = db.list_schema().await.unwrap();
        println!("dbs : {:?}", dbs);
        db.open_schema("SYSTEM").await.unwrap();
    }

    #[tokio::test]
    async fn test_show_tables() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("APPQOSSYS".to_string()));
        let db = db;
        let tables = db.list_table().await.unwrap();
        println!("tables : {:?}", tables);
        for t in tables.iter() {
            let cols = db.show_table_columns(&t.name).await.unwrap();
            println!("table:{} ,cols:{:?}", t.name, cols);
        }
    }

    #[tokio::test]
    async fn test_query_sql() {
        let _guard = test_lock().lock().await;
        let db: OracleDB = make_test_object(Some("SYSTEM".to_string()));

        // get query_uuid
        let query_uuid = "";
        db.open_connection(query_uuid).await.unwrap();
        // test auto commit
        db.set_auto_commit(query_uuid, false).await.unwrap();
        assert!(!db.get_auto_commit(query_uuid).await.unwrap());

        let sqls = [
            r#"
            CREATE TABLE TEST_QUERY (
                id NUMBER PRIMARY KEY,
                name VARCHAR2(50),
                age NUMBER
            )
        "#,
            "INSERT INTO TEST_QUERY (id, name, age) VALUES (1, 'Alice', 25)",
            "INSERT INTO TEST_QUERY (id, name, age) VALUES (2, 'Bob', 32)",
            "INSERT INTO TEST_QUERY (id, name, age) VALUES (3, 'Charlie',
18)",
        ];
        // test a rollback but we rollback nothings

        db.rollback(query_uuid).await.unwrap();

        let _ = db.drop_table("TEST_QUERY", false).await;
        for sql in sqls.iter() {
            db.execute_sql(query_uuid, sql).await.unwrap().unwrap();
        }

        db.commit(query_uuid).await.unwrap();

        let resp = db
            .show_table_collect("TEST_QUERY", ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }

    #[tokio::test]
    async fn test_type1() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let table_name = "TEST_TYPES1";
        let db = make_test_object(Some("SYSTEM".to_string()));
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let db = db;
        let _ = db.drop_table(table_name, false).await;
        db.execute_sql(
            query_id,
            format!(
                r#"
                CREATE TABLE {table_name} (
                    VARCHAR2_COL VARCHAR2(100),
                    NVARCHAR2_COL NVARCHAR2(100),
                    CHAR_COL CHAR(10),
                    NCHAR_COL NCHAR(10),
                    RAW_COL RAW(100)
                )
        "#
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert(
            "VARCHAR2_COL".to_string(),
            Some("这是一个 VARCHAR2 列的值".to_string()),
        );
        values.insert(
            "NVARCHAR2_COL".to_string(),
            Some("这是一个 NVARCHAR2 列的值".to_string()),
        );
        values.insert("CHAR_COL".to_string(), Some("测试char".to_string()));
        values.insert("NCHAR_COL".to_string(), Some("测试nchar".to_string()));
        values.insert("RAW_COL".to_string(), Some("0A0B0C0D0E0F".to_string()));

        db.table_insert(table_name, values).await.unwrap();

        let resp = db
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("VARCHAR2_COL asc"),
            )
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        {
            let e = db
                .export_table_to_string(
                    table_name,
                    crate::db::ExportKind::Csv {
                        include_header: false,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("export csv:{}", e);
            let e = db
                .export_table_to_string(
                    table_name,
                    crate::db::ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("export csv:{}", e);
            let e = db
                .export_table_to_string(table_name, crate::db::ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("export json:{}", e);
            let e = db
                .export_table_to_string(
                    table_name,
                    crate::db::ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("export sql:{}", e);
            let e = db
                .export_table_to_string(
                    table_name,
                    crate::db::ExportKind::Html {
                        include_header: true,
                    },
                )
                .await
                .unwrap();
            println!("export html:{}", e);
            let e = db
                .export_table_to_string(table_name, crate::db::ExportKind::MarkDown)
                .await
                .unwrap();
            println!("export markdown:{}", e);
        }
    }

    #[tokio::test]
    async fn test_type2() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let table_name = "TEST_TYPES2";
        let db = make_test_object(Some("SYSTEM".to_string()));
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let db = db;
        let _ = db.drop_table(table_name, false).await;

        db.execute_sql(
            query_id,
            format!(
                r#"
            CREATE TABLE {table_name} (
                BINARY_FLOAT BINARY_FLOAT,
                BINARY_DOUBLE BINARY_DOUBLE,
                NUM NUMBER,
                FLOAT_COL FLOAT
            )
        "#
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert("BINARY_FLOAT".to_string(), Some("3.14".to_string()));
        values.insert(
            "BINARY_DOUBLE".to_string(),
            Some("3.1415926535".to_string()),
        );
        values.insert("NUM".to_string(), Some("100".to_string()));
        values.insert("FLOAT_COL".to_string(), Some("2.71".to_string()));

        db.table_insert(table_name, values).await.unwrap();

        let resp = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("NUM asc"))
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        {
            let e = db
                .export_table_to_string(
                    table_name,
                    crate::db::ExportKind::Csv {
                        include_header: false,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("export csv:{}", e);
            let e = db
                .export_table_to_string(
                    table_name,
                    crate::db::ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("export csv:{}", e);
            let e = db
                .export_table_to_string(table_name, crate::db::ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("export json:{}", e);
            let e = db
                .export_table_to_string(
                    table_name,
                    crate::db::ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("export sql:{}", e);
        }
    }

    #[tokio::test]
    async fn test_type3() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let table_name = "TEST_TYPES3";
        let db = make_test_object(Some("SYSTEM".to_string()));
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table(table_name, false).await;
        db.execute_sql(
            query_id,
            format!(
                r#"
            CREATE TABLE {table_name} (
                CLOB_COL CLOB,
                NCLOB_COL NCLOB,
                BLOB_COL BLOB
            )
        "#
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert(
            "CLOB_COL".to_string(),
            Some("This is a CLOB column.".to_string()),
        );
        values.insert(
            "NCLOB_COL".to_string(),
            Some("这是一个 NCLOB 列。".to_string()),
        );
        values.insert("BLOB_COL".to_string(), Some("010203".to_string()));

        db.table_insert(table_name, values).await.unwrap();

        let resp = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test(None))
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        {
            let e = db
                .export_table_to_string(
                    table_name,
                    crate::db::ExportKind::Csv {
                        include_header: false,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("export csv:{}", e);
            let e = db
                .export_table_to_string(
                    table_name,
                    crate::db::ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("export csv:{}", e);
            let e = db
                .export_table_to_string(table_name, crate::db::ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("export json:{}", e);
            let e = db
                .export_table_to_string(
                    table_name,
                    crate::db::ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("export sql:{}", e);
        }
    }

    // TODO create table sql not passed

    #[tokio::test]
    async fn test_type4() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let table_name = "TEST_TYPES4";
        let db = make_test_object(Some("SYSTEM".to_string()));
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table(table_name, false).await;
        db.execute_sql(
            query_id,
            format!(
                r#"
            CREATE TABLE {table_name} (
                COL1 LONG,
                COL2 NUMBER(19),
                COL3 NUMBER(19)
            )"#
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert("COL1".to_string(), Some("abc".to_string()));

        values.insert("COL2".to_string(), Some("123".to_string()));
        values.insert("COL3".to_string(), Some("456".to_string()));

        db.table_insert(table_name, values).await.unwrap();

        let resp = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test(None))
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }
}
