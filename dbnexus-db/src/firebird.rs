/*
docker run -d   --name some-firebird  -e FIREBIRD_DATABASE=test.fdb   -e FIREBIRD_ROOT_PASSWORD=root  -e FIREBIRD_USER=alice -e         FIREBIRD_PASSWORD=root  -e   -e   -p 3050:3050    firebirdsql/firebird
*/
#![allow(unused)]
use std::{
    collections::{HashMap, HashSet},
    fs,
    ops::DerefMut,
    sync::Arc,
    time::Duration,
};

use crate::{
    datasource_config::FireBirdConfig,
    db::{
        BackendName, CheckConn, Field, ListTable, NamedStaff, QueryResp, QuerySql, ReadOnlyTrait,
        SchemaTrait, ServerMetas, ShowExtraArgs, SqlExecuteRet, TableListResp,
    },
    helper::{SqlKind, SqlWithKind, is_dangerous_sql_by_parser, sql_to_sql_seq},
    meta_value::MetaValue,
    query_log::QueryLog,
};

use dashmap::DashMap;
use r2d2::PooledConnection;
use rsfbclient::{Execute, Queryable, Row, builder_pure_rust};
use rsfbclient::{FbError, SqlType};
use sqlx::any;
use tokio::{sync::Mutex, task::block_in_place};
pub struct PoolManager {
    config: FireBirdConfig,
}

pub type Connection = rsfbclient::Connection<rsfbclient_rust::RustFbClient>;

impl r2d2::ManageConnection for PoolManager {
    type Connection = Connection;
    type Error = FbError;
    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let conn = builder_pure_rust()
            .host(&self.config.host)
            .port(self.config.port)
            .user(&self.config.username)
            .pass(&self.config.password)
            .db_name(&self.config.db)
            .connect()?;
        Ok(conn)
    }
    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        let _: Option<Row> = conn.query_first("SELECT 1 FROM RDB$DATABASE;", ())?;
        Ok(())
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        self.is_valid(conn).is_err()
    }
}

impl ReadOnlyTrait for FireBirdDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

impl BackendName for FireBirdDB {
    fn backend_name(&self) -> &str {
        "firebird"
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
fn infer_fields(row: &Row) -> Vec<Field> {
    row.cols
        .iter()
        .map(|x| {
            Field::from_name_type(x.name.as_str(), match &x.value {
                SqlType::Text(_) => "TEXT",
                SqlType::Integer(_) => "INTEGER",
                SqlType::Floating(_) => "FLOAT",
                SqlType::Timestamp(_) => "TIMESTAMP",
                SqlType::Binary(items) => "BLOB",
                SqlType::Boolean(_) => "BOOLEAN",
                SqlType::Null => "NULL",
            })
        })
        .collect()
}

fn encode_rows(rows: &Vec<Row>) -> Vec<Vec<Option<String>>> {
    let mut ret = Vec::with_capacity(rows.len());
    for row in rows.iter() {
        let mut data = Vec::with_capacity(row.cols.len());
        for v in row.cols.iter() {
            let value = match &v.value {
                SqlType::Text(x) => Some(x.clone()),
                SqlType::Integer(x) => Some(x.to_string()),
                SqlType::Floating(x) => Some(x.to_string()),
                SqlType::Timestamp(naive_date_time) => Some(naive_date_time.to_string()),
                SqlType::Binary(items) => todo!(),
                SqlType::Boolean(x) => Some(x.to_string()),
                SqlType::Null => None,
            };
            data.push(value);
        }

        ret.push(data);
    }

    ret
}

#[async_trait::async_trait]
impl QuerySql for FireBirdDB {
    async fn query_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let mut query_log = QueryLog::get_guard(query_id, sql);
        let ss = match get_sql_seq(sql) {
            Ok(x) => x,
            Err(_err) => {
                vec![SqlWithKind {
                    sql: sql.to_string(),
                    kind: SqlKind::Query,
                }]
            }
        };
        let mut ret = Vec::with_capacity(ss.len());
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;

        let conn = conn.deref_mut();

        for s in ss.iter() {
            match s.kind {
                SqlKind::Query => {
                    let rows: Result<Vec<Row>, _> = block_in_place(|| conn.query(&s.sql, ()));
                    match rows {
                        Ok(rows) => {
                            if rows.len() > 0 {
                                let fields = infer_fields(rows.get(0).unwrap());
                                let data = encode_rows(&rows);
                                ret.push(Ok(QueryResp { fields, data }));
                            }
                        }
                        Err(err) => ret.push(Err(err.to_string())),
                    }
                }
                _ => match block_in_place(|| conn.execute(&s.sql, ())) {
                    Ok(x) => ret.push(Ok(QueryResp::row_affected(x as u64))),
                    Err(err) => ret.push(Err(err.to_string())),
                },
            }
        }

        query_log.set_success(ret.iter().all(|x| x.is_ok()));
        anyhow::Ok(ret)
    }

    async fn execute_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let mut _guard = QueryLog::get_guard(query_id, sql);

        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;

        let conn = conn.deref_mut();

        let resp = block_in_place(|| conn.execute(sql, ()))
            .map(|x| QueryResp::row_affected(x as u64))
            .map_err(|e| e.to_string());

        _guard.set_success(resp.is_ok());

        anyhow::Ok(resp)
    }

    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        let conn = self.get_conn()?;
        self.query_connections
            .insert(id.clone(), Arc::new(Mutex::new(conn)));
        anyhow::Ok(id)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        let conn = self.get_conn()?;
        self.query_connections
            .insert(id.clone(), Arc::new(Mutex::new(conn)));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.query_connections.remove(&id);
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn QuerySql> {
        Box::new(Clone::clone(self))
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        is_dangerous_sql_by_parser(sql, &sqlparser::dialect::GenericDialect {})
    }
}

