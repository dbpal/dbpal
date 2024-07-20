use typescript_type_def::TypeDef;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DBPoolKey {
    Sqlite {
        path: String,
    },
    Mysql {
        conn: MysqlDBConfig,
    },
    PostGres {
        conn: PGDBConfig,
    },
    Redis {
        server: String,
    },
    MongoDB(MongoDataSource),
    SqlServer {
        config: SqlServerDBConfig,
        trust_cert: bool,
    },
    Oracle(OracleDBConfig),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize, TypeDef)]
pub struct MongoDataSource {
    pub host: String,
    pub port: u16,
    pub username_and_password: Option<(String, String)>,
    /// milliseconds
    pub connect_timeout: Option<u64>,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Deserialize,
    serde::Serialize,
    typescript_type_def::TypeDef,
)]
pub struct MysqlDBConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db_name: Option<String>,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Deserialize,
    serde::Serialize,
    typescript_type_def::TypeDef,
)]
pub struct PGDBConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db_name: String,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    serde::Deserialize,
    serde::Serialize,
    typescript_type_def::TypeDef,
)]

pub struct SqlServerDBConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db_name: String,
    pub trust_cert: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize, TypeDef)]
pub struct OracleDBConfig {
    pub username: String,
    pub password: String,
    pub connect_str: String,
}
