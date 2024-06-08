/// notice oracle sql 不能带有分号
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use std::vec;

use oracle::{Connection, Row};
use tokio::sync::Mutex;

use crate::datasource::{DBPoolKey, OracleDBConfig};
use crate::db::{
    create_csv, CreateTableAndIndexSql, DBTrait, ExeSqlError, ExportKind, Field, Index, IndexKind,
    ListTable, ListView, MultiDBTrait, QuerySql, SqlExecuteRet, SqlKind, SqlSeq, TableListResp,
};
use crate::pool::{NexusPool, PoolManager};

#[derive(Clone)]
pub struct OracleDB {
    config: OracleDBConfig,
    pm: PoolManager,
    read_only: bool,
    db_name: Option<String>,
    query_connections: Arc<Mutex<HashMap<uuid::Uuid, Connection>>>,
}

fn get_sql_seq(sql: &str) -> anyhow::Result<Vec<SqlSeq>> {
    // use oracle later when sqlparser supported
    let dialect = sqlparser::dialect::GenericDialect {};
    crate::db::sql_to_sql_seq(
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
        let mut guard = self.query_connections.lock().await;
        let conn = match guard.get_mut(&id) {
            Some(x) => x,
            None => {
                anyhow::bail!(format!("id '{}' not found", id));
            }
        };
        f(conn)
    }

    // a list of set function
    pub async fn query_set_autocommit(
        &self, id: uuid::Uuid, autocommit: bool,
    ) -> anyhow::Result<()> {
        self.with_query_connection(id, |x| -> anyhow::Result<()> {
            x.set_autocommit(autocommit);
            anyhow::Ok(())
        })
        .await
    }

    pub async fn query_autocommit(&self, id: uuid::Uuid) -> anyhow::Result<bool> {
        self.with_query_connection(id, |x| -> anyhow::Result<bool> {
            anyhow::Ok(x.autocommit())
        })
        .await
    }

    pub async fn query_commit(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.with_query_connection(id, |f| -> anyhow::Result<()> {
            f.commit()?;
            anyhow::Ok(())
        })
        .await
    }

    pub async fn query_rollback(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.with_query_connection(id, |f| -> anyhow::Result<()> {
            f.rollback()?;
            anyhow::Ok(())
        })
        .await
    }

    pub async fn query_call_timeout(&self, id: uuid::Uuid) -> anyhow::Result<Option<Duration>> {
        self.with_query_connection(id, |x| -> anyhow::Result<Option<Duration>> {
            let d = x.call_timeout()?;
            anyhow::Ok(d)
        })
        .await
    }

    pub async fn query_set_call_timeout(
        &self, id: uuid::Uuid, d: Option<Duration>,
    ) -> anyhow::Result<()> {
        self.with_query_connection(id, |x| -> anyhow::Result<()> {
            x.set_call_timeout(d)?;
            anyhow::Ok(())
        })
        .await
    }
}

const QUERY_AUTO_COMMIT: bool = false;

#[async_trait::async_trait]
impl QuerySql for OracleDB {
    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        let mut conn = self.get_conn().await?;
        conn.set_autocommit(QUERY_AUTO_COMMIT);
        self.query_connections.lock().await.insert(id.clone(), conn);
        anyhow::Ok(id)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        if let Some(x) = self.query_connections.lock().await.remove(&id) {
            tokio::spawn(async move { x.close() });
        }
        let mut conn = self.get_conn().await?;
        conn.set_autocommit(QUERY_AUTO_COMMIT);
        self.query_connections.lock().await.insert(id.clone(), conn);
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        if let Some(x) = self.query_connections.lock().await.remove(&id) {
            tokio::spawn(async move { x.close() });
        }
        anyhow::Ok(())
    }

    async fn query_sql(
        &self, id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<TableListResp, ExeSqlError>>> {
        let ss = match get_sql_seq(sql) {
            Ok(x) => x,
            Err(_err) => {
                vec![SqlSeq {
                    sql: sql.to_string(),
                    kind: SqlKind::Query,
                }]
            }
        };
        let mut conns = self.query_connections.lock().await;
        let conn = match conns.get_mut(&id) {
            Some(x) => x,
            None => anyhow::bail!(format!("conn  '{}' id not found", id)),
        };

        let mut ret = Vec::new();

        for s in ss.iter() {
            match s.kind {
                SqlKind::Query => {
                    match conn.query(&s.sql, &[]) {
                        Ok(rows) => {
                            let rows = rows.into_iter().collect::<Vec<_>>();
                            let cols = infer_schema_by_result(&rows, false)?;
                            let data = encode_rows(rows, false)?;
                            ret.push(Ok(TableListResp::new(cols, data.len(), data, &s.sql)));
                        }
                        Err(err) => {
                            ret.push(Err(ExeSqlError::new(s.sql.as_str(), err)));
                        }
                    };
                }
                SqlKind::Execution => {
                    let mut stmt = conn.execute(sql, &[])?;
                    let row_aff = stmt.row_count()?;

                    match stmt.execute(&[]) {
                        Ok(_) => {
                            ret.push(Ok(TableListResp::row_affected(s.sql.as_str(), row_aff)));
                        }
                        Err(err) => ret.push(Err(ExeSqlError::new(s.sql.as_str(), err))),
                    }
                }
                SqlKind::Begin | SqlKind::Commit | SqlKind::Rollback => {
                    anyhow::bail!("oracle doesn't work this way,this is should be unreachable");
                }
            }
        }

        anyhow::Ok(ret)
    }
}

