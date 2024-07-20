use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use dbnexus_db::datasource::{
    MongoDataSource, MysqlDBConfig, OracleDBConfig, PGDBConfig, SqlServerDBConfig,
};
use dbnexus_db::db::{
    CatalogTrait, DBTrait, Field, Index, ListTable, ListView, SchemaTrait, SqlExecuteRet,
    TableListResp,
};
use dbnexus_db::pool::PoolManager;
use tokio::sync::Mutex;
use typescript_type_def::TypeDef;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct DBNexusConfigs {
    connection_configs: HashMap<String, DBConnectionConfig>,
    user_settings: UserSettings,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, TypeDef)]
#[serde(tag = "conn_type", rename_all = "lowercase")]
pub enum DBConnectionConfig {
    Sqlite {
        path: String,
        read_only: bool,
        name: String,
    },
    Mysql {
        #[serde(flatten)]
        conn: MysqlDBConfig,
        read_only: bool,
        name: String,
    },
    MariaDB {
        #[serde(flatten)]
        conn: MysqlDBConfig,
        read_only: bool,
        name: String,
    },
    Postgres {
        #[serde(flatten)]
        conn: PGDBConfig,
        read_only: bool,
        name: String,
        schema: Option<String>,
    },
    SqlServer {
        #[serde(flatten)]
        conn: SqlServerDBConfig,
        read_only: bool,

        name: String,
        schema: Option<String>,
    },
    MongoDB {
        #[serde(flatten)]
        conn: MongoDataSource,
        read_only: bool,
        db_name: Option<String>,
        name: String,
    },
    Redis {
        server: String,
        read_only: bool,
        name: String,
    },
    Oracle {
        #[serde(flatten)]
        conn: OracleDBConfig,
        read_only: bool,
        db_name: Option<String>,
        // milliseond
        call_timeout: Option<u64>,
        name: String,
    },
}

#[async_trait::async_trait]

pub trait DbConfigLoader: Send + Sync + 'static {
    /// add one config
    async fn add(&self, uuid: &str, config: DBConnectionConfig) -> anyhow::Result<()>;
    /// delete one config
    async fn delete(&self, uuid: &str) -> anyhow::Result<()>;
    /// update one
    async fn update(&self, uuid: &str, config: DBConnectionConfig) -> anyhow::Result<()>;
    ///
    async fn get_config_cloned(&self, uuid: &str) -> anyhow::Result<Option<DBConnectionConfig>>;
    async fn list_all_configs_cloned(&self) -> anyhow::Result<HashMap<String, DBConnectionConfig>>;

    async fn get_user_settings_cloned(&self) -> anyhow::Result<UserSettings>;
    async fn save_user_settings(&self, x: UserSettings) -> anyhow::Result<()>;
}

pub struct FileJsonSaver {
    filepath: String,
    config: tokio::sync::Mutex<DBNexusConfigs>,
}

impl FileJsonSaver {
    pub fn new(filepath: impl Into<String>) -> anyhow::Result<Self> {
        let filepath = filepath.into();
        let config = FileJsonSaver::load_from_file(&filepath)?;
        anyhow::Ok(Self {
            filepath,
            config: Mutex::new(config),
        })
    }
}

impl FileJsonSaver {
    async fn save(&self, config: &DBNexusConfigs) -> anyhow::Result<()> {
        let s = serde_json::to_string_pretty(config)?;

        tokio::fs::write(&Path::new(self.filepath.as_str()), s.as_bytes()).await?;
        anyhow::Ok(())
    }

    fn load_from_file(filepath: &str) -> anyhow::Result<DBNexusConfigs> {
        let path = Path::new(filepath);
        if !path.exists() {
            return anyhow::Ok(Default::default());
        }
        let bs = std::fs::read(&path)?;
        let config: DBNexusConfigs = serde_json::from_slice(bs.as_slice())?;
        anyhow::Ok(config)
    }
}

#[async_trait::async_trait]
impl DbConfigLoader for FileJsonSaver {
    async fn list_all_configs_cloned(&self) -> anyhow::Result<HashMap<String, DBConnectionConfig>> {
        anyhow::Ok(self.config.lock().await.connection_configs.clone())
    }

    async fn get_user_settings_cloned(&self) -> anyhow::Result<UserSettings> {
        Ok(self.config.lock().await.user_settings.clone())
    }

    async fn save_user_settings(&self, x: UserSettings) -> anyhow::Result<()> {
        let mut g = self.config.lock().await;
        g.user_settings = x;
        self.save(g.deref()).await?;
        anyhow::Ok(())
    }

