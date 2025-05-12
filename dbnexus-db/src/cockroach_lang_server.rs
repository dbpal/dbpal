use std::sync::Arc;

use crate::cockroach::{COCKROACH_DATA_TYPES, CockRoachDB};
use crate::db::FormatSqlOpts;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, LangServerCommon, LanguageServer, LexerCompletionTrait,
    try_completion_on_syntax,
};
use crate::pg_lang_server::PG_KEYWORDS;

#[async_trait::async_trait]

impl LanguageServer for CockRoachDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            &sql,
            line,
            col,
            sqlparser::dialect::PostgreSqlDialect {},
            Arc::new(Clone::clone(self.pg.as_ref())),
        )
        .await;
        match r {
            Ok(xs) => return Ok(xs),
            Err(err) => {
                log::error!("completion on ast failed,err:{}", err);
            }
        };
        if !use_lexer {
            log::warn!("disabled use lexer , we will return empty data.");
            return anyhow::Ok(vec![]);
        }
        return LexerCompletionTrait::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::PostgreSqlDialect {},
            sqlparser::tokenizer::Location {
                line: line as u64,
                column: col as u64,
            },
        )
        .await;
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

#[async_trait::async_trait]
impl LexerCompletionTrait for CockRoachDB {
    type Dialect = sqlparser::dialect::PostgreSqlDialect;

    const KEY_WORDS: &'static [&'static str] = PG_KEYWORDS;
    const TYPES: &'static [&'static str] = COCKROACH_DATA_TYPES;

    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        let mut ret = AstCompletionHelper::list_table(self.pg.as_ref(), None, None).await?;
        ret.extend(
            AstCompletionHelper::list_view(self.pg.as_ref(), None, None)
                .await?
                .into_iter(),
        );
        anyhow::Ok(ret)
    }

    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self.pg.as_ref(), None, None, true).await
    }
}
