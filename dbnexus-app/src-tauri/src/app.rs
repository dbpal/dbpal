use std::collections::HashMap;
use std::sync::Arc;

use dashmap::DashMap;
use dbnexus_db::datasource_config::{
    CassandraDBConfig, ClickHouseConfig, DuckDBConfig, FireBirdConfig, LibSqlDBConfig,
    MongoDBConfig, MysqlDBConfig, OracleDBConfig, PGDBConfig, RedisDBConfig, SqlServerDBConfig,
    SqliteDBConfig,
};
use dbnexus_db::db::{CatalogTrait, DBTrait, SchemaTrait};
use dbnexus_db::mongo::MongoDB;
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
    pub async fn open(&self) -> anyhow::Result<Open> {
        match self {
            DBConnectionConfig::Sqlite { conn, name: _ } => {
                let db: Arc<dyn SchemaTrait> = Arc::new(
                    dbnexus_db::sqlite::SqliteDB::new(conn.clone(), conn.read_only).await?,
                );
                anyhow::Ok(Open::Schema(db))
            }
            DBConnectionConfig::Mysql { conn, name: _ } => match &conn.db {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(dbnexus_db::mysql::MysqlDB::new(conn.clone()).await?);
                    anyhow::Ok(Open::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(dbnexus_db::mysql::MysqlDB::new(conn.clone()).await?);
                    anyhow::Ok(Open::DataBase(db))
                }
            },
            DBConnectionConfig::MariaDB { conn, name: _ } => match &conn.db {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(dbnexus_db::mariadb::MariaDB::new(conn.clone()).await?);
                    anyhow::Ok(Open::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> = Arc::new({
                        let x = dbnexus_db::mariadb::MariaDB::new(conn.clone()).await?;
                        x
                    });
                    anyhow::Ok(Open::DataBase(db))
                }
            },
            DBConnectionConfig::Tidb { conn, name: _ } => match &conn.db {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(dbnexus_db::tidb::TiDB::new(conn.clone()).await?);

                    anyhow::Ok(Open::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(dbnexus_db::tidb::TiDB::new(conn.clone()).await?);
                    anyhow::Ok(Open::DataBase(db))
                }
            },
            DBConnectionConfig::Postgres { conn, name: _ } => {
                let o = Arc::new(dbnexus_db::pg::PostGresDB::new(conn.clone()).await?);
                if conn.db.is_none() {
                    return anyhow::Ok(Open::Catalog(o as Arc<dyn CatalogTrait>));
                }

                match &conn.schema {
                    None => anyhow::Ok(Open::DataBase(o as Arc<dyn DBTrait>)),
                    Some(_) => anyhow::Ok(Open::Schema(o as Arc<dyn SchemaTrait>)),
                }
            }
            DBConnectionConfig::CockRoach { conn, .. } => {
                let o = Arc::new(dbnexus_db::cockroach::CockRoachDB::new(conn.clone()).await?);
                if conn.db.is_none() {
                    return anyhow::Ok(Open::Catalog(o as Arc<dyn CatalogTrait>));
                }
                match &conn.schema {
                    None => anyhow::Ok(Open::DataBase(o as Arc<dyn DBTrait>)),
                    Some(_) => anyhow::Ok(Open::Schema(o as Arc<dyn SchemaTrait>)),
                }
            }
            DBConnectionConfig::MsSql { conn, name: _ } => {
                let o = Arc::new(dbnexus_db::mssql::MsSqlServerDB::new(conn.clone()).await?);
                if conn.db.is_none() {
                    return anyhow::Ok(Open::Catalog(o as Arc<dyn CatalogTrait>));
                }
                match &conn.schema {
                    Some(_) => anyhow::Ok(Open::Schema(o as Arc<dyn SchemaTrait>)),
                    None => anyhow::Ok(Open::DataBase(o as Arc<dyn DBTrait>)),
                }
            }
            DBConnectionConfig::Mongo { conn, name: _ } => {
                let db: Arc<MongoDB> = Arc::new(dbnexus_db::mongo::MongoDB::new(conn.clone())?);
                match conn.db {
                    Some(_) => anyhow::Ok(Open::Schema(db)),
                    None => anyhow::Ok(Open::DataBase(db)),
                }
            }
            DBConnectionConfig::Redis { conn, .. } => {
                let redis = dbnexus_db::redis::RedisDB::new(conn.clone()).await?;

                let db = match &conn.db {
                    Some(_) => Open::Schema(Arc::new(redis) as Arc<dyn SchemaTrait>),
                    None => Open::DataBase(Arc::new(redis) as Arc<dyn DBTrait>),
                };

                anyhow::Ok(db)
            }
            DBConnectionConfig::Oracle { conn, name: _ } => match conn.schema.as_ref() {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(dbnexus_db::oracle::OracleDB::new(conn.clone(), None)?);
                    anyhow::Ok(Open::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(dbnexus_db::oracle::OracleDB::new(conn.clone(), None)?);
                    anyhow::Ok(Open::DataBase(db))
                }
            },
            DBConnectionConfig::Cassandra { conn, .. } => {
                let db = dbnexus_db::cassandra::CassandraDB::new(conn.clone()).await?;
                let db = match &conn.db {
                    Some(_) => Open::Schema(Arc::new(db) as Arc<dyn SchemaTrait>),
                    None => Open::DataBase(Arc::new(db) as Arc<dyn DBTrait>),
                };
                anyhow::Ok(db)
            }
            DBConnectionConfig::DuckDB { conn: _conn, .. } => {
                #[cfg(feature = "duckdb")]
                {
                    let db = dbnexus_db::duckdb::DuckDB::new(_conn.clone())?;
                    return match &_conn.db {
                        Some(_) => match &_conn.schema {
                            Some(_) => anyhow::Ok(Open::from(Arc::new(db) as Arc<dyn SchemaTrait>)),
                            None => anyhow::Ok(Open::from(Arc::new(db) as Arc<dyn DBTrait>)),
                        },
                        None => anyhow::Ok(Open::from(Arc::new(db) as Arc<dyn CatalogTrait>)),
                    };
                }
                #[cfg(not(feature = "duckdb"))]
                anyhow::bail!("need enable duckdb support")
            }
            DBConnectionConfig::ClickHouse { conn, .. } => {
                let db = dbnexus_db::clickhouse::ClickHouseDB::new(conn.clone())?;
                let db = match &conn.db {
                    Some(_) => Open::Schema(Arc::new(db) as Arc<dyn SchemaTrait>),
                    None => Open::DataBase(Arc::new(db) as Arc<dyn DBTrait>),
                };

                anyhow::Ok(db)
            }
            DBConnectionConfig::LibSql { conn, .. } => {
                let db = dbnexus_db::libsql::LibSqlDB::new(conn.clone()).await?;
                anyhow::Ok(Open::from(Arc::new(db) as Arc<dyn SchemaTrait>))
            }
            DBConnectionConfig::FireBird { conn, .. } => {
                let db = dbnexus_db::firebird::FireBirdDB::new(conn.clone())?;
                anyhow::Ok(Open::from(Arc::new(db) as Arc<dyn SchemaTrait>))
            }
        }
    }
}

