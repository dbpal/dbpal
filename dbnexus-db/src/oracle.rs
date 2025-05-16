/// notice oracle sql 不能带有分号
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;
use std::vec;

use dashmap::DashMap;
use oracle::pool::Pool;
use oracle::{Connection, Row};
use tokio::sync::Mutex;
use tokio::task::block_in_place;

use crate::MemoryLimit;
use crate::datasource_config::OracleDBConfig;
use crate::db::{
    BackendName, CheckConn, ColumnType, CreateSchemaExtra, CreateTableAndIndexSql, DBTrait,
    DefAndMetas, ExportKind, ExportQueryResult, Field, HumanSize, Index, IndexKind,
    ListMaterializedView, ListTable, NamedStaff, ProcedureDetail, QueryResp, QuerySql, QuoteStyle,
    ReadOnlyTrait, SchemaTrait, ServerMetas, ShowExtraArgs, SqlBuilder, SqlExecuteRet,
    TriggerDetail, TriggerEnable, TriggerEvent, TriggerTiming, UdfOrProcedureExtraParameter,
    not_support_features,
};
use crate::helper::{SqlKind, SqlWithKind, create_csv, is_dangerous_sql_by_parser, sql_to_sql_seq};
use crate::meta_value::MetaValue;
use crate::oracle_lang_server::ORACLE_TYPES;
use crate::query_log::{QueryLogEvent, QueryLogMap};

#[derive(Clone)]
pub struct OracleDB {
    pool: Pool,
    query_connections: Arc<DashMap<uuid::Uuid, Arc<Mutex<Connection>>>>,
    pub(crate) config: OracleDBConfig,
    query_logs: QueryLogMap,
}

#[async_trait::async_trait]
impl ServerMetas for OracleDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        block_in_place(|| -> anyhow::Result<Vec<MetaValue>> {
            let mut ret = Vec::new();
            let conn = self.get_conn()?;

            {
                let sql = "SELECT * FROM v$version";
                let row = conn.query_row(&sql, &[])?;
                let banner: String = row.get(0)?;
                let banner_full: String = row.get(1)?;
                let banner_legacy: String = row.get(2)?;
                ret.push(MetaValue::new("BANNER", banner));
                ret.push(MetaValue::new("BANNER_FULL", banner_full));
                ret.push(MetaValue::new("BANNER_LEGACY", banner_legacy));
            }
            // TODO more info.
            Ok(ret)
        })
    }
}
impl std::fmt::Debug for OracleDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "oracle@{:?}", self.config)
    }
}

impl OracleDB {
    pub fn new(config: OracleDBConfig, min_conn: Option<u32>) -> anyhow::Result<Self> {
        let pool = oracle::pool::PoolBuilder::new(
            &config.username,
            &config.password,
            format!("//{}:{}/{}", config.host, config.port, config.db),
        )
        .min_connections(min_conn.unwrap_or(10))
        .max_connections(100)
        .build()?;
        anyhow::Ok(Self {
            pool,
            query_connections: Default::default(),
            config,
            query_logs: Default::default(),
        })
    }
}

fn get_sql_seq(sql: &str) -> anyhow::Result<Vec<SqlWithKind>> {
    // TODO use oracle later when sqlparser supported
    let dialect = sqlparser::dialect::GenericDialect {};
    sql_to_sql_seq(
        sql,
        &dialect,
        |s| SqlKind::get_from_statement(s),
        |x| SqlKind::get_from_sql(x),
    )
}

impl OracleDB {
    async fn with_query_connection<R>(
        &self, id: uuid::Uuid, f: impl FnOnce(&mut Connection) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let conn = match self.query_connections.get(&id) {
            Some(x) => x.clone(),
            None => {
                anyhow::bail!(format!("id '{}' not found", id));
            }
        };
        let mut lock = conn.lock().await;
        block_in_place(|| f(&mut lock))
    }
}

const QUERY_AUTO_COMMIT: bool = true;
#[async_trait::async_trait]
impl ExportQueryResult for OracleDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }

    async fn export_query_result(
        &self, sql: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::CSV {
                include_header,
                delimiter,
            } => {
                self.export_query_result_as_csv(sql, include_header, delimiter, sender)
                    .await
            }
            ExportKind::Json { pretty } => {
                self.export_query_result_as_json(sql, pretty, sender).await
            }
            ExportKind::JsonLine => self.export_query_result_as_json_line(sql, sender).await,
            ExportKind::Sql {
                include_table_schema: _,
            } => ExportKind::cannot_query_result_export_as_sql(),
        }
    }
}

#[async_trait::async_trait]
impl QuerySql for OracleDB {
    async fn get_commit(&self, query_id: uuid::Uuid) -> anyhow::Result<bool> {
        self.with_query_connection(query_id, |x| -> anyhow::Result<bool> {
            anyhow::Ok(x.autocommit())
        })
        .await
    }

    async fn set_commit(&self, query_id: uuid::Uuid, commit: bool) -> anyhow::Result<()> {
        self.with_query_connection(query_id, |x| -> anyhow::Result<()> {
            x.set_autocommit(commit);
            anyhow::Ok(())
        })
        .await
    }

    async fn commit(&self, query_id: uuid::Uuid) -> anyhow::Result<()> {
        self.with_query_connection(query_id, |f| -> anyhow::Result<()> {
            f.commit()?;
            anyhow::Ok(())
        })
        .await
    }

    async fn rollback(&self, query_id: uuid::Uuid) -> anyhow::Result<()> {
        self.with_query_connection(query_id, |f| -> anyhow::Result<()> {
            f.rollback()?;
            anyhow::Ok(())
        })
        .await
    }

