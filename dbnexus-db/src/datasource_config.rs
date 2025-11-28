use typescript_type_def::TypeDef;

use crate::db::OptionMustBeSomeErr;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct MongoDBConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub save_password: bool,
    pub conn_timeout: Option<ConnTimeOut>,
    pub db: Option<String>,
    pub read_only: bool,
    #[serde(flatten)]
    pub ssl: MongoSsl,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef, Default)]
pub struct MongoSsl {
    pub enable_ssl: bool,
    #[serde(flatten)]
    pub ssl_files: SSLFiles,
}
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct MysqlDBConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub save_password: bool,
    pub db: Option<String>,
    pub read_only: bool,
    #[serde(flatten)]
    pub ssl: MysqlSsl,
    pub conn_timeout: Option<ConnTimeOut>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, TypeDef)]
#[serde(transparent)]
pub struct ConnTimeOut(pub u32);

impl From<i32> for ConnTimeOut {
    fn from(value: i32) -> Self {
        ConnTimeOut(value as u32)
    }
}

impl From<ConnTimeOut> for std::time::Duration {
    fn from(value: ConnTimeOut) -> Self {
        std::time::Duration::from_millis(value.0 as u64)
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef, Default)]
pub struct MysqlSsl {
    pub ssl_mode: MySqlSslMode,
    #[serde(flatten)]
    pub ssl_files: SSLFiles,
}

impl MysqlDBConfig {
    pub(crate) fn schema_str(&self) -> anyhow::Result<&str> {
        let r = self.db.as_deref().option_must_be_some()?;
        anyhow::Ok(r)
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct LibSqlDBConfig {
    pub replica: bool,
    #[serde(flatten)]
    pub source: LibSqlDBConfigSource,
    pub read_only: bool,
    pub conn_timeout: Option<ConnTimeOut>,
}
#[derive(Clone, serde::Deserialize, serde::Serialize, TypeDef)]
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

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]

pub struct ClickHouseConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub save_password: bool,
    pub db: Option<String>,
    pub read_only: bool,
    pub https: bool,
    pub conn_timeout: Option<ConnTimeOut>,
}

