use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use typescript_type_def::TypeDef;

use crate::datasource_config::{
    MongoDBConfig, MysqlDBConfig, OracleDBConfig, PGDBConfig, RedisDBConfig, SqlServerDBConfig,
    SqliteDBConfig,
};
use crate::db::{
    CatalogTrait, DBTrait, Field, Index, ListTable, NamedStaff, SchemaTrait, SqlExecuteRet,
    TableListResp,
};
use crate::mongo::Mongo;
use crate::redis::{RedisDB, RedisListTable};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct DBNexusConfigs {
    connection_configs: Vec<DBConnectionConfig>,
}

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
            | DBConnectionConfig::Oracle { name, .. } => name.as_str(),
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
    SqlServer {
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
}

impl DBConnectionConfig {
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
            DBConnectionConfig::SqlServer { conn, .. } => {
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
        }
    }
}
impl DBConnectionConfig {
    pub async fn open(&self) -> anyhow::Result<Open> {
        match self {
            DBConnectionConfig::Sqlite { conn, name: _ } => {
                let db: Arc<dyn SchemaTrait> = Arc::new(
                    crate::sqlite::SqliteDB::new(conn.path.clone(), conn.read_only).await?,
                );

                anyhow::Ok(Open::Schema(db))
            }
            DBConnectionConfig::Mysql { conn, name: _ }
            | DBConnectionConfig::MariaDB { conn, name: _ } => match &conn.db {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(crate::mysql::MysqlDB::new(conn.clone()).await?);

                    anyhow::Ok(Open::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(crate::mysql::MysqlDB::new(conn.clone()).await?);
                    anyhow::Ok(Open::DataBase(db))
                }
            },
            DBConnectionConfig::Tidb { conn, name: _ } => match &conn.db {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(crate::tidb::TiDB::new(conn.clone()).await?);

                    anyhow::Ok(Open::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(crate::tidb::TiDB::new(conn.clone()).await?);
                    anyhow::Ok(Open::DataBase(db))
                }
            },

            DBConnectionConfig::Postgres { conn, name: _ } => {
                let o = Arc::new(crate::pg::PostGresDB::new(conn.clone()).await?);
                if conn.db.is_none() {
                    return anyhow::Ok(Open::Catalog(o as Arc<dyn CatalogTrait>));
                }

                match &conn.schema {
                    None => anyhow::Ok(Open::DataBase(o as Arc<dyn DBTrait>)),
                    Some(_) => anyhow::Ok(Open::Schema(o as Arc<dyn SchemaTrait>)),
                }
            }
            DBConnectionConfig::SqlServer { conn, name: _ } => {
                let o = Arc::new(crate::mssql::MsSqlServer::new(conn.clone()).await?);
                if conn.db.is_none() {
                    return anyhow::Ok(Open::Catalog(o as Arc<dyn CatalogTrait>));
                }
                match &conn.schema {
                    Some(_) => anyhow::Ok(Open::DataBase(o as Arc<dyn DBTrait>)),
                    None => anyhow::Ok(Open::Schema(o as Arc<dyn SchemaTrait>)),
                }
            }

            DBConnectionConfig::Mongo { conn, name: _ } => {
                let db: Arc<Mongo> = Arc::new(crate::mongo::Mongo::new(conn.clone())?);
                match conn.db {
                    Some(_) => anyhow::Ok(Open::Schema(db)),
                    None => anyhow::Ok(Open::DataBase(db)),
                }
            }
            DBConnectionConfig::Redis { conn, .. } => {
                let redis = crate::redis::RedisDB::new(conn.clone()).await?;

                let db = match &conn.db {
                    Some(_) => Open::Schema(Arc::new(redis) as Arc<dyn SchemaTrait>),
                    None => Open::DataBase(Arc::new(redis) as Arc<dyn DBTrait>),
                };

                anyhow::Ok(db)
            }
            DBConnectionConfig::Oracle { conn, name: _ } => match conn.schema.as_ref() {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(crate::oracle::OracleDB::new(conn.clone(), None)?);
                    anyhow::Ok(Open::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(crate::oracle::OracleDB::new(conn.clone(), None)?);

                    anyhow::Ok(Open::DataBase(db))
                }
            },
        }
    }
}

/// opened database variant for frontend
#[derive(Clone, Debug)]
pub enum Open {
    Catalog(Arc<dyn crate::db::CatalogTrait>),
    /// database can contains multi database
    DataBase(Arc<dyn crate::db::DBTrait>),
    /// a schema
    Schema(Arc<dyn crate::db::SchemaTrait>),
}

impl From<Arc<dyn crate::db::CatalogTrait>> for Open {
    fn from(value: Arc<dyn crate::db::CatalogTrait>) -> Self {
        Self::Catalog(value)
    }
}

impl From<Arc<dyn crate::db::DBTrait>> for Open {
    fn from(value: Arc<dyn crate::db::DBTrait>) -> Self {
        Self::DataBase(value)
    }
}
impl From<Arc<dyn crate::db::SchemaTrait>> for Open {
    fn from(value: Arc<dyn crate::db::SchemaTrait>) -> Self {
        Self::Schema(value)
    }
}

/// all the databases
#[derive(Clone)]
pub struct App {
    pub opens: Arc<Mutex<HashMap<String, Open>>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            opens: Default::default(),
        }
    }
}
type GetConfigFN = Box<dyn Fn(&str) -> anyhow::Result<DBConnectionConfig>>;

