use crate::db::{AutoCompletionQueryUuid, CatalogTrait};
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, DBExecutable, ErrAndLoc, LangServerCommon,
    LanguageServer, LexerCompletionTrait, PostGresExtension, try_parser_dot_lexer,
};

use crate::pg::PostGresDB;
use crate::pg_lang_server_static::POSTGRES_TYPES;
use crate::sqlparser_completion::TempTableColumn;
use sqlx::{AssertSqlSafe, Column, Executor, Row, SqlSafeStr, Statement};
use std::ops::DerefMut;
#[async_trait::async_trait]
impl LexerCompletionTrait for PostGresDB {
    const TYPES: &'static [&'static str] = POSTGRES_TYPES;
}

impl PostGresDB {
    fn schema_merge_config(&self, schema: Option<&str>) -> Option<String> {
        if schema.is_some() {
            return schema.map(|x| x.to_string());
        }
        self.config.schema.as_ref().map(|x| x.to_string())
    }
    const SCHEMA_CATALOG: &str = "pg_catalog";
}

fn is_pg_catalog(schema: Option<&String>) -> bool {
    schema
        .map(|x| x == PostGresDB::SCHEMA_CATALOG)
        .unwrap_or(false)
}

impl PostGresDB {
    async fn list_build_functions(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut lock = self.build_in_fn.lock().await;
        if !lock.is_empty() {
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
        let rows = sqlx::raw_sql(AssertSqlSafe(sql))
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
    async fn postgres_list_extension(
        &self, installed: bool,
    ) -> anyhow::Result<Vec<PostGresExtension>> {
        let mut conn = self.get_conn().await?;
        if !installed {
            let sql = "SELECT name,comment  FROM pg_available_extensions";
            let rows = sqlx::query(sql).fetch_all(conn.deref_mut()).await?;
            let mut ret = Vec::with_capacity(rows.len());
            for row in rows.iter() {
                let name: String = row.try_get(0)?;
                let comment: Option<String> = row.try_get(1)?;
                ret.push(PostGresExtension { name, comment });
            }
            return anyhow::Ok(ret);
        }
        let sql = "SELECT extname  FROM pg_extension";
        let rows = sqlx::query(sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            ret.push(PostGresExtension {
                name,
                comment: None,
            });
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
                name: x.name().to_string(),
                data_type: Some(x.type_info().to_string()),
            })
            .collect::<Vec<_>>();
        anyhow::Ok(ret)
    }

    async fn list_user(&self) -> anyhow::Result<Vec<String>> {
        let sql = "SELECT usename  from pg_user";
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for row in rows {
            let name: Option<String> = row.try_get(0)?;
            if let Some(name) = name {
                ret.push(name);
            }
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
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
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
        let rows = sqlx::raw_sql(AssertSqlSafe(sql))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn table_policy(&self, schema: Option<&str>, table: &str) -> anyhow::Result<Vec<String>> {
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
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
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
        let rows = sqlx::raw_sql(AssertSqlSafe(sql))
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
        &self, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let sql = if let Some(schema) = self.schema_merge_config(schema) {
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
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
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
        &self, schema: Option<&str>, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        let mut sql =
            "SELECT constraint_name FROM information_schema.table_constraints".to_string();
        let db = self.config.db_str();
        sql.push_str(format!(" WHERE table_catalog='{}'", db).as_str());
        let schema = self.schema_merge_config(schema);
        if schema.is_none() {
            return anyhow::Ok(vec![]);
        }
        let schema = schema.unwrap();
        sql.push_str(" AND ");
        sql.push_str(format!("table_schema='{}'", schema).as_str());
        sql.push_str(" AND ");
        sql.push_str(format!(" table_name='{}'", table_name).as_str());
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
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
        &self, schema: Option<&str>, table: &str,
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
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
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

    async fn list_column(
        &self, schema: Option<&str>, name: &str, is_call: bool,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = format!(
            r#"select * from {}"{}"{}"#,
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
    async fn get_key_words_fn(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut lock = self.key_word_completion.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql("select word from pg_get_keywords()")
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = vec![];
        for row in rows.iter() {
            let c: String = row.try_get(0)?;
            ret.push(CompletionItem {
                item: c.to_uppercase().into(),
                kind: CompletionKind::KeyWord,
                desc: None,
            });
        }
        *lock = ret;
        return anyhow::Ok(lock.clone());
    }
    async fn list_procedure(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
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
            .to_string()
        };
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_function(
        &self, schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let schema = self.schema_merge_config(schema);
        let mut ret = match schema {
            Some(schema) => {
                let sql = format!(
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

                log::info!("sql:{}", sql);
                let mut conn = self.get_conn().await?;
                let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
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

                ret
            }
            None => Default::default(),
        };

        if include_build_in {
            ret.extend(self.list_build_functions().await?.into_iter());
        }
        anyhow::Ok(ret)
    }
    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let mut sql = "SELECT viewname FROM pg_views ".to_string();
        if let Some(schema) = self.schema_merge_config(schema) {
            sql.push_str(format!(" WHERE schemaname='{}'", schema).as_str());
        }
        // postgres has materilized view in other table
        sql.push_str(
            "UNION SELECT matviewname from pg_matviews"
                .to_string()
                .as_str(),
        );
        let schema = self.schema_merge_config(schema);
        if let Some(schema) = &schema {
            sql.push_str(format!(" WHERE schemaname='{}'", schema).as_str());
        }
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }

        if !is_pg_catalog(schema.as_ref()) {
            ret.extend(
                LangServerCommon::list_view(self, Some(PostGresDB::SCHEMA_CATALOG))
                    .await?
                    .into_iter(),
            );
        }

        anyhow::Ok(ret)
    }

    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let db = self.config.db_str();
        let mut sql =
            r#"select table_name FROM information_schema.tables  WHERE table_type = 'BASE TABLE' "#
                .to_string();
        sql.push_str(format!(" AND table_catalog = '{}'", db).as_str());
        let schema = self.schema_merge_config(schema);
        if let Some(schema) = &schema {
            sql.push_str(format!(" AND table_schema = '{}'", schema).as_str());
        }
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }

        if !is_pg_catalog(schema.as_ref()) {
            ret.extend(
                LangServerCommon::list_table(self, Some(PostGresDB::SCHEMA_CATALOG))
                    .await?
                    .into_iter(),
            );
        }
        anyhow::Ok(ret)
    }
    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::PostgreSqlDialect {})
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let db = self.config.db_str();
        let sql = format!(
            r#"
        SELECT schema_name 
            FROM information_schema.schemata
            WHERE schemata.catalog_name ='{}'
    "#,
            db,
        );
        log::info!("sql:{}", sql);
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }

    async fn list_execute(&self, _: Option<&str>) -> anyhow::Result<Vec<DBExecutable>> {
        let conn = self.query_connection.get(&AutoCompletionQueryUuid::get());
        if conn.is_none() {
            anyhow::bail!("query_id should get connection")
        }
        let conn = conn.unwrap();
        let mut conn = conn.lock().await;

        let sql = r#"
            SELECT name,statement FROM pg_prepared_statements;
        "#;
        let rows = sqlx::raw_sql(sql)
            .fetch_all(conn.deref_mut().deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            let statement: String = row.try_get(1)?;
            ret.push(DBExecutable {
                name,
                statement: Some(statement),
            });
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl LanguageServer for PostGresDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        try_parser_dot_lexer(self, sql, line, col, use_lexer).await
    }

    fn parse_error(&self, sql: &str) -> anyhow::Result<Option<ErrAndLoc>> {
        LangServerCommon::parse_error(self, sql)
    }
}

#[cfg(test)]
mod tests {
    use crate::db::{AutoCompletionQueryUuid, QuerySql, monaco_editor_keyword_regexp};
    use crate::enable_test_log;
    use crate::language_server::LangServerCommon;
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::pg::{PostGresDB, make_test_object};
    use std::path::Path;

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
        let keywords = <PostGresDB as LangServerCommon>::get_key_words_fn(&db)
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
            r#"DROP EXTENSION plpgsql;
23 plpgsql"#,
            &db,
            false,
        )
        .await;
    }

    #[tokio::test]
    async fn test_prepare() {
        let _guard = AutoCompletionQueryUuid::test_mutex().lock().unwrap();
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        AutoCompletionQueryUuid::set(query_id.clone());
        db.execute_sql(
            query_id,
            r#"
            PREPARE select_one AS SELECT 1;
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        assert!(
            db.list_execute(None)
                .await
                .unwrap()
                .iter()
                .any(|x| x.name.eq("select_one"))
        );
    }
}
