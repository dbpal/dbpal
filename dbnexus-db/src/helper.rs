use std::borrow::Cow;

use line_index::{LineCol, TextSize};
use sqlparser::ast::Statement;
use sqlparser::dialect::Dialect;
use sqlparser::tokenizer::{Location, Token};

/// front can execute a sequence of sql.
/// this represent a single query and a kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlWithKind {
    pub sql: String,
    pub kind: SqlKind,
}
impl SqlWithKind {
    pub fn new(sql: impl Into<String>, kind: SqlKind) -> Self {
        let sql = sql.into();
        Self { sql, kind }
    }
}

impl std::fmt::Display for SqlWithKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "kind:{:?} sql:{}", self.kind, self.sql)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlKind {
    Query,
    Execution,
    /// start transaction.
    Begin,
    /// commit
    Commit,
    /// rollback
    Rollback,
}
fn is_statement_begin(s: &Statement) -> bool {
    match s {
        Statement::StartTransaction { .. } => true,
        _ => false,
    }
}

fn is_statement_commit(s: &Statement) -> bool {
    match s {
        Statement::Commit { .. } => true,
        _ => false,
    }
}

fn is_statement_rollback(s: &Statement) -> anyhow::Result<bool> {
    match s {
        Statement::Rollback { savepoint, .. } => {
            if savepoint.is_some() {
                anyhow::bail!("save point is not supported")
            } else {
                anyhow::Ok(true)
            }
        }
        _ => anyhow::Ok(false),
    }
}

impl SqlKind {
    pub(crate) fn get_from_statement(s: &Statement) -> anyhow::Result<Self> {
        let x = if is_statement_begin(s) {
            SqlKind::Begin
        } else if is_statement_commit(s) {
            SqlKind::Commit
        } else if is_statement_rollback(s)? {
            SqlKind::Rollback
        } else if is_statement_absolute_query(s) {
            SqlKind::Query
        } else {
            SqlKind::Execution
        };
        anyhow::Ok(x)
    }

    pub(crate) fn get_from_sql(sql: &str) -> anyhow::Result<Self> {
        let t = sql.to_uppercase();
        if t.starts_with("BEGIN") || (t.starts_with("START") && t.contains("TRANSACTION")) {
            return anyhow::Ok(SqlKind::Begin);
        }
        if t.starts_with("COMMIT") {
            return anyhow::Ok(SqlKind::Commit);
        }
        if t.starts_with("ROLLBACK") {
            return anyhow::Ok(SqlKind::Rollback);
        }
        anyhow::Ok(
            if t.starts_with("SELECT") || t.starts_with("SHOW") || t.starts_with("EXPLAIN") {
                SqlKind::Query
            } else {
                SqlKind::Execution
            },
        )
    }
}

/// cut statement into smaller base on statement or semi_colon.
pub fn sql_to_sql_seq(
    sql: &str, dialect: &dyn Dialect,
    statement_kind_stmt: impl Fn(&Statement) -> anyhow::Result<SqlKind>,
    statement_kind_sql: impl Fn(&str) -> anyhow::Result<SqlKind>,
) -> anyhow::Result<Vec<SqlWithKind>> {
    fn get_sql(
        lineindex: &line_index::LineIndex, sql: &str, start: Location, end: Option<Location>,
    ) -> String {
        let start_offset = lineindex
            .offset(LineCol {
                line: (start.line - 1) as u32,
                col: (start.column - 1) as u32,
            })
            .unwrap();

        let start_offset = usize::from(start_offset);

        let end_offset = end
            .map(|x| {
                {
                    lineindex.offset(LineCol {
                        line: (x.line - 1) as u32,
                        col: (x.column - 1) as u32,
                    })
                }
                .unwrap()
            })
            .unwrap_or(TextSize::from(sql.len() as u32));
        let end_offset = usize::from(end_offset);

        let s = sql[start_offset..end_offset].to_string();
        s.trim().to_string()
    }

    /// cut statement into smaller base on statement.
    fn cut_sql_to_each_statement_ast(
        sql: &str, dialect: &dyn Dialect,
        statement_kind: impl Fn(&Statement) -> anyhow::Result<SqlKind>,
    ) -> anyhow::Result<Vec<SqlWithKind>> {
        let lines = line_index::LineIndex::new(sql);
        let parser = sqlparser::parser::Parser::new(dialect);
        let mut parser = parser.try_with_sql(sql)?;
        let mut ret = Vec::new();
        while parser.peek_token().token != Token::EOF {
            while Token::SemiColon == parser.peek_token()
                || matches!(parser.peek_token().token, Token::Whitespace(_))
            {
                parser.next_token();
            }
            if parser.peek_token() == Token::EOF {
                break;
            }

            let start = parser.peek_token();
            let stmt = parser.parse_statement()?;
            let kind = statement_kind(&stmt)?;

            let sql = get_sql(&lines, sql, start.span.start, {
                let t = parser.peek_token();
                if t.token == Token::EOF {
                    None
                } else {
                    Some(t.span.start)
                }
            });
            ret.push(SqlWithKind::new(sql, kind));
        }
        anyhow::Ok(ret)
    }
    match cut_sql_to_each_statement_ast(sql, dialect, statement_kind_stmt) {
        Ok(x) => return Ok(x),
        Err(err) => {
            log::error!("on ast failedmerr:{}", err.to_string());
        }
    }
    // cut sql by semicolon
    fn cut_sql_to_each_statement_semi_colon(
        sql: &str, dialect: &dyn Dialect, statement_kind: impl Fn(&str) -> anyhow::Result<SqlKind>,
    ) -> anyhow::Result<Vec<SqlWithKind>> {
        let lines = line_index::LineIndex::new(sql);
        let tokens = sqlparser::tokenizer::Tokenizer::new(dialect, sql).tokenize_with_location()?;
        let tokens: Vec<_> = tokens
            .into_iter()
            .filter(|x| !matches!(x.token, Token::Whitespace(_)))
            .collect();

        if tokens.len() == 0 {
            anyhow::bail!("empty sql.");
        }

        log::trace!("tokens:{:?}", tokens);

        let mut start = &tokens[0];

        let mut ret = Vec::new();
        for index in 1..tokens.len() {
            let t = &tokens[index];

            // last one or `;`
            if t.token == Token::SemiColon || index == tokens.len() - 1 {
                let sql = get_sql(
                    &lines,
                    sql,
                    start.span.start,
                    if t.token == Token::EOF {
                        None
                    } else {
                        Some(t.span.start)
                    },
                );
                let kind = statement_kind(sql.as_str())?;
                if sql != ";" && !sql.is_empty() {
                    ret.push(SqlWithKind::new(sql, kind));
                }
                if index + 1 < tokens.len() {
                    start = &tokens[index + 1];
                }
            }
        }
        anyhow::Ok(ret)
    }
    cut_sql_to_each_statement_semi_colon(sql, dialect, statement_kind_sql)
}