    fn get_query_log(&self, id: uuid::Uuid) -> anyhow::Result<Vec<QueryLogEvent>> {
        anyhow::Ok(self.query_logs.get(&id))
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        is_dangerous_sql_by_parser(sql, &sqlparser::dialect::GenericDialect {})
    }

    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        let mut conn = self.get_conn()?;
        conn.set_autocommit(QUERY_AUTO_COMMIT);
        self.query_connections
            .insert(id.clone(), Arc::new(Mutex::new(conn)));
        anyhow::Ok(id)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        if let Some((_, conn)) = self.query_connections.remove(&id) {
            tokio::spawn(async move { conn.lock().await.close() });
        }
        let mut conn = self.get_conn()?;
        conn.set_autocommit(QUERY_AUTO_COMMIT);
        self.query_connections
            .insert(id.clone(), Arc::new(Mutex::new(conn)));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.query_logs.remove(&id);
        if let Some((_, x)) = self.query_connections.remove(&id) {
            tokio::spawn(async move { x.lock().await.close() });
        }
        anyhow::Ok(())
    }

    async fn execute_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let mut query_log = self.query_logs.get_guard(query_id, sql);

        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut lock = conn.lock().await;
        let conn = lock.deref_mut();
        let ret = match block_in_place(|| conn.execute(sql, &[])) {
            Ok(_) => Ok(QueryResp::execute_ok(sql.to_owned())),
            Err(err) => Err(err.to_string()),
        };
        query_log.set_success(ret.is_ok());
        anyhow::Ok(ret)
    }

    async fn query_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let mut query_log = self.query_logs.get_guard(query_id, sql);
        let ss = match get_sql_seq(sql) {
            Ok(x) => x,
            Err(_err) => {
                vec![SqlWithKind {
                    sql: sql.to_string(),
                    kind: SqlKind::Query,
                }]
            }
        };

        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut lock = conn.lock().await;
        let conn = lock.deref_mut();

        let mut ret: Vec<Result<QueryResp, String>> = Vec::new();
        let mut ml = MemoryLimit::new();
        for seq in ss.iter() {
            ml.exceed_error()?;
            log::trace!("oracle query sql:{}", seq);
            match seq.kind {
                SqlKind::Query => {
                    match block_in_place(|| conn.query(&seq.sql, &[])) {
                        Ok(rows) => {
                            ml.exceed_error()?;
                            let rows = rows.into_iter().collect::<Vec<_>>();
                            let cols = infer_schema_by_result(&rows)?;
                            let data = encode_rows(rows)?;
                            ret.push(Ok(QueryResp::new(cols, 0, data, seq.sql.to_owned())?));
                        }
                        Err(err) => {
                            ret.push(Err(err.to_string()));
                        }
                    };
                }
                SqlKind::Execution => {
                    let stmt = block_in_place(|| conn.execute(&seq.sql, &[]));
                    match stmt {
                        Ok(x) => {
                            match x.row_count() {
                                Ok(x) => {
                                    ret.push(Ok(QueryResp::row_affected(x, seq.sql.to_owned())))
                                }
                                Err(err) => ret.push(Err(err.to_string())),
                            }
                        }
                        Err(err) => ret.push(Err(err.to_string())),
                    }
                }
                SqlKind::Begin | SqlKind::Commit | SqlKind::Rollback => {
                    anyhow::bail!("oracle doesn't work this way,this is should be unreachable");
                }
            }
        }
        query_log.set_success(ret.iter().all(|x| x.is_ok()));
        anyhow::Ok(ret)
    }
}

fn gen_select_sql(
    table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize,
) -> String {
    let mut sql = format!(r#"SELECT * FROM "{table_name}""#);
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
    fn query_count(conn: &Connection, table: &str, where_str: Option<&str>) -> anyhow::Result<i64> {
        let row = block_in_place(|| {
            conn.query_row(
                format!(
                    r#"SELECT count(*) FROM "{table}" {}"#,
                    match where_str {
                        Some(x) => format!("  WHERE {} ", x),
                        None => "".to_string(),
                    }
                )
                .as_str(),
                &[],
            )
        })?;
        let value: i64 = row.get(0)?;
        anyhow::Ok(value)
    }
}

fn infer_schema_by_result(rows: &Vec<oracle::Result<Row>>) -> anyhow::Result<Vec<Field>> {
    if rows.len() == 0 {
        return anyhow::Ok(vec![]);
    }

    let x = match &rows[0] {
        Ok(x) => x,
        Err(err) => anyhow::bail!(format!("{}", err.to_string())),
    };
    infer_schema_by_row(x)
}
fn infer_schema_by_row(x: &oracle::Row) -> anyhow::Result<Vec<Field>> {
    let mut ret = Vec::with_capacity(x.column_info().len());
    for c in x.column_info().iter() {
        ret.push(Field::from_name_type(
            c.name().to_string(),
            c.oracle_type().to_string(),
        ))
    }
    anyhow::Ok(ret)
}

/// some query will auto include a ROWNUM column
fn encode_rows(rows: Vec<oracle::Result<Row>>) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let mut data = vec![];
    for r in rows {
        let mut values = vec![];

        let r = r?;
        for c in 0..r.sql_values().len() {
            let col_info = &r.column_info()[c];
            let type_not_support = Err(anyhow::Error::msg(format!(
                "type '{}' not supported",
                col_info.oracle_type()
            ))) as anyhow::Result<Vec<Vec<Option<String>>>>;

            let v = match col_info.oracle_type() {
                oracle::sql_type::OracleType::Varchar2(_) => {
                    let x: Option<String> = r.get(c)?;
                    x
                }
                oracle::sql_type::OracleType::NVarchar2(_) => {
                    let x: Option<String> = r.get(c)?;
                    x
                }
                oracle::sql_type::OracleType::Char(_) => {
                    let x: Option<String> = r.get(c)?;
                    x
                }
                oracle::sql_type::OracleType::NChar(_) => {
                    let x: Option<String> = r.get(c)?;
                    x
                }
                oracle::sql_type::OracleType::Rowid => {
                    let x: Option<String> = r.get(c)?;
                    x
                }
                oracle::sql_type::OracleType::Raw(_) => {
                    let x: Option<String> = r.get(c)?;
                    x
                }
                oracle::sql_type::OracleType::BinaryFloat => {
                    let x: Option<f32> = r.get(c)?;
                    x.map(|x| x.to_string())
                }

                oracle::sql_type::OracleType::BinaryDouble => {
                    let x: Option<f64> = r.get(c)?;
                    x.map(|x| x.to_string())
                }
                oracle::sql_type::OracleType::Number(..) => {
                    let x: Option<f64> = r.get(c)?;
                    x.map(|x| x.to_string())
                }
                oracle::sql_type::OracleType::Float(_) => {
                    let x: Option<f64> = r.get(c)?;
                    x.map(|x| x.to_string())
                }
                oracle::sql_type::OracleType::Date => {
                    let x: Option<chrono::NaiveDate> = r.get(c)?;
                    x.map(|x| x.to_string())
                }
                oracle::sql_type::OracleType::Timestamp(_) => {
                    let x: Option<chrono::NaiveDateTime> = r.get(c)?;
                    x.map(|x| x.to_string())
                }
                oracle::sql_type::OracleType::TimestampTZ(_) => return type_not_support,
                oracle::sql_type::OracleType::TimestampLTZ(_) => return type_not_support,
                oracle::sql_type::OracleType::IntervalDS(..) => return type_not_support,
                oracle::sql_type::OracleType::IntervalYM(_) => return type_not_support,
                oracle::sql_type::OracleType::CLOB => {
                    let x: Option<String> = r.get(c)?;
                    x
                }
                oracle::sql_type::OracleType::NCLOB => {
                    let x: Option<String> = r.get(c)?;
                    x
                }
                oracle::sql_type::OracleType::BLOB => {
                    let x: Option<String> = r.get(c)?;
                    x
                }
                oracle::sql_type::OracleType::BFILE => return type_not_support,
                oracle::sql_type::OracleType::Boolean => {
                    let x: Option<bool> = r.get(c)?;
                    x.map(|x| x.to_string())
                }
                oracle::sql_type::OracleType::RefCursor => return type_not_support,
                oracle::sql_type::OracleType::Object(_) => return type_not_support,
                oracle::sql_type::OracleType::Long => {
                    let x: Option<String> = r.get(c)?;
                    x
                }
                oracle::sql_type::OracleType::LongRaw => {
                    let x: Option<String> = r.get(c)?;
                    x
                }
                oracle::sql_type::OracleType::Json => return type_not_support,
                oracle::sql_type::OracleType::Xml => return type_not_support,
                oracle::sql_type::OracleType::Int64 => {
                    let x: Option<i64> = r.get(c)?;
                    x.map(|x| x.to_string())
                }
                oracle::sql_type::OracleType::UInt64 => {
                    let x: Option<u64> = r.get(c)?;
                    x.map(|x| x.to_string())
                }
            };
            values.push(v);
        }
        data.push(values);
    }

    anyhow::Ok(data)
}

