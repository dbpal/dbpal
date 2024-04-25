use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;

use dbnexus_db::db::{DBTrait, Field, Index, ListTable, ListView, SqlExecuteRet, TableListResp};
use dbnexus_db::pool::PoolManager;
use tokio::sync::Mutex;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, Default)]
pub struct DBNexusConfigs {
    connection_configs: HashMap<String, HashMap<String, String>>,
    user_settings: UserSettings,
}

pub type DBConnectionConfig = HashMap<String, String>;

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
    async fn list_all_configs_cloned(
        &self,
    ) -> anyhow::Result<HashMap<String, HashMap<String, String>>>;

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
    async fn list_all_configs_cloned(
        &self,
    ) -> anyhow::Result<HashMap<String, HashMap<String, String>>> {
        anyhow::Ok(self.config.lock().await.connection_configs.clone())
    }

    async fn get_user_settings_cloned(&self) -> anyhow::Result<UserSettings> {
        Ok(self.config.lock().await.user_settings.clone())
    }

    async fn save_user_settings(&self, x: UserSettings) -> anyhow::Result<()> {
        let mut g = self.config.lock().await;
        g.user_settings = x;
        self.save(g.deref()).await
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

async fn open_db(config: &HashMap<String, String>, pm: PoolManager) -> anyhow::Result<DB> {
    match config.get("kind").as_ref().map(|x| x.to_lowercase()) {
        Some(x) => {
            match x.as_str() {
                "sqlite" => {
                    let path = match config.get(&"path".to_string()) {
                        Some(x) => x.to_owned(),
                        None => return Err(anyhow::Error::msg("path is empty")),
                    };
                    let db: Arc<dyn DBTrait> =
                        Arc::new(dbnexus_db::sqlite::SqliteDB::new(pm, path, false));

                    anyhow::Ok(DB::One(db))
                }
                _ => Err(anyhow::Error::msg(format!("db kind '{}' not handle", x))),
            }
        }
        _ => Err(anyhow::Error::msg("kind is empty")),
    }
}

#[derive(Clone)]
pub enum DB {
    Multi(Arc<dyn dbnexus_db::db::MultiDBTrait>),
    One(Arc<dyn dbnexus_db::db::DBTrait>),
}

/// all the databases
pub struct DBS {
    pub(crate) loader: Arc<dyn DbConfigLoader>,
    dbs: Arc<Mutex<HashMap<String, DB>>>,
    pm: PoolManager,
}

impl DBS {
    pub fn new(loader: Arc<dyn DbConfigLoader>) -> Self {
        Self {
            loader,
            dbs: Default::default(),
            pm: Default::default(),
        }
    }
}

impl DBS {
    pub async fn open_db(&self, uuid: &str) -> anyhow::Result<DB> {
        if let Some(x) = self.dbs.lock().await.get(&uuid.to_string()) {
            return anyhow::Ok(x.clone());
        }
        let config = match self.loader.get_config_cloned(uuid).await? {
            Some(config) => config,
            None => anyhow::bail!(format!("connect '{}' not found", uuid)),
        };
        let db = open_db(&config, self.pm.clone()).await?;
        self.dbs.lock().await.insert(uuid.to_string(), db.clone());

        anyhow::Ok(db)
    }

    pub async fn open_db2(&self, uuid: &str, db_name: &str) -> anyhow::Result<DB> {
        let db = self.open_db(uuid).await?;
        match db {
            DB::Multi(x) => {
                let x = x.open_db(db_name).await?;
                let db = DB::One(x.clone());
                self.dbs
                    .lock()
                    .await
                    .insert(format!("{}-{}", uuid, db_name), db.clone());
                anyhow::Ok(db)
            }

            DB::One(_) => anyhow::bail!("this is not a multi database"),
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
        let db = self.get_one_db(uuid).await?;
        db.list_table().await
    }

    pub async fn show_columns(&self, uuid: &str, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let db = self.get_one_db(uuid).await?;
        db.show_columns(table_name).await
    }

    pub async fn show_table(
        &self, uuid: &str, table_name: &str, where_str: Option<&str>, order_by: Option<&str>,
        page: usize, page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let db = self.get_one_db(uuid).await?;
        db.show_table(table_name, where_str, order_by, page, page_size)
            .await
    }

    pub async fn show_indexes(&self, uuid: &str, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let db = self.get_one_db(uuid).await?;
        db.show_indexes(table_name).await
    }

    pub async fn insert(
        &self, uuid: &str, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let db = self.get_one_db(uuid).await?;
        db.insert(table_name, values).await
    }

    pub async fn update(
        &self, uuid: &str, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let db = self.get_one_db(uuid).await?;
        db.update(table_name, values, wheres).await
    }

    pub async fn truncate_table(&self, uuid: &str, table_name: &str) -> anyhow::Result<()> {
        let db = self.get_one_db(uuid).await?;
        db.truncate_table(table_name).await
    }

    pub async fn delete(
        &self, uuid: &str, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let db = self.get_one_db(uuid).await?;
        db.delete(table_name, wheres).await
    }

    pub async fn list_view(&self, uuid: &str) -> anyhow::Result<Vec<ListView>> {
        let db = self.get_one_db(uuid).await?;
        db.list_view().await
    }

    pub async fn drop_view(&self, uuid: &str, view_name: &str) -> anyhow::Result<()> {
        let db = self.get_one_db(uuid).await?;
        db.drop_view(view_name).await
    }

    pub async fn drop_table(&self, uuid: &str, table_name: &str) -> anyhow::Result<()> {
        let db = self.get_one_db(uuid).await?;
        db.drop_table(table_name).await
    }

    pub async fn show_view(
        &self, uuid: &str, view_name: &str, where_str: Option<&str>, order_by: Option<&str>,
        page: usize, page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let db = self.get_one_db(uuid).await?;
        db.show_view(view_name, where_str, order_by, page, page_size)
            .await
    }

    async fn get_one_db(&self, uuid: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        match self.get_db(uuid).await? {
            DB::Multi(_) => anyhow::bail!(format!("this '{}' a multi", uuid)),
            DB::One(x) => anyhow::Ok(x.clone()),
        }
    }

    async fn get_db(&self, uuid: &str) -> anyhow::Result<DB> {
        let db = self.dbs.lock().await.get(uuid).map(|x| x.clone());
        let x = match db {
            Some(x) => x,
            None => anyhow::bail!("db '{}' not found", uuid),
        };
        anyhow::Ok(x)
    }
}

#[derive(Clone, serde::Deserialize, serde::Serialize, Debug)]
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

    use super::DBNexusConfigs;

    #[test]
    fn print_config() {
        let s = DBNexusConfigs {
            user_settings: super::UserSettings { page_size: 100 },
            connection_configs: {
                let mut m = HashMap::new();
                let mut m2 = HashMap::new();
                m2.insert("kind".to_string(), "sqlite".to_string());
                m2.insert("path".to_string(), "D:\\sqlite.test.db".to_string());
                m.insert("uuid".to_string(), m2);
                m
            },
        };
        let s = serde_json::to_string_pretty(&s).unwrap();
        println!("config:{}", s);
    }
}