#[derive(Clone)]

pub struct FireBirdDB {
    config: FireBirdConfig,
    pool: r2d2::Pool<PoolManager>,
    query_connections: Arc<DashMap<uuid::Uuid, Arc<Mutex<PooledConnection<PoolManager>>>>>,
}

impl std::fmt::Debug for FireBirdDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "host={} user={}", self.config.host, self.config.username)
    }
}

#[async_trait::async_trait]
impl ServerMetas for FireBirdDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl SchemaTrait for FireBirdDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn quote_style(&self) -> &'static str {
        "\""
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        unimplemented!()
    }
    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        unimplemented!()
    }
    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let mut conn = self.get_conn()?;

        let pri_keys = {
            let sql = format!(
                r#"
            
                SELECT 
                    s.RDB$FIELD_NAME AS COLUMN_NAME
                FROM 
                    RDB$RELATION_CONSTRAINTS rc
                JOIN 
                    RDB$INDEX_SEGMENTS s ON rc.RDB$INDEX_NAME = s.RDB$INDEX_NAME
                WHERE 
                    rc.RDB$CONSTRAINT_TYPE = 'PRIMARY KEY'
                    AND rc.RDB$RELATION_NAME = '{table_name}'
                ORDER BY 
                    s.RDB$FIELD_POSITION;
            "#
            );
            let x: Vec<(String,)> = block_in_place(|| conn.query(&sql, ()))?;
            x.into_iter().map(|x| x.0).collect::<HashSet<_>>()
        };

        let sql = format!(
            r#"
            SELECT 
            rf.RDB$FIELD_NAME AS COLUMN_NAME, 
            t.RDB$TYPE_NAME AS DATA_TYPE,
            CASE WHEN f.RDB$NULL_FLAG = 1 THEN 'NO' ELSE 'YES' END AS IS_NULLABLE,
            f.RDB$DEFAULT_SOURCE AS DEFAULT_VALUE
        FROM 
            RDB$RELATION_FIELDS rf
        JOIN 
            RDB$FIELDS f ON rf.RDB$FIELD_SOURCE = f.RDB$FIELD_NAME
        JOIN 
            RDB$TYPES t ON f.RDB$FIELD_TYPE = t.RDB$TYPE
        WHERE 
            rf.RDB$RELATION_NAME = '{}'
            AND t.RDB$FIELD_NAME = 'RDB$FIELD_TYPE';
        
        "#,
            table_name
        );

        let rows: Vec<(String, String, String, Option<String>)> =
            block_in_place(|| conn.query(&sql, ()))?;
        let mut ret = Vec::new();
        for (name, typ, nullable, default_value) in rows {
            let is_primary = pri_keys.contains(&name);
            ret.push(Field {
                field: name.trim().to_owned(),
                typ,
                nullable: nullable == "YES",
                default_value,
                is_primary,
                is_auto_increment: false,
                sortable: true,
            });
        }

        anyhow::Ok(ret)
    }

    fn put_value_in_sql(&self, typ: &str, value: Option<&str>) -> anyhow::Result<String> {
        put_value_in_sql(typ, value)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<TableListResp> {
        let sql = gen_show_sql(table_name, where_str, order_by, page, page_size);
        log::info!("sql:{}", sql);

        let fields = self.show_table_columns(table_name).await?;
        let mut conn = self.get_conn()?;
        let rows: Vec<rsfbclient::Row> = block_in_place(|| conn.query(&sql, ()))?;

        let data = encode_rows(&rows);
        let total = self.query_count(table_name, where_str)?;

        anyhow::Ok(TableListResp {
            fields,
            total,
            data,
            sql: Some(sql),
        })
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;

        let sql = (self as &dyn SchemaTrait).insert_sql(None, table_name, &fs, &values)?;
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn()?;
        let changes = conn.execute(&sql, ())?;

        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(changes as u64),
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        unimplemented!()
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        unimplemented!()
    }

    async fn delete(
        &self, _table_name: &str, _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        unimplemented!()
    }

    async fn list_view(&self) -> anyhow::Result<Vec<NamedStaff>> {
        unimplemented!()
    }
    async fn drop_view(&self, _view_name: &str) -> anyhow::Result<()> {
        unimplemented!()
    }
    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        let sql = format!(r#"DROP TABLE "{}""#, table);
        let mut conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, ()))?;
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }
}

