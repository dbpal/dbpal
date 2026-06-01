use core::str;
use dashmap::DashMap;
use dbnexus_db::datasource_config::{
    CassandraDBConfig, ClickHouseConfig, CouchBaseDBConfig, Db2Config, DuckDBConfig,
    FireBirdConfig, LibSqlDBConfig, MongoDBConfig, MysqlDBConfig, OracleDBConfig, PGDBConfig,
    RedisDBConfig, SqlServerDBConfig, SqliteDBConfig,
};
use dbnexus_db::db::{
    BackendKind, BackendKindMysql, BackendKindPostGres, CatalogTrait, CheckConn, DBTrait,
    ExportQueryResult, QuerySql, SchemaTrait, ServerMetas,
};
use dbnexus_db::language_server::LanguageServer;
use dbnexus_db::mongo::MongoDB;
use dbnexus_db::mysql::MysqlDB;
use dbnexus_db::pg::PostGresDB;
use font_kit::error::SelectionError;
use specta::datatype::DataType;
use std::any::Any;
use std::ops::Deref;

use mongodb::bson::{self, Document};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, specta::Type, Default)]
pub struct OpenAIReq {
    pub url: String,
    pub api_key: String,
    pub model: String,
    pub prompt: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u64>,
    pub top_p: Option<f64>,
    // data:image/jpeg;base64 data encoded in base64 image data
    pub images: Vec<String>,

    // documents can be a text or sql
    pub documents: Vec<String>,

    pub user_system_prompt: Option<String>,
    pub search_engine: AiOnlineSearch,
    pub chat_history: Vec<ChatHistory>,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, specta::Type)]
#[serde(tag = "resp_type", rename_all = "lowercase")]
pub enum AISteamResp {
    Content {
        msg: String,
    },
    TokenUsage {
        input_token: u64,
        output_token: u64,
        total_token: u64,
    },
}

impl<T: Into<String>> From<T> for AISteamResp {
    fn from(value: T) -> Self {
        Self::Content { msg: value.into() }
    }
}

impl std::fmt::Display for AISteamResp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match &self {
                AISteamResp::Content { msg } => msg.clone(),
                AISteamResp::TokenUsage {
                    input_token,
                    output_token,
                    total_token,
                } => format!(
                    "input_token: {}, output_token: {}, total_token: {}",
                    input_token, output_token, total_token
                ),
            }
        )
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, specta::Type)]
#[serde(rename_all = "lowercase")]
pub struct ChatHistory {
    pub role: String,
    pub msg: String,
}

