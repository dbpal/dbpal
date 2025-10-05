use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use dbnexus_db::datasource_config::{
    CassandraDBConfig, ClickHouseConfig, DuckDBConfig, FireBirdConfig, LibSqlDBConfig,
    MongoDBConfig, MysqlDBConfig, OracleDBConfig, PGDBConfig, RedisDBConfig, SqlServerDBConfig,
    SqliteDBConfig,
};
use dbnexus_db::db::{CatalogTrait, DBTrait, SchemaTrait};
use dbnexus_db::helper::NextId;
use dbnexus_db::mongo::MongoDB;
use rand::Rng;
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
            | DBConnectionConfig::MsSql { name, .. }
            | DBConnectionConfig::Mongo { name, .. }
            | DBConnectionConfig::Redis { name, .. }
            | DBConnectionConfig::Oracle { name, .. }
            | DBConnectionConfig::Cassandra { name, .. }
            | DBConnectionConfig::ClickHouse { name, .. }
            | DBConnectionConfig::LibSql { name, .. }
            | DBConnectionConfig::FireBird { name, .. }
            | DBConnectionConfig::DuckDB { name, .. }
            | DBConnectionConfig::CockRoach { name, .. } => name.as_str(),
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
    },
    MariaDB {
        #[serde(flatten)]
        conn: MysqlDBConfig,
        name: String,
    },
    Tidb {
        #[serde(flatten)]
        conn: MysqlDBConfig,
        name: String,
    },
    Postgres {
        #[serde(flatten)]
        conn: PGDBConfig,
        name: String,
    },
    CockRoach {
        #[serde(flatten)]
        conn: PGDBConfig,
        name: String,
    },
    MsSql {
        #[serde(flatten)]
        conn: SqlServerDBConfig,
        name: String,
    },
    Mongo {
        #[serde(flatten)]
        conn: MongoDBConfig,
        name: String,
    },
    Redis {
        #[serde(flatten)]
        conn: RedisDBConfig,
        name: String,
    },
    Oracle {
        #[serde(flatten)]
        conn: OracleDBConfig,
        name: String,
    },
    Cassandra {
        #[serde(flatten)]
        conn: CassandraDBConfig,
        name: String,
    },
    ClickHouse {
        #[serde(flatten)]
        conn: ClickHouseConfig,
        name: String,
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
    },
    DuckDB {
        #[serde(flatten)]
        conn: DuckDBConfig,
        name: String,
    },
}

impl DBConnectionConfig {
    pub fn need_password(&mut self) -> bool {
        match self {
            DBConnectionConfig::Sqlite { .. } => false,
            DBConnectionConfig::DuckDB { .. } => false,
            DBConnectionConfig::LibSql { .. } => false,
            _ => true,
        }
    }

    pub fn set_password(&mut self, pass: String) {
        match self {
            DBConnectionConfig::Sqlite { .. } => {}
            DBConnectionConfig::Mysql { conn, .. } => {
                conn.password = Some(pass);
            }
            DBConnectionConfig::MariaDB { conn, .. } => {
                conn.password = Some(pass);
            }
            DBConnectionConfig::Tidb { conn, .. } => {
                conn.password = Some(pass);
            }
            DBConnectionConfig::Postgres { conn, .. } => {
                conn.password = Some(pass);
            }
            DBConnectionConfig::MsSql { conn, .. } => {
                conn.password = Some(pass);
            }
            DBConnectionConfig::Mongo { conn, .. } => {
                conn.password = Some(pass);
            }
            DBConnectionConfig::Redis { conn, .. } => {
                conn.password = Some(pass);
            }
            DBConnectionConfig::Oracle { conn, .. } => {
                conn.password = pass;
            }
            DBConnectionConfig::FireBird { conn, .. } => {
                conn.password = pass;
            }
            DBConnectionConfig::Cassandra { conn, .. } => conn.password = Some(pass),
            DBConnectionConfig::DuckDB { .. } => {}
            DBConnectionConfig::ClickHouse { conn, .. } => conn.password = Some(pass),
            DBConnectionConfig::LibSql { .. } => {}
            DBConnectionConfig::CockRoach { conn, .. } => {
                conn.password = Some(pass);
            }
        }
    }
}

