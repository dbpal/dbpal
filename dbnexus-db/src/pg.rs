use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use std::sync::Arc;

use sqlx::pool::PoolConnection;
use sqlx::postgres::PgRow;
use sqlx::{Postgres, Row};

use crate::completion::LexerCompletionBackend;
use crate::datasource::{CommDBConfig, DBConfig};
use crate::db::{
    sql_to_sql_seq, CreateTableAndIndexSql, DBTrait, ExeSqlError, Field, Index, IndexKind,
    ListTable, ListView, QuerySql, SqlExecuteRet, SqlSeq, TableListResp,
};
use crate::pg_format::*;
use crate::pool::{NexusPool, PoolManager};

/// postgres version of pg.
#[derive(Clone)]
pub struct PGDB {
    /// database name.
    pool: PoolManager,
    config: CommDBConfig,
}

#[async_trait::async_trait]
impl LexerCompletionBackend for PGDB {
    type Dialect = sqlparser::dialect::PostgreSqlDialect;

    const BUILD_IN_FUNCTIONS: &'static [&'static str] = PG_FUNCTIONS;
    const KEY_WORDS: &'static [&'static str] = PG_KEYWORDS;
    const TYPES: &'static [&'static str] = PG_TYPES;

    async fn list_table(&self) -> anyhow::Result<Vec<String>> {
        crate::db::DBTrait::list_table(self)
            .await
            .map(|x| x.into_iter().map(|x| x.table).collect())
    }

    async fn list_db(&self) -> anyhow::Result<Vec<String>> {
        crate::db::MultiDBTrait::list_db(self).await
    }

    async fn list_columns(&self, table_name: &str) -> anyhow::Result<Vec<String>> {
        crate::db::DBTrait::show_columns(self, table_name)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }
}

#[async_trait::async_trait]
impl crate::db::MultiDBTrait for PGDB {
    async fn list_db(&self) -> anyhow::Result<Vec<String>> {
        unimplemented!()
    }

    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        debug_assert!(self.config.db_name.is_none());
        let mut x = Clone::clone(self);
        x.config.db_name = Some(name.to_owned());
        Ok(Arc::new(x))
    }
}

fn gen_show_sql(
    table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize,
) -> String {
    let mut sql = format!("SELECT * from {table_name}");
    if let Some(where_str) = where_str {
        sql.push_str(format!(" where {where_str} ").as_str());
    }
    if let Some(order_by) = order_by {
        sql.push_str(format!(" order by {order_by} ").as_str());
    }
    let offset = (page - 1) * page_size;
    sql.push_str(format!(" offset {} limit {}", offset, page_size).as_str());

    sql
}

#[async_trait::async_trait]
impl DBTrait for PGDB {
    fn backend_name(&self) -> &str {
        "postgres"
    }

    async fn create_table_and_index_sql(
        &self, _table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        unimplemented!()
    }

    fn clone(&self) -> Box<dyn DBTrait> {
        Box::new(Clone::clone(self))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let total = self.query_count(view_name, where_str).await?;

        let sql = gen_show_sql(view_name, where_str, order_by, page, page_size);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await?;

        let data = encode_rows(&rows)?;
        let fields = get_rows_types(&rows);
        anyhow::Ok(TableListResp::new(fields, total, data, sql))
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let sql = format!("DROP VIEW {view_name}");
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let sql = format!("DROP TABLE {table}");
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListView>> {
        let mut conn = self.get_conn().await?;

        let rows = sqlx::query(
            r#"
        SELECT table_name
        FROM information_schema.tables
        WHERE table_type = 'VIEW'
          AND table_schema = 'public';
        "#,
        )
        .fetch_all(conn.deref_mut())
        .await?;

        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let view_name: String = r.get(0);
            ret.push(view_name);
        }
        anyhow::Ok(ret.into_iter().map(ListView::from).collect())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fields = self.show_columns(table_name).await?;
        let sql = (self as &dyn crate::db::DBTrait).delete_sql(table_name, &fields, &wheres)?;
        let mut conn = self.get_conn().await?;
        let r = sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: r.rows_affected(),
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        let s = format!(
            r#"
            TRUNCATE TABLE {table_name};
        "#
        );
        let mut conn = self.get_conn().await?;
        sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    fn put_value_in_sql(&self, f: &Field, value: Option<&str>) -> anyhow::Result<String> {
        put_value_in_sql(f, value)
    }

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_columns(table_name).await?;
        let s = (self as &dyn crate::db::DBTrait).update_sql(table_name, &fs, &values, &wheres)?;
        let mut conn = self.get_conn().await?;

        let r = sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql: s,
            changes: r.rows_affected(),
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_columns(table_name).await?;
        let s = (self as &dyn crate::db::DBTrait).insert_sql(table_name, &fs, &values)?;

        let mut conn = self.get_conn().await?;

        let r = sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql: s,
            changes: r.rows_affected(),
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"SELECT table_name
             FROM information_schema.tables
             WHERE table_type = 'BASE TABLE' AND table_schema = 'public' and table_catalog='{}'
     "#,
            self.config.db_name.as_ref().unwrap()
        );

        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await?;

        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let s: String = r.get(0);
            ret.push(s);
        }
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    async fn show_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let s = format!(
            r#"

            SELECT  column_name , data_type,is_nullable,column_default,udt_name
            FROM information_schema.columns where table_catalog='{}' and table_name = '{}' order by ordinal_position
        "#,
            self.config.db_name.as_ref().map(|x| x.as_str()).unwrap(),
            table_name
        );

        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(s.as_str())
            .fetch_all(conn.deref_mut())
            .await
            .unwrap();
        let mut ret = Vec::with_capacity(rows.len());
        let primary_keys = {
            let sql = format!(
                r#"
                -- 查表的主键详情
                SELECT  kcu.column_name
                  FROM information_schema.table_constraints AS tc
                  JOIN information_schema.key_column_usage  AS kcu
                    ON tc.constraint_name = kcu.constraint_name
                 WHERE tc.constraint_type = 'PRIMARY KEY'
                   AND tc.table_name      = '{table_name}';
        "#
            );

            let rows = sqlx::query(sql.as_str())
                .fetch_all(conn.deref_mut())
                .await?;

            let mut keys = HashSet::new();
            for r in rows.iter() {
                let key: String = r.get(0);
                keys.insert(key);
            }
            keys
        };

        for r in rows.iter() {
            let name: String = r.get(0 as usize);
            let data_type: String = r.get(1);
            let data_type = data_type.to_uppercase();
            let nullable: String = r.get(2);
            let default_value: Option<String> = r.get(3);
            let udt_name: Option<String> = r.get(4);
            let is_primary = primary_keys.contains(&name);
            let is_auto_increment = data_type.to_lowercase().contains("int")
                && default_value
                    .as_ref()
                    .map(|x| x.contains("nextval"))
                    .unwrap_or(false);

            ret.push(Field::new(
                name,
                match udt_name {
                    Some(udt_name) => {
                        get_pg_type_base_on_data_type_and_udtname(&data_type, &udt_name)
                    }
                    None => data_type,
                },
                nullable.to_lowercase() != "no",
                default_value,
                is_primary,
                is_auto_increment && is_primary,
            ));
        }

        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let total = self.query_count(table_name, where_str.clone()).await?;
        let sql = gen_show_sql(table_name, where_str, order_by, page, page_size);
        let mut conn = self.get_conn().await?;
        let row = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await?;
        let data = encode_rows(&row)?;
        let fields = self.show_columns(table_name).await?;
        anyhow::Ok(TableListResp::new(fields, total, data, sql))
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let mut conn = self.get_conn().await?;

        // this sql copied from https://stackoverflow.com/questions/2204058/list-columns-with-indexes-in-postgresql
        let rows = sqlx::query(
            format!(
                r#"
            SELECT
                t.relname as table_name,
                i.relname as index_name,
                array_agg(a.attname) as column_names
            from
                pg_class t,
                pg_class i,
                pg_index ix,
                pg_attribute a
            where
                t.oid = ix.indrelid
                and i.oid = ix.indexrelid
                and a.attrelid = t.oid
                and a.attnum = ANY(ix.indkey)
                and t.relkind = 'r'
                and t.relname = '{table_name}'
            group by
                t.relname,
                i.relname
                order by
                t.relname,
                i.relname;
        "#
            )
            .as_str(),
        )
        .fetch_all(conn.deref_mut())
        .await
        .unwrap();
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let index_name: String = r.get(1);
            let column_names: Option<Vec<String>> = r.get(2);
            let index_kind = get_index_kind(&mut conn, table_name, &index_name).await?;
            ret.push(Index::new(
                index_name,
                index_kind,
                column_names.unwrap_or(vec![]),
            ))
        }
        anyhow::Ok(ret)
    }
}

