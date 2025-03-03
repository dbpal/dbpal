#![allow(unused)]

use crate::duckdb::DuckDB;
use crate::language_server::{CompletionItem, LanguageServer};

#[async_trait::async_trait]

impl LanguageServer for DuckDB {
    async fn auto_completion(
        &self,
        query_id: uuid::Uuid,
        sql: &str,
        line: u32,
        col: u32,
        // if use the lexer to completing
        // right now this parameter only use in test.
        use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        unimplemented!()
    }
}

// CREATE TABLE employees (
// employee_id INTEGER PRIMARY KEY,
// first_name VARCHAR,
// last_name VARCHAR,
// position VARCHAR,
// salary DECIMAL(10, 2)
// );
//
