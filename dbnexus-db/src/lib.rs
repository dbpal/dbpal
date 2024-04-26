use std::sync::Once;

use lazy_static::lazy_static;
use log::{Level, LevelFilter, Metadata, Record};

pub mod completion;
pub mod datasource;
pub mod db;

pub mod mongo;
pub mod mysql;
pub mod oracle;
pub mod pg;
pub mod pg_format;
pub mod redis;

pub mod pool;

pub mod sql_server;
pub mod sqlite;

lazy_static! {
    static ref TEST_HOST: String = std::env::var("NEXUS_TEST_HOST").unwrap_or("".to_string());
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

pub fn init_log() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        log::set_logger(&LOGGER)
            .map(|()| log::set_max_level(LevelFilter::Trace))
            .unwrap()
    })
}
