use std::{sync::Arc, vec};

use crate::{
    clickhouse::{ClickHouseDB, JsonRespMeta},
    db::DBTrait,
    language_server::{
        AstCompletionHelper, CompletionItem, CompletionKind, LanguageServer, LexerCompletionTrait,
        try_completion_on_syntax,
    },
};

#[async_trait::async_trait]

impl LanguageServer for ClickHouseDB {
    async fn auto_completion(
        &self, query_id: uuid::Uuid, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            &sql,
            query_id,
            line,
            col,
            sqlparser::dialect::SQLiteDialect {},
            Arc::new(Clone::clone(self)),
        )
        .await;

        match r {
            Ok(xs) => return Ok(xs),
            Err(err) => {
                log::error!("completion on ast failed,err:{:?}", err);
            }
        };
        if !use_lexer {
            log::warn!("disabled use lexer , we can return empty data.");
            return anyhow::Ok(vec![]);
        }
        return LexerCompletionTrait::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::ClickHouseDialect {},
            sqlparser::tokenizer::Location {
                line: line as u64,
                column: col as u64,
            },
        )
        .await;
    }
}

impl ClickHouseDB {
    fn schema_merge_config<'a, 'b>(&'a self, schema: Option<&'b str>) -> Option<&'a str>
    where
        'b: 'a,
    {
        if schema.is_some() {
            return schema;
        }
        return self.config.db.as_deref();
    }

    fn not_allow_none_schema(schema: Option<&str>) -> anyhow::Result<&str> {
        match schema {
            Some(x) => anyhow::Ok(x),
            None => anyhow::bail!("empty schema"),
        }
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for ClickHouseDB {
    async fn list_schema(&self, _db: Option<&str>) -> anyhow::Result<Vec<String>> {
        DBTrait::list_schema(self).await
    }
    async fn list_table(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let db = Self::not_allow_none_schema(self.schema_merge_config(schema))?;
        let v = self.list_table_impl(db).await?;
        anyhow::Ok(v.into_iter().map(|x| x.name).collect())
    }

    async fn list_view(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let db = Self::not_allow_none_schema(self.schema_merge_config(schema))?;
        let mut ret = Vec::new();
        ret.extend(self.list_view_impl(db).await?.into_iter().map(|x| x.name));
        ret.extend(
            self.list_materialized_view_impl(db)
                .await?
                .into_iter()
                .map(|x| x.name),
        );
        anyhow::Ok(ret)
    }
    async fn list_column(
        &self, _db: Option<&str>, schema: Option<&str>, name: &str,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let db = Self::not_allow_none_schema(self.schema_merge_config(schema))?;
        let sql = format!(r#"select * from "{}"."{}"  limit 0 "#, db, name);
        let meta = self.query_as_json(&sql).await?.meta;
        anyhow::Ok(
            meta.into_iter()
                .map(|x| {
                    let JsonRespMeta { name, r#type } = x;
                    CompletionItem {
                        item: name.into(),
                        kind: CompletionKind::Column,
                        desc: Some(r#type.into()),
                    }
                })
                .collect(),
        )
    }

    async fn list_function(
        &self, _db: Option<&str>, _schema: Option<&str>, _include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let sql = "select name , description from system.functions";
        let conn = self.get_conn()?;
        let result: Vec<(String, String)> = conn.query(sql).fetch_all().await?;

        anyhow::Ok(
            result
                .into_iter()
                .map(|(name, desc)| CompletionItem {
                    item: name.into(),
                    kind: CompletionKind::Function,
                    desc: Some(desc.into()),
                })
                .collect::<Vec<_>>(),
        )
    }

    async fn list_index(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for ClickHouseDB {
    type Dialect = sqlparser::dialect::ClickHouseDialect;
    const KEY_WORDS: &'static [&'static str] = &[];
    const TEXTS: &'static [&'static str] = &["true", "false"];
    const TYPES: &'static [&'static str] = TYPES;
    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self, None, None, true).await
    }

    async fn get_key_words_fn(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let sql = "select * from system.keywords";
        let conn = self.get_conn()?;
        let ret: Vec<String> = conn.query(sql).fetch_all().await?;
        anyhow::Ok(
            ret.into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::KeyWord,
                    desc: None,
                })
                .collect(),
        )
    }

    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        let mut x = AstCompletionHelper::list_table(self, None, None)
            .await
            .ok()
            .unwrap_or_default();
        x.extend(
            AstCompletionHelper::list_view(self, None, None)
                .await
                .ok()
                .unwrap_or_default()
                .into_iter(),
        );
        anyhow::Ok(x)
    }
}

const TYPES: &'static [&'static str] = &[
    "UInt8",
    "UInt16",
    "UInt32",
    "UInt64",
    "UInt128",
    "UInt256",
    "Int8",
    "Int16",
    "Int32",
    "Int64",
    "Int128",
    "Int256",
    "Float32",
    "Float64",
    "Decimal",
    "String",
    "FixedString",
    "UUID",
    "IPv6",
    "IPv4",
    "Date",
    "Date32",
    "DateTime",
    "DateTime64",
    "Tuple",
    "Map",
    "LowCardinality",
    "Nullable",
    "JSON",
];

#[cfg(test)]

mod tests {
    use std::collections::HashSet;

    use crate::{
        clickhouse::{ClickHouseDB, make_test_object},
        db::monaco_editor_keyword_regexp,
        language_server::LexerCompletionTrait,
    };

    // clickhouse is a little different keyword can select from database
    // but frontend still need this definition.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn print_key_words() {
        let db = make_test_object();

        let keywords = <ClickHouseDB as LexerCompletionTrait>::get_key_words_fn(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|x| x.item.to_string())
            .collect::<Vec<_>>();
        // `ADMIN OPTION FOR` clickhouse can have this kind of result.

        let keywords = keywords
            .into_iter()
            .map(|x| x.split(" ").map(|x| x.to_uppercase()).collect::<Vec<_>>())
            .flat_map(|x| x.into_iter())
            .collect::<HashSet<_>>();

        let keywords = keywords.into_iter().collect::<Vec<_>>();

        let keywords: Vec<&str> = keywords.iter().map(|x| x.as_str()).collect();

        let s = monaco_editor_keyword_regexp(&keywords[..]);
        println!("{}", s);
    }
}
