#![allow(unused)]
use std::{collections::HashMap, sync::Arc, time::Duration};

use cassandra_cpp::{AsRustType, Cluster, LendingIterator, Session};
use futures_util::TryFutureExt;

use crate::{
    datasource_config::CassandraDBConfig,
    db::{
        BackendName, CheckConn, CreateSchemaExtra, DBTrait, Field, Index, ListTable, QueryResp,
        QuerySql, SchemaTrait, ServerMetas, SqlExecuteRet, TableListResp, not_support_features,
    },
    meta_value::MetaValue,
};

#[derive(Clone)]
pub(crate) struct CassandraDB {
    session: Session,
    config: CassandraDBConfig,
}

impl CassandraDB {
    pub async fn new(config: CassandraDBConfig) -> anyhow::Result<Self> {
        let mut cluster = Cluster::default();
        cluster
            .set_contact_points("127.0.0.1")
            .map_err(err_to_anyhow)?;
        if config.username.is_some() && config.password.is_some() {
            cluster
                .set_credentials(
                    config.username.as_deref().unwrap(),
                    config.password.as_deref().unwrap(),
                )
                .map_err(err_to_anyhow)?;
        }
        if let Some(port) = config.port {
            cluster.set_port(port).map_err(err_to_anyhow)?;
        }

        let session = if let Some(db) = config.db.as_deref() {
            cluster.connect_keyspace(db).await.map_err(err_to_anyhow)?
        } else {
            cluster.connect().await.map_err(err_to_anyhow)?
        };
        anyhow::Ok(Self { config, session })
    }
}

fn err_to_anyhow(err: cassandra_cpp::Error) -> anyhow::Error {
    log::error!("cassandra error:{:?}", err);
    anyhow::Error::msg(err.to_string())
}

#[async_trait::async_trait]

impl ServerMetas for CassandraDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl CheckConn for CassandraDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        unimplemented!()
    }
}

impl std::fmt::Debug for CassandraDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "host={} port={:?}", self.config.host, self.config.port)
    }
}

impl BackendName for CassandraDB {
    fn backend_name(&self) -> &str {
        "cassandra"
    }
}
#[async_trait::async_trait]
impl QuerySql for CassandraDB {
    async fn query_sql(
        &self, _query_id: uuid::Uuid, _sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        unimplemented!()
    }
    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        anyhow::Ok(uuid::Uuid::new_v4())
    }

    async fn re_connect(&self, _id: uuid::Uuid) -> anyhow::Result<()> {
        anyhow::Ok(())
    }

    async fn close_connection(&self, _id: uuid::Uuid) -> anyhow::Result<()> {
        anyhow::Ok(())
    }

    fn is_dangerous_sql(&self, _sql: &str) -> anyhow::Result<bool> {
        unimplemented!()
    }
}

impl CassandraDB {
    async fn list_table_impl(&self, keyspace: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            "SELECT table_name FROM system_schema.tables WHERE keyspace_name = '{keyspace}';"
        );
        let rows = self.session.execute(sql).await.map_err(err_to_anyhow)?;
        let mut rows_iter = rows.iter();

        let mut ret = Vec::with_capacity(rows.row_count() as usize);

        while let Some(row) = rows_iter.next() {
            let name: String = row.get(0).map_err(err_to_anyhow)?;
            ret.push(name);
        }

        anyhow::Ok(ret.into_iter().map(|x| ListTable::from(x)).collect())
    }
}

#[async_trait::async_trait]

impl SchemaTrait for CassandraDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }
    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        unimplemented!()
    }
    fn quote_style(&self) -> &'static str {
        "\""
    }
    fn clone(&self) -> Box<dyn SchemaTrait> {
        unimplemented!()
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.config.schema_str()).await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        unimplemented!()
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        unimplemented!()
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        unimplemented!()
    }
    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(&table_name).await?;
        let sql = (self as &dyn SchemaTrait).insert_sql(None, table_name, &fs, &values)?;
        self.session.execute(&sql).await.map_err(err_to_anyhow)?;

        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: 1,
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }
    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fs = self.show_table_columns(table_name).await?;
        let sql = (self as &dyn crate::db::SchemaTrait).update_sql(
            self.config.db.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &values,
            &wheres,
            type_support_compare,
        )?;
        self.session.execute(&sql).await.map_err(err_to_anyhow)?;
        anyhow::Ok(SqlExecuteRet::from((sql, 1)))
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"TRUNCATE "{table_name}""#);
        self.session.execute(&sql).await.map_err(err_to_anyhow)?;
        anyhow::Ok(())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;

        let sql = (self as &dyn SchemaTrait).delete_sql(
            self.config.db.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &wheres,
            type_support_compare,
        )?;
        self.session.execute(&sql).await.map_err(err_to_anyhow)?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: 1,
            last_insert_id_string: None,
            last_insert_row_id: None,
        })
    }
    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"DROP TABLE "{}";"#, table);
        self.session.execute(&sql).await.map_err(err_to_anyhow)?;
        anyhow::Ok(())
    }
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

fn type_support_compare(typ: &str) -> bool {
    true
}

#[async_trait::async_trait]
impl DBTrait for CassandraDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let rows = self
            .session
            .execute("DESCRIBE KEYSPACES")
            .await
            .map_err(err_to_anyhow)?;
        let mut ret = Vec::with_capacity(rows.row_count() as usize);
        let mut iter = rows.iter();
        while let Some(row) = iter.next() {
            let name: String = row.get(0).map_err(err_to_anyhow)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut config = self.config.clone();
        config.db = Some(name.to_string());
        let x = CassandraDB::new(config).await?;
        anyhow::Ok(Arc::new(x))
    }

    async fn create_schema(
        &self, name: &str, extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        let extra = match &extra {
            Some(x) => x,
            None => anyhow::bail!("cassandra need extra paramater."),
        };
        let (class_name, replication_factor) = match extra {
            CreateSchemaExtra::Cassandra {
                class_name,
                replication_factor,
            } => (class_name, replication_factor),
        };

        let sql = format!(
            r#"
            CREATE KEYSPACE {}
            WITH REPLICATION = {{
                'class': '{}',
                'replication_factor': {}
            }};
        "#,
            name, class_name, replication_factor
        );
        self.session.execute(sql).await.map_err(err_to_anyhow)?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!("DROP KEYSPACE {name};");
        self.session.execute(&sql).await.map_err(err_to_anyhow)?;
        anyhow::Ok(())
    }
}

#[cfg(test)]
pub(crate) async fn make_test_object(db: Option<String>) -> CassandraDB {
    use crate::TEST_HOST;

    let ss = CassandraDB::new(CassandraDBConfig {
        host: TEST_HOST.to_string(),
        port: Some(9042),
        username: None,
        password: None,
        read_only: false,
        db,
    })
    .await
    .unwrap();
    ss
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]

    async fn test_list_schema() {
        let db = make_test_object(None).await;
        let _r = db.drop_schema("abc").await;
        if let Err(err) = _r {
            println!("xxxx:{}", err);
        }
        db.create_schema(
            "abc",
            Some(CreateSchemaExtra::Cassandra {
                class_name: "SimpleStrategy".to_string(),
                replication_factor: 1,
            }),
        )
        .await
        .unwrap();

        let schemas = db.list_schema().await.unwrap();
        println!("list schema:{:?}", schemas)
    }

    #[tokio::test]
    async fn test_list_table() {
        let db = make_test_object(Some("system".to_string())).await;
        let schemas = db.list_table().await.unwrap();
        println!("list table:{:?}", schemas)
    }
}
