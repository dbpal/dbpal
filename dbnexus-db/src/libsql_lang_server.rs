use crate::db::name_start_ascii_alphabetic;
use crate::language_server::{
    try_parser_dot_lexer, AstCompletionHelper, CompletionItem, CompletionKind, CompletionList,
    ErrAndLoc, GetKeyWords, LangServerCommon, LanguageServer, LexerCompletionTrait,
};
use crate::libsql::LibSqlDB;
use crate::sqlite::SqliteDB;
use crate::sqlite_lang_server::{SQLITE_KEY_WORDS, SQLITE_TYPES};
use crate::sqlparser_completion::TempTableColumn;
use libsql::params;
use tokio::sync::Mutex;

#[async_trait::async_trait]
impl GetKeyWords for LibSqlDB {
    // some backend can use a query to query this.
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        static KEY_WORDS_CACHE: Mutex<Vec<String>> = Mutex::const_new(Vec::new());
        let mut lock = KEY_WORDS_CACHE.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        lock.extend(
            SQLITE_KEY_WORDS
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<_>>(),
        );
        return anyhow::Ok(lock.clone());
    }
}

#[async_trait::async_trait]
impl LanguageServer for LibSqlDB {
    async fn try_get_max_id(&self, name: &str) -> anyhow::Result<String> {
        let sql = format!("SELECT MAX(id) FROM \"{name}\"");
        let conn = self.get_conn().await?;
        let mut row = conn.query(&sql, params!()).await?;
        while let Some(item) = row.next().await? {
            let value = item.get_value(0)?;
            return anyhow::Ok(match value {
                libsql::Value::Null => "null".to_string(),
                libsql::Value::Integer(x) => x.to_string(),
                libsql::Value::Real(x) => x.to_string(),
                libsql::Value::Text(x) => x.to_string(),
                libsql::Value::Blob(..) => {
                    continue;
                }
            });
        }

        anyhow::bail!("no max id found")
    }

    async fn ai_text_completion(
        &self,
        text: &str,
        line: u32,
        col: u32,
    ) -> anyhow::Result<CompletionList> {
        LexerCompletionTrait::ai_text_completion(self, text, line, col).await
    }

    fn extract_table_select_selection(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_selection(self, sql)
    }
    fn extract_table_select_sort(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_sort(self, sql)
    }

    async fn auto_completion(
        &self,
        query_id: Option<String>,
        sql: &str,
        line: u32,
        col: u32,
        use_lexer: bool,
    ) -> anyhow::Result<CompletionList> {
        self.completion_id.set_optional(query_id);
        try_parser_dot_lexer(self, sql, line, col, use_lexer).await
    }

    async fn check_sql(&self, sql: &str) -> anyhow::Result<Option<ErrAndLoc>> {
        LangServerCommon::parse_err(self, sql)
    }
    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        LangServerCommon::is_dangerous_sql_by_parser(self, sql)
    }
}

#[async_trait::async_trait]

impl LexerCompletionTrait for LibSqlDB {
    async fn get_types(&self) -> anyhow::Result<Vec<String>> {
        let r = SQLITE_TYPES
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>();
        anyhow::Ok(r)
    }

    async fn get_text(&self) -> anyhow::Result<Vec<String>> {
        let r = ["true", "false"]
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<_>>();
        anyhow::Ok(r)
    }
}

