#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DBConfig {
    Sqlite {
        path: String,
    },
    Mysql {
        conn: CommDBConfig,
    },
    PostGres {
        conn: CommDBConfig,
    },
    Redis {
        server: String,
    },
    MongoDB(MongoDataSource),
    SqlServer {
        config: CommDBConfig,
        trust_cert: bool,
    },
    Oracle(OracleDBConfig),
}
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MongoDataSource {
    pub host: String,
    pub port: u16,
    pub username_and_password: Option<(String, String)>,
}

/// username password host and port database name
/// packed together.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommDBConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub db_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct OracleDBConfig {
    pub username: String,
    pub password: String,
    pub connect_str: String,
}
