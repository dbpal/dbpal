#![allow(unused)]
use std::{
    collections::HashMap,
    ffi::CStr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use crate::{
    datasource_config::{Db2Config, OdbcCommon},
    db::{
        not_support_feature, BackendKind, CatalogTrait, CheckConn, CreateSchemaExtra, DBTrait,
        ExportFrame, ExportKind, ExportQueryResult, Field, GetBackendKind, ListTable, MetaValue,
        QueryEventEmitter, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas,
        ShowTableParameter, SqlBuilder, SqlExecuteRet, POOL_MAX_CONNECTIONS,
    },
};
use dashmap::DashMap;
use futures_util::{io::Close, stream::Cycle};
use gcp_bigquery_client::google::cloud::bigquery::storage::v1::read_rows_response::Schema;
use ibm_db::{
    safe::AutocommitOn, Allocated, ColumnDescriptor, Cursor, ODBCConnectionManager, ResultSetState,
    SqlDataType, Statement, SSHORT,
};
use r2d2::{Pool, PooledConnection};
use std::ops::Deref;
use tokio::task::spawn_blocking;

impl GetBackendKind for Db2Db {
    fn backend_name(&self) -> BackendKind {
        BackendKind::Db2
    }
}

#[derive(Clone)]
pub struct Db2Db {
    config: Db2Config,
    pool: Arc<Pool<ODBCConnectionManager>>,
    query_connection: Arc<DashMap<String, Db2QueryConn>>,
}

pub type Db2QueryConn = Arc<Mutex<PooledConnection<ODBCConnectionManager>>>;

impl ReadOnlyTrait for Db2Db {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

impl Db2Db {
    pub(crate) fn get_query_conn(&self, query_id: &str) -> anyhow::Result<Db2QueryConn> {
        match self.query_connection.get(&query_id.to_string()) {
            Some(x) => anyhow::Ok(x.clone()),
            None => anyhow::bail!(format!("query conn '{}' not found", query_id)),
        }
    }
}

// https://www.ibm.com/docs/en/db2-for-zos/13.0.0?topic=attributes-length-sql-data-types
fn encode_row(
    cols: &[ColumnDescriptor],
    batch: &mut Cursor<'_, '_, '_, Allocated, AutocommitOn>,
) -> anyhow::Result<Vec<Option<String>>> {
    let mut ret = Vec::with_capacity(cols.len());
    for i in 0..cols.len() {
        let index = (i + 1) as u16;

        let v: Option<String> = match &cols[i].data_type {
            SqlDataType::SQL_UNKNOWN_TYPE => todo!(),
            SqlDataType::SQL_CHAR => {
                let s: Option<String> = batch.get_data(index)?;
                s.map(|s| s.trim().to_string())
            }
            SqlDataType::SQL_NUMERIC => todo!(),
            SqlDataType::SQL_DECIMAL => todo!(),
            SqlDataType::SQL_INTEGER => {
                let s: Option<i32> = batch.get_data(index)?;
                s.map(|x| x.to_string())
            }
            SqlDataType::SQL_SMALLINT => {
                let s: Option<i16> = batch.get_data(index)?;
                s.map(|x| x.to_string())
            }
            SqlDataType::SQL_FLOAT => {
                let s: Option<f64> = batch.get_data(index)?;
                s.map(|x| x.to_string())
            }
            SqlDataType::SQL_REAL => todo!(),
            SqlDataType::SQL_DOUBLE => {
                let s: Option<f64> = batch.get_data(index)?;
                s.map(|x| x.to_string())
            }
            SqlDataType::SQL_DATETIME => todo!(),
            SqlDataType::SQL_VARCHAR => {
                let s: Option<String> = batch.get_data(index)?;
                s
            }
            SqlDataType::SQL_DATE => todo!(),
            SqlDataType::SQL_TIME => todo!(),
            SqlDataType::SQL_TIMESTAMP => todo!(),
            SqlDataType::SQL_EXT_TIME_OR_INTERVAL => todo!(),
            SqlDataType::SQL_EXT_TIMESTAMP => todo!(),
            SqlDataType::SQL_EXT_LONGVARCHAR => todo!(),
            SqlDataType::SQL_EXT_BINARY => todo!(),
            SqlDataType::SQL_EXT_VARBINARY => todo!(),
            SqlDataType::SQL_EXT_LONGVARBINARY => todo!(),
            SqlDataType::SQL_EXT_BIGINT => todo!(),
            SqlDataType::SQL_EXT_TINYINT => todo!(),
            SqlDataType::SQL_EXT_BIT => todo!(),
            SqlDataType::SQL_EXT_WCHAR => {
                let s: Option<String> = batch.get_data(index)?;
                s.map(|s| s.trim().to_string())
            }
            SqlDataType::SQL_EXT_WVARCHAR => {
                let s: Option<String> = batch.get_data(index)?;
                s
            }
            SqlDataType::SQL_EXT_WLONGVARCHAR => {
                let s: Option<String> = batch.get_data(index)?;
                s
            }
            SqlDataType::SQL_EXT_GUID => todo!(),
            SqlDataType::SQL_SS_VARIANT => todo!(),
            SqlDataType::SQL_SS_UDT => todo!(),
            SqlDataType::SQL_SS_XML => todo!(),
            SqlDataType::SQL_SS_TABLE => todo!(),
            SqlDataType::SQL_SS_TIME2 => todo!(),
            SqlDataType::SQL_SS_TIMESTAMPOFFSET => todo!(),
        };
        ret.push(v);
    }
    anyhow::Ok(ret)
}
#[async_trait::async_trait]

impl QuerySql for Db2Db {
    async fn query_sql(
        &self,
        query_id: &str,
        sql: &str,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let sql = sql.to_owned();
        let this = Clone::clone(self);
        let query_id = query_id.to_owned();
        spawn_blocking(move || {
            let conn = this.get_query_conn(&query_id)?;
            let conn = conn.lock().expect("lock failed");
            let stmt = Statement::with_parent(conn.raw()).map_err(convert_db2_err)?;
            let mut cursor = stmt.exec_direct(&sql).map_err(convert_db2_err)?;
            match cursor {
                ResultSetState::Data(mut statement) => {
                    let mut fields = vec![];
                    let mut cols = vec![];
                    for index in 0..statement.num_result_cols()? {
                        let index = index + 1;
                        let col = statement.describe_col(index as u16)?;
                        fields.push(Field::from_name_type(
                            col.name.clone(),
                            format!("{:?}", col.data_type),
                        ));
                        cols.push(col);
                    }
                    emit.send(
                        QueryResp {
                            fields,
                            ..Default::default()
                        }
                        .into(),
                    )?;
                    while let Some(mut batch) = statement.fetch().map_err(convert_db2_err)? {
                        let d = encode_row(&cols, &mut batch)?;
                        emit.send(d.into())?;
                    }
                }

                ResultSetState::NoData(statement) => {
                    emit.send(
                        QueryResp {
                            ..Default::default()
                        }
                        .into(),
                    )?;
                }
            }
            anyhow::Ok(())
        })
        .await?
    }

    async fn execute_sql(
        &self,
        query_id: &str,
        sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let sql = sql.to_owned();
        let this = Clone::clone(self);
        let query_id = query_id.to_owned();
        spawn_blocking(move || {
            let conn = this.get_query_conn(&query_id)?;
            let conn = conn.lock().expect("lock failed");
            let stmt = Statement::with_parent(conn.raw()).map_err(convert_db2_err)?;
            let r = match stmt.exec_direct(&sql) {
                Ok(_) => Ok(QueryResp::execute_ok(None)),
                Err(err) => Err(err.to_string()),
            };
            anyhow::Ok(r)
        })
        .await?
    }

    async fn simple_execute_sql(&self, sql: &str) -> anyhow::Result<Result<QueryResp, String>> {
        match self.execute(sql).await {
            Ok(x) => anyhow::Ok(Ok(QueryResp::execute_ok(sql.to_string()))),
            Err(err) => anyhow::Ok(Err(err.to_string())),
        }
    }

    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        let id = id.to_owned();
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.pool.get()?;
            this.query_connection.insert(id, Arc::new(Mutex::new(conn)));
            anyhow::Ok(())
        })
        .await?
    }

    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        self.query_connection.remove(&id.to_string());
        anyhow::Ok(())
    }
}