/// opened database variant for frontend
#[derive(Clone, Debug)]
pub enum Open {
    Catalog(Arc<dyn dbnexus_db::db::CatalogTrait>),
    /// database can contains multi database
    DataBase(Arc<dyn dbnexus_db::db::DBTrait>),
    /// a schema
    Schema(Arc<dyn dbnexus_db::db::SchemaTrait>),
}

impl From<Arc<dyn dbnexus_db::db::CatalogTrait>> for Open {
    fn from(value: Arc<dyn dbnexus_db::db::CatalogTrait>) -> Self {
        Self::Catalog(value)
    }
}

impl From<Arc<dyn dbnexus_db::db::DBTrait>> for Open {
    fn from(value: Arc<dyn dbnexus_db::db::DBTrait>) -> Self {
        Self::DataBase(value)
    }
}
impl From<Arc<dyn dbnexus_db::db::SchemaTrait>> for Open {
    fn from(value: Arc<dyn dbnexus_db::db::SchemaTrait>) -> Self {
        Self::Schema(value)
    }
}

/// all the databases
#[derive(Clone, Default)]
pub struct App {
    pub opens: Arc<Mutex<HashMap<String, Open>>>,
    pub saved_password: Arc<DashMap<String, String>>,
}

impl App {
    pub fn new() -> Self {
        Default::default()
    }
}
type GetConfigFN = Box<dyn Fn(&str) -> anyhow::Result<DBConnectionConfig>>;