impl DBConnectionConfig {
    pub fn get_name(&self) -> &str {
        match self {
            DBConnectionConfig::Sqlite { name, .. }
            | DBConnectionConfig::Mysql { name, .. }
            | DBConnectionConfig::MariaDB { name, .. }
            | DBConnectionConfig::Tidb { name, .. }
            | DBConnectionConfig::Postgres { name, .. }
            | DBConnectionConfig::SqlServer { name, .. }
            | DBConnectionConfig::Mongo { name, .. }
            | DBConnectionConfig::Redis { name, .. }
            | DBConnectionConfig::Oracle { name, .. }
            | DBConnectionConfig::Cassandra { name, .. }
            | DBConnectionConfig::ClickHouse { name, .. }
            | DBConnectionConfig::LibSql { name, .. }
            | DBConnectionConfig::FireBird { name, .. }
            | DBConnectionConfig::DuckDB { name, .. }
            | DBConnectionConfig::CockRoach { name, .. }
            | DBConnectionConfig::Db2 { name, .. }
            | DBConnectionConfig::CouchBase { name, .. } => name.as_str(),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "conn_type", rename_all = "lowercase")]
pub enum DBConnectionConfig {
    Sqlite {
        #[serde(flatten)]
        conn: SqliteDBConfig,
        name: String,
    },
    Mysql {
        #[serde(flatten)]
        conn: MysqlDBConfig,
        name: String,
        save_password: bool,
    },
    MariaDB {
        #[serde(flatten)]
        conn: MysqlDBConfig,
        name: String,
        save_password: bool,
    },
    Tidb {
        #[serde(flatten)]
        conn: MysqlDBConfig,
        name: String,
        save_password: bool,
    },
    Postgres {
        #[serde(flatten)]
        conn: PGDBConfig,
        name: String,
        save_password: bool,
    },
    CockRoach {
        #[serde(flatten)]
        conn: PGDBConfig,
        name: String,
        save_password: bool,
    },
    SqlServer {
        #[serde(flatten)]
        conn: SqlServerDBConfig,
        name: String,
        save_password: bool,
    },
    Mongo {
        #[serde(flatten)]
        conn: MongoDBConfig,
        name: String,
        save_password: bool,
    },
    Redis {
        #[serde(flatten)]
        conn: RedisDBConfig,
        name: String,
        save_password: bool,
    },
    Oracle {
        #[serde(flatten)]
        conn: OracleDBConfig,
        name: String,
        save_password: bool,
    },
    Cassandra {
        #[serde(flatten)]
        conn: CassandraDBConfig,
        name: String,
        save_password: bool,
    },
    ClickHouse {
        #[serde(flatten)]
        conn: ClickHouseConfig,
        name: String,
        save_password: bool,
    },
    LibSql {
        #[serde(flatten)]
        conn: LibSqlDBConfig,
        name: String,
    },
    FireBird {
        #[serde(flatten)]
        conn: FireBirdConfig,
        name: String,
        save_password: bool,
    },
    DuckDB {
        #[serde(flatten)]
        conn: DuckDBConfig,
        name: String,
    },
    Db2 {
        #[serde(flatten)]
        conn: Db2Config,
        name: String,
        save_password: bool,
    },

