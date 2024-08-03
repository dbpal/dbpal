use std::borrow::Cow;
use std::collections::HashSet;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use sqlparser::tokenizer::Token;
use typescript_type_def::TypeDef;

use crate::db::DBTrait;

#[async_trait::async_trait]
pub trait AutoCompletionTrait {
    async fn auto_completion(
        &self, sql: &str, pos: sqlparser::tokenizer::Location,
    ) -> anyhow::Result<Vec<CompletionItem>>;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompletionItem {
    pub name: Cow<'static, str>,
    pub kind: CompletionKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, TypeDef)]
pub enum CompletionKind {
    KeyWord,
    Function,
    Dbname,
    TableName,
    Column,
    Text,
    Type,
}

#[async_trait::async_trait]
pub trait DBListCompletionItem {
    /// some fact about the backend
    /// postgres and mysql etc support multiple database
    /// while sqlite only contains one database
    fn is_multi_db(&self) -> bool {
        true
    }
    async fn list_table(&self, db: Option<String>) -> anyhow::Result<Vec<String>>;
    async fn list_db(&self) -> anyhow::Result<Vec<String>>;
    async fn list_columns(
        &self, db: Option<String>, table_name: &str,
    ) -> anyhow::Result<Vec<String>>;
}

#[async_trait::async_trait]
impl<T: DBTrait> DBListCompletionItem for T {
    async fn list_table(&self, db: Option<String>) -> anyhow::Result<Vec<String>> {
        let x = DBTrait::open_schema(self, db.as_deref().unwrap()).await?;

        x.list_table()
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect::<Vec<_>>())
    }

    async fn list_db(&self) -> anyhow::Result<Vec<String>> {
        DBTrait::list_schema(self).await
    }

    async fn list_columns(
        &self, db: Option<String>, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        let x = DBTrait::open_schema(self, db.as_deref().unwrap()).await?;
        x.show_columns(table_name)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect::<Vec<_>>())
    }
}

pub fn in_pos(
    pos: sqlparser::tokenizer::Location, t: sqlparser::tokenizer::Location, len: impl Into<u64>,
) -> bool {
    pos.line == t.line && (pos.column >= t.column && pos.column <= (t.column + len.into()))
}

#[async_trait::async_trait]

pub trait DBLexerCompletionBackend: DBListCompletionItem {
    const KEY_WORDS: &'static [&'static str];
    const BUILD_IN_FUNCTIONS: &'static [&'static str];
    const TYPES: &'static [&'static str];
    const TEXTS: &'static [&'static str] = &[];

    type Dialect: sqlparser::dialect::Dialect + Sync + Send + 'static;

    async fn extra_completion_item(&self) -> Vec<CompletionItem> {
        vec![]
    }

    async fn do_auto_completion(
        &self, sql: &str, dialect: &Self::Dialect, pos: sqlparser::tokenizer::Location,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let tokens = sqlparser::tokenizer::Tokenizer::new(dialect, sql)
            .tokenize_with_location()
            .unwrap_or(vec![]);
        let all_item = self.all_completion_items().await;

        for t in tokens.iter() {
            let pattern = t.token.to_string();
            match t.token {
                Token::Word(_) => {
                    // we only completion on word.
                }
                _ => {
                    continue;
                }
            }
            if in_pos(pos, t.location, pattern.len() as u64) {
                // matched
                return anyhow::Ok(match_pattern_and_sort(all_item, pattern.as_str()));
            }
        }
        return anyhow::Ok(all_item);
    }

    async fn all_completion_items(&self) -> Vec<CompletionItem> {
        let mut ret = Vec::new();
        ret.extend(
            Self::KEY_WORDS
                .iter()
                .map(|x| CompletionItem {
                    name: Cow::Borrowed(*x),
                    kind: CompletionKind::KeyWord,
                })
                .collect::<Vec<_>>(),
        );
        ret.extend(
            Self::BUILD_IN_FUNCTIONS
                .iter()
                .map(|x| CompletionItem {
                    name: Cow::Borrowed(*x),
                    kind: CompletionKind::Function,
                })
                .collect::<Vec<_>>(),
        );
        ret.extend(
            Self::TYPES
                .iter()
                .map(|x| CompletionItem {
                    name: Cow::Borrowed(*x),
                    kind: CompletionKind::Type,
                })
                .collect::<Vec<_>>(),
        );
        ret.extend(
            Self::TEXTS
                .iter()
                .map(|x| CompletionItem {
                    name: Cow::Borrowed(*x),
                    kind: CompletionKind::Text,
                })
                .collect::<Vec<_>>(),
        );
        let list_db = self.list_db().await.unwrap_or(vec![]);

        ret.extend(
            list_db
                .iter()
                .map(|x| CompletionItem {
                    name: Cow::Owned(x.clone()),
                    kind: CompletionKind::Dbname,
                })
                .collect::<Vec<_>>(),
        );

        if self.is_multi_db() {
            for db in list_db.iter() {
                let list_tables = self
                    .list_table(Some(db.to_string()))
                    .await
                    .unwrap_or(vec![]);
                for t in list_tables.iter() {
                    let cols = self
                        .list_columns(Some(db.to_string()), t.as_str())
                        .await
                        .unwrap_or(vec![]);
                    ret.extend(cols.into_iter().map(|x| CompletionItem {
                        name: Cow::Owned(x),
                        kind: CompletionKind::Column,
                    }));
                }
                ret.extend(
                    list_tables
                        .into_iter()
                        .map(|x| CompletionItem {
                            name: Cow::Owned(x),
                            kind: CompletionKind::TableName,
                        })
                        .collect::<Vec<_>>(),
                );
            }
        } else {
            let list_tables = self.list_table(None).await.unwrap_or(vec![]);
            for t in list_tables.iter() {
                let cols = self.list_columns(None, t.as_str()).await.unwrap_or(vec![]);
                ret.extend(cols.into_iter().map(|x| CompletionItem {
                    name: Cow::Owned(x),
                    kind: CompletionKind::Column,
                }));
            }

            ret.extend(
                list_tables
                    .into_iter()
                    .map(|x| CompletionItem {
                        name: Cow::Owned(x),
                        kind: CompletionKind::TableName,
                    })
                    .collect::<Vec<_>>(),
            );
        }
        ret.extend(self.extra_completion_item().await.into_iter());
        ret
    }
}

pub fn match_pattern_and_sort(items: Vec<CompletionItem>, pattern: &str) -> Vec<CompletionItem> {
    pub struct AndScore {
        item: CompletionItem,
        score: i64,
    }
    let matcher = SkimMatcherV2::default();
    let mut matched = Vec::new();
    let mut names = HashSet::new();

    for item in items.into_iter() {
        if names.contains(&item.name.to_lowercase()) {
            // don't handle already handled.
            continue;
        }

        let m = matcher.fuzzy_match(&item.name, pattern);
        if let Some(s) = m {
            names.insert(item.name.to_lowercase());

            matched.push(AndScore { item, score: s });
        }
    }
    // desc order
    matched.sort_by(|a, b| b.score.cmp(&a.score));
    matched.into_iter().map(|x| x.item).collect()
}

#[cfg(test)]
mod tests {}