fn put_value_in_sql(typ: &str, value: Option<&str>) -> anyhow::Result<String> {
    let null_string = "NULL".to_string();
    let typ = typ.to_string().to_uppercase();

    match typ.as_str() {
        "VARCHAR2" => {
            anyhow::Ok(match value {
                Some(x) => format!("'{}'", x),
                None => null_string,
            })
        }
        _ if typ.starts_with("VARCHAR2") => {
            anyhow::Ok(match value {
                Some(x) => format!("'{}'", x),
                None => null_string,
            })
        }
        "NVARCHAR2" => {
            anyhow::Ok(match value {
                Some(x) => format!("N'{}'", x),
                None => null_string,
            })
        }
        _ if typ.starts_with("NVARCHAR2") => {
            anyhow::Ok(match value {
                Some(x) => format!("N'{}'", x),
                None => null_string,
            })
        }

        "CHAR" => {
            anyhow::Ok(match value {
                Some(x) => format!("'{}'", x),
                None => null_string,
            })
        }
        _ if typ.starts_with("CHAR") => {
            anyhow::Ok(match value {
                Some(x) => format!("'{}'", x),
                None => null_string,
            })
        }
        "NCHAR" => {
            anyhow::Ok(match value {
                Some(x) => format!("N'{}'", x),
                None => null_string,
            })
        }
        _ if typ.starts_with("NCHAR") => {
            anyhow::Ok(match value {
                Some(x) => format!("N'{}'", x),
                None => null_string,
            })
        }
        "RAW" => {
            anyhow::Ok(match value {
                Some(x) => format!("'{}'", x),
                None => null_string,
            })
        }
        _ if typ.starts_with("RAW") => {
            anyhow::Ok(match value {
                Some(x) => format!("'{}'", x),
                None => null_string,
            })
        }

        "BINARY_FLOAT" | "BINARY_DOUBLE" | "NUMBER" | "FLOAT" => {
            anyhow::Ok(match value {
                Some(x) => x.to_string(),
                None => null_string,
            })
        }

        _ if typ.starts_with("FLOAT") || typ.starts_with("NUMBER") => {
            anyhow::Ok(match value {
                Some(x) => x.to_string(),
                None => null_string,
            })
        }

        "CLOB" => {
            anyhow::Ok(match value {
                Some(x) => format!("'{}'", x),
                None => null_string,
            })
        }
        "NCLOB" => {
            anyhow::Ok(match value {
                Some(x) => format!("N'{}'", x),
                None => null_string,
            })
        }
        "BLOB" => {
            anyhow::Ok(match value {
                Some(x) => format!("'{}'", x),
                None => null_string,
            })
        }
        "LONG" => {
            anyhow::Ok(match value {
                Some(x) => format!("'{}'", x),
                None => null_string,
            })
        }
        "TIMESTAMP" => {
            anyhow::Ok(match value {
                Some(x) => format!("'{}'", x),
                None => null_string,
            })
        }
        _ if typ.starts_with("TIMESTAMP") => {
            anyhow::Ok(match value {
                Some(x) => format!("'{}'", x),
                None => null_string,
            })
        }
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

fn encode_rows_as_json(rows: Vec<oracle::Result<Row>>) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut datas = vec![];
    for r in rows {
        let r = r?;
        let values = encode_row_as_json(&r)?;
        datas.push(serde_json::Value::Object(values));
    }
    anyhow::Ok(datas)
}

fn create_view_sql(
    conn: &Connection, view_name: &str, materialized: bool,
) -> anyhow::Result<String> {
    let sql = if materialized {
        format!(
            "SELECT DBMS_METADATA.GET_DDL('MATERIALIZED_VIEW', '{view_name}') AS create_statement FROM dual"
        )
    } else {
        format!("SELECT text FROM user_views WHERE view_name = '{view_name}'")
    };
    log::info!("sql:{}", sql);
    let row = block_in_place(|| conn.query_row(&sql, &[]))?;
    let s: String = row.get(0)?;
    anyhow::Ok(s)
}
fn create_table_sql(conn: &Connection, table_name: &str) -> anyhow::Result<String> {
    let sql = format!("SELECT DBMS_METADATA.GET_DDL('TABLE', '{table_name}')  FROM dual");
    log::info!("sql:{}", sql);
    let row = block_in_place(|| conn.query_row(&sql, &[]))?;
    let s: String = row.get(0)?;
    anyhow::Ok(s)
}

fn create_index_sql(conn: &Connection, name: &str, schema: &str) -> anyhow::Result<String> {
    let sql = format!("SELECT DBMS_METADATA.GET_DDL('INDEX', '{name}', '{schema}') FROM dual;");
    log::info!("sql:{}", sql);
    let row = block_in_place(|| conn.query_row(&sql, &[]))?;
    let s: String = row.get(0)?;
    anyhow::Ok(s)
}

fn query_index_unique(conn: &Connection, name: &str, table_name: &str) -> anyhow::Result<bool> {
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
    let row = conn.query_row(&sql, &[])?;
    let u: String = row.get(0)?;
    anyhow::Ok(u == "UNIQUE")
}

fn query_index_primary(conn: &Connection, name: &str, table_name: &str) -> anyhow::Result<bool> {
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
    let row = conn.query_row(&sql, &[])?;
    let u: i64 = row.get(0)?;
    anyhow::Ok(u > 0)
}

impl OracleDB {
    async fn export_table_as_sql(
        &self, table_name: &str, include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            let ss = SchemaTrait::create_table_and_index_sql(self, table_name).await?;
            sender.send(Cow::Owned(ss.create_table.clone())).await?;
            sender.send(";\n".into()).await?;
            for index in ss.indexes.into_iter() {
                sender.send(Cow::Owned(index)).await?;
                sender.send(";\n".into()).await?;
            }
        }
        let conn = self.get_conn()?;
        let cols = SchemaTrait::show_table_columns(self, table_name).await?;
        let sql = format!("SELECT * from {}", table_name);
        log::info!("sql:{}", sql);
        let rows = block_in_place(|| conn.query(sql.as_str(), &[]))?;
        let rows = rows.into_iter().collect::<Vec<_>>();

        let data = encode_rows(rows)?;
        for d in data.into_iter() {
            let mut m = HashMap::new();
            for (d, f) in d.into_iter().zip(cols.iter()) {
                m.insert(f.field.clone(), d);
            }
            let insert_sql = (self).insert_sql(None, table_name, &cols, &m)?;
            sender.send(Cow::Owned(insert_sql)).await?;
            sender.send(";\n".into()).await?;
        }

        anyhow::Ok(())
    }

    async fn export_query_result_as_json_line(
        &self, sql: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn()?;

        let rows = block_in_place(|| conn.query(&sql, &[]))?;
        let rows = rows.into_iter().collect::<Vec<_>>();
        for r in rows {
            let r = r?;
            let json = encode_row_as_json(&r)?;
            let str = serde_json::to_string(&json)?;
            sender.send(Cow::Owned(str)).await?;
            sender.send("\n".into()).await?;
        }

        anyhow::Ok(())
    }

    async fn export_table_as_json_line(
        &self, table_name: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from {}", table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_json_line(&sql, sender).await
    }

    async fn export_query_result_as_json(
        &self, sql: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn()?;

        let rows = block_in_place(|| conn.query(&sql, &[]))?;
        let rows = rows.into_iter().collect::<Vec<_>>();
        let json = encode_rows_as_json(rows)?;
        let str = if pretty {
            serde_json::to_string_pretty(&json)
        } else {
            serde_json::to_string(&json)
        }?;
        sender.send(Cow::Owned(str)).await?;
        anyhow::Ok(())
    }

    async fn export_table_as_json(
        &self, table_name: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from {}", table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_json(&sql, pretty, sender).await
    }

    async fn export_query_result_as_csv(
        &self,
        sql: &str,
        include_header: bool, // include header if possible,
        delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn()?;

        let rows = block_in_place(|| conn.query(&sql, &[]))?;
        let rows: Vec<_> = rows.collect();
        if include_header && rows.len() > 0 {
            let fs = match rows.get(0).unwrap() {
                Ok(x) => {
                    x.column_info()
                        .iter()
                        .map(|x| Some(x.name().to_owned()))
                        .collect::<Vec<_>>()
                }
                Err(_) => vec![],
            };

            sender
                .send(Cow::Owned(create_csv(&vec![fs], delimiter)))
                .await?;
        }

        let data = encode_rows(rows)?;
        sender
            .send(Cow::Owned(create_csv(&data, delimiter)))
            .await?;
        anyhow::Ok(())
    }

    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        if include_header {
            let fs = self.show_table_columns(table_name).await?;
            let fs = fs.iter().map(|x| Some(x.field.clone())).collect::<Vec<_>>();
            sender
                .send(Cow::Owned(create_csv(&vec![fs], delimiter)))
                .await?;
        }
        let sql = format!("SELECT * from {}", table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_csv(
            &sql, false, // we have emit header before
            delimiter, sender,
        )
        .await
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
        let conn = self.get_conn()?;
        let _ = block_in_place(|| conn.query("SELECT 1 FROM DUAL", &[]))?;
        anyhow::Ok(ins.elapsed())
    }
}

