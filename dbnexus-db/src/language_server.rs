use std::borrow::Cow;
use std::collections::HashSet;
use std::time::Duration;

use sqlparser::ast::{SetExpr, Statement};
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Location, Token, TokenWithSpan};
use tokio::task::block_in_place;
use typescript_type_def::TypeDef;

use crate::db::{
    BackendKind, CreatedTypeDef, GetBackendKind, SequenceDetail, SqlEvent, not_support_feature,
};
use crate::helper::try_remove_one_at;
use crate::sqlparser_completion::{CompletionStatement, Scopes, TempTableColumn};

#[async_trait::async_trait]
pub trait LanguageServer: GetBackendKind + GetKeyWords {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>>;

    fn parse_error(&self, _sql: &str) -> anyhow::Result<Option<ErrAndLoc>> {
        anyhow::Ok(None)
    }
    fn is_dangerous_sql(&self, _sql: &str) -> anyhow::Result<bool>;

    // if possible
    fn extract_table_select_selection(&self, sql: &str) -> anyhow::Result<Option<String>>;
    fn extract_table_select_sort(&self, sql: &str) -> anyhow::Result<Option<String>>;
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CompletionItem {
    pub item: Cow<'static, str>,
    pub kind: CompletionKind,
    pub desc: Option<Cow<'static, str>>,
}

impl From<CompletionItem> for String {
    fn from(value: CompletionItem) -> Self {
        value.item.into()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, TypeDef)]
pub enum CompletionKind {
    Command, // command right now use by redis
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
    Prepare,
    Module,
    Domain,
    Stream,
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

pub(crate) fn remove_duplicate(items: Vec<CompletionItem>) -> Vec<CompletionItem> {
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
    const TYPES: &'static [&'static str];
    const TEXTS: &'static [&'static str] = &[];

    async fn do_auto_completion(
        &self, sql: &str, line: u32, col: u32,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let pos = sqlparser::tokenizer::Location {
            line: line as u64,
            column: col as u64,
        };
        let tokens = sqlparser::tokenizer::Tokenizer::new(self.dialect().as_ref(), sql)
            .tokenize_with_location()
            .unwrap_or_default();
        if self.backend_name().support_at() {
            // 试图去匹配 @@
            for t in tokens.iter() {
                if let Token::Word(x) = &t.token {
                    if x.value.starts_with("@@") && in_pos(pos, t.span.start, x.value.len() as u64)
                    {
                        // 词法匹配到了@@
                        return anyhow::Ok(self.list_variable_completion().await);
                    }
                }
            }
        }
        let mut all_item = self.all_completion_items().await;
        all_item.extend(self.get_table_columns_if_possible(&tokens[..]).await);
        if !self.backend_name().support_at() {
            // this is not access var via @@ we better completion all variable
            all_item.extend(self.list_variable_completion().await);
        }
        // 推送词法部分
        for t in tokens.iter() {
            if let Token::Word(x) = &t.token {
                if x.keyword == sqlparser::keywords::Keyword::NoKeyword
                    && !in_pos(pos, t.span.start, x.value.len() as u64)
                {
                    let mut value = x.value.clone();
                    //@my_var 会被sqlparser解析成一个标识符
                    if self.backend_name().support_at() {
                        value = try_remove_one_at(value);
                    }
                    all_item.push(CompletionItem {
                        item: value.into(),
                        kind: CompletionKind::Text,
                        desc: None,
                    });
                }
            }
        }
        return anyhow::Ok(remove_duplicate(all_item));
    }

