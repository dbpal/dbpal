use typescript_type_def::TypeDef;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct MongoDBConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub save_password: bool,
    /// milliseconds
    pub call_timeout: Option<u64>,
    pub db: Option<String>,
    pub read_only: bool,
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
    pub ssl: EnableSsl,
}

impl MysqlDBConfig {
    pub(crate) fn db_str(&self) -> &str {
        self.db.as_ref().map(|x| x.as_str()).expect("must be some")
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct LibSqlDBConfig {
    pub replica: bool,
    #[serde(flatten)]
    pub source: LibSqlDBConfigSouce,
    pub read_only: bool,
}
#[derive(Clone, serde::Deserialize, serde::Serialize, TypeDef)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum LibSqlDBConfigSouce {
    Local {
        path: String,
    },
    Remote {
        url: String,
        auth_token: String,
        path: String,
    },
}

impl std::fmt::Debug for LibSqlDBConfigSouce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LibSqlDBConfigSouce::Local { path } => write!(f, "{}", path),
            LibSqlDBConfigSouce::Remote { url, path, .. } => write!(f, "url:{} path:{}", url, path),
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
}

impl ClickHouseConfig {
    pub(crate) fn db_str(&self) -> &str {
        self.db.as_ref().map(|x| x.as_str()).expect("must be some")
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct SqliteDBConfig {
    pub path: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct DuckDBConfig {
    pub path: String,
    pub read_only: bool,
    pub db: Option<String>,
    pub schema: Option<String>,
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
        }
    }

    pub(crate) fn make_conn_string(&self) -> String {
        let mut ret = "redis://".to_owned();
        match &self.username {
            Some(username) => {
                ret.push_str(&username);
                match &self.password {
                    Some(pass) => {
                        ret.push_str(format!(":{}", pass).as_str());
                    }
                    None => {}
                }
                ret.push('@');
            }
            None => {}
        }
        // host and port
        ret.push_str(self.host.as_str());
        match self.port {
            Some(port) => {
                ret.push_str(format!(":{}", port).as_str());
            }
            None => {}
        }
        match &self.db {
            Some(db) => {
                ret.push_str(format!("/{}", &db[2..]).as_str());
            }
            None => {
                ret.push_str("/");
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
    pub ssl: EnableSsl,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef, Default)]
pub struct EnableSsl {
    #[serde(default)]
    pub ssl_enable: bool,
    #[serde(default)]
    pub ssl_ca: Option<String>,
    #[serde(default)]
    pub ssl_cert: Option<String>,
    #[serde(default)]
    pub ssl_key: Option<String>,
}

impl PGDBConfig {
    pub(crate) fn db_str(&self) -> &str {
        self.db.as_ref().unwrap().as_str()
    }
    pub(crate) fn schema_str(&self) -> &String {
        self.schema.as_ref().expect("must be some")
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
}

impl SqlServerDBConfig {
    pub(crate) fn must_db(&self) -> &String {
        self.db.as_ref().unwrap()
    }

    pub(crate) fn must_schema(&self) -> &String {
        self.schema.as_ref().unwrap()
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
    // millisecond.
    pub call_timeout: Option<u64>,
    pub read_only: bool,
    pub db: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, typescript_type_def::TypeDef)]
pub struct CassandraDBConfig {
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub save_password: bool,
    pub db: Option<String>,
    pub read_only: bool,
}

impl CassandraDBConfig {
    pub(crate) fn key_space_str(&self) -> &str {
        self.db.as_deref().unwrap()
    }
}