    CouchBase {
        #[serde(flatten)]
        conn: CouchBaseDBConfig,
        name: String,
        save_password: bool,
    },
}

impl DBConnectionConfig {
    pub async fn open(&self) -> anyhow::Result<DatabaseBackend> {
        match self {
            DBConnectionConfig::Sqlite { conn, .. } => {
                let db: Arc<dyn SchemaTrait> =
                    Arc::new(dbnexus_db::sqlite::SqliteDB::new(conn.clone()).await?)
                        as Arc<dyn SchemaTrait>;
                anyhow::Ok(DatabaseBackend::Schema(db))
            }
            DBConnectionConfig::Mysql { conn, .. } => match &conn.db {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> = Arc::new(
                        dbnexus_db::mysql::MysqlDB::new(conn.clone(), BackendKindMysql::Mysql)
                            .await?,
                    ) as Arc<dyn SchemaTrait>;
                    anyhow::Ok(DatabaseBackend::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> = Arc::new(
                        dbnexus_db::mysql::MysqlDB::new(conn.clone(), BackendKindMysql::Mysql)
                            .await?,
                    ) as Arc<dyn DBTrait>;
                    anyhow::Ok(DatabaseBackend::DataBase(db))
                }
            },
            DBConnectionConfig::MariaDB { conn, .. } => match &conn.db {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(MysqlDB::new(conn.clone(), BackendKindMysql::Mariadb).await?)
                            as Arc<dyn SchemaTrait>;
                    anyhow::Ok(DatabaseBackend::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(MysqlDB::new(conn.clone(), BackendKindMysql::Mariadb).await?)
                            as Arc<dyn DBTrait>;
                    anyhow::Ok(DatabaseBackend::DataBase(db))
                }
            },
            DBConnectionConfig::Tidb { conn, .. } => match &conn.db {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(MysqlDB::new(conn.clone(), BackendKindMysql::Tidb).await?)
                            as Arc<dyn SchemaTrait>;
                    anyhow::Ok(DatabaseBackend::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(MysqlDB::new(conn.clone(), BackendKindMysql::Tidb).await?)
                            as Arc<dyn DBTrait>;
                    anyhow::Ok(DatabaseBackend::DataBase(db))
                }
            },
            DBConnectionConfig::Postgres { conn, .. } => {
                let o = Arc::new(
                    dbnexus_db::pg::PostGresDB::new(conn.clone(), BackendKindPostGres::PostGres)
                        .await?,
                );
                if conn.db.is_none() {
                    return anyhow::Ok(DatabaseBackend::Catalog(o as Arc<dyn CatalogTrait>));
                }

                match &conn.schema {
                    None => anyhow::Ok(DatabaseBackend::DataBase(o as Arc<dyn DBTrait>)),
                    Some(_) => anyhow::Ok(DatabaseBackend::Schema(o as Arc<dyn SchemaTrait>)),
                }
            }
            DBConnectionConfig::CockRoach { conn, .. } => {
                let o =
                    Arc::new(PostGresDB::new(conn.clone(), BackendKindPostGres::CockRoach).await?);
                if conn.db.is_none() {
                    return anyhow::Ok(DatabaseBackend::Catalog(o as Arc<dyn CatalogTrait>));
                }
                match &conn.schema {
                    None => anyhow::Ok(DatabaseBackend::DataBase(o as Arc<dyn DBTrait>)),
                    Some(_) => anyhow::Ok(DatabaseBackend::Schema(o as Arc<dyn SchemaTrait>)),
                }
            }
            DBConnectionConfig::SqlServer { conn, .. } => {
                let o = Arc::new(dbnexus_db::mssql::SqlServerDB::new(conn.clone()).await?);
                if conn.db.is_none() {
                    return anyhow::Ok(DatabaseBackend::Catalog(o as Arc<dyn CatalogTrait>));
                }
                match &conn.schema {
                    Some(_) => anyhow::Ok(DatabaseBackend::Schema(o as Arc<dyn SchemaTrait>)),
                    None => anyhow::Ok(DatabaseBackend::DataBase(o as Arc<dyn DBTrait>)),
                }
            }
            DBConnectionConfig::Mongo { conn, .. } => {
                let db: Arc<MongoDB> = Arc::new(dbnexus_db::mongo::MongoDB::new(conn.clone())?);
                match conn.db {
                    Some(_) => anyhow::Ok(DatabaseBackend::Schema(db as Arc<dyn SchemaTrait>)),
                    None => anyhow::Ok(DatabaseBackend::DataBase(db as Arc<dyn DBTrait>)),
                }
            }
            DBConnectionConfig::Redis { conn, .. } => {
                let redis = dbnexus_db::redis::RedisDB::new(conn.clone()).await?;
                let db = match &conn.db {
                    Some(_) => DatabaseBackend::Schema(Arc::new(redis) as Arc<dyn SchemaTrait>),
                    None => DatabaseBackend::DataBase(Arc::new(redis) as Arc<dyn DBTrait>),
                };
                anyhow::Ok(db)
            }
            DBConnectionConfig::Oracle { conn, .. } => match conn.schema.as_ref() {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(dbnexus_db::oracle::OracleDB::new(conn.clone(), None).await?)
                            as Arc<dyn SchemaTrait>;
                    anyhow::Ok(DatabaseBackend::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(dbnexus_db::oracle::OracleDB::new(conn.clone(), None).await?)
                            as Arc<dyn DBTrait>;
                    anyhow::Ok(DatabaseBackend::DataBase(db))
                }
            },
            DBConnectionConfig::Cassandra { conn, .. } => {
                let db = dbnexus_db::cassandra::CassandraDB::new(conn.clone()).await?;
                let db = match &conn.db {
                    Some(_) => DatabaseBackend::Schema(Arc::new(db) as Arc<dyn SchemaTrait>),
                    None => DatabaseBackend::DataBase(Arc::new(db) as Arc<dyn DBTrait>),
                };
                anyhow::Ok(db)
            }
            DBConnectionConfig::DuckDB { conn: _conn, .. } => {
                #[cfg(feature = "duckdb")]
                {
                    let db = dbnexus_db::duckdb::DuckDB::new(_conn.clone()).await?;
                    match &_conn.db {
                        Some(_) => match &_conn.schema {
                            Some(_) => anyhow::Ok(DatabaseBackend::from(
                                Arc::new(db) as Arc<dyn SchemaTrait>
                            )),
                            None => {
                                anyhow::Ok(DatabaseBackend::from(Arc::new(db) as Arc<dyn DBTrait>))
                            }
                        },
                        None => {
                            anyhow::Ok(DatabaseBackend::from(Arc::new(db) as Arc<dyn CatalogTrait>))
                        }
                    }
                }
                #[cfg(not(feature = "duckdb"))]
                anyhow::bail!("duckdb feature not enabled")
            }
            DBConnectionConfig::ClickHouse { conn, .. } => {
                let db = dbnexus_db::clickhouse::ClickHouseDB::new(conn.clone())?;
                let db = match &conn.db {
                    Some(_) => DatabaseBackend::Schema(Arc::new(db) as Arc<dyn SchemaTrait>),
                    None => DatabaseBackend::DataBase(Arc::new(db) as Arc<dyn DBTrait>),
                };

                anyhow::Ok(db)
            }
            DBConnectionConfig::LibSql { conn, .. } => {
                let db = dbnexus_db::libsql::LibSqlDB::new(conn.clone()).await?;
                anyhow::Ok(DatabaseBackend::from(Arc::new(db) as Arc<dyn SchemaTrait>))
            }
            DBConnectionConfig::FireBird { conn, .. } => {
                let db = dbnexus_db::firebird::FireBirdDB::new(conn.clone()).await?;
                anyhow::Ok(DatabaseBackend::from(Arc::new(db) as Arc<dyn SchemaTrait>))
            }
            DBConnectionConfig::Db2 { conn: _conn, .. } => {
                // let db = dbnexus_db::db2::Db2Db::new(_conn.clone()).await?;
                // let r = match &_conn.schema {
                //     Some(_) => DatabaseBackend::Schema(Arc::new(db) as Arc<dyn SchemaTrait>),
                //     None => DatabaseBackend::DataBase(Arc::new(db) as Arc<dyn DBTrait>),
                // };
                // anyhow::Ok(r)
                anyhow::bail!("db2 not supported")
            }

            DBConnectionConfig::CouchBase { conn, .. } => {
                let db = dbnexus_db::couchbase::CouchBaseDB::new(conn.clone()).await?;
                let db = match conn.bucket {
                    Some(..) => match conn.scope {
                        Some(_) => DatabaseBackend::from(Arc::new(db) as Arc<dyn SchemaTrait>),
                        None => DatabaseBackend::from(Arc::new(db) as Arc<dyn DBTrait>),
                    },
                    None => DatabaseBackend::from(Arc::new(db) as Arc<dyn CatalogTrait>),
                };
                anyhow::Ok(db)
            }
        }
    }
}

/// opened database variant for frontend
#[derive(Clone, Debug)]
pub enum DatabaseBackend {
    Catalog(Arc<dyn dbnexus_db::db::CatalogTrait>),
    DataBase(Arc<dyn dbnexus_db::db::DBTrait>),
    Schema(Arc<dyn dbnexus_db::db::SchemaTrait>),
}

impl DatabaseBackend {
    pub fn as_any(&self) -> &dyn Any {
        match self {
            DatabaseBackend::Catalog(x) => x.as_any(),
            DatabaseBackend::DataBase(x) => x.as_any(),
            DatabaseBackend::Schema(x) => x.as_any(),
        }
    }
    pub(crate) async fn check_conn(&self) -> anyhow::Result<Duration> {
        self.as_check_conn().check_conn().await
    }