const SELECT_1_SQL: &str = "SELECT 1 FROM SYSIBM.SYSDUMMY1";

impl Db2Db {
    pub async fn new(config: Db2Config) -> anyhow::Result<Self> {
        spawn_blocking(move || {
            let conn_str = config.make_odbc_str();
            let mut builder = r2d2::Pool::<ODBCConnectionManager>::builder();
            builder = builder.max_size(POOL_MAX_CONNECTIONS);
            if let Some(timeout) = config.conn_timeout {
                builder = builder.connection_timeout(timeout.into());
            }
            builder = builder.min_idle(Some(0));
            let pool = builder.build(ODBCConnectionManager::new(conn_str))?;
            anyhow::Ok(Db2Db {
                config,
                pool: Arc::new(pool),
                query_connection: Default::default(),
            })
        })
        .await?
    }
}

impl Db2Config {
    fn make_odbc_str(&self) -> String {
        let Db2Config {
            odbc_common: OdbcCommon { driver_name, .. },
            host,
            port,
            username,
            password,
            db,
            ..
        } = self;

        let mut ret = format!(
            "Driver={{{driver_name}}};Hostname={host};Port={port};Protocol=TCPIP;UID={username};PWD={password};CodePage=1208;Database={db};"
        );
        #[cfg(debug_assertions)]
        println!("conn str:{}", ret);
        ret
    }
}

