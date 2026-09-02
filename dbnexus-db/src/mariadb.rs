#[cfg(test)]
use crate::datasource_config::MysqlDBConfig;
use crate::datasource_config::SslFilesWithStrModel;
#[cfg(test)]
use crate::db::BackendKindMysql;
use crate::db::SequenceDetail;
use crate::mysql::MysqlDB;
use sqlx::types::BigDecimal;
use sqlx::{AssertSqlSafe, Row};
use std::ops::DerefMut;

// AI generate this cool.
/// MariaDB 11.x 全部内置函数名称
pub const MARIADB_FUNCTIONS: &[&str] = &[
    // 数值
    "ABS",
    "ACOS",
    "ADDDATE",
    "ADDTIME",
    "AES_DECRYPT",
    "AES_ENCRYPT",
    "ASIN",
    "ATAN",
    "ATAN2",
    "AVG",
    "BENCHMARK",
    "BIN",
    "BIN_TO_UUID",
    "BIT_AND",
    "BIT_COUNT",
    "BIT_LENGTH",
    "BIT_OR",
    "BIT_XOR",
    "CEIL",
    "CEILING",
    "CONV",
    "COS",
    "COT",
    "CRC32",
    "DEGREES",
    "DIV",
    "EXP",
    "FLOOR",
    "FORMAT",
    "FOUND_ROWS",
    "FROM_BASE64",
    "HEX",
    "LN",
    "LOG",
    "LOG10",
    "LOG2",
    "MOD",
    "OCT",
    "OCTET_LENGTH",
    "PI",
    "POW",
    "POWER",
    "RADIANS",
    "RAND",
    "ROUND",
    "SIGN",
    "SIN",
    "SQRT",
    "STD",
    "STDDEV",
    "STDDEV_POP",
    "STDDEV_SAMP",
    "TAN",
    "TRUNCATE",
    "UNCOMPRESS",
    "UNCOMPRESSED_LENGTH",
    "UNHEX",
    "VAR_POP",
    "VAR_SAMP",
    "VARIANCE",
    // 字符串
    "ASCII",
    "CHAR",
    "CHARACTER_LENGTH",
    "CHAR_LENGTH",
    "CHARSET",
    "COERCIBILITY",
    "COLLATION",
    "COMPRESS",
    "CONCAT",
    "CONCAT_WS",
    "ELT",
    "FIELD",
    "FIND_IN_SET",
    "FORMAT",
    "HEX",
    "INSERT",
    "INSTR",
    "LCASE",
    "LEFT",
    "LENGTH",
    "LOCATE",
    "LOWER",
    "LPAD",
    "LTRIM",
    "MAKE_SET",
    "MID",
    "OCTET_LENGTH",
    "ORD",
    "POSITION",
    "QUOTE",
    "REPEAT",
    "REPLACE",
    "REVERSE",
    "RIGHT",
    "RPAD",
    "RTRIM",
    "SOUNDEX",
    "SPACE",
    "STRCMP",
    "SUBSTR",
    "SUBSTRING",
    "SUBSTRING_INDEX",
    "TRIM",
    "UCASE",
    "UNHEX",
    "UPPER",
    "UUID_SHORT",
    // 日期时间
    "ADDDATE",
    "ADDTIME",
    "CONVERT_TZ",
    "CURDATE",
    "CURRENT_DATE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "CURTIME",
    "DATE",
    "DATEDIFF",
    "DATE_ADD",
    "DATE_FORMAT",
    "DATE_SUB",
    "DAY",
    "DAYNAME",
    "DAYOFMONTH",
    "DAYOFWEEK",
    "DAYOFYEAR",
    "EXTRACT",
    "FROM_DAYS",
    "FROM_UNIXTIME",
    "GET_FORMAT",
    "HOUR",
    "LAST_DAY",
    "LOCALTIME",
    "LOCALTIMESTAMP",
    "MAKEDATE",
    "MAKETIME",
    "MICROSECOND",
    "MINUTE",
    "MONTH",
    "MONTHNAME",
    "NOW",
    "PERIOD_ADD",
    "PERIOD_DIFF",
    "QUARTER",
    "SEC_TO_TIME",
    "SECOND",
    "STR_TO_DATE",
    "SUBDATE",
    "SUBTIME",
    "SYSDATE",
    "TIME",
    "TIME_FORMAT",
    "TIME_TO_SEC",
    "TIMEDIFF",
    "TIMESTAMP",
    "TIMESTAMPADD",
    "TIMESTAMPDIFF",
    "TO_DAYS",
    "TO_SECONDS",
    "UNIX_TIMESTAMP",
    "UTC_DATE",
    "UTC_TIME",
    "UTC_TIMESTAMP",
    "WEEK",
    "WEEKDAY",
    "WEEKOFYEAR",
    "YEAR",
    "YEARWEEK",
    // JSON
    "JSON_ARRAY",
    "JSON_ARRAY_APPEND",
    "JSON_ARRAY_INSERT",
    "JSON_ARRAYAGG",
    "JSON_CONTAINS",
    "JSON_CONTAINS_PATH",
    "JSON_DEPTH",
    "JSON_EXTRACT",
    "JSON_INSERT",
    "JSON_KEYS",
    "JSON_LENGTH",
    "JSON_MERGE",
    "JSON_MERGE_PATCH",
    "JSON_MERGE_PRESERVE",
    "JSON_OBJECT",
    "JSON_OBJECTAGG",
    "JSON_OVERLAPS",
    "JSON_PRETTY",
    "JSON_QUOTE",
    "JSON_REMOVE",
    "JSON_REPLACE",
    "JSON_SCHEMA_VALID",
    "JSON_SCHEMA_VALIDATION_REPORT",
    "JSON_SEARCH",
    "JSON_SET",
    "JSON_STORAGE_FREE",
    "JSON_STORAGE_SIZE",
    "JSON_TABLE",
    "JSON_TYPE",
    "JSON_UNQUOTE",
    "JSON_VALID",
    "JSON_VALUE",
    // 聚合/窗口
    "ANY_VALUE",
    "AVG",
    "BIT_AND",
    "BIT_OR",
    "BIT_XOR",
    "COUNT",
    "COUNT(DISTINCT)",
    "CUME_DIST",
    "DENSE_RANK",
    "FIRST_VALUE",
    "GROUP_CONCAT",
    "LAG",
    "LAST_VALUE",
    "LEAD",
    "MAX",
    "MIN",
    "NTH_VALUE",
    "NTILE",
    "PERCENT_RANK",
    "RANK",
    "ROW_NUMBER",
    "STD",
    "STDDEV",
    "STDDEV_POP",
    "STDDEV_SAMP",
    "SUM",
    "VAR_POP",
    "VAR_SAMP",
    "VARIANCE",
    // 加密/哈希
    "AES_DECRYPT",
    "AES_ENCRYPT",
    "COMPRESS",
    "DECODE",
    "ENCODE",
    "MD5",
    "PASSWORD",
    "RANDOM_BYTES",
    "SHA1",
    "SHA2",
    "UNCOMPRESS",
    "VALIDATE_PASSWORD_STRENGTH",
    // 锁
    "GET_LOCK",
    "IS_FREE_LOCK",
    "IS_USED_LOCK",
    "RELEASE_ALL_LOCKS",
    "RELEASE_LOCK",
    // 系统信息
    "BENCHMARK",
    "BINLOG_GTID_POS",
    "COERCIBILITY",
    "COLLATION",
    "CONNECTION_ID",
    "CURRENT_ROLE",
    "CURRENT_USER",
    "DATABASE",
    "FOUND_ROWS",
    "ICU_VERSION",
    "LAST_INSERT_ID",
    "ROW_COUNT",
    "SCHEMA",
    "SESSION_USER",
    "SYSTEM_USER",
    "USER",
    "VERSION",
];

