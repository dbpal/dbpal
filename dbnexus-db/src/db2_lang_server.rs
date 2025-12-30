#![allow(unused)]

use crate::{
    db2::Db2Db,
    language_server::{LangServerCommon, LanguageServer},
};

#[async_trait::async_trait]

impl LanguageServer for Db2Db {
    async fn auto_completion(
        &self, query_id: Option<String>, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<crate::language_server::CompletionList> {
        todo!()
    }

    async fn ai_text_completion(
        &self, sql: &str, line: u32, col: u32,
    ) -> anyhow::Result<crate::language_server::CompletionList> {
        todo!()
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        todo!()
    }

    fn extract_table_select_selection(&self, sql: &str) -> anyhow::Result<Option<String>> {
        todo!()
    }

    fn extract_table_select_sort(&self, sql: &str) -> anyhow::Result<Option<String>> {
        todo!()
    }
}
