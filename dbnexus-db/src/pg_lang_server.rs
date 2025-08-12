use std::ops::DerefMut;
use std::sync::Arc;

use sqlx::{AssertSqlSafe, Column, Executor, Row, SqlSafeStr, Statement};

use crate::db::{CatalogTrait, FormatSqlOpts};
use crate::helper::sqlformat_format_sql;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, LangServerCommon, LanguageServer,
    LexerCompletionTrait, try_completion_on_syntax,
};
use crate::pg::PostGresDB;
use crate::sqlparser_completion::TempTableColumn;
#[async_trait::async_trait]
impl LexerCompletionTrait for PostGresDB {
    type Dialect = sqlparser::dialect::PostgreSqlDialect;

    const TYPES: &'static [&'static str] = TYPES;

    async fn list_procedure(&self) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_procedure(self, None, None).await
    }
    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        let mut ret = AstCompletionHelper::list_table(self, None, None).await?;
        ret.extend(
            AstCompletionHelper::list_view(self, None, None)
                .await?
                .into_iter(),
        );
        anyhow::Ok(ret)
    }

    async fn list_columns(&self, table_name: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        AstCompletionHelper::list_column(self, None, None, table_name, false).await
    }

    async fn get_key_words_fn(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut lock = self.key_word_completion.lock().await;
        if lock.len() > 0 {
            return anyhow::Ok(lock.clone());
        }
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query("select word from pg_get_keywords()")
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = vec![];
        for row in rows.iter() {
            let c: String = row.get_unchecked(0);
            ret.push(CompletionItem {
                item: c.to_uppercase().into(),
                kind: CompletionKind::KeyWord,
                desc: None,
            });
        }
        *lock = ret;
        return anyhow::Ok(lock.clone());
    }

    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self, None, None, true).await
    }
}

impl PostGresDB {
    fn db_merge_config<'a, 'b>(&'a self, db: Option<&'b str>) -> Option<&'a str>
    where
        'b: 'a,
    {
        if db.is_some() {
            return db;
        }
        return self.config.db.as_ref().map(|x| x.as_str());
    }

    fn schema_merge_config<'a, 'b>(&'a self, schema: Option<&'b str>) -> Option<&'a str>
    where
        'b: 'a,
    {
        if schema.is_some() {
            return schema;
        }
        return self.config.schema.as_deref();
    }
}

impl PostGresDB {
    async fn list_build_functions(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut lock = self.build_in_fn.lock().await;
        if lock.len() > 0 {
            return anyhow::Ok(lock.clone());
        }
        let sql = r#"SELECT p.proname AS function_name,
             d.description AS function_description
            FROM pg_catalog.pg_proc p
            JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            LEFT JOIN pg_catalog.pg_description d ON d.objoid = p.oid
            WHERE n.nspname = 'pg_catalog'  -- 只查询内置函数
            ORDER BY p.proname;"#;
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(AssertSqlSafe(sql))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            let desc: Option<String> = r.try_get(1)?;
            ret.push(CompletionItem {
                item: name.into(),
                kind: CompletionKind::Function,
                desc: desc.map(|x| x.into()),
            });
        }
        *lock = ret.clone();
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]

impl AstCompletionHelper for PostGresDB {
    async fn list_table(
        &self, db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let mut sql =
            r#"select table_name FROM information_schema.tables  WHERE table_type = 'BASE TABLE' "#
                .to_string();
        if let Some(db) = self.db_merge_config(db) {
            sql.push_str(format!(" AND table_catalog = '{}'", db).as_str());
        }
        if let Some(schema) = self.schema_merge_config(schema) {
            sql.push_str(format!(" AND table_schema = '{}'", schema).as_str());
        }
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_procedure(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let sql = if let Some(schema) = self.schema_merge_config(schema) {
            format!(
                r#"
            SELECT 
                p.proname AS procedure_name
            FROM 
                pg_catalog.pg_proc p
            JOIN 
                pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            WHERE 
                n.nspname = '{}'  -- 指定目标模式
                AND p.prokind = 'p'  -- 仅选择存储过程
            ORDER BY 
                p.proname;
            "#,
                schema
            )
        } else {
            format!(
                r#"
            SELECT 
                p.proname AS procedure_name
            FROM 
                pg_catalog.pg_proc p
            JOIN 
                pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            WHERE 
                p.prokind = 'p'  -- 仅选择存储过程
            ORDER BY 
                p.proname;
            "#
            )
        };
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_view(
        &self,
        _db: Option<&str>, // can't filter on catelog 根据表结构
        schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let mut sql = "SELECT viewname FROM pg_views ".to_string();
        if let Some(schema) = self.schema_merge_config(schema) {
            sql.push_str(format!(" WHERE schemaname='{}'", schema).as_str());
        }
        // postgres has materilized view in other table
        sql.push_str(format!("UNION SELECT matviewname from pg_matviews").as_str());

        if let Some(schema) = self.schema_merge_config(schema) {
            sql.push_str(format!(" WHERE schemaname='{}'", schema).as_str());
        }

        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }

        anyhow::Ok(ret)
    }

    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        let mut conn = self.get_conn().await?;
        let s = conn
            .deref_mut()
            .prepare_with(AssertSqlSafe(sql).into_sql_str(), &[])
            .await?;
        let ret = s
            .columns()
            .iter()
            .map(|x| TempTableColumn {
                name: x.name().to_string().into(),
                typ: Some(x.type_info().to_string().into()),
            })
            .collect::<Vec<_>>();
        anyhow::Ok(ret)
    }

