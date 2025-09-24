use crate::cockroach::{COCKROACH_TYPES, CockRoachDB};

use crate::language_server::{
    AstCompletionHelper, CompletionItem, DBExecutable, ErrAndLoc, LangServerCommon, LanguageServer,
    LexerCompletionTrait, PostGresExtension, try_parser_dot_lexer,
};
use crate::sqlparser_completion::TempTableColumn;
#[async_trait::async_trait]

impl LanguageServer for CockRoachDB {
    fn extract_table_select_selection(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_selection(self.db.as_ref(), sql)
    }
    fn extract_table_select_sort(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_sort(self.db.as_ref(), sql)
    }
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        LangServerCommon::key_words(self)
            .await
            .map(|x| x.into_iter().map(|x| x.into()).collect())
    }
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        try_parser_dot_lexer(self, sql, line, col, use_lexer).await
    }

    fn parse_error(&self, sql: &str) -> anyhow::Result<Option<ErrAndLoc>> {
        LanguageServer::parse_error(self.db.as_ref(), sql)
    }
    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        LangServerCommon::is_dangerous_sql_by_parser(self, sql)
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for CockRoachDB {
    async fn postgres_list_extension(
        &self, installed: bool,
    ) -> anyhow::Result<Vec<PostGresExtension>> {
        AstCompletionHelper::postgres_list_extension(self.db.as_ref(), installed).await
    }

    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        AstCompletionHelper::get_query_columns(self.db.as_ref(), sql).await
    }

    async fn table_policy(&self, schema: Option<&str>, table: &str) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::table_policy(self.db.as_ref(), schema, table).await
    }

    async fn list_postgres_rule(
        &self, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_postgres_rule(self.db.as_ref(), schema, table).await
    }
    async fn list_constraint(
        &self, schema: Option<&str>, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_constraint(self.db.as_ref(), schema, table_name).await
    }
    async fn list_table_trigger(
        &self, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_table_trigger(self.db.as_ref(), schema, table).await
    }
}

#[async_trait::async_trait]
impl LangServerCommon for CockRoachDB {
    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_index(self.db.as_ref(), schema).await
    }

    async fn list_variable(&self) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_variable(self.db.as_ref()).await
    }

    async fn list_role(&self) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_role(self.db.as_ref()).await
    }
    async fn list_user(&self) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_user(self.db.as_ref()).await
    }
    async fn list_domain(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_domain(self.db.as_ref(), schema).await
    }
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_database(self.db.as_ref()).await
    }

    async fn key_words(&self) -> anyhow::Result<Vec<CompletionItem>> {
        LangServerCommon::key_words(self.db.as_ref()).await
    }
    async fn list_column(
        &self, schema: Option<&str>, name: &str, is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        LangServerCommon::list_column(self.db.as_ref(), schema, name, is_call).await
    }
    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::PostgreSqlDialect {})
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_schema(self.db.as_ref()).await
    }
    async fn list_execute(&self, _: Option<&str>) -> anyhow::Result<Vec<DBExecutable>> {
        LangServerCommon::list_execute(self.db.as_ref(), None).await
    }
    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_table(self.db.as_ref(), schema).await
    }
    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_view(self.db.as_ref(), schema).await
    }

    async fn list_function(
        &self, schema: Option<&str>, build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        LangServerCommon::list_function(self.db.as_ref(), schema, build_in).await
    }
    async fn list_procedure(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_procedure(self.db.as_ref(), schema).await
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for CockRoachDB {
    const TYPES: &'static [&'static str] = COCKROACH_TYPES;
}
