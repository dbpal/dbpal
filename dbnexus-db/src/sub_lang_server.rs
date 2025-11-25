use std::collections::HashSet;

use crate::{
    language_server::{CompletionList, ErrAndLoc, LanguageServer},
    sub::SubBackend,
};

impl SubBackend {
    fn as_lang_server(&self) -> &dyn LanguageServer {
        match self {
            SubBackend::TiDB(x) => x.as_ref(),
        }
    }
}

#[async_trait::async_trait]
impl LanguageServer for SubBackend {
    async fn auto_completion(
        &self, query_id: Option<String>, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<CompletionList> {
        self.as_lang_server()
            .auto_completion(query_id, sql, line, col, use_lexer)
            .await
    }

    async fn ai_text_completion(
        &self, sql: &str, line: u32, col: u32,
    ) -> anyhow::Result<CompletionList> {
        self.as_lang_server()
            .ai_text_completion(sql, line, col)
            .await
    }

    async fn try_get_max_id(&self, _name: &str) -> anyhow::Result<String> {
        self.as_lang_server().try_get_max_id(_name).await
    }

    async fn get_search_path(&self, _query_id: &str) -> anyhow::Result<HashSet<String>> {
        self.as_lang_server().get_search_path(_query_id).await
    }

    async fn check_sql(&self, _sql: &str) -> anyhow::Result<Option<ErrAndLoc>> {
        self.as_lang_server().check_sql(_sql).await
    }

    fn is_dangerous_sql(&self, _sql: &str) -> anyhow::Result<bool> {
        self.as_lang_server().is_dangerous_sql(_sql)
    }

    // extract `a=`` from `select * from where a =1`
    fn extract_table_select_selection(&self, sql: &str) -> anyhow::Result<Option<String>> {
        self.as_lang_server().extract_table_select_selection(sql)
    }
    // extrat name asc from a select statement.
    fn extract_table_select_sort(&self, sql: &str) -> anyhow::Result<Option<String>> {
        self.as_lang_server().extract_table_select_sort(sql)
    }
}