    async fn add(&self, uuid: &str, config: DBConnectionConfig) -> anyhow::Result<()> {
        let mut lock = self.config.lock().await;
        lock.connection_configs.insert(uuid.to_string(), config);
        self.save(&lock).await?;
        anyhow::Ok(())
    }

    async fn delete(&self, uuid: &str) -> anyhow::Result<()> {
        let mut lock = self.config.lock().await;
        lock.connection_configs.remove(&uuid.to_string());
        self.save(&lock).await?;
        anyhow::Ok(())
    }

    async fn update(&self, uuid: &str, config: DBConnectionConfig) -> anyhow::Result<()> {
        self.add(uuid, config).await
    }

    async fn get_config_cloned(&self, uuid: &str) -> anyhow::Result<Option<DBConnectionConfig>> {
        let c = self
            .config
            .lock()
            .await
            .connection_configs
            .get(&uuid.to_string())
            .map(|x| x.clone());

        anyhow::Ok(c)
    }
}

/// open a
pub(crate) async fn open(config: &DBConnectionConfig, pm: PoolManager) -> anyhow::Result<Open> {
    match config {
        DBConnectionConfig::Sqlite {
            path,
            read_only,
            name: _,
        } => {
            let db: Arc<dyn SchemaTrait> = Arc::new(dbnexus_db::sqlite::SqliteDB::new(
                pm.clone(),
                path.clone(),
                *read_only,
            ));

            anyhow::Ok(Open::Schema(db))
        }
        DBConnectionConfig::Mysql {
            conn,
            read_only,
            name: _,
        }
        | DBConnectionConfig::MariaDB {
            conn,
            read_only,
            name: _,
        } => match &conn.db_name {
            Some(_) => {
                let db: Arc<dyn SchemaTrait> = Arc::new(dbnexus_db::mysql::MysqlDB::new(
                    pm.clone(),
                    conn.clone(),
                    *read_only,
                ));

                anyhow::Ok(Open::Schema(db))
            }
            None => {
                let db: Arc<dyn DBTrait> = Arc::new(dbnexus_db::mysql::MysqlDB::new(
                    pm.clone(),
                    conn.clone(),
                    *read_only,
                ));

                anyhow::Ok(Open::DataBase(db))
            }
        },
        DBConnectionConfig::Postgres {
            conn,
            read_only,
            name: _,
            schema,
        } => {
            let o = Arc::new(dbnexus_db::pg::PostGresDB::new(
                pm.clone(),
                conn.clone(),
                *read_only,
                schema.clone(),
            ));

            if conn.db_name.is_none() {
                anyhow::Ok(Open::Catelog(o as Arc<dyn CatalogTrait>))
            } else {
                match schema {
                    Some(_) => anyhow::Ok(Open::DataBase(o as Arc<dyn DBTrait>)),
                    None => anyhow::Ok(Open::Schema(o as Arc<dyn SchemaTrait>)),
                }
            }
        }
        DBConnectionConfig::SqlServer {
            conn,
            read_only,
            name: _,
            schema,
        } => {
            let o = Arc::new(
                dbnexus_db::sql_server::SqlServer::new(conn.clone(), *read_only, schema.clone())
                    .await?,
            );
            if conn.db_name.is_none() {
                anyhow::Ok(Open::Catelog(o as Arc<dyn CatalogTrait>))
            } else {
                match schema {
                    Some(_) => anyhow::Ok(Open::DataBase(o as Arc<dyn DBTrait>)),
                    None => anyhow::Ok(Open::Schema(o as Arc<dyn SchemaTrait>)),
                }
            }
        }

        DBConnectionConfig::MongoDB {
            conn,
            read_only,
            db_name,
            name: _,
        } => {
            let db: Arc<dyn SchemaTrait> = Arc::new(dbnexus_db::mongo::Mongo::new(
                pm.clone(),
                conn.clone(),
                db_name.clone(),
                *read_only,
            ));

            anyhow::Ok(Open::Schema(db))
        }
        DBConnectionConfig::Redis {
            server,
            read_only,
            name: _,
        } => {
            let db: Arc<dyn SchemaTrait> = Arc::new(dbnexus_db::redis::RedisDB::new(
                pm.clone(),
                server.to_string(),
                *read_only,
            ));

            anyhow::Ok(Open::Schema(db))
        }
        DBConnectionConfig::Oracle {
            conn,
            read_only,
            db_name,
            call_timeout,
            name: _,
        } => match db_name.as_ref() {
            Some(_) => {
                let db: Arc<dyn SchemaTrait> = Arc::new(dbnexus_db::oracle::OracleDB::new(
                    conn.clone(),
                    pm.clone(),
                    *read_only,
                    db_name.clone(),
                    call_timeout.clone().map(Duration::from_millis),
                ));

                anyhow::Ok(Open::Schema(db))
            }
            None => {
                let db: Arc<dyn DBTrait> = Arc::new(dbnexus_db::oracle::OracleDB::new(
                    conn.clone(),
                    pm.clone(),
                    *read_only,
                    db_name.clone(),
                    call_timeout.clone().map(Duration::from_millis),
                ));

                anyhow::Ok(Open::DataBase(db))
            }
        },
    }
}