impl FireBirdDB {
    fn query_count(&self, name: &str, where_str: Option<&str>) -> anyhow::Result<usize> {
        let mut conn = self.get_conn()?;
        let mut s = format!(r#"SELECT count(1) from "{name}" "#);
        if let Some(where_str) = where_str {
            s.push_str(format!(" where {where_str}").as_str());
        }
        let row: Option<(i64,)> = conn.query_first(&s, ())?;
        anyhow::Ok(row.map(|x| x.0 as usize).unwrap_or_default())
    }
}

fn put_value_in_sql(typ: &str, value: Option<&str>) -> anyhow::Result<String> {
    let typ = typ.to_lowercase();
    log::trace!("encode value '{:?}' on type '{}'", value, typ);
    match typ {
        _ => anyhow::Ok(value.map(|x| x.to_string()).unwrap_or("null".to_string())),
    }
}

#[async_trait::async_trait]
impl CheckConn for FireBirdDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let mut conn = self.get_conn()?;
        let now = std::time::Instant::now();
        let _: Option<Row> =
            block_in_place(|| conn.query_first("SELECT 1 FROM RDB$DATABASE;", ()))?;

        anyhow::Ok(now.elapsed())
    }
}

impl FireBirdDB {
    pub fn new(config: FireBirdConfig) -> anyhow::Result<Self> {
        let pool = r2d2::Pool::builder().build(PoolManager {
            config: config.clone(),
        })?;

        anyhow::Ok(Self {
            config,
            pool,
            query_connections: Default::default(),
        })
    }
    pub(crate) fn get_conn(&self) -> anyhow::Result<r2d2::PooledConnection<PoolManager>> {
        let conn = block_in_place(|| self.pool.get())?;
        anyhow::Ok(conn)
    }
}

