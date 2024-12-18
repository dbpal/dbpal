use std::sync::{
    Once,
    atomic::{AtomicUsize, Ordering},
};

use lazy_static::lazy_static;
use log::{Level, LevelFilter, Metadata, Record};

pub mod completion;
pub mod datasource_config;
pub mod db;
pub mod meta_value;

pub mod app;
pub mod helper;
pub mod mariadb;
pub mod mongo;
pub mod mssql;
pub mod mssql_completion;
pub mod mysql;
pub mod mysql_completion;
pub mod oracle;
pub mod pg;
pub mod pg_completion;
pub mod pg_format;
pub mod query_log;
pub mod redis;
pub mod redis_completion;
pub mod sqlite;
pub mod sqlite_completion;
pub mod sqlparser_completing;
pub mod sqlparser_completion;
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
            eprintln!("{} - {}", record.level(), record.args());
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

const CANNOT_GET_CONN_ERR: &str = "can't get conneciton";

/// limit memory when doing some operation
/// like select a lot of data from database.
pub struct MemoryLimit;

#[allow(dead_code)]
impl MemoryLimit {
    pub fn instance() -> &'static AtomicUsize {
        // default to 200M
        static SIZE: AtomicUsize = AtomicUsize::new(200 * 1024 * 1024);
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

    pub fn exceed_error() -> anyhow::Result<()> {
        if Self::exceed() {
            Err(anyhow::Error::msg(format!(
                "exceed max memory of {}",
                Self::get_limit()
            )))
        } else {
            Ok(())
        }
    }
}

pub async fn random_sleep_impl() {
    let duration = {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let millis = rng.gen_range(500..3000);
        let duration = std::time::Duration::from_millis(millis);
        duration
    };

    tokio::time::sleep(duration).await
}
