#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HelloRequest {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HelloResponse {
    #[prost(string, tag = "1")]
    pub message: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MysqlDbConfig {
    #[prost(string, tag = "1")]
    pub host: ::prost::alloc::string::String,
    /// Protobuf 中没有 u16，因此使用 uint32
    #[prost(uint32, tag = "2")]
    pub port: u32,
    #[prost(string, tag = "3")]
    pub username: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub password: ::prost::alloc::string::String,
    /// 可选字段将由 Protobuf 处理
    #[prost(string, optional, tag = "5")]
    pub db_name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, tag = "6")]
    pub read_only: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RedisDbConfig {
    #[prost(string, tag = "1")]
    pub host: ::prost::alloc::string::String,
    /// 使用 uint32 表示无符号整数
    #[prost(uint32, optional, tag = "2")]
    pub port: ::core::option::Option<u32>,
    /// 可选字段
    #[prost(string, optional, tag = "3")]
    pub username: ::core::option::Option<::prost::alloc::string::String>,
    /// 可选字段
    #[prost(string, optional, tag = "4")]
    pub password: ::core::option::Option<::prost::alloc::string::String>,
    /// 可选字段
    #[prost(string, optional, tag = "5")]
    pub db: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, tag = "6")]
    pub read_only: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MongoConfig {
    #[prost(string, tag = "1")]
    pub host: ::prost::alloc::string::String,
    /// 使用 uint32 表示无符号整数
    #[prost(uint32, tag = "2")]
    pub port: u32,
    /// 可选字段
    #[prost(string, optional, tag = "3")]
    pub username: ::core::option::Option<::prost::alloc::string::String>,
    /// 可选字段
    #[prost(string, optional, tag = "4")]
    pub password: ::core::option::Option<::prost::alloc::string::String>,
    /// 毫秒
    #[prost(uint64, optional, tag = "5")]
    pub connect_timeout: ::core::option::Option<u64>,
    #[prost(string, optional, tag = "6")]
    pub db: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, tag = "7")]
    pub read_only: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct PgDbConfig {
    #[prost(string, tag = "1")]
    pub host: ::prost::alloc::string::String,
    /// 使用 uint32 表示无符号整数
    #[prost(uint32, tag = "2")]
    pub port: u32,
    #[prost(string, tag = "3")]
    pub username: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub password: ::prost::alloc::string::String,
    /// 可选字段
    #[prost(string, optional, tag = "5")]
    pub db_name: ::core::option::Option<::prost::alloc::string::String>,
    /// 可选字段
    #[prost(string, optional, tag = "6")]
    pub schema: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, tag = "7")]
    pub read_only: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SqlServerDbConfig {
    #[prost(string, tag = "1")]
    pub host: ::prost::alloc::string::String,
    /// 使用 uint32 表示无符号整数
    #[prost(uint32, tag = "2")]
    pub port: u32,
    #[prost(string, tag = "3")]
    pub username: ::prost::alloc::string::String,
    #[prost(string, tag = "4")]
    pub password: ::prost::alloc::string::String,
    /// 可选字段
    #[prost(string, optional, tag = "5")]
    pub db_name: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, tag = "6")]
    pub trust_cert: bool,
    #[prost(string, optional, tag = "7")]
    pub schema: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, tag = "8")]
    pub read_only: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OracleDbConfig {
    #[prost(string, tag = "1")]
    pub username: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub password: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub connect_str: ::prost::alloc::string::String,
    #[prost(string, optional, tag = "4")]
    pub schema: ::core::option::Option<::prost::alloc::string::String>,
    /// 毫秒
    #[prost(uint64, optional, tag = "5")]
    pub call_timeout: ::core::option::Option<u64>,
    #[prost(bool, tag = "6")]
    pub read_only: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DbConnectionConfig {
    #[prost(oneof = "db_connection_config::Config", tags = "1, 2, 3, 4, 5, 6, 7, 8")]
    pub config: ::core::option::Option<db_connection_config::Config>,
}
/// Nested message and enum types in `DBConnectionConfig`.
pub mod db_connection_config {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct SqliteConfig {
        #[prost(string, tag = "1")]
        pub path: ::prost::alloc::string::String,
        #[prost(string, tag = "2")]
        pub name: ::prost::alloc::string::String,
        #[prost(bool, tag = "3")]
        pub read_only: bool,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct MysqlConfig {
        /// 假设 MysqlDbConfig 已经定义
        #[prost(message, optional, tag = "1")]
        pub conn: ::core::option::Option<super::MysqlDbConfig>,
        #[prost(string, tag = "2")]
        pub name: ::prost::alloc::string::String,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct MariaDbConfig {
        /// 假设 MysqlDbConfig 已经定义
        #[prost(message, optional, tag = "1")]
        pub conn: ::core::option::Option<super::MysqlDbConfig>,
        #[prost(string, tag = "2")]
        pub name: ::prost::alloc::string::String,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct PostgresConfig {
        /// 假设 PGDbConfig 已经定义
        #[prost(message, optional, tag = "1")]
        pub conn: ::core::option::Option<super::PgDbConfig>,
        #[prost(string, tag = "2")]
        pub name: ::prost::alloc::string::String,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct SqlServerConfig {
        /// 假设 SqlServerDbConfig 已经定义
        #[prost(message, optional, tag = "1")]
        pub conn: ::core::option::Option<super::SqlServerDbConfig>,
        #[prost(string, tag = "2")]
        pub name: ::prost::alloc::string::String,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct MongoDbConfig {
        /// 假设 MongoConfig 已经定义
        #[prost(message, optional, tag = "1")]
        pub conn: ::core::option::Option<super::MongoConfig>,
        #[prost(string, tag = "2")]
        pub name: ::prost::alloc::string::String,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct RedisConfig {
        #[prost(message, optional, tag = "1")]
        pub conn: ::core::option::Option<super::RedisDbConfig>,
        #[prost(string, tag = "2")]
        pub name: ::prost::alloc::string::String,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct OracleConfig {
        /// 假设 OracleDbConfig 已经定义
        #[prost(message, optional, tag = "1")]
        pub conn: ::core::option::Option<super::OracleDbConfig>,
        #[prost(string, tag = "2")]
        pub name: ::prost::alloc::string::String,
    }
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Config {
        #[prost(message, tag = "1")]
        Sqlite(SqliteConfig),
        #[prost(message, tag = "2")]
        Mysql(MysqlConfig),
        #[prost(message, tag = "3")]
        Mariadb(MariaDbConfig),
        #[prost(message, tag = "4")]
        Postgres(PostgresConfig),
        #[prost(message, tag = "5")]
        Sqlserver(SqlServerConfig),
        #[prost(message, tag = "6")]
        Mongo(MongoDbConfig),
        #[prost(message, tag = "7")]
        Redis(RedisConfig),
        #[prost(message, tag = "8")]
        Oracle(OracleConfig),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateConfig {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "2")]
    pub config: ::core::option::Option<DbConnectionConfig>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AddConfigResp {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DelConfigReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListAllConfigResp {
    #[prost(map = "string, message", tag = "1")]
    pub items: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        DbConnectionConfig,
    >,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TestConnResp {
    #[prost(uint64, tag = "1")]
    pub ping_value: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OnlyUuidReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListTableResp {
    #[prost(message, repeated, tag = "1")]
    pub tables: ::prost::alloc::vec::Vec<ListTableRespItem>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListTableRespItem {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub size: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RedisListTableResp {
    #[prost(string, repeated, tag = "1")]
    pub string_tables: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag = "2")]
    pub list_tables: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag = "3")]
    pub set_tables: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag = "4")]
    pub zset_tables: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, repeated, tag = "5")]
    pub hash_tables: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UuidAndName {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    /// maybe table view name staff.
    /// uuid and name can locate a lot of things.
    #[prost(string, tag = "2")]
    pub name: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Field {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub typ: ::prost::alloc::string::String,
    #[prost(bool, tag = "3")]
    pub nullable: bool,
    #[prost(string, optional, tag = "4")]
    pub default_value: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(bool, tag = "5")]
    pub is_primary: bool,
    #[prost(bool, tag = "6")]
    pub is_auto_increment: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ShowColumnsResp {
    #[prost(message, repeated, tag = "1")]
    pub columns: ::prost::alloc::vec::Vec<Field>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RedisMatchKeysResp {
    #[prost(string, repeated, tag = "1")]
    pub keys: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TableViewListReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub table_name: ::prost::alloc::string::String,
    #[prost(string, optional, tag = "3")]
    pub where_str: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "4")]
    pub order_by: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint64, tag = "5")]
    pub page: u64,
    #[prost(uint64, tag = "6")]
    pub page_size: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TableListResp {
    /// List of fields
    #[prost(message, repeated, tag = "1")]
    pub fields: ::prost::alloc::vec::Vec<Field>,
    /// Total number of records
    #[prost(uint64, tag = "2")]
    pub total: u64,
    /// List of rows, each row is a list of optional strings
    #[prost(message, repeated, tag = "3")]
    pub data: ::prost::alloc::vec::Vec<Row>,
    /// SQL query string
    #[prost(string, optional, tag = "4")]
    pub sql: ::core::option::Option<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Row {
    /// List of optional string values
    #[prost(message, repeated, tag = "1")]
    pub values: ::prost::alloc::vec::Vec<RowData>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Index {
    /// Name of the index
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    /// Type of index
    #[prost(enumeration = "IndexKind", tag = "2")]
    pub kind: i32,
    /// List of columns the index is on
    #[prost(string, repeated, tag = "3")]
    pub on: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RowData {
    #[prost(string, optional, tag = "1")]
    pub data: ::core::option::Option<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ShowIndexResp {
    #[prost(message, repeated, tag = "1")]
    pub indexes: ::prost::alloc::vec::Vec<Index>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct String2OptionString {
    #[prost(map = "string, message", tag = "1")]
    pub values: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        String2OptionStringValue,
    >,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct String2OptionStringValue {
    #[prost(string, optional, tag = "1")]
    pub value: ::core::option::Option<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InsertReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub table_name: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub values: ::core::option::Option<String2OptionString>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SqlExecuteRet {
    /// The executed SQL statement
    #[prost(string, tag = "1")]
    pub sql: ::prost::alloc::string::String,
    /// Number of changes made by the SQL statement
    #[prost(uint64, tag = "2")]
    pub changes: u64,
    /// ID of the last inserted row (optional)
    #[prost(int64, optional, tag = "3")]
    pub last_insert_row_id: ::core::option::Option<i64>,
    /// Last inserted ID as a string (optional, for databases like MongoDB)
    #[prost(string, optional, tag = "4")]
    pub last_insert_id_string: ::core::option::Option<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UpdateReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub table_name: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub values: ::core::option::Option<String2OptionString>,
    #[prost(message, optional, tag = "4")]
    pub wheres: ::core::option::Option<String2OptionString>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DeleteReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub table_name: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "4")]
    pub wheres: ::core::option::Option<String2OptionString>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NamedStaff {
    /// Name of the staff member
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NamedStaffVec {
    #[prost(message, repeated, tag = "1")]
    pub named: ::prost::alloc::vec::Vec<NamedStaff>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OpendKindResp {
    #[prost(enumeration = "OpendKind", tag = "1")]
    pub kind: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct StringResp {
    #[prost(string, tag = "1")]
    pub value: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QuerySqlReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub sql: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub query_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QuerySqlResp {
    #[prost(message, repeated, tag = "1")]
    pub results: ::prost::alloc::vec::Vec<QueryRespOrErr>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryResp {
    #[prost(message, repeated, tag = "1")]
    pub fields: ::prost::alloc::vec::Vec<Field>,
    #[prost(message, repeated, tag = "2")]
    pub data: ::prost::alloc::vec::Vec<Row>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct QueryRespOrErr {
    #[prost(oneof = "query_resp_or_err::Result", tags = "1, 2")]
    pub result: ::core::option::Option<query_resp_or_err::Result>,
}
/// Nested message and enum types in `QueryRespOrErr`.
pub mod query_resp_or_err {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Result {
        #[prost(message, tag = "1")]
        Err(super::Err),
        #[prost(message, tag = "2")]
        Resp(super::QueryResp),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Err {
    #[prost(string, tag = "1")]
    pub err: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UuidAndQueryId {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub query_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UserSettings {
    /// Size of the page
    #[prost(uint32, tag = "1")]
    pub page_size: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FormatSqlReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub sql: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TriggerEnable {
    #[prost(oneof = "trigger_enable::Enable", tags = "1, 2")]
    pub enable: ::core::option::Option<trigger_enable::Enable>,
}
/// Nested message and enum types in `TriggerEnable`.
pub mod trigger_enable {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Enable {
        #[prost(message, tag = "1")]
        Postgres(super::TriggerEnablePostgres),
        #[prost(bool, tag = "2")]
        JustEnable(bool),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TriggerEnablePostgres {
    #[prost(string, optional, tag = "1")]
    pub enable: ::core::option::Option<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CommonriggerReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub table: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub name: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct TriggerDetail {
    /// Trigger timing (Before, After, InsteadOf)
    #[prost(enumeration = "TriggerTiming", tag = "1")]
    pub timing: i32,
    /// Trigger event (Insert, Update, Delete)
    #[prost(enumeration = "TriggerEvent", tag = "2")]
    pub event: i32,
    /// SQL statement for the trigger
    #[prost(string, tag = "3")]
    pub sql: ::prost::alloc::string::String,
    /// Trigger enable options (optional)
    #[prost(message, optional, tag = "4")]
    pub enable: ::core::option::Option<TriggerEnable>,
    /// List of metadata values
    #[prost(message, repeated, tag = "5")]
    pub metas: ::prost::alloc::vec::Vec<MetaValue>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct MetaValue {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, optional, tag = "2")]
    pub value: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "3")]
    pub desc: ::core::option::Option<::prost::alloc::string::String>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RenameTableReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub old_table: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub new_table: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DuplicateTableReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub table: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub dup: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UuidAndPattern {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub pattern: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportTableReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub table_name: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub filepath: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "4")]
    pub kind: ::core::option::Option<ExportKind>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportQueryResultReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub sql: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub filepath: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "4")]
    pub kind: ::core::option::Option<ExportKind>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportKind {
    #[prost(oneof = "export_kind::Kind", tags = "1, 2, 3, 4")]
    pub kind: ::core::option::Option<export_kind::Kind>,
}
/// Nested message and enum types in `ExportKind`.
pub mod export_kind {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Kind {
        #[prost(message, tag = "1")]
        Csv(super::ExportKindCsv),
        #[prost(message, tag = "2")]
        Json(super::ExportKindJson),
        #[prost(message, tag = "3")]
        JsonLine(super::ExportKindJsonLine),
        #[prost(message, tag = "4")]
        Sql(super::ExportKindSql),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportKindCsv {
    #[prost(bool, tag = "1")]
    pub include_header: bool,
    /// using string for char representation
    #[prost(string, tag = "2")]
    pub delimiter: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportKindJson {
    #[prost(bool, tag = "1")]
    pub pretty: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportKindJsonLine {}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExportKindSql {
    #[prost(bool, tag = "1")]
    pub include_table_schema: bool,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct UdfDetail {
    /// Definition of the SQL.
    #[prost(string, tag = "1")]
    pub def: ::prost::alloc::string::String,
    /// List of MetaValue.
    #[prost(message, repeated, tag = "2")]
    pub metas: ::prost::alloc::vec::Vec<MetaValue>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DropUdfExtraParameter {
    #[prost(oneof = "drop_udf_extra_parameter::ParameterType", tags = "1")]
    pub parameter_type: ::core::option::Option<drop_udf_extra_parameter::ParameterType>,
}
/// Nested message and enum types in `DropUdfExtraParameter`.
pub mod drop_udf_extra_parameter {
    #[allow(clippy::derive_partial_eq_without_eq)]
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum ParameterType {
        #[prost(message, tag = "1")]
        Postgres(super::DropUdfExtraParameterPostGres),
    }
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DropUdfExtraParameterPostGres {
    /// e.g., "a integer, b integer"
    #[prost(string, tag = "1")]
    pub parameters: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DropUdfReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub name: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "3")]
    pub extra: ::core::option::Option<DropUdfExtraParameter>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ProcedureDetail {
    /// Definition of the SQL.
    #[prost(string, tag = "1")]
    pub def: ::prost::alloc::string::String,
    /// List of MetaValue.
    #[prost(message, repeated, tag = "2")]
    pub metas: ::prost::alloc::vec::Vec<MetaValue>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AutoCompletionReq {
    #[prost(string, tag = "1")]
    pub uuid: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub sql: ::prost::alloc::string::String,
    #[prost(uint32, tag = "3")]
    pub line: u32,
    #[prost(uint32, tag = "4")]
    pub col: u32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AutoCompletionItem {
    #[prost(string, tag = "1")]
    pub item: ::prost::alloc::string::String,
    #[prost(enumeration = "CompletionKind", tag = "2")]
    pub ck: i32,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct AutoCompletionResp {
    #[prost(message, repeated, tag = "1")]
    pub items: ::prost::alloc::vec::Vec<AutoCompletionItem>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum IndexKind {
    /// Normal index
    Normal = 0,
    /// Unique index
    Unique = 1,
    /// Primary index
    Primary = 2,
}
impl IndexKind {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            IndexKind::Normal => "NORMAL",
            IndexKind::Unique => "UNIQUE",
            IndexKind::Primary => "PRIMARY",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "NORMAL" => Some(Self::Normal),
            "UNIQUE" => Some(Self::Unique),
            "PRIMARY" => Some(Self::Primary),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum OpendKind {
    /// Catalog kind
    Catelog = 0,
    /// Database kind
    Database = 1,
    /// Schema kind
    Schema = 2,
}
impl OpendKind {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            OpendKind::Catelog => "CATELOG",
            OpendKind::Database => "DATABASE",
            OpendKind::Schema => "SCHEMA",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "CATELOG" => Some(Self::Catelog),
            "DATABASE" => Some(Self::Database),
            "SCHEMA" => Some(Self::Schema),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TriggerTiming {
    /// Before trigger timing
    Before = 0,
    /// After trigger timing
    After = 1,
    /// Instead of trigger timing
    InsteadOf = 2,
}
impl TriggerTiming {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            TriggerTiming::Before => "BEFORE",
            TriggerTiming::After => "AFTER",
            TriggerTiming::InsteadOf => "INSTEAD_OF",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "BEFORE" => Some(Self::Before),
            "AFTER" => Some(Self::After),
            "INSTEAD_OF" => Some(Self::InsteadOf),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum TriggerEvent {
    /// Insert event
    Insert = 0,
    /// Update event
    Update = 1,
    /// Delete event
    Delete = 2,
}
impl TriggerEvent {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            TriggerEvent::Insert => "INSERT",
            TriggerEvent::Update => "UPDATE",
            TriggerEvent::Delete => "DELETE",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "INSERT" => Some(Self::Insert),
            "UPDATE" => Some(Self::Update),
            "DELETE" => Some(Self::Delete),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum CompletionKind {
    /// Default value for unknown completion kind
    SchemaName = 0,
    /// Command used by Redis
    Command = 1,
    /// Keyword
    Keyword = 2,
    /// Function
    Function = 3,
    /// Database name
    Dbname = 4,
    /// Table name
    Tablename = 5,
    /// Column
    Column = 6,
    /// Text
    Text = 7,
    /// Type
    Type = 8,
}
impl CompletionKind {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            CompletionKind::SchemaName => "SchemaName",
            CompletionKind::Command => "COMMAND",
            CompletionKind::Keyword => "KEYWORD",
            CompletionKind::Function => "FUNCTION",
            CompletionKind::Dbname => "DBNAME",
            CompletionKind::Tablename => "TABLENAME",
            CompletionKind::Column => "COLUMN",
            CompletionKind::Text => "TEXT",
            CompletionKind::Type => "TYPE",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "SchemaName" => Some(Self::SchemaName),
            "COMMAND" => Some(Self::Command),
            "KEYWORD" => Some(Self::Keyword),
            "FUNCTION" => Some(Self::Function),
            "DBNAME" => Some(Self::Dbname),
            "TABLENAME" => Some(Self::Tablename),
            "COLUMN" => Some(Self::Column),
            "TEXT" => Some(Self::Text),
            "TYPE" => Some(Self::Type),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod my_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct MyServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl MyServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> MyServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> MyServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            MyServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn say_hello(
            &mut self,
            request: impl tonic::IntoRequest<super::HelloRequest>,
        ) -> std::result::Result<tonic::Response<super::HelloResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/app.MyService/SayHello");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("app.MyService", "SayHello"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn return_err(
            &mut self,
            request: impl tonic::IntoRequest<super::HelloRequest>,
        ) -> std::result::Result<tonic::Response<super::HelloResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/app.MyService/ReturnErr");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("app.MyService", "ReturnErr"));
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated client implementations.
pub mod config_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct ConfigServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl ConfigServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> ConfigServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> ConfigServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            ConfigServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn add(
            &mut self,
            request: impl tonic::IntoRequest<super::DbConnectionConfig>,
        ) -> std::result::Result<tonic::Response<super::AddConfigResp>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/app.ConfigService/Add");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("app.ConfigService", "Add"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateConfig>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/app.ConfigService/Update");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("app.ConfigService", "Update"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn del(
            &mut self,
            request: impl tonic::IntoRequest<super::DelConfigReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/app.ConfigService/Del");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("app.ConfigService", "Del"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn list_all(
            &mut self,
            request: impl tonic::IntoRequest<()>,
        ) -> std::result::Result<
            tonic::Response<super::ListAllConfigResp>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.ConfigService/ListAll",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("app.ConfigService", "ListAll"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn open(
            &mut self,
            request: impl tonic::IntoRequest<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static("/app.ConfigService/open");
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("app.ConfigService", "open"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn test_conn(
            &mut self,
            request: impl tonic::IntoRequest<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::TestConnResp>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.ConfigService/TestConn",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.ConfigService", "TestConn"));
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated client implementations.
pub mod database_service_client {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    #[derive(Debug, Clone)]
    pub struct DatabaseServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl DatabaseServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> DatabaseServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> DatabaseServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + Send + Sync,
        {
            DatabaseServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        pub async fn list_table(
            &mut self,
            request: impl tonic::IntoRequest<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::ListTableResp>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ListTable",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "ListTable"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn redis_list_table(
            &mut self,
            request: impl tonic::IntoRequest<super::OnlyUuidReq>,
        ) -> std::result::Result<
            tonic::Response<super::RedisListTableResp>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/RedisListTable",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "RedisListTable"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn redis_match_keys(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndPattern>,
        ) -> std::result::Result<
            tonic::Response<super::RedisMatchKeysResp>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/RedisMatchKeys",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "RedisMatchKeys"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn show_columns(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndName>,
        ) -> std::result::Result<
            tonic::Response<super::ShowColumnsResp>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ShowColumns",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "ShowColumns"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn show_talbe(
            &mut self,
            request: impl tonic::IntoRequest<super::TableViewListReq>,
        ) -> std::result::Result<tonic::Response<super::TableListResp>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ShowTalbe",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "ShowTalbe"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn show_index(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<super::ShowIndexResp>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ShowIndex",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "ShowIndex"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn insert(
            &mut self,
            request: impl tonic::IntoRequest<super::InsertReq>,
        ) -> std::result::Result<tonic::Response<super::SqlExecuteRet>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/Insert",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "Insert"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn update(
            &mut self,
            request: impl tonic::IntoRequest<super::UpdateReq>,
        ) -> std::result::Result<tonic::Response<super::SqlExecuteRet>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/Update",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "Update"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn truncate_table(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/TruncateTable",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "TruncateTable"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn delete(
            &mut self,
            request: impl tonic::IntoRequest<super::DeleteReq>,
        ) -> std::result::Result<tonic::Response<super::SqlExecuteRet>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/Delete",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "Delete"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn list_view(
            &mut self,
            request: impl tonic::IntoRequest<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::NamedStaffVec>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ListView",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "ListView"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn drop_view(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/DropView",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "DropView"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn drop_table(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/DropTable",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "DropTable"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn show_view(
            &mut self,
            request: impl tonic::IntoRequest<super::TableViewListReq>,
        ) -> std::result::Result<tonic::Response<super::TableListResp>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ShowView",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "ShowView"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn open_schema(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/OpenSchema",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "OpenSchema"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn open_db(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/OpenDB",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "OpenDB"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn opened_kind(
            &mut self,
            request: impl tonic::IntoRequest<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::OpendKindResp>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/OpenedKind",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "OpenedKind"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn query_sql(
            &mut self,
            request: impl tonic::IntoRequest<super::QuerySqlReq>,
        ) -> std::result::Result<tonic::Response<super::QuerySqlResp>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/QuerySql",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "QuerySql"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn query_sql_open_connection(
            &mut self,
            request: impl tonic::IntoRequest<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::StringResp>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/QuerySqlOpenConnection",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("app.DatabaseService", "QuerySqlOpenConnection"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn query_sql_close_connection(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndQueryId>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/QuerySqlCloseConnection",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("app.DatabaseService", "QuerySqlCloseConnection"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn query_sql_close_re_connection(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndQueryId>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/QuerySqlCloseReConnection",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("app.DatabaseService", "QuerySqlCloseReConnection"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn save_user_settings(
            &mut self,
            request: impl tonic::IntoRequest<super::UserSettings>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/SaveUserSettings",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "SaveUserSettings"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_user_settings(
            &mut self,
            request: impl tonic::IntoRequest<()>,
        ) -> std::result::Result<tonic::Response<super::UserSettings>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/GetUserSettings",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "GetUserSettings"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn export_table(
            &mut self,
            request: impl tonic::IntoRequest<super::ExportTableReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ExportTable",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "ExportTable"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn format_sql(
            &mut self,
            request: impl tonic::IntoRequest<super::FormatSqlReq>,
        ) -> std::result::Result<tonic::Response<super::StringResp>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/FormatSql",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "FormatSql"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn list_trigger(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<super::NamedStaffVec>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ListTrigger",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "ListTrigger"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn show_trigger(
            &mut self,
            request: impl tonic::IntoRequest<super::CommonriggerReq>,
        ) -> std::result::Result<tonic::Response<super::TriggerDetail>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ShowTrigger",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "ShowTrigger"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn enable_trigger(
            &mut self,
            request: impl tonic::IntoRequest<super::CommonriggerReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/EnableTrigger",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "EnableTrigger"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn disable_trigger(
            &mut self,
            request: impl tonic::IntoRequest<super::CommonriggerReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/DisableTrigger",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "DisableTrigger"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn rename_table(
            &mut self,
            request: impl tonic::IntoRequest<super::RenameTableReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/RenameTable",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "RenameTable"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn duplicate_table(
            &mut self,
            request: impl tonic::IntoRequest<super::DuplicateTableReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/DuplicateTable",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "DuplicateTable"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn export_query_result_to_file(
            &mut self,
            request: impl tonic::IntoRequest<super::ExportQueryResultReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ExportQueryResultToFile",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(
                    GrpcMethod::new("app.DatabaseService", "ExportQueryResultToFile"),
                );
            self.inner.unary(req, path, codec).await
        }
        pub async fn list_udf(
            &mut self,
            request: impl tonic::IntoRequest<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::NamedStaffVec>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ListUdf",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "ListUdf"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn show_udf(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<super::UdfDetail>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ShowUdf",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "ShowUdf"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn drop_udf(
            &mut self,
            request: impl tonic::IntoRequest<super::DropUdfReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/DropUdf",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "DropUdf"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn list_procedure(
            &mut self,
            request: impl tonic::IntoRequest<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::NamedStaffVec>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ListProcedure",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "ListProcedure"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn show_procedure(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndName>,
        ) -> std::result::Result<
            tonic::Response<super::ProcedureDetail>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/ShowProcedure",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "ShowProcedure"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn drop_procedure(
            &mut self,
            request: impl tonic::IntoRequest<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/DropProcedure",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "DropProcedure"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn do_auto_completion(
            &mut self,
            request: impl tonic::IntoRequest<super::AutoCompletionReq>,
        ) -> std::result::Result<
            tonic::Response<super::AutoCompletionResp>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Unknown,
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/app.DatabaseService/DoAutoCompletion",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("app.DatabaseService", "DoAutoCompletion"));
            self.inner.unary(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod my_service_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with MyServiceServer.
    #[async_trait]
    pub trait MyService: Send + Sync + 'static {
        async fn say_hello(
            &self,
            request: tonic::Request<super::HelloRequest>,
        ) -> std::result::Result<tonic::Response<super::HelloResponse>, tonic::Status>;
        async fn return_err(
            &self,
            request: tonic::Request<super::HelloRequest>,
        ) -> std::result::Result<tonic::Response<super::HelloResponse>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct MyServiceServer<T: MyService> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: MyService> MyServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for MyServiceServer<T>
    where
        T: MyService,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/app.MyService/SayHello" => {
                    #[allow(non_camel_case_types)]
                    struct SayHelloSvc<T: MyService>(pub Arc<T>);
                    impl<T: MyService> tonic::server::UnaryService<super::HelloRequest>
                    for SayHelloSvc<T> {
                        type Response = super::HelloResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::HelloRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).say_hello(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = SayHelloSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.MyService/ReturnErr" => {
                    #[allow(non_camel_case_types)]
                    struct ReturnErrSvc<T: MyService>(pub Arc<T>);
                    impl<T: MyService> tonic::server::UnaryService<super::HelloRequest>
                    for ReturnErrSvc<T> {
                        type Response = super::HelloResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::HelloRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).return_err(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ReturnErrSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: MyService> Clone for MyServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    impl<T: MyService> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: MyService> tonic::server::NamedService for MyServiceServer<T> {
        const NAME: &'static str = "app.MyService";
    }
}
/// Generated server implementations.
pub mod config_service_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with ConfigServiceServer.
    #[async_trait]
    pub trait ConfigService: Send + Sync + 'static {
        async fn add(
            &self,
            request: tonic::Request<super::DbConnectionConfig>,
        ) -> std::result::Result<tonic::Response<super::AddConfigResp>, tonic::Status>;
        async fn update(
            &self,
            request: tonic::Request<super::UpdateConfig>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn del(
            &self,
            request: tonic::Request<super::DelConfigReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn list_all(
            &self,
            request: tonic::Request<()>,
        ) -> std::result::Result<
            tonic::Response<super::ListAllConfigResp>,
            tonic::Status,
        >;
        async fn open(
            &self,
            request: tonic::Request<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn test_conn(
            &self,
            request: tonic::Request<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::TestConnResp>, tonic::Status>;
    }
    #[derive(Debug)]
    pub struct ConfigServiceServer<T: ConfigService> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: ConfigService> ConfigServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for ConfigServiceServer<T>
    where
        T: ConfigService,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/app.ConfigService/Add" => {
                    #[allow(non_camel_case_types)]
                    struct AddSvc<T: ConfigService>(pub Arc<T>);
                    impl<
                        T: ConfigService,
                    > tonic::server::UnaryService<super::DbConnectionConfig>
                    for AddSvc<T> {
                        type Response = super::AddConfigResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::DbConnectionConfig>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).add(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = AddSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.ConfigService/Update" => {
                    #[allow(non_camel_case_types)]
                    struct UpdateSvc<T: ConfigService>(pub Arc<T>);
                    impl<
                        T: ConfigService,
                    > tonic::server::UnaryService<super::UpdateConfig> for UpdateSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UpdateConfig>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).update(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = UpdateSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.ConfigService/Del" => {
                    #[allow(non_camel_case_types)]
                    struct DelSvc<T: ConfigService>(pub Arc<T>);
                    impl<
                        T: ConfigService,
                    > tonic::server::UnaryService<super::DelConfigReq> for DelSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::DelConfigReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).del(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DelSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.ConfigService/ListAll" => {
                    #[allow(non_camel_case_types)]
                    struct ListAllSvc<T: ConfigService>(pub Arc<T>);
                    impl<T: ConfigService> tonic::server::UnaryService<()>
                    for ListAllSvc<T> {
                        type Response = super::ListAllConfigResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(&mut self, request: tonic::Request<()>) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).list_all(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ListAllSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.ConfigService/open" => {
                    #[allow(non_camel_case_types)]
                    struct openSvc<T: ConfigService>(pub Arc<T>);
                    impl<
                        T: ConfigService,
                    > tonic::server::UnaryService<super::OnlyUuidReq> for openSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::OnlyUuidReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).open(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = openSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.ConfigService/TestConn" => {
                    #[allow(non_camel_case_types)]
                    struct TestConnSvc<T: ConfigService>(pub Arc<T>);
                    impl<
                        T: ConfigService,
                    > tonic::server::UnaryService<super::OnlyUuidReq>
                    for TestConnSvc<T> {
                        type Response = super::TestConnResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::OnlyUuidReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).test_conn(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = TestConnSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: ConfigService> Clone for ConfigServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    impl<T: ConfigService> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: ConfigService> tonic::server::NamedService for ConfigServiceServer<T> {
        const NAME: &'static str = "app.ConfigService";
    }
}
/// Generated server implementations.
pub mod database_service_server {
    #![allow(unused_variables, dead_code, missing_docs, clippy::let_unit_value)]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with DatabaseServiceServer.
    #[async_trait]
    pub trait DatabaseService: Send + Sync + 'static {
        async fn list_table(
            &self,
            request: tonic::Request<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::ListTableResp>, tonic::Status>;
        async fn redis_list_table(
            &self,
            request: tonic::Request<super::OnlyUuidReq>,
        ) -> std::result::Result<
            tonic::Response<super::RedisListTableResp>,
            tonic::Status,
        >;
        async fn redis_match_keys(
            &self,
            request: tonic::Request<super::UuidAndPattern>,
        ) -> std::result::Result<
            tonic::Response<super::RedisMatchKeysResp>,
            tonic::Status,
        >;
        async fn show_columns(
            &self,
            request: tonic::Request<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<super::ShowColumnsResp>, tonic::Status>;
        async fn show_talbe(
            &self,
            request: tonic::Request<super::TableViewListReq>,
        ) -> std::result::Result<tonic::Response<super::TableListResp>, tonic::Status>;
        async fn show_index(
            &self,
            request: tonic::Request<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<super::ShowIndexResp>, tonic::Status>;
        async fn insert(
            &self,
            request: tonic::Request<super::InsertReq>,
        ) -> std::result::Result<tonic::Response<super::SqlExecuteRet>, tonic::Status>;
        async fn update(
            &self,
            request: tonic::Request<super::UpdateReq>,
        ) -> std::result::Result<tonic::Response<super::SqlExecuteRet>, tonic::Status>;
        async fn truncate_table(
            &self,
            request: tonic::Request<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn delete(
            &self,
            request: tonic::Request<super::DeleteReq>,
        ) -> std::result::Result<tonic::Response<super::SqlExecuteRet>, tonic::Status>;
        async fn list_view(
            &self,
            request: tonic::Request<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::NamedStaffVec>, tonic::Status>;
        async fn drop_view(
            &self,
            request: tonic::Request<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn drop_table(
            &self,
            request: tonic::Request<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn show_view(
            &self,
            request: tonic::Request<super::TableViewListReq>,
        ) -> std::result::Result<tonic::Response<super::TableListResp>, tonic::Status>;
        async fn open_schema(
            &self,
            request: tonic::Request<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn open_db(
            &self,
            request: tonic::Request<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn opened_kind(
            &self,
            request: tonic::Request<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::OpendKindResp>, tonic::Status>;
        async fn query_sql(
            &self,
            request: tonic::Request<super::QuerySqlReq>,
        ) -> std::result::Result<tonic::Response<super::QuerySqlResp>, tonic::Status>;
        async fn query_sql_open_connection(
            &self,
            request: tonic::Request<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::StringResp>, tonic::Status>;
        async fn query_sql_close_connection(
            &self,
            request: tonic::Request<super::UuidAndQueryId>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn query_sql_close_re_connection(
            &self,
            request: tonic::Request<super::UuidAndQueryId>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn save_user_settings(
            &self,
            request: tonic::Request<super::UserSettings>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn get_user_settings(
            &self,
            request: tonic::Request<()>,
        ) -> std::result::Result<tonic::Response<super::UserSettings>, tonic::Status>;
        async fn export_table(
            &self,
            request: tonic::Request<super::ExportTableReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn format_sql(
            &self,
            request: tonic::Request<super::FormatSqlReq>,
        ) -> std::result::Result<tonic::Response<super::StringResp>, tonic::Status>;
        async fn list_trigger(
            &self,
            request: tonic::Request<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<super::NamedStaffVec>, tonic::Status>;
        async fn show_trigger(
            &self,
            request: tonic::Request<super::CommonriggerReq>,
        ) -> std::result::Result<tonic::Response<super::TriggerDetail>, tonic::Status>;
        async fn enable_trigger(
            &self,
            request: tonic::Request<super::CommonriggerReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn disable_trigger(
            &self,
            request: tonic::Request<super::CommonriggerReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn rename_table(
            &self,
            request: tonic::Request<super::RenameTableReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn duplicate_table(
            &self,
            request: tonic::Request<super::DuplicateTableReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn export_query_result_to_file(
            &self,
            request: tonic::Request<super::ExportQueryResultReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn list_udf(
            &self,
            request: tonic::Request<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::NamedStaffVec>, tonic::Status>;
        async fn show_udf(
            &self,
            request: tonic::Request<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<super::UdfDetail>, tonic::Status>;
        async fn drop_udf(
            &self,
            request: tonic::Request<super::DropUdfReq>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn list_procedure(
            &self,
            request: tonic::Request<super::OnlyUuidReq>,
        ) -> std::result::Result<tonic::Response<super::NamedStaffVec>, tonic::Status>;
        async fn show_procedure(
            &self,
            request: tonic::Request<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<super::ProcedureDetail>, tonic::Status>;
        async fn drop_procedure(
            &self,
            request: tonic::Request<super::UuidAndName>,
        ) -> std::result::Result<tonic::Response<()>, tonic::Status>;
        async fn do_auto_completion(
            &self,
            request: tonic::Request<super::AutoCompletionReq>,
        ) -> std::result::Result<
            tonic::Response<super::AutoCompletionResp>,
            tonic::Status,
        >;
    }
    #[derive(Debug)]
    pub struct DatabaseServiceServer<T: DatabaseService> {
        inner: _Inner<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    struct _Inner<T>(Arc<T>);
    impl<T: DatabaseService> DatabaseServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            let inner = _Inner(inner);
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for DatabaseServiceServer<T>
    where
        T: DatabaseService,
        B: Body + Send + 'static,
        B::Error: Into<StdError> + Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            let inner = self.inner.clone();
            match req.uri().path() {
                "/app.DatabaseService/ListTable" => {
                    #[allow(non_camel_case_types)]
                    struct ListTableSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::OnlyUuidReq>
                    for ListTableSvc<T> {
                        type Response = super::ListTableResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::OnlyUuidReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).list_table(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ListTableSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/RedisListTable" => {
                    #[allow(non_camel_case_types)]
                    struct RedisListTableSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::OnlyUuidReq>
                    for RedisListTableSvc<T> {
                        type Response = super::RedisListTableResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::OnlyUuidReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).redis_list_table(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = RedisListTableSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/RedisMatchKeys" => {
                    #[allow(non_camel_case_types)]
                    struct RedisMatchKeysSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndPattern>
                    for RedisMatchKeysSvc<T> {
                        type Response = super::RedisMatchKeysResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndPattern>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).redis_match_keys(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = RedisMatchKeysSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/ShowColumns" => {
                    #[allow(non_camel_case_types)]
                    struct ShowColumnsSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndName>
                    for ShowColumnsSvc<T> {
                        type Response = super::ShowColumnsResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndName>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).show_columns(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ShowColumnsSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/ShowTalbe" => {
                    #[allow(non_camel_case_types)]
                    struct ShowTalbeSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::TableViewListReq>
                    for ShowTalbeSvc<T> {
                        type Response = super::TableListResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::TableViewListReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).show_talbe(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ShowTalbeSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/ShowIndex" => {
                    #[allow(non_camel_case_types)]
                    struct ShowIndexSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndName>
                    for ShowIndexSvc<T> {
                        type Response = super::ShowIndexResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndName>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).show_index(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ShowIndexSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/Insert" => {
                    #[allow(non_camel_case_types)]
                    struct InsertSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::InsertReq> for InsertSvc<T> {
                        type Response = super::SqlExecuteRet;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::InsertReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).insert(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = InsertSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/Update" => {
                    #[allow(non_camel_case_types)]
                    struct UpdateSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UpdateReq> for UpdateSvc<T> {
                        type Response = super::SqlExecuteRet;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UpdateReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).update(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = UpdateSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/TruncateTable" => {
                    #[allow(non_camel_case_types)]
                    struct TruncateTableSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndName>
                    for TruncateTableSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndName>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).truncate_table(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = TruncateTableSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/Delete" => {
                    #[allow(non_camel_case_types)]
                    struct DeleteSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::DeleteReq> for DeleteSvc<T> {
                        type Response = super::SqlExecuteRet;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::DeleteReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).delete(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DeleteSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/ListView" => {
                    #[allow(non_camel_case_types)]
                    struct ListViewSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::OnlyUuidReq>
                    for ListViewSvc<T> {
                        type Response = super::NamedStaffVec;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::OnlyUuidReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).list_view(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ListViewSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/DropView" => {
                    #[allow(non_camel_case_types)]
                    struct DropViewSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndName>
                    for DropViewSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndName>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).drop_view(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DropViewSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/DropTable" => {
                    #[allow(non_camel_case_types)]
                    struct DropTableSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndName>
                    for DropTableSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndName>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).drop_table(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DropTableSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/ShowView" => {
                    #[allow(non_camel_case_types)]
                    struct ShowViewSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::TableViewListReq>
                    for ShowViewSvc<T> {
                        type Response = super::TableListResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::TableViewListReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).show_view(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ShowViewSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/OpenSchema" => {
                    #[allow(non_camel_case_types)]
                    struct OpenSchemaSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndName>
                    for OpenSchemaSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndName>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).open_schema(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = OpenSchemaSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/OpenDB" => {
                    #[allow(non_camel_case_types)]
                    struct OpenDBSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndName> for OpenDBSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndName>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).open_db(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = OpenDBSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/OpenedKind" => {
                    #[allow(non_camel_case_types)]
                    struct OpenedKindSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::OnlyUuidReq>
                    for OpenedKindSvc<T> {
                        type Response = super::OpendKindResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::OnlyUuidReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).opened_kind(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = OpenedKindSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/QuerySql" => {
                    #[allow(non_camel_case_types)]
                    struct QuerySqlSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::QuerySqlReq>
                    for QuerySqlSvc<T> {
                        type Response = super::QuerySqlResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::QuerySqlReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).query_sql(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = QuerySqlSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/QuerySqlOpenConnection" => {
                    #[allow(non_camel_case_types)]
                    struct QuerySqlOpenConnectionSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::OnlyUuidReq>
                    for QuerySqlOpenConnectionSvc<T> {
                        type Response = super::StringResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::OnlyUuidReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).query_sql_open_connection(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = QuerySqlOpenConnectionSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/QuerySqlCloseConnection" => {
                    #[allow(non_camel_case_types)]
                    struct QuerySqlCloseConnectionSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndQueryId>
                    for QuerySqlCloseConnectionSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndQueryId>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).query_sql_close_connection(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = QuerySqlCloseConnectionSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/QuerySqlCloseReConnection" => {
                    #[allow(non_camel_case_types)]
                    struct QuerySqlCloseReConnectionSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndQueryId>
                    for QuerySqlCloseReConnectionSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndQueryId>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).query_sql_close_re_connection(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = QuerySqlCloseReConnectionSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/SaveUserSettings" => {
                    #[allow(non_camel_case_types)]
                    struct SaveUserSettingsSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UserSettings>
                    for SaveUserSettingsSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UserSettings>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).save_user_settings(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = SaveUserSettingsSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/GetUserSettings" => {
                    #[allow(non_camel_case_types)]
                    struct GetUserSettingsSvc<T: DatabaseService>(pub Arc<T>);
                    impl<T: DatabaseService> tonic::server::UnaryService<()>
                    for GetUserSettingsSvc<T> {
                        type Response = super::UserSettings;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(&mut self, request: tonic::Request<()>) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).get_user_settings(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = GetUserSettingsSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/ExportTable" => {
                    #[allow(non_camel_case_types)]
                    struct ExportTableSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::ExportTableReq>
                    for ExportTableSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ExportTableReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).export_table(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ExportTableSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/FormatSql" => {
                    #[allow(non_camel_case_types)]
                    struct FormatSqlSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::FormatSqlReq>
                    for FormatSqlSvc<T> {
                        type Response = super::StringResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::FormatSqlReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).format_sql(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = FormatSqlSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/ListTrigger" => {
                    #[allow(non_camel_case_types)]
                    struct ListTriggerSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndName>
                    for ListTriggerSvc<T> {
                        type Response = super::NamedStaffVec;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndName>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).list_trigger(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ListTriggerSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/ShowTrigger" => {
                    #[allow(non_camel_case_types)]
                    struct ShowTriggerSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::CommonriggerReq>
                    for ShowTriggerSvc<T> {
                        type Response = super::TriggerDetail;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::CommonriggerReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).show_trigger(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ShowTriggerSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/EnableTrigger" => {
                    #[allow(non_camel_case_types)]
                    struct EnableTriggerSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::CommonriggerReq>
                    for EnableTriggerSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::CommonriggerReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).enable_trigger(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = EnableTriggerSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/DisableTrigger" => {
                    #[allow(non_camel_case_types)]
                    struct DisableTriggerSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::CommonriggerReq>
                    for DisableTriggerSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::CommonriggerReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).disable_trigger(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DisableTriggerSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/RenameTable" => {
                    #[allow(non_camel_case_types)]
                    struct RenameTableSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::RenameTableReq>
                    for RenameTableSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::RenameTableReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).rename_table(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = RenameTableSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/DuplicateTable" => {
                    #[allow(non_camel_case_types)]
                    struct DuplicateTableSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::DuplicateTableReq>
                    for DuplicateTableSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::DuplicateTableReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).duplicate_table(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DuplicateTableSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/ExportQueryResultToFile" => {
                    #[allow(non_camel_case_types)]
                    struct ExportQueryResultToFileSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::ExportQueryResultReq>
                    for ExportQueryResultToFileSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ExportQueryResultReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).export_query_result_to_file(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ExportQueryResultToFileSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/ListUdf" => {
                    #[allow(non_camel_case_types)]
                    struct ListUdfSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::OnlyUuidReq> for ListUdfSvc<T> {
                        type Response = super::NamedStaffVec;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::OnlyUuidReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).list_udf(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ListUdfSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/ShowUdf" => {
                    #[allow(non_camel_case_types)]
                    struct ShowUdfSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndName> for ShowUdfSvc<T> {
                        type Response = super::UdfDetail;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndName>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).show_udf(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ShowUdfSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/DropUdf" => {
                    #[allow(non_camel_case_types)]
                    struct DropUdfSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::DropUdfReq> for DropUdfSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::DropUdfReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move { (*inner).drop_udf(request).await };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DropUdfSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/ListProcedure" => {
                    #[allow(non_camel_case_types)]
                    struct ListProcedureSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::OnlyUuidReq>
                    for ListProcedureSvc<T> {
                        type Response = super::NamedStaffVec;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::OnlyUuidReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).list_procedure(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ListProcedureSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/ShowProcedure" => {
                    #[allow(non_camel_case_types)]
                    struct ShowProcedureSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndName>
                    for ShowProcedureSvc<T> {
                        type Response = super::ProcedureDetail;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndName>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).show_procedure(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = ShowProcedureSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/DropProcedure" => {
                    #[allow(non_camel_case_types)]
                    struct DropProcedureSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::UuidAndName>
                    for DropProcedureSvc<T> {
                        type Response = ();
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::UuidAndName>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).drop_procedure(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DropProcedureSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/app.DatabaseService/DoAutoCompletion" => {
                    #[allow(non_camel_case_types)]
                    struct DoAutoCompletionSvc<T: DatabaseService>(pub Arc<T>);
                    impl<
                        T: DatabaseService,
                    > tonic::server::UnaryService<super::AutoCompletionReq>
                    for DoAutoCompletionSvc<T> {
                        type Response = super::AutoCompletionResp;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::AutoCompletionReq>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                (*inner).do_auto_completion(request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let inner = inner.0;
                        let method = DoAutoCompletionSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        Ok(
                            http::Response::builder()
                                .status(200)
                                .header("grpc-status", "12")
                                .header("content-type", "application/grpc")
                                .body(empty_body())
                                .unwrap(),
                        )
                    })
                }
            }
        }
    }
    impl<T: DatabaseService> Clone for DatabaseServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    impl<T: DatabaseService> Clone for _Inner<T> {
        fn clone(&self) -> Self {
            Self(Arc::clone(&self.0))
        }
    }
    impl<T: std::fmt::Debug> std::fmt::Debug for _Inner<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{:?}", self.0)
        }
    }
    impl<T: DatabaseService> tonic::server::NamedService for DatabaseServiceServer<T> {
        const NAME: &'static str = "app.DatabaseService";
    }
}