fn gen_select_sql(
    table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize,
) -> String {
    let mut sql = format!("SELECT {table_name}.*, ROWNUM as {ROW_COL} FROM \"{table_name}\"",);
    if let Some(where_str) = where_str {
        sql.push_str(format!(" WHERE {}", where_str).as_str());
    }
    if let Some(order_by) = order_by {
        sql.push_str(format!(" ORDER BY {}", order_by).as_str());
    }

    format!(
        "SELECT * FROM ({})  WHERE ROWNUM BETWEEN {} AND {}",
        sql,
        (page - 1) * page_size + 1,
        page * page_size
    )
}

impl OracleDB {
    fn query_count(conn: &Connection, name: &str, where_str: Option<&str>) -> anyhow::Result<i64> {
        let row = conn.query_row(
            format!(
                "SELECT count(*) FROM {name} {}",
                match where_str {
                    Some(x) => format!("  WHERE {} ", x),
                    None => "".to_string(),
                }
            )
            .as_str(),
            &[],
        )?;
        let value: i64 = row.get(0)?;
        anyhow::Ok(value)
    }
}

const ROW_COL: &str = "RNUMXXX_YYY_ZZZZ";

fn infer_schema_by_result(
    rows: &Vec<oracle::Result<Row>>, expect_row_num: bool,
) -> anyhow::Result<Vec<Field>> {
    if rows.len() == 0 {
        return anyhow::Ok(vec![]);
    }

    let x = match &rows[0] {
        Ok(x) => x,
        Err(err) => anyhow::bail!(format!("{}", err.to_string())),
    };
    infer_schema_by_row(x, expect_row_num)
}
fn infer_schema_by_row(x: &oracle::Row, expect_row_num: bool) -> anyhow::Result<Vec<Field>> {
    let mut ret = Vec::with_capacity(x.column_info().len());
    for c in x.column_info().iter() {
        if expect_row_num && c.name() == ROW_COL {
            continue;
        }
        ret.push(Field::from_name_type(
            c.name().to_string(),
            c.oracle_type().to_string(),
        ))
    }
    anyhow::Ok(ret)
}