    async fn all_completion_items(&self) -> Vec<CompletionItem> {
        let mut ret = Vec::new();
        ret.extend(self.key_words_completion().await);
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
        ret.extend(
            self.list_table(None)
                .await
                .map_err(|x| log::error!("list table failed,err:{}", x))
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Table,
                    desc: None,
                }),
        );
        ret.extend(
            self.list_view(None)
                .await
                .map_err(|x| log::error!("list table failed,err:{}", x))
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Table,
                    desc: None,
                }),
        );
        ret.extend(
            self.list_function(None, true)
                .await
                .map_err(|err| log::error!("list function failed,err:{}", err))
                .unwrap_or_default(),
        );
        ret.extend(self.list_schema_completion().await);
        ret.extend(
            self.list_procedure(None)
                .await
                .map_err(|err| log::error!("list procedure failed,err:{}", err))
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Procedure,
                    desc: None,
                }),
        );
        ret.extend(
            self.sqlite_pragma()
                .await
                .map_err(|err| log::error!("list sqlite pragma,err:{}", err))
                .unwrap_or_default(),
        );
        ret.extend(
            self.sqlite_module_name()
                .await
                .map_err(|err| log::error!("list sqlite pragma,err:{}", err))
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Module,
                    desc: None,
                }),
        );
        ret.extend(
            self.clickhouse_format_clause()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::KeyWord,
                    desc: None,
                }),
        );

        // here mssql list_procedure and list_execute will have the same result.
        // will make duplicate object.
        if self.backend_name() != BackendKind::SqlServer {
            ret.extend(
                self.list_execute(None)
                    .await
                    .map_err(|err| log::error!("list execute failed,err:{}", err))
                    .unwrap_or_default()
                    .into_iter()
                    .map(CompletionItem::from),
            );
        }
        ret.extend(self.user_and_role().await);
        ret.extend(self.list_database_completion().await);
        ret.extend(self.list_sequence_completion(None).await);
        ret.extend(self.list_domain_completion(None).await);
        ret.extend(self.list_index_completion(None).await);
        ret
    }

    async fn get_table_columns_if_possible(&self, tokens: &[TokenWithSpan]) -> Vec<CompletionItem> {
        let mut tables: HashSet<String> = self
            .list_table(None)
            .await
            .map_err(|e| log::error!("list table or view failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|x: String| x.to_lowercase())
            .collect::<HashSet<_>>();

        let views = self
            .list_view(None)
            .await
            .map_err(|e| log::error!("list table or view failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|x: String| x.to_lowercase())
            .collect::<HashSet<_>>();
        tables.extend(views);

        let mut table_hits = HashSet::new();
        for t in tokens.iter() {
            if let Token::Word(x) = &t.token {
                // lowercase match is ok for me
                // we just need make sure this table is in the database(not exactly)
                let name = x.value.to_lowercase();
                if tables.contains(&name) {
                    table_hits.insert(x.value.clone());
                }
            }
        }
        let mut ret = vec![];
        for t in table_hits.iter() {
            let columns = self
                .list_column(None, t, None)
                .await
                .map_err(|e| log::error!("list columns failed,err:{}", e))
                .unwrap_or_default();
            ret.extend(columns.into_iter().map(CompletionItem::from));
        }

        remove_duplicate(ret)
    }
}

impl From<CreatedTypeDef> for CompletionItem {
    fn from(value: CreatedTypeDef) -> Self {
        let CreatedTypeDef { name, sql } = value;
        Self {
            item: name.into(),
            kind: CompletionKind::Type,
            desc: sql.map(|x| x.into()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct PostGresExtension {
    pub(crate) name: String,
    pub(crate) comment: Option<String>,
}

impl From<PostGresExtension> for CompletionItem {
    fn from(value: PostGresExtension) -> Self {
        let PostGresExtension { name, comment } = value;
        Self {
            item: name.into(),
            kind: CompletionKind::Extension,
            desc: comment.map(|x| x.into()),
        }
    }
}

impl From<SqlEvent> for CompletionItem {
    fn from(value: SqlEvent) -> Self {
        CompletionItem {
            item: value.name.into(),
            kind: CompletionKind::Event,
            desc: Some(value.sql.into()),
        }
    }
}

#[async_trait::async_trait]
pub trait GetKeyWords {
    async fn key_words(&self) -> anyhow::Result<Vec<String>>;
}

#[async_trait::async_trait]

pub trait LangServerCommon: GetBackendKind + GetKeyWords {
    async fn list_cluster(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
    fn clickhouse_format_clause(&self) -> Vec<String> {
        vec![]
    }

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>>;

    // postgres use `show all`
    // mysql use `show variables`
    // to query all variables
    async fn list_variable(&self) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn list_event(&self, _schema: Option<&str>) -> anyhow::Result<Vec<SqlEvent>> {
        anyhow::Ok(vec![])
    }
    async fn sqlite_module_name(&self) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn sqlite_pragma(&self) -> anyhow::Result<Vec<CompletionItem>> {
        anyhow::Ok(vec![])
    }
    // some backend user and role can have the same name
    async fn user_and_role(&self) -> Vec<CompletionItem> {
        let mut ret: Vec<CompletionItem> = self
            .list_user()
            .await
            .map_err(|e| log::error!("list user failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|x| CompletionItem {
                item: x.into(),
                kind: CompletionKind::User,
                desc: None,
            })
            .collect();

        ret.extend(
            self.list_role()
                .await
                .map_err(|e| log::error!("list user failed,err:{}", e))
                .unwrap_or_default()
                .into_iter()
                .map(|x| CompletionItem {
                    item: x.into(),
                    kind: CompletionKind::Role,
                    desc: None,
                }),
        );
        remove_duplicate(ret)
    }
    async fn list_role(&self) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn list_user(&self) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    /*
    -- 标准 SQL（PostgreSQL、Oracle、DB2 支持；MySQL 没有）
    CREATE DOMAIN us_postal_code AS CHAR(5)
        CHECK (
            VALUE ~ '^[0-9]{5}$'          -- 必须是 5 位数字
        );

    -- 以后任何表都能直接用
    CREATE TABLE addr (
        zip us_postal_code              -- 复用约束
    );
     */
    async fn list_domain(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }

    fn is_dangerous_sql_by_parser(&self, sql: &str) -> anyhow::Result<bool> {
        let ss = self.parse_statements(sql)?;

        for s in ss.iter() {
            let x = match s {
                Statement::Truncate { .. } => true,
                Statement::Drop { .. } => true,
                Statement::DropFunction { .. } => true,
                Statement::DropSecret { .. } => true,
                Statement::DropTrigger { .. } => true,
                Statement::DropDomain(..) => true,
                Statement::DropProcedure { .. } => true,
                Statement::DropPolicy { .. } => true,
                Statement::DropExtension { .. } => true,
                Statement::Discard { .. } => true,
                Statement::DropConnector { .. } => true,
                Statement::Delete(x) => x.selection.is_none(),
                Statement::Update { selection, .. } => selection.is_none(),
                _ => false,
            };
            if x {
                return anyhow::Ok(true);
            }
        }
        anyhow::Ok(false)
    }

    fn parse_statements(&self, sql: &str) -> anyhow::Result<Vec<sqlparser::ast::Statement>> {
        let dialect = self.dialect();
        let ss = sqlparser::parser::Parser::parse_sql(dialect.as_ref(), sql)?;
        anyhow::Ok(ss)
    }

    // list user created types.
    async fn list_created_type(
        &self, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<CreatedTypeDef>> {
        anyhow::Ok(vec![])
    }
    async fn list_sequence(&self, _schema: Option<&str>) -> anyhow::Result<Vec<SequenceDetail>> {
        anyhow::Ok(vec![])
    }

    fn parse_error(&self, sql: &str) -> anyhow::Result<Option<ErrAndLoc>> {
        let dialect = self.dialect();
        let x = sqlparser::parser::Parser::parse_sql(dialect.as_ref(), sql);
        anyhow::Ok(match x {
            Ok(_) => None,
            Err(x) => Some(ErrAndLoc::from(x)),
        })
    }

    async fn hit_schema(&self, name: &str) -> bool {
        self.list_schema()
            .await
            .map_err(|e| log::error!("list schema error: {}", e))
            .unwrap_or_default()
            .iter()
            .any(|x| x.eq_ignore_ascii_case(name))
    }
    async fn hit_table(&self, schema: Option<&str>, name: &str) -> bool {
        let tables = self
            .list_table(schema)
            .await
            .map_err(|e| log::error!("list table failed,err:{}", e))
            .unwrap_or_default();
        let views = self
            .list_view(schema)
            .await
            .map_err(|e| log::error!("list view failed,err:{}", e))
            .unwrap_or_default();

        tables
            .iter()
            .chain(views.iter())
            .any(|x| x.eq_ignore_ascii_case(name))
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
    async fn list_execute(&self, _schema: Option<&str>) -> anyhow::Result<Vec<DBExecutable>> {
        anyhow::Ok(vec![])
    }
    async fn list_execute_completion(&self, schema: Option<&str>) -> Vec<CompletionItem> {
        self.list_execute(schema)
            .await
            .map_err(|e| log::error!("list prepare failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(CompletionItem::from)
            .collect()
    }

    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync>;
    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>>;
    // list view or materialized view
    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>>;

    async fn list_function(
        &self, schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>>;
    async fn list_procedure(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn list_procedure_completion(&self, schema: Option<&str>) -> Vec<CompletionItem> {
        self.list_procedure(schema)
            .await
            .map_err(|e| {
                log::error!("list procedure error:{:?}", e);
            })
            .ok()
            .unwrap_or_default()
            .into_iter()
            .map(|x| CompletionItem {
                kind: CompletionKind::Procedure,
                item: x.into(),
                desc: None,
            })
            .collect::<Vec<_>>()
    }

    async fn list_column(
        &self, schema: Option<&str>, name: &str, is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>>;
    async fn list_column_oked(
        &self, schema: Option<&str>, name: &str, is_call: Option<Vec<String>>,
    ) -> Vec<TempTableColumn> {
        self.list_column(schema, name, is_call)
            .await
            .map_err(|e| log::error!("list column error: {}", e))
            .unwrap_or_default()
    }
    // completion on ast struct like `SCHEMA.XXXX`  , 'TABLE.XXXX'
    async fn try_completion_dot(
        &self, sql: &str, line: u32, col: u32,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let loc = sqlparser::tokenizer::Location {
            line: line as u64,
            column: col as u64,
        };
        let tokens: Vec<_> = sqlparser::tokenizer::Tokenizer::new(self.dialect().as_ref(), sql)
            .tokenize_with_location()
            .unwrap_or_default()
            .into_iter()
            .filter(|x| !matches!(&x.token, Token::Whitespace(_)))
            .collect();
        for (index, t) in tokens.iter().enumerate() {
            if index == 0 {
                continue;
            }
            if let (Token::Period, Some(Token::Word(x))) =
                (&t.token, tokens.get(index - 1).map(|x| &x.token))
            {
                if !in_pos(loc, t.span.start, 1u64) {
                    continue;
                }
                // hit position
                // matched a `aaa.`
                let schema = &x.value;
                let mut ret = vec![];

                if self.hit_schema(schema).await {
                    // try completion on schema.xxx syntax
                    ret.extend(
                        self.list_table(Some(schema))
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
                        self.list_view(Some(schema))
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
                        self.list_function(Some(schema), false)
                            .await
                            .map_err(|e| log::error!("list function failed,err:{}", e))
                            .unwrap_or_default()
                            .into_iter(),
                    );
                    ret.extend(
                        self.list_procedure(Some(schema))
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
                    // sql server will have the same result
                    if self.backend_name() != BackendKind::SqlServer {
                        ret.extend(
                            self.list_execute(Some(schema))
                                .await
                                .map_err(|e| log::error!("list procedure failed,err:{}", e))
                                .unwrap_or_default()
                                .into_iter()
                                .map(|x| CompletionItem {
                                    item: x.name.into(),
                                    kind: CompletionKind::Function,
                                    desc: x.statement.map(|x| x.into()),
                                }),
                        );
                    }

                    return anyhow::Ok(ret);
                }

                {
                    let table = schema;
                    let schema = if index >= 3 {
                        match (tokens.get(index - 3), tokens.get(index - 2)) {
                            (Some(schema), Some(dot)) => match (&schema.token, &dot.token) {
                                (Token::Word(x), Token::Period) => Some(x.value.as_str()),
                                _ => None,
                            },
                            _ => None,
                        }
                    } else {
                        None
                    };

                    if self.hit_table(schema, table).await {
                        // try a TABLE.xxxx
                        // right now schema is table name
                        // we hope the use not use table alias
                        log::trace!("hit table or view '{}' trying to completion columns", table);
                        ret.extend(
                            self.list_column(schema, table, None)
                                .await
                                .map_err(|e| log::error!("list column failed: {}", e))
                                .unwrap_or_default()
                                .into_iter()
                                .map(CompletionItem::from),
                        );
                    }
                }
                return anyhow::Ok(ret);
            }
        }
        anyhow::bail!("can't find completion");
    }

    async fn key_words_completion(&self) -> Vec<CompletionItem> {
        self.key_words()
            .await
            .map_err(|e| log::error!("get keywords failed,e:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|name| CompletionItem {
                item: name.into(),
                kind: CompletionKind::KeyWord,
                desc: None,
            })
            .collect()
    }
    async fn list_role_completion(&self) -> Vec<CompletionItem> {
        self.list_role()
            .await
            .map_err(|e| log::error!("list role failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|name| CompletionItem {
                item: name.into(),
                kind: CompletionKind::User,
                desc: None,
            })
            .collect()
    }
    async fn list_user_completion(&self) -> Vec<CompletionItem> {
        self.list_user()
            .await
            .map_err(|e| log::error!("list user failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|name| CompletionItem {
                item: name.into(),
                kind: CompletionKind::User,
                desc: None,
            })
            .collect()
    }
    async fn list_schema_completion(&self) -> Vec<CompletionItem> {
        self.list_schema()
            .await
            .map_err(|e| log::error!("list schema failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|name| CompletionItem {
                item: name.into(),
                kind: CompletionKind::Schema,
                desc: None,
            })
            .collect()
    }

    async fn list_database_completion(&self) -> Vec<CompletionItem> {
        self.list_database()
            .await
            .map_err(|e| log::error!("list database failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|name| CompletionItem {
                item: name.into(),
                kind: CompletionKind::Database,
                desc: None,
            })
            .collect()
    }

    async fn list_database_schema_completion(&self) -> Vec<CompletionItem> {
        let mut results = self.list_schema_completion().await;
        results.extend(self.list_database_completion().await);
        results
    }
    async fn list_variable_completion(&self) -> Vec<CompletionItem> {
        self.list_variable()
            .await
            .map_err(|e| log::error!("list variable failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|name| CompletionItem {
                item: name.into(),
                kind: CompletionKind::Variable,
                desc: None,
            })
            .collect()
    }
    async fn list_event_completion(&self, schema: Option<&str>) -> Vec<CompletionItem> {
        self.list_event(schema)
            .await
            .map_err(|e| log::error!("list event failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(CompletionItem::from)
            .collect()
    }
    async fn list_sequence_completion(&self, schema: Option<&str>) -> Vec<CompletionItem> {
        self.list_sequence(schema)
            .await
            .map_err(|e| log::error!("list sequence failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(CompletionItem::from)
            .collect()
    }
    async fn list_domain_completion(&self, schema: Option<&str>) -> Vec<CompletionItem> {
        self.list_domain(schema)
            .await
            .map_err(|e| log::error!("list domain failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|x| CompletionItem {
                item: x.into(),
                kind: CompletionKind::Domain,
                desc: None,
            })
            .collect()
    }
    async fn list_index_completion(&self, schema: Option<&str>) -> Vec<CompletionItem> {
        self.list_index(schema)
            .await
            .map_err(|e| log::error!("list index failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|x| CompletionItem {
                item: x.into(),
                kind: CompletionKind::Index,
                desc: None,
            })
            .collect()
    }

    fn tokenize(&self, sql: &str, remove_white_space: bool) -> anyhow::Result<Vec<Token>> {
        let d = self.dialect();
        let tokens = sqlparser::tokenizer::Tokenizer::new(d.as_ref(), sql).tokenize()?;
        anyhow::Ok(if remove_white_space {
            tokens
                .into_iter()
                .filter(|x| !matches!(x, Token::Whitespace(_)))
                .collect()
        } else {
            tokens
        })
    }

    // extract where condition from `select * from table where a =1`;
    // if possible
    fn extract_table_select_selection(&self, sql: &str) -> anyhow::Result<Option<String>> {
        let ss = self.parse_statements(sql).unwrap_or_default();
        if let Some(statement) = ss.get(0) {
            match statement {
                Statement::Query(query) => match query.body.as_ref() {
                    SetExpr::Select(x) => {
                        return anyhow::Ok(x.selection.as_ref().map(|x| x.to_string()));
                    }
                    _ => {
                        anyhow::bail!(format!(
                            "should be a select statement,but ${:?}",
                            query.body.as_ref()
                        ))
                    }
                },
                _ => {
                    anyhow::bail!(format!("should be a query statement ,but:{:?}", ss[0]))
                }
            }
        }
        // if parse failed
        let tokens: Vec<_> = self
            .tokenize(sql, true)?
            .into_iter()
            .filter(|x| !matches!(x, Token::Whitespace(_)))
            .collect();
        if let Some(index) = tokens.iter().position(|x| match x {
            Token::Word(x) => x.keyword == Keyword::WHERE,
            _ => false,
        }) {
            let token_slice = &tokens[index + 1..];
            anyhow::Ok(Some(
                token_slice
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(" "),
            ))
        } else {
            anyhow::Ok(None)
        }
    }

    fn extract_table_select_sort(&self, sql: &str) -> anyhow::Result<Option<String>> {
        let tokens: Vec<_> = self
            .tokenize(sql, true)?
            .into_iter()
            .filter(|x| !matches!(x, Token::Whitespace(_)))
            .collect();
        if let Some(index) = tokens.iter().position(|x| match x {
            Token::Word(x) => x.keyword == Keyword::ORDER,
            _ => false,
        }) {
            if tokens.get(index + 1).is_none() {
                anyhow::bail!("order by without 'by' keyword");
            }
            // skip order by
            let token_slice = &tokens[index + 2..];
            if token_slice.len() == 0 {
                return anyhow::Ok(None);
            }
            anyhow::Ok(Some(
                token_slice
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(" "),
            ))
        } else {
            anyhow::Ok(None)
        }
    }

    // https://github.com/launchbadge/sqlx/issues/4031
    // due to this issue , somebackend only allow use commit or rollback on ui
    fn contains_tx_operation_error(&self, sql: &str) -> anyhow::Result<()> {
        let ss = self.parse_statements(sql).unwrap();
        for s in ss.iter() {
            match s {
                Statement::Commit { .. }
                | Statement::Rollback { .. }
                | Statement::StartTransaction { .. } => {
                    anyhow::bail!("please use commit or rollback on UI")
                }
                _ => {}
            }
        }
        anyhow::Ok(())
    }
}

impl From<SequenceDetail> for CompletionItem {
    fn from(value: SequenceDetail) -> Self {
        let SequenceDetail { name, sql, .. } = value;

        CompletionItem {
            item: name.into(),
            kind: CompletionKind::Sequence,
            desc: Some(sql.into()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DBExecutable {
    pub(crate) name: String,
    pub(crate) statement: Option<String>,
}
impl From<DBExecutable> for CompletionItem {
    fn from(value: DBExecutable) -> Self {
        let DBExecutable { name, statement } = value;

        CompletionItem {
            item: name.into(),
            kind: CompletionKind::Prepare,
            desc: statement.map(|x| x.into()),
        }
    }
}

#[async_trait::async_trait]
pub trait AstCompletionHelper: Send + Sync + 'static + GetBackendKind + LangServerCommon {
    async fn postgres_list_extension(
        &self, _installed: bool,
    ) -> anyhow::Result<Vec<PostGresExtension>> {
        anyhow::Ok(vec![])
    }
    async fn get_query_columns(&self, _sql: &str) -> anyhow::Result<Vec<TempTableColumn>>;

    async fn duckdb_extensions(
        &self, _installed: Option<bool>,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        anyhow::Ok(vec![])
    }

    async fn list_clickhouse_table_projects(
        &self, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }

    async fn mysql_show_create_trigger(
        &self, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        anyhow::Ok(vec![])
    }

    async fn list_partition(
        &self, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn table_policy(
        &self, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn table_policy_completion(
        &self, schema: Option<&str>, table: &str,
    ) -> Vec<CompletionItem> {
        self.table_policy(schema, table)
            .await
            .map_err(|e| log::error!("list table policy failed,err:{}", e))
            .unwrap_or_default()
            .into_iter()
            .map(|x| CompletionItem {
                item: x.into(),
                kind: CompletionKind::Policy,
                desc: None,
            })
            .collect()
    }

    async fn list_postgres_rule(
        &self, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn list_constraint(
        &self, _schema: Option<&str>, _table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
    async fn list_table_trigger(
        &self, _schema: Option<&str>, _table: &str,
    ) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }
}

/// first try parser first
/// then a syntax like schema... table...
/// then lexer   
pub(crate) async fn try_parser_dot_lexer<T: LexerCompletionTrait + AstCompletionHelper>(
    backend: &T, sql: &str, line: u32, col: u32, use_lexer: bool,
) -> anyhow::Result<Vec<CompletionItem>> {
    let dot = backend.try_completion_dot(sql, line, col).await;
    let r = tokio::time::timeout(Duration::from_secs(4), async move {
        try_parser_completion(sql, line, col, backend).await
    })
    .await
    .inspect_err(|_| {
        log::error!("ast completion not finished in reasonable time this maybe a bug,sql:{} line:{} col:{} backend:{}" ,
        sql,line,col,backend.backend_name());
    })?;
    match r {
        Ok(mut xs) => {
            // xxx. 有可能和后面的元素结合产生不同寻常的结果
            // 有时候是关键字也会结合 这点一般的解析程序不太一样呢
            // 自动完成更多的给出可能性 但是有可能是错误的 问题不大
            xs.extend(dot.unwrap_or_default());
            return Ok(remove_duplicate(xs));
        }
        Err(err) => {
            log::error!("completion on ast failed,err:{}", err);
        }
    };
    // try dot
    if let Ok(x) = dot {
        return anyhow::Ok(x);
    }
    if !use_lexer {
        log::warn!("disabled use lexer , we will return empty data.");
        return anyhow::Ok(vec![]);
    }
    return LexerCompletionTrait::do_auto_completion(backend, sql, line, col).await;
}

pub(crate) async fn try_parser_completion(
    sql: &str, line: u32, col: u32, helper: &dyn AstCompletionHelper,
) -> anyhow::Result<Vec<CompletionItem>> {
    let loc = Location {
        line: line as u64,
        column: col as u64,
    };
    let d = helper.dialect();
    let statements = block_in_place(|| -> anyhow::Result<Vec<Statement>> {
        let parser = sqlparser::parser::Parser::new(d.as_ref());
        let mut parser = parser.try_with_sql(sql)?;
        let mut statements = Vec::new();
        while parser.peek_token().token != Token::EOF {
            while parser.peek_token().token == Token::SemiColon {
                parser.next_token();
            }
            if parser.peek_token().token == Token::EOF {
                break;
            }
            match parser.parse_statement() {
                Ok(s) => {
                    statements.push(s);
                }
                Err(err) => {
                    // here is very fine.no need to break
                    log::error!("parse sql error:{}", err);
                    // we better skip to next to avoid infinite loop
                    parser.next_token();
                }
            }
        }
        anyhow::Ok(statements)
    })?;

    let scopes = Scopes::new();

    for statement in statements {
        log::trace!("completion on ast:{:?}", statement);
        let mut c = CompletionStatement::new(loc, helper, &scopes);
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

    use std::ops::Deref;

    use super::{CompletionItem, LanguageServer};

    /// test if completion items contains a contains string
    /// use in test to see if completion is ok.
    /// this fn not returns just panic if not contains.
    pub fn test_completion_items_contains(
        items: &[CompletionItem], line: usize, col: u64, name: &str,
    ) {
        assert!(
            items.iter().any(|x| x.item.eq_ignore_ascii_case(name)),
            "line {}:{} items:{:?},expect:{:?}",
            line,
            col,
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
                log::trace!("on pos:{}:{}", item.line, i.0);
                let items = db
                    .auto_completion(&item.sql, 1, i.0 as u32, use_lexer)
                    .await
                    .unwrap();
                for should in i.1.iter() {
                    test_completion_items_contains(&items[..], item.line, i.0, &should);
                }
            }
        }
    }
    //
}

#[derive(Clone, Debug, typescript_type_def ::TypeDef, serde::Serialize)]
pub struct ErrAndLoc {
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub msg: String,
}
