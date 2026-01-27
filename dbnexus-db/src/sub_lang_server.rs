use std::collections::HashSet;

use crate::{
    cockroach::COCKROACH_TYPES,
    db::{CreatedTypeDef, SequenceDetail, SqlEvent},
    language_server::{
        try_parser_dot_lexer, AstCompletionHelper, CompletionItem, CompletionList, DBExecutable,
        ErrAndLoc, GetKeyWords, LangServerCommon, LanguageServer, LexerCompletionTrait,
        PostGresExtension,
    },
    mariadb::MariaDB,
    sqlparser_completion::TempTableColumn,
    sub::SubDB,
    tidb::TiDB,
};

impl SubDB {
    fn as_lang_server(&self) -> &dyn LanguageServer {
        match self {
            SubDB::TiDB(x) => x.as_ref() as &dyn LanguageServer,
            SubDB::MariaDb(db) => db.as_ref() as &dyn LanguageServer,
            SubDB::CocoRoach(db) => db.as_ref() as &dyn LanguageServer,
        }
    }
}

#[async_trait::async_trait]
impl LanguageServer for SubDB {
    async fn auto_completion(
        &self,
        query_id: Option<String>,
        sql: &str,
        line: u32,
        col: u32,
        use_lexer: bool,
    ) -> anyhow::Result<CompletionList> {
        if let Some(query_id) = query_id {
            match self {
                SubDB::TiDB(x) => {
                    x.completion_id.set(query_id);
                }
                SubDB::MariaDb(db) => {
                    db.completion_id.set(query_id);
                }
                SubDB::CocoRoach(db) => {
                    db.completion_id.set(query_id);
                }
            }
        }
        try_parser_dot_lexer(self, sql, line, col, use_lexer).await
    }

    async fn ai_text_completion(
        &self,
        sql: &str,
        line: u32,
        col: u32,
    ) -> anyhow::Result<CompletionList> {
        LexerCompletionTrait::ai_text_completion(self, sql, line, col).await
    }

    async fn try_get_max_id(&self, name: &str) -> anyhow::Result<String> {
        self.as_lang_server().try_get_max_id(name).await
    }

    async fn get_search_path(&self, query_id: &str) -> anyhow::Result<HashSet<String>> {
        self.as_lang_server().get_search_path(query_id).await
    }

    async fn check_sql(&self, sql: &str) -> anyhow::Result<Option<ErrAndLoc>> {
        self.as_lang_server().check_sql(sql).await
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        self.as_lang_server().is_dangerous_sql(sql)
    }

    fn extract_table_select_selection(&self, sql: &str) -> anyhow::Result<Option<String>> {
        self.as_lang_server().extract_table_select_selection(sql)
    }

    fn extract_table_select_sort(&self, sql: &str) -> anyhow::Result<Option<String>> {
        self.as_lang_server().extract_table_select_sort(sql)
    }
}

#[async_trait::async_trait]

impl GetKeyWords for SubDB {
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        match self {
            SubDB::TiDB(x) => {
                let t = TiDB { db: x.clone() };
                t.key_words().await
            }
            _ => self.as_lang_server_common().key_words().await,
        }
    }
}

#[async_trait::async_trait]

impl LexerCompletionTrait for SubDB {
    async fn get_types(&self) -> anyhow::Result<Vec<String>> {
        match self {
            SubDB::TiDB(db) => db.as_ref().get_types().await,
            SubDB::MariaDb(db) => db.as_ref().get_types().await,
            SubDB::CocoRoach(..) => {
                anyhow::Ok(COCKROACH_TYPES.iter().map(|x| x.to_string()).collect())
            }
        }
    }
}

impl SubDB {
    fn as_lang_server_common(&self) -> &dyn LangServerCommon {
        match self {
            SubDB::TiDB(x) => x.as_ref() as &dyn LangServerCommon,
            SubDB::MariaDb(db) => db.as_ref() as &dyn LangServerCommon,
            SubDB::CocoRoach(db) => db.as_ref() as &dyn LangServerCommon,
        }
    }
}

#[async_trait::async_trait]

