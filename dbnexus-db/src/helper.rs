use std::borrow::Cow;
use std::sync::Arc;

use dashmap::DashMap;
use line_index::{LineCol, TextSize};
use sqlparser::ast::Statement;
use sqlparser::dialect::Dialect;
use sqlparser::tokenizer::{Location, Token};

/// cut statement into smaller base on statement or semi_colon.
pub fn sql_cut_by_semicolon(sql: &str, dialect: &dyn Dialect) -> anyhow::Result<Vec<String>> {
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

    // cut sql by semicolon
    fn cut_sql_to_each_statement_semi_colon(
        sql: &str, dialect: &dyn Dialect,
    ) -> anyhow::Result<Vec<String>> {
        let lines = line_index::LineIndex::new(sql);
        let tokens = sqlparser::tokenizer::Tokenizer::new(dialect, sql).tokenize_with_location()?;
        let tokens: Vec<_> = tokens
            .into_iter()
            .filter(|x| !matches!(x.token, Token::Whitespace(_)))
            .collect();

        if tokens.is_empty() {
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
                if sql != ";" && !sql.is_empty() {
                    ret.push(sql);
                }
                if index + 1 < tokens.len() {
                    start = &tokens[index + 1];
                }
            }
        }
        anyhow::Ok(ret)
    }
    cut_sql_to_each_statement_semi_colon(sql, dialect)
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
        s.push('\n');
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
            Statement::Delete(x) => x.selection.is_none(),
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

#[allow(unused)]
pub struct TimeSome {
    some: String,
    start: std::time::Instant,
}

impl Drop for TimeSome {
    fn drop(&mut self) {
        println!(
            "start:{:?} elapsed:{}ms end:{}",
            self.start,
            self.start.elapsed().as_millis(),
            self.some
        );
    }
}

impl TimeSome {
    pub fn new(some: String) -> Self {
        let start = std::time::Instant::now();
        println!("start:{} time:{:?}", some, start);
        Self { some, start }
    }
}

/// format sql routine for crate `sqlformat`.
pub(crate) fn sqlformat_format_sql(sql: &str, op: Option<sqlformat::FormatOptions>) -> String {
    sqlformat::format(sql, &sqlformat::QueryParams::None, &op.unwrap_or_default())
}

#[derive(Debug, Clone, Default)]
pub struct TXExecuteTimes {
    times: Arc<DashMap<uuid::Uuid, usize>>,
}

impl TXExecuteTimes {
    pub fn inc(&self, uuid: uuid::Uuid) {
        match self.times.get_mut(&uuid) {
            Some(mut x) => {
                *x += 1;
            }
            None => {
                self.times.insert(uuid, 1);
            }
        }
    }

    pub fn reset(&self, uuid: uuid::Uuid) {
        self.times.remove(&uuid);
    }

    pub fn get(&self, uuid: uuid::Uuid) -> usize {
        self.times.get(&uuid).map(|x| *x).unwrap_or_default()
    }
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
