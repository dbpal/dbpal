use crate::db::{AutoCompletionUuid, name_start_ascii_alphabetic};
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, LangServerCommon, LanguageServer,
    try_parser_and_lexer,
};
use crate::libsql::LibSqlDB;
use crate::sqlite::SqliteDB;
use crate::sqlite_lang_server_static::{SQLITE_KEY_WORDS, SQLITE_TYPES};
use crate::sqlparser_completion::TempTableColumn;
use libsql::params;

#[async_trait::async_trait]
impl LanguageServer for LibSqlDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        try_parser_and_lexer(self, sql, line, col, use_lexer).await
    }
}

#[async_trait::async_trait]

impl crate::language_server::LexerCompletionTrait for LibSqlDB {
    const TEXTS: &'static [&'static str] = &["true", "false"];
    const TYPES: &'static [&'static str] = SQLITE_TYPES;

    async fn list_columns(&self, table_name: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        AstCompletionHelper::list_column(self, None, table_name, false).await
    }

    async fn extra_completion_item(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::sqlite_pragma(self).await
    }

    // some backend can use a query to query this.
    async fn get_key_words_fn(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut lock = self.keywords_completion.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        lock.extend(
            SQLITE_KEY_WORDS
                .iter()
                .map(|x| CompletionItem {
                    item: x.to_string().into(),
                    kind: CompletionKind::KeyWord,
                    desc: None,
                })
                .collect::<Vec<_>>(),
        );
        return anyhow::Ok(lock.clone());
    }

    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        let mut ret: Vec<_> = self
            .list_table_impl(SqliteDB::MAIN_DB)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())?;
        ret.extend(
            self.list_view_impl(SqliteDB::MAIN_DB)
                .await
                .map(|x| x.into_iter().map(|x| x.name).collect::<Vec<_>>())?
                .into_iter(),
        );
        anyhow::Ok(ret)
    }

    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self, None, true).await
    }
}

#[async_trait::async_trait]
impl LangServerCommon for LibSqlDB {
    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::SQLiteDialect {})
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let conn = self.query_connections.get(&AutoCompletionUuid::get());
        if conn.is_none() {
            anyhow::bail!("query id should have connection")
        }
        let conn = conn.unwrap();
        let sql = "PRAGMA database_list";
        let mut rows = conn.query(sql, params!()).await?;
        let mut ret = vec![];
        while let Some(row) = rows.next().await? {
            let name: String = row.get(1)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

impl LibSqlDB {
    fn merge_schema(db: Option<&str>) -> &str {
        db.unwrap_or(SqliteDB::MAIN_DB)
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for LibSqlDB {
    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = Self::merge_schema(schema);
        self.list_view_impl(schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = Self::merge_schema(schema);
        self.list_table_impl(schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        let conn = self.get_conn().await?;
        log::info!("sql:{}", sql);
        let mut stmt = conn.prepare(sql).await?;
        let ret = stmt
            .columns()
            .iter()
            .map(|x| TempTableColumn {
                name: x.name().to_string(),
                typ: Some(x.decl_type().unwrap_or_default().to_string()),
            })
            .collect::<Vec<_>>();
        // check if prepare return the reuslt.
        if !ret.is_empty() {
            return anyhow::Ok(ret);
        }

        let row = stmt.query_row(params!()).await?;
        anyhow::Ok(
            (0..row.column_count())
                .map(|index| TempTableColumn {
                    name: row.column_name(index).unwrap_or_default().to_owned(),
                    typ: row.column_type(index).ok().map(|x| format!("{:?}", x)),
                })
                .collect(),
        )
    }

    async fn list_column(
        &self, _schema: Option<&str>, name: &str, is_call: bool,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = format!(
            r#"select * from "{name}"{} limit 1"#,
            if is_call { "()" } else { "" }
        );
        self.get_query_columns(&sql).await
    }

    async fn list_function(
        &self, _schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        if !include_build_in {
            // if we don't want build in function, we can return empty.
            return anyhow::Ok(vec![]);
        }
        let mut lock = self.build_fn_cache.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }

        let sql = r#"
            SELECT name 
            FROM pragma_function_list()
            ORDER BY name;
        "#;
        let conn = self.get_conn().await?;
        let mut rows = conn.query(sql, params!()).await?;
        while let Some(row) = rows.next().await? {
            let name: String = row.get(0)?;
            if !name_start_ascii_alphabetic(&name) {
                continue;
            }
            lock.push(CompletionItem {
                item: name.into(),
                kind: CompletionKind::Function,
                desc: None,
            });
        }
        return anyhow::Ok(lock.clone());
    }

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = Self::merge_schema(schema);
        self.list_index_name_impl(schema).await
    }

    async fn sqlite_pragma(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut lock = self.pragma_completion.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        let sql = r#"
           PRAGMA pragma_list
        "#;
        let conn = self.get_conn().await?;
        let mut rows = conn.query(sql, params!()).await?;
        while let Some(row) = rows.next().await? {
            let name: String = row.get(0)?;
            lock.push(CompletionItem {
                item: name.into(),
                kind: CompletionKind::Pragma,
                desc: None,
            });
        }
        return anyhow::Ok(lock.clone());
    }
}

impl LibSqlDB {
    pub(crate) async fn list_index_name_impl(&self, db: &str) -> anyhow::Result<Vec<String>> {
        let sql = format!(
            r#"
        SELECT name
            FROM "{db}".sqlite_master
            WHERE type = 'index';"#
        );
        log::info!("sql:{}", sql);
        let conn = self.get_conn().await?;
        let mut rows = conn.query(&sql, params!()).await?;
        let mut ret = Vec::new();
        while let Some(r) = rows.next().await? {
            let name: String = r.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use crate::db::{QuerySql, SchemaTrait};
    use crate::enable_test_log;
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::libsql::make_test_object;
    use std::path::Path;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_auto_completion() {
        enable_test_log();

        let db = make_test_object().await;
        let _ = db.drop_table("departments", false).await;
        let _ = db.drop_table("employees", false).await;
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
                department_id INTEGER
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

        let _ = db
            .query_sql(
                query_id,
                r#"
        
            insert into departments values (1 , 'DOD');
            insert into employees values(1 , 'yuyang' , 1) ;
        "#,
            )
            .await;

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
        // enable_test_log();
        let db = make_test_object().await;
        test_file_auto_completion(
            r#"select employees.id  from employees
13 employees ; 19 name ; 32 departments ; 32 main"#,
            &db,
            false,
        )
        .await;
    }
}