    #[allow(unused)]
    pub(crate) fn backend_kind(&self) -> BackendKind {
        match self {
            DatabaseBackend::Catalog(catalog_trait) => catalog_trait.backend_kind(),
            DatabaseBackend::DataBase(dbtrait) => dbtrait.backend_kind(),
            DatabaseBackend::Schema(schema_trait) => schema_trait.backend_kind(),
        }
    }
    pub(crate) fn as_query_sql(&self) -> &dyn QuerySql {
        match self {
            DatabaseBackend::Catalog(x) => x.as_ref(),
            DatabaseBackend::DataBase(x) => x.as_ref(),
            DatabaseBackend::Schema(x) => x.as_ref(),
        }
    }
    pub(crate) fn as_export_query_result(&self) -> &dyn ExportQueryResult {
        match self {
            DatabaseBackend::Catalog(x) => x.as_ref(),
            DatabaseBackend::DataBase(x) => x.as_ref(),
            DatabaseBackend::Schema(x) => x.as_ref(),
        }
    }

    pub(crate) fn as_language_server(&self) -> &dyn LanguageServer {
        match self {
            DatabaseBackend::Catalog(x) => x.as_ref(),
            DatabaseBackend::DataBase(x) => x.as_ref(),
            DatabaseBackend::Schema(x) => x.as_ref(),
        }
    }
    pub(crate) fn as_check_conn(&self) -> &dyn CheckConn {
        match self {
            DatabaseBackend::Catalog(x) => x.as_ref(),
            DatabaseBackend::DataBase(x) => x.as_ref(),
            DatabaseBackend::Schema(x) => x.as_ref(),
        }
    }
    pub(crate) fn as_server_mates(&self) -> &dyn ServerMetas {
        match self {
            DatabaseBackend::Catalog(x) => x.as_ref(),
            DatabaseBackend::DataBase(x) => x.as_ref(),
            DatabaseBackend::Schema(x) => x.as_ref(),
        }
    }
}

impl From<Arc<dyn dbnexus_db::db::CatalogTrait>> for DatabaseBackend {
    fn from(value: Arc<dyn dbnexus_db::db::CatalogTrait>) -> Self {
        Self::Catalog(value)
    }
}

impl From<Arc<dyn dbnexus_db::db::DBTrait>> for DatabaseBackend {
    fn from(value: Arc<dyn dbnexus_db::db::DBTrait>) -> Self {
        Self::DataBase(value)
    }
}
impl From<Arc<dyn dbnexus_db::db::SchemaTrait>> for DatabaseBackend {
    fn from(value: Arc<dyn dbnexus_db::db::SchemaTrait>) -> Self {
        Self::Schema(value)
    }
}

#[derive(Clone, Default)]
pub struct DBPal {
    /// all the databases
    pub backend: Arc<Mutex<HashMap<String, DatabaseBackend>>>,
}

impl DBPal {
    pub fn new() -> Self {
        Default::default()
    }
}

pub(crate) fn name_to_parts(name: &str) -> Vec<&str> {
    name.split("/").collect()
}

impl From<tokio::time::error::Elapsed> for ErrorFrontend {
    fn from(_value: tokio::time::error::Elapsed) -> Self {
        Self {
            reason: "operation timeout".into(),
            detail: "".into(),
            cancelled: false,
        }
    }
}

fn name_starts(name: &str, prefix: &str) -> bool {
    let name = name_to_parts(name);
    let prefix = name_to_parts(prefix);

    name.len() >= prefix.len() && name.iter().zip(prefix.iter()).all(|(x, y)| *x == *y)
}

impl DBPal {
    #[allow(unused)]
    pub async fn print_keys(&self) {
        for k in self.backend.lock().await.keys() {
            println!("!!!!!!!!!!!!!!!!!{}", k);
        }
    }

