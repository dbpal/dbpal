use sqlx::{AssertSqlSafe, Column, Executor, Row, SqlSafeStr, Statement};
use std::ops::DerefMut;

use crate::db::{AutoCompletionQueryUuid, name_start_ascii_alphabetic};
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, ErrAndLoc, LangServerCommon,
    LanguageServer, try_parser_dot_lexer,
};
use crate::sqlite::SqliteDB;
use crate::sqlite_lang_server_static::{SQLITE_KEY_WORDS, SQLITE_TYPES};
use crate::sqlparser_completion::TempTableColumn;

#[async_trait::async_trait]
impl LangServerCommon for SqliteDB {
    async fn list_column(
        &self, schema: Option<&str>, name: &str, is_call: bool,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let schema = Self::mrege_schema(schema);
        let sql = format!(
            r#"select * from "{schema}"."{name}{}" limit 1"#,
            if is_call { "()" } else { "" }
        );
        log::info!("sql:{}", sql);
        self.get_query_columns(&sql).await
    }
    // some backend can use a query to query this.
    async fn key_words(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut lock = self.keywords_completion.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        let ret: Vec<_> = SQLITE_KEY_WORDS
            .iter()
            .map(|x| CompletionItem {
                item: x.to_string().into(),
                kind: CompletionKind::KeyWord,
                desc: None,
            })
            .collect();
        *lock = ret.clone();
        anyhow::Ok(ret)
    }
    async fn list_function(
        &self, _schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        // sqlite function are all build in functions
        if !include_build_in {
            return anyhow::Ok(vec![]);
        }

        let mut lock = self.build_in_fn_completion.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        let sql = r#"
            SELECT name 
            FROM pragma_function_list()
            ORDER BY name;
        "#;
        let mut conn = self.get_conn().await?;

        let rows = sqlx::raw_sql(sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = vec![];

        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            if !name_start_ascii_alphabetic(&name) {
                // I have seen -> comming out
                continue;
            }
            ret.push(CompletionItem {
                item: name.into(),
                kind: CompletionKind::Function,
                desc: None,
            });
        }
        *lock = ret.clone();
        return anyhow::Ok(lock.clone());
    }

    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = Self::mrege_schema(schema);
        self.list_view_impl(schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = Self::mrege_schema(schema);
        self.list_table_impl(schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::SQLiteDialect {})
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let sql = "PRAGMA database_list";
        log::info!("sql:{}", sql);

        let rows = match self.query_connection.get(&AutoCompletionQueryUuid::get()) {
            Some(conn) => {
                let mut conn = conn.lock().await;
                let conn = conn.deref_mut();
                sqlx::raw_sql(AssertSqlSafe(sql))
                    .fetch_all(conn.deref_mut())
                    .await?
            }
            None => {
                let mut conn = self.get_conn().await?;
                sqlx::raw_sql(AssertSqlSafe(sql))
                    .fetch_all(conn.deref_mut())
                    .await?
            }
        };
        let mut ret = vec![];
        for row in rows.iter() {
            let name: String = row.try_get(1)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl crate::language_server::LexerCompletionTrait for SqliteDB {
    const TEXTS: &'static [&'static str] = &["true", "false"];
    const TYPES: &'static [&'static str] = SQLITE_TYPES;

    async fn extra_completion_item(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut x = AstCompletionHelper::sqlite_pragma(self).await?;
        let y = AstCompletionHelper::sqlite_module_name(self).await?;
        x.extend(y.into_iter().map(|x| CompletionItem {
            item: x.into(),
            kind: CompletionKind::Module,
            desc: None,
        }));
        anyhow::Ok(x)
    }
}

#[async_trait::async_trait]
impl LanguageServer for SqliteDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        try_parser_dot_lexer(self, sql, line, col, use_lexer).await
    }

    fn parse_error(&self, sql: &str) -> anyhow::Result<Option<ErrAndLoc>> {
        LangServerCommon::parse_error(self, sql)
    }
}

impl SqliteDB {
    pub(crate) async fn list_index_name_impl(&self, db: &str) -> anyhow::Result<Vec<String>> {
        let sql = format!(
            r#"
        SELECT name
            FROM "{db}".sqlite_master
            WHERE type = 'index';"#
        );
        log::info!("sql:{}", sql);

        let rows = match self.query_connection.get(&AutoCompletionQueryUuid::get()) {
            Some(conn) => {
                let mut conn = conn.lock().await;
                sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                    .fetch_all(conn.deref_mut().deref_mut())
                    .await?
            }
            None => {
                let mut conn = self.get_conn().await?;
                sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                    .fetch_all(conn.deref_mut())
                    .await?
            }
        };

        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name = r.get(0);
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

impl SqliteDB {
    pub(crate) fn mrege_schema(schema: Option<&str>) -> &str {
        match schema {
            Some(s) => s,
            None => SqliteDB::MAIN_DB,
        }
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for SqliteDB {
    // issue https://github.com/launchbadge/sqlx/issues/4010
    async fn sqlite_module_name(&self) -> anyhow::Result<Vec<String>> {
        let sql = "SELECT name  FROM pragma_module_list";
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = vec![];
        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        let s = match self.query_connection.get(&AutoCompletionQueryUuid::get()) {
            Some(conn) => {
                let mut conn = conn.lock().await;
                let conn = conn.deref_mut();
                conn.deref_mut()
                    .prepare_with(AssertSqlSafe(sql).into_sql_str(), &[])
                    .await?
            }
            None => {
                let mut conn = self.get_conn().await?;
                conn.deref_mut()
                    .prepare_with(AssertSqlSafe(sql).into_sql_str(), &[])
                    .await?
            }
        };

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

    async fn list_index(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = Self::mrege_schema(_schema);
        self.list_index_name_impl(schema).await
    }

    async fn sqlite_pragma(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut lock = self.pragma_completion.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        let sql = "PRAGMA pragma_list";
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            ret.push(CompletionItem {
                item: name.into(),
                kind: CompletionKind::Pragma,
                desc: None,
            });
        }
        *lock = ret.clone();
        return anyhow::Ok(lock.clone());
    }
}

#[cfg(test)]
mod tests {
    use crate::db::QuerySql;
    use crate::enable_test_log;
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::sqlite::make_test_object;
    use std::path::Path;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_auto_completion() {
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        let _ = db
            .query_sql(
                query_id,
                r#"
            -- 测试用到的表格 和建表语句
            -- 创建 departments 表
            CREATE TABLE departments (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            );

            -- 创建 employees 表
            CREATE TABLE employees (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                department_id INTEGER,
                FOREIGN KEY (department_id) REFERENCES departments (id)
            );

            -- create view for test
            create view employees_view as select * from employees;
        "#,
            )
            .await;

        let _ = db
            .query_sql(
                query_id,
                r#"
                create index employees_name on employees(name)
        "#,
            )
            .await;

        enable_test_log();
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("sqlite_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();

        test_file_auto_completion(&content, &db, false).await;
    }

    #[ignore = "手动跑跑的测试"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = make_test_object().await;
        test_file_auto_completion(
            r#"select employees.id  from employees
36 departments"#,
            &db,
            false,
        )
        .await;
    }
}