/// some query will auto include a ROWNUM column
fn encode_rows(
    rows: Vec<oracle::Result<Row>>, except_rnum: bool,
) -> anyhow::Result<Vec<Vec<Option<String>>>> {
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
            if except_rnum && col_info.name() == ROW_COL {
                continue;
            }
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
                oracle::sql_type::OracleType::Date => return type_not_support,
                oracle::sql_type::OracleType::Timestamp(_) => return type_not_support,
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

impl OracleDB {
    fn db_name_unchecked(&self) -> &String {
        self.db_name.as_ref().unwrap()
    }
}

fn put_value_in_sql(f: &Field, value: Option<&str>) -> anyhow::Result<String> {
    let null_string = "NULL".to_string();

    match f.typ.to_string().to_uppercase().as_str() {
        "VARCHAR2" => {
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
        "CHAR" => {
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
        "RAW" => {
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
        // "BFILE" => {
        //     anyhow::Ok(match value {
        //         Some(x) => format!("'{}'", x),
        //         None => null_string,
        //     })
        // }
        _ => anyhow::bail!(format!("type '{}' not handled", f.typ)),
    }
}

fn encode_rows_as_json(rows: Vec<oracle::Result<Row>>) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut datas = vec![];
    for r in rows {
        let mut values = serde_json::map::Map::new();
        let r = r?;
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
                oracle::sql_type::OracleType::Date => return type_not_support,
                oracle::sql_type::OracleType::Timestamp(_) => return type_not_support,
                oracle::sql_type::OracleType::TimestampTZ(_) => return type_not_support,
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
        datas.push(serde_json::Value::Object(values));
    }

    anyhow::Ok(datas)
}

fn create_view_sql(conn: &Connection, view_name: &str) -> anyhow::Result<String> {
    let sql = format!("SELECT text FROM user_views WHERE view_name = '{view_name}'");
    let row = conn.query_row(&sql, &[])?;
    let s: String = row.get(0)?;
    anyhow::Ok(s)
}
fn create_table_sql(conn: &Connection, table_name: &str) -> anyhow::Result<String> {
    let sql = format!("SELECT DBMS_METADATA.GET_DDL('TABLE', '{table_name}')  FROM dual");
    let row = conn.query_row(&sql, &[])?;
    let s: String = row.get(0)?;
    anyhow::Ok(s)
}

fn create_index_sql(conn: &Connection, name: &str, schema: &str) -> anyhow::Result<String> {
    let sql = format!("SELECT DBMS_METADATA.GET_DDL('INDEX', '{name}', '{schema}') FROM dual;");
    let row = conn.query_row(&sql, &[])?;
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
            let ss = DBTrait::create_table_and_index_sql(self, table_name).await?;
            sender.send(Cow::Owned(ss.create_table.clone())).await?;
            sender.send(Cow::Borrowed(";\n")).await?;
            for index in ss.indexes.into_iter() {
                sender.send(Cow::Owned(index)).await?;
                sender.send(Cow::Borrowed(";\n")).await?;
            }
        }
        let conn = self.get_conn().await?;
        let cols = DBTrait::show_columns(self, table_name).await?;
        let sql = format!("select * from {}", table_name);
        let rows = conn.query(sql.as_str(), &[])?;
        let rows = rows.into_iter().collect::<Vec<_>>();

        let data = encode_rows(rows, false)?;
        for d in data.into_iter() {
            let mut m = HashMap::new();
            for (d, f) in d.into_iter().zip(cols.iter()) {
                m.insert(f.name.clone(), d);
            }
            let insert_sql = (self as &dyn DBTrait).insert_sql(table_name, &cols, &m)?;
            sender.send(Cow::Owned(insert_sql)).await?;
            sender.send(Cow::Borrowed("\n")).await?;
        }

        anyhow::Ok(())
    }

    async fn export_table_as_json(
        &self, table_name: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn().await?;

        let sql = format!("select * from {}", table_name);
        let rows = conn.query(&sql, &[])?;
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

    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn().await?;
        if include_header {
            let fs = self.show_columns(table_name).await?;
            let fs = fs.iter().map(|x| Some(x.name.clone())).collect::<Vec<_>>();
            sender
                .send(Cow::Owned(create_csv(&vec![fs], delimiter)))
                .await?;
        }

        let sql = format!("select * from {}", table_name);
        let rows = conn.query(&sql, &[])?;
        let rows: Vec<_> = rows.collect();
        let data = encode_rows(rows, false)?;
        sender
            .send(Cow::Owned(create_csv(&data, delimiter)))
            .await?;
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl DBTrait for OracleDB {
    async fn create_table_and_index_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let conn = self.get_conn().await?;

        let create_table = create_table_sql(&conn, table_name)?;
        let indexes = self.show_indexes(&table_name).await?;
        let mut indexes_sql = Vec::new();
        for i in indexes.iter() {
            let s = create_index_sql(&conn, &i.name, self.db_name.as_deref().unwrap())?;
            indexes_sql.push(s);
        }
        anyhow::Ok(CreateTableAndIndexSql {
            create_table,
            indexes: indexes_sql,
        })
    }

    async fn create_view_sql(&self, view_name: &str) -> anyhow::Result<String> {
        let conn = self.get_conn().await?;

        anyhow::Ok(create_view_sql(&conn, view_name)?)
    }

    async fn export_table(
        &self, table_name: &str, kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::CSV {
                include_header,
                delimiter,
            } => {
                self.export_table_as_csv(table_name, include_header, delimiter, sender)
                    .await
            }
            ExportKind::Json { pretty } => {
                self.export_table_as_json(table_name, pretty, sender).await
            }
            ExportKind::Sql {
                include_table_schema,
            } => {
                self.export_table_as_sql(table_name, include_table_schema, sender)
                    .await
            }
        }
    }

    async fn show_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let sql = format!(
            "SELECT COLUMN_NAME, DATA_TYPE , NULLABLE ,DATA_DEFAULT FROM DBA_TAB_COLUMNS WHERE OWNER = '{}' AND TABLE_NAME = '{table_name}'",
            self.db_name.as_ref().unwrap()
        );
        let mut ret = Vec::new();
        let conn = self.get_conn().await?;

        let rows = conn.query(sql.as_str(), &[])?;
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
                self.db_name_unchecked(),
                table_name,
                table_name
            );
            let rows = conn.query(sql.as_str(), &[])?;
            let mut names = HashSet::new();
            for r in rows {
                let r = r?;
                let name: String = r.get(0)?;
                names.insert(name);
            }
            names
        };
        for x in ret.iter_mut() {
            if names.contains(&x.name) {
                x.is_primary = true;
            }
        }
        anyhow::Ok(ret)
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    fn backend_name(&self) -> &str {
        "oracle"
    }

    fn clone(&self) -> Box<dyn DBTrait> {
        Box::new(Clone::clone(self))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"SELECT TABLE_NAME FROM DBA_TABLES WHERE OWNER = '{}'"#,
            self.db_name.as_ref().unwrap()
        );
        let conn = self.get_conn().await?;
        let result = conn.query(&sql, &[])?;
        let mut ret = Vec::new();
        for r in result {
            let r = r?;
            let name: String = r.get(0)?;
            ret.push(name);
        }

        anyhow::Ok(ret.into_iter().map(|x| ListTable::from(x)).collect())
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let conn = self.get_conn().await?;
        let sql = gen_select_sql(table_name, where_str, order_by, page, page_size);
        let total = OracleDB::query_count(&conn, table_name, where_str)?;
        log::info!("query sql:{}", sql);
        let rows = conn.query(sql.as_str(), &[]).unwrap();
        let data = encode_rows(rows.into_iter().collect::<Vec<_>>(), true)?;
        let cols = self.show_columns(table_name).await?;
        anyhow::Ok(TableListResp::new(cols, total as usize, data, sql))
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
        let conn = self.get_conn().await?;
        let rows = conn.query(&sql, &[])?;
        let mut ret: HashMap<String, Index> = HashMap::new();
        for r in rows {
            let r = r?;
            let name: String = r.get(0)?;
            let col_name = r.get(1)?;
            if let Some(index) = ret.get_mut(&name) {
                index.on.push(col_name);
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
                        name,
                        kind: if primary {
                            IndexKind::Primary
                        } else if unique {
                            IndexKind::Unique
                        } else {
                            IndexKind::Normal
                        },
                        on: vec![col_name],
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
        let mut conn = self.get_conn().await?;
        conn.set_autocommit(true);

        let fs = self.show_columns(&table_name).await?;
        let sql = (self as &dyn DBTrait).insert_sql(table_name, &fs, &values)?;
        log::info!("insert sql:{}", sql);

        let stmt = conn.execute(&sql, &[])?;
        let changes = stmt.row_count()?;
        let id = stmt.last_row_id()?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes,
            last_insert_row_id: None,
            last_insert_id_string: id,
        })
    }

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fs = self.show_columns(table_name).await?;
        let sql =
            (self as &dyn crate::db::DBTrait).update_sql(table_name, &fs, &values, &wheres)?;
        let conn = self.get_conn().await?;

        let mut stmt = conn.execute(sql.as_str(), &[])?;
        stmt.execute(&[])?;

        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: stmt.row_count()?,
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    /// format a value make it suitable for insert into sql.
    /// this method current use in `insert_sql` etc.
    fn put_value_in_sql(&self, f: &Field, value: Option<&str>) -> anyhow::Result<String> {
        put_value_in_sql(f, value)
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("TRUNCATE TABLE  {table_name}");
        let conn = self.get_conn().await?;
        conn.execute(&sql, &[])?;
        anyhow::Ok(())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fields = self.show_columns(table_name).await?;
        let sql = (self as &dyn crate::db::DBTrait).delete_sql(table_name, &fields, &wheres)?;
        let conn = self.get_conn().await?;
        let stmt = conn.execute(&sql, &[])?;
        let row_count = stmt.row_count()?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: row_count,
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListView>> {
        let conn = self.get_conn().await?;
        let sql = format!(
            r#"
        SELECT VIEW_NAME   FROM all_views WHERE OWNER  = '{}'
            "#,
            self.db_name.as_deref().unwrap()
        );
        let rows = conn.query(&sql, &[])?;
        let mut ret = Vec::new();
        for r in rows {
            let r = r?;
            let name: String = r.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret.into_iter().map(ListView::from).collect())
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        let sql = format!("DROP VIEW {view_name}");
        let conn = self.get_conn().await?;
        conn.execute(&sql, &[])?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str) -> anyhow::Result<()> {
        let sql = format!("DROP TABLE {table} CASCADE CONSTRAINTS PURGE");
        let conn = self.get_conn().await?;
        let mut stmt = conn.execute(&sql.as_str(), &[])?;
        stmt.execute(&[])?;
        anyhow::Ok(())
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let conn = self.get_conn().await?;
        let sql = gen_select_sql(view_name, where_str, order_by, page, page_size);
        let total = OracleDB::query_count(&conn, view_name, where_str)?;
        log::info!("query sql:{}", sql);
        let rows = conn.query(sql.as_str(), &[]).unwrap();
        let rows = rows.into_iter().collect::<Vec<_>>();
        let cols = infer_schema_by_result(&rows, true)?;
        let data = encode_rows(rows, true)?;
        anyhow::Ok(TableListResp::new(cols, total as usize, data, sql))
    }
}
#[async_trait::async_trait]
impl MultiDBTrait for OracleDB {
    async fn list_db(&self) -> anyhow::Result<Vec<String>> {
        let conn = self.get_conn().await?;
        let rows = conn
            .query(r#"SELECT USERNAME FROM DBA_USERS"#, &[])
            .unwrap();
        let mut ret = Vec::new();
        for r in rows {
            let r = r?;
            let db: String = r.get(0)?;
            ret.push(db);
        }
        anyhow::Ok(ret)
    }

    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        let mut x = Clone::clone(self);
        x.db_name = Some(name.to_string());
        anyhow::Ok(Arc::new(x))
    }
}