impl ClickHouseConfig {
    pub(crate) fn db_str(&self) -> &str {
        self.db.as_deref().expect("must be some")
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct SqliteDBConfig {
    pub path: String,
    pub read_only: bool,
    pub conn_timeout: Option<ConnTimeOut>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct DuckDBConfig {
    pub path: String,
    pub read_only: bool,
    pub db: Option<String>,
    pub schema: Option<String>,
    pub conn_timeout: Option<ConnTimeOut>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct FireBirdConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub save_password: bool,
    pub db: String,
    pub read_only: bool,
    pub conn_timeout: Option<ConnTimeOut>,
}

impl DuckDBConfig {
    #[allow(unused)]
    pub(crate) fn schema_str(&self) -> &str {
        self.schema.as_ref().expect("must be some")
    }

    #[allow(unused)]
    pub(crate) fn db_str(&self) -> &str {
        self.db.as_ref().expect("must be some")
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct RedisDBConfig {
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub save_password: bool,
    pub db: Option<String>,
    pub read_only: bool,
    pub enable_ssl: bool,
    pub conn_timeout: Option<ConnTimeOut>,
}

impl RedisDBConfig {
    // I used this in test and why??
    #[cfg(test)]
    fn new(
        host: String, port: Option<u16>, username: Option<String>, password: Option<String>,
        db: Option<String>,
    ) -> Self {
        Self {
            host,
            port,
            username,
            password,
            db,
            read_only: false,
            save_password: true,
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

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct PGDBConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub save_password: bool,
    pub db: Option<String>,
    pub schema: Option<String>,
    pub read_only: bool,
    #[serde(flatten)]
    pub ssl: PgSsl,
    pub conn_timeout: Option<ConnTimeOut>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef, Default)]
pub struct PgSsl {
    pub ssl_mode: PgSslMode,
    #[serde(flatten)]
    pub ssl_files: SSLFiles,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default, TypeDef)]
pub struct SSLFiles {
    pub ssl_ca: Option<String>,
    pub ssl_cert: Option<String>,
    pub ssl_key: Option<String>,
}

impl PGDBConfig {
    pub(crate) fn db_str(&self) -> anyhow::Result<&str> {
        let r = self.db.as_ref().option_must_be_some()?.as_str();
        anyhow::Ok(r)
    }
    pub(crate) fn schema_str(&self) -> anyhow::Result<&str> {
        let r = self.schema.as_ref().option_must_be_some()?.as_str();
        anyhow::Ok(r)
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]

pub struct SqlServerDBConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub save_password: bool,
    pub db: Option<String>,
    pub trust_cert: bool,
    pub schema: Option<String>,
    pub read_only: bool,
    pub conn_timeout: Option<ConnTimeOut>,
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

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, TypeDef)]
pub struct OracleDBConfig {
    pub username: String,
    pub password: String,
    pub save_password: bool,
    pub host: String,
    pub port: u16,
    pub schema: Option<String>,
    pub conn_timeout: Option<ConnTimeOut>,
    pub read_only: bool,
    pub db: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct CassandraDBConfig {
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub save_password: bool,
    pub db: Option<String>,
    pub read_only: bool,
    pub ssl: CassandraSsl,
    pub conn_timeout: Option<ConnTimeOut>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct CassandraSsl {
    #[serde(flatten)]
    pub ssl_files: SSLFiles,
}

impl CassandraDBConfig {
    pub(crate) fn key_space_str(&self) -> anyhow::Result<&str> {
        let r = self.db.as_deref().option_must_be_some()?;
        anyhow::Ok(r)
    }
}

// this struct copied from sqlx postgres
#[derive(Clone, Copy, Default, Debug, serde::Deserialize, serde::Serialize, TypeDef)]
#[serde(rename_all = "lowercase")]
pub enum PgSslMode {
    /// Only try a non-SSL connection.
    ///
    Disable,

    /// First try a non-SSL connection; if that fails, try an SSL connection.
    Allow,

    /// First try an SSL connection; if that fails, try a non-SSL connection.
    ///
    /// This is the default if no other mode is specified
    #[default]
    Prefer,

    /// Only try an SSL connection. If a root CA file is present, verify the connection
    /// in the same way as if `VerifyCa` was specified.
    Require,

    /// Only try an SSL connection, and verify that the server certificate is issued by a
    /// trusted certificate authority (CA).
    VerifyCa,

    /// Only try an SSL connection; verify that the server certificate is issued by a trusted
    /// CA and that the requested server host name matches that in the certificate.
    VerifyFull,
}

impl From<PgSslMode> for sqlx::postgres::PgSslMode {
    fn from(x: PgSslMode) -> Self {
        match x {
            PgSslMode::Disable => sqlx::postgres::PgSslMode::Disable,
            PgSslMode::Allow => sqlx::postgres::PgSslMode::Allow,
            PgSslMode::Prefer => sqlx::postgres::PgSslMode::Prefer,
            PgSslMode::Require => sqlx::postgres::PgSslMode::Require,
            PgSslMode::VerifyCa => sqlx::postgres::PgSslMode::VerifyCa,
            PgSslMode::VerifyFull => sqlx::postgres::PgSslMode::VerifyFull,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde ::Deserialize, TypeDef)]
#[serde(rename_all = "lowercase")]
pub enum MySqlSslMode {
    /// Establish an unencrypted connection.
    Disabled,

    /// Establish an encrypted connection if the server supports encrypted connections, falling
    /// back to an unencrypted connection if an encrypted connection cannot be established.
    ///
    /// This is the default if `ssl_mode` is not specified.
    #[default]
    Preferred,

    /// Establish an encrypted connection if the server supports encrypted connections.
    /// The connection attempt fails if an encrypted connection cannot be established.
    Required,

    /// Like `Required`, but additionally verify the server Certificate Authority (CA)
    /// certificate against the configured CA certificates. The connection attempt fails
    /// if no valid matching CA certificates are found.
    VerifyCa,

    /// Like `VerifyCa`, but additionally perform host name identity verification by
    /// checking the host name the client uses for connecting to the server against the
    /// identity in the certificate that the server sends to the client.
    VerifyIdentity,
}

impl From<MySqlSslMode> for sqlx::mysql::MySqlSslMode {
    fn from(value: MySqlSslMode) -> Self {
        match value {
            MySqlSslMode::Disabled => sqlx::mysql::MySqlSslMode::Disabled,
            MySqlSslMode::Preferred => sqlx::mysql::MySqlSslMode::Preferred,
            MySqlSslMode::Required => sqlx::mysql::MySqlSslMode::Required,
            MySqlSslMode::VerifyCa => sqlx::mysql::MySqlSslMode::VerifyCa,
            MySqlSslMode::VerifyIdentity => sqlx::mysql::MySqlSslMode::VerifyIdentity,
        }
    }
}
