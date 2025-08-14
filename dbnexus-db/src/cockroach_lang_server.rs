use std::sync::Arc;

use crate::cockroach::{COCKROACH_DATA_TYPES, CockRoachDB};
use crate::db::FormatSqlOpts;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, LangServerCommon, LanguageServer, LexerCompletionTrait,
    try_completion_on_syntax,
};

use crate::sqlparser_completion::TempTableColumn;

#[async_trait::async_trait]

impl LanguageServer for CockRoachDB {
    async fn auto_completion(
        &self, _query_id: Option<uuid::Uuid>, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            sql,
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
    async fn list_procedure(&self) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_procedure(self.pg.as_ref(), None, None).await
    }

    async fn list_columns(&self, table_name: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        AstCompletionHelper::list_column(self.pg.as_ref(), None, None, table_name, false).await
    }
    type Dialect = sqlparser::dialect::PostgreSqlDialect;

    const TYPES: &'static [&'static str] = COCKROACH_DATA_TYPES;

    async fn get_key_words_fn(&self) -> anyhow::Result<Vec<CompletionItem>> {
        self.pg.get_key_words_fn().await
    }

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

#[cfg(test)]
mod tests {
    use crate::{
        cockroach::{CockRoachDB, make_test_object},
        db::monaco_editor_keyword_regexp,
        language_server::LexerCompletionTrait,
    };

    #[tokio::test]
    async fn print_key_words() {
        let db = make_test_object().await;

        let keywords = <CockRoachDB as LexerCompletionTrait>::get_key_words_fn(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|x| x.item.to_string())
            .collect::<Vec<_>>();

        let keywords: Vec<&str> = keywords.iter().map(|x| x.as_str()).collect();

        let s = monaco_editor_keyword_regexp(&keywords[..]);
        println!("{}", s);
    }
}
