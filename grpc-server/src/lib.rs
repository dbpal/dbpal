pub mod app_grpc_def {

    use dbnexus_db::{app as applib, datasource_config};

    tonic::include_proto!("app");

    impl From<datasource_config::MysqlDbConfig> for MysqlDbConfig {
        fn from(value: datasource_config::MysqlDbConfig) -> Self {
            let datasource_config::MysqlDbConfig {
                host,
                port,
                username,
                password,
                db_name,
                read_only,
            } = value;
            MysqlDbConfig {
                host,
                port: port as u32,
                username,
                password,
                db_name,
                read_only,
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
                read_only,
            } = self;
            datasource_config::MysqlDbConfig {
                host,
                port: port as u16,
                username,
                password,
                db_name,
                read_only,
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
                read_only,
            } = value;
            RedisDbConfig {
                host,
                port: port.map(|x| x as u32),
                username,
                password,
                db,
                read_only,
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
                read_only,
            } = self;
            datasource_config::RedisDbConfig {
                host,
                port: port.map(|x| x as u16),
                username,
                password,
                db,
                read_only,
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
                db,
                read_only,
            } = value;
            MongoConfig {
                host,
                port: port as u32,
                username,
                password,
                connect_timeout,
                db,
                read_only,
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
                db,
                read_only,
            } = self;
            datasource_config::MongoConfig {
                host,
                port: port as u16,
                username,
                password,
                connect_timeout,
                db,
                read_only,
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
                schema,
                read_only,
            } = value;
            PgDbConfig {
                host,
                port: port as u32,
                username,
                password,
                db_name,
                schema,
                read_only,
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
                schema,
                read_only,
            } = self;
            datasource_config::PGDbConfig {
                host,
                port: port as u16,
                username,
                password,
                db_name,
                schema,
                read_only,
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
                schema,
                read_only,
            } = value;
            SqlServerDbConfig {
                host,
                port: port as u32,
                username,
                password,
                db_name,
                trust_cert,
                schema,
                read_only,
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
                schema,
                read_only,
            } = self;
            datasource_config::SqlServerDbConfig {
                host,
                port: port as u16,
                username,
                password,
                db_name,
                trust_cert,
                schema,
                read_only,
            }
        }
    }

    impl From<datasource_config::OracleDbConfig> for OracleDbConfig {
        fn from(value: datasource_config::OracleDbConfig) -> Self {
            let datasource_config::OracleDbConfig {
                username,
                password,
                connect_str,
                schema,
                call_timeout,
                read_only,
            } = value;
            OracleDbConfig {
                username,
                password,
                connect_str,
                schema,
                call_timeout,
                read_only,
            }
        }
    }

    impl Into<datasource_config::OracleDbConfig> for OracleDbConfig {
        fn into(self) -> datasource_config::OracleDbConfig {
            let Self {
                username,
                password,
                connect_str,
                schema,
                call_timeout,
                read_only,
            } = self;
            datasource_config::OracleDbConfig {
                username,
                password,
                connect_str,
                schema,
                call_timeout,
                read_only,
            }
        }
    }

    impl From<applib::DBConnectionConfig> for DbConnectionConfig {
        fn from(value: applib::DBConnectionConfig) -> Self {
            match value {
                applib::DBConnectionConfig::Sqlite {
                    path,
                    read_only,
                    name,
                } => Self {
                    config: Some(db_connection_config::Config::Sqlite(
                        db_connection_config::SqliteConfig {
                            path,
                            name,
                            read_only,
                        },
                    )),
                },
                applib::DBConnectionConfig::Mysql { conn, name } => Self {
                    config: Some(db_connection_config::Config::Mysql(
                        db_connection_config::MysqlConfig {
                            conn: Some(conn.into()),

                            name,
                        },
                    )),
                },
                applib::DBConnectionConfig::MariaDB { conn, name } => Self {
                    config: Some(db_connection_config::Config::Mysql(
                        db_connection_config::MysqlConfig {
                            conn: Some(conn.into()),

                            name,
                        },
                    )),
                },
                applib::DBConnectionConfig::Postgres { conn, name } => Self {
                    config: Some(db_connection_config::Config::Postgres(
                        db_connection_config::PostgresConfig {
                            conn: Some(conn.into()),
                            name,
                        },
                    )),
                },
                applib::DBConnectionConfig::SqlServer { conn, name } => Self {
                    config: Some(db_connection_config::Config::Sqlserver(
                        db_connection_config::SqlServerConfig {
                            conn: Some(conn.into()),

                            name,
                        },
                    )),
                },

                applib::DBConnectionConfig::Mongo { conn, name } => Self {
                    config: Some(db_connection_config::Config::Mongo(
                        db_connection_config::MongoDbConfig {
                            conn: Some(conn.into()),

                            name,
                        },
                    )),
                },
                applib::DBConnectionConfig::Redis { config, name } => Self {
                    config: Some(db_connection_config::Config::Redis(
                        db_connection_config::RedisConfig {
                            config: Some(config.into()),
                            name,
                        },
                    )),
                },
                applib::DBConnectionConfig::Oracle { conn, name } => Self {
                    config: Some(db_connection_config::Config::Oracle(
                        db_connection_config::OracleConfig {
                            conn: Some(conn.into()),

                            name,
                        },
                    )),
                },
            }
        }
    }

    impl Into<applib::DBConnectionConfig> for DbConnectionConfig {
        fn into(self) -> applib::DBConnectionConfig {
            match self.config.unwrap() {
                db_connection_config::Config::Sqlite(sqlite_config) => {
                    let db_connection_config::SqliteConfig {
                        path,
                        name,
                        read_only,
                    } = sqlite_config;
                    applib::DBConnectionConfig::Sqlite {
                        path,
                        name,
                        read_only,
                    }
                }
                db_connection_config::Config::Mysql(mysql_config) => {
                    let db_connection_config::MysqlConfig { conn, name } = mysql_config;
                    applib::DBConnectionConfig::Mysql {
                        conn: conn.unwrap().into(),
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