impl MysqlDB {
    pub(crate) async fn mariadb_list_sequences(
        &self,
        schema: &str,
    ) -> anyhow::Result<Vec<SequenceDetail>> {
        let sql = format!(
            "
            select SEQUENCE_NAME , DATA_TYPE,
            START_VALUE, MINIMUM_VALUE ,
            MAXIMUM_VALUE  , INCREMENT
            from information_schema.SEQUENCES where SEQUENCE_SCHEMA = '{}'
            ",
            schema
        );
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            let data_type: String = row.try_get(1)?;
            let start_value: BigDecimal = row.try_get(2)?;
            let min_value: BigDecimal = row.try_get(3)?;
            let max_value: BigDecimal = row.try_get(4)?;
            let increment: i64 = row.try_get(5)?;
            let create_sql = {
                let sql = format!("SHOW CREATE SEQUENCE {name}");
                let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                    .fetch_one(conn.deref_mut())
                    .await?;
                let create_sql: String = row.try_get(1)?;
                create_sql
            };
            ret.push(SequenceDetail {
                name,
                data_type,
                start_value: start_value.to_string(),
                min_value: min_value.to_string(),
                max_value: max_value.to_string(),
                increment_by: increment.to_string(),
                last_value: None, // TODO get last value
                sql: create_sql,
            });
        }
        anyhow::Ok(ret)
    }
}

