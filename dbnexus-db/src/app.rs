use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use crate::datasource_config::{
    MongoConfig, MysqlDbConfig, OracleDbConfig, PGDbConfig, RedisDbConfig, SqlServerDbConfig,
};

use crate::db::{
    CatalogTrait, DBTrait, Field, Index, ListTable, NamedStaff, SchemaTrait, SqlExecuteRet,
    TableListResp,
};
use crate::redis::{RedisDB, RedisListTable};

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
        conn: MysqlDbConfig,
        name: String,
    },
    MariaDB {
        #[serde(flatten)]
        conn: MysqlDbConfig,
        name: String,
    },
    Postgres {
        #[serde(flatten)]
        conn: PGDbConfig,
        name: String,
    },
    SqlServer {
        #[serde(flatten)]
        conn: SqlServerDbConfig,
        name: String,
    },
    Mongo {
        #[serde(flatten)]
        conn: MongoConfig,
        name: String,
    },
    Redis {
        name: String,
        conn: RedisDbConfig,
    },
    Oracle {
        #[serde(flatten)]
        conn: OracleDbConfig,
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
pub async fn open(config: &DBConnectionConfig) -> anyhow::Result<Open> {
    match config {
        DBConnectionConfig::Sqlite {
            path,
            read_only,
            name: _,
        } => {
            let db: Arc<dyn SchemaTrait> =
                Arc::new(crate::sqlite::SqliteDB::new(path.clone(), *read_only).await?);

            anyhow::Ok(Open::Schema(db))
        }
        DBConnectionConfig::Mysql { conn, name: _ }
        | DBConnectionConfig::MariaDB { conn, name: _ } => match &conn.db_name {
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
        DBConnectionConfig::Postgres { conn, name: _ } => {
            let o = Arc::new(crate::pg::PostGresDB::new(conn.clone()).await?);

            if conn.db_name.is_none() {
                anyhow::Ok(Open::Catalog(o as Arc<dyn CatalogTrait>))
            } else {
                match &conn.schema {
                    Some(_) => anyhow::Ok(Open::DataBase(o as Arc<dyn DBTrait>)),
                    None => anyhow::Ok(Open::Schema(o as Arc<dyn SchemaTrait>)),
                }
            }
        }
        DBConnectionConfig::SqlServer { conn, name: _ } => {
            let o = Arc::new(crate::sql_server::SqlServer::new(conn.clone()).await?);
            if conn.db_name.is_none() {
                anyhow::Ok(Open::Catalog(o as Arc<dyn CatalogTrait>))
            } else {
                match &conn.schema {
                    Some(_) => anyhow::Ok(Open::DataBase(o as Arc<dyn DBTrait>)),
                    None => anyhow::Ok(Open::Schema(o as Arc<dyn SchemaTrait>)),
                }
            }
        }

        DBConnectionConfig::Mongo { conn, name: _ } => {
            let db: Arc<dyn SchemaTrait> = Arc::new(crate::mongo::Mongo::new(conn.clone())?);

            anyhow::Ok(Open::Schema(db))
        }
        DBConnectionConfig::Redis { conn, .. } => {
            let redis = crate::redis::RedisDB::new(conn.clone()).await?;

            let db = match &conn.db {
                Some(_) => Open::DataBase(Arc::new(redis) as Arc<dyn DBTrait>),
                None => Open::Schema(Arc::new(redis) as Arc<dyn SchemaTrait>),
            };

            anyhow::Ok(db)
        }
        DBConnectionConfig::Oracle { conn, name: _ } => match conn.schema.as_ref() {
            Some(_) => {
                let db: Arc<dyn SchemaTrait> =
                    Arc::new(crate::oracle::OracleDB::new(conn.clone())?);

                anyhow::Ok(Open::Schema(db))
            }
            None => {
                let db: Arc<dyn DBTrait> = Arc::new(crate::oracle::OracleDB::new(conn.clone())?);

                anyhow::Ok(Open::DataBase(db))
            }
        },
    }
}

/// opend database variant for frontend
#[derive(Clone)]
pub enum Open {
    Catalog(Arc<dyn crate::db::CatalogTrait>),
    /// database can contains multi database
    DataBase(Arc<dyn crate::db::DBTrait>),
    /// a schema
    Schema(Arc<dyn crate::db::SchemaTrait>),
}

impl Open {
    pub fn get_open_kind(&self) -> OpendKind {
        use OpendKind::*;
        match self {
            Open::Catalog(_) => Catalog,
            Open::DataBase(_) => DataBase,
            Open::Schema(_) => Schema,
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, TypeDef)]
pub enum OpendKind {
    Catalog,
    DataBase,
    Schema,
}

/// all the databases
#[derive(Clone)]
pub struct App {
    pub loader: Arc<dyn DbConfigLoader>,
    opens: Arc<Mutex<HashMap<String, Open>>>,
}

impl App {
    pub fn new(loader: Arc<dyn DbConfigLoader>) -> Self {
        Self {
            loader,
            opens: Default::default(),
        }
    }
}

impl App {
    pub async fn open(&self, uuid: &str) -> anyhow::Result<Open> {
        if let Some(x) = self.opens.lock().await.get(&uuid.to_string()) {
            return anyhow::Ok(x.clone());
        }
        let config = match self.loader.get_config_cloned(uuid).await? {
            Some(config) => config,
            None => anyhow::bail!(format!("connect '{}' not found", uuid)),
        };
        let db = open(&config).await?;
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

        self.opens.lock().await.insert(uuid.to_string(), db.clone());

        anyhow::Ok(db)
    }

    pub async fn open_db(&self, uuid: &str, db_name: &str) -> anyhow::Result<Open> {
        let db = self.open(uuid).await?;
        match db {
            Open::Catalog(x) => {
                let x = x.open_db(db_name).await?;
                x.check_conn().await?;
                let db = Open::DataBase(x.clone());
                self.opens
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
                self.opens
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
        for k in self.opens.lock().await.keys() {
            if k.starts_with(uuid) {
                v.push(k.clone());
            }
        }
        for k in v.into_iter() {
            self.opens.lock().await.remove(&k);
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

    pub async fn list_view(&self, uuid: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let db = self.get_schema(uuid).await?;
        db.list_view().await
    }

    pub async fn drop_view(&self, uuid: &str, view_name: &str) -> anyhow::Result<()> {
        let db = self.get_schema(uuid).await?;
        db.drop_view(view_name).await
    }

    pub async fn drop_table(
        &self, uuid: &str, table_name: &str, cascade: bool,
    ) -> anyhow::Result<()> {
        let db = self.get_schema(uuid).await?;
        db.drop_table(table_name ,cascade).await
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
        if !self.opens.lock().await.contains_key(uuid) {
            let db_uuids: Vec<_> = uuid.split("~").collect();
            match db_uuids.len() {
                1 => {
                    // nothing
                    // the frontend should open database
                    // for themself.
                }
                2 => {
                    // trying to open
                    match self.get_opened(db_uuids[0]).await? {
                        Open::Catalog(c) => {
                            let db = c.open_db(db_uuids[1]).await?;
                            self.opens.lock().await.insert(
                                format!("{}~{}", db_uuids[0], db_uuids[1]),
                                Open::DataBase(db),
                            );
                        }
                        Open::DataBase(d) => {
                            let s = d.open_schema(db_uuids[1]).await?;
                            self.opens.lock().await.insert(
                                format!("{}~{}", db_uuids[0], db_uuids[1]),
                                Open::Schema(s),
                            );
                        }
                        _ => anyhow::bail!("already a schema not a nest struct."),
                    }
                }

                3 => {
                    let db_uuid = format!("{}~{}", db_uuids[0], db_uuids[1]);
                    match self.get_opened(db_uuids[0]).await? {
                        Open::Catalog(c) => {
                            if self.opens.lock().await.contains_key(&db_uuid) {
                                match self.get_opened(&db_uuid).await? {
                                    Open::DataBase(db) => {
                                        let schema = db.open_schema(db_uuids[2]).await?;
                                        self.opens
                                            .lock()
                                            .await
                                            .insert(uuid.to_string(), Open::Schema(schema));
                                    }
                                    _ => anyhow::bail!("this should a database"),
                                }
                            } else {
                                let db = c.open_db(db_uuids[1]).await?;
                                self.opens
                                    .lock()
                                    .await
                                    .insert(db_uuid, Open::DataBase(db.clone()));
                                let schema = db.open_schema(db_uuids[2]).await?;
                                self.opens
                                    .lock()
                                    .await
                                    .insert(uuid.to_string(), Open::Schema(schema));
                            }
                        }
                        _ => {
                            anyhow::bail!("must be a catalog open")
                        }
                    }
                }
                _ => {
                    unreachable!()
                }
            }
        }

        match self.get_opened(uuid).await? {
            Open::Schema(x) => anyhow::Ok(x.clone()),
            _ => anyhow::bail!(format!("this '{}' a multi", uuid)),
        }
    }

    pub async fn get_opened(&self, uuid: &str) -> anyhow::Result<Open> {
        let db = self.opens.lock().await.get(uuid).map(|x| x.clone());
        let x = match db {
            Some(x) => x,
            None => anyhow::bail!("db '{}' not found", uuid),
        };
        anyhow::Ok(x)
    }

    pub async fn redis_list_table(&self, uuid: &str) -> anyhow::Result<RedisListTable> {
        let db = self.get_schema(uuid).await?;
        match db.as_any().downcast_ref::<RedisDB>() {
            Some(x) => x.redis_list_table().await,
            None => anyhow::bail!(format!("'{}' not a redis", uuid)),
        }
    }

    pub async fn redis_match_keys(&self, uuid: &str, pattern: &str) -> anyhow::Result<Vec<String>> {
        let db = self.get_schema(uuid).await?;
        match db.as_any().downcast_ref::<RedisDB>() {
            Some(x) => x.redis_match_keys(pattern).await,
            None => anyhow::bail!(format!("'{}' not a redis", uuid)),
        }
    }
}

#[derive(Clone, serde::Deserialize, serde::Serialize, Debug, TypeDef)]
pub struct UserSettings {
    pub page_size: usize,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self { page_size: 100 }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::datasource_config::MysqlDbConfig;

    use super::DBNexusConfigs;
    use crate::app::DBConnectionConfig;

    #[test]
    fn print_config() {
        let s = DBNexusConfigs {
            user_settings: super::UserSettings { page_size: 100 },
            connection_configs: {
                let mut m = HashMap::new();
                m.insert("xxxx".to_string(), DBConnectionConfig::Sqlite {
                    path: "d:\\sqlite.test.db".to_string(),
                    read_only: false,
                    name: "haha".to_string(),
                });
                m.insert("yyyy".to_string(), DBConnectionConfig::Mysql {
                    conn: MysqlDbConfig {
                        host: "127.0.0.1".to_string(),
                        port: 33,
                        username: "aaa".to_string(),
                        password: "bbb".to_string(),
                        db_name: None,
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
