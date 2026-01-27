use dbnexus_db::datasource_config::{
    CassandraDBConfig, ClickHouseConfig, CouchBaseDBConfig, Db2Config, DuckDBConfig,
    FireBirdConfig, LibSqlDBConfig, MongoDBConfig, MysqlDBConfig, OracleDBConfig, PGDBConfig,
    RedisDBConfig, SqlServerDBConfig, SqliteDBConfig,
};
use dbnexus_db::db::{
    CatalogTrait, CheckConn, DBTrait, ExportQueryResult, QuerySql, SchemaTrait, ServerMetas,
};
use dbnexus_db::language_server::LanguageServer;
use dbnexus_db::mongo::MongoDB;
use dbnexus_db::sub::SubDB;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use typescript_type_def::TypeDef;
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

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, TypeDef)]
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
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(dbnexus_db::mysql::MysqlDB::new(conn.clone()).await?)
                            as Arc<dyn SchemaTrait>;
                    anyhow::Ok(DatabaseBackend::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(dbnexus_db::mysql::MysqlDB::new(conn.clone()).await?)
                            as Arc<dyn DBTrait>;
                    anyhow::Ok(DatabaseBackend::DataBase(db))
                }
            },
            DBConnectionConfig::MariaDB { conn, .. } => match &conn.db {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(SubDB::new_mariadb(conn.clone()).await?) as Arc<dyn SchemaTrait>;
                    anyhow::Ok(DatabaseBackend::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(SubDB::new_mariadb(conn.clone()).await?) as Arc<dyn DBTrait>;
                    anyhow::Ok(DatabaseBackend::DataBase(db))
                }
            },
            DBConnectionConfig::Tidb { conn, .. } => match &conn.db {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(SubDB::new_tidb(conn.clone()).await?) as Arc<dyn SchemaTrait>;
                    anyhow::Ok(DatabaseBackend::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(SubDB::new_tidb(conn.clone()).await?) as Arc<dyn DBTrait>;
                    anyhow::Ok(DatabaseBackend::DataBase(db))
                }
            },
            DBConnectionConfig::Postgres { conn, .. } => {
                let o = Arc::new(dbnexus_db::pg::PostGresDB::new(conn.clone()).await?);
                if conn.db.is_none() {
                    return anyhow::Ok(DatabaseBackend::Catalog(o as Arc<dyn CatalogTrait>));
                }

                match &conn.schema {
                    None => anyhow::Ok(DatabaseBackend::DataBase(o as Arc<dyn DBTrait>)),
                    Some(_) => anyhow::Ok(DatabaseBackend::Schema(o as Arc<dyn SchemaTrait>)),
                }
            }
            DBConnectionConfig::CockRoach { conn, .. } => {
                let o = Arc::new(SubDB::new_cockroach(conn.clone()).await?);
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
                        Arc::new(dbnexus_db::oracle::OracleDB::new(conn.clone(), None)?)
                            as Arc<dyn SchemaTrait>;
                    anyhow::Ok(DatabaseBackend::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(dbnexus_db::oracle::OracleDB::new(conn.clone(), None)?)
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
                    let db = dbnexus_db::duckdb::DuckDB::new(_conn.clone())?;
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
    /// database can contains multi database
    DataBase(Arc<dyn dbnexus_db::db::DBTrait>),
    /// a schema
    Schema(Arc<dyn dbnexus_db::db::SchemaTrait>),
}

impl DatabaseBackend {
    pub(crate) async fn check_conn(&self) -> anyhow::Result<Duration> {
        self.as_check_conn().check_conn().await
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
                let db_name = format!("{}/{}", &name_parts[0], &name_parts[1]);
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

    pub async fn conn_opened(&self, name: &str) -> bool {
        let x = name_to_parts(name);
        self.backend.lock().await.contains_key(&x[0].to_string())
    }
}

#[allow(dead_code)]
pub(crate) fn generate_random_string(len: usize) -> String {
    use rand::prelude::*;
    // 定义只包含字母和数字的字符集
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                              abcdefghijklmnopqrstuvwxyz\
                              0123456789";

    let mut rng = rand::rng();
    let rand_string: String = (0..len)
        .map(|_| {
            let idx = rng.random_range(0usize..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    rand_string
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