/// opend database variant for front
#[derive(Clone)]
pub enum Open {
    Catelog(Arc<dyn dbnexus_db::db::CatalogTrait>),
    /// database can contains multi database
    DataBase(Arc<dyn dbnexus_db::db::DBTrait>),
    /// a schema
    Schema(Arc<dyn dbnexus_db::db::SchemaTrait>),
}

impl Open {
    pub(crate) fn get_open_kind(&self) -> OpendKind {
        use OpendKind::*;
        match self {
            Open::Catelog(_) => Catelog,
            Open::DataBase(_) => DataBase,
            Open::Schema(_) => Schema,
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, TypeDef)]
pub enum OpendKind {
    Catelog,
    DataBase,
    Schema,
}

/// all the databases
pub struct App {
    pub(crate) loader: Arc<dyn DbConfigLoader>,
    dbs: Arc<Mutex<HashMap<String, Open>>>,
    pm: PoolManager,
}

impl App {
    pub fn new(loader: Arc<dyn DbConfigLoader>) -> Self {
        Self {
            loader,
            dbs: Default::default(),
            pm: Default::default(),
        }
    }
}

impl App {
    pub async fn open(&self, uuid: &str) -> anyhow::Result<Open> {
        if let Some(x) = self.dbs.lock().await.get(&uuid.to_string()) {
            return anyhow::Ok(x.clone());
        }
        let config = match self.loader.get_config_cloned(uuid).await? {
            Some(config) => config,
            None => anyhow::bail!(format!("connect '{}' not found", uuid)),
        };
        let db = open(&config, self.pm.clone()).await?;
        match &db {
            Open::DataBase(x) => {
                x.check_conn().await?;
            }
            Open::Schema(x) => {
                x.check_conn().await?;
            }
            Open::Catelog(x) => {
                x.check_conn().await?;
            }
        }

        self.dbs.lock().await.insert(uuid.to_string(), db.clone());

        anyhow::Ok(db)
    }

    pub async fn open_db(&self, uuid: &str, db_name: &str) -> anyhow::Result<Open> {
        let db = self.open(uuid).await?;
        match db {
            Open::Catelog(x) => {
                let x = x.open_db(db_name).await?;
                x.check_conn().await?;
                let db = Open::DataBase(x.clone());
                self.dbs
                    .lock()
                    .await
                    .insert(format!("{}-{}", uuid, db_name), db.clone());
                anyhow::Ok(db)
            }

            _ => anyhow::bail!("this is not a catalog"),
        }
    }
    pub async fn open_schema(&self, uuid: &str, db_name: &str) -> anyhow::Result<Open> {
        let db = self.open(uuid).await?;
        match db {
            Open::DataBase(x) => {
                let x = x.open_schema(db_name).await?;
                x.check_conn().await?;
                let db = Open::Schema(x.clone());
                self.dbs
                    .lock()
                    .await
                    .insert(format!("{}-{}", uuid, db_name), db.clone());
                anyhow::Ok(db)
            }

            _ => anyhow::bail!("this is not a database"),
        }
    }

    pub async fn add_config(&self, uuid: &str, config: DBConnectionConfig) -> anyhow::Result<()> {
        self.loader.add(uuid, config).await
    }

    pub async fn delete_config(&self, uuid: &str) -> anyhow::Result<()> {
        self.loader.delete(uuid).await?;
        self.delete_prefix_dbs(uuid).await;
        anyhow::Ok(())
    }

    pub async fn update_config(
        &self, uuid: &str, config: DBConnectionConfig,
    ) -> anyhow::Result<()> {
        self.loader.update(uuid, config).await?;
        self.delete_prefix_dbs(uuid).await;
        anyhow::Ok(())
    }