fn type_support_compare(_typ: &str) -> bool {
    true
}
impl OracleDB {
    pub(crate) fn query_size(
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
        let conn = self.get_conn()?;
        let rows = block_in_place(|| conn.query(&sql, &[]))?;
        let mut ret = HashMap::new();
        for row in rows {
            let row = row?;
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
        let conn = self.get_conn()?;
        let result = block_in_place(|| conn.query(&sql, &[]))?;
        let mut ret = Vec::new();
        for r in result {
            let r = r?;
            let name: String = r.get(0)?;
            ret.push(name);
        }
        let sizes = self.query_size(&ret[..])?;

        anyhow::Ok(
            ret.into_iter()
                .map(|x| {
                    ListTable {
                        size: sizes
                            .get(&x)
                            .map(|x| x.clone().map(|x| HumanSize(Box::new(x))))
                            .flatten(),
                        name: x,
                    }
                })
                .collect(),
        )
    }

    pub(crate) async fn list_view_impl(&self, schema: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let conn = self.get_conn()?;
        let sql = format!(
            r#"
        SELECT VIEW_NAME   FROM all_views WHERE OWNER  = '{}'
            "#,
            schema
        );
        log::info!("sql:{}", sql);
        let rows = block_in_place(|| conn.query(&sql, &[]))?;
        let mut ret = Vec::new();
        for r in rows {
            let r = r?;
            let name: String = r.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
    }
}
impl ReadOnlyTrait for OracleDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

#[async_trait::async_trait]
impl SchemaTrait for OracleDB {
    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListMaterializedView>> {
        let sql = format!(
            "SELECT MVIEW_NAME FROM USER_MVIEWS  WHERE OWNER='{}' ",
            self.schema_str()
        );
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        let rows = block_in_place(|| conn.query(&sql, &[]))?;

        let mut ret = Vec::new();

        for row in rows {
            let row = row?;
            let name: String = row.get(0)?;

            ret.push(name);
        }
        let sizes = self.query_size(&ret[..])?;

        anyhow::Ok(
            ret.into_iter()
                .map(|x| {
                    ListTable {
                        size: sizes
                            .get(&x)
                            .map(|x| x.clone().map(|x| HumanSize(Box::new(x))))
                            .flatten(),
                        name: x,
                    }
                })
                .collect(),
        )
    }

    async fn drop_materialized_view(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!("DROP MATERIALIZED VIEW {name}");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;

        block_in_place(|| conn.execute(&sql, &[]))?;
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
        let conn = self.get_conn()?;
        let rows = block_in_place(|| conn.query(&sql, &[]))?;
        let mut ret = Vec::new();

        for row in rows {
            let row = row?;
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
        let sql = format!("CREATE TABLE {dup} AS SELECT * FROM {table}");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, &[]))?;
        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(format!("RENAME {old} TO {new}").as_str(), &[]))?;
        anyhow::Ok(())
    }

