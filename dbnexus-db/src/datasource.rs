use typescript_type_def::TypeDef;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DBPoolKey {
    Sqlite { path: String },
    Mysql { conn: MysqlDBConfig },
    PostGres { conn: PGDBConfig },
    Redis { server: String },
    MongoDB(MongoDataSource),
    // sql server don't use pool
    // SqlServer { config: SqlServerDBConfig },
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
    pub db_name: Option<String>,
}

impl PGDBConfig {
    pub(crate) fn db_str(&self) -> &str {
        self.db_name.as_ref().expect("must be some").as_str()
    }
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