impl Db2Db {
    pub(crate) async fn execute(&self, sql: impl Into<String>) -> anyhow::Result<()> {
        let sql = sql.into();
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let stmt = Statement::with_parent(conn.raw()).map_err(convert_db2_err)?;
            stmt.exec_direct(&sql).map_err(convert_db2_err)?;
            anyhow::Ok(())
        })
        .await?
    }
}

impl Db2Db {
    pub(crate) async fn list_schema_impl(&self) -> anyhow::Result<Vec<String>> {
        let sql = "select schemaname from syscat.schemata";

        let r = self
            .query_something(sql, |b| {
                let x: Option<String> = b.get_data(1)?;
                anyhow::Ok(x)
            })
            .await?;
        anyhow::Ok(r.into_iter().flatten().collect())
    }
}

#[async_trait::async_trait]
impl DBTrait for Db2Db {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        self.list_schema_impl().await
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut config = self.config.clone();
        config.schema = Some(name.to_owned());
        Ok(Arc::new(Db2Db::new(config).await?))
    }

    async fn create_schema(
        &self,
        name: &str,
        extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        let sql = format!("CREATE SCHEMA {}", name);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!("DROP SCHEMA {}", name);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

pub(crate) fn convert_db2_err(x: Box<dyn std::error::Error>) -> anyhow::Error {
    anyhow::Error::msg(x.to_string())
}

impl Db2Db {
    pub(crate) fn get_conn(&self) -> anyhow::Result<PooledConnection<ODBCConnectionManager>> {
        let conn = self.pool.get()?;
        if let Some(schema) = self.config.schema.as_ref() {
            let sql = format!("set schema {}", schema);
            let stmt = Statement::with_parent(conn.raw()).map_err(convert_db2_err)?;
            stmt.exec_direct(&sql).map_err(convert_db2_err)?;
        }
        anyhow::Ok(conn)
    }
}

#[async_trait::async_trait]

impl CheckConn for Db2Db {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let stmt = Statement::with_parent(conn.raw()).map_err(convert_db2_err)?;
            let now = std::time::Instant::now();
            stmt.exec_direct(SELECT_1_SQL).map_err(convert_db2_err)?;
            anyhow::Ok(now.elapsed())
        })
        .await?
    }
}

#[cfg(test)]
async fn make_test_object() -> Db2Db {
    use crate::TEST_HOST;
    Db2Db::new(Db2Config {
        odbc_common: OdbcCommon {
            driver_name: "Driver_IBM_DB2".to_string(),
            max_str_len: 4096,
            max_bin_len: 4096,
        },
        host: TEST_HOST.to_string(),
        port: 50000,
        username: "db2inst1".to_string(),
        password: "db2inst1-pwd".to_string(),
        db: "TSKDB".to_string(),
        read_only: false,
        conn_timeout: Some(5000.into()),
        schema: Some("TEST".to_string()),
    })
    .await
    .unwrap()
}

