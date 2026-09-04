use dbnexus_db::datasource_config::SomeTimeOut;

pub mod duckdb;
pub mod duckdb_lang_server;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct DuckDBConfig {
    pub path: String,
    pub read_only: bool,
    pub db: Option<String>,
    pub schema: Option<String>,
    pub conn_timeout: Option<SomeTimeOut>,
    pub max_memory: Option<String>,
}

impl DuckDBConfig {
    pub fn schema_str(&self) -> &str {
        self.schema.as_ref().expect("must be some")
    }

    pub fn db_str(&self) -> &str {
        self.db.as_ref().expect("must be some")
    }
}
