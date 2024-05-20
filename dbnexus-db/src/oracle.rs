/// notice oracle sql 不能带有分号
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use std::vec;

use oracle::{Connection, Row};
use tokio::sync::Mutex;

use crate::datasource::{DBPoolKey, OracleDBConfig};
use crate::db::{
    DBTrait, ExeSqlError, Field, Index, ListTable, ListView, MultiDBTrait, QuerySql, SqlExecuteRet,
    SqlKind, SqlSeq, TableListResp,
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

#[async_trait::async_trait]
impl QuerySql for OracleDB {
    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        let conn = self.get_conn().await?;
        self.query_connections.lock().await.insert(id.clone(), conn);
        anyhow::Ok(id)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        if let Some(x) = self.query_connections.lock().await.remove(&id) {
            tokio::spawn(async move { x.close() });
        }
        let conn = self.get_conn().await?;
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
                            let cols = infer_schema_by_result(&rows)?;
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
                    anyhow::bail!("oracle doesn't work this way this is should be unreachable");
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
    let mut sql = format!("SELECT *, ROWNUM as {} FROM \"{}\"", ROW_COL, table_name);
    if let Some(where_str) = where_str {
        sql.push_str(format!(" WHERE {}", where_str).as_str());
    }
    if let Some(order_by) = order_by {
        sql.push_str(format!(" ORDER BY {}", order_by).as_str());
    }

    format!(
        "SELECT t.* from ({}) BETWEEN {} AND {}",
        sql,
        (page - 1) * page_size + 1,
        page * page_size
    )
}

impl OracleDB {
    fn query_count(
        conn: &mut Connection, name: &str, where_str: Option<&str>,
    ) -> anyhow::Result<i64> {
        let row = conn.query_row(
            format!(
                "SELECT count(*) from {name} {}",
                match where_str {
                    Some(x) => format!("  where {} ", x),
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

const ROW_COL: &str = "RNUM";

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
fn encode_rows(
    rows: Vec<oracle::Result<Row>>, except_rnum: bool,
) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let mut data = vec![];
    for r in rows {
        let mut values = vec![];

        let r = r?;
        for c in 0..r.sql_values().len() {
            let col_info = &r.column_info()[c];
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
                oracle::sql_type::OracleType::BinaryFloat => todo!(),
                oracle::sql_type::OracleType::BinaryDouble => todo!(),
                oracle::sql_type::OracleType::Number(..) => todo!(),
                oracle::sql_type::OracleType::Float(_) => todo!(),
                oracle::sql_type::OracleType::Date => todo!(),
                oracle::sql_type::OracleType::Timestamp(_) => todo!(),
                oracle::sql_type::OracleType::TimestampTZ(_) => todo!(),
                oracle::sql_type::OracleType::TimestampLTZ(_) => todo!(),
                oracle::sql_type::OracleType::IntervalDS(..) => todo!(),
                oracle::sql_type::OracleType::IntervalYM(_) => todo!(),
                oracle::sql_type::OracleType::CLOB => todo!(),
                oracle::sql_type::OracleType::NCLOB => todo!(),
                oracle::sql_type::OracleType::BLOB => todo!(),
                oracle::sql_type::OracleType::BFILE => todo!(),
                oracle::sql_type::OracleType::RefCursor => todo!(),
                oracle::sql_type::OracleType::Boolean => todo!(),
                oracle::sql_type::OracleType::Object(_) => todo!(),
                oracle::sql_type::OracleType::Long => todo!(),
                oracle::sql_type::OracleType::LongRaw => todo!(),
                oracle::sql_type::OracleType::Json => todo!(),
                oracle::sql_type::OracleType::Xml => todo!(),
                oracle::sql_type::OracleType::Int64 => todo!(),
                oracle::sql_type::OracleType::UInt64 => todo!(),
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
    match f.typ.to_string().to_uppercase() {
        _ => anyhow::bail!(format!("type '{}' not handled", f.typ)),
    }
}

#[async_trait::async_trait]
impl DBTrait for OracleDB {
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
        let mut conn = self.get_conn().await?;
        let sql = gen_select_sql(table_name, where_str, order_by, page, page_size);
        let total = OracleDB::query_count(&mut conn, table_name, where_str)?;
        let rows = conn.query(sql.as_str(), &[])?;
        let data = encode_rows(rows.into_iter().collect::<Vec<_>>(), true)?;
        let cols = self.show_columns(table_name).await?;
        anyhow::Ok(TableListResp::new(cols, total as usize, data, sql))
    }

    async fn show_indexes(&self, _table_name: &str) -> anyhow::Result<Vec<Index>> {
        unimplemented!()
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let conn = self.get_conn().await?;
        let fs = self.show_columns(&table_name).await?;
        let sql = (self as &dyn DBTrait).insert_sql(table_name, &fs, &values)?;
        let mut stmt = conn.execute(&sql, &[])?;
        stmt.execute(&[])?;
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
        let mut conn = self.get_conn().await?;

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

    async fn truncate_table(&self, _table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;

        unimplemented!()
    }

    async fn delete(
        &self, _table_name: &str, _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        unimplemented!()
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListView>> {
        unimplemented!()
    }

    async fn drop_view(&self, _view_name: &str) -> anyhow::Result<()> {
        unimplemented!()
    }

    async fn drop_table(&self, _table: &str) -> anyhow::Result<()> {
        unimplemented!()
    }

    async fn show_view(
        &self, _view_name: &str, _where_str: Option<&str>, _order_by: Option<&str>, _page: usize,
        _page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        unimplemented!()
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
    use crate::TEST_HOST;
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
        let table_name = "test_type1";

        let db = make_test_object(None);

        let conn = db.get_conn().await.unwrap();
        match conn.execute(
            format!("DROP TABLE {table_name} CASCADE CONSTRAINTS PURGE").as_str(),
            &[],
        ) {
            Ok(mut x) => x.execute(&[]),
            Err(_) => Ok(()),
        }
        .unwrap();

        let mut stmt = conn
            .execute(
                format!(
                    r#"
                CREATE TABLE {table_name} (
                    varchar2_col VARCHAR2(100),
                    nvarchar2_col NVARCHAR2(100),
                    char_col CHAR(10),
                    nchar_col NCHAR(10),
                    raw_col RAW(100)
                )
        "#
                )
                .as_str(),
                &[],
            )
            .unwrap();
        stmt.execute(&[]).unwrap();
    }
}
