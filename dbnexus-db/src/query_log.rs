use std::sync::{Arc, Mutex};

use dashmap::DashMap;
use typescript_type_def::TypeDef;

use crate::helper::current_timestamp;

#[derive(Clone, Debug, Default)]
pub struct QueryLog {
    pub(crate) logs: Vec<QueryLogEvent>,
}

/// use single instance to manage this.
impl QueryLog {
    pub(crate) fn get_instance() -> Arc<DashMap<uuid::Uuid, Arc<Mutex<QueryLog>>>> {
        static LOGS: std::sync::OnceLock<Arc<DashMap<uuid::Uuid, Arc<Mutex<QueryLog>>>>> =
            std::sync::OnceLock::new();
        LOGS.get_or_init(|| Default::default()).clone()
    }

    pub(crate) fn get_guard(id: uuid::Uuid, sql: impl Into<String>) -> QueryLogGuard {
        let log = QueryLog::get_instance().entry(id).or_default().clone();
        QueryLogGuard::new(sql, log)
    }

    pub(crate) fn remove(id: &uuid::Uuid) {
        QueryLog::get_instance().remove(id);
    }
}

impl QueryLog {
    pub(crate) fn append(&mut self, sql: impl Into<String>, success: bool) {
        self.logs.push(QueryLogEvent::new(sql, success));
    }
}

#[derive(Clone, Debug, serde::Serialize, TypeDef)]
pub struct QueryLogEvent {
    time: u64,
    sql: String,
    success: bool,
}

impl QueryLogEvent {
    pub(crate) fn new(sql: impl Into<String>, success: bool) -> Self {
        Self {
            time: current_timestamp(),
            sql: sql.into(),
            success,
        }
    }
}

pub struct QueryLogGuard {
    success: bool,
    sql: String,
    dest: Arc<std::sync::Mutex<QueryLog>>,
}

impl QueryLogGuard {
    fn new(sql: impl Into<String>, dest: Arc<std::sync::Mutex<QueryLog>>) -> Self {
        Self {
            success: Default::default(),
            sql: sql.into(),
            dest,
        }
    }
    pub(crate) fn set_success(&mut self, success: bool) {
        self.success = success;
    }
}
impl Drop for QueryLogGuard {
    fn drop(&mut self) {
        self.dest
            .lock()
            .unwrap()
            .append(self.sql.clone(), self.success)
    }
}
