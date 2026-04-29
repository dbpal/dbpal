use sqlx::{AssertSqlSafe, Column, Executor, Row, SqlSafeStr, Statement};

use std::ops::DerefMut;
use tokio::sync::Mutex;

use crate::db::name_start_ascii_alphabetic;
use crate::language_server::{
    try_parser_dot_lexer, AstCompletionHelper, CompletionItem, CompletionKind, CompletionList,
    GetKeyWords, LangServerCommon, LanguageServer, LexerCompletionTrait,
};
use crate::sqlite::SqliteDB;
use crate::sqlparser_completion::TempTableColumn;
#[async_trait::async_trait]
impl GetKeyWords for SqliteDB {
    // some backend can use a query to query this.
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        let ret: Vec<_> = SQLITE_KEY_WORDS.iter().map(|x| x.to_string()).collect();
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl LangServerCommon for SqliteDB {
    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = Self::merge_schema(schema);
        self.list_index_name_impl(schema).await
    }

    // issue https://github.com/launchbadge/sqlx/issues/4010
    async fn sqlite_module_name(&self) -> anyhow::Result<Vec<String>> {
        static MODULE_NAME: Mutex<Vec<String>> = Mutex::const_new(Vec::new());
        let mut lock = MODULE_NAME.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
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
        *lock = ret.clone();
        anyhow::Ok(ret)
    }
    async fn sqlite_pragma(&self) -> anyhow::Result<Vec<CompletionItem>> {
        static PRAGMA_COMPLETION: Mutex<Vec<CompletionItem>> = Mutex::const_new(Vec::new());
        let mut lock = PRAGMA_COMPLETION.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        let sql = "PRAGMA pragma_list";
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::new();
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

    async fn list_column(
        &self,
        schema: Option<&str>,
        name: &str,
        is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let schema = Self::merge_schema(schema);
        let sql = format!(
            r#"select * from "{schema}"."{name}{}" limit 1"#,
            if let Some(is_call) = is_call {
                format!("({})", is_call.join(","))
            } else {
                "".to_string()
            }
        );
        log::info!("sql:{}", sql);
        self.get_query_columns(&sql).await
    }

    async fn list_function(
        &self,
        _schema: Option<&str>,
        include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        if !include_build_in {
            return anyhow::Ok(vec![]);
        }
        // sqlite function are all build in functions
        static BUILD_IN_FN_COMPLETION: Mutex<Vec<CompletionItem>> = Mutex::const_new(Vec::new());
        let mut lock = BUILD_IN_FN_COMPLETION.lock().await;
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
        log::info!("sql:{}", sql);
        let rows = match self.query_connection.get(&self.completion_id.get()) {
            Some(conn) => {
                let mut conn = conn.lock().await;
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
    async fn get_types(&self) -> anyhow::Result<Vec<String>> {
        let r = SQLITE_TYPES.iter().map(|x| x.to_string()).collect();
        anyhow::Ok(r)
    }

    async fn get_text(&self) -> anyhow::Result<Vec<String>> {
        let r = ["true", "false"].iter().map(|x| x.to_string()).collect();
        anyhow::Ok(r)
    }
}

#[async_trait::async_trait]
impl LanguageServer for SqliteDB {
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

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        LangServerCommon::is_dangerous_sql_by_parser(self, sql)
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

        let rows = match self.query_connection.get(&self.completion_id.get()) {
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
    pub(crate) fn merge_schema(schema: Option<&str>) -> &str {
        match schema {
            Some(s) => s,
            None => SqliteDB::MAIN_DB,
        }
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for SqliteDB {
    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        let s = match self.query_connection.get(&self.completion_id.get()) {
            Some(conn) => {
                let mut conn = conn.lock().await;
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
}

pub(crate) const SQLITE_TYPES: &[&str] = &[
    "INTEGER",
    "REAL",
    "TEXT",
    "BLOB",
    "INT",
    "TINYINT",
    "SMALLINT",
    "MEDIUMINT",
    "BIGINT",
    "UNSIGNED BIG INT",
    "INT2",
    "INT8",
    "CHARACTER",
    "VARCHAR",
    "VARYING CHARACTER",
    "NCHAR",
    "NATIVE CHARACTER",
    "NVARCHAR",
    "CLOB",
    "DOUBLE",
    "DOUBLE PRECISION",
    "FLOAT",
    "NUMERIC",
    "DECIMAL",
    "BOOLEAN",
    "DATE",
    "DATETIME",
];

pub(crate) const SQLITE_KEY_WORDS: &[&str] = &[
    "ABORT",
    "ACTION",
    "ADD",
    "AFTER",
    "ALL",
    "ALTER",
    "ALWAYS",
    "ANALYZE",
    "AND",
    "AS",
    "ASC",
    "ATTACH",
    "AUTOINCREMENT",
    "BEFORE",
    "BEGIN",
    "BETWEEN",
    "BY",
    "CASCADE",
    "CASE",
    "CAST",
    "CHECK",
    "COLLATE",
    "COLUMN",
    "COMMIT",
    "CONFLICT",
    "CONSTRAINT",
    "CREATE",
    "CROSS",
    "CURRENT",
    "CURRENT_DATE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "DATABASE",
    "DEFAULT",
    "DEFERRABLE",
    "DEFERRED",
    "DELETE",
    "DESC",
    "DETACH",
    "DISTINCT",
    "DO",
    "DROP",
    "EACH",
    "ELSE",
    "END",
    "ESCAPE",
    "EXCEPT",
    "EXCLUDE",
    "EXCLUSIVE",
    "EXISTS",
    "EXPLAIN",
    "FAIL",
    "FILTER",
    "FIRST",
    "FOLLOWING",
    "FOR",
    "FOREIGN",
    "FROM",
    "FULL",
    "GENERATED",
    "GLOB",
    "GROUP",
    "GROUPS",
    "HAVING",
    "IF",
    "IGNORE",
    "IMMEDIATE",
    "IN",
    "INDEX",
    "INDEXED",
    "INITIALLY",
    "INNER",
    "INSERT",
    "INSTEAD",
    "INTERSECT",
    "INTO",
    "IS",
    "ISNULL",
    "JOIN",
    "KEY",
    "LAST",
    "LEFT",
    "LIKE",
    "LIMIT",
    "MATCH",
    "MATERIALIZED",
    "NATURAL",
    "NO",
    "NOT",
    "NOTHING",
    "NOTNULL",
    "NULL",
    "NULLS",
    "OF",
    "OFFSET",
    "ON",
    "OR",
    "ORDER",
    "OTHERS",
    "OUTER",
    "OVER",
    "PARTITION",
    "PLAN",
    "PRAGMA",
    "PRECEDING",
    "PRIMARY",
    "QUERY",
    "RAISE",
    "RANGE",
    "RECURSIVE",
    "REFERENCES",
    "REGEXP",
    "REINDEX",
    "RELEASE",
    "RENAME",
    "REPLACE",
    "RESTRICT",
    "RETURNING",
    "RIGHT",
    "ROLLBACK",
    "ROW",
    "ROWS",
    "SAVEPOINT",
    "SELECT",
    "SET",
    "TABLE",
    "TEMP",
    "TEMPORARY",
    "THEN",
    "TIES",
    "TO",
    "TRANSACTION",
    "TRIGGER",
    "UNBOUNDED",
    "UNION",
    "UNIQUE",
    "UPDATE",
    "USING",
    "VACUUM",
    "VALUES",
    "VIEW",
    "VIRTUAL",
    "WHEN",
    "WHERE",
    "WINDOW",
    "WITH",
    "WITHOUT",
];

#[cfg(test)]
mod tests {
    use crate::db::{QuerySql, SchemaTrait};
    use crate::enable_test_log;
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::sqlite::make_test_object;
    use std::path::Path;

    #[tokio::test]
    async fn test_auto_completion() {
        enable_test_log();
        let db = make_test_object().await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("departments", false).await;
        let _ = db.drop_table("employees", false).await;
        let _ = db.drop_view("employees_view").await;

        db.execute_sql(
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
                create index employees_name on employees(name)
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        enable_test_log();
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("sqlite_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();

        test_file_auto_completion(&content, &db, None, false).await;
    }

    #[ignore = "手动跑跑的测试"]
    #[tokio::test]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = make_test_object().await;
        test_file_auto_completion(include_str!("./xxx.sql"), &db, None, true).await;
    }
}
