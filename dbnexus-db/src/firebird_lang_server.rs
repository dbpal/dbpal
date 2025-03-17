#![allow(unused)]

use crate::{
    firebird::FireBirdDB,
    language_server::{CompletionItem, LangServerCommon, LanguageServer},
};

#[async_trait::async_trait]
impl LanguageServer for FireBirdDB {
    async fn auto_completion(
        &self, query_id: uuid::Uuid, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl LangServerCommon for FireBirdDB {}
