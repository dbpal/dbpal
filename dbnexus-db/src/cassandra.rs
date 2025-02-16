#![allow(unused)]
use core::panic;
use std::{arch::x86_64, collections::HashMap, iter::Cloned, sync::Arc, time::Duration};

use cdrs_tokio::{
    authenticators::{StaticPasswordAuthenticator, StaticPasswordAuthenticatorProvider},
    cluster::{NodeTcpConfigBuilder, session::TcpSessionBuilder},
    frame::{
        message_error::ErrorBody,
        message_response::ResponseBody,
        message_result::{ColType, RowsMetadata},
    },
    load_balancing::RoundRobinLoadBalancingStrategy,
    types::{
        AsCassandraType, IntoRustByIndex, blob::Blob, cassandra_type::CassandraType, list::List,
        prelude::Decimal, rows::Row,
    },
};
use dashmap::DashMap;
use futures_util::TryFutureExt;
use num_bigint::ToBigInt;
use sqlparser::keywords::QUALIFY;

use crate::{
    datasource_config::CassandraDBConfig,
    db::{
        BackendName, CheckConn, CreateSchemaExtra, DBTrait, Field, Index, ListTable, QueryResp,
        QuerySql, SchemaTrait, ServerMetas, SqlExecuteRet, TableListResp, not_support_features,
    },
    helper::{SqlKind, SqlWithKind, sql_to_sql_seq},
    meta_value::MetaValue,
};
use cdrs_tokio::cluster::session::SessionBuilder;

#[derive(Clone)]
pub(crate) struct CassandraDB {
    session: Arc<
        cdrs_tokio::cluster::session::Session<
            cdrs_tokio::transport::TransportTcp,
            cdrs_tokio::cluster::TcpConnectionManager,
            RoundRobinLoadBalancingStrategy<
                cdrs_tokio::transport::TransportTcp,
                cdrs_tokio::cluster::TcpConnectionManager,
            >,
        >,
    >,
    config: CassandraDBConfig,

    paging_cache: Arc<DashMap<String, Arc<PageCache>>>,
}