    async fn list_column(
        &self, db: Option<&str>, schema: Option<&str>, name: &str, is_call: bool,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = format!(
            r#"select * from {}{}"{}"{}"#,
            match db {
                Some(x) => format!("\"{}\".", x),
                None => "".to_string(),
            },
            match schema {
                Some(x) => format!("\"{}\".", x),
                None => "".to_string(),
            },
            name,
            if is_call { "()" } else { "" }
        );
        log::info!("sql:{}", sql);
        self.get_query_columns(&sql).await
    }

    async fn list_function(
        &self,
        _db: Option<&str>, // how to filter on this???
        schema: Option<&str>,
        include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let sql;
        if let Some(schema) = self.schema_merge_config(schema) {
            sql = format!(
                r#"
            SELECT  
                p.proname AS function_name
            FROM pg_catalog.pg_proc p
            JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            WHERE n.nspname = '{}'  -- 替换为你的模式名
            ORDER BY p.proname;
        "#,
                schema
            );
        } else {
            sql = format!(
                r#"
            SELECT 
                p.proname AS function_name
            FROM pg_catalog.pg_proc p
            JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            ORDER BY p.proname;
        "#,
            );
        }
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(CompletionItem {
                item: name.into(),
                desc: None,
                kind: CompletionKind::Function,
            });
        }
        if include_build_in {
            ret.extend(self.list_build_functions().await?.into_iter());
        }
        anyhow::Ok(ret)
    }

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let sql = if let Some(schema) = self.schema_merge_config(schema) {
            format!(
                r#"
            SELECT indexname AS index_name
                FROM pg_indexes
                WHERE schemaname = '{}';
                "#,
                schema
            )
        } else {
            r#"
            SELECT indexname AS index_name
                FROM pg_indexes
                "#
            .to_string()
        };
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_variable(&self) -> anyhow::Result<Vec<String>> {
        let sql = "show all";
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(AssertSqlSafe(sql))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn postgres_table_policy(
        &self, _db: Option<&str>, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let sql = if let Some(schema) = self.schema_merge_config(schema) {
            format!(
                r#"SELECT policyname
                FROM pg_policies
                WHERE tablename = '{}' AND schemaname='{}';  -- 替换为你的表名"#,
                table, schema
            )
        } else {
            format!(
                r#"SELECT policyname
                FROM pg_policies
                WHERE tablename = '{}';  -- 替换为你的表名"#,
                table
            )
        };
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }

    async fn list_role(&self) -> anyhow::Result<Vec<String>> {
        let sql = r#"
        SELECT rolname AS role_name
            FROM pg_roles
        "#;
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(AssertSqlSafe(sql))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }

    async fn list_postgres_rule(
        &self, _db: Option<&str>, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        // 这里schemaname实际上是db的名字 我观察出来的
        // 对吗？？？
        let sql = if let Some(schema) = self.db_merge_config(schema) {
            format!(
                r#"select rulename from pg_rules where tablename='{}' and schemaname='{}'"#,
                table, schema
            )
        } else {
            format!(
                r#"select rulename from pg_rules where tablename='{}'"#,
                table
            )
        };
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }

    async fn list_constraint(
        &self, db: Option<&str>, schema: Option<&str>, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        let mut sql =
            "SELECT constraint_name FROM information_schema.table_constraints".to_string();
        let mut has_where = false;
        if let Some(db) = self.db_merge_config(db) {
            has_where = true;
            sql.push_str(format!(" WHERE table_catalog='{}'", db).as_str());
        }
        if let Some(schema) = self.schema_merge_config(schema) {
            if has_where {
                sql.push_str(" AND ");
            } else {
                sql.push_str(" WHERE ");
            }
            has_where = true;
            sql.push_str(format!("table_schema='{}'", schema).as_str());
        }
        if has_where {
            sql.push_str(" AND ");
        } else {
            sql.push_str(" WHERE ");
        }
        sql.push_str(format!(" table_name='{}'", table_name).as_str());
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }

    async fn list_prepare_name(&self, query_id: uuid::Uuid) -> anyhow::Result<Vec<String>> {
        let sql = r#"
        SELECT name
            FROM pg_prepared_statements;
        "#;
        log::info!("sql:{}", sql);
        let conn = match self.query_connection.get(&query_id).map(|x| x.clone()) {
            Some(x) => x,
            None => anyhow::bail!(format!("can't find query of {}", query_id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let rows = sqlx::query(AssertSqlSafe(sql))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }

    async fn list_table_trigger(
        &self, _db: Option<&str>, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let sql = if let Some(schema) = self.schema_merge_config(schema) {
            format!(
                r#"
                SELECT tgname AS trigger_name
                    FROM pg_trigger
                    JOIN pg_class ON pg_trigger.tgrelid = pg_class.oid
                    JOIN pg_namespace ON pg_class.relnamespace = pg_namespace.oid
                    WHERE pg_class.relname = '{}'
                    AND pg_namespace.nspname = '{}'
            "#,
                table, schema,
            )
        } else {
            format!(
                r#"
                SELECT tgname AS trigger_name
                    FROM pg_trigger
                    JOIN pg_class ON pg_trigger.tgrelid = pg_class.oid
                    JOIN pg_namespace ON pg_class.relnamespace = pg_namespace.oid
                    WHERE pg_class.relname = '{}'
            "#,
                table,
            )
        };
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]

impl LangServerCommon for PostGresDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        CatalogTrait::list_database(self).await
    }

    async fn list_schema(&self, db: Option<&str>) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let db = self.db_merge_config(db);
        if db.is_none() {
            log::error!("list_schema using a empty db:{:?}", db);
        }
        let sql = format!(
            r#"
        SELECT schema_name 
            FROM information_schema.schemata
            WHERE schemata.catalog_name ='{}'
    "#,
            db.unwrap_or_default(),
        );
        log::info!("sql:{}", sql);
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl LanguageServer for PostGresDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            &sql,
            line,
            col,
            sqlparser::dialect::PostgreSqlDialect {},
            Arc::new(Clone::clone(self)),
        )
        .await;
        match r {
            Ok(xs) => return Ok(xs),
            Err(err) => {
                log::error!("completion on ast failed,err:{}", err);
            }
        };
        if !use_lexer {
            log::warn!("disabled use lexer , we will return empty data.");
            return anyhow::Ok(vec![]);
        }
        return LexerCompletionTrait::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::PostgreSqlDialect {},
        )
        .await;
    }

    fn format_sql(&self, sql: &str, ops: Option<FormatSqlOpts>) -> anyhow::Result<String> {
        anyhow::Ok(sqlformat_format_sql(sql, ops.map(|x| x.into())))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::db::{QuerySql, monaco_editor_keyword_regexp};
    use crate::enable_test_log;
    use crate::language_server::LexerCompletionTrait;
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::pg::{PostGresDB, make_test_object};

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_auto_completion() {
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
        -- 我们创建一下的表来做测试的

        CREATE TABLE departments (
            id SERIAL PRIMARY KEY,
            name VARCHAR(100) NOT NULL
        );
        CREATE TABLE employees (
            id SERIAL PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            department_id INTEGER REFERENCES departments(id)
        );
        CREATE OR REPLACE RULE do_nothing_rule AS
            ON INSERT TO employees
            DO INSTEAD
            NOTHING;
        CREATE OR REPLACE FUNCTION do_nothing()
        RETURNS TRIGGER AS $$
        BEGIN
            RETURN NULL;  -- 不执行任何操作
        END;
        $$ LANGUAGE plpgsql;

        CREATE TRIGGER do_nothing_trigger
        BEFORE INSERT OR UPDATE OR DELETE ON employees
        FOR EACH ROW
        EXECUTE FUNCTION do_nothing();
        create  view employees_test as select * from employees e;

        ALTER TABLE employees ENABLE ROW LEVEL SECURITY;

        CREATE POLICY no_op_policy
        ON employees
        USING (true)  -- 任何行都可以访问
        WITH CHECK (true);  -- 任何行都可以插入/更新
        "#,
            )
            .await;

        enable_test_log();
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("pg_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();

        test_file_auto_completion(&content, &db, false).await;
    }

    #[tokio::test]
    async fn print_key_words() {
        let db = make_test_object().await;
        let keywords = <PostGresDB as LexerCompletionTrait>::get_key_words_fn(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|x| x.item.to_string())
            .collect::<Vec<_>>();
        let keywords: Vec<&str> = keywords.iter().map(|x| x.as_str()).collect();
        let s = monaco_editor_keyword_regexp(&keywords[..]);
        println!("{}", s);
    }

    #[ignore = "手动跑跑的测试"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = make_test_object().await;
        test_file_auto_completion(
            r#"select word from pg_get_keywords() where word 
46 word"#,
            &db,
            false,
        )
        .await;
    }
}

pub(crate) const TYPES: &[&str] = &[
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
    "SERIAL",
    "BIGSERIAL",
    "SMALLSERIAL",
];