impl OracleDB {
    async fn get_conn(&self) -> anyhow::Result<oracle::Connection> {
        let pool = match self.pm.get(&DBPoolKey::Oracle(self.config.clone())).await? {
            NexusPool::Oracle(p) => p.clone(),
            _ => unreachable!(),
        };
        let conn: oracle::Connection = tokio::task::spawn_blocking(move || pool.get())
            .await
            .unwrap()?;
        match &self.db_name {
            Some(db_name) => {
                let sql = format!("ALTER SESSION SET CURRENT_SCHEMA={db_name}");
                conn.execute(&sql, &[])?;
            }
            None => {}
        }
        return anyhow::Ok(conn);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{init_log, TEST_HOST};
    fn make_test_object(db_name: Option<String>) -> OracleDB {
        let x = OracleDB {
            pm: Default::default(),
            read_only: false,
            config: OracleDBConfig {
                username: "system".to_string(),
                password: "root".to_string(),
                connect_str: format!("//{}/ORCL", *TEST_HOST),
            },
            db_name,
            query_connections: Default::default(),
        };
        x
    }
    #[tokio::test]
    async fn test_view() {
        let db = make_test_object(Some("SYSTEM".to_string()));
        let conn = db.get_conn().await.unwrap();
        let db = &db as &dyn DBTrait;
        let _ = db.drop_table("TEST_VIEW_TABLE").await;
        let _ = db.drop_view("PRODUCT_VIEW").await;

        let ss = vec![
            r#"
        CREATE TABLE TEST_VIEW_TABLE (
    product_id   NUMBER GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    product_name VARCHAR2(50) NOT NULL,
    category     VARCHAR2(30) NOT NULL,
    price        NUMBER(8,2)  NOT NULL
)"#,
            "INSERT INTO TEST_VIEW_TABLE (product_name, category, price) VALUES ('iPhone 13', 'Electronics', 799.99)",
            "INSERT INTO TEST_VIEW_TABLE (product_name, category, price) VALUES ('Samsung Galaxy S21', 'Electronics', 699.99)",
            r#"
            CREATE VIEW PRODUCT_VIEW AS
SELECT product_id, product_name, category, price
FROM  TEST_VIEW_TABLE"#,
        ];

        for s in ss.iter() {
            conn.execute(*s, &[]).unwrap();
        }
        let views = db.list_view().await.unwrap();
        println!("{:?}", views);

        let resp = db.show_view("PRODUCT_VIEW", None, None, 1, 100).await;
        println!("show view:{:?}", resp);
        db.drop_view("PRODUCT_VIEW").await.unwrap();

        db.drop_table("TEST_VIEW_TABLE").await.unwrap();
    }
    #[tokio::test]
    async fn test_show_indexes() {
        let db = make_test_object(Some("SYSTEM".to_string()));
        let conn = db.get_conn().await.unwrap();
        let db = &db as &dyn DBTrait;
        let table_name = "TEST_INDEXES";
        let _ = db.drop_table(&table_name).await;
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

    #[tokio::test]
    async fn test_multidb_trait() {
        let db = make_test_object(None);
        let db = &db as &dyn MultiDBTrait;
        let dbs = db.list_db().await.unwrap();
        println!("dbs : {:?}", dbs);
        db.open_db("SYSTEM").await.unwrap();
    }

    #[tokio::test]
    async fn test_show_tables() {
        let db = make_test_object(Some("APPQOSSYS".to_string()));
        let db = &db as &dyn DBTrait;
        let tables = db.list_table().await.unwrap();
        println!("tables : {:?}", tables);
        for t in tables.iter() {
            let cols = db.show_columns(&t.table).await.unwrap();
            println!("table:{} ,cols:{:?}", t.table, cols);
        }
    }

    #[tokio::test]
    async fn test_type1() {
        init_log();
        let table_name = "TEST_TYPES1";
        let db = make_test_object(Some("SYSTEM".to_string()));
        let mut conn = db.get_conn().await.unwrap();
        conn.set_autocommit(true);
        let db = &db as &dyn DBTrait;
        let _ = db.drop_table(&table_name).await;

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
            .show_table(&table_name, Some("1 = 1"), None, 1, 100)
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

    #[tokio::test]
    async fn test_type2() {
        init_log();
        let table_name = "TEST_TYPES2";
        let db = make_test_object(Some("SYSTEM".to_string()));
        let mut conn = db.get_conn().await.unwrap();
        conn.set_autocommit(true);
        let db = &db as &dyn DBTrait;
        let _ = db.drop_table(&table_name).await;

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
            .show_table(&table_name, Some("1 = 1"), None, 1, 100)
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

    #[tokio::test]
    async fn test_type3() {
        init_log();
        let table_name = "TEST_TYPES3";
        let db = make_test_object(Some("SYSTEM".to_string()));
        let mut conn = db.get_conn().await.unwrap();
        conn.set_autocommit(true);
        let db = &db as &dyn DBTrait;
        let _ = db.drop_table(&table_name).await;

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
            .show_table(&table_name, Some("1 = 1"), None, 1, 100)
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
    // #[tokio::test]
    // async fn test_type4() {
    //     init_log();
    //     let table_name = "TEST_TYPES4";
    //     let db = make_test_object(Some("SYSTEM".to_string()));
    //     let mut conn = db.get_conn().await.unwrap();
    //     conn.set_autocommit(true);
    //     let db = &db as &dyn DBTrait;
    //     let _ = db.drop_table(&table_name).await;
    //     println!(
    //         r#"
    //     CREATE TABLE {table_name} (
    //         COL1 LONG,
    //         COL2 LONGRAW,
    //         COL3 NUMBER(19,0),
    //         COL4 NUMBER(5,0) UNSIGNED
    //     )"#
    //     );

    //     conn.execute(
    //         format!(
    //             r#"
    //         CREATE TABLE {table_name} (
    //             COL1 LONG,
    //             COL2 LONGRAW
    //             -- COL3 NUMBER(19,0),
    //             -- COL4 NUMBER(5,0) UNSIGNED
    //         )"#
    //         )
    //         .as_str(),
    //         &[],
    //     )
    //     .unwrap();

    //     let mut values = HashMap::new();
    //     values.insert("COL1".to_string(), Some("abc".to_string()));
    //     values.insert("COL2".to_string(), Some("010203".to_string()));
    //     values.insert("COL3".to_string(), Some("123".to_string()));
    //     values.insert("COL4".to_string(), Some("456".to_string()));

    //     db.insert(&table_name, values).await.unwrap();

    //     let resp = db
    //         .show_table(&table_name, Some("1 = 1"), None, 1, 100)
    //         .await
    //         .unwrap();
    //     println!("resp:{:?}", resp);
    // }
}
