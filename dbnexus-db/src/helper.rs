/// create csv file list of string
/// each line of line separator
pub(crate) fn create_csv(
    ds: &[Option<String>],
    delimiter: char,
) -> anyhow::Result<tokio_util::bytes::Bytes> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter as u8)
        .from_writer(Vec::new());

    writer.write_record(ds.iter().map(|x| match x {
        Some(x) => x.as_bytes(),
        None => &[],
    }))?;
    writer.flush()?;
    let r = writer.into_inner()?;
    let r = tokio_util::bytes::Bytes::from(r);
    anyhow::Ok(r)
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

impl TimeSome {
    pub fn new(some: String) -> Self {
        let start = std::time::Instant::now();
        println!("start:{} time:{:?}", some, start);
        Self { some, start }
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
}