    async fn delete_prefix_dbs(&self, uuid: &str) {
        let mut v = Vec::new();
        for k in self.dbs.lock().await.keys() {
            if k.starts_with(uuid) {
                v.push(k.clone());
            }
        }
        for k in v.into_iter() {
            self.dbs.lock().await.remove(&k);
        }
    }

    pub async fn list_table(&self, uuid: &str) -> anyhow::Result<Vec<ListTable>> {
        let db = self.get_schema(uuid).await?;
        db.list_table().await
    }

    pub async fn show_columns(&self, uuid: &str, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let db = self.get_schema(uuid).await?;
        db.show_columns(table_name).await
    }

    pub async fn show_table(
        &self, uuid: &str, table_name: &str, where_str: Option<&str>, order_by: Option<&str>,
        page: usize, page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let db = self.get_schema(uuid).await?;
        db.show_table(table_name, where_str, order_by, page, page_size)
            .await
    }

    pub async fn show_indexes(&self, uuid: &str, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let db = self.get_schema(uuid).await?;
        db.show_indexes(table_name).await
    }

    pub async fn insert(
        &self, uuid: &str, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let db = self.get_schema(uuid).await?;
        db.insert(table_name, values).await
    }

    pub async fn update(
        &self, uuid: &str, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let db = self.get_schema(uuid).await?;
        db.update(table_name, values, wheres).await
    }

    pub async fn truncate_table(&self, uuid: &str, table_name: &str) -> anyhow::Result<()> {
        let db = self.get_schema(uuid).await?;
        db.truncate_table(table_name).await
    }

    pub async fn delete(
        &self, uuid: &str, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let db = self.get_schema(uuid).await?;
        db.delete(table_name, wheres).await
    }

    pub async fn list_view(&self, uuid: &str) -> anyhow::Result<Vec<ListView>> {
        let db = self.get_schema(uuid).await?;
        db.list_view().await
    }

    pub async fn drop_view(&self, uuid: &str, view_name: &str) -> anyhow::Result<()> {
        let db = self.get_schema(uuid).await?;
        db.drop_view(view_name).await
    }

    pub async fn drop_table(&self, uuid: &str, table_name: &str) -> anyhow::Result<()> {
        let db = self.get_schema(uuid).await?;
        db.drop_table(table_name).await
    }

    pub async fn show_view(
        &self, uuid: &str, view_name: &str, where_str: Option<&str>, order_by: Option<&str>,
        page: usize, page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let db = self.get_schema(uuid).await?;
        db.show_view(view_name, where_str, order_by, page, page_size)
            .await
    }

    pub async fn get_schema(&self, uuid: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        match self.get_opened(uuid).await? {
            Open::Schema(x) => anyhow::Ok(x.clone()),
            _ => anyhow::bail!(format!("this '{}' a multi", uuid)),
        }
    }

    pub async fn get_opened(&self, uuid: &str) -> anyhow::Result<Open> {
        let db = self.dbs.lock().await.get(uuid).map(|x| x.clone());
        let x = match db {
            Some(x) => x,
            None => anyhow::bail!("db '{}' not found", uuid),
        };
        anyhow::Ok(x)
    }
}

#[derive(Clone, serde::Deserialize, serde::Serialize, Debug, TypeDef)]
pub struct UserSettings {
    page_size: usize,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self { page_size: 100 }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use dbnexus_db::datasource::MysqlDBConfig;

    use super::DBNexusConfigs;
    use crate::storage::DBConnectionConfig;

    #[test]
    fn print_config() {
        let s = DBNexusConfigs {
            user_settings: super::UserSettings { page_size: 100 },
            connection_configs: {
                let mut m = HashMap::new();
                m.insert(
                    "xxxx".to_string(),
                    DBConnectionConfig::Sqlite {
                        path: "d:\\sqlite.test.db".to_string(),
                        read_only: false,
                        name: "haha".to_string(),
                    },
                );
                m.insert(
                    "yyyy".to_string(),
                    DBConnectionConfig::Mysql {
                        conn: MysqlDBConfig {
                            host: "127.0.0.1".to_string(),
                            port: 33,
                            username: "aaa".to_string(),
                            password: "bbb".to_string(),
                            db_name: None,
                        },
                        read_only: false,
                        name: "haha".to_string(),
                    },
                );
                m
            },
        };
        let s = serde_json::to_string_pretty(&s).unwrap();
        println!("config:{}", s);
    }
}