impl PGDB {
    async fn query_count(&self, name: &str, where_str: Option<&str>) -> anyhow::Result<usize> {
        let mut conn = self.get_conn().await?;
        let mut s = format!("SELECT count(1) from {name} ");
        if let Some(where_str) = where_str {
            s.push_str(format!(" where {where_str}").as_str());
        }
        let row = sqlx::query(s.as_str()).fetch_one(conn.deref_mut()).await?;
        let count: i64 = row.get(0);
        anyhow::Ok(count as usize)
    }
}

impl PGDB {
    pub fn new(db: impl Into<String>, pool: PoolManager, config: CommDBConfig) -> Self {
        Self { pool, config }
    }

    pub async fn get_conn(&self) -> anyhow::Result<PoolConnection<Postgres>> {
        let pool = match self
            .pool
            .get(&DBConfig::PostGres {
                conn: self.config.clone(),
            })
            .await?
        {
            NexusPool::Pg(p) => p.clone(),
            _ => unreachable!(),
        };

        let conn = pool.acquire().await?;
        return anyhow::Ok(conn);
    }
}

fn get_sql_seq(sql: &str) -> anyhow::Result<Vec<SqlSeq>> {
    let dialect = sqlparser::dialect::PostgreSqlDialect {};
    use sqlparser::ast::Statement;
    sql_to_sql_seq(
        sql,
        &dialect,
        |s| {
            match s {
                Statement::Query(_) => crate::db::SqlKind::Query,
                _ => crate::db::SqlKind::Execution,
            }
        },
        |x| {
            let t = x.to_uppercase();
            if t.starts_with("SELECT") || t.starts_with("SHOW") || t.starts_with("EXPLAIN") {
                crate::db::SqlKind::Query
            } else {
                crate::db::SqlKind::Execution
            }
        },
    )
}

fn get_rows_types(rows: &Vec<PgRow>) -> Vec<Field> {
    if rows.len() == 0 {
        return vec![];
    }
    let mut ret = Vec::with_capacity(rows[0].columns().len());
    for c in rows[0].columns().iter() {
        let column = c as &dyn sqlx::Column<Database = sqlx::Postgres>;
        ret.push(Field {
            name: column.name().to_string(),
            typ: column.type_info().to_string(),
            nullable: true,
            default_value: None,
            is_primary: false,
            is_auto_increment: false,
        });
    }
    ret
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_output_bytes() {
        let x = output_bytes(&[98, 126]);
        assert_eq!(x, "0x627E")
    }
}

///
fn get_pg_type_base_on_data_type_and_udtname(data_type: &str, udt_name: &str) -> String {
    if data_type.to_uppercase() == "ARRAY" {
        match udt_name {
            "_macaddr8" => return "MACADDR8[]".to_string(),
            "_bytea" => return "BYTEA[]".to_string(),
            "_bool" => return "BOOL[]".to_string(),
            "_name" => return "NAME[]".to_string(),
            "_bpchar" => return "BPCHAR[]".to_string(),
            "_int2" => return "INT2[]".to_string(),
            "_int4" => return "INT4[]".to_string(),
            "_text" => return "TEXT[]".to_string(),
            "_varchar" => return "VARCHAR[]".to_string(),
            "_int8" => return "INT8[]".to_string(),

            "_path" => return "PATH[]".to_string(),
            "_float4" => return "FLOAT4[]".to_string(),
            "_float8" => return "FLOAT8[]".to_string(),
            "_macaddr" => return "MACADDR[]".to_string(),
            "_inet" => return "INET[]".to_string(),

            "_timestamp" => return "TIMESTAMP[]".to_string(),
            "_date" => return "DATE[]".to_string(),
            "_time" => return "TIME[]".to_string(),
            "_timestamptz" => return "TIMESTAMPTZ[]".to_string(),

            "_interval" => return "INTERVAL[]".to_string(),
            "_numeric" => return "NUMERIC[]".to_string(),
            "_timetz" => return "TIMETZ[]".to_string(),
            "_bit" => return "BIT[]".to_string(),
            "_varbit" => return "VARBIT[]".to_string(),

            "_uuid" => return "UUID[]".to_string(),
            "_jsonb" => return "JSONB[]".to_string(),
            "_int4range" => return "INT4RANGE[]".to_string(),

            "_numrange" => return "NUMRANGE[]".to_string(),
            "_tsrange" => return "TSRANGE[]".to_string(),
            "_tstzrange" => return "TSTZRANGE[]".to_string(),

            "_daterange" => return "DATERANGE[]".to_string(),
            "_int8range" => return "INT8RANGE[]".to_string(),
            "_jsonpath" => return "JSONPATH[]".to_string(),
            "_money" => return "MONEY[]".to_string(),

            _ => {
                log::error!("udtname '{}' not handled", udt_name)
            }
        }
    }

    return data_type.to_string();
}
#[async_trait::async_trait]
impl QuerySql for PGDB {
    async fn query_sql(
        &self, sql: &str,
    ) -> anyhow::Result<Vec<Result<TableListResp, ExeSqlError>>> {
        let seq = get_sql_seq(sql);
        let x = match seq {
            anyhow::Result::Ok(e) => e,
            anyhow::Result::Err(_err) => {
                log::error!("parse sql fail use as query");
                vec![SqlSeq {
                    sql: sql.to_string(),
                    kind: crate::db::SqlKind::Query,
                }]
            }
        };

        let mut ret = Vec::with_capacity(x.len());
        let mut conn = self.get_conn().await?;
        use crate::db::SqlKind;

        for c in x.iter() {
            let result = match c.kind {
                SqlKind::Query => {
                    let result = sqlx::query(&c.sql).fetch_all(conn.deref_mut()).await;
                    let result: Result<TableListResp, ExeSqlError> = match result {
                        Ok(rows) => {
                            let data = encode_rows(&rows)?;
                            let fields = get_rows_types(&rows);
                            Ok(TableListResp::new(fields, data.len(), data, &c.sql))
                        }
                        Err(err) => {
                            Err(ExeSqlError::new(
                                &c.sql,
                                anyhow::Error::msg(err.to_string()),
                            ))
                        }
                    };
                    result
                }

                SqlKind::Execution => {
                    let result = sqlx::query(&c.sql).execute(conn.deref_mut()).await;
                    let result: Result<TableListResp, ExeSqlError> = match result {
                        Ok(rows) => Ok(TableListResp::row_affected(&c.sql, rows.rows_affected())),
                        Err(err) => {
                            Err(ExeSqlError::new(
                                &c.sql,
                                anyhow::Error::msg(err.to_string()),
                            ))
                        }
                    };
                    result
                }
            };
            ret.push(result);
        }
        anyhow::Ok(ret)
    }
}