impl App {
    pub async fn insert_open(&self, name: impl Into<String>, open: Open) {
        let name = name.into();
        self.opens.lock().await.insert(name, open);
    }

    pub unsafe fn get_config_ref() -> &'static mut Option<GetConfigFN> {
        static mut GET_CONFIG_FN: Option<GetConfigFN> = None;
        #[allow(static_mut_refs)] // this var will init once by main
        &mut GET_CONFIG_FN
    }

    pub async fn enter_open_child(&self, name: &str, open: &Open) -> anyhow::Result<()> {
        match &open {
            Open::Catalog(catalog_trait) => {
                let databases = catalog_trait.list_database().await?;
                for database in databases.iter() {
                    let name = format!("{}/{}", name, database);
                    let dbtrait = match self.get_opened(&name).await {
                        Ok(x) => match x {
                            Open::DataBase(x) => x.clone(),
                            _ => unreachable!(),
                        },
                        Err(_) => {
                            log::info!("enter child named:{}", name);
                            let dbtrait = catalog_trait.open_db(database).await?;
                            self.insert_open(name.clone(), dbtrait.clone().into()).await;
                            dbtrait.clone()
                        }
                    };

                    let schemas = dbtrait.list_schema().await?;
                    for schema in schemas.iter() {
                        let name = format!("{}/{}", name, schema);
                        match self.get_opened(&name).await {
                            Ok(_) => {}
                            Err(_) => {
                                log::info!("enter child named:{}", name);
                                let s = dbtrait.open_schema(schema).await?;
                                self.insert_open(name, s.clone().into()).await;
                            }
                        }
                    }
                }
            }
            Open::DataBase(dbtrait) => {
                let schemas = dbtrait.list_schema().await?;
                for schema in schemas.iter() {
                    let name = format!("{}/{}", name, schema);
                    match self.get_opened(&name).await {
                        Ok(_) => {}
                        Err(_) => {
                            log::info!("enter child named:{}", name);
                            let s = dbtrait.open_schema(schema).await?;
                            self.insert_open(name, s.clone().into()).await;
                        }
                    }
                }
            }
            Open::Schema(_) => {}
        }
        anyhow::Ok(())
    }

    pub async fn open(&self, name: &str) -> anyhow::Result<Open> {
        if let Some(x) = self.opens.lock().await.get(&name.to_string()) {
            return anyhow::Ok(x.clone());
        }
        let config = unsafe { (Self::get_config_ref().as_ref().map(|x| x(name))).unwrap()? };
        #[cfg(debug_assertions)]
        println!("open config:{:?}", config);
        let db = config.open().await?;
        match &db {
            Open::DataBase(x) => {
                x.check_conn().await?;
            }
            Open::Schema(x) => {
                x.check_conn().await?;
            }
            Open::Catalog(x) => {
                x.check_conn().await?;
            }
        }
        self.enter_open_child(name, &db).await?;
        self.insert_open(name.to_string(), db.clone()).await;
        anyhow::Ok(db)
    }

    pub async fn close_conn_has_prefix(&self, name: &str) {
        let mut v = Vec::new();
        for k in self.opens.lock().await.keys() {
            if k.starts_with(name) {
                v.push(k.clone());
            }
        }

        for k in v.into_iter() {
            self.opens.lock().await.remove(&k);
        }
    }

    pub async fn get_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        match self.get_opened(name).await? {
            Open::Schema(x) => anyhow::Ok(x.clone()),
            _ => anyhow::bail!(format!("'{}' a schema trait", name)),
        }
    }

    pub async fn get_opened(&self, name: &str) -> anyhow::Result<Open> {
        let db = self.opens.lock().await.get(name).cloned();
        let x = match db {
            Some(x) => x,
            None => anyhow::bail!("'{}' not found,the connection maybe closed???", name),
        };
        anyhow::Ok(x)
    }
}
