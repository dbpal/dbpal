#![allow(unused)]

use crate::{
    cassandra::CassandraDB,
    language_server::{CompletionItem, LanguageServer},
};

#[async_trait::async_trait]
impl LanguageServer for CassandraDB {
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