    pub async fn close_conn_has_prefix(&self, prefix: &str) {
        let mut v = Vec::new();
        for k in self.backend.lock().await.keys() {
            if name_starts(k, prefix) {
                v.push(k.clone());
            }
        }
        for conn in v {
            log::info!("delete conn:{}", conn);
            self.backend.lock().await.remove(&conn);
        }
    }

    pub async fn get_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        match self.get_backend(name).await? {
            DatabaseBackend::Schema(x) => anyhow::Ok(x.clone()),
            backend => anyhow::bail!(format!("'{}' not a schema trait,but:{:?}", name, backend)),
        }
    }

    pub async fn establish_conn(
        &self,
        config: &DBConnectionConfig,
    ) -> anyhow::Result<DatabaseBackend> {
        let db = config.open().await?;
        // make sure connection is ok
        db.check_conn().await?;
        self.backend
            .lock()
            .await
            .insert(config.get_name().to_owned(), db.clone());
        anyhow::Ok(db)
    }

    pub async fn get_backend(&self, name: &str) -> anyhow::Result<DatabaseBackend> {
        let db = self.backend.lock().await.get(name).cloned();
        if let Some(db) = db {
            return anyhow::Ok(db);
        }
        // try open and enter
        let name_parts = name_to_parts(name);
        match name_parts.len() {
            1 => {
                anyhow::bail!(format!("connection '{}' not established", name))
            }
            2 => {
                let backend = self
                    .backend
                    .lock()
                    .await
                    .get(&name_parts[0].to_string())
                    .cloned();
                match backend {
                    Some(DatabaseBackend::DataBase(x)) => {
                        let schema = x.open_schema(name_parts[1]).await?;
                        self.backend
                            .lock()
                            .await
                            .insert(name.to_string(), schema.clone().into());
                        anyhow::Ok(DatabaseBackend::Schema(schema))
                    }
                    Some(DatabaseBackend::Catalog(x)) => {
                        let db = x.open_db(name_parts[1]).await?;
                        self.backend
                            .lock()
                            .await
                            .insert(name.to_string(), db.clone().into());
                        anyhow::Ok(DatabaseBackend::DataBase(db))
                    }
                    _ => {
                        log::error!(
                            "'{}' not found ,should be catalog or database trait but:{:?}",
                            name_parts[0],
                            backend
                        );
                        anyhow::bail!(format!("connection '{}' not established", name))
                    }
                }
            }
            3 => {
                let db_name = append_handle_name(name_parts[0], name_parts[1]);
                let open = self.backend.lock().await.get(&db_name).cloned();
                let db_trait = match open {
                    Some(x) => match x {
                        DatabaseBackend::DataBase(x) => x.clone(),
                        x => {
                            anyhow::bail!("this must be db trait but:{:?}", x)
                        }
                    },
                    None => {
                        let c = self
                            .backend
                            .lock()
                            .await
                            .get(&name_parts[0].to_string())
                            .cloned();
                        match c {
                            Some(x) => match x {
                                DatabaseBackend::Catalog(x) => {
                                    let db = x.open_db(name_parts[1]).await?;
                                    self.backend.lock().await.insert(db_name, db.clone().into());
                                    db
                                }
                                _ => {
                                    log::error!("internal error,catalog must been entered ");
                                    anyhow::bail!(format!("connection '{}' not established", name))
                                }
                            },
                            None => {
                                log::error!("internal error,catalog must been entered");
                                anyhow::bail!(format!("connection '{}' not established", name))
                            }
                        }
                    }
                };
                let schema = db_trait.open_schema(name_parts[2]).await?;
                self.backend
                    .lock()
                    .await
                    .insert(name.to_string(), schema.clone().into());
                anyhow::Ok(DatabaseBackend::Schema(schema))
            }
            _ => {
                // nothing to do here
                log::error!("'{}' name part not right, this is a internal error", name);
                anyhow::bail!(format!("connection '{}' not established", name))
            }
        }
    }
}

