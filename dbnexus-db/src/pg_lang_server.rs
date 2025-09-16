use crate::db::{AutoCompletionQueryUuid, CatalogTrait, CreatedTypeDef, SequenceDetail};
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, DBExecutable, ErrAndLoc, LangServerCommon,
    LanguageServer, LexerCompletionTrait, PostGresExtension, try_parser_dot_lexer,
};

use crate::pg::PostGresDB;
use crate::sqlparser_completion::TempTableColumn;
use sqlx::{AssertSqlSafe, Column, Executor, Row, SqlSafeStr, Statement};
use std::collections::HashSet;
use std::ops::DerefMut;
#[async_trait::async_trait]
impl LexerCompletionTrait for PostGresDB {
    const TYPES: &'static [&'static str] = POSTGRES_TYPES;
}

impl PostGresDB {
    pub(crate) async fn get_search_path(
        &self, schema: Option<&str>, include_catalog: bool,
    ) -> HashSet<String> {
        if schema.is_some() {
            let mut m = HashSet::new();
            m.insert(schema.unwrap().to_string());
            return m;
        }
        let ret = match self.query_connection.get(&AutoCompletionQueryUuid::get()) {
            Some(conn) => {
                let mut conn = conn.lock().await;
                let conn = conn.deref_mut();
                let row = sqlx::raw_sql("SHOW search_path;")
                    .fetch_one(conn.deref_mut())
                    .await
                    .map_err(|e| log::error!("query search path failed,err:{}", e))
                    .ok();
                match row {
                    Some(x) => {
                        // information_schema, public like this
                        let s: Option<String> = x.try_get(0).ok().flatten();
                        let ret = s.map(|string| {
                            string
                                .split(",")
                                .map(|x| x.trim().to_string())
                                .collect::<HashSet<_>>()
                        });
                        ret
                    }
                    None => None,
                }
            }
            None => None,
        };
        if ret.is_some() {
            let mut ret = ret.unwrap();
            if include_catalog {
                ret.insert(Self::SCHEMA_CATALOG.to_string());
            }
            return ret;
        }
        let mut ret = HashSet::new();
        if let Some(s) = self.config.schema.as_ref() {
            ret.insert(s.to_string());
        }
        if include_catalog {
            ret.insert(Self::SCHEMA_CATALOG.to_string());
        }
        ret
    }
    const SCHEMA_CATALOG: &str = "pg_catalog";
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

    async fn table_policy(&self, schema: Option<&str>, table: &str) -> anyhow::Result<Vec<String>> {
        let schemas = self.get_search_path(schema, false).await;
        let mut ret = Vec::new();
        let mut conn = self.get_conn().await?;
        for schema in schemas.iter() {
            let sql = format!(
                r#"SELECT policyname
                FROM pg_policies
                WHERE tablename = '{}' AND schemaname='{}';  -- 替换为你的表名"#,
                table, schema
            );
            log::info!("sql:{}", sql);
            let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .fetch_all(conn.deref_mut())
                .await?;

            for r in rows.iter() {
                let name: String = r.try_get(0)?;
                ret.push(name)
            }
        }

        anyhow::Ok(ret)
    }

    async fn list_postgres_rule(
        &self, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let schemas = self.get_search_path(schema, false).await;
        let ret = vec![];
        let mut conn = self.get_conn().await?;
        for schema in schemas.iter() {
            let sql = format!(
                r#"select rulename from pg_rules where tablename='{}' and schemaname='{}'"#,
                table, schema
            );
            let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .fetch_all(conn.deref_mut())
                .await?;
            let mut ret = Vec::with_capacity(rows.len());
            for r in rows.iter() {
                let name: String = r.try_get(0)?;
                ret.push(name)
            }
        }
        anyhow::Ok(ret)
    }

