use typescript_type_def::TypeDef;

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct MongoDataSource {
    pub host: String,
    pub port: u16,
    pub username_and_password: Option<(String, String)>,
    /// milliseconds
    pub connect_timeout: Option<u64>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, typescript_type_def::TypeDef)]
pub struct MysqlDBConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db_name: Option<String>,
}
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, typescript_type_def::TypeDef)]
pub struct RedisDBConfig {
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub db: Option<String>,
}
impl RedisDBConfig {
    #[allow(dead_code)]
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
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, typescript_type_def::TypeDef)]
pub struct PGDBConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db_name: Option<String>,
}

impl PGDBConfig {
    pub(crate) fn db_str(&self) -> &str {
        self.db_name.as_ref().expect("must be some").as_str()
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, typescript_type_def::TypeDef)]

pub struct SqlServerDBConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db_name: Option<String>,
    pub trust_cert: bool,
}
impl SqlServerDBConfig {
    #[allow(dead_code)]
    pub(crate) fn db_str(&self) -> &str {
        self.db_name.as_ref().expect("must be some").as_str()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, TypeDef)]
pub struct OracleDBConfig {
    pub username: String,
    pub password: String,
    pub connect_str: String,
}