#[derive(serde::Serialize, Debug, Clone, specta::Type)]
pub struct ErrorFrontend {
    // short message on why
    pub reason: String,
    // long message about why and backtrace
    pub detail: String,
    pub cancelled: bool,
}

impl ErrorFrontend {
    pub(crate) fn cancelled(msg: Option<String>) -> Self {
        Self {
            reason: msg.unwrap_or("this task is cancelled".to_string()),
            detail: "".to_string(),
            cancelled: true,
        }
    }
}

pub fn append_handle_name(name: &str, name2: &str) -> String {
    format!("{}/{}", name, name2)
}

// 管理所有任务的取消令牌

#[derive(Clone, Default)]
pub struct TaskManager {
    pub tokens: Arc<DashMap<String, CancellationToken>>,
}

impl From<SelectionError> for ErrorFrontend {
    fn from(value: SelectionError) -> Self {
        ErrorFrontend {
            reason: value.to_string(),
            detail: value.to_string(),
            cancelled: false,
        }
    }
}
impl From<tauri::Error> for ErrorFrontend {
    fn from(value: tauri::Error) -> Self {
        let x = Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
            cancelled: false,
        };
        log::error!("error:{}", x.reason);
        x
    }
}
impl From<anyhow::Error> for ErrorFrontend {
    fn from(value: anyhow::Error) -> Self {
        let x = Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
            cancelled: false,
        };
        log::error!("error:{}", x.reason);
        x
    }
}

