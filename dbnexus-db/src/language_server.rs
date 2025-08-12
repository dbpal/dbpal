use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use sqlparser::ast::Statement;
use sqlparser::tokenizer::{Location, Token, TokenWithSpan};
use tokio::task::block_in_place;
use typescript_type_def::TypeDef;

use crate::db::{FormatSqlOpts, GetBackendKind, not_support_feature};
use crate::sqlparser_completion::{CompletionStatement, TempTableColumn};

#[async_trait::async_trait]
pub trait LanguageServer: GetBackendKind {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>>;

    fn format_sql(&self, _sql: &str, _ops: Option<FormatSqlOpts>) -> anyhow::Result<String> {
        not_support_feature(self.backend_name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompletionItem {
    pub item: Cow<'static, str>,
    pub kind: CompletionKind,
    pub desc: Option<Cow<'static, str>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, TypeDef)]
pub enum CompletionKind {
    Command, // commnad right now use by redis
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
    ClickHouseTableProjection,
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

pub(crate) fn remove_dupliate(items: Vec<CompletionItem>) -> Vec<CompletionItem> {
    items
        .into_iter()
        .map(CompletionItemNameHasher::from)
        .collect::<HashSet<_>>()
        .into_iter()
        .map(|x| x.into())
        .collect::<Vec<_>>()
}

pub struct CompletionItemNameHasher {
    pub(crate) item: CompletionItem,
}
impl From<CompletionItemNameHasher> for CompletionItem {
    fn from(value: CompletionItemNameHasher) -> Self {
        value.item
    }
}
impl std::hash::Hash for CompletionItemNameHasher {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.item.item.hash(state);
    }
}

impl std::cmp::Eq for CompletionItemNameHasher {}
impl std::cmp::PartialEq for CompletionItemNameHasher {
    fn eq(&self, other: &Self) -> bool {
        self.item.item == other.item.item
    }
}

impl From<CompletionItem> for CompletionItemNameHasher {
    fn from(item: CompletionItem) -> Self {
        Self { item }
    }
}

#[async_trait::async_trait]
pub trait LexerCompletionTrait: LangServerCommon {
    type Dialect: sqlparser::dialect::Dialect + Sync + Send + 'static;

    const TYPES: &'static [&'static str];
    const TEXTS: &'static [&'static str] = &[];

    async fn extra_completion_item(&self) -> Vec<CompletionItem> {
        vec![]
    }

    async fn list_columns(&self, table_name: &str) -> anyhow::Result<Vec<TempTableColumn>>;

    // some backend can use a query to query this.
    async fn get_key_words_fn(&self) -> anyhow::Result<Vec<CompletionItem>>;

    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>>;

    // table use very often.
    // if there is some error
    // we still completion on table.
    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>>;

    async fn list_procedure(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }

    async fn do_auto_completion(
        &self, sql: &str, dialect: &Self::Dialect,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let tokens = sqlparser::tokenizer::Tokenizer::new(dialect, sql)
            .tokenize_with_location()
            .unwrap_or(vec![]);
        let mut all_item = self.all_completion_items().await?;
        all_item.extend(self.get_table_columns_if_possible(&tokens[..]).await);
        return anyhow::Ok(all_item);
    }

    async fn all_completion_items(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut ret = Vec::new();
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

        ret.extend(
            self.list_database()
                .await?
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Database,
                    desc: None,
                }),
        );
        ret.extend(
            self.list_schema(None)
                .await?
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Schema,
                    desc: None,
                }),
        );
        ret.extend(
            self.list_procedure()
                .await?
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Procedure,
                    desc: None,
                }),
        );
        anyhow::Ok(ret)
    }

    async fn get_table_columns_if_possible(&self, tokens: &[TokenWithSpan]) -> Vec<CompletionItem> {
        let tables = self
            .list_table_or_view()
            .await
            .map_err(|e| log::error!("list table or view failed,err:{}", e))
            .ok()
            .unwrap_or_default()
            .into_iter()
            .map(|x: String| x.to_lowercase())
            .collect::<HashSet<_>>();
        let mut table_hits = HashSet::new();
        for t in tokens.iter() {
            match &t.token {
                Token::Word(x) => {
                    // lowcase match is ok for me
                    // we just need make sure this table is in the database(not exactly)
                    let name = x.value.to_lowercase();
                    if tables.contains(&name) {
                        table_hits.insert(x.value.clone());
                    }
                }
                _ => {}
            }
        }
        let mut ret = vec![];
        for t in table_hits.iter() {
            let columns = self
                .list_columns(t)
                .await
                .map_err(|e| log::error!("list columns failed,err:{}", e))
                .unwrap_or_default();
            ret.extend(columns.into_iter().map(CompletionItem::from));
        }

        remove_dupliate(ret)
    }
}

#[async_trait::async_trait]

