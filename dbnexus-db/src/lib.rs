#![warn(clippy::unwrap_used)]
use lazy_static::lazy_static;
use log::{Level, LevelFilter, Metadata, Record};
use std::sync::Once;

#[cfg(feature = "bigquery")]
pub mod bigquery;
#[cfg(feature = "bigquery")]
pub mod bigquery_lang_server;
pub mod cassandra;
pub mod cassandra_lang_server;
pub mod clickhouse;
pub mod clickhouse_lang_server;
pub mod cockroach;
pub mod couchbase;
pub mod couchbase_lang_server;

#[cfg(feature = "hive")]
pub mod hive;

pub mod datasource_config;
pub mod db;
#[cfg(feature = "duckdb")]
pub mod duckdb;
#[cfg(feature = "duckdb")]
pub mod duckdb_lang_server;
pub mod firebird;
pub mod firebird_lang_server;
pub mod helper;
pub mod language_server;
pub mod libsql;
pub mod libsql_lang_server;
pub mod mariadb;
pub mod mongo;
pub mod mongo_lang_server;
pub mod mssql;
pub mod mssql_lang_server;
pub mod mysql;
pub mod mysql_lang_server;

#[cfg(feature = "odbc")]
pub mod odbc;
pub mod oracle;
pub mod oracle_lang_server;
pub mod pg;
pub mod pg_format;
pub mod pg_lang_server;
pub mod redis;
pub mod redis_lang_server;
pub mod scylla;
#[cfg(feature = "snowflake")]
pub mod snowflake;
#[cfg(feature = "snowflake")]
pub mod snowflake_lang_server;
pub mod sqlite;
pub mod sqlite_lang_server;
pub mod sqlparser_completing;
pub mod sqlparser_completion;
pub mod tidb;
pub mod update_on_resultset;

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
                "{}:{} {}-{}",
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
            .expect("should set ok")
    })
}
