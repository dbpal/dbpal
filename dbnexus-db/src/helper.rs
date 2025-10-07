use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;

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

pub fn separate_sql_by_sqlparser_tokenizer(
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
                if !cache.is_empty() {
                    ret.push(cache.join(" "));
                    cache.clear();
                }
            }
            _ => {
                cache.push(t.to_string());
            }
        }
    }
    if !cache.is_empty() {
        ret.push(cache.join(" "));
    }

    anyhow::Ok(ret)
}

fn escape_csv(s: &str, delimiter: char) -> Cow<'_, str> {
    if s.contains(delimiter) {
        Cow::Owned(format!(r#""{}""#, s))
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

#[derive(Default, Clone)]
pub struct NextId {
    next_id: Arc<AtomicU32>,
}

impl NextId {
    pub fn next(&self) -> u32 {
        self.next_id
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }

    pub fn get(&self) -> u32 {
        self.next_id.load(std::sync::atomic::Ordering::Relaxed)
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
    fn test_current_timestamp() {
        let x = current_timestamp();
        println!("{}", x);
    }

    #[test]
    fn test_sql_separate() {
        {
            let d = sqlparser::dialect::ClickHouseDialect {};
            let x = separate_sql_by_sqlparser_tokenizer(
                r#"
        
            select 1;
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
            let x = separate_sql_by_sqlparser_tokenizer(
                r#"
        
            select 1;

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

#[inline]
pub(crate) fn remove_first_char_string(s: String) -> String {
    s.chars().skip(1).collect()
}

// try remove a @ used for mysql etc database
pub(crate) fn try_remove_one_at(value: String) -> String {
    if value.starts_with('@') && !value.starts_with("@@") {
        remove_first_char_string(value)
    } else {
        value
    }
}