#[cfg(test)]
pub(crate) fn make_config() -> MysqlDBConfig {
    MysqlDBConfig {
        conn_timeout: Some(5000.into()),
        host: crate::TEST_HOST.to_string(),
        username: "root".to_string(),
        password: Some("root".to_string()),
        port: 3307,
        db: Some("test".to_string()),
        read_only: false,
        ssl: SslFilesWithStrModel::mysql_default(),
        charset: Default::default(),
    }
}

impl SslFilesWithStrModel {
    #[cfg(test)]
    pub(crate) fn mysql_default() -> Self {
        Self {
            ssl_mode: "disabled".to_owned(),
            ssl_files: Default::default(),
        }
    }
}

#[cfg(test)]
pub(crate) async fn make_test_object_mariadb(config: MysqlDBConfig) -> MysqlDB {
    use crate::mysql::MysqlDB;
    let db = MysqlDB::new(config, BackendKindMysql::Mariadb)
        .await
        .unwrap();
    db
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::db::{GetTestExportTableKinds, QuerySql, SchemaTrait, ShowTableParameter};
    use crate::language_server::LangServerCommon;
    use crate::mariadb::make_config;
    use crate::mysql::test_fns::{self};

    /*
        need set /etc/mysql/mariadb.conf.d/50-server.cnf
        /etc/mysql/mariadb.conf.d/50-server.cnf
    */
    #[tokio::test]
    async fn test_prepare() {
        let db = make_test_object_mariadb(make_config()).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            PREPARE stmtxxx FROM 'SELECT 1';
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.completion_id.set(query_id.to_owned());
        let ts = db.list_execute(None).await.unwrap();
        assert!(ts.iter().any(|x| x.name.eq("stmtxxx")), "stmtxxx{:?}", ts);
    }

    #[tokio::test]
    async fn test_sequence() {
        let db = make_test_object_mariadb(make_config()).await;
        let name = "test_seq";
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_sequence(name, false).await;

        db.execute_sql(
            query_id,
            r#"
            CREATE SEQUENCE test_seq;
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let sequences = SchemaTrait::list_sequence(&db).await.unwrap();
        for seq in sequences.iter() {
            if seq.name.eq(name) {
                println!("{:?}", seq)
            }
        }
        let name2 = format!("{name}2");
        db.rename_sequence(name, &name2).await.unwrap();
        db.rename_sequence(&name2, name).await.unwrap();
    }

    #[tokio::test]

    async fn test_inet() {
        let db = make_test_object_mariadb(make_config()).await;

        let table_name = "test_inet";

        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();

        // 1. 使用 MariaDB 原生的 INET4 和 INET6 类型
        db.execute_sql(
            query_id,
            format!(
                r#"
            CREATE TABLE {table_name} (
                id INT AUTO_INCREMENT PRIMARY KEY,
                ipv4_col INET4,
                ipv6_col INET6
            );
            "#
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        // 2. 准备 Rust 标准库的 Ipv4Addr 和 Ipv6Addr 测试数据
        use std::net::{Ipv4Addr, Ipv6Addr};

        let ip4: Ipv4Addr = "192.168.1.1".parse().unwrap();
        let ip6: Ipv6Addr = "2001:db8::1".parse().unwrap();

        // MariaDB 在接收 INET 字段时支持直接传入文本表示
        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert("ipv4_col".to_string(), Some(ip4.to_string()));
        values.insert("ipv6_col".to_string(), Some(ip6.to_string()));

        // 3. 执行插入
        db.table_insert(table_name, &values).await.unwrap();

        // 4. 查询并打印结果（MariaDB 会自动将内部的二进制转换回标准字符串返回给客户端）
        let cols = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("{:?}", cols);

        GetTestExportTableKinds::test_export(&db, table_name).await;
        db.test_table_export_and_import(table_name).await;
    }

    #[tokio::test]
    async fn test_duplicate() {
        test_fns::test_duplicate(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_transaction() {
        test_fns::test_transaction(make_test_object_mariadb(make_config()).await).await;
    }

    #[tokio::test]
    async fn test_server_metas() {
        test_fns::test_server_metas(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_procedure() {
        test_fns::test_procedure(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_index() {
        test_fns::test_index(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_check_conn() {
        test_fns::test_check_conn(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_export() {
        test_fns::test_export(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_rename() {
        test_fns::test_rename(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_udf() {
        test_fns::test_udf(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types1() {
        test_fns::test_types1(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types2() {
        test_fns::test_types2(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types3() {
        test_fns::test_types3(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types4() {
        test_fns::test_types4(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_triggers() {
        test_fns::test_triggers(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_list_database() {
        test_fns::test_list_database(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_table_column() {
        test_fns::test_table_column(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_view() {
        test_fns::test_view(make_test_object_mariadb(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_event() {
        test_fns::test_event(make_test_object_mariadb(make_config()).await).await;
    }
}