async fn get_index_kind(
    conn: &mut PoolConnection<Postgres>, table_name: &str, index_name: &str,
) -> anyhow::Result<IndexKind> {
    let sql = format!(
        "SELECT indexdef from pg_indexes where tablename='{table_name}' and indexname='{index_name}'"
    );
    let row = sqlx::query(sql.as_str())
        .fetch_one(conn.deref_mut())
        .await?;

    let sql: String = row.get(0);
    let ss = sqlparser::parser::Parser::parse_sql(
        &sqlparser::dialect::PostgreSqlDialect {},
        sql.as_str(),
    )?;
    if ss.len() != 1 {
        anyhow::bail!("parse create index should contains one statement");
    }
    let s = &ss[0];
    use sqlparser::ast::Statement;

    match s {
        Statement::CreateIndex {
            name: _,
            table_name: _,
            using: _,
            columns: _,
            unique,
            concurrently: _,
            if_not_exists: _,
            include: _,
            nulls_distinct: _,
            predicate: _,
        } => {
            if *unique {
                anyhow::Ok(IndexKind::Unique)
            } else {
                anyhow::Ok(IndexKind::Unique)
            }
        }

        _ => {
            anyhow::bail!(format!("the sql is not a create index"))
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use std::ops::DerefMut;

    use super::PGDB;
    use crate::datasource::CommDBConfig;
    use crate::db::DBTrait;
    use crate::pool::PoolManager;
    use crate::{init_log, TEST_HOST};

    fn make_test_object() -> PGDB {
        // list all table and query
        let pool_manager = PoolManager::new();
        let pg = PGDB::new(
            "test".to_string(),
            pool_manager.clone(),
            CommDBConfig {
                host: TEST_HOST.to_string(),
                port: 5432,
                username: "postgres".to_string(),
                password: "root".to_string(),
                db_name: Some("test".to_string()),
            },
        );

        pg
    }

    #[tokio::test]
    async fn test_show_index() {
        let table_name = "index_test";
        let db = make_test_object();
        let mut conn = db.get_conn().await.unwrap();
        let _ = sqlx::query(format!("drop table {table_name}").as_str())
            .execute(conn.deref_mut())
            .await;

        let _ =   sqlx::query(format!(
            r#" create table {table_name} (a int, b int, c int, constraint pk_test primary key(a, b)); "#,
        ).as_str()) .execute(conn.deref_mut())
        .await;

        let db = &db as &dyn crate::db::DBTrait;
        let indexs = db.show_indexes(table_name).await.unwrap();
        println!("{:?}", indexs);
    }

    #[tokio::test]
    async fn test() {
        // list all table and query
        let pg = make_test_object();

        let pg = &pg as &dyn DBTrait;
        let table = pg.list_table().await.unwrap();
        println!("{:?}", table);
    }

    #[tokio::test]
    async fn test_types1() {
        let table_name = "test_types1";
        let pg = make_test_object();
        let mut conn = pg.get_conn().await.unwrap();
        let _ = sqlx::query(format!("drop table {table_name}").as_str())
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
            CREATE TABLE {table_name} (
                id SERIAL PRIMARY KEY,
                bool_column BOOL,
                bytea_column BYTEA,
                char_column CHAR,
                name_column NAME,
                int8_column INT8,
                int2_column INT2,
                int4_column INT4
            );"#
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        // sqlx::query(
        //     r#"
        //     INSERT INTO test_types1 (bool_column, bytea_column, char_column,
        // name_column, int8_column, int2_column, int4_column)     VALUES
        //         (TRUE, E'\\x0123456789ABCDEF', 'A', 'John', 1000000000, 10, 100),
        //         (FALSE, E'\\x00FF00FF', 'B', 'Jane', -500000000, 20, -200),
        //         (TRUE, E'\\x00', 'C', 'Mike', 0, 30, 0);
        // "#,
        // ).execute(conn.deref_mut() ).await.unwrap();
        let pg = &pg as &dyn DBTrait;
        let mut values = HashMap::new();
        values.insert("bool_column".to_string(), Some("true".to_string()));
        values.insert(
            "bytea_column".to_string(),
            Some("E'\\x0123456789ABCDEF'".to_string()),
        );
        values.insert("char_column".to_string(), Some("A".to_string()));
        values.insert("name_column".to_string(), Some("John".to_string()));
        values.insert("int8_column".to_string(), Some("1".to_string()));
        values.insert("int2_column".to_string(), Some("2".to_string()));
        values.insert("int4_column".to_string(), Some("100".to_string()));
        pg.insert(table_name, values.clone()).await.unwrap();
        let mut wheres = HashMap::new();
        wheres.insert("id".to_string(), Some("1".to_string()));
        pg.update(table_name, values, wheres).await.unwrap();
        let x = pg
            .show_table("test_types1", None, None, 1, 100)
            .await
            .unwrap();
        println!("{:?}", x);
    }

    #[tokio::test]
    async fn test_types2() {
        init_log();
        let table_name = "test_types2";

        let pg = make_test_object();
        let mut conn = pg.get_conn().await.unwrap();
        let _ = sqlx::query(format!("drop table {table_name}").as_str())
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
            CREATE TABLE {table_name} (
                id SERIAL PRIMARY KEY,
                text_column TEXT,
                oid_column OID,
                json_column JSON,
                float4_column FLOAT4,
                float8_column FLOAT8,
                macaddr8_column MACADDR8,
                macaddr8_array_column MACADDR8[]
            );
        "#
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        // sqlx::query(
        //     r#"
        //             INSERT INTO test_types2 (text_column, oid_column, json_column,
        // float4_column, float8_column, macaddr8_column, macaddr8_array_column)
        // VALUES
        //     ('Sample text', 12345, '{"key": "value"}', 3.14, 5.6789,
        // '08:00:2b:01:02:03:04:05', ARRAY['08:00:2b:01:02:03:04:06',
        // '08:00:2b:01:02:03:04:07']::MACADDR8[]);         "#,
        // )
        // .execute(conn.deref_mut())
        // .await
        // .unwrap();

        let mut values = HashMap::new();
        values.insert("text_column".to_string(), Some("Sample text".to_string()));
        values.insert("oid_column".to_string(), Some("12345".to_string()));
        values.insert("json_column".to_string(), Some("3.14".to_string()));
        values.insert("float4_column".to_string(), Some("3.14".to_string()));
        values.insert("float8_column".to_string(), Some("5.6789".to_string()));
        values.insert(
            "macaddr8_column".to_string(),
            Some("08:00:2b:01:02:03:04:05".to_string()),
        );
        values.insert(
            "macaddr8_array_column".to_string(),
            Some("{08:00:2b:01:02:03:04:06, 08:00:2b:01:02:03:04:07}".to_string()),
        );
        let pg = &pg as &dyn DBTrait;
        pg.insert(table_name, values).await.unwrap();

        let x = pg.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", x);
    }

    #[tokio::test]
    async fn test_types3() {
        init_log();
        let pg = make_test_object();
        let table_name = "test_types3";
        let mut conn = pg.get_conn().await.unwrap();
        let _ = sqlx::query(format!("drop table {table_name}").as_str())
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
            CREATE TABLE {table_name} (
                macaddr_column MACADDR,
                inet_column INET,
                bool_array_column BOOL[],
                bytea_array_column BYTEA[],
                char_array_column CHAR[],
                name_array_column NAME[]
            );
        "#
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        let pg = &pg as &dyn DBTrait;

        let mut values = HashMap::new();
        values.insert(
            "macaddr_column".to_string(),
            Some("08:00:2b:01:02:03".to_string()),
        );
        values.insert("inet_column".to_string(), Some("192.168.0.1".to_string()));
        values.insert(
            "bool_array_column".to_string(),
            Some("{true , false }".to_string()),
        );
        values.insert(
            "bytea_array_column".to_string(),
            Some("{0x12345}".to_string()),
        );
        values.insert(
            "char_array_column".to_string(),
            Some("{a  , b }".to_string()),
        );
        values.insert(
            "name_array_column".to_string(),
            Some("{ a  , b }".to_string()),
        );

        pg.insert(table_name, values).await.unwrap();

        let x = pg.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", x);
    }

    #[tokio::test]
    async fn test_types4() {
        init_log();

        let table_name = "test_types4";
        let pg = make_test_object();
        let mut conn = pg.get_conn().await.unwrap();
        let _ = sqlx::query(format!("drop table {table_name}").as_str())
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
            CREATE TABLE {table_name} (
                int2_array_column INT2[],
                int4_array_column INT4[],
                text_array_column TEXT[],
                varchar_array_column VARCHAR[],
                int8_array_column INT8[]
            );
        "#
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        //         sqlx::query(
        //             r#"

        //             INSERT INTO test_types4 (int2_array_column, int4_array_column,
        // text_array_column, varchar_array_column, int8_array_column) VALUES
        //     (ARRAY[1, 2, 3], ARRAY[4, 5, 6], ARRAY['A', 'B', 'C'], ARRAY['John',
        // 'Jane', 'Bob'], ARRAY[7, 8, 9]);         "#,
        //         ).execute(conn.deref_mut()).await.unwrap();
        let mut values = HashMap::new();
        values.insert("int2_array_column".to_string(), Some("{1,2}".to_string()));
        values.insert("int4_array_column".to_string(), Some("{1,2}".to_string()));
        values.insert("text_array_column".to_string(), Some("{a,b}".to_string()));
        values.insert(
            "varchar_array_column".to_string(),
            Some("{c,d}".to_string()),
        );
        values.insert("int8_array_column".to_string(), Some("{1,100}".to_string()));
        let pg = &pg as &dyn DBTrait;
        pg.insert(table_name, values).await.unwrap();

        let x = pg.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", x);
    }

    #[tokio::test]
    async fn test_types5() {
        init_log();
        let table_name = "test_types5";
        let pg = make_test_object();
        let mut conn = pg.get_conn().await.unwrap();
        let _ = sqlx::query(format!("drop table {table_name}").as_str())
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
            CREATE TABLE {table_name} (
                path_column PATH,
                path_array_column PATH[],
                float4_array_column FLOAT4[],
                float8_array_column FLOAT8[],
                macaddr_array_column MACADDR[],
                inet_array_column INET[],
                date_column DATE
            )
        "#,
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        //         sqlx::query(
        //             r#"

        //             INSERT INTO test_types5 (path_column, path_array_column,
        // float4_array_column, float8_array_column, macaddr_array_column,
        // inet_array_column, date_column) VALUES
        //     ('(1.0,2.0),(3.0,4.0)'::PATH, ARRAY['(5.0,6.0),(7.0,8.0)'::PATH,
        // '(9.0,10.0),(11.0,12.0)'::PATH],         ARRAY[1.23, 4.56, 7.89],
        // ARRAY[12.34, 56.78, 90.12],         ARRAY['08:00:2b:01:02:03'
        // ::MACADDR, '08:00:2b:04:05:06'::MACADDR],         ARRAY['192.168.0.1'
        // ::INET, '10.0.0.1'::INET], '2022-01-01'::DATE);         "#,
        //         ).execute(conn.deref_mut()).await.unwrap();
        let pg = &pg as &dyn DBTrait;

        let mut values = HashMap::new();
        values.insert(
            "path_column".to_string(),
            Some("(1.0,2.0),(3.0,4.0)".to_string()),
        );
        values.insert(
            "path_array_column".to_string(),
            Some("{\"((5,6),(7,8))\",\"((9,10),(11,12))\"}".to_string()),
        );
        values.insert("float4_array_column".to_string(), Some("{1,2}".to_string()));
        values.insert("float8_array_column".to_string(), Some("{3,4}".to_string()));

        values.insert(
            "macaddr_array_column".to_string(),
            Some("{01:02:03:04:05:06}".to_string()),
        );
        values.insert(
            "inet_array_column".to_string(),
            Some("{192.168.1.1}".to_string()),
        );

        values.insert("date_column".to_string(), Some("2022-01-01".to_string()));

        pg.insert(table_name, values).await.unwrap();

        let x = pg.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", x);
    }

    #[tokio::test]
    async fn test_types6() {
        init_log();
        let table_name = "test_types6";
        let pg = make_test_object();
        let mut conn = pg.get_conn().await.unwrap();
        let _ = sqlx::query(format!("drop table {table_name}").as_str())
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    col_time TIME,
                    col_timestamp TIMESTAMP,
                    col_timestamp_array TIMESTAMP[],
                    col_date_array DATE[],
                    col_time_array TIME[],
                    col_timestamptz TIMESTAMPTZ,
                    col_timestamptz_array TIMESTAMPTZ[]
                );
        "#,
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        let pg = &pg as &dyn DBTrait;

        let mut values = HashMap::new();
        values.insert("col_time".to_string(), Some("10:00:00".to_string()));
        values.insert(
            "col_timestamp".to_string(),
            Some("2024-03-01 10:00:00".to_string()),
        );
        values.insert(
            "col_timestamp_array".to_string(),
            Some("{\"2024-03-01 10:00:00\",\"2024-03-02 12:00:00\"}".to_string()),
        );
        values.insert(
            "col_date_array".to_string(),
            Some("{2024-03-01,2024-03-02}".to_string()),
        );
        values.insert(
            "col_date_array".to_string(),
            Some("{2024-03-01,2024-03-02}".to_string()),
        );
        values.insert(
            "col_time_array".to_string(),
            Some("{10:00:00,12:00:00}".to_string()),
        );
        values.insert(
            "col_timestamptz".to_string(),
            Some("2024-03-01 10:00:00+00".to_string()),
        );
        values.insert(
            "col_timestamptz_array".to_string(),
            Some("{\"2024-03-01 10:00:00+00\",\"2024-03-02 12:00:00+00\"}".to_string()),
        );

        pg.insert(table_name, values).await.unwrap();

        let x = pg.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", x);
    }

    #[tokio::test]
    async fn test_types7() {
        init_log();
        let table_name = "test_types7";
        let pg = make_test_object();
        let mut conn = pg.get_conn().await.unwrap();
        let _ = sqlx::query(format!("drop table {table_name}").as_str())
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    interval_col INTERVAL,
                    interval_array_col INTERVAL[],
                    numeric_array_col NUMERIC[],
                    timetz_col TIMETZ,
                    timetz_array_col TIMETZ[]
                );
        "#,
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        let pg = &pg as &dyn DBTrait;

        let mut values = HashMap::new();
        values.insert(
            "interval_col".to_string(),
            Some("1 day 02:30:00".to_string()),
        );
        values.insert(
            "interval_array_col".to_string(),
            Some("{\"2 days\",03:00:00,00:04:00}".to_string()),
        );
        values.insert(
            "numeric_array_col".to_string(),
            Some("{1.23,4.56,7.89}".to_string()),
        );
        values.insert("timetz_col".to_string(), Some("12:30:00+03".to_string()));
        values.insert(
            "timetz_array_col".to_string(),
            Some("{10:00:00+03,15:30:00+03}".to_string()),
        );
        pg.insert(table_name, values).await.unwrap();
        let x = pg.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", x);
    }

    #[tokio::test]
    async fn test_types8() {
        init_log();
        let table_name = "test_types8";
        let pg = make_test_object();
        let mut conn = pg.get_conn().await.unwrap();
        let _ = sqlx::query(format!("drop table {table_name}").as_str())
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
                CREATE TABLE {table_name}(
                    bit_column BIT,
                    bit_array_column BIT[],
                    varbit_column VARBIT,
                    varbit_array_column VARBIT[],
                    numeric_column NUMERIC
                  );
        "#,
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        let pg = &pg as &dyn DBTrait;

        let mut values = HashMap::new();
        values.insert("bit_column".to_string(), Some("1".to_string()));
        values.insert("bit_array_column".to_string(), Some("{0,1}".to_string()));
        values.insert("varbit_column".to_string(), Some("101010".to_string()));
        values.insert(
            "varbit_array_column".to_string(),
            Some("{101,010}".to_string()),
        );

        values.insert("numeric_column".to_string(), Some("123.45".to_string()));
        pg.insert(table_name, values).await.unwrap();
        let x = pg.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", x);
    }

    #[tokio::test]
    async fn test_types9() {
        init_log();
        let table_name = "test_types9";
        let pg = make_test_object();
        let mut conn = pg.get_conn().await.unwrap();
        let _ = sqlx::query(format!("drop table {table_name}").as_str())
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    uuid_column UUID,
                    uuid_array_column UUID[],
                    jsonb_column JSONB,
                    jsonb_array_column JSONB[],
                    int4range_column INT4RANGE,
                    int4range_array_column INT4RANGE[]
                  );
        "#,
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        let pg = &pg as &dyn DBTrait;

        let mut values = HashMap::new();
        values.insert(
            "uuid_column".to_string(),
            Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
        );
        values.insert(
            "uuid_array_column".to_string(),
            Some(
                "{123e4567-e89b-12d3-a456-426655440000,987e6543-c21d-43a1-b876-446655440000}"
                    .to_string(),
            ),
        );
        values.insert(
            "jsonb_column".to_string(),
            Some("{\"key1\": \"value1\", \"key2\": \"value2\"}".to_string()),
        );
        values.insert(
            "jsonb_array_column".to_string(),
            Some(
                "{\"{\\\"key3\\\": \\\"value3\\\"}\",\"{\\\"key4\\\": \\\"value4\\\"}\"}"
                    .to_string(),
            ),
        );
        values.insert("int4range_column".to_string(), Some("[10,20)".to_string()));
        values.insert(
            "int4range_array_column".to_string(),
            Some("{\"[30,40)\",\"[50,60)\"}".to_string()),
        );

        pg.insert(table_name, values).await.unwrap();
        let x = pg.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", x);
    }

    #[tokio::test]
    async fn test_types10() {
        init_log();
        let table_name = "test_types10";
        let pg = make_test_object();
        let mut conn = pg.get_conn().await.unwrap();
        let _ = sqlx::query(format!("drop table {table_name}").as_str())
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    num_range NUMRANGE,
                    num_range_array NUMRANGE[],
                    ts_range TSRANGE,
                    ts_range_array TSRANGE[],
                    tstz_range TSTZRANGE,
                    tstz_range_array TSTZRANGE[]
                );
        "#,
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        let pg = &pg as &dyn DBTrait;

        let mut values = HashMap::new();
        values.insert("num_range".to_string(), Some("[1,10)".to_string()));
        values.insert(
            "num_range_array".to_string(),
            Some("{\"[1,5)\",\"[6,10]\"}".to_string()),
        );
        values.insert(
            "ts_range".to_string(),
            Some("[\"2024-01-01 00:00:00\",\"2024-01-10 00:00:00\")".to_string()),
        );
        values.insert(
            "ts_range_array".to_string(),
            Some("{\"[\\\"2024-01-01 00:00:00\\\",\\\"2024-01-05 00:00:00\\\")\",\"[\\\"2024-01-06 00:00:00\\\",\\\"2024-01-10 00:00:00\\\"]\"}".to_string()),
        );
        values.insert(
            "tstz_range".to_string(),
            Some("[\"2024-01-01 00:00:00+00\",\"2024-01-10 00:00:00+00\")".to_string()),
        );
        values.insert("tstz_range_array".to_string(), Some("{}".to_string()));

        pg.insert(table_name, values).await.unwrap();
        let x = pg.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", x);
    }

    #[tokio::test]
    async fn test_types11() {
        init_log();
        let table_name = "test_types11";
        let pg = make_test_object();
        let mut conn = pg.get_conn().await.unwrap();
        let _ = sqlx::query(format!("drop table {table_name}").as_str())
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    date_range DATERANGE,
                    date_range_array DATERANGE[],
                    int8_range INT8RANGE,
                    int8_range_array INT8RANGE[],
                    json_path JSONPATH,
                    json_path_array JSONPATH[],
                    money_value MONEY,
                    money_value_array MONEY[]
                );

        "#,
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        let pg = &pg as &dyn DBTrait;

        let mut values = HashMap::new();
        values.insert(
            "date_range".to_string(),
            Some("[2024-01-01,2024-01-11)".to_string()),
        );
        values.insert(
            "date_range_array".to_string(),
            Some("{\"[2024-01-01,2024-01-06)\",\"[2024-01-07,2024-01-10)\"}".to_string()),
        );
        values.insert("int8_range".to_string(), Some("[1,11)".to_string()));
        values.insert(
            "int8_range_array".to_string(),
            Some("{\"[1,6)\",\"[7,10)\"}".to_string()),
        );
        values.insert(
            "json_path".to_string(),
            Some("$.\"store\".\"book\"[0].\"title\"".to_string()),
        );
        values.insert(
            "json_path_array".to_string(),
            Some("{\"$.\\\"store\\\".\\\"book\\\"[0].\\\"title\\\"\"}".to_string()),
        );
        values.insert("money_value".to_string(), Some("$10.50".to_string()));
        values.insert(
            "money_value_array".to_string(),
            Some("{$5.00,$15.75}".to_string()),
        );
        pg.insert(table_name, values).await.unwrap();
        let x = pg.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", x);
    }
}