impl App {
    pub unsafe fn get_config_ref() -> &'static mut Option<GetConfigFN> {
        static mut XXX: Option<GetConfigFN> = None;
        #[allow(static_mut_refs)] // this var will init once by main
        &mut XXX
    }

    pub async fn enter_open_child(&self, name: &str, db: &Open) -> anyhow::Result<()> {
        match &db {
            Open::Catalog(catalog_trait) => {
                let databases = catalog_trait.list_database().await?;
                for database in databases.iter() {
                    let dbtrait = catalog_trait.open_db(&database).await?;
                    self.opens
                        .lock()
                        .await
                        .insert(format!("{}/{}", name, database), dbtrait.clone().into());

                    let schemas = dbtrait.list_schema().await?;
                    for schema in schemas.iter() {
                        let s = dbtrait.open_schema(&schema).await?;
                        self.opens
                            .lock()
                            .await
                            .insert(format!("{}/{}/{}", name, database, schema), s.into());
                    }
                }
            }
            Open::DataBase(dbtrait) => {
                let schemas = dbtrait.list_schema().await?;
                for database in schemas.iter() {
                    let s = dbtrait.open_schema(&database).await?;
                    self.opens
                        .lock()
                        .await
                        .insert(format!("{}/{}", name, database), s.into());
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
        self.opens.lock().await.insert(name.to_string(), db.clone());
        anyhow::Ok(db)
    }

    pub async fn open_schema(&self, name: &str, db_name: &str) -> anyhow::Result<Open> {
        let db = self.open(name).await?;
        match db {
            Open::DataBase(x) => {
                let x = x.open_schema(db_name).await?;
                x.check_conn().await?;
                let db = Open::Schema(x.clone());
                self.opens
                    .lock()
                    .await
                    .insert(format!("{}-{}", name, db_name), db.clone());
                anyhow::Ok(db)
            }
            _ => anyhow::bail!("this is not a database"),
        }
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

    pub async fn list_table(&self, name: &str) -> anyhow::Result<Vec<ListTable>> {
        let db = self.get_schema(name).await?;
        db.list_table().await
    }

    pub async fn show_table_columns(
        &self, name: &str, table_name: &str,
    ) -> anyhow::Result<Vec<Field>> {
        let db = self.get_schema(name).await?;
        db.show_table_columns(table_name).await
    }

    pub async fn show_table(
        &self, name: &str, table_name: &str, where_str: Option<&str>, order_by: Option<&str>,
        page: usize, page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let db = self.get_schema(name).await?;
        db.show_table(table_name, where_str, order_by, page, page_size)
            .await
    }

    pub async fn show_indexes(&self, name: &str, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let db = self.get_schema(name).await?;
        db.show_indexes(table_name).await
    }

    pub async fn insert(
        &self, name: &str, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let db = self.get_schema(name).await?;
        db.insert(table_name, values).await
    }

    pub async fn update(
        &self, name: &str, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let db = self.get_schema(name).await?;
        db.update(table_name, values, wheres).await
    }

    pub async fn truncate_table(&self, name: &str, table_name: &str) -> anyhow::Result<()> {
        let db = self.get_schema(name).await?;
        db.truncate_table(table_name).await
    }

    pub async fn delete(
        &self, name: &str, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let db = self.get_schema(name).await?;
        db.delete(table_name, wheres).await
    }

    pub async fn list_view(&self, name: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let db = self.get_schema(name).await?;
        db.list_view().await
    }

    pub async fn drop_view(&self, name: &str, view_name: &str) -> anyhow::Result<()> {
        let db = self.get_schema(name).await?;
        db.drop_view(view_name).await
    }

    pub async fn drop_table(
        &self, name: &str, table_name: &str, cascade: bool,
    ) -> anyhow::Result<()> {
        let db = self.get_schema(name).await?;
        db.drop_table(table_name, cascade).await
    }

    pub async fn show_view(
        &self, name: &str, view_name: &str, where_str: Option<&str>, order_by: Option<&str>,
        page: usize, page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let db = self.get_schema(name).await?;
        db.show_view(view_name, where_str, order_by, page, page_size)
            .await
    }

    pub async fn get_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        if !self.opens.lock().await.contains_key(name) {
            self.open(name).await?;
        }

        match self.get_opened(name).await? {
            Open::Schema(x) => anyhow::Ok(x.clone()),
            _ => anyhow::bail!(format!("this '{}' a multi", name)),
        }
    }

    pub async fn get_opened(&self, name: &str) -> anyhow::Result<Open> {
        let db = self.opens.lock().await.get(name).map(|x| x.clone());
        let x = match db {
            Some(x) => x,
            None => anyhow::bail!("db '{}' not found", name),
        };
        anyhow::Ok(x)
    }

    pub async fn redis_list_table(
        &self, name: &str, filter: Option<&str>, max: usize,
    ) -> anyhow::Result<RedisListTable> {
        let db = self.get_schema(name).await?;
        match db.as_any().downcast_ref::<RedisDB>() {
            Some(x) => x.redis_list_table(filter, max).await,
            None => anyhow::bail!(format!("'{}' not a redis", name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DBNexusConfigs;
    use crate::app::DBConnectionConfig;
    use crate::datasource_config::MysqlDBConfig;

    #[test]
    fn print_config() {
        let s = DBNexusConfigs {
            connection_configs: {
                let mut m = Vec::new();

                m.push(DBConnectionConfig::Mysql {
                    conn: MysqlDBConfig {
                        host: "127.0.0.1".to_string(),
                        port: 33,
                        username: "aaa".to_string(),
                        password: Some("bbb".to_string()),
                        db: None,
                        read_only: false,
                    },
                    name: "haha".to_string(),
                });
                m
            },
        };
        let s = serde_json::to_string_pretty(&s).unwrap();
        println!("config:{}", s);
    }
}
