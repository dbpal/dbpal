#![allow(unused)]

use crate::duckdb::DuckDB;
use crate::language_server::{CompletionItem, LanguageServer};

#[async_trait::async_trait]

impl LanguageServer for DuckDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        unimplemented!()
    }
}
