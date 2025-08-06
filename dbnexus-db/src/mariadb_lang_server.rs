use std::ops::DerefMut;
use std::sync::Arc;

use crate::db::DBTrait;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, LangServerCommon, LanguageServer, LexerCompletionTrait,
    try_completion_on_syntax,
};
use crate::mariadb::MariaDB;
use crate::mysql::MysqlDB;
use sqlx::{AssertSqlSafe, Row};

#[async_trait::async_trait]
impl LanguageServer for MariaDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            &sql,
            line,
            col,
            sqlparser::dialect::MySqlDialect {},
            Arc::new(Clone::clone(self)),
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
            &sqlparser::dialect::MySqlDialect {},
            sqlparser::tokenizer::Location {
                line: line as u64,
                column: col as u64,
            },
        )
        .await;
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for MariaDB {
    async fn list_table(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let mut result = AstCompletionHelper::list_table(self.db.as_ref(), None, schema).await?;
        let mut conn = self.db.get_conn().await?;
        let sql = format!(
            "select SEQUENCE_NAME  from information_schema.SEQUENCES where SEQUENCE_SCHEMA = '{}'",
            MysqlDB::not_allow_empty_schema(self.db.schema_merge_config(schema))
                .map_err(|e| { log::error!("err:{}", e) })
                .ok()
                .unwrap_or("")
        );
        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            result.push(name);
        }
        anyhow::Ok(result)
    }

    async fn list_view(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_view(self.db.as_ref(), None, schema).await
    }

    async fn list_column(
        &self, _db: Option<&str>, schema: Option<&str>, name: &str,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_column(self.db.as_ref(), None, schema, &name).await
    }

    async fn list_function(
        &self, _db: Option<&str>, schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self.db.as_ref(), None, schema, include_build_in).await
    }

    async fn list_procedure(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_procedure(self.db.as_ref(), None, schema).await
    }

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_index(self.db.as_ref(), schema).await
    }

    async fn list_variable(&self) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_variable(self.db.as_ref()).await
    }

    async fn list_partition(
        &self, _db: Option<&str>, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_partition(self.db.as_ref(), None, schema, &table).await
    }

    async fn list_constraint(
        &self, _db: Option<&str>, schema: Option<&str>, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_constraint(self.db.as_ref(), None, schema, &table_name).await
    }

    async fn list_table_trigger(
        &self, _db: Option<&str>, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_table_trigger(self.db.as_ref(), None, schema, &table).await
    }
}

#[async_trait::async_trait]

impl LangServerCommon for MariaDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
    async fn list_schema(&self, _db: Option<&str>) -> anyhow::Result<Vec<String>> {
        DBTrait::list_schema(self).await
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for MariaDB {
    type Dialect = sqlparser::dialect::MySqlDialect;

    const KEY_WORDS: &'static [&'static str] = MysqlDB::KEY_WORDS;
    const TEXTS: &'static [&'static str] = MysqlDB::TEXTS;
    const TYPES: &'static [&'static str] = MysqlDB::TYPES;

    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self, None, None, true).await
    }

    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        self.db.list_table_or_view().await
    }
}