impl From<std::io::Error> for ErrorFrontend {
    fn from(value: std::io::Error) -> Self {
        let x = Self {
            reason: format!("{}", value),
            detail: "".to_string(),
            cancelled: false,
        };
        log::error!("error:{}", x.reason);
        x
    }
}

impl From<hex::FromHexError> for ErrorFrontend {
    fn from(value: hex::FromHexError) -> Self {
        let x = Self {
            reason: format!("{}", value),
            detail: "".to_string(),
            cancelled: false,
        };
        log::error!("error:{}", x.reason);
        x
    }
}
impl From<String> for ErrorFrontend {
    fn from(value: String) -> Self {
        Self {
            reason: value,
            detail: "".to_string(),
            cancelled: false,
        }
    }
}
impl From<&str> for ErrorFrontend {
    fn from(value: &str) -> Self {
        Self {
            reason: value.to_owned(),
            detail: "".to_string(),
            cancelled: false,
        }
    }
}

impl From<bson::oid::Error> for ErrorFrontend {
    fn from(value: bson::oid::Error) -> Self {
        Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
            cancelled: false,
        }
    }
}
impl From<serde_json::Error> for ErrorFrontend {
    fn from(value: serde_json::Error) -> Self {
        Self {
            reason: format!("{}", value),
            detail: format!("{:?}", value),
            cancelled: false,
        }
    }
}

// cancel token and msg
#[derive(Clone)]
pub struct CancelTokenMsg {
    token: CancellationToken,
    msg: Arc<std::sync::Mutex<Option<String>>>,
}
impl CancelTokenMsg {
    pub fn cancel(&self, msg: impl Into<String>) {
        let msg = msg.into();
        *self.msg.lock().expect("lock failed") = Some(msg);
        self.token.cancel();
    }
    pub fn get_msg(&self) -> Option<String> {
        self.msg.lock().expect("lock failed").clone()
    }
    pub fn new(token: CancellationToken) -> Self {
        Self {
            token,
            msg: Arc::new(std::sync::Mutex::new(None)),
        }
    }
}

