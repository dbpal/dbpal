#![allow(unused)]
use std::{
    collections::HashMap,
    ffi::CStr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use crate::{
    datasource_config::{Db2Config, OdbcCommon},
    db::{
        BackendKind, CatalogTrait, CheckConn, CreateSchemaExtra, DBTrait, ExportFrame, ExportKind,
        ExportQueryResult, Field, GetBackendKind, ListTable, MetaValue, POOL_MAX_CONNECTIONS,
        QueryEventEmitter, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas,
        ShowTableParameter, SqlExecuteRet, not_support_feature,
    },
};
use dashmap::DashMap;
use futures_util::{io::Close, stream::Cycle};
use gcp_bigquery_client::google::cloud::bigquery::storage::v1::read_rows_response::Schema;
use ibm_db::{
    Allocated, ColumnDescriptor, Cursor, ODBCConnectionManager, ResultSetState, SqlDataType,
    Statement, safe::AutocommitOn,
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

pub struct Db2QueryConn {
    conn: Arc<Mutex<PooledConnection<ODBCConnectionManager>>>,
    commit_mode: AtomicBool,
}

impl Db2QueryConn {
    fn new(conn: PooledConnection<ODBCConnectionManager>) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
            commit_mode: AtomicBool::new(true),
        }
    }
}

impl ReadOnlyTrait for Db2Db {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

impl Db2Db {
    pub(crate) fn get_query_conn(&self, query_id: &str) -> anyhow::Result<Db2QueryConn> {
        unimplemented!()
    }
}

fn encode_row(
    cols: &[ColumnDescriptor], batch: &mut Cursor<'_, '_, '_, Allocated, AutocommitOn>,
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
            SqlDataType::SQL_INTEGER => todo!(),
            SqlDataType::SQL_SMALLINT => todo!(),
            SqlDataType::SQL_FLOAT => todo!(),
            SqlDataType::SQL_REAL => todo!(),
            SqlDataType::SQL_DOUBLE => todo!(),
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
        &self, query_id: &str, sql: &str, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let sql = sql.to_owned();
        let this = Clone::clone(self);
        let query_id = query_id.to_owned();
        spawn_blocking(move || {
            let conn = this.get_query_conn(&query_id)?;
            let conn = conn.conn.lock().expect("lock failed");
            let stmt = Statement::with_parent(conn.raw()).map_err(convert_db2_err)?;
            let mut cursor = stmt.exec_direct(&sql).map_err(convert_db2_err)?;
            match cursor {
                ResultSetState::Data(mut statement) => {
                    let mut fields = vec![];
                    let mut cols = vec![];
                    for index in 1..statement.num_result_cols()? + 1 {
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
        &self, query_id: &str, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let sql = sql.to_owned();
        let this = Clone::clone(self);
        let query_id = query_id.to_owned();
        spawn_blocking(move || {
            let conn = this.get_query_conn(&query_id)?;
            let conn = conn.conn.lock().expect("lock failed");
            let stmt = Statement::with_parent(conn.raw()).map_err(convert_db2_err)?;
            let r = match stmt.exec_direct(&sql) {
                Ok(_) => Ok(QueryResp::execute_ok(None)),
                Err(err) => Err(err.to_string()),
            };
            anyhow::Ok(r)
        })
        .await?
    }

    async fn execute_sql_no_open_conn(
        &self, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
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
            let query_conn = Db2QueryConn::new(conn);
            this.query_connection.insert(id, query_conn);
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

#[async_trait::async_trait]
impl DBTrait for Db2Db {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let sql = "select schemaname from syscat.schemata";
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let stmt = Statement::with_parent(conn.raw()).map_err(convert_db2_err)?;
            let cursor = stmt.exec_direct(&sql).map_err(convert_db2_err)?;
            let ret = match cursor {
                ResultSetState::Data(mut statement) => {
                    let mut ret = vec![];
                    while let Some(mut cursor) = statement.fetch().unwrap() {
                        match cursor.get_data::<&str>(1)? {
                            Some(name) => {
                                ret.push(name.trim().to_owned());
                            }
                            None => {}
                        }
                    }
                    ret
                }
                ResultSetState::NoData(statement) => vec![],
            };
            anyhow::Ok(ret)
        })
        .await?
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut config = self.config.clone();
        config.schema = Some(name.to_owned());
        Ok(Arc::new(Db2Db::new(config).await?))
    }

    async fn create_schema(
        &self, name: &str, extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        let sql = format!("CREATE TABLESPACE {}", name);
        self.execute(sql).await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!("DROP TABLESPACE {}", name);
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
        schema: None,
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
        &self, query_id: Option<&str>, _sql: &str, _kind: ExportKind,
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

#[async_trait::async_trait]

impl SchemaTrait for Db2Db {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        todo!()
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        todo!()
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        todo!()
    }

    async fn show_table(
        &self, table_name: &str, _: ShowTableParameter, _: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        todo!()
    }

    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        todo!()
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        todo!()
    }

    async fn table_delete(
        &self, _table_name: &str, _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        todo!()
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        todo!()
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        todo!()
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        todo!()
    }

    fn create_table_sql(&self, req: &crate::db::CreateTableReq) -> anyhow::Result<Vec<String>> {
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
        let db = make_test_object().await;
        let s = db.list_schema().await.unwrap();
        println!("schema:{:?}", s);
    }
}
