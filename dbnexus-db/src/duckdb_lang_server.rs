use tokio::task::spawn_blocking;

use crate::duckdb::DuckDB;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, CompletionList, GetKeyWords,
    LangServerCommon, LanguageServer, LexerCompletionTrait, try_parser_dot_lexer,
};
use crate::sqlparser_completion::TempTableColumn;
use std::collections::HashSet;

#[async_trait::async_trait]
impl LanguageServer for DuckDB {
    async fn auto_completion(
        &self, query_id: Option<String>, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<CompletionList> {
        self.completion_id.set_optional(query_id);
        try_parser_dot_lexer(self, sql, line, col, use_lexer).await
    }

    async fn ai_text_completion(
        &self, sql: &str, line: u32, col: u32,
    ) -> anyhow::Result<CompletionList> {
        LexerCompletionTrait::ai_text_completion(self, sql, line, col).await
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        LangServerCommon::is_dangerous_sql_by_parser(self, sql)
    }

    fn extract_table_select_selection(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_selection(self, sql)
    }

    fn extract_table_select_sort(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_sort(self, sql)
    }
}

#[async_trait::async_trait]

impl LangServerCommon for DuckDB {
    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = self.get_schema(schema).await?;
        let ret = self.list_table_impl(schema.as_str()).await?;
        anyhow::Ok(ret.into_iter().map(|x| x.name).collect())
    }

    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = self.get_schema(schema).await?;
        let ret = self.list_view_impl(&schema).await?;
        anyhow::Ok(ret.into_iter().map(|x| x.name).collect())
    }

    async fn list_column(
        &self, schema: Option<&str>, name: &str, is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let this = Clone::clone(self);
        let schema = this.get_schema(schema).await?;
        let sql = format!(r#"select * from "{}"."{}" limit 1"#, schema, name);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            stmt.execute([])?;
            let fs = stmt
                .column_names()
                .into_iter()
                .enumerate()
                .map(|(index, name)| TempTableColumn {
                    name: name.into(),
                    data_type: Some(stmt.column_type(index).to_string().into()),
                })
                .collect::<Vec<_>>();
            anyhow::Ok(fs)
        })
        .await?
    }

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        todo!()
    }

    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::DuckDbDialect {})
    }

    async fn list_function(
        &self, _schema: Option<&str>, _include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let this = Clone::clone(self);
        let sql = r#"SELECT function_name ,description  FROM duckdb_functions()"#;
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query([])?;
            let mut ret = Vec::new();
            let mut names = HashSet::new();
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                if names.contains(&name) {
                    continue;
                }
                let desc: Option<String> = row.get(1)?;
                ret.push(CompletionItem {
                    item: name.clone().into(),
                    kind: CompletionKind::Function,
                    desc: desc.map(|x| x.into()),
                });
                names.insert(name);
            }
            anyhow::Ok(ret)
        })
        .await?
    }
}

#[async_trait::async_trait]

impl AstCompletionHelper for DuckDB {
    async fn duckdb_extensions(
        &self, installed: Option<bool>,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let mut sql =
            "SELECT extension_name, installed, description FROM duckdb_extensions() ".to_string();
        if let Some(installed) = installed {
            sql.push_str(format!("where installed={}", installed).as_str());
        }
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            let mut rows = stmt.query([])?;
            let mut ret = Vec::new();
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                let desc: Option<String> = row.get(2)?;
                ret.push(CompletionItem {
                    item: name.into(),
                    kind: CompletionKind::Extension,
                    desc: desc.map(|x| x.into()),
                });
            }
            anyhow::Ok(ret)
        })
        .await?
    }

    async fn get_query_columns(
        &self, _sql: &str,
    ) -> anyhow::Result<Vec<crate::sqlparser_completion::TempTableColumn>> {
        todo!()
    }
}

impl DuckDB {
    async fn get_schema(&self, schema: Option<&str>) -> anyhow::Result<String> {
        match schema {
            Some(x) => anyhow::Ok(x.to_string()),
            None => todo!(),
        }
    }
}

#[async_trait::async_trait]

impl GetKeyWords for DuckDB {
    // some backend can use a query to query this.
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let sql = "select keyword_name from duckdb_keywords() ";
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query([])?;
            let mut ret = Vec::new();
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                let name = name.to_uppercase();
                ret.push(name);
            }
            anyhow::Ok(ret)
        })
        .await?
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for DuckDB {
    async fn get_types(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(TYPES.iter().map(|x| x.to_string()).collect())
    }

    async fn get_text(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
}

const TYPES: &'static [&'static str] = &[
    "BIGINT",
    "INT8",
    "LONG",
    "BIT",
    "BITSTRING",
    "BLOB",
    "BYTEA",
    "BINARY",
    "VARBINARY",
    "BOOLEAN",
    "BOOL",
    "LOGICAL",
    "DATE",
    "DECIMAL",
    "NUMERIC",
    "DOUBLE",
    "FLOAT8",
    "FLOAT",
    "FLOAT4",
    "REAL",
    "HUGEINT",
    "INTEGER",
    "INT4",
    "INT",
    "SIGNED",
    "INTERVAL",
    "JSON",
    "SMALLINT",
    "INT2",
    "SHORT",
    "TIME",
    "TIMESTAMP WITH TIME ZONE",
    "TIMESTAMPTZ",
    "TIMESTAMP",
    "DATETIME",
    "TINYINT",
    "INT1",
    "UBIGINT",
    "UHUGEINT",
    "UINTEGER",
    "USMALLINT",
    "UTINYINT",
    "UUID",
    "VARCHAR",
    "MAP",
    "STRUCT",
    "UNION",
];

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::db::QuerySql;
    use crate::duckdb::make_test_object;
    use crate::enable_test_log;
    use crate::language_server::completion_test::test_file_auto_completion;

    // partition test failed , sqlparser can't parse a valid partition statement.
    #[tokio::test]
    async fn test_auto_completion() {
        let db = make_test_object(Some("main".to_string()));
        enable_test_log();
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _err = db
            .execute_sql(
                query_id,
                r#"
                    CREATE TABLE employees (
                        employee_id INTEGER PRIMARY KEY,  
                        first_name TEXT NOT NULL,        
                        last_name TEXT NOT NULL,         
                        job_title TEXT NOT NULL,         
                        hire_date DATE NOT NULL,          
                        salary DECIMAL(10, 2) NOT NULL  
                    );
                r#"
                 "#,
            )
            .await
            .unwrap();
        if let Err(err) = _err {
            println!("create employees error failed:{:?}", err)
        }
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("duckdb_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        test_file_auto_completion(&content, &db, false).await;
    }
}