#[async_trait::async_trait]

impl ExportQueryResult for Db2Db {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        todo!()
    }

    async fn export_query_result(
        &self,
        query_id: Option<&str>,
        _sql: &str,
        _kind: ExportKind,
        _sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        todo!()
    }
}

#[async_trait::async_trait]

impl ServerMetas for Db2Db {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        anyhow::Ok(vec![])
    }
}

impl std::fmt::Debug for Db2Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "db2@{:?}", self.config)
    }
}

impl Db2Db {
    pub(crate) async fn query_something<R: Default + Send + Sync + 'static>(
        &self,
        sql: impl Into<String>,
        mut extract_value: impl FnMut(&mut Cursor<'_, '_, '_, Allocated, AutocommitOn>) -> anyhow::Result<R>
            + Send
            + Sync
            + 'static,
    ) -> anyhow::Result<Vec<R>> {
        let this = Clone::clone(self);
        let sql = sql.into();
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let stmt = Statement::with_parent(conn.raw()).map_err(convert_db2_err)?;
            let cursor = stmt.exec_direct(&sql).map_err(convert_db2_err)?;
            match cursor {
                ResultSetState::Data(mut statement) => {
                    #[cfg(debug_assertions)]
                    {
                        let num_result_cols = statement.num_result_cols()?;
                        println!(
                            "################################################# {}",
                            num_result_cols
                        );
                        for idx in 0..num_result_cols {
                            let idx = idx + 1;
                            println!("col #{}", idx);
                            let d = statement.describe_col(idx as u16).unwrap();
                            println!("col #{} {:?}", idx, d);
                        }
                    }
                    let mut ret = Vec::new();
                    while let Some(mut batch) = statement.fetch().map_err(convert_db2_err)? {
                        let r = extract_value(&mut batch)?;
                        ret.push(r);
                    }
                    anyhow::Ok(ret)
                }
                ResultSetState::NoData(statement) => anyhow::Ok(Default::default()),
            }
        })
        .await?
    }
}

impl Db2Db {
    fn query_count(&self, table: &str, where_str: Option<&str>) -> anyhow::Result<i64> {
        let mut sql = format!(
            r#"select count(*) from "{}"."{}" "#,
            self.config.schema_str(),
            table
        );
        if let Some(w) = where_str {
            sql.push_str(format!(" where {}", w).as_str());
        }
        let conn = self.get_conn()?;
        let stmt = Statement::with_parent(conn.raw()).map_err(convert_db2_err)?;
        let cursor = stmt.exec_direct(&sql).map_err(convert_db2_err)?;
        match cursor {
            ResultSetState::Data(mut statement) => {
                let batch = statement.fetch().map_err(convert_db2_err)?;
                let mut batch = match batch {
                    Some(x) => x,
                    None => anyhow::bail!("count(*) should not return NoData"),
                };
                let r: Option<i64> = batch.get_data(1)?;
                anyhow::Ok(r.unwrap_or_default())
            }
            ResultSetState::NoData(statement) => {
                anyhow::bail!("count(*) should not return NoData")
            }
        }
    }
}

impl Db2Db {
    pub(crate) async fn list_table_impl(&self, schema: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"
    SELECT TABNAME 
    FROM SYSCAT.TABLES WHERE TABSCHEMA = '{}'   AND TYPE = 'T';
        "#,
            schema
        );
        let r = self
            .query_something(sql, |b| {
                let name: Option<String> = b.get_data(1)?;
                anyhow::Ok(name)
            })
            .await?;
        let r: Vec<_> = r.into_iter().flatten().map(ListTable::from).collect();
        anyhow::Ok(r)
    }
}

#[async_trait::async_trait]