const PG_TYPES: &[&str] = &[
    "BOOL",
    "BYTEA",
    "NAME",
    "INT8",
    "INT2",
    "INT4",
    "TEXT",
    "OID",
    "JSON",
    "JSON[]",
    "POINT",
    "LSEG",
    "PATH",
    "BOX",
    "POLYGON",
    "LINE",
    "LINE[]",
    "CIDR",
    "CIDR[]",
    "FLOAT4",
    "FLOAT8",
    "UNKNOWN",
    "CIRCLE",
    "CIRCLE[]",
    "MACADDR8",
    "MACADDR8[]",
    "MACADDR",
    "INET",
    "BOOL[]",
    "BYTEA[]",
    "NAME[]",
    "INT2[]",
    "INT4[]",
    "TEXT[]",
    "CHAR[]",
    "VARCHAR[]",
    "INT8[]",
    "POINT[]",
    "LSEG[]",
    "PATH[]",
    "BOX[]",
    "FLOAT4[]",
    "FLOAT8[]",
    "POLYGON[]",
    "OID[]",
    "MACADDR[]",
    "INET[]",
    "CHAR",
    "VARCHAR",
    "DATE",
    "TIME",
    "TIMESTAMP",
    "TIMESTAMP[]",
    "DATE[]",
    "TIME[]",
    "TIMESTAMPTZ",
    "TIMESTAMPTZ[]",
    "INTERVAL",
    "INTERVAL[]",
    "NUMERIC[]",
    "TIMETZ",
    "TIMETZ[]",
    "BIT",
    "BIT[]",
    "VARBIT",
    "VARBIT[]",
    "NUMERIC",
    "RECORD",
    "RECORD[]",
    "UUID",
    "UUID[]",
    "JSONB",
    "JSONB[]",
    "INT4RANGE",
    "INT4RANGE[]",
    "NUMRANGE",
    "NUMRANGE[]",
    "TSRANGE",
    "TSRANGE[]",
    "TSTZRANGE",
    "TSTZRANGE[]",
    "DATERANGE",
    "DATERANGE[]",
    "INT8RANGE",
    "INT8RANGE[]",
    "JSONPATH",
    "JSONPATH[]",
    "MONEY",
    "MONEY[]",
    "VOID",
];

