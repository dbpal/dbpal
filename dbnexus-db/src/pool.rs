use std::sync::Arc;

use dashmap::DashMap;
use mongodb::options::ServerAddress;
use sqlx::{MySqlPool, PgPool, SqlitePool};

use crate::datasource::DBConfig;

#[derive(Default, Clone)]
pub struct PoolManager {
    map: Arc<DashMap<DBConfig, NexusPool>>,
}

impl PoolManager {
    pub fn new() -> Self {
        Self::default()
    }
}

impl PoolManager {
    pub async fn get(&self, ds: &crate::datasource::DBConfig) -> anyhow::Result<NexusPool> {
        if let Some(p) = self.map.get(ds) {
            return anyhow::Result::Ok(p.clone());
        }
        let p = make_pool(ds).await?;
        self.map.insert(ds.clone(), p.clone());
        return anyhow::Ok(p);
    }
}

async fn make_pool(db: &DBConfig) -> anyhow::Result<crate::pool::NexusPool> {
    use crate::pool::NexusPool::*;

    match db {
        DBConfig::Sqlite { path } => {
            let pool = sqlx::sqlite::SqlitePool::connect(path).await?;
            anyhow::Ok(Sqlite(pool))
        }
        DBConfig::Mysql { conn } => {
            let mut opts = sqlx::mysql::MySqlConnectOptions::new();
            opts = opts
                .host(&conn.host)
                .port(conn.port)
                .username(&conn.username)
                .password(&conn.password);
            if let Some(db) = &conn.db_name {
                opts = opts.database(&db);
            }
            let pool = sqlx::mysql::MySqlPool::connect_with(opts).await?;
            anyhow::Ok(Mysql(pool))
        }
        DBConfig::PostGres { conn } => {
            let mut opts = sqlx::postgres::PgConnectOptions::new();
            opts = opts
                .host(&conn.host)
                .port(conn.port)
                .username(&conn.username)
                .password(&conn.password);
            if let Some(db) = &conn.db_name {
                opts = opts.database(&db);
            }

            let pool = sqlx::postgres::PgPool::connect_with(opts).await?;
            anyhow::Ok(Pg(pool))
        }
        DBConfig::Redis { server } => {
            let client = redis::Client::open(server.as_str())?;
            let m = redis::aio::ConnectionManager::new(client).await?;
            anyhow::Ok(Redis(m))
        }

        DBConfig::MongoDB(ds) => {
            let mut opts = mongodb::options::ClientOptions::default();
            opts.hosts = vec![ServerAddress::Tcp {
                host: ds.host.to_string(),
                port: Some(ds.port),
            }];

            if let Some(_user) = &ds.username_and_password {
                // TODO
            }
            let client = mongodb::Client::with_options(opts)?;
            anyhow::Ok(MongoDB(client))
        }
        DBConfig::SqlServer {
            config: c,
            trust_cert,
        } => {
            let mut config = tiberius::Config::new();
            config.host(&c.host);
            config.port(c.port);
            config.authentication(tiberius::AuthMethod::sql_server(&c.username, &c.password));
            if let Some(db) = &c.db_name {
                config.database(&db);
            }

            if *trust_cert {
                config.trust_cert(); // on production, it is not a good idea to do this
            }
            let manager = bb8_tiberius::ConnectionManager::new(config);
            let pool = bb8::Pool::builder().max_size(15).build(manager).await?;
            anyhow::Ok(SqlServer(pool))
        }
        DBConfig::Oracle(config) => {
            let pool = oracle::pool::PoolBuilder::new(
                &config.username,
                &config.password,
                &config.connect_str,
            )
            .max_connections(20)
            .build()?;
            anyhow::Ok(Oracle(pool))
        }
    }
}

#[derive(Clone)]
pub enum NexusPool {
    Sqlite(SqlitePool),
    Mysql(MySqlPool),
    Pg(PgPool),
    MongoDB(mongodb::Client),
    Redis(redis::aio::ConnectionManager),
    SqlServer(bb8::Pool<bb8_tiberius::ConnectionManager>),
    Oracle(oracle::pool::Pool),
}