fn is_statement_absolute_query(s: &Statement) -> bool {
    match s {
        Statement::Query(_) => true,
        Statement::Fetch { .. } => true,
        Statement::ShowFunctions { filter: _ } => true,
        Statement::ShowVariable { variable: _ } => true,
        Statement::ShowVariables { .. } => true,
        Statement::ShowCreate { .. } => true,
        Statement::ShowColumns { .. } => true,
        Statement::ShowTables { .. } => true,
        Statement::ShowCollation { filter: _ } => true,
        Statement::ExplainTable { .. } => true,
        Statement::Explain { .. } => true,
        _ => false,
    }
}

/// create csv file list of string
/// each line of line separator
pub(crate) fn create_csv(ds: &Vec<Vec<Option<String>>>, delimiter: char) -> String {
    let mut s = String::new();
    for d in ds.iter() {
        let mut s2 = String::new();
        let last = d.len() - 1;
        for (index, x) in d.iter().enumerate() {
            if let Some(x) = x {
                s2.push_str(&escape_csv(x, delimiter));
            }
            if index != last {
                s2.push(delimiter);
            }
        }
        s.push_str(s2.as_str());
        s.push_str("\n");
    }
    s
}

fn escape_csv(s: &str, delimiter: char) -> Cow<'_, str> {
    if s.contains(delimiter) {
        let s = s.replace("\"", "\\\"");
        Cow::Owned(format!("\"{}\"", s))
    } else {
        Cow::Borrowed(s)
    }
}

/// get current timestamp
pub fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();
    let timestamp: u64 = now
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();
    timestamp
}

pub(crate) fn is_dangerous_sql_by_parser(sql: &str, dialect: &dyn Dialect) -> anyhow::Result<bool> {
    let ss = sqlparser::parser::Parser::parse_sql(dialect, sql)?;
    for s in ss.iter() {
        let x = match s {
            Statement::Truncate { .. } => true,
            Statement::Drop { .. } => true,
            Statement::DropFunction { .. } => true,
            Statement::DropSecret { .. } => true,
            Statement::Delete(x) => {
                if x.selection.is_none() {
                    true
                } else {
                    false
                }
            }
            _ => false,
        };
        if x {
            return anyhow::Ok(true);
        }
    }
    anyhow::Ok(false)
}

pub struct OffsetLimitIter<Index = usize> {
    pub(crate) total: Index,
    pub(crate) offset: Index,
    pub(crate) size: Index,
}

impl<T: Default> OffsetLimitIter<T> {
    pub(crate) fn new(total: T, size: T) -> Self {
        Self {
            offset: Default::default(),
            total,
            size,
        }
    }
}

impl<T: std::ops::Add + std::ops::AddAssign + Copy + std::cmp::Ord> Iterator
    for OffsetLimitIter<T>
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset < self.total {
            let offset = self.offset;
            self.offset += self.size;
            Some(offset)
        } else {
            None
        }
    }
}
/// format sql routine for crate `sqlformat`.
pub(crate) fn sqlformat_format_sql(sql: &str, op: Option<sqlformat::FormatOptions>) -> String {
    sqlformat::format(sql, &sqlformat::QueryParams::None, &op.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_off_iter() {
        assert_eq!(
            OffsetLimitIter::new(9, 3).collect::<Vec<_>>(),
            vec![0, 3, 6]
        );
        assert_eq!(
            OffsetLimitIter::new(9, 4).collect::<Vec<_>>(),
            vec![0, 4, 8]
        );
    }

    #[test]
    fn test_sqlformat_format_sql() {
        assert_eq!(sqlformat_format_sql("select 1", None), "select\n  1");
    }

    #[test]
    fn test_escape_csv() {
        assert_eq!(escape_csv("\" aaaa,", ','), "\"\\\" aaaa,\"");
    }

    #[test]
    fn test_current_timestamp() {
        let x = current_timestamp();
        println!("{}", x);
    }
}