pub(crate) fn gen_show_sql(
    table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize,
) -> String {
    let mut sql = format!(r#"SELECT * FROM "{table_name}""#);
    if let Some(where_str) = where_str {
        sql.push_str(format!(" WHERE {where_str} ").as_str());
    }
    if let Some(order_by) = order_by {
        sql.push_str(format!(" ORDER BY {order_by} ").as_str());
    }
    let offset = (page - 1) * page_size;
    sql.push_str(format!(" ROWS {} TO {} ", offset + 1, offset + page_size + 1).as_str());
    sql
}

#[cfg(test)]
fn make_test_object() -> FireBirdDB {
    FireBirdDB::new(FireBirdConfig {
        host: "localhost".to_string(),
        port: 3050,
        username: "SYSDBA".to_string(),
        password: "root".to_string(),
        db: "/var/lib/firebird/data/test.fdb".to_string(),
        read_only: false,
    })
    .unwrap()
}

#[cfg(test)]

mod tests {
    use std::hash::Hash;

    use crate::enable_test_log;

    use super::*;
    use Queryable;
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_check_conn() {
        let db = make_test_object();
        let d = crate::db::CheckConn::check_conn(&db).await.unwrap();
        println!("ping time:{:?}", d);
    }

    #[test]

    fn xxx() {
        let mut conn = builder_pure_rust()
            .host("localhost")
            .user("SYSDBA")
            .pass("root")
            .db_name("/var/lib/firebird/data/test.fdb")
            .connect()
            .unwrap();
        let ret = conn
            .query_iter::<'_, _, Row>("SELECT 1 FROM RDB$DATABASE", ())
            .unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_table() {
        enable_test_log();
        let table_name = "TEST_TABLE";
        let db = make_test_object();
        let _err = db.drop_table(table_name, false).await;
        if let Err(err) = _err {
            println!("drop table err:{}", err.to_string());
        }
        let query_id = db.open_connection().await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE TEST_TABLE (
                ID  INTEGER primary key,
                COL_INTEGER INTEGER,
                col_smallint SMALLINT,
                col_bigint BIGINT,
                col_float FLOAT,
                col_double_precision DOUBLE PRECISION,
                col_decimal DECIMAL(10, 2),
                col_numeric NUMERIC(10, 2)
            )
        "#,
        )
        .await
        .unwrap();
        let mut values = HashMap::new();
        values.insert("ID".to_string(), Some("1".to_string()));
        values.insert("COL_INTEGER".to_string(), Some("123".to_string()));
        values.insert("COL_SMALLINT".to_string(), Some("45".to_string()));

        values.insert("COL_BIGINT".to_string(), Some("9876543210".to_string()));

        values.insert("COL_FLOAT".to_string(), Some("3.14".to_string()));

        values.insert(
            "COL_DOUBLE_PRECISION".to_string(),
            Some("2.718281828459045".to_string()),
        );

        values.insert("COL_DECIMAL".to_string(), Some("123.45".to_string()));
        values.insert("COL_NUMERIC".to_string(), Some("678.90".to_string()));
        db.insert(table_name, values).await.unwrap();

        {
            let resp = db
                .show_table(table_name, Some("1 =1 "), Some("ID asc "), 1, 100, None)
                .await
                .unwrap();
            println!("resp:{:?}", resp);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_table2() {
        let table_name = "TEST_TABLE2";
        let db = make_test_object();
        let _err = db.drop_table(table_name, false).await;
        if let Err(err) = _err {
            println!("drop table err:{}", err.to_string());
        }
        let query_id = db.open_connection().await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE TEST_TABLE2 (
                id  INTEGER primary key,
                col_char CHAR(10),
                col_varchar VARCHAR(50),
                col_blob_text BLOB SUB_TYPE TEXT CHARACTER SET UTF8,
                col_blob_binary BLOB SUB_TYPE BINARY,
                col_date DATE,
                col_time TIME,
                col_timestamp TIMESTAMP,
                col_boolean BOOLEAN
            );
        "#,
        )
        .await
        .unwrap();
        let mut values = HashMap::new();
        values.insert("id".to_string(), Some("1".to_string()));
        values.insert("col_char".to_string(), Some("FixedText".to_string()));
        values.insert(
            "col_varchar".to_string(),
            Some("Variable Length Text".to_string()),
        );
        values.insert(
            "col_blob_text".to_string(),
            Some("This is a text BLOB".to_string()),
        );
        values.insert("col_date".to_string(), Some("2025-03-17".to_string()));
        values.insert("col_time".to_string(), Some("14:30:00".to_string()));
        values.insert(
            "col_timestamp".to_string(),
            Some("2025-03-17 14:30:00".to_string()),
        );
        values.insert("col_boolean".to_string(), Some("TRUE".to_string()));
        db.insert(table_name, values).await.unwrap();
    }
}