const PG_FUNCTIONS: &[&str] = &[
    "boolin",
    "boolout",
    "byteain",
    "byteaout",
    "charin",
    "charout",
    "namein",
    "nameout",
    "int2in",
    "int2out",
    "int2vectorin",
    "int2vectorout",
    "int4in",
    "int4out",
    "regprocin",
    "regprocout",
    "to_regproc",
    "to_regprocedure",
    "textin",
    "textout",
    "tidin",
    "tidout",
    "xidin",
    "xidout",
    "xid8in",
    "xid8out",
    "xid8recv",
    "xid8send",
    "cidin",
    "cidout",
    "oidvectorin",
    "oidvectorout",
    "boollt",
    "boolgt",
    "booleq",
    "chareq",
    "nameeq",
    "int2eq",
    "int2lt",
    "int4eq",
    "int4lt",
    "texteq",
    "starts_with",
    "text_starts_with_support",
    "xideq",
    "xidneq",
    "xid8eq",
    "xid8ne",
    "xid8lt",
    "xid8gt",
    "xid8le",
    "xid8ge",
    "xid8cmp",
    "xid",
    "xid8_larger",
    "xid8_smaller",
    "cideq",
    "charne",
    "charlt",
    "charle",
    "chargt",
    "charge",
    "int4",
    "char",
    "nameregexeq",
    "nameregexne",
    "textregexeq",
    "textregexne",
    "textregexeq_support",
    "textlen",
    "textcat",
    "boolne",
    "version",
    "pg_ddl_command_in",
    "pg_ddl_command_out",
    "pg_ddl_command_recv",
    "pg_ddl_command_send",
    "eqsel",
    "neqsel",
    "scalarltsel",
    "scalargtsel",
    "eqjoinsel",
    "neqjoinsel",
    "scalarltjoinsel",
    "scalargtjoinsel",
    "scalarlesel",
    "scalargesel",
    "scalarlejoinsel",
    "scalargejoinsel",
    "unknownin",
    "unknownout",
    "box_above_eq",
    "box_below_eq",
    "point_in",
    "point_out",
    "lseg_in",
    "lseg_out",
    "path_in",
    "path_out",
    "box_in",
    "box_out",
    "box_overlap",
    "box_ge",
    "box_gt",
    "box_eq",
    "box_lt",
    "box_le",
    "point_above",
    "point_left",
    "point_right",
    "point_below",
    "point_eq",
    "on_pb",
    "on_ppath",
    "box_center",
    "areasel",
    "areajoinsel",
    "int4mul",
    "int4ne",
    "int2ne",
    "int2gt",
    "int4gt",
    "int2le",
    "int4le",
    "int4ge",
    "int2ge",
    "int2mul",
    "int2div",
    "int4div",
    "int2mod",
    "int4mod",
    "textne",
    "int24eq",
    "int42eq",
    "int24lt",
    "int42lt",
    "int24gt",
    "int42gt",
    "int24ne",
    "int42ne",
    "int24le",
    "int42le",
    "int24ge",
    "int42ge",
    "int24mul",
    "int42mul",
    "int24div",
    "int42div",
    "int2pl",
    "int4pl",
    "int24pl",
    "int42pl",
    "int2mi",
    "int4mi",
    "int24mi",
    "int42mi",
    "oideq",
    "oidne",
    "box_same",
    "box_contain",
    "box_left",
    "box_overleft",
    "box_overright",
    "box_right",
    "box_contained",
    "box_contain_pt",
    "pg_node_tree_in",
    "pg_node_tree_out",
    "pg_node_tree_recv",
    "pg_node_tree_send",
    "float4in",
    "float4out",
    "float4mul",
    "float4div",
    "float4pl",
    "float4mi",
    "float4um",
    "float4abs",
    "float4_accum",
    "float4larger",
    "float4smaller",
    "int4um",
    "int2um",
    "float8in",
    "float8out",
    "float8mul",
    "float8div",
    "float8pl",
    "float8mi",
    "float8um",
    "float8abs",
    "float8_accum",
    "float8_combine",
    "float8larger",
    "float8smaller",
    "lseg_center",
    "poly_center",
    "dround",
    "dtrunc",
    "ceil",
    "ceiling",
    "floor",
    "sign",
    "dsqrt",
    "dcbrt",
    "dpow",
    "dexp",
    "dlog1",
    "float8",
    "float4",
    "int2",
    "int2",
    "line_distance",
    "nameeqtext",
    "namelttext",
    "nameletext",
    "namegetext",
    "namegttext",
    "namenetext",
    "btnametextcmp",
    "texteqname",
    "textltname",
    "textlename",
    "textgename",
    "textgtname",
    "textnename",
    "bttextnamecmp",
    "nameconcatoid",
    "timeofday",
    "inter_sl",
    "inter_lb",
    "float48mul",
    "float48div",
    "float48pl",
    "float48mi",
    "float84mul",
    "float84div",
    "float84pl",
    "float84mi",
    "float4eq",
    "float4ne",
    "float4lt",
    "float4le",
    "float4gt",
    "float4ge",
    "float8eq",
    "float8ne",
    "float8lt",
    "float8le",
    "float8gt",
    "float8ge",
    "float48eq",
    "float48ne",
    "float48lt",
    "float48le",
    "float48gt",
    "float48ge",
    "float84eq",
    "float84ne",
    "float84lt",
    "float84le",
    "float84gt",
    "float84ge",
    "width_bucket",
    "float8",
    "float4",
    "int4",
    "int2",
    "float8",
    "int4",
    "float4",
    "int4",
    "heap_tableam_handler",
    "bthandler",
    "hashhandler",
    "gisthandler",
    "ginhandler",
    "spghandler",
    "brinhandler",
    "brin_summarize_new_values",
    "brin_summarize_range",
    "brin_desummarize_range",
    "amvalidate",
    "pg_indexam_has_property",
    "pg_index_has_property",
    "pg_index_column_has_property",
    "pg_indexam_progress_phasename",
    "poly_same",
    "poly_contain",
    "poly_left",
    "poly_overleft",
    "poly_overright",
    "poly_right",
    "poly_contained",
    "poly_overlap",
    "poly_in",
    "poly_out",
    "btint2cmp",
    "btint2sortsupport",
    "btint4cmp",
    "btint4sortsupport",
    "btint8cmp",
    "btint8sortsupport",
    "btfloat4cmp",
    "btfloat4sortsupport",
    "btfloat8cmp",
    "btfloat8sortsupport",
    "btoidcmp",
    "btoidsortsupport",
    "btoidvectorcmp",
    "btcharcmp",
    "btnamecmp",
    "btnamesortsupport",
    "bttextcmp",
    "bttextsortsupport",
    "btvarstrequalimage",
    "cash_cmp",
    "btarraycmp",
    "in_range",
    "in_range",
    "in_range",
    "in_range",
    "in_range",
    "in_range",
    "in_range",
    "in_range",
    "in_range",
    "in_range",
    "lseg_distance",
    "lseg_interpt",
    "dist_ps",
    "dist_sp",
    "dist_pb",
    "dist_bp",
    "dist_sb",
    "dist_bs",
    "close_ps",
    "close_pb",
    "close_sb",
    "on_ps",
    "path_distance",
    "dist_ppath",
    "dist_pathp",
    "on_sb",
    "inter_sb",
    "text",
    "text",
    "name",
    "bpchar",
    "name",
    "hashint2",
    "hashint2extended",
    "hashint4",
    "hashint4extended",
    "hashint8",
    "hashint8extended",
    "hashfloat4",
    "hashfloat4extended",
    "hashfloat8",
    "hashfloat8extended",
    "hashoid",
    "hashoidextended",
    "hashchar",
    "hashcharextended",
    "hashname",
    "hashnameextended",
    "hashtext",
    "hashtextextended",
    "hashvarlena",
    "hashvarlenaextended",
    "hashoidvector",
    "hashoidvectorextended",
    "hash_aclitem",
    "hash_aclitem_extended",
    "hashmacaddr",
    "hashmacaddrextended",
    "hashinet",
    "hashinetextended",
    "hash_numeric",
    "hash_numeric_extended",
    "hashmacaddr8",
    "hashmacaddr8extended",
    "num_nulls",
    "num_nonnulls",
    "text_larger",
    "text_smaller",
    "int8in",
    "int8out",
    "int8um",
    "int8pl",
    "int8mi",
    "int8mul",
    "int8div",
    "int8eq",
    "int8ne",
    "int8lt",
    "int8gt",
    "int8le",
    "int8ge",
    "int84eq",
    "int84ne",
    "int84lt",
    "int84gt",
    "int84le",
    "int84ge",
    "int4",
    "int8",
    "float8",
    "int8",
    "hash_array",
    "hash_array_extended",
    "float4",
    "int8",
    "int2",
    "int8",
    "namelt",
    "namele",
    "namegt",
    "namege",
    "namene",
    "bpchar",
    "varchar_support",
    "varchar",
    "oidvectorne",
    "oidvectorlt",
    "oidvectorle",
    "oidvectoreq",
    "oidvectorge",
    "oidvectorgt",
    "getpgusername",
    "oidlt",
    "oidle",
    "octet_length",
    "get_byte",
    "set_byte",
    "get_bit",
    "set_bit",
    "overlay",
    "overlay",
    "bit_count",
    "dist_pl",
    "dist_lp",
    "dist_sl",
    "dist_ls",
    "dist_cpoly",
    "dist_polyc",
    "poly_distance",
    "dist_ppoly",
    "dist_polyp",
    "dist_cpoint",
    "text_lt",
    "text_le",
    "text_gt",
    "text_ge",
    "current_user",
    "session_user",
    "system_user",
    "array_eq",
    "array_ne",
    "array_lt",
    "array_gt",
    "array_le",
    "array_ge",
    "array_dims",
    "array_ndims",
    "array_in",
    "array_out",
    "array_lower",
    "array_upper",
    "array_length",
    "cardinality",
    "array_append",
    "array_prepend",
    "array_cat",
    "string_to_array",
    "string_to_array",
    "string_to_table",
    "string_to_table",
    "array_to_string",
    "array_to_string",
    "array_larger",
    "array_smaller",
    "array_position",
    "array_position",
    "array_positions",
    "generate_subscripts",
    "generate_subscripts",
    "array_fill",
    "array_fill",
    "unnest",
    "array_unnest_support",
    "array_remove",
    "array_replace",
    "array_agg_transfn",
    "array_agg_combine",
    "array_agg_serialize",
    "array_agg_deserialize",
    "array_agg_finalfn",
    "array_agg",
    "array_agg_array_transfn",
];