    async fn create_table_and_index_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let conn = self.get_conn()?;

        let create_table = create_table_sql(&conn, table_name)?;
        let indexes = self.show_indexes(&table_name).await?;
        let mut indexes_sql = Vec::new();
        for i in indexes.iter() {
            let s = create_index_sql(&conn, &i.name.as_ref().unwrap(), self.schema_str())?;
            indexes_sql.push(s);
        }
        anyhow::Ok(CreateTableAndIndexSql {
            create_table,
            indexes: indexes_sql,
        })
    }

    async fn create_view_sql(&self, view_name: &str, materialized: bool) -> anyhow::Result<String> {
        let conn = self.get_conn()?;

        anyhow::Ok(create_view_sql(&conn, view_name, materialized)?)
    }

    async fn export_table_or_view(
        &self, name: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::CSV {
                include_header,
                delimiter,
            } => {
                self.export_table_as_csv(name, include_header, delimiter, sender)
                    .await
            }
            ExportKind::Json { pretty } => self.export_table_as_json(name, pretty, sender).await,
            ExportKind::JsonLine => self.export_table_as_json_line(name, sender).await,

            ExportKind::Sql {
                include_table_schema,
            } => {
                self.export_table_as_sql(name, include_table_schema, sender)
                    .await
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
        let conn = self.get_conn()?;

        let rows = block_in_place(|| conn.query(sql.as_str(), &[]))?;
        for r in rows {
            let r = r?;
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
            let rows = block_in_place(|| conn.query(sql.as_str(), &[]))?;
            let mut names = HashSet::new();
            for r in rows {
                let r = r?;
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
        Box::new(Clone::clone(self))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.schema_str()).await
    }

    async fn create_index(
        &self, table: &str, name: &str, kind: IndexKind, fields: &[String],
    ) -> anyhow::Result<()> {
        let sql = format!(
            "CREATE {} INDEX {} ON {table}({})",
            match kind {
                IndexKind::Index => "",
                IndexKind::Primary => anyhow::bail!("can't create primary index"),
                IndexKind::Unique => "Unique",
            },
            name,
            fields.join(",")
        );
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, &[]))?;

        anyhow::Ok(())
    }

    async fn rename_index(
        &self, _table_name: &str, oldname: &str, _newname: &str,
    ) -> anyhow::Result<()> {
        let sql = format!("ALTER INDEX {oldname} RENAME TO {_newname}");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, &[]))?;
        anyhow::Ok(())
    }

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<()> {
        let sql = format!("DROP INDEX {index}");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;

        block_in_place(|| conn.execute(&sql, &[]))?;
        anyhow::Ok(())
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        let sql = format!("ALTER TABLE {table} DROP COLUMN {col}");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, &[]))?;
        anyhow::Ok(())
    }

    fn list_data_types(&self) -> anyhow::Result<&'static [&'static str]> {
        anyhow::Ok(ORACLE_TYPES)
    }

    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn()?;
        if old_name != new_name {
            let sql = format!("ALTER TABLE {table} RENAME COLUMN {old_name} TO {new_name}");
            log::info!("sql:{}", sql);
            block_in_place(|| conn.execute(&sql, &[]))?;
        }
        if let Some(typ) = typ {
            let sql = format!("ALTER TABLE {table} MODIFY {new_name} {}", typ.typ);
            conn.execute(&sql, &[])?;
            log::info!("sql:{}", sql);
            if let Some(nullness) = typ.nullness.as_ref() {
                let sql = format!("ALTER TABLE {table} MODIFY {new_name} {nullness}");
                log::info!("sql:{}", sql);
                block_in_place(|| conn.execute(&sql, &[]))?;
            }
            if let Some(dv) = typ.default_value.as_ref() {
                let sql = format!(
                    "ALTER TABLE {table} MODIFY {new_name} DEFAULT {}",
                    self.put_value_in_sql(&typ.typ, None, Some(dv.as_str()))?
                );
                log::info!("sql:{}", sql);
                block_in_place(|| conn.execute(&sql, &[]))?;
            }
        }
        anyhow::Ok(())
    }

    async fn add_table_column(
        &self, table: &str, name: &str, typ: ColumnType,
    ) -> anyhow::Result<()> {
        let sql = format!(
            "ALTER TABLE {table} ADD {name}  {}  {} {}",
            typ.typ,
            if let Some(nullness) = typ.nullness {
                nullness.clone()
            } else {
                "".to_owned()
            },
            if let Some(dv) = typ.default_value.as_ref() {
                format!(
                    "DEFAULT {}",
                    self.put_value_in_sql(&typ.typ, None, Some(dv.as_str()))?
                )
            } else {
                "".to_owned()
            }
        );
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, &[]))?;
        anyhow::Ok(())
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        let conn = self.get_conn()?;
        let sql = gen_select_sql(table_name, where_str, order_by, page, page_size);
        log::info!("query sql:{}", sql);
        let total = OracleDB::query_count(&conn, table_name, where_str)?;
        let rows = block_in_place(|| conn.query(sql.as_str(), &[]))?;
        let data = encode_rows(rows.into_iter().collect::<Vec<_>>())?;
        let cols = self.show_table_columns(table_name).await?;
        anyhow::Ok(QueryResp::new(cols, total as usize, data, sql)?)
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
        let conn = self.get_conn()?;
        let rows = block_in_place(|| conn.query(&sql, &[]))?;
        let mut ret: HashMap<String, Index> = HashMap::new();
        for r in rows {
            let r = r?;
            let name: String = r.get(0)?;
            let col_name = r.get(1)?;
            if let Some(index) = ret.get_mut(&name) {
                index.columns.push(col_name);
            } else {
                let primary = query_index_primary(&conn, &name, table_name).unwrap();
                let unique = if primary {
                    false
                } else {
                    query_index_unique(&conn, &name, table_name)?
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
        anyhow::Ok(ret.into_iter().map(|(_, v)| v).collect())
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let mut conn = self.get_conn()?;
        conn.set_autocommit(true);

        let fs = self.show_table_columns(&table_name).await?;
        let sql = (self).insert_sql(None, table_name, &fs, &values)?;
        log::info!("insert sql:{}", sql);

        let stmt = block_in_place(|| conn.execute(&sql, &[]))?;
        let changes = stmt.row_count()?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(changes),
        })
    }

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fs = self.show_table_columns(table_name).await?;
        let sql = self.update_sql(None, table_name, &fs, &values, &wheres)?;
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;

        let stmt = block_in_place(|| conn.execute(sql.as_str(), &[]))?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(stmt.row_count()?),
        })
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("TRUNCATE TABLE  {table_name}");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, &[]))?;
        anyhow::Ok(())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fields = self.show_table_columns(table_name).await?;
        let sql = self.delete_sql(None, table_name, &fields, &wheres)?;
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        let stmt = block_in_place(|| conn.execute(&sql, &[]))?;
        let row_count = stmt.row_count()?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(row_count),
        })
    }

    async fn list_view(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_view_impl(self.schema_str()).await
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        let sql = format!("DROP VIEW {view_name}");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, &[]))?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        let sql = if cascade {
            format!("DROP TABLE {table} CASCADE CONSTRAINTS")
        } else {
            format!("DROP TABLE {table}")
        };
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        let _ = block_in_place(|| conn.execute(&sql.as_str(), &[]))?;
        anyhow::Ok(())
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        let conn = self.get_conn()?;
        let sql = gen_select_sql(view_name, where_str, order_by, page, page_size);
        log::info!("sql:{}", sql);
        let total = OracleDB::query_count(&conn, view_name, where_str)?;
        log::info!("query sql:{}", sql);
        let rows = block_in_place(|| conn.query(sql.as_str(), &[]))?;
        let rows = rows.into_iter().collect::<Vec<_>>();
        if rows.len() == 0 {
            return anyhow::Ok(QueryResp::new(
                self.show_view_columns(view_name).await?,
                total as usize,
                vec![],
                sql,
            )?);
        }
        let cols = infer_schema_by_result(&rows)?;
        let data = encode_rows(rows)?;
        anyhow::Ok(QueryResp::new(cols, total as usize, data, sql)?)
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
        let conn = self.get_conn()?;
        let mut ret = Vec::new();
        let rows = block_in_place(|| conn.query(&sql, &[]))?;
        for row in rows {
            let row = row?;
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
        let conn = self.get_conn()?;
        let row = block_in_place(|| conn.query_row(&sql, &[]))?;

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
        let sql = format!("DROP TRIGGER {name}");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;

        block_in_place(|| conn.execute(&sql, &[]))?;
        anyhow::Ok(())
    }

    async fn disable_trigger(&self, _table: &str, name: &str) -> anyhow::Result<()> {
        let sql = format!("ALTER TRIGGER {name} DISABLE");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, &[]))?;
        anyhow::Ok(())
    }

    async fn enable_trigger(&self, _table: &str, name: &str) -> anyhow::Result<()> {
        let sql = format!("ALTER TRIGGER {name} ENABLE");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, &[]))?;
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
        let conn = self.get_conn()?;
        let mut ret = Vec::new();
        for row in block_in_place(|| conn.query(&sql, &[]))? {
            let row = row?;
            let name: String = row.get(0)?;
            ret.push(NamedStaff::from(name));
        }
        anyhow::Ok(ret)
    }

    async fn show_udf(
        &self, name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<DefAndMetas> {
        let sql = self.get_user_source(name)?;
        log::info!("sql:{}", sql);
        anyhow::Ok(DefAndMetas {
            def: sql,
            metas: vec![],
        })
    }

    async fn drop_udf(
        &self, name: &str, _extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let sql = format!("DROP FUNCTION {name}");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, &[]))?;
        anyhow::Ok(())
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_procedure_impl(self.schema_str()).await
    }

    async fn show_procedure(
        &self, name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<ProcedureDetail> {
        let sql = self.get_user_source(name)?;
        log::info!("sql:{}", sql);
        anyhow::Ok(ProcedureDetail {
            def: sql,
            metas: vec![],
        })
    }

    async fn drop_procedure(
        &self, name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let sql = format!("DROP PROCEDURE {}", name);
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, &[]))?;
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
        let conn = self.get_conn()?;
        let rows = block_in_place(|| conn.query(&sql, &[]))?;
        let mut ret = Vec::new();
        for row in rows {
            let row = row?;
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

impl QuoteStyle for OracleDB {
    fn quote_style(&self) -> &'static str {
        "\""
    }
}
impl SqlBuilder for OracleDB {
    /// format a value make it suitable for insert into sql.
    /// this method current use in `insert_sql` etc.
    fn put_value_in_sql(
        &self, typ: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        put_value_in_sql(typ, value)
    }