    async fn list_constraint(
        &self, schema: Option<&str>, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        let schemas = self.get_search_path(schema, false).await;
        let mut conn = self.get_conn().await?;
        let mut ret = Vec::new();
        for schema in schemas.iter() {
            let mut sql =
                "SELECT constraint_name FROM information_schema.table_constraints".to_string();
            let db = self.config.db_str();
            sql.push_str(format!(" WHERE table_catalog='{}'", db).as_str());
            sql.push_str(" AND ");
            sql.push_str(format!("table_schema='{}'", schema).as_str());
            sql.push_str(" AND ");
            sql.push_str(format!(" table_name='{}'", table_name).as_str());
            let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .fetch_all(conn.deref_mut())
                .await?;

            for r in rows.iter() {
                let name: String = r.try_get(0)?;
                ret.push(name)
            }
        }
        anyhow::Ok(ret)
    }
    async fn list_table_trigger(
        &self, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let schemas = self.get_search_path(schema, false).await;
        let mut ret = Vec::new();
        for schema in schemas.iter() {
            let sql = format!(
                r#"
                SELECT tgname AS trigger_name
                    FROM pg_trigger
                    JOIN pg_class ON pg_trigger.tgrelid = pg_class.oid
                    JOIN pg_namespace ON pg_class.relnamespace = pg_namespace.oid
                    WHERE pg_class.relname = '{}'
                    AND pg_namespace.nspname = '{}'
            "#,
                table, schema,
            );
            log::info!("sql:{}", sql);
            let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .fetch_all(conn.deref_mut())
                .await?;
            for r in rows.iter() {
                let name: String = r.try_get(0)?;
                ret.push(name)
            }
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]

impl LangServerCommon for PostGresDB {
    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schemas = self.get_search_path(schema, true).await;
        let mut conn = self.get_conn().await?;
        let mut ret = Vec::new();
        for schema in schemas.iter() {
            let sql = format!(
                r#"
            SELECT indexname AS index_name
                FROM pg_indexes
                WHERE schemaname = '{}';
                "#,
                schema
            );
            let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .fetch_all(conn.deref_mut())
                .await?;
            for r in rows.iter() {
                let name: String = r.try_get(0)?;
                ret.push(name);
            }
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

    async fn list_domain(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schemas = self.get_search_path(schema, true).await;
        let mut conn = self.get_conn().await?;
        let mut ret = Vec::new();
        for schema in schemas.iter() {
            let sql = format!(
                r#"
                SELECT domain_name
                FROM information_schema.domains
                WHERE domain_schema = '{}'   -- 换成实际 schema 名
                ORDER BY domain_name;
            "#,
                schema,
            );
            let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
                .fetch_all(conn.deref_mut())
                .await?;
            for row in rows {
                let name: Option<String> = row.try_get(0)?;
                if let Some(name) = name {
                    ret.push(name);
                }
            }
        }

        anyhow::Ok(ret)
    }

    async fn list_sequence(&self, schema: Option<&str>) -> anyhow::Result<Vec<SequenceDetail>> {
        let schema = self.get_search_path(schema, true).await;
        let mut ret = Vec::new();
        for schema in schema.iter() {
            ret.extend(
                self.list_sequences_impl(schema)
                    .await
                    .unwrap_or_default()
                    .into_iter(),
            );
        }
        anyhow::Ok(ret)
    }

    async fn list_created_type(&self, schema: Option<&str>) -> anyhow::Result<Vec<CreatedTypeDef>> {
        let schema = self.get_search_path(schema, true).await;
        let mut ret = Vec::new();
        for schema in schema.iter() {
            let x = self
                .list_created_types_impl(schema, false)
                .await
                .unwrap_or_default();
            ret.extend(x.into_iter());
        }
        anyhow::Ok(ret)
    }
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        CatalogTrait::list_database(self).await
    }

    async fn list_column(
        &self, schema: Option<&str>, name: &str, is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = format!(
            r#"select * from {}"{}"{}"#,
            match schema {
                Some(x) => format!("\"{}\".", x),
                None => "".to_string(),
            },
            name,
            if let Some(args) = is_call {
                format!("({})", args.join(","))
            } else {
                "".to_string()
            }
        );
        log::info!("sql:{}", sql);
        self.get_query_columns(&sql).await
    }
    async fn key_words(&self) -> anyhow::Result<Vec<CompletionItem>> {
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
        let schemas = self.get_search_path(schema, true).await;
        let mut conn = self.get_conn().await?;
        let mut ret = Vec::new();
        for schema in schemas.iter() {
            let sql = format!(
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
            );
            log::info!("sql:{}", sql);

            let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .fetch_all(conn.deref_mut())
                .await?;
            for r in rows.iter() {
                let name: String = r.try_get(0)?;
                ret.push(name);
            }
        }
        anyhow::Ok(ret)
    }

    async fn list_function(
        &self, schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let schemas = self.get_search_path(schema, include_build_in).await;
        let mut ret = Vec::new();
        for schema in schemas.iter() {
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

            for r in rows.iter() {
                let name: String = r.try_get(0)?;
                ret.push(CompletionItem {
                    item: name.into(),
                    desc: None,
                    kind: CompletionKind::Function,
                });
            }
        }
        anyhow::Ok(ret)
    }
    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schemas = self.get_search_path(schema, true).await;
        let mut conn = self.get_conn().await?;
        let mut ret = Vec::new();
        for schema in schemas.iter() {
            let sql = format!(
                r#"
                SELECT viewname 
                FROM pg_views 
                WHERE schemaname = '{schema}'

                UNION 

                SELECT matviewname 
                FROM pg_matviews 
                WHERE schemaname = '{schema}';
            "#,
            );
            let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .fetch_all(conn.deref_mut())
                .await?;
            for r in rows.iter() {
                let name: String = r.try_get(0)?;
                ret.push(name);
            }
        }

        anyhow::Ok(ret)
    }

    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let db = self.config.db_str();
        let schemas = self.get_search_path(schema, true).await;
        let mut conn = self.get_conn().await?;
        let mut ret = Vec::new();
        for schema in schemas.iter() {
            let sql = format!(
                r#"select table_name FROM information_schema.tables  WHERE table_type = 'BASE TABLE' AND table_schema = '{schema}'AND table_catalog = '{db}'  "#
            );
            let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .fetch_all(conn.deref_mut())
                .await?;
            for r in rows.iter() {
                let name: String = r.try_get(0)?;
                ret.push(name);
            }
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
    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        LangServerCommon::is_dangerous_sql_by_parser(self, sql)
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
        enable_test_log();
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();

        let sql = [
            r#"CREATE TABLE departments (
            id SERIAL PRIMARY KEY,
            name VARCHAR(100) NOT NULL
        );"#,
            r#"
            CREATE TABLE departments (
            id SERIAL PRIMARY KEY,
            name VARCHAR(100) NOT NULL
        );
        "#,
            r#"
        CREATE TABLE employees (
            id SERIAL PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            department_id INTEGER REFERENCES departments(id)
        );
            "#,
            r#"
            CREATE OR REPLACE RULE do_nothing_rule AS
            ON INSERT TO employees
            DO INSTEAD
            NOTHING;"#,
            r#"
            CREATE OR REPLACE FUNCTION do_nothing()
            RETURNS TRIGGER AS $$
            BEGIN
                RETURN NULL;  -- 不执行任何操作
            END;
            $$ LANGUAGE plpgsql;
            "#,
            r#"
        CREATE TRIGGER do_nothing_trigger
        BEFORE INSERT OR UPDATE OR DELETE ON employees
        FOR EACH ROW
        EXECUTE FUNCTION do_nothing();
            "#,
            r#"

        create  view employees_test as select * from employees e;
            "#,
            r#"
                ALTER TABLE employees ENABLE ROW LEVEL SECURITY;
            "#,
            r#"
        CREATE POLICY no_op_policy
        ON employees
        USING (true)  -- 任何行都可以访问
        WITH CHECK (true);  -- 任何行都可以插入/更新
            "#,
            r#"
        CREATE SEQUENCE do_nothing_seq;

            "#,
            r#"
                CREATE OR REPLACE PROCEDURE do_nothing_procedure()
                LANGUAGE plpgsql
                AS $$
                BEGIN
                    -- 这里什么都不做
                    RETURN;
                END;
                $$;
            "#,
        ];
        for sql in sql.iter() {
            let ret = db.execute_sql(query_id, sql).await;
            if let Some(err) = ret.err() {
                log::error!("execute sql error:should be fine,err{}", err);
            }
        }

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
        let keywords = <PostGresDB as LangServerCommon>::key_words(&db)
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
pub(crate) const POSTGRES_TYPES: &[&str] = &[
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
