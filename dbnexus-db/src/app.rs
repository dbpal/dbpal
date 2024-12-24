use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use tokio::sync::Mutex;
use typescript_type_def::TypeDef;

use crate::datasource_config::{
    MongoConfig, MysqlDbConfig, OracleDbConfig, PGDbConfig, RedisDbConfig, SqlServerDbConfig,
};
use crate::db::{
    DBTrait, Field, Index, ListTable, NamedStaff, SchemaTrait, SqlExecuteRet, TableListResp,
};
use crate::redis::{RedisDB, RedisListTable};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct DBNexusConfigs {
    connection_configs: Vec<DBConnectionConfig>,
    user_settings: UserSettings,
}

impl DBConnectionConfig {
    pub fn get_name(&self) -> &str {
        match self {
            DBConnectionConfig::Sqlite { name, .. }
            | DBConnectionConfig::Mysql { name, .. }
            | DBConnectionConfig::MariaDB { name, .. }
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
        #[serde(flatten)]
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
    async fn add(&self, config: DBConnectionConfig) -> anyhow::Result<()>;
    /// delete one config
    async fn delete(&self, name: &str) -> anyhow::Result<()>;

    ///
    async fn get_config_cloned(&self, uuid: &str) -> anyhow::Result<DBConnectionConfig>;
    async fn list_all_configs_cloned(&self) -> anyhow::Result<Vec<DBConnectionConfig>>;

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
    async fn list_all_configs_cloned(&self) -> anyhow::Result<Vec<DBConnectionConfig>> {
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

    async fn add(&self, config: DBConnectionConfig) -> anyhow::Result<()> {
        let mut lock = self.config.lock().await;
        for old in lock.connection_configs.iter() {
            if old.get_name().eq_ignore_ascii_case(config.get_name()) {
                anyhow::bail!(format!("name '{}' already exists.", config.get_name()))
            }
        }
        lock.connection_configs.push(config);
        self.save(&lock).await?;
        anyhow::Ok(())
    }

    async fn delete(&self, name: &str) -> anyhow::Result<()> {
        let mut lock = self.config.lock().await;
        let mut index = None;
        lock.connection_configs
            .iter()
            .enumerate()
            .for_each(|(i, x)| {
                if x.get_name() == name {
                    index = Some(i);
                }
            });

        if let Some(index) = index {
            lock.connection_configs.remove(index);
            self.save(&lock).await?;
        }
        anyhow::Ok(())
    }

    async fn get_config_cloned(&self, name: &str) -> anyhow::Result<DBConnectionConfig> {
        let ret = self
            .config
            .lock()
            .await
            .deref()
            .connection_configs
            .iter()
            .find(|x| x.get_name() == name)
            .map(|x| x.clone());
        match ret {
            Some(x) => return anyhow::Ok(x),
            None => {
                anyhow::bail!(format!("config named '{}' not found", name))
            }
        }
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
        | DBConnectionConfig::MariaDB { conn, name: _ } => {
            match &conn.db {
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
            }
        }
        DBConnectionConfig::Postgres { conn, name: _ } => {
            let o = Arc::new(crate::pg::PostGresDB::new(conn.clone()).await?);
            match &conn.schema {
                None => anyhow::Ok(Open::DataBase(o as Arc<dyn DBTrait>)),
                Some(_) => anyhow::Ok(Open::Schema(o as Arc<dyn SchemaTrait>)),
            }
        }
        DBConnectionConfig::SqlServer { conn, name: _ } => {
            let o = Arc::new(crate::mssql::MsSqlServer::new(conn.clone()).await?);
            match &conn.schema {
                Some(_) => anyhow::Ok(Open::DataBase(o as Arc<dyn DBTrait>)),
                None => anyhow::Ok(Open::Schema(o as Arc<dyn SchemaTrait>)),
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
        DBConnectionConfig::Oracle { conn, name: _ } => {
            match conn.schema.as_ref() {
                Some(_) => {
                    let db: Arc<dyn SchemaTrait> =
                        Arc::new(crate::oracle::OracleDB::new(conn.clone())?);

                    anyhow::Ok(Open::Schema(db))
                }
                None => {
                    let db: Arc<dyn DBTrait> =
                        Arc::new(crate::oracle::OracleDB::new(conn.clone())?);

                    anyhow::Ok(Open::DataBase(db))
                }
            }
        }
    }
}

/// opend database variant for frontend
#[derive(Clone, Debug)]
pub enum Open {
    /// database can contains multi database
    DataBase(Arc<dyn crate::db::DBTrait>),
    /// a schema
    Schema(Arc<dyn crate::db::SchemaTrait>),
}

impl Open {
    pub fn as_any(&self) -> &dyn std::any::Any {
        match self {
            Open::DataBase(dbtrait) => dbtrait.as_any(),
            Open::Schema(schema_trait) => schema_trait.as_any(),
        }
    }
}
/// all the databases
#[derive(Clone)]
pub struct App {
    pub loader: Arc<dyn DbConfigLoader>,
    pub opens: Arc<Mutex<HashMap<String, Open>>>,
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
    pub async fn open(&self, name: &str) -> anyhow::Result<Open> {
        if let Some(x) = self.opens.lock().await.get(&name.to_string()) {
            return anyhow::Ok(x.clone());
        }
        let config = self.loader.get_config_cloned(name).await?;
        let db = open(&config).await?;
        match &db {
            Open::DataBase(x) => {
                x.check_conn().await?;
            }
            Open::Schema(x) => {
                x.check_conn().await?;
            }
        }

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

    pub async fn add_config(&self, config: DBConnectionConfig) -> anyhow::Result<()> {
        self.loader.add(config).await
    }

    pub async fn delete_config(&self, name: &str) -> anyhow::Result<()> {
        self.loader.delete(name).await?;
        self.delete_prefix_dbs(name).await;
        anyhow::Ok(())
    }

    // pub async fn update_config(
    //     &self, uuid: &str, config: DBConnectionConfig,
    // ) -> anyhow::Result<()> {
    //     self.loader.update(uuid, config).await?;
    //     self.delete_prefix_dbs(uuid).await;
    //     anyhow::Ok(())
    // }

    async fn delete_prefix_dbs(&self, name: &str) {
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

    pub async fn show_columns(&self, name: &str, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let db = self.get_schema(name).await?;
        db.show_columns(table_name).await
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
            let db_uuids: Vec<_> = name.split(":").collect();
            match db_uuids.len() {
                1 => {
                    // I am trying to open the database so the frontend no need call `open` himself
                    self.open(name).await?;
                }
                2 => {
                    // trying to open
                    match self.open(db_uuids[0]).await? {
                        Open::DataBase(d) => {
                            let s = d.open_schema(db_uuids[1]).await?;
                            self.opens.lock().await.insert(
                                format!("{}:{}", db_uuids[0], db_uuids[1]),
                                Open::Schema(s),
                            );
                        }
                        _ => anyhow::bail!("already a schema not a nest struct."),
                    }
                }
                _ => {
                    unreachable!()
                }
            }
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

    pub async fn redis_list_table(&self, name: &str) -> anyhow::Result<RedisListTable> {
        let db = self.get_schema(name).await?;
        match db.as_any().downcast_ref::<RedisDB>() {
            Some(x) => x.redis_list_table().await,
            None => anyhow::bail!(format!("'{}' not a redis", name)),
        }
    }

    pub async fn redis_match_keys(&self, name: &str, pattern: &str) -> anyhow::Result<Vec<String>> {
        let db = self.get_schema(name).await?;
        match db.as_any().downcast_ref::<RedisDB>() {
            Some(x) => x.redis_match_keys(pattern).await,
            None => anyhow::bail!(format!("'{}' not a redis", name)),
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
    use super::DBNexusConfigs;
    use crate::app::DBConnectionConfig;
    use crate::datasource_config::MysqlDbConfig;

    #[test]
    fn print_config() {
        let s = DBNexusConfigs {
            user_settings: super::UserSettings { page_size: 100 },
            connection_configs: {
                let mut m = Vec::new();
                m.push(DBConnectionConfig::Sqlite {
                    path: "d:\\sqlite.test.db".to_string(),
                    read_only: false,
                    name: "haha".to_string(),
                });
                m.push(DBConnectionConfig::Mysql {
                    conn: MysqlDbConfig {
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
