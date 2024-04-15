use std::collections::HashMap;

use crate::datasource::{DBConfig, OracleDBConfig};
use crate::db::{
    DBTrait, ExeSqlError, Field, Index, ListTable, ListView, QuerySql, SqlExecuteRet, TableListResp,
};
use crate::pool::{NexusPool, PoolManager};

#[derive(Clone)]
pub struct OracleDB {
    config: OracleDBConfig,
    pm: PoolManager,
}

#[async_trait::async_trait]
impl QuerySql for OracleDB {
    async fn query_sql(
        &self, _sql: &str,
    ) -> anyhow::Result<Vec<Result<TableListResp, ExeSqlError>>> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl DBTrait for OracleDB {
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
        unimplemented!()
    }

    async fn show_columns(&self, _table_name: &str) -> anyhow::Result<Vec<Field>> {
        unimplemented!()
    }

    async fn show_table(
        &self, _table_name: &str, _where_str: Option<&str>, _order_by: Option<&str>, _page: usize,
        _page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        unimplemented!()
    }

    async fn show_indexes(&self, _table_name: &str) -> anyhow::Result<Vec<Index>> {
        unimplemented!()
    }

    async fn insert(
        &self, _table_name: &str, _values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        unimplemented!()
    }

    async fn update(
        &self, _table_name: &str, _values: HashMap<String, Option<String>>,
        _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        unimplemented!()
    }

    /// format a value make it suitable for insert into sql.
    /// this method current use in `insert_sql` etc.
    fn put_value_in_sql(&self, _f: &Field, _value: Option<&str>) -> anyhow::Result<String> {
        unreachable!("I don't need impl this this is a internal bug")
    }

    async fn truncate_table(&self, _table_name: &str) -> anyhow::Result<()> {
        unimplemented!()
    }

    async fn delete(
        &self, _table_name: &str, _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
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

impl OracleDB {
    async fn get_conn(&self) -> anyhow::Result<oracle::Connection> {
        let pool = match self.pm.get(&DBConfig::Oracle(self.config.clone())).await? {
            NexusPool::Oracle(p) => p.clone(),
            _ => unreachable!(),
        };
        let conn = tokio::task::block_in_place(|| pool.get())?;
        return anyhow::Ok(conn);
    }
}