impl LangServerCommon for SubDB {
    async fn postgres_list_extension(
        &self,
        _installed: bool,
    ) -> anyhow::Result<Vec<PostGresExtension>> {
        self.as_ast_completion_helper()
            .postgres_list_extension(_installed)
            .await
    }
    async fn list_cluster(&self) -> anyhow::Result<Vec<String>> {
        self.as_lang_server_common().list_cluster().await
    }
    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        self.as_lang_server_common().list_index(schema).await
    }

    async fn list_variable(&self) -> anyhow::Result<Vec<String>> {
        self.as_lang_server_common().list_variable().await
    }
    async fn list_event(&self, _schema: Option<&str>) -> anyhow::Result<Vec<SqlEvent>> {
        self.as_lang_server_common().list_event(_schema).await
    }

    async fn list_role(&self) -> anyhow::Result<Vec<String>> {
        self.as_lang_server_common().list_role().await
    }
    async fn list_user(&self) -> anyhow::Result<Vec<String>> {
        self.as_lang_server_common().list_user().await
    }
    async fn list_domain(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        self.as_lang_server_common().list_domain(schema).await
    }

    async fn list_created_type(&self, schema: Option<&str>) -> anyhow::Result<Vec<CreatedTypeDef>> {
        self.as_lang_server_common().list_created_type(schema).await
    }
    async fn list_sequence(&self, schema: Option<&str>) -> anyhow::Result<Vec<SequenceDetail>> {
        self.as_lang_server_common().list_sequence(schema).await
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        self.as_lang_server_common().list_schema().await
    }
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        self.as_ast_completion_helper().list_database().await
    }
    async fn list_execute(&self, schema: Option<&str>) -> anyhow::Result<Vec<DBExecutable>> {
        self.as_lang_server_common().list_execute(schema).await
    }
    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        self.as_lang_server_common().dialect()
    }
    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        match self {
            SubDB::MariaDb(x) => {
                let t = MariaDB { db: x.clone() };
                t.lang_server_list_table(schema).await
            }
            _ => self.as_lang_server_common().list_table(schema).await,
        }
    }
    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        self.as_lang_server_common().list_view(schema).await
    }

    async fn list_function(
        &self,
        schema: Option<&str>,
        include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        match self {
            SubDB::TiDB(x) => {
                let t = TiDB { db: x.clone() };
                t.list_function(schema, include_build_in).await
            }
            SubDB::MariaDb(db) => {
                let t = MariaDB { db: db.clone() };
                t.language_server_list_function(schema, include_build_in)
                    .await
            }
            _ => {
                self.as_lang_server_common()
                    .list_function(schema, include_build_in)
                    .await
            }
        }
    }
    async fn list_procedure(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        self.as_lang_server_common().list_procedure(schema).await
    }
    async fn list_column(
        &self,
        schema: Option<&str>,
        name: &str,
        is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        self.as_lang_server_common()
            .list_column(schema, name, is_call)
            .await
    }
}

impl SubDB {
    fn as_ast_completion_helper(&self) -> &dyn AstCompletionHelper {
        match self {
            SubDB::TiDB(x) => x.as_ref() as &dyn AstCompletionHelper,
            SubDB::MariaDb(db) => db.as_ref() as &dyn AstCompletionHelper,
            SubDB::CocoRoach(db) => db.as_ref() as &dyn AstCompletionHelper,
        }
    }
}

#[async_trait::async_trait]

impl AstCompletionHelper for SubDB {
    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        self.as_ast_completion_helper().get_query_columns(sql).await
    }

    async fn mysql_show_create_trigger(
        &self,
        schema: Option<&str>,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        self.as_ast_completion_helper()
            .mysql_show_create_trigger(schema)
            .await
    }

    async fn list_partition(
        &self,
        schema: Option<&str>,
        table: &str,
    ) -> anyhow::Result<Vec<String>> {
        self.as_ast_completion_helper()
            .list_partition(schema, table)
            .await
    }
    async fn table_policy(
        &self,
        _schema: Option<&str>,
        table: &str,
    ) -> anyhow::Result<Vec<String>> {
        self.as_ast_completion_helper()
            .table_policy(_schema, table)
            .await
    }

    async fn list_postgres_rule(
        &self,
        _schema: Option<&str>,
        table: &str,
    ) -> anyhow::Result<Vec<String>> {
        self.as_ast_completion_helper()
            .list_postgres_rule(_schema, table)
            .await
    }
    async fn list_constraint(
        &self,
        _schema: Option<&str>,
        table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        self.as_ast_completion_helper()
            .list_constraint(_schema, table_name)
            .await
    }
    async fn list_table_trigger(
        &self,
        _schema: Option<&str>,
        table: &str,
    ) -> anyhow::Result<Vec<String>> {
        self.as_ast_completion_helper()
            .list_table_trigger(_schema, table)
            .await
    }
}