impl SchemaTrait for Db2Db {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        let sql = format!(r#"CREATE TABLE "{}" AS SELECT * FROM "{}""#, dup, table);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.config.schema_str()).await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let sql = format!(
            r#"
        SELECT 
            C.COLNAME,
            C.TYPENAME,
            C.NULLS,
            C.DEFAULT
        FROM 
            SYSCAT.COLUMNS C
        JOIN 
            SYSCAT.TABLES T ON C.TABNAME = T.TABNAME AND C.TABSCHEMA = T.TABSCHEMA
        WHERE 
            T.TABSCHEMA = '{}' AND
            T.TABNAME = '{}'
        ORDER BY 
            C.COLNO;
        "#,
            self.config.schema_str(),
            table_name
        );

        self.query_something(sql, |x| -> anyhow::Result<Field> {
            let name: Option<String> = x.get_data(1)?;
            let data_type: Option<String> = x.get_data(2)?;

            unimplemented!()
        })
        .await?;

        unimplemented!()
    }

    async fn show_table(
        &self,
        table_name: &str,
        _: ShowTableParameter,
        _: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn table_insert(
        &self,
        table_name: &str,
        values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = self.insert_sql(self.config.schema.as_deref(), table_name, &fs, &values)?;
        self.execute(&sql).await?;
        anyhow::Ok(SqlExecuteRet { sql, changes: None })
    }

    async fn table_update(
        &self,
        table_name: &str,
        values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = self.update_sql(
            self.config.schema.as_deref(),
            table_name,
            &fs,
            &values,
            &wheres,
        )?;
        self.execute(&sql).await?;
        anyhow::Ok(SqlExecuteRet { sql, changes: None })
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<String> {
        let sql = format!(r#"TRUNCATE TABLE  "{table_name}" "#);
        self.execute(sql).await
    }

    async fn table_delete(
        &self,
        table_name: &str,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = self.delete_sql(self.config.schema.as_deref(), table_name, &fs, &wheres)?;
        self.execute(&sql).await?;
        anyhow::Ok(SqlExecuteRet { sql, changes: None })
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<String> {
        let sql = format!(
            r#"DROP TABLE "{}"."{}" {}"#,
            self.config.schema_str(),
            table,
            if cascade { "CASCADE" } else { "" }
        );
        println!("sql:{}", sql);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        crate::language_server::LexerCompletionTrait::get_types(self).await
    }
}

impl SqlBuilder for Db2Db {
    fn put_value_in_sql(&self, _typ: &str, _value: Option<&str>) -> anyhow::Result<String> {
        todo!()
    }
}

pub struct Db2OdbcHelper {
    config: Db2Config,
}

// 是的，这个理解完全正确——无法通过标准 SQL 语句列出 DB2 实例下的所有数据库，必须使用命令行工具或实例级 API。
// 所以db2不实现catalogtrait

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::TestQueryEventEmit;
    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object().await;
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_query() {
        let db = make_test_object().await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.query_sql(
            query_id,
            SELECT_1_SQL,
            Arc::new(TestQueryEventEmit::default()),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_schema() {
        let name = "ABC";
        let db = make_test_object().await;
        let _err = db.create_schema(name, None).await;
        if let Err(_err) = _err {
            println!("create scheam err:{}", _err);
        }
        let s = db.list_schema().await.unwrap();
        println!("schema:{:?}", s);
        db.drop_schema(name).await;
    }

    #[tokio::test]

    async fn test_types1() {
        let table_name = "TEST_TYPES1";
        let query_id = "";
        let db = make_test_object().await;
        db.open_connection(query_id).await.unwrap();
        {
            let _err = db.drop_table(table_name, false).await;
            if let Err(_err) = _err {
                println!("drop table err:{}", _err);
            }
        }
        // db2 need schema for create table.
        db.execute_sql(
            query_id,
            r#"
        CREATE TABLE TEST.TEST_TYPES1 (
            ID INT,
            BIGINT_COLUMN BIGINT,
            BINARY_COLUMN BINARY(100),
            BLOB_COLUMN BLOB,
            BOOLEAN_COLUMN BOOLEAN,
            CHARACTER_COLUMN CHAR(100),
            CLOB_COLUMN CLOB
        )
"#,
        )
        .await
        .unwrap()
        .unwrap();

        for t in db.list_table().await.unwrap() {
            let cols = db.show_table_columns(&t.name).await.unwrap();
            println!("##################################################");
            for col in cols.iter() {
                println!("cols{:?}", col);
            }
        }
    }
}