    fn type_support_compare(&self, typ: &str) -> bool {
        type_support_compare(typ)
    }
}
impl OracleDB {
    fn get_user_source(&self, name: &str) -> anyhow::Result<String> {
        let sql = format!(
            r#"
            SELECT text
                FROM user_source
                WHERE name = '{name}'
                ORDER BY line
        "#
        );
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        let rows = block_in_place(|| conn.query(&sql, &[]))?;

        let mut sql = "".to_owned();
        for row in rows {
            let row = row?;
            let t: String = row.get(0)?;
            sql.push_str(t.as_str());
            sql.push('\n');
        }
        anyhow::Ok(sql)
    }
}

impl BackendName for OracleDB {
    fn backend_name(&self) -> &str {
        "oracle"
    }
}

#[async_trait::async_trait]
impl DBTrait for OracleDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let conn = self.get_conn()?;
        let rows = block_in_place(|| conn.query(r#"SELECT USERNAME FROM DBA_USERS"#, &[]))?;
        let mut ret = Vec::new();
        for r in rows {
            let r = r?;
            let db: String = r.get(0)?;
            ret.push(db);
        }
        ret.sort();
        anyhow::Ok(ret)
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut x = Clone::clone(self);
        x.config.schema = Some(name.to_string());
        x.query_connections = Default::default();
        anyhow::Ok(Arc::new(x))
    }

    async fn create_schema(
        &self, _name: &str, _extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }

    async fn drop_schema(&self, _name: &str) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }
}

