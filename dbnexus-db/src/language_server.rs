use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::Arc;

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use sqlparser::ast::Statement;
use sqlparser::tokenizer::{Location, Token};
use typescript_type_def::TypeDef;

use crate::db::{BackendName, FormatSqlOpts, monaco_editor_keyword_regexp, not_support_features};
use crate::sqlparser_completion::CompletionStament;

#[derive(Debug, Clone, Copy, enum_iterator::Sequence)]
pub enum TokenModifier {
    Declaration,
}

impl Into<u32> for TokenModifier {
    fn into(self) -> u32 {
        self as u32
    }
}

impl TokenModifier {
    #[allow(dead_code)] // actual used in test.
    fn to_static_str(self) -> &'static str {
        match self {
            Self::Declaration => "declaration",
        }
    }
}

impl std::fmt::Display for TokenModifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_static_str())
    }
}

#[async_trait::async_trait]
pub trait LanguageServer: BackendName {
    async fn auto_completion(
        &self, query_id: uuid::Uuid, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>>;
    async fn senmatic_tokens(&self, _sql: &str) -> anyhow::Result<Vec<SemanticToken>> {
        // not used but still can be used in the future
        anyhow::Ok(vec![])
    }
    fn format_sql(&self, _sql: &str, _ops: Option<FormatSqlOpts>) -> anyhow::Result<String> {
        not_support_features(self.backend_name())
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, typescript_type_def::TypeDef, PartialOrd,
)]
pub struct SemanticToken {
    pub line: u32, // 1 based
    pub col: u32,  // 1 based
    length: u32,
    token_type: u32,
    token_modifiers: Option<u32>,
}

#[allow(unused)]
impl SemanticToken {
    pub(crate) fn new(
        line: u32, col: u32, length: u32, token_type: u32, token_modifiers: impl Into<Option<u32>>,
    ) -> Self {
        Self {
            line,
            col,
            length,
            token_type,
            token_modifiers: token_modifiers.into(),
        }
    }
}

impl Ord for SemanticToken {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let o = self.line.cmp(&other.line);
        if o == Ordering::Equal {
            self.col.cmp(&other.col)
        } else {
            return o;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompletionItem {
    pub item: Cow<'static, str>,
    pub kind: CompletionKind,
    pub desc: Option<Cow<'static, str>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, TypeDef, enum_iterator::Sequence)]
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
    Rule,
    Cluster,
}

impl Into<u32> for CompletionKind {
    fn into(self) -> u32 {
        self as u32
    }
}
impl std::fmt::Display for CompletionKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let x = format!("{:?}", self).to_lowercase();
        write!(f, "{}", x)
    }
}

pub fn in_pos(
    pos: sqlparser::tokenizer::Location, t: sqlparser::tokenizer::Location, len: impl Into<u64>,
) -> bool {
    pos.line == t.line && (pos.column >= t.column && pos.column <= (t.column + len.into()))
}

#[async_trait::async_trait]
pub trait LexerCompletionTrait {
    type Dialect: sqlparser::dialect::Dialect + Sync + Send + 'static;
    const KEY_WORDS: &'static [&'static str];
    const TYPES: &'static [&'static str];
    const TEXTS: &'static [&'static str] = &[];

    async fn extra_completion_item(&self) -> Vec<CompletionItem> {
        vec![]
    }

    // some backend can use a query to query this.
    async fn get_key_words_fn(&self) -> anyhow::Result<Vec<CompletionItem>> {
        anyhow::Ok(vec![])
    }

    fn consider_keywords() -> HashSet<String> {
        Self::KEY_WORDS
            .iter()
            .chain(Self::TYPES.iter())
            .chain(Self::TEXTS.iter())
            .map(|x| x.to_uppercase())
            .collect()
    }

    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>>;

    // table use very often.
    // if there is some error
    // we still completion on table.
    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>>;

    async fn do_auto_completion(
        &self, sql: &str, dialect: &Self::Dialect, pos: sqlparser::tokenizer::Location,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let tokens = sqlparser::tokenizer::Tokenizer::new(dialect, sql)
            .tokenize_with_location()
            .unwrap_or(vec![]);
        let all_item = self.all_completion_items().await?;
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

    async fn all_completion_items(&self) -> anyhow::Result<Vec<CompletionItem>> {
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
        ret.extend(self.get_key_words_fn().await?.into_iter());
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
        ret.extend(
            self.list_table_or_view()
                .await?
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Table,
                    desc: None,
                }),
        );
        ret.extend(self.list_function().await?.into_iter());

        anyhow::Ok(ret)
    }

    fn monaco_editor_keyword_regexp() -> String {
        let combined: Vec<&'static str> = [Self::KEY_WORDS, Self::TYPES, Self::TEXTS].concat();
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
pub trait AstCompletionHelper: Send + Sync + 'static + BackendName {
    async fn list_table(
        &self, db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>>;
    async fn list_view(
        &self, db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>>;
    async fn list_column(
        &self, db: Option<&str>, schema: Option<&str>, name: &str,
    ) -> anyhow::Result<Vec<CompletionItem>>;
    async fn list_function(
        &self, db: Option<&str>, schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>>;

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>>;

    async fn list_procedure(
        &self, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        not_support_features(self.backend_name())
    }

    async fn mysql_show_create_trigger(&self) -> anyhow::Result<Vec<CompletionItem>> {
        anyhow::Ok(vec![])
    }

    async fn list_cluster(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }

    // postgres use `show all`
    // mysql use `show variables`
    // to query all variables
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
    helper: Arc<dyn AstCompletionHelper>,
) -> anyhow::Result<Vec<CompletionItem>> {
    let loc = Location {
        line: line as u64,
        column: col as u64,
    };

    let statements = tokio::task::block_in_place(|| -> anyhow::Result<Vec<Statement>> {
        let parser = sqlparser::parser::Parser::new(&dialet);
        let mut parser = parser.try_with_sql(sql)?;
        let mut statements = Vec::new();
        while parser.peek_token().token != Token::EOF {
            while parser.peek_token().token == Token::SemiColon {
                parser.next_token();
            }
            match parser.parse_statement() {
                Ok(s) => {
                    statements.push(s);
                }
                Err(err) => {
                    // here is very fine.no need to break
                    log::error!("parse sql error:{}", err);
                    // we better skip to next to avoid infinit loop
                    parser.next_token();
                }
            }
        }
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

    use super::{CompletionItem, LanguageServer};

    /// test if completion items contains a centains string
    /// use in test to see if completion is ok.
    /// this fn not returns just panic if not contains.
    pub fn test_completion_items_contains(items: &[CompletionItem], line: usize, name: &str) {
        assert!(
            items.iter().any(|x| x.item.eq_ignore_ascii_case(name)),
            "line:{} items:{:?},expect:{:?}",
            line,
            items,
            name
        )
    }

    #[test]
    fn test_test_completion_items_contains() {
        use super::CompletionItem;
        use crate::language_server::CompletionKind;

        test_completion_items_contains(
            &[CompletionItem {
                item: "aaa".into(),
                kind: CompletionKind::Text,
                desc: None,
            }],
            1,
            "aaa",
        );
    }
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub(crate) struct CompletionOneTest {
        pub(crate) sql: String,
        pub(crate) tests: Vec<(u64, Vec<String>)>,
        pub(crate) line: usize,
    }

    pub(crate) fn parse_auto_completion_test_file(s: &str) -> Vec<CompletionOneTest> {
        let x: Vec<_> = s.lines().collect();
        x.chunks(2)
            .enumerate()
            .map(|(index, x)| {
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
                    line: (index + 1) * 2,
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

        assert_eq!(x, vec![
            CompletionOneTest {
                sql: "append str".to_string(),
                tests: vec![
                    (1, vec!["append".to_string()]),
                    (11, vec!["str".to_string()])
                ],
                line: 2
            },
            CompletionOneTest {
                sql: "select xxxx".to_string(),
                tests: vec![
                    (1, vec!["select".to_string()]),
                    (4, vec!["xxxx".to_string()])
                ],
                line: 4
            }
        ]);
    }

    pub(crate) async fn test_file_auto_completion(
        content: &str, db: &dyn LanguageServer, id: uuid::Uuid, use_lexer: bool,
    ) {
        let items = parse_auto_completion_test_file(&content);
        for item in items.iter() {
            for i in item.tests.iter() {
                log::trace!("on pos:{}", i.0);
                let items = db
                    .auto_completion(id, &item.sql, 1, i.0 as u32, use_lexer)
                    .await
                    .unwrap();
                for should in i.1.iter() {
                    test_completion_items_contains(&items[..], item.line, &should);
                }
            }
        }
    }
    //
}
