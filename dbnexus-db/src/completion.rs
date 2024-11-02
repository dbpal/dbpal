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
        &self, sql: &str, line: u32, col: u32,
    ) -> anyhow::Result<Vec<CompletionItem>>;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompletionItem {
    pub item: Cow<'static, str>,
    pub kind: CompletionKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, TypeDef)]
pub enum CompletionKind {
    // commnad right now use by redis
    Command,
    KeyWord,
    Function,
    Dbname,
    SchemaName,
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
            .map(|x| x.into_iter().map(|x| x.field).collect::<Vec<_>>())
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
                    item: Cow::Borrowed(*x),
                    kind: CompletionKind::KeyWord,
                })
                .collect::<Vec<_>>(),
        );
        ret.extend(
            Self::BUILD_IN_FUNCTIONS
                .iter()
                .map(|x| CompletionItem {
                    item: Cow::Borrowed(*x),
                    kind: CompletionKind::Function,
                })
                .collect::<Vec<_>>(),
        );
        ret.extend(
            Self::TYPES
                .iter()
                .map(|x| CompletionItem {
                    item: Cow::Borrowed(*x),
                    kind: CompletionKind::Type,
                })
                .collect::<Vec<_>>(),
        );
        ret.extend(
            Self::TEXTS
                .iter()
                .map(|x| CompletionItem {
                    item: Cow::Borrowed(*x),
                    kind: CompletionKind::Text,
                })
                .collect::<Vec<_>>(),
        );
        let list_db = self.list_db().await.unwrap_or(vec![]);

        ret.extend(
            list_db
                .iter()
                .map(|x| CompletionItem {
                    item: Cow::Owned(x.clone()),
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
                        item: Cow::Owned(x),
                        kind: CompletionKind::Column,
                    }));
                }
                ret.extend(
                    list_tables
                        .into_iter()
                        .map(|x| CompletionItem {
                            item: Cow::Owned(x),
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
                    item: Cow::Owned(x),
                    kind: CompletionKind::Column,
                }));
            }

            ret.extend(
                list_tables
                    .into_iter()
                    .map(|x| CompletionItem {
                        item: Cow::Owned(x),
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
        if names.contains(&item.item.to_lowercase()) {
            // don't handle already handled.
            continue;
        }

        let m = matcher.fuzzy_match(&item.item, pattern);
        if let Some(s) = m {
            names.insert(item.item.to_lowercase());

            matched.push(AndScore { item, score: s });
        }
    }
    // desc order
    matched.sort_by(|a, b| b.score.cmp(&a.score));
    matched.into_iter().map(|x| x.item).collect()
}

#[cfg(test)]
pub(crate) mod completion_test {

    use super::{AutoCompletionTrait, CompletionItem};

    /// test if completion items contains a centains string
    /// use in test to see if completion is ok.
    /// this fn not returns just panic if not contains.
    pub fn test_completion_items_contains(items: &[CompletionItem], name: &str) {
        assert!(
            items.iter().any(|x| x.item.eq_ignore_ascii_case(name)),
            "items:{:?},expect:{:?}",
            items,
            name
        )
    }

    #[test]
    fn test_test_completion_items_contains() {
        use crate::completion::CompletionKind;

        use super::CompletionItem;

        test_completion_items_contains(
            &[CompletionItem {
                item: "aaa".into(),
                kind: CompletionKind::Text,
            }],
            "aaa",
        );
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct CompletionOneTest {
        pub(crate) sql: String,
        pub(crate) tests: Vec<(u64, Vec<String>)>,
    }

    pub(crate) fn parse_auto_completion_test_file(s: &str) -> Vec<CompletionOneTest> {
        let x: Vec<_> = s.lines().collect();
        x.chunks(2)
            .map(|x| {
                let cmd = &x[0];
                let e = &x[1];
                let es = e.split(";");
                let es = es
                    .map(|x| {
                        let es = x.split(" ");
                        let mut es = es.filter(|x| !x.is_empty()).collect::<Vec<_>>().into_iter();
                        let col = es.next().unwrap().parse::<u64>().unwrap();
                        let es = es.map(|x| x.to_string()).collect::<Vec<_>>();
                        (col, es)
                    })
                    .collect::<Vec<_>>();

                CompletionOneTest {
                    sql: cmd.to_string(),
                    tests: es,
                }
            })
            .collect()
    }

    #[test]
    fn test_parse_auto_completion_test_file() {
        let x = parse_auto_completion_test_file(
            r#"append str
1 append ; 11 str
select xxxx
1 select ; 4 xxxx
"#,
        );

        assert_eq!(
            x,
            vec![
                CompletionOneTest {
                    sql: "append str".to_string(),
                    tests: vec![
                        (1, vec!["append".to_string()]),
                        (11, vec!["str".to_string()])
                    ]
                },
                CompletionOneTest {
                    sql: "select xxxx".to_string(),
                    tests: vec![
                        (1, vec!["select".to_string()]),
                        (4, vec!["xxxx".to_string()])
                    ]
                }
            ]
        );
    }

    pub(crate) async fn test_file_auto_completion(content: &str, db: &dyn AutoCompletionTrait) {
        let items = parse_auto_completion_test_file(&content);
        for item in items.iter() {
            log::trace!("########completion on {:?}", item.sql);
            for i in item.tests.iter() {
                log::trace!("on pos:{}", i.0);
                let items = db.auto_completion(&item.sql, 1, i.0 as u32).await.unwrap();
                for should in i.1.iter() {
                    test_completion_items_contains(&items[..], &should);
                }
            }
        }
    }
}
