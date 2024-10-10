pub mod app {
    use db_connection_config::{
        MongoDbConfig, MysqlConfig, OracleConfig, PostgresConfig, RedisConfig, SqlServerConfig,
        SqliteConfig,
    };
    use dbnexus_db::{app, datasource_config};

    tonic::include_proto!("app");

    impl From<datasource_config::MysqlDbConfig> for MysqlDbConfig {
        fn from(value: datasource_config::MysqlDbConfig) -> Self {
            let datasource_config::MysqlDbConfig {
                host,
                port,
                username,
                password,
                db_name,
            } = value;
            MysqlDbConfig {
                host,
                port: port as u32,
                username,
                password,
                db_name,
            }
        }
    }

    impl Into<datasource_config::MysqlDbConfig> for MysqlDbConfig {
        fn into(self) -> datasource_config::MysqlDbConfig {
            let Self {
                host,
                port,
                username,
                password,
                db_name,
            } = self;
            datasource_config::MysqlDbConfig {
                host,
                port: port as u16,
                username,
                password,
                db_name,
            }
        }
    }

    impl From<datasource_config::RedisDbConfig> for RedisDbConfig {
        fn from(value: datasource_config::RedisDbConfig) -> Self {
            let datasource_config::RedisDbConfig {
                host,
                port,
                username,
                password,
                db,
            } = value;
            RedisDbConfig {
                host,
                port: port.map(|x| x as u32),
                username,
                password,
                db,
            }
        }
    }

    impl Into<datasource_config::RedisDbConfig> for RedisDbConfig {
        fn into(self) -> datasource_config::RedisDbConfig {
            let Self {
                host,
                port,
                username,
                password,
                db,
            } = self;
            datasource_config::RedisDbConfig {
                host,
                port: port.map(|x| x as u16),
                username,
                password,
                db,
            }
        }
    }

    impl From<datasource_config::MongoConfig> for MongoConfig {
        fn from(value: datasource_config::MongoConfig) -> Self {
            let datasource_config::MongoConfig {
                host,
                port,
                connect_timeout,
                username,
                password,
            } = value;
            MongoConfig {
                host,
                port: port as u32,
                username,
                password,
                connect_timeout,
            }
        }
    }

    impl Into<datasource_config::MongoConfig> for MongoConfig {
        fn into(self) -> datasource_config::MongoConfig {
            let Self {
                host,
                port,
                username,
                password,
                connect_timeout,
            } = self;
            datasource_config::MongoConfig {
                host,
                port: port as u16,
                username,
                password,
                connect_timeout,
            }
        }
    }

    impl From<datasource_config::PGDbConfig> for PgDbConfig {
        fn from(value: datasource_config::PGDbConfig) -> Self {
            let datasource_config::PGDbConfig {
                host,
                port,
                username,
                password,
                db_name,
            } = value;
            PgDbConfig {
                host,
                port: port as u32,
                username,
                password,
                db_name,
            }
        }
    }

    impl Into<datasource_config::PGDbConfig> for PgDbConfig {
        fn into(self) -> datasource_config::PGDbConfig {
            let Self {
                host,
                port,
                username,
                password,
                db_name,
            } = self;
            datasource_config::PGDbConfig {
                host,
                port: port as u16,
                username,
                password,
                db_name,
            }
        }
    }
    impl From<datasource_config::SqlServerDbConfig> for SqlServerDbConfig {
        fn from(value: datasource_config::SqlServerDbConfig) -> Self {
            let datasource_config::SqlServerDbConfig {
                host,
                port,
                username,
                password,
                db_name,
                trust_cert,
            } = value;
            SqlServerDbConfig {
                host,
                port: port as u32,
                username,
                password,
                db_name,
                trust_cert,
            }
        }
    }

    impl Into<datasource_config::SqlServerDbConfig> for SqlServerDbConfig {
        fn into(self) -> datasource_config::SqlServerDbConfig {
            let Self {
                host,
                port,
                username,
                password,
                db_name,
                trust_cert,
            } = self;
            datasource_config::SqlServerDbConfig {
                host,
                port: port as u16,
                username,
                password,
                db_name,
                trust_cert,
            }
        }
    }

    impl From<datasource_config::OracleDbConfig> for OracleDbConfig {
        fn from(value: datasource_config::OracleDbConfig) -> Self {
            let datasource_config::OracleDbConfig {
                username,
                password,
                connect_str,
            } = value;
            OracleDbConfig {
                username,
                password,
                connect_str,
            }
        }
    }

    impl Into<datasource_config::OracleDbConfig> for OracleDbConfig {
        fn into(self) -> datasource_config::OracleDbConfig {
            let Self {
                username,
                password,
                connect_str,
            } = self;
            datasource_config::OracleDbConfig {
                username,
                password,
                connect_str,
            }
        }
    }
    impl From<app::DBConnectionConfig> for DbConnectionConfig {
        fn from(value: app::DBConnectionConfig) -> Self {
            match value {
                app::DBConnectionConfig::Sqlite {
                    path,
                    read_only,
                    name,
                } => Self {
                    config: Some(db_connection_config::Config::Sqlite(SqliteConfig {
                        path,
                        read_only,
                        name,
                    })),
                },
                app::DBConnectionConfig::Mysql {
                    conn,
                    read_only,
                    name,
                } => Self {
                    config: Some(db_connection_config::Config::Mysql(MysqlConfig {
                        conn: Some(conn.into()),
                        read_only,
                        name,
                    })),
                },
                app::DBConnectionConfig::MariaDB {
                    conn,
                    read_only,
                    name,
                } => Self {
                    config: Some(db_connection_config::Config::Mysql(MysqlConfig {
                        conn: Some(conn.into()),
                        read_only,
                        name,
                    })),
                },
                app::DBConnectionConfig::Postgres {
                    conn,
                    read_only,
                    name,
                    schema,
                } => Self {
                    config: Some(db_connection_config::Config::Postgres(PostgresConfig {
                        conn: Some(conn.into()),
                        read_only,
                        name,
                        schema,
                    })),
                },
                app::DBConnectionConfig::SqlServer {
                    conn,
                    read_only,
                    name,
                    schema,
                } => Self {
                    config: Some(db_connection_config::Config::Sqlserver(SqlServerConfig {
                        conn: Some(conn.into()),
                        read_only,
                        name,
                        schema,
                    })),
                },

                app::DBConnectionConfig::Mongo {
                    conn,
                    read_only,
                    db_name,
                    name,
                } => Self {
                    config: Some(db_connection_config::Config::Mongo(MongoDbConfig {
                        conn: Some(conn.into()),
                        read_only,
                        db_name,
                        name,
                    })),
                },
                app::DBConnectionConfig::Redis {
                    read_only,
                    config,
                    name,
                } => Self {
                    config: Some(db_connection_config::Config::Redis(RedisConfig {
                        config: Some(config.into()),
                        read_only,
                        name,
                    })),
                },
                app::DBConnectionConfig::Oracle {
                    conn,
                    read_only,
                    db_name,
                    call_timeout,
                    name,
                } => Self {
                    config: Some(db_connection_config::Config::Oracle(OracleConfig {
                        conn: Some(conn.into()),
                        read_only,
                        name,
                        db_name,
                        call_timeout,
                    })),
                },
            }
        }
    }

    impl Into<app::DBConnectionConfig> for DbConnectionConfig {
        fn into(self) -> app::DBConnectionConfig {
            match self.config.unwrap() {
                db_connection_config::Config::Sqlite(sqlite_config) => {
                    let SqliteConfig {
                        path,
                        read_only,
                        name,
                    } = sqlite_config;
                    app::DBConnectionConfig::Sqlite {
                        path,
                        read_only,
                        name,
                    }
                }
                db_connection_config::Config::Mysql(mysql_config) => {
                    let MysqlConfig {
                        conn,
                        read_only,
                        name,
                    } = mysql_config;
                    app::DBConnectionConfig::Mysql {
                        conn: conn.unwrap().into(),
                        read_only,
                        name,
                    }
                }
                db_connection_config::Config::Mariadb(maria_db_config) => todo!(),
                db_connection_config::Config::Postgres(postgres_config) => todo!(),
                db_connection_config::Config::Sqlserver(sql_server_config) => todo!(),
                db_connection_config::Config::Mongo(mongo_db_config) => todo!(),
                db_connection_config::Config::Redis(redis_config) => todo!(),
                db_connection_config::Config::Oracle(oracle_config) => todo!(),
            }
        }
    }
}