#[async_trait::async_trait]
impl LangServerCommon for LibSqlDB {
    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = Self::merge_schema(schema);
        self.list_index_name_impl(schema).await
    }
    async fn sqlite_module_name(&self) -> anyhow::Result<Vec<String>> {
        static CACHE: Mutex<Vec<String>> = Mutex::const_new(Vec::new());

        let mut lock = CACHE.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        let sql = "SELECT name  FROM pragma_module_list";
        let conn = self.get_conn().await?;
        let mut rows = conn.query(sql, params!()).await?;
        let mut ret = vec![];
        while let Some(row) = rows.next().await? {
            let name: String = row.get(0)?;
            ret.push(name);
        }
        *lock = ret.clone();
        anyhow::Ok(ret)
    }

    async fn sqlite_pragma(&self) -> anyhow::Result<Vec<CompletionItem>> {
        static PRAGMA_COMPLETION: Mutex<Vec<CompletionItem>> = Mutex::const_new(Vec::new());
        let mut lock = PRAGMA_COMPLETION.lock().await;

        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        let sql: &'static str = r#"
            PRAGMA pragma_list
        "#;
        let mut ret = vec![];
        let conn = self.get_conn().await?;
        let mut rows = conn.query(sql, params!()).await?;
        while let Some(row) = rows.next().await? {
            let name: String = row.get(0)?;
            ret.push(CompletionItem {
                item: name.into(),
                kind: CompletionKind::Pragma,
                desc: None,
            });
        }
        *lock = ret;
        return anyhow::Ok(lock.clone());
    }

    async fn list_column(
        &self,
        _schema: Option<&str>,
        name: &str,
        is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = format!(
            r#"select * from "{name}"{} limit 1"#,
            if let Some(args) = is_call {
                format!("({})", args.join(","))
            } else {
                "".to_string()
            }
        );
        self.get_query_columns(&sql).await
    }

    async fn list_function(
        &self,
        _schema: Option<&str>,
        include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        if !include_build_in {
            // libsql only have build in functions
            return anyhow::Ok(vec![]);
        }
        static BUILD_IN_FN_CACHE: Mutex<Vec<CompletionItem>> = Mutex::const_new(Vec::new());
        let mut lock = BUILD_IN_FN_CACHE.lock().await;
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
        let mut ret = Vec::new();

        while let Some(row) = rows.next().await? {
            let name: String = row.get(0)?;
            if !name_start_ascii_alphabetic(&name) {
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
    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::SQLiteDialect {})
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let sql = "PRAGMA database_list";
        let mut rows = match self.query_connection.get(&self.completion_id.get()) {
            Some(conn) => conn.query(sql, params!()).await?,
            None => {
                let conn = self.get_conn().await?;
                conn.query(sql, params!()).await?
            }
        };
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
    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        let conn = self.get_conn().await?;
        log::info!("sql:{}", sql);
        let mut stmt = conn.prepare(sql).await?;
        let ret = stmt
            .columns()
            .iter()
            .map(|x| TempTableColumn {
                name: x.name().to_string(),
                data_type: Some(x.decl_type().unwrap_or_default().to_string()),
            })
            .collect::<Vec<_>>();
        // check if prepare return the result.
        if !ret.is_empty() {
            return anyhow::Ok(ret);
        }

        let row = stmt.query_row(params!()).await?;
        anyhow::Ok(
            (0..row.column_count())
                .map(|index| TempTableColumn {
                    name: row.column_name(index).unwrap_or_default().to_owned(),
                    data_type: row.column_type(index).ok().map(|x| format!("{:?}", x)),
                })
                .collect(),
        )
    }
}

impl LibSqlDB {
    pub(crate) async fn list_index_name_impl(&self, db: &str) -> anyhow::Result<Vec<String>> {
        let sql = format!(
            r#"
        SELECT name
            FROM "{db}".sqlite_master
            WHERE type = 'index'"#
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
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::language_server::LanguageServer;
    use crate::libsql::make_test_object;
    use std::ops::Deref;
    use std::path::Path;

    #[tokio::test]
    async fn test_auto_completion() {
        let db = make_test_object().await.lock().await;
        let _ = db.drop_table("departments", false).await;
        let _ = db.drop_table("employees", false).await;
        let _ = db.drop_view("employees_view").await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            -- 测试用到的表格 和建表语句
            -- 创建 departments 表
            CREATE TABLE departments (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
            -- 创建 employees 表
            CREATE TABLE employees (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                department_id INTEGER
            );

        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
             create view employees_view as select * from employees;

        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
                create index employees_name on employees(name)
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(query_id, "replace into departments values (1,'test')")
            .await
            .unwrap()
            .unwrap();

        db.execute_sql(
            query_id,
            r#"
            insert into departments values (2 , 'DOD');
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("sqlite_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();

        test_file_auto_completion(&content, db.deref(), None, false).await;
    }

    #[ignore = "手动跑跑的测试"]
    #[tokio::test]
    async fn test_completion_xxx() {
        // enable_test_log();
        let db = make_test_object().await.lock().await;
        test_file_auto_completion(
            r#"select employees.id  from employees
13 employees ; 19 name ; 32 departments ; 32 main"#,
            db.deref(),
            None,
            false,
        )
        .await;
    }
    #[ignore = "run manually"]
    #[tokio::test]

    async fn test_try_get_max_id() {
        let db = make_test_object().await.lock().await;
        let max_id = db.try_get_max_id("departments").await.unwrap();
        println!("max id is:{}", max_id);
    }
}
