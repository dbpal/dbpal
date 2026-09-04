use std::collections::HashMap;

use crate::db::OptionMustBeSomeErr;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct MongoDBConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub conn_timeout: Option<SomeTimeOut>,
    pub db: Option<String>,
    pub read_only: bool,
    pub ssl_enable: bool,
    pub allow_invalid_hostnames: Option<bool>,
    pub allow_invalid_certificates: Option<bool>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct MysqlDBConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub db: Option<String>,
    pub read_only: bool,
    #[serde(flatten)]
    pub ssl: SslFilesWithStrModel,
    pub conn_timeout: Option<SomeTimeOut>,
    pub charset: MysqlCharset,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(transparent)]
pub struct MysqlCharset(pub String);

impl From<String> for MysqlCharset {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl Default for MysqlCharset {
    fn default() -> Self {
        Self("utf8mb4".into())
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(transparent)]
pub struct SomeTimeOut(pub u32);

impl From<i32> for SomeTimeOut {
    fn from(value: i32) -> Self {
        SomeTimeOut(value as u32)
    }
}

impl From<SomeTimeOut> for std::time::Duration {
    fn from(value: SomeTimeOut) -> Self {
        std::time::Duration::from_millis(value.0 as u64)
    }
}

impl MysqlDBConfig {
    pub(crate) fn schema_str(&self) -> &str {
        self.db.as_deref().expect("must be some")
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct LibSqlDBConfig {
    pub replica: bool,
    #[serde(flatten)]
    pub source: LibSqlDBConfigSource,
    pub read_only: bool,
    pub conn_timeout: Option<SomeTimeOut>,
}
#[derive(Clone, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum LibSqlDBConfigSource {
    Local {
        path: String,
    },
    Remote {
        url: String,
        auth_token: String,
        path: Option<String>,
    },
}

impl std::fmt::Debug for LibSqlDBConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibSqlDBConfigSource::Local { path } => write!(f, "{}", path),
            LibSqlDBConfigSource::Remote { url, path, .. } => {
                write!(f, "url:{} path:{:?}", url, path)
            }
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]

pub struct ClickHouseConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub db: Option<String>,
    pub read_only: bool,
    pub https: bool,
    pub conn_timeout: Option<SomeTimeOut>,
}

impl ClickHouseConfig {
    pub(crate) fn db_str(&self) -> &str {
        self.db.as_deref().expect("must be some")
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct SqliteDBConfig {
    pub path: String,
    pub read_only: bool,
    pub conn_timeout: Option<SomeTimeOut>,
    pub locking_model: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct FireBirdConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db: String,
    pub read_only: bool,
    pub conn_timeout: Option<SomeTimeOut>,
    pub dialect: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct RedisDBConfig {
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,

    pub db: Option<String>,
    pub read_only: bool,
    pub enable_ssl: bool,
    pub conn_timeout: Option<SomeTimeOut>,
}

impl RedisDBConfig {
    // I used this in test and why??
    #[cfg(test)]
    fn new(
        host: String,
        port: Option<u16>,
        username: Option<String>,
        password: Option<String>,
        db: Option<String>,
    ) -> Self {
        Self {
            host,
            port,
            username,
            password,
            db,
            read_only: false,

            conn_timeout: None,
            enable_ssl: false,
        }
    }

    pub(crate) fn make_conn_string(&self) -> String {
        let mut ret = format!("redis{}://", if self.enable_ssl { "s" } else { "" });
        if let Some(username) = &self.username {
            ret.push_str(username);
            if let Some(pass) = &self.password {
                ret.push_str(format!(":{}", pass).as_str());
            }
            ret.push('@');
        }
        // host and port
        ret.push_str(self.host.as_str());
        if let Some(port) = self.port {
            ret.push_str(format!(":{}", port).as_str());
        }
        match &self.db {
            Some(db) => {
                ret.push_str(format!("/{}", &db[2..]).as_str());
            }
            None => {
                ret.push('/');
            }
        }
        ret
    }
}

#[cfg(test)]
#[test]
fn test_redis_make_conn_string() {
    assert_eq!(
        RedisDBConfig::new("127.0.0.1".to_string(), Some(111), None, None, None).make_conn_string(),
        "redis://127.0.0.1:111/"
    );
    assert_eq!(
        RedisDBConfig::new("127.0.0.1".to_string(), None, None, None, None).make_conn_string(),
        "redis://127.0.0.1/"
    );
    assert_eq!(
        RedisDBConfig::new(
            "127.0.0.1".to_string(),
            None,
            Some("aaa".to_owned()),
            Some("bbb".to_owned()),
            None
        )
        .make_conn_string(),
        "redis://aaa:bbb@127.0.0.1/"
    );
    assert_eq!(
        RedisDBConfig::new(
            "127.0.0.1".to_string(),
            None,
            Some("aaa".to_owned()),
            None,
            None
        )
        .make_conn_string(),
        "redis://aaa@127.0.0.1/"
    );

    assert_eq!(
        RedisDBConfig::new(
            "127.0.0.1".to_string(),
            None,
            Some("aaa".to_owned()),
            None,
            Some("db1".to_owned())
        )
        .make_conn_string(),
        "redis://aaa@127.0.0.1/1"
    );
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct PGDBConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,

    pub db: Option<String>,
    pub schema: Option<String>,
    pub read_only: bool,
    #[serde(flatten)]
    pub ssl: SslFilesWithStrModel,
    pub conn_timeout: Option<SomeTimeOut>,
    // https://docs.rs/sqlx/latest/sqlx/postgres/struct.PgConnectOptions.html#method.options
    pub options: HashMap<String, String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct SslFilesWithStrModel {
    pub ssl_mode: String,
    #[serde(flatten)]
    pub ssl_files: SSLFiles,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, specta::Type)]
pub struct SSLFiles {
    pub ssl_ca: Option<String>,
    pub ssl_cert: Option<String>,
    pub ssl_key: Option<String>,
}

impl PGDBConfig {
    pub(crate) fn db_str(&self) -> &str {
        self.db.as_ref().expect("must be some").as_str()
    }
    pub(crate) fn schema_str(&self) -> &str {
        self.schema.as_ref().expect("must be some")
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]

pub struct SqlServerDBConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub db: Option<String>,
    pub trust_cert: bool,
    pub schema: Option<String>,
    pub read_only: bool,
    pub conn_timeout: Option<SomeTimeOut>,
    pub cert_ca: Option<String>,
}

impl SqlServerDBConfig {
    pub(crate) fn db_str(&self) -> anyhow::Result<&String> {
        let x = self.db.as_ref().option_must_be_some()?;
        anyhow::Ok(x)
    }

    pub(crate) fn schema_str(&self) -> anyhow::Result<&String> {
        let x = self.schema.as_ref().option_must_be_some()?;
        anyhow::Ok(x)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct OracleDBConfig {
    pub username: String,
    pub password: String,
    pub host: String,
    pub port: u16,
    pub schema: Option<String>,
    pub conn_timeout: Option<SomeTimeOut>,
    pub read_only: bool,
    pub db: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct CassandraDBConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub db: Option<String>,
    pub read_only: bool,
    pub ssl_enable: bool,
    pub ssl_ca_file: Option<String>,
    pub conn_timeout: Option<SomeTimeOut>,
    pub dialect: CassandraDialect,
    pub consistency: CassandraConsistency,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(transparent)]
pub struct CassandraConsistency(pub u16);

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub enum CassandraDialect {
    Cassandra,
    Scylla,
}

impl CassandraDBConfig {
    pub(crate) fn key_space_str(&self) -> anyhow::Result<&str> {
        let r = self.db.as_deref().option_must_be_some()?;
        anyhow::Ok(r)
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct BigQueryDbConfig {
    pub dataset_id: Option<String>,
    pub credentials_file: String,
    pub read_only: bool,
    pub conn_timeout: Option<SomeTimeOut>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct Db2Config {
    #[serde(flatten)]
    pub odbc_common: OdbcCommon,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db: String,

    pub read_only: bool,
    pub conn_timeout: Option<SomeTimeOut>,
    pub schema: Option<String>,
}

impl Db2Config {
    #[allow(unused)]
    pub(crate) fn schema_str(&self) -> &str {
        self.schema.as_ref().expect("must be some")
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct OdbcCommon {
    pub driver_name: String,
    pub max_str_len: u32,
    pub max_bin_len: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CouchBaseDBConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub read_only: bool,
    pub bucket: Option<String>,
    pub scope: Option<String>,
    pub conn_timeout: Option<SomeTimeOut>,

    pub ssl_enable: bool,
    pub danger_accept_invalid_hostnames: Option<bool>,
    pub danger_accept_invalid_certificates: Option<bool>,
    pub ca_certificates: Vec<String>,
}

impl CouchBaseDBConfig {
    pub(crate) fn conn_str(&self) -> String {
        // 根据是否启用 SSL 选择协议前缀
        let protocol = if self.ssl_enable {
            "couchbases://"
        } else {
            "couchbase://"
        };

        // 构建连接地址: host:port
        format!("{}{}:{}", protocol, self.host, self.port)
    }
}

impl CouchBaseDBConfig {
    pub(crate) fn scope_str(&self) -> &str {
        self.scope.as_deref().expect("must be some")
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]

pub struct Neo4JDbConfig {
    pub uri: String,
    pub username: String,
    pub password: String,
    pub read_only: bool,
    pub db: Option<String>,
    pub client_certificate: Option<String>,
}

#[cfg(test)]
impl SSLFiles {
    pub fn test_cert() -> Self {
        Self {
            ssl_ca: Some(Self::test_cert_append_path("ca.crt")),
            ssl_cert: Some(Self::test_cert_append_path("client.crt")),
            ssl_key: Some(Self::test_cert_append_path("client.key")),
        }
    }

    pub(crate) fn test_cert_append_path(x: &str) -> String {
        let path = cert_files_dir();
        use crate::helper::cert_files_dir;
        use std::path::PathBuf;
        let mut p = PathBuf::from(&path);
        p.push(x);
        p.to_str().unwrap().to_owned()
    }
}
