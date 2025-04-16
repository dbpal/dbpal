use std::sync::{Arc, Mutex};

use dashmap::DashMap;
use typescript_type_def::TypeDef;

use crate::helper::current_timestamp;

#[derive(Clone, Debug, Default)]
pub struct QueryLog {
    pub(crate) logs: Vec<QueryLogEvent>,
}

#[derive(Clone, Debug, Default)]
pub struct QueryLogMap {
    logs: Arc<DashMap<uuid::Uuid, Arc<Mutex<QueryLog>>>>,
}

impl QueryLogMap {
    pub(crate) fn get_guard(&self, id: uuid::Uuid, sql: impl Into<String>) -> QueryLogGuard {
        let log = self.logs.entry(id).or_default().clone();
        QueryLogGuard::new(sql, log)
    }
    pub(crate) fn remove(&self, id: &uuid::Uuid) {
        self.logs.remove(id);
    }
    pub(crate) fn get(&self, id: &uuid::Uuid) -> Vec<QueryLogEvent> {
        self.logs
            .get(id)
            .map(|x| x.lock().unwrap().logs.clone())
            .unwrap_or(vec![])
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