#[derive(Clone, Debug)]
struct PageCache {
    result: QueryResp,
    condition: PageCacheCondition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PageCacheCondition {
    where_str: Option<String>,
    order_by: Option<String>,
}

impl PageCache {
    fn page(&self, page: usize, page_size: usize) -> QueryResp {
        let length = self.result.data.len();
        let start = (page - 1) * page_size;
        if start >= length {
            return QueryResp {
                fields: self.result.fields.clone(),
                data: vec![],
            };
        }

        let end = page * page_size;
        let end = std::cmp::min(end, length);

        QueryResp {
            fields: self.result.fields.clone(),
            data: (&self.result.data[start..end]).to_vec(),
        }
    }
}

impl CassandraDB {
    pub async fn new(config: CassandraDBConfig) -> anyhow::Result<Self> {
        let mut node_builder = NodeTcpConfigBuilder::new();

        node_builder = node_builder
            .with_contact_point(format!("{}:{}", config.host, config.port.unwrap_or(9042)).into());

        if config.username.is_some() && config.password.is_some() {
            let a = StaticPasswordAuthenticatorProvider::new(
                config.username.as_deref().unwrap(),
                config.password.as_deref().unwrap(),
            );
            node_builder = node_builder.with_authenticator_provider(Arc::new(a))
        }

        let cluster_config = node_builder.build().await?;

        let mut session_builder =
            TcpSessionBuilder::new(RoundRobinLoadBalancingStrategy::new(), cluster_config);
        if let Some(keyspace) = config.keyspace.as_deref() {
            session_builder = session_builder.with_keyspace(keyspace.to_owned());
        }

        let session: cdrs_tokio::cluster::session::Session<
            cdrs_tokio::transport::TransportTcp,
            cdrs_tokio::cluster::TcpConnectionManager,
            RoundRobinLoadBalancingStrategy<
                cdrs_tokio::transport::TransportTcp,
                cdrs_tokio::cluster::TcpConnectionManager,
            >,
        > = session_builder.build().await?;

        anyhow::Ok(Self {
            session: Arc::new(session),
            config,
            paging_cache: Default::default(),
        })
    }
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

fn infer_fields(rows: &RowsMetadata) -> Vec<Field> {
    rows.col_specs
        .iter()
        .map(|x| Field::from_name_type(x.name.clone(), x.col_type.id.to_string()))
        .collect()
}

fn encode_rows(meta: &RowsMetadata, rows: &Vec<Row>) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let mut ret = vec![];
    for row in rows.iter() {
        let mut data = vec![];

        for index in 0..meta.col_specs.len() {
            let ty = &meta.col_specs[index];

            let value = match ty.col_type.id {
                ColType::Custom => {
                    anyhow::bail!("not support custom datatype")
                }
                ColType::Ascii => {
                    let value: Option<String> = row.get_by_index(index)?;
                    value
                }
                ColType::Bigint => {
                    let value: Option<i64> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }

                ColType::Blob => {
                    let value: Option<Blob> = row.get_by_index(index)?;
                    value
                        .map(|x| x.into_vec())
                        .map(|x| format!("0x{}", hex::encode(&x[..])))
                }
                ColType::Boolean => {
                    let value: Option<bool> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Counter => todo!(),
                ColType::Decimal => {
                    let value: Option<Decimal> = row.get_by_index(index)?;
                    value.map(|x| format_decimal(&x))
                }
                ColType::Double => {
                    let value: Option<f64> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Float => {
                    let value: Option<f32> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Int => todo!(),
                ColType::Timestamp => todo!(),
                ColType::Uuid => {
                    let value: Option<uuid::Uuid> = row.get_by_index(index)?;
                    value.map(|x| x.to_string())
                }
                ColType::Varchar => todo!(),
                ColType::Varint => todo!(),
                ColType::Timeuuid => todo!(),
                ColType::Inet => todo!(),
                ColType::Date => todo!(),
                ColType::Time => todo!(),
                ColType::Smallint => todo!(),
                ColType::Tinyint => todo!(),
                ColType::Duration => todo!(),
                ColType::List => {
                    let x: Option<List> = row.get_by_index(index)?;
                    match x {
                        Some(list) => match list.as_cassandra_type()? {
                            Some(CassandraType::List(mut vs)) => {
                                let mut ret = Vec::with_capacity(vs.len());
                                for v in vs.iter_mut() {
                                    ret.push(encode_rows_canssrandra_type(v)?);
                                }
                                Some(format!("[{}]", ret.join(",")))
                            }

                            _ => unreachable!(),
                        },
                        None => None,
                    }
                }
                ColType::Map => todo!(),
                ColType::Set => todo!(),
                ColType::Udt => todo!(),
                ColType::Tuple => todo!(),
                _ => todo!(),
            };
            data.push(value);
        }

        ret.push(data);
    }

    anyhow::Ok(ret)
}

fn format_decimal(x: &Decimal) -> String {
    format!(
        "{}.{}",
        x.as_plain().to_string(),
        x.unscaled.clone() % 10i32.pow(x.scale as u32).to_bigint().unwrap()
    )
}
fn encode_rows_canssrandra_type(value: &mut CassandraType) -> anyhow::Result<String> {
    let ret = match value {
        CassandraType::Ascii(x) => format!("'{}'", x),
        CassandraType::Bigint(x) => x.to_string(),
        CassandraType::Blob(blob) => format!("0x{}", hex::encode(blob.as_mut_slice())),
        CassandraType::Boolean(x) => x.to_string(),
        CassandraType::Counter(x) => x.to_string(),
        CassandraType::Decimal(decimal) => format_decimal(decimal),
        CassandraType::Double(x) => x.to_string(),
        CassandraType::Float(x) => x.to_string(),
        CassandraType::Int(x) => x.to_string(),
        CassandraType::Timestamp(_) => todo!(),
        CassandraType::Uuid(uuid) => uuid.to_string(),
        CassandraType::Varchar(x) => format!("'{}'", x),
        CassandraType::Varint(big_int) => big_int.to_string(),
        CassandraType::Timeuuid(uuid) => todo!(),
        CassandraType::Inet(ip_addr) => todo!(),
        CassandraType::Date(_) => todo!(),
        CassandraType::Time(_) => todo!(),
        CassandraType::Smallint(_) => todo!(),
        CassandraType::Tinyint(_) => todo!(),
        CassandraType::Duration(duration) => todo!(),
        CassandraType::List(cassandra_types) => todo!(),
        CassandraType::Map(items) => todo!(),
        CassandraType::Set(cassandra_types) => todo!(),
        CassandraType::Udt(hash_map) => todo!(),
        CassandraType::Tuple(cassandra_types) => todo!(),
        CassandraType::Vector(cassandra_types) => todo!(),
        CassandraType::Null => todo!(),
        _ => todo!(),
    };
    anyhow::Ok(ret)
}
#[async_trait::async_trait]
impl QuerySql for CassandraDB {
    async fn query_sql(
        &self, _query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let seq = get_sql_seq(sql).unwrap_or(vec![SqlWithKind {
            sql: sql.to_string(),
            kind: SqlKind::Query,
        }]);
        let mut ret = vec![];
        for s in seq.iter() {
            match self.session.query(&s.sql).await {
                Ok(x) => {
                    let body = x.response_body()?;
                    let meta = body.as_rows_metadata().map(|x| x.clone());
                    let rows = body.into_rows().unwrap_or_default();
                    if rows.len() > 0 && meta.is_some() {
                        let meta = meta.unwrap();
                        let fields = infer_fields(&meta);
                        let data = encode_rows(&meta, &rows)?;
                        ret.push(Ok(QueryResp { fields, data }));
                    }
                }

                Err(err) => {
                    ret.push(Err(err.to_string()));
                }
            }
        }
        anyhow::Ok(ret)
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
        let body = self.session.query(sql).await?.response_body()?;
        check_body_error(&body)?;
        let mut ret = Vec::new();
        for row in body.into_rows().unwrap_or_default() {
            let name: Option<String> = row.get_by_index(0)?;
            ret.push(name.unwrap());
        }

        anyhow::Ok(ret.into_iter().map(|x| ListTable::from(x)).collect())
    }
}

fn check_body_error(body: &ResponseBody) -> anyhow::Result<()> {
    match body {
        ResponseBody::Error(e) => Err(anyhow::Error::msg(e.message.clone())),
        _ => Ok(()),
    }
}

impl CassandraDB {
    fn invalid_page_caching(&self, table: &str) {
        self.paging_cache.remove(&table.to_string());
    }

    fn page_caching_totoal(&self, name: &str) -> usize {
        self.paging_cache
            .get(&name.to_string())
            .map(|x| x.result.data.len())
            .unwrap_or(0)
    }

    async fn page_caching_show(
        &self, name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<QueryResp> {
        // try get value from cache
        let condition = PageCacheCondition {
            where_str: where_str.map(|x| x.to_owned()),
            order_by: order_by.map(|x| x.to_owned()),
        };

        {
            let cache = self.paging_cache.get(&name.to_string()).map(|x| x.clone());
            if let Some(cache) = cache {
                if cache.condition.eq(&condition) {
                    // hit cache
                    log::info!(
                        "hit cache for table:{} page:{} page_size:{}",
                        name,
                        page,
                        page_size
                    );
                    return anyhow::Ok(cache.page(page, page_size));
                }
            }
        }
        //
        log::info!(
            "cache not hit table:{} page:{} page_size:{},trying load data from server",
            name,
            page,
            page_size
        );
        let sql = format!(
            r#"SELECT * FROM "{name}" {} {} LIMIT 10000 "#,
            if let Some(where_str) = where_str {
                format!(" WHERE {} ", where_str)
            } else {
                "".to_string()
            },
            if let Some(order_by) = order_by {
                format!(" ORDER BY {} ", order_by)
            } else {
                "".to_string()
            }
        );
        let body = self.session.query(&sql).await?.response_body()?;
        let meta = body.as_rows_metadata().map(|x| x.clone());
        if meta.is_none() {
            return anyhow::Ok(QueryResp {
                fields: vec![],
                data: vec![],
            });
        }
        let meta = meta.unwrap();
        let rows = body.into_rows().unwrap_or_default();
        let fields = infer_fields(&meta);

        let data = encode_rows(&meta, &rows)?;
        // put in cache
        let cache = PageCache {
            result: QueryResp { fields, data },
            condition,
        };

        let ret = cache.page(page, page_size);
        self.paging_cache.insert(name.to_owned(), Arc::new(cache));
        anyhow::Ok(ret)
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
        not_support_features(self.backend_name())
    }
    fn quote_style(&self) -> &'static str {
        "\""
    }
    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.config.keyspace_str()).await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let sql = format!(
            r#" SELECT column_name, type, kind
        FROM system_schema.columns
        WHERE keyspace_name = '{}' AND table_name = '{}';"#,
            self.config.keyspace_str(),
            table_name
        );

        let rows = self
            .session
            .query(&sql)
            .await?
            .response_body()?
            .into_rows()
            .unwrap_or_default();
        let mut ret = Vec::new();
        for row in rows.iter() {
            let name: Option<String> = row.get_by_index(0)?;
            let name = name.unwrap();
            let typ: Option<String> = row.get_by_index(1)?;
            let typ = typ.unwrap();
            let kind: Option<String> = row.get_by_index(2)?;
            let kind = kind.unwrap();
            let is_primary = kind == "partition_key" || kind == "clustering";

            ret.push(Field {
                field: name,
                typ,
                nullable: !is_primary,
                default_value: None,
                is_primary,
                is_auto_increment: false,
            });
        }

        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let mut ret = self
            .page_caching_show(table_name, where_str, order_by, page, page_size)
            .await?;

        ret.fields = self.show_table_columns(&table_name).await?;

        let QueryResp { fields, data } = ret;

        anyhow::Ok(TableListResp {
            fields,
            data,
            total: self.page_caching_totoal(table_name),
            sql: None,
        })
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

        self.session.query(&sql).await?;

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
            self.config.keyspace.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &values,
            &wheres,
            type_support_compare,
        )?;
        self.session.query(&sql).await?;
        anyhow::Ok(SqlExecuteRet::from((sql, 1)))
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"TRUNCATE "{table_name}""#);
        self.session.query(&sql).await?;
        anyhow::Ok(())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;

        let sql = (self as &dyn SchemaTrait).delete_sql(
            self.config.keyspace.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &wheres,
            type_support_compare,
        )?;
        self.session.query(&sql).await?;
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
        self.session.query(&sql).await?;
        anyhow::Ok(())
    }
    fn read_only(&self) -> bool {
        self.config.read_only
    }

