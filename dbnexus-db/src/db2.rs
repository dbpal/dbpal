#![allow(unused)]
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use dashmap::DashMap;
use futures_util::io::Close;
use gcp_bigquery_client::google::cloud::bigquery::storage::v1::read_rows_response::Schema;
use r2d2::{Pool, PooledConnection};
use tokio::task::spawn_blocking;

use crate::{
    datasource_config::{Db2Config, OdbcCommon},
    db::{
        BackendKind, CheckConn, CreateSchemaExtra, DBTrait, ExportFrame, ExportKind,
        ExportQueryResult, Field, GetBackendKind, ListTable, MetaValue, POOL_MAX_CONNECTIONS,
        QueryEventEmitter, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas,
        ShowTableParameter, SqlExecuteRet, not_support_feature,
    },
    odbc::{ODBC_ENV, OdbcDbQuery, OdbcHelper, OdbcPool},
};

impl GetBackendKind for Db2Db {
    fn backend_name(&self) -> BackendKind {
        BackendKind::Db2
    }
}

#[derive(Clone)]
pub struct Db2Db {
    config: Db2Config,
    pool: Arc<Pool<OdbcPool>>,
    query_connection: Arc<DashMap<String, OdbcDbQuery>>,
}

impl ReadOnlyTrait for Db2Db {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

impl Db2Db {
    pub(crate) fn get_query_conn(&self, query_id: &str) -> anyhow::Result<OdbcDbQuery> {
        match self.query_connection.get(&query_id.to_string()) {
            Some(x) => anyhow::Ok(x.clone()),
            None => anyhow::bail!("query connection closed???"),
        }
    }
}

#[async_trait::async_trait]

impl QuerySql for Db2Db {
    async fn get_auto_commit(&self, query_id: &str) -> anyhow::Result<bool> {
        let conn = self.get_query_conn(query_id)?;
        let r = conn.get_auto_commit();
        anyhow::Ok(r)
    }

    async fn set_auto_commit(&self, query_id: &str, commit: bool) -> anyhow::Result<()> {
        let conn = self.get_query_conn(query_id)?;
        conn.set_auto_commit(commit).await;
        anyhow::Ok(())
    }
    async fn commit(&self, query_id: &str) -> anyhow::Result<()> {
        let conn = self.get_query_conn(query_id)?;
        conn.commit().await;
        anyhow::Ok(())
    }
    async fn rollback(&self, query_id: &str) -> anyhow::Result<()> {
        let conn = self.get_query_conn(query_id)?;
        conn.rollback().await;
        anyhow::Ok(())
    }

    async fn query_sql(
        &self, query_id: &str, sql: &str, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let conn = self.get_query_conn(query_id)?;
        conn.query_sql(sql, emit).await?;
        anyhow::Ok(())
    }

    async fn execute_sql(
        &self, query_id: &str, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let conn = self.get_query_conn(query_id)?;
        let r = conn.execute_sql(sql).await?;
        anyhow::Ok(r)
    }

    async fn execute_sql_no_open_conn(
        &self, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let this = Clone::clone(self);
        let sql = sql.to_owned();
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let r = conn.execute(&sql, (), None);
            anyhow::Ok(match r {
                Ok(_) => todo!(),
                Err(err) => Err(err.to_string()),
            })
        })
        .await?
    }

    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let id = id.to_owned();
        spawn_blocking(move || {
            let conn = crate::odbc::make_conn(&this.config.make_odbc_str())?;
            this.query_connection.insert(
                id,
                OdbcDbQuery::new(
                    conn,
                    Arc::new(Db2OdbcHelper {
                        config: this.config.clone(),
                    }),
                ),
            );
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
    pub(crate) fn new(config: Db2Config) -> anyhow::Result<Self> {
        let conn_str = config.make_odbc_str();
        let mut builder = r2d2::Pool::<OdbcPool>::builder();
        builder = builder.max_size(POOL_MAX_CONNECTIONS);
        if let Some(timeout) = config.conn_timeout {
            builder = builder.connection_timeout(timeout.into());
        }
        builder = builder.min_idle(Some(5));
        let pool = builder.build(OdbcPool {
            conn_str,
            select_1_sql: SELECT_1_SQL.to_string(),
        })?;
        anyhow::Ok(Db2Db {
            config,
            pool: Arc::new(pool),
            query_connection: Default::default(),
        })
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

        format!(
            "Driver={{{driver_name}}};Database={db};Hostname={host};Port={port};Protocol=TCPIP;UID={username};PWD={password};CodePage=1208"
        )
    }
}

#[async_trait::async_trait]

impl DBTrait for Db2Db {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        todo!()
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        todo!()
    }

    async fn create_schema(
        &self, name: &str, extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        todo!()
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        todo!()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        todo!()
    }
}

impl Db2Db {
    pub(crate) fn get_conn(&self) -> anyhow::Result<PooledConnection<OdbcPool>> {
        let conn = self.pool.get()?;
        anyhow::Ok(conn)
    }
}

#[async_trait::async_trait]

impl CheckConn for Db2Db {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let now = std::time::Instant::now();
            let conn = this.get_conn()?;
            conn.execute(SELECT_1_SQL, (), Some(5))?;
            anyhow::Ok(now.elapsed())
        })
        .await?
    }
}

#[cfg(test)]

fn make_test_object() -> Db2Db {
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
        save_password: true,
        read_only: false,
        conn_timeout: Some(5000.into()),
    })
    .unwrap()
}

#[async_trait::async_trait]

impl ExportQueryResult for Db2Db {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        todo!()
    }

    async fn export_query_result(
        &self, query_id: &str, _sql: &str, _kind: ExportKind,
        _sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        todo!()
    }
}

#[async_trait::async_trait]

impl ServerMetas for Db2Db {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        todo!()
    }
}

impl std::fmt::Debug for Db2Db {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

#[async_trait::async_trait]

impl SchemaTrait for Db2Db {
    fn as_any(&self) -> &dyn std::any::Any {
        todo!()
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
}

pub struct Db2OdbcHelper {
    config: Db2Config,
}

impl OdbcHelper for Db2OdbcHelper {
    fn get_max_str_len(&self) -> usize {
        self.config.odbc_common.max_str_len
    }

    fn get_max_bin_len(&self) -> usize {
        self.config.odbc_common.max_bin_len
    }

    fn format_timestamp(&self, x: &odbc_api::sys::Timestamp) -> String {
        todo!()
    }

    fn format_bit(&self, x: &odbc_api::Bit) -> String {
        todo!()
    }

    fn format_binary(&self, x: &[u8]) -> String {
        todo!()
    }

    fn format_numeric(&self, x: &odbc_api::sys::Numeric) -> String {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::TestQueryEventEmit;
    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object();
        crate::db::CheckConn::check_conn(&db).await.unwrap();
    }

    #[tokio::test]
    async fn test_query() {
        let db = make_test_object();
        crate::db::CheckConn::check_conn(&db).await.unwrap();
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
}
