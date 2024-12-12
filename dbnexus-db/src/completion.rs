use std::borrow::Cow;

use std::collections::HashSet;
use std::sync::Arc;

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;
use sqlparser::ast::Statement;
use sqlparser::tokenizer::{Location, Token};
use typescript_type_def::TypeDef;

use crate::db::{monaco_editor_keyword_regexp, not_support_features, BackendName};
use crate::sqlparser_completion::CompletionStament;

#[async_trait::async_trait]
pub trait AutoCompletionTrait {
    async fn auto_completion(
        &self,
        uuid: uuid::Uuid,
        sql: &str,
        line: u32,
        col: u32,
        // if use the lexer to completing
        // right now this parameter only use in test.
        use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>>;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompletionItem {
    pub item: Cow<'static, str>,
    pub kind: CompletionKind,
    pub desc: Option<Cow<'static, str>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, TypeDef)]
pub enum CompletionKind {
    // commnad right now use by redis
    Command,
    KeyWord,
    Function,
    Database,
    Schema,
    Table,
    View,
    Column,
    Text,
    Type,
    Index,
    Sequence,
    Stage,
    Trigger,
    Event,
    Procedure,
    Extension,
    User,
    Role,
    Constraint,
    Partition,
    PrepareStatement,
    Pragma,
    Policy,
    Variable,
}

pub fn in_pos(
    pos: sqlparser::tokenizer::Location, t: sqlparser::tokenizer::Location, len: impl Into<u64>,
) -> bool {
    pos.line == t.line && (pos.column >= t.column && pos.column <= (t.column + len.into()))
}

#[async_trait::async_trait]
pub trait LexerCompletionTrait {
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
            if in_pos(pos, t.span.start, pattern.len() as u64) {
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
                    desc: None,
                })
                .collect::<Vec<_>>(),
        );
        ret.extend(
            Self::BUILD_IN_FUNCTIONS
                .iter()
                .map(|x| CompletionItem {
                    item: Cow::Borrowed(*x),
                    kind: CompletionKind::Function,
                    desc: None,
                })
                .collect::<Vec<_>>(),
        );
        ret.extend(
            Self::TYPES
                .iter()
                .map(|x| CompletionItem {
                    item: Cow::Borrowed(*x),
                    kind: CompletionKind::Type,
                    desc: None,
                })
                .collect::<Vec<_>>(),
        );
        ret.extend(
            Self::TEXTS
                .iter()
                .map(|x| CompletionItem {
                    item: Cow::Borrowed(*x),
                    kind: CompletionKind::Text,
                    desc: None,
                })
                .collect::<Vec<_>>(),
        );
        ret.extend(self.extra_completion_item().await.into_iter());
        ret
    }

    fn monaco_editor_keyword_regexp(&self) -> String {
        let combined: Vec<&'static str> = [
            Self::KEY_WORDS,
            Self::TYPES,
            Self::TEXTS,
            Self::BUILD_IN_FUNCTIONS,
        ]
        .concat();

        let s = monaco_editor_keyword_regexp(&combined[..]);
        return s;
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

#[async_trait::async_trait]
pub trait CompletionOnSynTaxHelper: Send + Sync + 'static + BackendName {
    async fn list_table(
        &self, db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>>;
    async fn list_view(
        &self, db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>>;
    async fn list_column(
        &self, db: Option<&str>, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>>;
    async fn list_function(
        &self, db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<CompletionItem>>;

    async fn list_procedure(
        &self, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        not_support_features(self.backend_name())
    }

    async fn list_index(&self) -> anyhow::Result<Vec<String>>;

    /*
    postgres use `show all`
    mysql use `show variables`
    to query all variables
     */
    async fn list_variable(&self) -> anyhow::Result<Vec<String>> {
        not_support_features(self.backend_name())
    }
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        not_support_features(self.backend_name())
    }
    async fn list_partition(
        &self, _db: Option<&str>, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_features(self.backend_name())
    }
    async fn postgres_table_policy(
        &self, _db: Option<&str>, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_features(self.backend_name())
    }
    async fn list_schema(&self, _db: Option<&str>) -> anyhow::Result<Vec<String>> {
        not_support_features(self.backend_name())
    }
    async fn list_role(&self) -> anyhow::Result<Vec<String>> {
        not_support_features(self.backend_name())
    }
    async fn list_postgres_rule(
        &self, _db: Option<&str>, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_features(self.backend_name())
    }
    async fn list_constraint(
        &self, _db: Option<&str>, _schema: Option<&str>, _table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_features(self.backend_name())
    }
    async fn list_table_trigger(
        &self, _db: Option<&str>, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_features(self.backend_name())
    }
    async fn list_prepare_name(&self, _query_id: uuid::Uuid) -> anyhow::Result<Vec<String>> {
        not_support_features(self.backend_name())
    }

    fn sqlite_pragma(&self) -> Vec<CompletionItem> {
        vec![]
    }
}

pub(crate) async fn try_completion_on_syntax<T: sqlparser::dialect::Dialect + Send + Sync>(
    sql: &str, query_id: uuid::Uuid, line: u32, col: u32, dialet: T,
    helper: Arc<dyn CompletionOnSynTaxHelper>,
) -> anyhow::Result<Vec<CompletionItem>> {
    let loc = Location {
        line: line as u64,
        column: col as u64,
    };

    let statements = tokio::task::block_in_place(|| -> anyhow::Result<Vec<Statement>> {
        let parser = sqlparser::parser::Parser::new(&dialet);
        let mut parser = parser.try_with_sql(sql)?;
        let statements: Vec<Statement> = parser.parse_statements()?;
        anyhow::Ok(statements)
    })?;

    for statement in statements {
        log::trace!("completion on ast:{:?}", statement);
        let mut c = CompletionStament::new(loc, helper.clone(), query_id);
        c.completion_on_statement(&statement).await;
        if c.finished() {
            let m = c.result.unwrap_or_default();
            return anyhow::Ok(m);
        }
    }
    anyhow::bail!("can't find completion.")
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
                desc: None,
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

    pub(crate) async fn test_file_auto_completion(
        content: &str, db: &dyn AutoCompletionTrait, id: uuid::Uuid, use_lexer: bool,
    ) {
        let items = parse_auto_completion_test_file(&content);
        for item in items.iter() {
            // log::trace!("########completion on {:?}", item.sql);
            for i in item.tests.iter() {
                log::trace!("on pos:{}", i.0);
                let items = db
                    .auto_completion(id, &item.sql, 1, i.0 as u32, use_lexer)
                    .await
                    .unwrap();
                for should in i.1.iter() {
                    test_completion_items_contains(&items[..], &should);
                }
            }
        }
    }
    //
}