    fn put_value_in_sql(&self, typ: &str, value: Option<&str>) -> anyhow::Result<String> {
        put_value_in_sql(typ, value)
    }
}

fn put_value_in_sql(typ: &str, value: Option<&str>) -> anyhow::Result<String> {
    match typ.to_lowercase().as_str() {
        "ascii" | "varchar" => anyhow::Ok(
            value
                .map(|x| format!("'{}'", x))
                .unwrap_or("null".to_string()),
        ),
        _ => anyhow::Ok(value.map(|x| x.to_string()).unwrap_or("null".to_string())),
    }
}

fn type_support_compare(typ: &str) -> bool {
    true
}

#[async_trait::async_trait]
impl DBTrait for CassandraDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let body = self
            .session
            .query("DESCRIBE KEYSPACES")
            .await?
            .response_body()?;
        check_body_error(&body)?;
        let mut ret = Vec::new();

        for row in body.into_rows().unwrap_or_default() {
            let name: Option<String> = row.get_by_index(0)?;
            ret.push(name.unwrap());
        }
        anyhow::Ok(ret)
    }
    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut config = self.config.clone();
        config.keyspace = Some(name.to_string());
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
        self.session.query(sql).await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!("DROP KEYSPACE {name};");
        self.session.query(&sql).await?;
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
        keyspace: db,
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

        let _ = db
            .create_schema(
                "test",
                Some(CreateSchemaExtra::Cassandra {
                    class_name: "SimpleStrategy".to_string(),
                    replication_factor: 1,
                }),
            )
            .await;

        let schemas = db.list_schema().await.unwrap();
        println!("list schema:{:?}", schemas)
    }

    #[tokio::test]
    async fn test_list_table() {
        let db = make_test_object(Some("system".to_string())).await;
        let schemas = db.list_table().await.unwrap();
        println!("list table:{:?}", schemas)
    }

    #[tokio::test]
    async fn test_show_table_column() {
        let db = make_test_object(Some("test".to_string())).await;
        let fields = db.show_table_columns("example_table").await.unwrap();
        for field in fields {
            println!("field:{:?}", field);
        }
    }

    #[tokio::test]
    async fn test_xxxxx() {
        let db = make_test_object(None).await;
        let x = db.session.query("xxxxxxxxx").await;
        // should have error on `x` no need check on response.
        assert!(x.is_err());
    }

    #[tokio::test]
    async fn test_types1() {
        let table_name = "test_types1";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_types1 (
                    id UUID PRIMARY KEY,
                    ascii_column ASCII,
                    bigint_column BIGINT,
                    blob_column BLOB,
                    boolean_column BOOLEAN,
                    decimal_column DECIMAL,
                    double_column DOUBLE
                );
        "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);
        /*
        INSERT INTO test_types1 (id, ascii_column, bigint_column, blob_column, boolean_column, decimal_column, double_column)
        VALUES (e6361ec1-b0a6-4eb6-b7bd-2db2d13329ab, 'example_ascii', 1234567890123456789, 0x48656c6c6f, true, 123.45, 3.14159);
        */

        let mut values = HashMap::new();

        values.insert(
            "id".to_string(),
            Some("e6361ec1-b0a6-4eb6-b7bd-2db2d13329ab".to_string()),
        );
        values.insert(
            "ascii_column".to_string(),
            Some("example_ascii".to_string()),
        );
        values.insert(
            "bigint_column".to_string(),
            Some("1234567890123456789".to_string()),
        );
        values.insert("blob_column".to_string(), Some("0x48656c6c6f".to_string()));
        values.insert("boolean_column".to_string(), Some("true".to_string()));

        values.insert("decimal_column".to_string(), Some("123.45".to_string()));
        values.insert("double_column".to_string(), Some("3.14159".to_string()));

        db.insert(&table_name, values).await.unwrap();

        {
            let resp = db.show_table(table_name, None, None, 1, 100).await;

            println!("resp:{:?}", resp);
        }
    }

    #[tokio::test]
    async fn test_types2() {
        let table_name = "test_types2";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_types2 (
                    id UUID PRIMARY KEY,
                    list_value List<double>
                    
                );
        "#,
            )
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()), "{:?}", x);
        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert("list_value".to_string(), Some("[1,2,3]".to_string()));
        db.insert(&table_name, values).await.unwrap();

        {
            let resp = db.show_table(table_name, None, None, 1, 100).await;
            println!("resp:{:?}", resp);
        }
    }
}
