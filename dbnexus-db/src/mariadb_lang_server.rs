use std::ops::DerefMut;

use crate::db::DBTrait;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, DBPrepare, LangServerCommon,
    LanguageServer, LexerCompletionTrait, try_completion_on_syntax,
};
use crate::mariadb::MariaDB;
use crate::mysql::MysqlDB;
use crate::sqlparser_completion::TempTableColumn;
use sqlx::{AssertSqlSafe, Row};

#[async_trait::async_trait]
impl LanguageServer for MariaDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(sql, line, col, sqlparser::dialect::MySqlDialect {}, self)
            .await;
        match r {
            Ok(xs) => return Ok(xs),
            Err(err) => {
                log::error!("completion on ast failed,err:{}", err);
            }
        };
        if !use_lexer {
            log::warn!("disabled use lexer , we will return empty data.");
            return anyhow::Ok(vec![]);
        }
        return LexerCompletionTrait::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::MySqlDialect {},
        )
        .await;
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for MariaDB {
    async fn keywords(&self) -> anyhow::Result<Vec<CompletionItem>> {
        LexerCompletionTrait::get_key_words_fn(self).await
    }

    async fn list_table(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let mut result = AstCompletionHelper::list_table(self.db.as_ref(), None, schema).await?;
        let mut conn = self.db.get_conn().await?;
        let sql = format!(
            "select SEQUENCE_NAME  from information_schema.SEQUENCES where SEQUENCE_SCHEMA = '{}'",
            MysqlDB::not_allow_empty_schema(self.db.merge_schema(schema))
                .map_err(|e| { log::error!("err:{}", e) })
                .ok()
                .unwrap_or("")
        );
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            result.push(name);
        }
        anyhow::Ok(result)
    }

    async fn list_view(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_view(self.db.as_ref(), None, schema).await
    }

    async fn list_column(
        &self, _db: Option<&str>, schema: Option<&str>, name: &str, is_call: bool,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        AstCompletionHelper::list_column(self.db.as_ref(), None, schema, name, is_call).await
    }

    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        AstCompletionHelper::get_query_columns(self.db.as_ref(), sql).await
    }
    async fn list_function(
        &self, _db: Option<&str>, schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let mut ret =
            AstCompletionHelper::list_function(self.db.as_ref(), None, schema, false).await?;
        if include_build_in {
            ret.extend(MARIADB_FUNCTIONS.iter().map(|x| CompletionItem {
                item: (*x).into(),
                kind: CompletionKind::Function,
                desc: None,
            }));
        }
        anyhow::Ok(ret)
    }

    async fn list_procedure(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_procedure(self.db.as_ref(), None, schema).await
    }

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_index(self.db.as_ref(), schema).await
    }

    async fn list_variable(&self) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_variable(self.db.as_ref()).await
    }

    async fn list_partition(
        &self, _db: Option<&str>, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_partition(self.db.as_ref(), None, schema, table).await
    }

    async fn list_constraint(
        &self, _db: Option<&str>, schema: Option<&str>, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_constraint(self.db.as_ref(), None, schema, table_name).await
    }

    async fn list_table_trigger(
        &self, _db: Option<&str>, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_table_trigger(self.db.as_ref(), None, schema, table).await
    }
}

#[async_trait::async_trait]

impl LangServerCommon for MariaDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
    async fn list_schema(&self, _db: Option<&str>) -> anyhow::Result<Vec<String>> {
        DBTrait::list_schema(self).await
    }
    async fn list_prepares(&self) -> anyhow::Result<Vec<DBPrepare>> {
        LangServerCommon::list_prepares(self.db.as_ref()).await
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for MariaDB {
    async fn list_columns(&self, table_name: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        AstCompletionHelper::list_column(self, None, None, table_name, false).await
    }
    type Dialect = sqlparser::dialect::MySqlDialect;

    const TEXTS: &'static [&'static str] = MysqlDB::TEXTS;
    const TYPES: &'static [&'static str] = MysqlDB::TYPES;

    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self, None, None, true).await
    }

    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        self.db.list_table_or_view().await
    }

    async fn get_key_words_fn(&self) -> anyhow::Result<Vec<CompletionItem>> {
        self.db.get_key_words_fn().await
    }
}

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
