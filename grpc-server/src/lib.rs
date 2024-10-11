pub mod app_grpc_def {

    use std::{collections::HashMap, hash::Hash};

    use dbnexus_db::{app as applib, datasource_config, db, meta_value, redis};

    include!("../proto_generated/app.rs");

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
                applib::DBConnectionConfig::Redis { conn: config, name } => Self {
                    config: Some(db_connection_config::Config::Redis(
                        db_connection_config::RedisConfig {
                            conn: Some(config.into()),
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
                db_connection_config::Config::Mariadb(config) => {
                    let db_connection_config::MariaDbConfig { conn, name } = config;
                    applib::DBConnectionConfig::Mysql {
                        conn: conn.unwrap().into(),
                        name,
                    }
                }
                db_connection_config::Config::Postgres(config) => {
                    let db_connection_config::PostgresConfig { conn, name } = config;
                    applib::DBConnectionConfig::Postgres {
                        conn: conn.unwrap().into(),
                        name,
                    }
                }
                db_connection_config::Config::Sqlserver(config) => {
                    let db_connection_config::SqlServerConfig { conn, name } = config;
                    applib::DBConnectionConfig::SqlServer {
                        conn: conn.unwrap().into(),
                        name,
                    }
                }
                db_connection_config::Config::Mongo(config) => {
                    let db_connection_config::MongoDbConfig { conn, name } = config;
                    applib::DBConnectionConfig::Mongo {
                        conn: conn.unwrap().into(),
                        name,
                    }
                }
                db_connection_config::Config::Redis(config) => {
                    let db_connection_config::RedisConfig { conn, name } = config;
                    applib::DBConnectionConfig::Redis {
                        conn: conn.unwrap().into(),
                        name,
                    }
                }
                db_connection_config::Config::Oracle(config) => {
                    let db_connection_config::OracleConfig { conn, name } = config;
                    applib::DBConnectionConfig::Oracle {
                        conn: conn.unwrap().into(),
                        name,
                    }
                }
            }
        }
    }

    impl From<db::ListTable> for ListTableRespItem {
        fn from(value: db::ListTable) -> Self {
            let db::ListTable { name, size } = value;
            Self {
                name,
                size: size.map(|x| x.to_string()).unwrap_or_default(),
            }
        }
    }

    impl From<redis::RedisListTable> for RedisListTableResp {
        fn from(value: redis::RedisListTable) -> Self {
            let redis::RedisListTable {
                string_tables,
                list_tables,
                set_tables,
                hash_tables,
                zset_tables,
            } = value;

            RedisListTableResp {
                string_tables,
                list_tables,
                set_tables,
                hash_tables,
                zset_tables,
            }
        }
    }
    impl From<db::Field> for Field {
        fn from(value: db::Field) -> Self {
            let db::Field {
                name,
                typ,
                nullable,
                default_value,
                is_primary,
                is_auto_increment,
            } = value;
            Field {
                name,
                typ,
                nullable,
                default_value,
                is_primary,
                is_auto_increment,
            }
        }
    }

    impl From<Option<String>> for RowData {
        fn from(value: Option<String>) -> Self {
            Self { data: value }
        }
    }
    impl From<Vec<Option<String>>> for Row {
        fn from(value: Vec<Option<String>>) -> Self {
            Self {
                values: value.into_iter().map(|x| x.into()).collect(),
            }
        }
    }
    impl From<db::TableListResp> for TableListResp {
        fn from(value: db::TableListResp) -> Self {
            let db::TableListResp {
                fields,
                total,
                sql,
                data,
            } = value;
            Self {
                fields: fields.into_iter().map(|x| x.into()).collect(),
                total: total as u64,
                data: data.into_iter().map(|x| x.into()).collect(),
                sql,
            }
        }
    }
    impl From<db::IndexKind> for IndexKind {
        fn from(value: db::IndexKind) -> Self {
            match value {
                db::IndexKind::Normal => Self::Normal,
                db::IndexKind::Unique => Self::Unique,
                db::IndexKind::Primary => Self::Primary,
            }
        }
    }
    impl From<db::Index> for Index {
        fn from(value: db::Index) -> Self {
            let db::Index { name, kind, on } = value;
            Self {
                name,
                kind: IndexKind::from(kind) as i32,
                on,
            }
        }
    }
    impl From<Option<String>> for String2OptionStringValue {
        fn from(value: Option<String>) -> Self {
            Self { value }
        }
    }
    impl From<String2OptionStringValue> for Option<String> {
        fn from(value: String2OptionStringValue) -> Self {
            value.value
        }
    }

    impl From<HashMap<String, Option<String>>> for String2OptionString {
        fn from(value: HashMap<String, Option<String>>) -> Self {
            let mut m = HashMap::new();
            value
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .for_each(|(k, v)| {
                    m.insert(k, v);
                });
            Self { values: m }
        }
    }
    impl From<String2OptionString> for HashMap<String, Option<String>> {
        fn from(value: String2OptionString) -> Self {
            let mut m = HashMap::new();
            value
                .values
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .for_each(|(k, v)| {
                    m.insert(k, v);
                });
            m
        }
    }
    impl From<db::SqlExecuteRet> for SqlExecuteRet {
        fn from(value: db::SqlExecuteRet) -> Self {
            let db::SqlExecuteRet {
                sql,
                changes,
                last_insert_row_id,
                last_insert_id_string,
            } = value;
            Self {
                sql,
                changes,
                last_insert_row_id,
                last_insert_id_string,
            }
        }
    }
    impl From<db::NamedStaff> for NamedStaff {
        fn from(value: db::NamedStaff) -> Self {
            let db::NamedStaff { name } = value;
            Self { name }
        }
    }
    impl From<Vec<db::NamedStaff>> for NamedStaffVec {
        fn from(value: Vec<db::NamedStaff>) -> Self {
            Self {
                named: value.into_iter().map(|x| x.into()).collect(),
            }
        }
    }
    impl From<String> for StringResp {
        fn from(value: String) -> Self {
            Self { value }
        }
    }
    impl From<db::QueryResp> for QueryResp {
        fn from(value: db::QueryResp) -> Self {
            let db::QueryResp { fields, data } = value;
            Self {
                data: data.into_iter().map(|x| x.into()).collect(),
                fields: fields.into_iter().map(|x| x.into()).collect(),
            }
        }
    }

    impl From<Result<db::QueryResp, String>> for QueryRespOrErr {
        fn from(value: Result<db::QueryResp, String>) -> Self {
            match value {
                Ok(x) => QueryRespOrErr {
                    result: Some(query_resp_or_err::Result::Resp(x.into())),
                },
                Err(err) => QueryRespOrErr {
                    result: Some(query_resp_or_err::Result::Err(Err { err })),
                },
            }
        }
    }

    impl From<applib::UserSettings> for UserSettings {
        fn from(value: applib::UserSettings) -> Self {
            let applib::UserSettings { page_size } = value;
            Self {
                page_size: page_size as u32,
            }
        }
    }
    impl From<UserSettings> for applib::UserSettings {
        fn from(value: UserSettings) -> Self {
            let UserSettings { page_size } = value;
            Self {
                page_size: page_size as usize,
            }
        }
    }
    impl From<db::TriggerTiming> for TriggerTiming {
        fn from(value: db::TriggerTiming) -> Self {
            match value {
                db::TriggerTiming::After => TriggerTiming::After,
                db::TriggerTiming::Before => TriggerTiming::Before,
                db::TriggerTiming::InsteadOf => TriggerTiming::InsteadOf,
            }
        }
    }
    impl From<db::TriggerEvent> for TriggerEvent {
        fn from(value: db::TriggerEvent) -> Self {
            match value {
                db::TriggerEvent::Delete => TriggerEvent::Delete,
                db::TriggerEvent::Update => TriggerEvent::Update,
                db::TriggerEvent::Insert => TriggerEvent::Insert,
            }
        }
    }
    impl From<db::TriggerEnable> for TriggerEnable {
        fn from(value: db::TriggerEnable) -> Self {
            match value {
                db::TriggerEnable::Bool { enable } => TriggerEnable {
                    enable: Some(trigger_enable::Enable::JustEnable(enable)),
                },
                db::TriggerEnable::PostGres { enable } => TriggerEnable {
                    enable: Some(trigger_enable::Enable::Postgres(TriggerEnablePostgres {
                        enable: enable.map(|x| x.to_string()),
                    })),
                },
            }
        }
    }
    impl From<meta_value::MetaValue> for MetaValue {
        fn from(value: meta_value::MetaValue) -> Self {
            let meta_value::MetaValue { name, value, desc } = value;
            Self {
                name: name.into(),
                value: value.map(|x| x.to_string()),
                desc: desc.map(|x| x.to_string()),
            }
        }
    }
    impl From<db::TriggerDetail> for TriggerDetail {
        fn from(value: db::TriggerDetail) -> Self {
            let db::TriggerDetail {
                timing,
                event,
                sql,
                enable,
                metas,
            } = value;

            Self {
                timing: TriggerTiming::from(timing) as i32,
                event: TriggerEvent::from(event) as i32,
                sql: sql.into(),
                enable: enable.map(|x| x.into()),
                metas: metas.into_iter().map(|x| x.into()).collect(),
            }
        }
    }

    impl From<HashMap<String, applib::DBConnectionConfig>> for ListAllConfigResp {
        fn from(value: HashMap<String, applib::DBConnectionConfig>) -> Self {
            let mut m = HashMap::new();
            value
                .into_iter()
                .map(|(k, v)| (k, v.into()))
                .for_each(|(k, v)| {
                    m.insert(k, v);
                });

            ListAllConfigResp { items: m }
        }
    }
}