impl OracleDB {
    pub(crate) fn get_conn(&self) -> anyhow::Result<oracle::Connection> {
        let mut conn: oracle::Connection = block_in_place(|| self.pool.get())?;
        match &self.config.schema {
            Some(db_name) => {
                let sql = format!("ALTER SESSION SET CURRENT_SCHEMA={db_name}");
                log::info!("sql:{}", sql);
                block_in_place(|| conn.execute(&sql, &[]))?;
            }
            None => {}
        }
        block_in_place(|| {
            conn.set_autocommit(true);
        });
        if let Some(timeout) = self.config.call_timeout {
            conn.set_call_timeout(Some(Duration::from_millis(timeout)))?;
        }
        return anyhow::Ok(conn);
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{TEST_HOST, enable_test_log};
    fn make_test_object(schema: Option<String>) -> OracleDB {
        let x = OracleDB::new(
            OracleDBConfig {
                username: "system".to_string(),
                password: "welcome123".to_string(),
                schema,
                read_only: false,
                host: TEST_HOST.to_string(),
                port: 1521,
                call_timeout: None,
                db: "XE".to_string(),
                save_password: true,
            },
            Some(2),
        )
        .unwrap();
        x
    }

    // when I test via `cargo test`
    // sometimes I have this error
    // ERROR - Transaction (Process ID 76) was deadlocked on lock resources with
    // another process and has been chosen as the deadlock victim. Rerun the
    // transaction. code=1205 maybe I need mutex to run one test a time.
    fn test_lock() -> &'static Mutex<()> {
        static MUTEX: Mutex<()> = Mutex::const_new(());
        &MUTEX
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_server_metas() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(None);
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_procedure() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));
        let query_id = db.open_connection().await.unwrap();
        let udf_name = "DO_NOTHING_P";
        let _ = db.drop_procedure(udf_name, None).await;

        db.query_sql(
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_udf() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));
        let query_id = db.open_connection().await.unwrap();
        let udf_name = "DO_NOTHING";
        let _ = db.drop_udf(udf_name, None).await;
        db.query_sql(
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
        .unwrap();

        db.query_sql(
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
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_trigger() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));
        let query_id = db.open_connection().await.unwrap();
        let table_name = "TEST_TRIGGER";
        let _ = db.drop_table(&table_name, false).await;

        db.query_sql(
            query_id,
            r#"
                CREATE TABLE TEST_TRIGGER (
                    id NUMBER GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
                    name VARCHAR2(100),
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                )"#,
        )
        .await
        .unwrap();
        db.query_sql(
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
        .unwrap();

        let triggers = db.list_trigger(&table_name).await.unwrap();
        for tri in triggers.iter() {
            let show_trigger = db.show_trigger(&table_name, &tri.name).await.unwrap();
            println!("trigger detail:{:?}", show_trigger);
            db.disable_trigger(&table_name, &tri.name).await.unwrap();
            db.enable_trigger(&table_name, &tri.name).await.unwrap();
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_rename() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));

        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_table("TEST_RENAME", false).await;
        let _ = db.drop_table("TEST_RENAME2", false).await;
        db.query_sql(
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
        .unwrap();
        db.rename_table("TEST_RENAME", "TEST_RENAME2")
            .await
            .unwrap();

        db.show_table("TEST_RENAME2", None, None, 1, 100, None)
            .await
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]

    async fn test_duplicate() {
        let _guard = test_lock().lock().await;
        enable_test_log();

        let db = make_test_object(Some("SYSTEM".to_string()));

        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_table("TEST_DUPLICATE", false).await;
        let _ = db.drop_table("TEST_DUPLICATE2", false).await;

        let resp = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE TEST_DUPLICATE (
                    ID NUMBER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                    NAME VARCHAR2(100) NOT NULL
                );

                INSERT INTO TEST_DUPLICATE (NAME) VALUES ('xxxyyy1');
                INSERT INTO TEST_DUPLICATE (NAME) VALUES ('xxxyyy2');
                INSERT INTO TEST_DUPLICATE (NAME) VALUES ('xxxyyy3');
                INSERT INTO TEST_DUPLICATE (NAME) VALUES ('xxxyyy4');
                INSERT INTO TEST_DUPLICATE (NAME) VALUES ('xxxyyy5');

                "#,
            )
            .await
            .unwrap();
        for r in resp {
            assert!(r.is_ok(), "err:{:?}", r);
        }

        db.duplicate_table("TEST_DUPLICATE", "TEST_DUPLICATE2")
            .await
            .unwrap();

        let resp = db
            .show_table("TEST_DUPLICATE2", None, None, 1, 100, None)
            .await
            .unwrap();
        assert_eq!(resp.data.len(), 5);
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_check_conn() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(None);
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_view() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));
        let conn = db.get_conn().unwrap();
        let db = db;
        let _ = db.drop_table("TEST_VIEW_TABLE", false).await;
        let _ = db.drop_view("TEST_VIEW").await;

        let ss = vec![
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
            conn.execute(*s, &[]).unwrap();
        }
        let views = db.list_view().await.unwrap();
        println!("{:?}", views);

        let resp = db.show_view("TEST_VIEW", None, None, 1, 100, None).await;
        println!("show view:{:?}", resp);
        let sql = db.create_view_sql("TEST_VIEW", false).await.unwrap();
        println!("create view sql:{}", sql);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_materilized_view() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));
        let conn = db.get_conn().unwrap();
        let db = db;
        let _ = db.drop_table("TEST_MATERLIZED_VIEW_TABLE", false).await;
        let _ = db.drop_materialized_view("TEST_MATERLIZED_VIEW").await;

        let ss = vec![
            r#"
        CREATE TABLE  TEST_MATERLIZED_VIEW_TABLE (
            product_id   NUMBER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            product_name VARCHAR2(50) NOT NULL,
            category     VARCHAR2(30) NOT NULL,
            price        NUMBER(8,2)  NOT NULL
        )"#,
            "INSERT INTO TEST_MATERLIZED_VIEW_TABLE (product_name, category, price)
VALUES ('iPhone 13', 'Electronics', 799.99)",
            "INSERT INTO
TEST_MATERLIZED_VIEW_TABLE (product_name, category, price) VALUES ('Samsung Galaxy S21',
'Electronics', 699.99)",
            r#"
            CREATE MATERIALIZED VIEW TEST_MATERLIZED_VIEW AS
            SELECT product_id, product_name, category, price
            FROM  TEST_MATERLIZED_VIEW_TABLE"#,
        ];

        for s in ss.iter() {
            conn.execute(*s, &[])
                .expect(format!("sql {} execute failed", *s).as_str());
        }
        let views = db.list_materialized_view().await.unwrap();
        println!("list_materialized_view:{:?}", views);

        let resp = db
            .show_view("TEST_MATERLIZED_VIEW", None, None, 1, 100, None)
            .await;
        println!("show view:{:?}", resp);
        let sql = db
            .create_view_sql("TEST_MATERLIZED_VIEW", true)
            .await
            .unwrap();
        println!("create view sql:{}", sql);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_show_indexes() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));
        let conn = db.get_conn().unwrap();
        let db = db;
        let table_name = "TEST_INDEXES";
        let _ = db.drop_table(&table_name, false).await;
        conn.execute(
            format!(
                r#"
        CREATE TABLE {table_name} (
            order_id     NUMBER(10)     PRIMARY KEY,
            customer_id  NUMBER(10)     NOT NULL,
            order_date   DATE           NOT NULL,
            CONSTRAINT unique_customer_order UNIQUE (customer_id, order_date)
        )
        "#
            )
            .as_str(),
            &[],
        )
        .unwrap();
        let indexes = db.show_indexes(table_name).await.unwrap();
        println!("{:?}", indexes);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_table_column() {
        enable_test_log();
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("SYSTEM".to_string()));

        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;

        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .execute_sql(
                query_id,
                r#"
                CREATE TABLE test_column (
                    id NUMBER GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY  -- 主键，自增
                    
                )
                "#,
            )
            .await
            .unwrap()
            .unwrap();

        db.add_table_column(
            table_name,
            "name",
            ColumnType {
                typ: "CHAR".to_string(),
                nullness: Some("not null".to_string()),
                after: None,

                default_value: None,
            },
        )
        .await
        .unwrap();

        db.modify_table_column(
            table_name,
            "name",
            "name2",
            Some(ColumnType {
                typ: "NCHAR(100)".to_string(),
                nullness: Some("null".to_string()),
                after: None,
                default_value: Some("abc".to_string()),
            }),
        )
        .await
        .unwrap();

        db.drop_table_column(table_name, "name2").await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_multidb_trait() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(None);
        let db = &db as &dyn DBTrait;
        let dbs = db.list_schema().await.unwrap();
        println!("dbs : {:?}", dbs);
        db.open_schema("SYSTEM").await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_query_sql() {
        let _guard = test_lock().lock().await;
        let db: OracleDB = make_test_object(Some("SYSTEM".to_string()));

        // get query_uuid
        let query_uuid = db.open_connection().await.unwrap();
        // test auto commit
        db.set_commit(query_uuid, false).await.unwrap();
        assert!(!db.get_commit(query_uuid).await.unwrap());

        let sqls = vec![
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
            let _ = db.query_sql(query_uuid, *sql).await;
        }

        db.commit(query_uuid).await.unwrap();

        let resp = db
            .show_table("TEST_QUERY", None, None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_type1() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let table_name = "TEST_TYPES1";
        let db = make_test_object(Some("SYSTEM".to_string()));
        let mut conn = db.get_conn().unwrap();
        conn.set_autocommit(true);
        let db = db;
        let _ = db.drop_table(&table_name, false).await;

        conn.execute(
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
            &[],
        )
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

        db.insert(&table_name, values).await.unwrap();

        let resp = db
            .show_table(&table_name, Some("1 = 1"), None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        {
            let e = db
                .export_table_to_string(
                    table_name,
                    crate::db::ExportKind::CSV {
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
                    crate::db::ExportKind::CSV {
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_type2() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let table_name = "TEST_TYPES2";
        let db = make_test_object(Some("SYSTEM".to_string()));
        let mut conn = db.get_conn().unwrap();
        conn.set_autocommit(true);
        let db = db;
        let _ = db.drop_table(&table_name, false).await;

        conn.execute(
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
            &[],
        )
        .unwrap();

        let mut values = HashMap::new();
        values.insert("BINARY_FLOAT".to_string(), Some("3.14".to_string()));
        values.insert(
            "BINARY_DOUBLE".to_string(),
            Some("3.1415926535".to_string()),
        );
        values.insert("NUM".to_string(), Some("100".to_string()));
        values.insert("FLOAT_COL".to_string(), Some("2.71".to_string()));

        db.insert(&table_name, values).await.unwrap();

        let resp = db
            .show_table(&table_name, Some("1 = 1"), None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        {
            let e = db
                .export_table_to_string(
                    table_name,
                    crate::db::ExportKind::CSV {
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
                    crate::db::ExportKind::CSV {
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_type3() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let table_name = "TEST_TYPES3";
        let db = make_test_object(Some("SYSTEM".to_string()));
        let mut conn = db.get_conn().unwrap();
        conn.set_autocommit(true);
        let db = db;
        let _ = db.drop_table(&table_name, false).await;

        conn.execute(
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
            &[],
        )
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

        db.insert(&table_name, values).await.unwrap();

        let resp = db
            .show_table(&table_name, Some("1 = 1"), None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);
        {
            let e = db
                .export_table_to_string(
                    table_name,
                    crate::db::ExportKind::CSV {
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
                    crate::db::ExportKind::CSV {
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_type4() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let table_name = "TEST_TYPES4";
        let db = make_test_object(Some("SYSTEM".to_string()));
        let mut conn = db.get_conn().unwrap();
        conn.set_autocommit(true);
        let _ = db.drop_table(&table_name, false).await;

        conn.execute(
            format!(
                r#"
            CREATE TABLE {table_name} (
                COL1 LONG,
                 
                COL2 NUMBER(19),
                COL3 NUMBER(19)
            )"#
            )
            .as_str(),
            &[],
        )
        .unwrap();

        let mut values = HashMap::new();
        values.insert("COL1".to_string(), Some("abc".to_string()));

        values.insert("COL2".to_string(), Some("123".to_string()));
        values.insert("COL3".to_string(), Some("456".to_string()));

        db.insert(&table_name, values).await.unwrap();

        let resp = db
            .show_table(&table_name, Some("1 = 1"), None, 1, 100, None)
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }
}
