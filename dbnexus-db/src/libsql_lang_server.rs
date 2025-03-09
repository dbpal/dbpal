#![allow(unused)]

use crate::{
    language_server::{CompletionItem, LanguageServer},
    libsql::LibSqlDB,
};

#[async_trait::async_trait]

impl LanguageServer for LibSqlDB {
    async fn auto_completion(
        &self, query_id: uuid::Uuid, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        unimplemented!()
    }
}