const PG_KEYWORDS: &[&str] = &[
    "ABORT",
    "ABS",
    "ABSENT",
    "ABSOLUTE",
    "ACCESS",
    "ACCORDING",
    "ACOS",
    "ACTION",
    "ADA",
    "ADD",
    "ADMIN",
    "AFTER",
    "AGGREGATE",
    "ALL",
    "ALLOCATE",
    "ALSO",
    "ALTER",
    "ALWAYS",
    "ANALYSE",
    "ANALYZE",
    "AND",
    "ANY",
    "ANY_VALUE",
    "ARE",
    "ARRAY",
    "ARRAY_AGG",
    "ARRAY_​MAX_​CARDINALITY",
    "AS",
    "ASC",
    "ASENSITIVE",
    "ASIN",
    "ASSERTION",
    "ASSIGNMENT",
    "ASYMMETRIC",
    "AT",
    "ATAN",
    "ATOMIC",
    "ATTACH",
    "ATTRIBUTE",
    "ATTRIBUTES",
    "AUTHORIZATION",
    "AVG",
    "BACKWARD",
    "BASE64",
    "BEFORE",
    "BEGIN",
    "BEGIN_FRAME",
    "BEGIN_PARTITION",
    "BERNOULLI",
    "BETWEEN",
    "BIGINT",
    "BINARY",
    "BIT",
    "BIT_LENGTH",
    "BLOB",
    "BLOCKED",
    "BOM",
    "BOOLEAN",
    "BOTH",
    "BREADTH",
    "BTRIM",
    "BY",
    "C",
    "CACHE",
    "CALL",
    "CALLED",
    "CARDINALITY",
    "CASCADE",
    "CASCADED",
    "CASE",
    "CAST",
    "CATALOG",
    "CATALOG_NAME",
    "CEIL",
    "CEILING",
    "CHAIN",
    "CHAINING",
    "CHAR",
    "CHARACTER",
    "CHARACTERISTICS",
    "CHARACTERS",
    "CHARACTER_LENGTH",
    "CHARACTER_​SET_​CATALOG",
    "CHARACTER_SET_NAME",
    "CHARACTER_SET_SCHEMA",
    "CHAR_LENGTH",
    "CHECK",
    "CHECKPOINT",
    "CLASS",
    "CLASSIFIER",
    "CLASS_ORIGIN",
    "CLOB",
    "CLOSE",
    "CLUSTER",
    "COALESCE",
    "COBOL",
    "COLLATE",
    "COLLATION",
    "COLLATION_CATALOG",
    "COLLATION_NAME",
    "COLLATION_SCHEMA",
    "COLLECT",
    "COLUMN",
    "COLUMNS",
    "COLUMN_NAME",
    "COMMAND_FUNCTION",
    "COMMAND_​FUNCTION_​CODE",
    "COMMENT",
    "COMMENTS",
    "COMMIT",
    "COMMITTED",
    "COMPRESSION",
    "CONCURRENTLY",
    "CONDITION",
    "CONDITIONAL",
    "CONDITION_NUMBER",
    "CONFIGURATION",
    "CONFLICT",
    "CONNECT",
    "CONNECTION",
    "CONNECTION_NAME",
    "CONSTRAINT",
    "CONSTRAINTS",
    "CONSTRAINT_CATALOG",
    "CONSTRAINT_NAME",
    "CONSTRAINT_SCHEMA",
    "CONSTRUCTOR",
    "CONTAINS",
    "CONTENT",
    "CONTINUE",
    "CONTROL",
    "CONVERSION",
    "CONVERT",
    "COPARTITION",
    "COPY",
    "CORR",
    "CORRESPONDING",
    "COS",
    "COSH",
    "COST",
    "COUNT",
    "COVAR_POP",
    "COVAR_SAMP",
    "CREATE",
    "CROSS",
    "CSV",
    "CUBE",
    "CUME_DIST",
    "CURRENT",
    "CURRENT_CATALOG",
    "CURRENT_DATE",
    "CURRENT_DEFAULT_TRANSFORM_GROUP",
    "CURRENT_PATH",
    "CURRENT_ROLE",
    "CURRENT_ROW",
    "CURRENT_SCHEMA",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "CURRENT_TRANSFORM_GROUP_FOR_TYPE",
    "CURRENT_USER",
    "CURSOR",
    "CURSOR_NAME",
    "CYCLE",
    "DATA",
    "DATABASE",
    "DATALINK",
    "DATE",
    "DATETIME_INTERVAL_CODE",
    "DATETIME_INTERVAL_PRECISION",
    "DAY",
    "DB",
    "DEALLOCATE",
    "DEC",
    "DECFLOAT",
    "DECIMAL",
    "DECLARE",
    "DEFAULT",
    "DEFAULTS",
    "DEFERRABLE",
    "DEFERRED",
    "DEFINE",
    "DEFINED",
    "DEFINER",
    "DEGREE",
    "DELETE",
    "DELIMITER",
    "DELIMITERS",
    "DENSE_RANK",
    "DEPENDS",
    "DEPTH",
    "DEREF",
    "DERIVED",
    "DESC",
    "DESCRIBE",
    "DESCRIPTOR",
    "DETACH",
    "DETERMINISTIC",
    "DIAGNOSTICS",
    "DICTIONARY",
    "DISABLE",
    "DISCARD",
    "DISCONNECT",
    "DISPATCH",
    "DISTINCT",
    "DLNEWCOPY",
    "DLPREVIOUSCOPY",
    "DLURLCOMPLETE",
    "DLURLCOMPLETEONLY",
    "DLURLCOMPLETEWRITE",
    "DLURLPATH",
    "DLURLPATHONLY",
    "DLURLPATHWRITE",
    "DLURLSCHEME",
    "DLURLSERVER",
    "DLVALUE",
    "DO",
    "DOCUMENT",
    "DOMAIN",
    "DOUBLE",
    "DROP",
    "DYNAMIC",
    "DYNAMIC_FUNCTION",
    "DYNAMIC_FUNCTION_CODE",
    "EACH",
    "ELEMENT",
    "ELSE",
    "EMPTY",
    "ENABLE",
    "ENCODING",
    "ENCRYPTED",
    "END",
    "END-EXEC",
    "END_FRAME",
    "END_PARTITION",
    "ENFORCED",
    "ENUM",
    "EQUALS",
    "ERROR",
    "ESCAPE",
    "EVENT",
    "EVERY",
    "EXCEPT",
    "EXCEPTION",
    "EXCLUDE",
    "EXCLUDING",
    "EXCLUSIVE",
    "EXEC",
    "EXECUTE",
    "EXISTS",
    "EXP",
    "EXPLAIN",
    "EXPRESSION",
    "EXTENSION",
    "EXTERNAL",
    "EXTRACT",
    "FALSE",
    "FAMILY",
    "FETCH",
    "FILE",
    "FILTER",
    "FINAL",
    "FINALIZE",
    "FINISH",
    "FIRST",
    "FIRST_VALUE",
    "FLAG",
    "FLOAT",
    "FLOOR",
    "FOLLOWING",
    "FOR",
    "FORCE",
    "FOREIGN",
    "FORMAT",
    "FORTRAN",
    "FORWARD",
    "FOUND",
    "FRAME_ROW",
    "FREE",
    "FREEZE",
    "FROM",
    "FS",
    "FULFILL",
    "FULL",
    "FUNCTION",
    "FUNCTIONS",
    "FUSION",
    "G",
    "GENERAL",
    "GENERATED",
    "GET",
    "GLOBAL",
    "GO",
    "GOTO",
    "GRANT",
    "GRANTED",
    "GREATEST",
    "GROUP",
    "GROUPING",
    "GROUPS",
    "HANDLER",
    "HAVING",
    "HEADER",
    "HEX",
    "HIERARCHY",
    "HOLD",
    "HOUR",
    "ID",
    "IDENTITY",
    "IF",
    "IGNORE",
    "ILIKE",
    "IMMEDIATE",
    "IMMEDIATELY",
    "IMMUTABLE",
    "IMPLEMENTATION",
    "IMPLICIT",
    "IMPORT",
    "IN",
    "INCLUDE",
    "INCLUDING",
    "INCREMENT",
    "INDENT",
    "INDEX",
    "INDEXES",
    "INDICATOR",
    "INHERIT",
    "INHERITS",
    "INITIAL",
    "INITIALLY",
    "INLINE",
    "INNER",
    "INOUT",
    "INPUT",
    "INSENSITIVE",
    "INSERT",
    "INSTANCE",
    "INSTANTIABLE",
    "INSTEAD",
    "INT",
    "INTEGER",
    "INTEGRITY",
    "INTERSECT",
    "INTERSECTION",
    "INTERVAL",
    "INTO",
    "INVOKER",
    "IS",
    "ISNULL",
    "ISOLATION",
    "JOIN",
    "JSON",
    "JSON_ARRAY",
    "JSON_ARRAYAGG",
    "JSON_EXISTS",
    "JSON_OBJECT",
    "JSON_OBJECTAGG",
    "JSON_QUERY",
    "JSON_SCALAR",
    "JSON_SERIALIZE",
    "JSON_TABLE",
    "JSON_TABLE_PRIMITIVE",
    "JSON_VALUE",
    "K",
    "KEEP",
    "KEY",
    "KEYS",
    "KEY_MEMBER",
    "KEY_TYPE",
    "LABEL",
    "LAG",
    "LANGUAGE",
    "LARGE",
    "LAST",
    "LAST_VALUE",
    "LATERAL",
    "LEAD",
    "LEADING",
    "LEAKPROOF",
    "LEAST",
    "LEFT",
    "LENGTH",
    "LEVEL",
    "LIBRARY",
    "LIKE",
    "LIKE_REGEX",
    "LIMIT",
    "LINK",
    "LISTAGG",
    "LISTEN",
    "LN",
    "LOAD",
    "LOCAL",
    "LOCALTIME",
    "LOCALTIMESTAMP",
    "LOCATION",
    "LOCATOR",
    "LOCK",
    "LOCKED",
    "LOG",
    "LOG10",
    "LOGGED",
    "LOWER",
    "LPAD",
    "LTRIM",
    "M",
    "MAP",
    "MAPPING",
    "MATCH",
    "MATCHED",
    "MATCHES",
    "MATCH_NUMBER",
    "MATCH_RECOGNIZE",
    "MATERIALIZED",
    "MAX",
    "MAXVALUE",
    "MEASURES",
    "MEMBER",
    "MERGE",
    "MESSAGE_LENGTH",
    "MESSAGE_OCTET_LENGTH",
    "MESSAGE_TEXT",
    "METHOD",
    "MIN",
    "MINUTE",
    "MINVALUE",
    "MOD",
    "MODE",
    "MODIFIES",
    "MODULE",
    "MONTH",
    "MORE",
    "MOVE",
    "MULTISET",
    "MUMPS",
    "NAME",
    "NAMES",
    "NAMESPACE",
    "NATIONAL",
    "NATURAL",
    "NCHAR",
    "NCLOB",
    "NESTED",
    "NESTING",
    "NEW",
    "NEXT",
    "NFC",
    "NFD",
    "NFKC",
    "NFKD",
    "NIL",
    "NO",
    "NONE",
    "NORMALIZE",
    "NORMALIZED",
    "NOT",
    "NOTHING",
    "NOTIFY",
    "NOTNULL",
    "NOWAIT",
    "NTH_VALUE",
    "NTILE",
    "NULL",
    "NULLABLE",
    "NULLIF",
    "NULLS",
    "NULL_ORDERING",
    "NUMBER",
    "NUMERIC",
    "OBJECT",
    "OCCURRENCE",
    "OCCURRENCES_REGEX",
    "OCTETS",
    "OCTET_LENGTH",
    "OF",
    "OFF",
    "OFFSET",
    "OIDS",
    "OLD",
    "OMIT",
    "ON",
    "ONE",
    "ONLY",
    "OPEN",
    "OPERATOR",
    "OPTION",
    "OPTIONS",
    "OR",
    "ORDER",
    "ORDERING",
    "ORDINALITY",
    "OTHERS",
    "OUT",
    "OUTER",
    "OUTPUT",
    "OVER",
    "OVERFLOW",
    "OVERLAPS",
    "OVERLAY",
    "OVERRIDING",
    "OWNED",
    "OWNER",
    "P",
    "PAD",
    "PARALLEL",
    "PARAMETER",
    "PARAMETER_MODE",
    "PARAMETER_NAME",
    "PARAMETER_​ORDINAL_​POSITION",
    "PARAMETER_​SPECIFIC_​CATALOG",
    "PARAMETER_​SPECIFIC_​NAME",
    "PARAMETER_​SPECIFIC_​SCHEMA",
    "PARSER",
    "PARTIAL",
    "PARTITION",
    "PASCAL",
    "PASS",
    "PASSING",
    "PASSTHROUGH",
    "PASSWORD",
    "PAST",
    "PATH",
    "PATTERN",
    "PER",
    "PERCENT",
    "PERCENTILE_CONT",
    "PERCENTILE_DISC",
    "PERCENT_RANK",
    "PERIOD",
    "PERMISSION",
    "PERMUTE",
    "PIPE",
    "PLACING",
    "PLAN",
    "PLANS",
    "PLI",
    "POLICY",
    "PORTION",
    "POSITION",
    "POSITION_REGEX",
    "POWER",
    "PRECEDES",
    "PRECEDING",
    "PRECISION",
    "PREPARE",
    "PREPARED",
    "PRESERVE",
    "PREV",
    "PRIMARY",
    "PRIOR",
    "PRIVATE",
    "PRIVILEGES",
    "PROCEDURAL",
    "PROCEDURE",
    "PROCEDURES",
    "PROGRAM",
    "PRUNE",
    "PTF",
    "PUBLIC",
    "PUBLICATION",
    "QUOTE",
    "QUOTES",
    "RANGE",
    "RANK",
    "READ",
    "READS",
    "REAL",
    "REASSIGN",
    "RECHECK",
    "RECOVERY",
    "RECURSIVE",
    "REF",
    "REFERENCES",
    "REFERENCING",
    "REFRESH",
    "REGR_AVGX",
    "REGR_AVGY",
    "REGR_COUNT",
    "REGR_INTERCEPT",
    "REGR_R2",
    "REGR_SLOPE",
    "REGR_SXX",
    "REGR_SXY",
    "REGR_SYY",
    "REINDEX",
    "RELATIVE",
    "RELEASE",
    "RENAME",
    "REPEATABLE",
    "REPLACE",
    "REPLICA",
    "REQUIRING",
    "RESET",
    "RESPECT",
    "RESTART",
    "RESTORE",
    "RESTRICT",
    "RESULT",
    "RETURN",
    "RETURNED_CARDINALITY",
    "RETURNED_LENGTH",
    "RETURNED_OCTET_LENGTH",
    "RETURNED_SQLSTATE",
    "RETURNING",
    "RETURNS",
    "REVOKE",
    "RIGHT",
    "ROLE",
    "ROLLBACK",
    "ROLLUP",
    "ROUTINE",
    "ROUTINES",
    "ROUTINE_CATALOG",
    "ROUTINE_NAME",
    "ROUTINE_SCHEMA",
    "ROW",
    "ROWS",
    "ROW_COUNT",
    "ROW_NUMBER",
    "RPAD",
    "RTRIM",
    "RULE",
    "RUNNING",
    "SAVEPOINT",
    "SCALAR",
    "SCALE",
    "SCHEMA",
    "SCHEMAS",
    "SCHEMA_NAME",
    "SCOPE",
    "SCOPE_CATALOG",
    "SCOPE_NAME",
    "SCOPE_SCHEMA",
    "SCROLL",
    "SEARCH",
    "SECOND",
    "SECTION",
    "SECURITY",
    "SEEK",
    "SELECT",
    "SELECTIVE",
    "SELF",
    "SEMANTICS",
    "SENSITIVE",
    "SEQUENCE",
    "SEQUENCES",
    "SERIALIZABLE",
    "SERVER",
    "SERVER_NAME",
    "SESSION",
    "SESSION_USER",
    "SET",
    "SETOF",
    "SETS",
    "SHARE",
    "SHOW",
    "SIMILAR",
    "SIMPLE",
    "SIN",
    "SINH",
    "SIZE",
    "SKIP",
    "SMALLINT",
    "SNAPSHOT",
    "SOME",
    "SORT_DIRECTION",
    "SOURCE",
    "SPACE",
    "SPECIFIC",
    "SPECIFICTYPE",
    "SPECIFIC_NAME",
    "SQL",
    "SQLCODE",
    "SQLERROR",
    "SQLEXCEPTION",
    "SQLSTATE",
    "SQLWARNING",
    "SQRT",
    "STABLE",
    "STANDALONE",
    "START",
    "STATE",
    "STATEMENT",
    "STATIC",
    "STATISTICS",
    "STDDEV_POP",
    "STDDEV_SAMP",
    "STDIN",
    "STDOUT",
    "STORAGE",
    "STORED",
    "STRICT",
    "STRING",
    "STRIP",
    "STRUCTURE",
    "STYLE",
    "SUBCLASS_ORIGIN",
    "SUBMULTISET",
    "SUBSCRIPTION",
    "SUBSET",
    "SUBSTRING",
    "SUBSTRING_REGEX",
    "SUCCEEDS",
    "SUM",
    "SUPPORT",
    "SYMMETRIC",
    "SYSID",
    "SYSTEM",
    "SYSTEM_TIME",
    "SYSTEM_USER",
    "T",
    "TABLE",
    "TABLES",
    "TABLESAMPLE",
    "TABLESPACE",
    "TABLE_NAME",
    "TAN",
    "TANH",
    "TEMP",
    "TEMPLATE",
    "TEMPORARY",
    "TEXT",
    "THEN",
    "THROUGH",
    "TIES",
    "TIME",
    "TIMESTAMP",
    "TIMEZONE_HOUR",
    "TIMEZONE_MINUTE",
    "TO",
    "TOKEN",
    "TOP_LEVEL_COUNT",
    "TRAILING",
    "TRANSACTION",
    "TRANSACTIONS_​COMMITTED",
    "TRANSACTIONS_​ROLLED_​BACK",
    "TRANSACTION_ACTIVE",
    "TRANSFORM",
    "TRANSFORMS",
    "TRANSLATE",
    "TRANSLATE_REGEX",
    "TRANSLATION",
    "TREAT",
    "TRIGGER",
    "TRIGGER_CATALOG",
    "TRIGGER_NAME",
    "TRIGGER_SCHEMA",
    "TRIM",
    "TRIM_ARRAY",
    "TRUE",
    "TRUNCATE",
    "TRUSTED",
    "TYPE",
    "TYPES",
    "UESCAPE",
    "UNBOUNDED",
    "UNCOMMITTED",
    "UNCONDITIONAL",
    "UNDER",
    "UNENCRYPTED",
    "UNION",
    "UNIQUE",
    "UNKNOWN",
    "UNLINK",
    "UNLISTEN",
    "UNLOGGED",
    "UNMATCHED",
    "UNNAMED",
    "UNNEST",
    "UNTIL",
    "UNTYPED",
    "UPDATE",
    "UPPER",
    "URI",
    "USAGE",
    "USER",
    "USER_​DEFINED_​TYPE_​CATALOG",
    "USER_​DEFINED_​TYPE_​CODE",
    "USER_​DEFINED_​TYPE_​NAME",
    "USER_​DEFINED_​TYPE_​SCHEMA",
    "USING",
    "UTF16",
    "UTF32",
    "UTF8",
    "VACUUM",
    "VALID",
    "VALIDATE",
    "VALIDATOR",
    "VALUE",
    "VALUES",
    "VALUE_OF",
    "VARBINARY",
    "VARCHAR",
    "VARIADIC",
    "VARYING",
    "VAR_POP",
    "VAR_SAMP",
    "VERBOSE",
    "VERSION",
    "VERSIONING",
    "VIEW",
    "VIEWS",
    "VOLATILE",
    "WHEN",
    "WHENEVER",
    "WHERE",
    "WHITESPACE",
    "WIDTH_BUCKET",
    "WINDOW",
    "WITH",
    "WITHIN",
    "WITHOUT",
    "WORK",
    "WRAPPER",
    "WRITE",
    "XML",
    "XMLAGG",
    "XMLATTRIBUTES",
    "XMLBINARY",
    "XMLCAST",
    "XMLCOMMENT",
    "XMLCONCAT",
    "XMLDECLARATION",
    "XMLDOCUMENT",
    "XMLELEMENT",
    "XMLEXISTS",
    "XMLFOREST",
    "XMLITERATE",
    "XMLNAMESPACES",
    "XMLPARSE",
    "XMLPI",
    "XMLQUERY",
    "XMLROOT",
    "XMLSCHEMA",
    "XMLSERIALIZE",
    "XMLTABLE",
    "XMLTEXT",
    "XMLVALIDATE",
    "YEAR",
    "YES",
    "ZONE",
];
