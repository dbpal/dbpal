use crate::{
    completion::{
        try_completion_on_syntax, AutoCompletionTrait, CompletionItem, LexerCompletionTrait,
    },
    sqlite::SqliteDB,
};

#[async_trait::async_trait]
impl AutoCompletionTrait for SqliteDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            &sql,
            line,
            col,
            &sqlparser::dialect::SQLiteDialect {},
            unimplemented!(),
        )
        .await;
        if r.is_ok() {
            return r;
        }
        return LexerCompletionTrait::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::SQLiteDialect {},
            sqlparser::tokenizer::Location {
                line: line as u64,
                column: col as u64,
            },
        )
        .await;
    }
}