impl TaskManager {
    pub fn add(&self, task_id: String) -> (CancellationToken, impl Drop) {
        let token = CancellationToken::new();
        struct XDrop {
            task_id: String,
            manager: TaskManager,
        }
        impl Drop for XDrop {
            fn drop(&mut self) {
                self.manager.tokens.remove(&self.task_id);
            }
        }
        self.tokens.insert(task_id.clone(), token.clone());
        (
            token,
            XDrop {
                task_id,
                manager: self.clone(),
            },
        )
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct JsUint8Array(Vec<u8>);

impl specta::Type for JsUint8Array {
    fn definition(_: &mut specta::Types) -> DataType {
        DataType::Reference(specta_typescript::define("Uint8Array"))
    }
}

impl Deref for JsUint8Array {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, specta::Type)]
pub struct MongoFindParameter {
    pub(crate) filter: JsUint8Array,
    pub(crate) hint: Option<JsUint8Array>,
    pub(crate) projection: Option<JsUint8Array>,
    pub(crate) sort: Option<JsUint8Array>,
    pub(crate) min: Option<JsUint8Array>,
    pub(crate) max: Option<JsUint8Array>,
    pub(crate) limit: Option<i64>,
    pub(crate) skip: Option<u64>,
}

pub fn parse_bson_doc(bs: &[u8]) -> Result<Document, ErrorFrontend> {
    bson::Document::from_reader(std::io::Cursor::new(bs)).map_err(ErrorFrontend::from)
}

impl From<mongodb::bson::de::Error> for ErrorFrontend {
    fn from(value: mongodb::bson::de::Error) -> Self {
        Self {
            reason: value.to_string(),
            detail: format!("{:?}", value),
            cancelled: false,
        }
    }
}

impl TryFrom<MongoFindParameter> for dbnexus_db::mongo::MongoFindParameter {
    type Error = ErrorFrontend;
    fn try_from(value: MongoFindParameter) -> Result<Self, Self::Error> {
        let MongoFindParameter {
            filter,
            hint,
            projection,
            sort,
            min,
            max,
            limit,
            skip,
        } = value;
        let r = dbnexus_db::mongo::MongoFindParameter {
            filter: parse_bson_doc(&filter.0)?,
            hint: if let Some(doc) = hint.as_ref() {
                Some(parse_bson_doc(&doc.0)?)
            } else {
                None
            },
            projection: if let Some(doc) = projection.as_ref() {
                Some(parse_bson_doc(&doc.0)?)
            } else {
                None
            },
            sort: if let Some(doc) = sort.as_ref() {
                Some(parse_bson_doc(&doc.0)?)
            } else {
                None
            },
            min: if let Some(doc) = min.as_ref() {
                Some(parse_bson_doc(&doc.0)?)
            } else {
                None
            },
            max: if let Some(doc) = max.as_ref() {
                Some(parse_bson_doc(&doc.0)?)
            } else {
                None
            },
            limit,
            skip,
        };
        Ok(r)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct MongoShowTablePara {
    pub(crate) sort: JsUint8Array,
    pub(crate) filter: JsUint8Array,
    pub(crate) page: u64,
    pub(crate) page_size: u64,
}

impl TryFrom<MongoShowTablePara> for dbnexus_db::mongo::MongoShowTablePara {
    type Error = ErrorFrontend;

    fn try_from(value: MongoShowTablePara) -> Result<Self, Self::Error> {
        let MongoShowTablePara {
            filter,
            page,
            page_size,
            sort,
        } = value;
        Ok(dbnexus_db::mongo::MongoShowTablePara {
            filter: parse_bson_doc(&filter.0)?,
            sort: parse_bson_doc(&sort.0)?,
            page,
            page_size,
        })
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(unused)]
pub enum SetZoom {
    In,
    Out,
    Reset,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(unused)]
pub enum AiStreamMsg {
    Notice { msg: String },
    Resp { msg: String },
}

#[derive(Debug, Clone, serde::Serialize, Default, serde::Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
#[allow(unused)]
pub struct AiOnlineSearch {
    pub enable: bool,
    pub provider: String, // duckduckgo
    pub api_key: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]

    fn test_name_starts() {
        assert!(name_starts("pg/xxx", "pg"));
        assert!(name_starts("pg/xxx/abc", "pg"));
        //
        assert!(!name_starts("pg", "pg/xxx"));
    }
}
