use std::ops::DerefMut;

use crate::{
    completion::{
        try_completion_on_syntax, AutoCompletionTrait, CompletionItem, CompletionOnSynTaxHelper,
        LexerCompletionTrait,
    },
    db::{Field, SchemaTrait},
    sqlite::SqliteDB,
};
use sqlparser::ast::ObjectType;
use sqlx::Row;

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

impl SqliteDB {
    async fn list_index_name_impl(&self) -> anyhow::Result<Vec<String>> {
        let sql = r#"
        SELECT name
            FROM sqlite_master
            WHERE type = 'index';"#;
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name = r.get(0);
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl CompletionOnSynTaxHelper for SqliteDB {
    async fn list_table(
        &self, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        SchemaTrait::list_table(self)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }
    async fn list_column(
        &self, _db: Option<&str>, _schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<Field>> {
        SchemaTrait::show_columns(self, &table).await
    }
    async fn list_function(
        &self, _catelog: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        anyhow::bail!("sqlite can't use sql to create function.")
    }

    async fn list_drop_by_type(
        &self, typ: ObjectType, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        match typ {
            ObjectType::Table => CompletionOnSynTaxHelper::list_table(self, None, None).await,
            ObjectType::View => SchemaTrait::list_view(self)
                .await
                .map(|x| x.into_iter().map(|x| x.name).collect::<Vec<_>>()),
            ObjectType::Index => {
                // use by drop index of something
                // in this case we don't know the table
                // so we must can tell which index is base on index's name
                self.list_index_name_impl().await
            }
            _ => {
                anyhow::bail!("not support object type '{}' by sqlte.", typ)
            }
        }
    }
    async fn list_index(&self) -> anyhow::Result<Vec<String>> {
        self.list_index_name_impl().await
    }
}