impl DBConnectionConfig {
    pub async fn open(&self) -> anyhow::Result<DatabaseBackend> {
        match self {
            DBConnectionConfig::Sqlite { conn, name: _ } => {
                let db: Arc<dyn SchemaTrait> = Arc::new(
                    dbnexus_db::sqlite::SqliteDB::new(conn.clone(), conn.read_only).await?,
                );
                anyhow::Ok(DatabaseBackend::Schema(db))
            }
            DBConnectionConfig::Mysql { conn, name: _ } => match &conn.db {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(dbnexus_db::mysql::MysqlDB::new(conn.clone()).await?);
                    anyhow::Ok(DatabaseBackend::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(dbnexus_db::mysql::MysqlDB::new(conn.clone()).await?);
                    anyhow::Ok(DatabaseBackend::DataBase(db))
                }
            },
            DBConnectionConfig::MariaDB { conn, name: _ } => match &conn.db {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(dbnexus_db::mariadb::MariaDB::new(conn.clone()).await?);
                    anyhow::Ok(DatabaseBackend::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> = Arc::new({
                        let x = dbnexus_db::mariadb::MariaDB::new(conn.clone()).await?;
                        x
                    });
                    anyhow::Ok(DatabaseBackend::DataBase(db))
                }
            },
            DBConnectionConfig::Tidb { conn, name: _ } => match &conn.db {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(dbnexus_db::tidb::TiDB::new(conn.clone()).await?);

                    anyhow::Ok(DatabaseBackend::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(dbnexus_db::tidb::TiDB::new(conn.clone()).await?);
                    anyhow::Ok(DatabaseBackend::DataBase(db))
                }
            },
            DBConnectionConfig::Postgres { conn, name: _ } => {
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
                let o = Arc::new(dbnexus_db::cockroach::CockRoachDB::new(conn.clone()).await?);
                if conn.db.is_none() {
                    return anyhow::Ok(DatabaseBackend::Catalog(o as Arc<dyn CatalogTrait>));
                }
                match &conn.schema {
                    None => anyhow::Ok(DatabaseBackend::DataBase(o as Arc<dyn DBTrait>)),
                    Some(_) => anyhow::Ok(DatabaseBackend::Schema(o as Arc<dyn SchemaTrait>)),
                }
            }
            DBConnectionConfig::MsSql { conn, name: _ } => {
                let o = Arc::new(dbnexus_db::mssql::SqlServerDB::new(conn.clone()).await?);
                if conn.db.is_none() {
                    return anyhow::Ok(DatabaseBackend::Catalog(o as Arc<dyn CatalogTrait>));
                }
                match &conn.schema {
                    Some(_) => anyhow::Ok(DatabaseBackend::Schema(o as Arc<dyn SchemaTrait>)),
                    None => anyhow::Ok(DatabaseBackend::DataBase(o as Arc<dyn DBTrait>)),
                }
            }
            DBConnectionConfig::Mongo { conn, name: _ } => {
                let db: Arc<MongoDB> = Arc::new(dbnexus_db::mongo::MongoDB::new(conn.clone())?);
                match conn.db {
                    Some(_) => anyhow::Ok(DatabaseBackend::Schema(db)),
                    None => anyhow::Ok(DatabaseBackend::DataBase(db)),
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
            DBConnectionConfig::Oracle { conn, name: _ } => match conn.schema.as_ref() {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(dbnexus_db::oracle::OracleDB::new(conn.clone(), None)?);
                    anyhow::Ok(DatabaseBackend::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(dbnexus_db::oracle::OracleDB::new(conn.clone(), None)?);
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
                    return match &_conn.db {
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
                    };
                }
                #[cfg(not(feature = "duckdb"))]
                anyhow::bail!("need enable duckdb support")
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
                let db = dbnexus_db::firebird::FireBirdDB::new(conn.clone())?;
                anyhow::Ok(DatabaseBackend::from(Arc::new(db) as Arc<dyn SchemaTrait>))
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
        match self {
            DatabaseBackend::Catalog(x) => x.check_conn().await,
            DatabaseBackend::DataBase(x) => x.check_conn().await,
            DatabaseBackend::Schema(x) => x.check_conn().await,
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

/// all the databases
#[derive(Clone, Default)]
pub struct DBPal {
    pub backend: Arc<Mutex<HashMap<String, DatabaseBackend>>>,
    pub saved_password: Arc<DashMap<String, String>>,
    pub backend_crash_msg: Arc<DashMap<u32, String>>,
    pub backend_crash_next_id: NextId,
}

impl DBPal {
    pub fn new() -> Self {
        Default::default()
    }

    #[allow(unused)]
    pub(crate) fn set_backend_crash_msg(&self, msg: String) -> u32 {
        let id = self.backend_crash_next_id.next();
        self.backend_crash_msg.insert(id, msg);
        id
    }
}
type GetConfigFN = Box<dyn Fn(&str) -> anyhow::Result<DBConnectionConfig>>;

pub(crate) fn name_to_parts(name: &str) -> Vec<&str> {
    name.split("/").collect()
}

fn name_starts(name: &str, prefix: &str) -> bool {
    let name = name_to_parts(name);
    let prefix = name_to_parts(prefix);

    name.len() >= prefix.len() && name.iter().zip(prefix.iter()).all(|(x, y)| *x == *y)
}

impl DBPal {
    pub unsafe fn get_config_ref() -> &'static mut Option<GetConfigFN> {
        static mut GET_CONFIG_FN: Option<GetConfigFN> = None;
        #[allow(static_mut_refs)] // this var will init once by main
        &mut GET_CONFIG_FN
    }

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
            _ => anyhow::bail!(format!("'{}' a schema trait", name)),
        }
    }

    pub async fn open_conn(&self, name: &str) -> anyhow::Result<DatabaseBackend> {
        let config = unsafe { Self::get_config_ref().as_ref().map(|x| x(name)) }.unwrap()?;
        #[cfg(debug_assertions)]
        println!("open config:{:?}", config);
        let db = config.open().await?;
        // make sure connection is ok
        db.check_conn().await?;
        self.backend
            .lock()
            .await
            .insert(name.to_string(), db.clone());
        return anyhow::Ok(db);
    }

    pub async fn get_backend(&self, name: &str) -> anyhow::Result<DatabaseBackend> {
        let db = self.backend.lock().await.get(name).cloned();
        if db.is_some() {
            return anyhow::Ok(db.unwrap());
        }
        // try open and enter
        let name_parts = name_to_parts(name);
        match name_parts.len() {
            1 => {
                anyhow::bail!(format!("connection '{}' not open", name))
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
                        let schema = x.open_schema(&name_parts[1]).await?;
                        self.backend
                            .lock()
                            .await
                            .insert(name.to_string(), schema.clone().into());
                        anyhow::Ok(DatabaseBackend::Schema(schema))
                    }
                    Some(DatabaseBackend::Catalog(x)) => {
                        let db = x.open_db(&name_parts[1]).await?;
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
                        anyhow::bail!(format!("connection '{}' not open", name))
                    }
                }
            }
            3 => {
                let db_name = format!("{}/{}", &name_parts[0], &name_parts[1]);
                let open = self.backend.lock().await.get(&db_name).cloned();
                let db_trait = match open {
                    Some(x) => match x {
                        DatabaseBackend::DataBase(x) => x.clone(),
                        _ => {
                            anyhow::bail!("this must be db trait")
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
                                    let db = x.open_db(&name_parts[1]).await?;
                                    self.backend.lock().await.insert(db_name, db.clone().into());
                                    db
                                }
                                _ => {
                                    log::error!("internal error,catalog must been entered ");
                                    anyhow::bail!(format!("connection '{}' not open", name))
                                }
                            },
                            None => {
                                log::error!("internal error,catalog must been entered");
                                anyhow::bail!(format!("connection '{}' not open", name))
                            }
                        }
                    }
                };
                let schema = db_trait.open_schema(&name_parts[2]).await?;
                self.backend
                    .lock()
                    .await
                    .insert(name.to_string(), schema.clone().into());
                anyhow::Ok(DatabaseBackend::Schema(schema))
            }
            _ => {
                // nothing to do here
                log::error!("'{}' name part not right, this is a internal error", name);
                anyhow::bail!(format!("connection '{}' not open", name))
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
    // 定义只包含字母和数字的字符集
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                              abcdefghijklmnopqrstuvwxyz\
                              0123456789";

    let mut rng = rand::thread_rng();
    let rand_string: String = (0..len)
        .map(|_| {
            let idx = rng.random_range(0..CHARSET.len());
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
