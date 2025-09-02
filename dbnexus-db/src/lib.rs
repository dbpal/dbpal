use std::sync::Once;
use std::sync::atomic::{AtomicUsize, Ordering};

use lazy_static::lazy_static;
use log::{Level, LevelFilter, Metadata, Record};

pub mod datasource_config;
pub mod db;
pub mod language_server;
pub mod meta_value;

pub mod cassandra;
pub mod cassandra_lang_server;
pub mod clickhouse;
pub mod clickhouse_lang_server;
pub mod cockroach;
pub mod cockroach_lang_server;
#[cfg(feature = "duckdb")]
pub mod duckdb;
#[cfg(feature = "duckdb")]
pub mod duckdb_lang_server;
pub mod firebird;
pub mod firebird_lang_server;
pub mod helper;
pub mod libsql;
pub mod libsql_lang_server;
pub mod mariadb;
pub mod mariadb_lang_server;
pub mod mongo;
pub mod mongo_lang_server;
pub mod mssql;
pub mod mssql_lang_server;
pub mod mysql;
pub mod mysql_lang_server;
pub mod oracle;
pub mod oracle_lang_server;
pub mod pg;
pub mod pg_format;
pub mod pg_lang_server;
pub mod pg_lang_server_static;
pub mod redis;
pub mod redis_lang_server;
pub mod sqlite;
pub mod sqlite_lang_server;
pub mod sqlite_lang_server_static;
pub mod sqlparser_completing;
pub mod sqlparser_completion;
pub mod tidb;
pub mod tidb_lang_server;
pub mod typedef;

lazy_static! {
    static ref TEST_HOST: String =
        std::env::var("NEXUS_TEST_HOST").unwrap_or("127.0.0.1".to_string());
}

struct SimpleLogger;
impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            eprintln!(
                "{}:{}  {} - {}",
                record.file().unwrap_or_default(),
                record.line().unwrap_or_default(),
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}
const LOGGER: SimpleLogger = SimpleLogger;

pub fn enable_test_log() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        log::set_logger(&LOGGER)
            .map(|()| log::set_max_level(LevelFilter::Trace))
            .unwrap()
    })
}

/// limit memory when doing some operation
/// like select a lot of data from database.
pub struct MemoryLimit {
    count: usize,
    m: usize,
}

impl Default for MemoryLimit {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryLimit {
    pub fn new() -> Self {
        Self { count: 0, m: 10000 }
    }
}

#[allow(unused)]
impl MemoryLimit {
    pub fn instance() -> &'static AtomicUsize {
        // default to 200M
        static SIZE: AtomicUsize = AtomicUsize::new(1200 * 1024 * 1024);
        &SIZE
    }

    pub fn get_limit() -> usize {
        Self::instance().load(Ordering::Relaxed)
    }

    pub fn set_limit(x: usize) {
        Self::instance().store(x, Ordering::Relaxed)
    }

    pub fn exceed() -> bool {
        memory_stats::memory_stats()
            .map(|usage| usage.physical_mem > Self::get_limit())
            .unwrap_or(false)
    }

    pub fn exceed_error(&mut self) -> anyhow::Result<()> {
        self.count += 1;
        if (self.count % self.m) == 0 && Self::exceed() {
            Err(anyhow::Error::msg(format!(
                "exceed max memory of {}m",
                Self::get_limit() >> 20
            )))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn xxx() {}
}
