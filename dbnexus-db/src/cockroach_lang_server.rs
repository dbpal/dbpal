use crate::cockroach::CockRoachDB;
use crate::db::FormatSqlOpts;
use crate::language_server::{CompletionItem, LangServerCommon, LanguageServer};

#[async_trait::async_trait]

impl LanguageServer for CockRoachDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        self.pg.auto_completion(sql, line, col, use_lexer).await
    }

    fn format_sql(&self, sql: &str, ops: Option<FormatSqlOpts>) -> anyhow::Result<String> {
        self.pg.format_sql(sql, ops)
    }
}

#[async_trait::async_trait]

impl LangServerCommon for CockRoachDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_database(self.pg.as_ref()).await
    }

    async fn list_schema(&self, _db: Option<&str>) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_schema(self.pg.as_ref(), None).await
    }
}