pub trait LangServerCommon: GetBackendKind {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
    async fn list_schema(&self, _db: Option<&str>) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
}
#[async_trait::async_trait]
pub trait AstCompletionHelper: Send + Sync + 'static + GetBackendKind + LangServerCommon {
    async fn list_table(
        &self, db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>>;
    async fn list_view(
        &self, db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>>;
    async fn list_column(
        &self, db: Option<&str>, schema: Option<&str>, name: &str, is_call: bool,
    ) -> anyhow::Result<Vec<TempTableColumn>>;

    async fn list_function(
        &self, db: Option<&str>, schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>>;

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>>;

    async fn get_query_columns(&self, _sql: &str) -> anyhow::Result<Vec<TempTableColumn>>;

    async fn duckdb_extensions(
        &self, _installed: Option<bool>,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        anyhow::Ok(vec![])
    }

    async fn list_procedure(
        &self, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn list_clickhouse_table_projects(
        &self, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }

    async fn keywords(&self) -> anyhow::Result<Vec<CompletionItem>> {
        not_support_feature(self.backend_name())
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
        not_support_feature(self.backend_name())
    }

    async fn list_partition(
        &self, _db: Option<&str>, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn postgres_table_policy(
        &self, _db: Option<&str>, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }

    async fn list_role(&self) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn list_postgres_rule(
        &self, _db: Option<&str>, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn list_constraint(
        &self, _db: Option<&str>, _schema: Option<&str>, _table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn list_table_trigger(
        &self, _db: Option<&str>, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn list_prepare_name(&self, _query_id: uuid::Uuid) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    fn sqlite_pragma(&self) -> Vec<CompletionItem> {
        vec![]
    }
}

pub(crate) async fn try_completion_on_syntax<T: sqlparser::dialect::Dialect + Send + Sync>(
    sql: &str, line: u32, col: u32, dialet: T, helper: Arc<dyn AstCompletionHelper>,
) -> anyhow::Result<Vec<CompletionItem>> {
    let loc = Location {
        line: line as u64,
        column: col as u64,
    };

    let statements = block_in_place(|| -> anyhow::Result<Vec<Statement>> {
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
        let mut c = CompletionStatement::new(loc, helper.clone());
        c.completion_on_statement(&statement).await;
        if c.finished() {
            let m = c.result.unwrap_or_default();
            return anyhow::Ok(m);
        }
    }
    let tokens: Vec<_> = sqlparser::tokenizer::Tokenizer::new(&dialet, sql)
        .tokenize_with_location()
        .unwrap_or_default()
        .into_iter()
        .filter(|x| match &x.token {
            Token::Whitespace(_) => false,
            _ => true,
        })
        .collect();
    for (index, t) in tokens.iter().enumerate() {
        if index == 0 {
            continue;
        }
        match (&t.token, tokens.get(index - 1).map(|x| &x.token)) {
            (Token::Period, Some(Token::Word(x))) => {
                if in_pos(loc, t.span.start, 1u64) {
                    //matched a `aaa.`
                    let schema = &x.value;
                    let mut ret = vec![];
                    ret.extend(
                        helper
                            .list_table(None, Some(&schema))
                            .await
                            .map_err(|e| log::error!("list table failed,err:{}", e))
                            .unwrap_or_default()
                            .into_iter()
                            .map(|x| CompletionItem {
                                item: x.into(),
                                kind: CompletionKind::Table,
                                desc: None,
                            }),
                    );
                    ret.extend(
                        helper
                            .list_view(None, Some(&schema))
                            .await
                            .map_err(|e| log::error!("list view failed,err:{}", e))
                            .unwrap_or_default()
                            .into_iter()
                            .map(|x| CompletionItem {
                                item: x.into(),
                                kind: CompletionKind::View,
                                desc: None,
                            }),
                    );
                    ret.extend(
                        helper
                            .list_function(None, Some(&schema), false)
                            .await
                            .map_err(|e| log::error!("list function failed,err:{}", e))
                            .unwrap_or_default()
                            .into_iter(),
                    );
                    ret.extend(
                        helper
                            .list_procedure(None, Some(&schema))
                            .await
                            .map_err(|e| log::error!("list procedure failed,err:{}", e))
                            .unwrap_or_default()
                            .into_iter()
                            .map(|x| CompletionItem {
                                item: x.into(),
                                kind: CompletionKind::Procedure,
                                desc: None,
                            }),
                    );
                    return anyhow::Ok(ret);
                }
            }
            _ => {}
        }
    }

    anyhow::bail!("can't find completion.")
}

#[cfg(test)]
pub(crate) mod completion_test {

    use std::ops::Deref;

    use super::{CompletionItem, LanguageServer};

    /// test if completion items contains a centains string
    /// use in test to see if completion is ok.
    /// this fn not returns just panic if not contains.
    pub fn test_completion_items_contains(items: &[CompletionItem], line: usize, name: &str) {
        assert!(
            items.iter().any(|x| x.item.eq_ignore_ascii_case(name)),
            "line:{} items:{:?},expect:{:?}",
            line,
            {
                let items = items.iter().map(|x| x.item.deref()).collect::<Vec<_>>();
                items.join(",")
            },
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

        assert_eq!(
            x,
            vec![
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
            ]
        );
    }

    pub(crate) async fn test_file_auto_completion(
        content: &str, db: &dyn LanguageServer, use_lexer: bool,
    ) {
        let items = parse_auto_completion_test_file(&content);
        for item in items.iter() {
            for i in item.tests.iter() {
                log::trace!("on pos:{}", i.0);
                let items = db
                    .auto_completion(&item.sql, 1, i.0 as u32, use_lexer)
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
