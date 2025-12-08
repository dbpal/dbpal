use crate::duckdb::{DuckDB, use_database};
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, CompletionList, GetKeyWords,
    LangServerCommon, LanguageServer, LexerCompletionTrait,
};
use crate::sqlparser_completion::TempTableColumn;

use std::collections::HashSet;
use tokio::task::block_in_place;
#[async_trait::async_trait]
impl LanguageServer for DuckDB {
    async fn auto_completion(
        &self, query_id: Option<String>, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<CompletionList> {
        unimplemented!()
    }

    async fn ai_text_completion(
        &self, sql: &str, line: u32, col: u32,
    ) -> anyhow::Result<CompletionList> {
        todo!()
    }

    fn is_dangerous_sql(&self, _sql: &str) -> anyhow::Result<bool> {
        todo!()
    }

    fn extract_table_select_selection(&self, sql: &str) -> anyhow::Result<Option<String>> {
        todo!()
    }

    fn extract_table_select_sort(&self, sql: &str) -> anyhow::Result<Option<String>> {
        todo!()
    }
}

#[async_trait::async_trait]

impl LangServerCommon for DuckDB {
    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let db = Self::not_allow_empty_db(self.db_merge_config(schema))?;
        let schema = Self::not_allow_empty_db(self.schema_merge_config(schema))?;
        let ret = self.list_table_impl(db, schema)?;
        anyhow::Ok(ret.into_iter().map(|x| x.name).collect())
    }

    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let db = Self::not_allow_empty_db(self.db_merge_config(schema))?;
        let schema = Self::not_allow_empty_db(self.schema_merge_config(schema))?;
        let ret = self.list_view_impl(db, schema)?;
        anyhow::Ok(ret.into_iter().map(|x| x.name).collect())
    }

    async fn list_column(
        &self, schema: Option<&str>, name: &str, is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let schema = Self::not_allow_empty_db(self.schema_merge_config(schema))?;
        let sql = format!(r#"select * from "{}"."{}" limit 1"#, schema, name);
        let conn = self.get_conn()?;
        use_database(&conn, schema)?;
        let mut stmt = block_in_place(|| conn.prepare(&sql))?;
        block_in_place(|| stmt.execute([]))?;
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
    }

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        todo!()
    }

    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        todo!()
    }

    async fn list_function(
        &self, _schema: Option<&str>, _include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let sql = r#"SELECT function_name ,description  FROM duckdb_functions()"#;

        let conn = self.get_conn()?;
        let mut stmt = block_in_place(|| conn.prepare(sql))?;
        let mut rows = block_in_place(|| stmt.query([]))?;
        let mut ret = Vec::new();
        let mut names = HashSet::new();

        while let Some(row) = block_in_place(|| rows.next())? {
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

        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = block_in_place(|| stmt.query([]))?;
        let mut ret = Vec::new();
        while let Some(row) = block_in_place(|| rows.next())? {
            let name: String = row.get(0)?;
            let desc: Option<String> = row.get(2)?;
            ret.push(CompletionItem {
                item: name.into(),
                kind: CompletionKind::Extension,
                desc: desc.map(|x| x.into()),
            });
        }

        anyhow::Ok(ret)
    }

    async fn get_query_columns(
        &self, _sql: &str,
    ) -> anyhow::Result<Vec<crate::sqlparser_completion::TempTableColumn>> {
        todo!()
    }
}

impl DuckDB {
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

    fn not_allow_empty_db<'a>(db: Option<&'a str>) -> anyhow::Result<&'a str> {
        match db {
            Some(x) => return anyhow::Ok(x),
            None => anyhow::bail!("empty database"),
        }
    }

    fn not_allow_empty_schema<'a>(db: Option<&'a str>) -> anyhow::Result<&'a str> {
        match db {
            Some(x) => return anyhow::Ok(x),
            None => anyhow::bail!("empty schema"),
        }
    }
}

#[async_trait::async_trait]

impl GetKeyWords for DuckDB {
    // some backend can use a query to query this.
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        let sql = "select keyword_name from duckdb_keywords() ";
        let conn = self.get_conn()?;
        let mut stmt = block_in_place(|| conn.prepare(sql))?;
        let mut rows = block_in_place(|| stmt.query([]))?;
        let mut ret = Vec::new();
        while let Some(row) = block_in_place(|| rows.next())? {
            let name: String = row.get(0)?;
            let name = name.to_uppercase();
            ret.push(name);
        }
        anyhow::Ok(ret)
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
