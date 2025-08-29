use std::borrow::Cow;

use sqlparser::ast::Statement;
use sqlparser::dialect::Dialect;
use sqlparser::tokenizer::Token;

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

pub fn seperate_sql_by_sqlparser_tokenizer(
    sql: &str, dialect: &dyn Dialect,
) -> anyhow::Result<Vec<String>> {
    let tokens: Vec<_> = sqlparser::tokenizer::Tokenizer::new(dialect, sql)
        .tokenize()?
        .into_iter()
        .filter(|x| !matches!(x, Token::Whitespace(_)))
        .collect();
    let mut ret = vec![];
    let mut cache = vec![];
    for t in tokens.iter() {
        match t {
            Token::SemiColon => {
                if cache.len() > 0 {
                    ret.push(cache.join(" "));
                    cache.clear();
                }
            }
            _ => {
                cache.push(t.to_string());
            }
        }
    }
    if cache.len() > 0 {
        ret.push(cache.join(" "));
    }

    anyhow::Ok(ret)
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
            Statement::DropTrigger { .. } => true,
            Statement::DropDomain(..) => true,
            Statement::DropProcedure { .. } => true,
            Statement::DropPolicy { .. } => true,
            Statement::DropExtension { .. } => true,
            Statement::Discard { .. } => true,
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
    fn test_escape_csv() {
        assert_eq!(escape_csv("\" aaaa,", ','), "\"\\\" aaaa,\"");
    }

    #[test]
    fn test_current_timestamp() {
        let x = current_timestamp();
        println!("{}", x);
    }

    #[test]
    fn test_sql_seprate() {
        {
            let d = sqlparser::dialect::ClickHouseDialect {};
            let x = seperate_sql_by_sqlparser_tokenizer(
                r#"
        
            sele1ct 1;
            ;;;;;;


            select 2;
        "#,
                &d,
            )
            .unwrap();
            assert_eq!(x.len(), 2);
            for s in x.iter() {
                println!("{}", s);
            }
        }

        {
            let d = sqlparser::dialect::ClickHouseDialect {};
            let x = seperate_sql_by_sqlparser_tokenizer(
                r#"
        
            sele1ct 1;

            select 2
        "#,
                &d,
            )
            .unwrap();
            assert_eq!(x.len(), 2);
            for s in x.iter() {
                println!("{}", s);
            }
        }
    }
}
