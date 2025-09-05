use std::ops::DerefMut;

#[cfg(test)]
use crate::datasource_config::MysqlDBConfig;
use crate::db::DBTrait;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, DBExecutable, ErrAndLoc, LangServerCommon,
    LanguageServer, LexerCompletionTrait, try_parser_dot_lexer,
};
use crate::mysql::MysqlDB;
use crate::sqlparser_completion::TempTableColumn;
use crate::tidb::TiDB;
use sqlx::Row;

#[async_trait::async_trait]
impl LanguageServer for TiDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        try_parser_dot_lexer(self, sql, line, col, use_lexer).await
    }
    fn parse_error(&self, sql: &str) -> anyhow::Result<Option<ErrAndLoc>> {
        LanguageServer::parse_error(self.db.as_ref(), sql)
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for TiDB {
    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        AstCompletionHelper::get_query_columns(self.db.as_ref(), sql).await
    }

    async fn list_user(&self) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_user(self.db.as_ref()).await
    }

    async fn list_role(&self) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_user(self.db.as_ref()).await
    }

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_index(self.db.as_ref(), schema).await
    }

    async fn list_variable(&self) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_variable(self.db.as_ref()).await
    }

    async fn list_partition(
        &self, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_partition(self.db.as_ref(), schema, table).await
    }

    async fn list_constraint(
        &self, schema: Option<&str>, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_constraint(self.db.as_ref(), schema, table_name).await
    }

    async fn list_table_trigger(
        &self, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_table_trigger(self.db.as_ref(), schema, table).await
    }
}

#[async_trait::async_trait]

impl LangServerCommon for TiDB {
    async fn list_column(
        &self, schema: Option<&str>, name: &str, is_call: bool,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        LangServerCommon::list_column(self.db.as_ref(), schema, name, is_call).await
    }
    // some backend can use a query to query this.
    async fn key_words(&self) -> anyhow::Result<Vec<CompletionItem>> {
        /*
            原因一句话：TiDB v7.5.1 还没有 information_schema.keywords 这张表，所以你查不到。

            该表是 v7.5.3（2024-08-05 发布的 LTS）才正式加入的

        。
        你当前的 8.0.11-TiDB-v7.5.1 恰好是 7.5.1，版本号里前面只是伪装成 MySQL 8.0 的“兼容前缀”，实际内核仍是 7.5.1，因此 表不存在。
         */
        let r = self.db.key_words().await.ok().unwrap_or_default();
        if !r.is_empty() {
            return anyhow::Ok(r);
        }
        let r = TIDB_KEYWORDS
            .iter()
            .map(|x| CompletionItem {
                item: (*x).into(),
                desc: None,
                kind: CompletionKind::KeyWord,
            })
            .collect::<Vec<_>>();
        anyhow::Ok(r)
    }

    async fn list_procedure(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_procedure(self.db.as_ref(), schema).await
    }
    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::MySqlDialect {})
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        DBTrait::list_schema(self).await
    }
    async fn list_execute(&self, _: Option<&str>) -> anyhow::Result<Vec<DBExecutable>> {
        LangServerCommon::list_execute(self.db.as_ref(), None).await
    }
    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_table(self.db.as_ref(), schema).await
    }

    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_view(self.db.as_ref(), schema).await
    }

    async fn list_function(
        &self, _schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        if !include_build_in {
            return anyhow::Ok(vec![]);
        }

        // rightnow we only support build in functions.
        let mut lock = self.build_in_fn.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        let mut ret = vec![];
        let mut conn = self.db.get_conn().await?;
        let rows = sqlx::raw_sql("SHOW BUILTINS;")
            .fetch_all(conn.deref_mut())
            .await?;
        for row in rows.iter() {
            let name: Option<String> = row.try_get(0)?;
            if let Some(name) = name {
                ret.push(CompletionItem {
                    item: name.into(),
                    kind: CompletionKind::Function,
                    desc: None,
                });
            }
        }
        *lock = ret.clone();
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for TiDB {
    const TEXTS: &'static [&'static str] = MysqlDB::TEXTS;
    const TYPES: &'static [&'static str] = MysqlDB::TYPES;
}

#[allow(dead_code)]
#[cfg(test)]
pub(crate) fn make_config() -> MysqlDBConfig {
    MysqlDBConfig {
        conn_timeout: None,
        host: crate::TEST_HOST.to_string(),
        username: "root".to_string(),
        password: Some("root".to_string()),
        port: 3307,
        db: Some("test".to_string()),
        read_only: false,
        save_password: true,
        ssl: Default::default(),
    }
}

#[cfg(test)]
pub(crate) async fn make_test_object_tidb(config: MysqlDBConfig) -> TiDB {
    let db: TiDB = TiDB::new(config).await.unwrap();
    db
}

#[cfg(test)]
mod tests {
    use crate::{
        db::{AutoCompletionQueryUuid, QuerySql},
        language_server::LangServerCommon,
        tidb::make_config,
        tidb_lang_server::make_test_object_tidb,
    };

    /*
    TODO support this.
    在 TiDB 中，“当前连接” 已经 PREPARE 的语句不会单独写入 information_schema 表，而是 保存在会话内存 里，因此官方没有 “一条 SQL 列出所有 prepare” 的固定表。
    但你可以通过以下 两种方式 实时查看：
     */
    #[ignore = "xxxx"]
    #[tokio::test]
    async fn test_prepare() {
        let _guard = AutoCompletionQueryUuid::test_mutex().lock().unwrap();
        let db = make_test_object_tidb(make_config()).await;
        let query_id = db.open_connection().await.unwrap();
        AutoCompletionQueryUuid::set(query_id.clone());
        db.execute_sql(
            query_id,
            r#"
            PREPARE stmtxxx FROM 'SELECT 1';
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        assert!(
            db.list_execute(None)
                .await
                .unwrap()
                .iter()
                .any(|x| x.name.eq("stmtxxx"))
        );
    }
}
/// TiDB 关键字全集（含保留/非保留）
pub const TIDB_KEYWORDS: &[&str] = &[
    "ACCOUNT",
    "ACTION",
    "ADD",
    "ADMIN",
    "ADVISE",
    "AFTER",
    "AGAINST",
    "AGO",
    "ALGORITHM",
    "ALL",
    "ALTER",
    "ALWAYS",
    "ANALYZE",
    "AND",
    "ANY",
    "APPLY",
    "ARRAY",
    "AS",
    "ASC",
    "ASCII",
    "ATTRIBUTE",
    "ATTRIBUTES",
    "AUTO_ID_CACHE",
    "AUTO_INCREMENT",
    "AUTO_RANDOM",
    "AUTO_RANDOM_BASE",
    "AVG",
    "AVG_ROW_LENGTH",
    "BACKEND",
    "BACKUP",
    "BACKUPS",
    "BATCH",
    "BDR",
    "BEGIN",
    "BERNOULLI",
    "BETWEEN",
    "BIGINT",
    "BINARY",
    "BINDING",
    "BINDINGS",
    "BINDING_CACHE",
    "BINLOG",
    "BIT",
    "BLOB",
    "BLOCK",
    "BOOL",
    "BOOLEAN",
    "BOTH",
    "BTREE",
    "BUCKETS",
    "BUILTINS",
    "BY",
    "BYTE",
    "CACHE",
    "CALIBRATE",
    "CALL",
    "CANCEL",
    "CAPTURE",
    "CARDINALITY",
    "CASCADE",
    "CASCADED",
    "CASE",
    "CAUSAL",
    "CHAIN",
    "CHANGE",
    "CHAR",
    "CHARACTER",
    "CHARSET",
    "CHECK",
    "CHECKPOINT",
    "CHECKSUM",
    "CHECKSUM_CONCURRENCY",
    "CIPHER",
    "CLEANUP",
    "CLIENT",
    "CLIENT_ERRORS_SUMMARY",
    "CLOSE",
    "CLUSTER",
    "CLUSTERED",
    "CMSKETCH",
    "COALESCE",
    "COLLATE",
    "COLLATION",
    "COLUMN",
    "COLUMN_FORMAT",
    "COLUMN_STATS_USAGE",
    "COLUMNS",
    "COMMENT",
    "COMMIT",
    "COMMITTED",
    "COMPACT",
    "COMPRESSED",
    "COMPRESSION",
    "COMPRESSION_LEVEL",
    "COMPRESSION_TYPE",
    "CONCURRENCY",
    "CONFIG",
    "CONNECTION",
    "CONSISTENCY",
    "CONSISTENT",
    "CONSTRAINT",
    "CONTEXT",
    "CONTINUE",
    "CONVERT",
    "CORRELATION",
    "CPU",
    "CREATE",
    "CROSS",
    "CSV_BACKSLASH_ESCAPE",
    "CSV_DELIMITER",
    "CSV_HEADER",
    "CSV_NOT_NULL",
    "CSV_NULL",
    "CSV_SEPARATOR",
    "CSV_TRIM_LAST_SEPARATORS",
    "CUME_DIST",
    "CURRENT",
    "CURRENT_DATE",
    "CURRENT_ROLE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "CURRENT_USER",
    "CURSOR",
    "CYCLE",
    "DATA",
    "DATABASE",
    "DATABASES",
    "DATE",
    "DATETIME",
    "DAY",
    "DAY_HOUR",
    "DAY_MICROSECOND",
    "DAY_MINUTE",
    "DAY_SECOND",
    "DDL",
    "DEALLOCATE",
    "DEC",
    "DECIMAL",
    "DECLARE",
    "DEFAULT",
    "DEFINER",
    "DELAYED",
    "DELAY_KEY_WRITE",
    "DELETE",
    "DENSE_RANK",
    "DESC",
    "DESCRIBE",
    "DETERMINISTIC",
    "DIAGNOSTICS",
    "DIRECTORY",
    "DISABLE",
    "DISCARD",
    "DISK",
    "DISTINCT",
    "DISTINCTROW",
    "DIV",
    "DO",
    "DOUBLE",
    "DROP",
    "DUAL",
    "DUMPFILE",
    "DUPLICATE",
    "DYNAMIC",
    "EACH",
    "ELSE",
    "ELSEIF",
    "ENABLE",
    "ENCLOSED",
    "ENCRYPTION",
    "END",
    "ENFORCED",
    "ENGINE",
    "ENGINES",
    "ENUM",
    "ERROR",
    "ERRORS",
    "ESCAPE",
    "ESCAPED",
    "EVENT",
    "EVENTS",
    "EVERY",
    "EXCEPT",
    "EXCHANGE",
    "EXCLUDE",
    "EXECUTE",
    "EXISTS",
    "EXIT",
    "EXPANSION",
    "EXPIRE",
    "EXPLAIN",
    "EXTENDED",
    "EXTENSION",
    "EXTERNAL",
    "EXTRACT",
    "FALSE",
    "FAULTS",
    "FETCH",
    "FIELDS",
    "FILE",
    "FILE_BLOCK_SIZE",
    "FILTER",
    "FIRST",
    "FIRST_VALUE",
    "FIXED",
    "FLOAT",
    "FLOAT4",
    "FLOAT8",
    "FLUSH",
    "FOLLOWING",
    "FOR",
    "FORCE",
    "FOREIGN",
    "FORMAT",
    "FOUND",
    "FROM",
    "FULL",
    "FULLTEXT",
    "FUNCTION",
    "GENERAL",
    "GENERATED",
    "GEOMETRY",
    "GEOMETRYCOLLECTION",
    "GET_FORMAT",
    "GET_LOCK",
    "GLOBAL",
    "GRANT",
    "GRANTS",
    "GROUP",
    "GROUP_CONCAT",
    "GROUP_REPLICATION",
    "HANDLER",
    "HASH",
    "HAVING",
    "HELP",
    "HIGH_PRIORITY",
    "HISTORY",
    "HOST",
    "HOSTS",
    "HOUR",
    "HOUR_MICROSECOND",
    "HOUR_MINUTE",
    "HOUR_SECOND",
    "IDENTIFIED",
    "IF",
    "IGNORE",
    "IMPORT",
    "IN",
    "INDEX",
    "INDEXES",
    "INFILE",
    "INITIAL_SIZE",
    "INNER",
    "INOUT",
    "INSENSITIVE",
    "INSERT",
    "INSERT_METHOD",
    "INSTALL",
    "INSTANCE",
    "INT",
    "INT1",
    "INT2",
    "INT3",
    "INT4",
    "INT8",
    "INTEGER",
    "INTERVAL",
    "INTO",
    "INVISIBLE",
    "IO",
    "IO_AFTER_GTIDS",
    "IO_BEFORE_GTIDS",
    "IPC",
    "IS",
    "ISOLATION",
    "ISSUER",
    "ITERATE",
    "JOIN",
    "JSON",
    "JSON_ARRAY",
    "JSON_ARRAYAGG",
    "JSON_OBJECT",
    "JSON_OBJECTAGG",
    "JSON_TABLE",
    "JSON_VALUE",
    "KEY",
    "KEY_BLOCK_SIZE",
    "KEYS",
    "KILL",
    "LAG",
    "LANGUAGE",
    "LAST",
    "LAST_VALUE",
    "LATERAL",
    "LEAD",
    "LEADING",
    "LEAVE",
    "LEFT",
    "LESS",
    "LEVEL",
    "LIKE",
    "LIMIT",
    "LINEAR",
    "LINES",
    "LIST",
    "LOAD",
    "LOCAL",
    "LOCALTIME",
    "LOCALTIMESTAMP",
    "LOCK",
    "LOCKED",
    "LOCKS",
    "LOGFILE",
    "LOGS",
    "LONG",
    "LONGBLOB",
    "LONGTEXT",
    "LOOP",
    "LOW_PRIORITY",
    "MASTER",
    "MASTER_AUTO_POSITION",
    "MASTER_BIND",
    "MASTER_CONNECT_RETRY",
    "MASTER_DELAY",
    "MASTER_HEARTBEAT_PERIOD",
    "MASTER_HOST",
    "MASTER_LOG_FILE",
    "MASTER_LOG_POS",
    "MASTER_PASSWORD",
    "MASTER_PORT",
    "MASTER_RETRY_COUNT",
    "MASTER_SSL",
    "MASTER_SSL_CA",
    "MASTER_SSL_CAPATH",
    "MASTER_SSL_CERT",
    "MASTER_SSL_CIPHER",
    "MASTER_SSL_CRL",
    "MASTER_SSL_CRLPATH",
    "MASTER_SSL_KEY",
    "MASTER_TLS_VERSION",
    "MASTER_USER",
    "MATCH",
    "MAX",
    "MAXVALUE",
    "MAX_CONNECTIONS_PER_HOUR",
    "MAX_QUERIES_PER_HOUR",
    "MAX_ROWS",
    "MAX_SIZE",
    "MAX_UPDATES_PER_HOUR",
    "MAX_USER_CONNECTIONS",
    "MBRContains",
    "MBRCoveredBy",
    "MBRCovers",
    "MBRDisjoint",
    "MBREquals",
    "MBRIntersects",
    "MBROverlaps",
    "MBRTouches",
    "MBRWithin",
    "MEDIUM",
    "MEDIUMBLOB",
    "MEDIUMINT",
    "MEDIUMTEXT",
    "MEMORY",
    "MERGE",
    "MICROSECOND",
    "MIDDLEINT",
    "MIN",
    "MINUTE",
    "MINUTE_MICROSECOND",
    "MINUTE_SECOND",
    "MIN_ROWS",
    "MOD",
    "MODE",
    "MODIFIES",
    "MODIFY",
    "MONTH",
    "MULTILINESTRING",
    "MULTIPOINT",
    "MULTIPOLYGON",
    "MUTEX",
    "MYSQL_ERRNO",
    "NAME",
    "NAMES",
    "NATIONAL",
    "NATURAL",
    "NCHAR",
    "NDB",
    "NDBCLUSTER",
    "NESTED",
    "NEVER",
    "NEW",
    "NEXT",
    "NO",
    "NODEGROUP",
    "NONE",
    "NOT",
    "NOW",
    "NO_WAIT",
    "NO_WRITE_TO_BINLOG",
    "NTH_VALUE",
    "NTILE",
    "NULL",
    "NULLS",
    "NUMBER",
    "NUMERIC",
    "NVARCHAR",
    "OF",
    "OFF",
    "OFFSET",
    "OLD",
    "ON",
    "ONE",
    "ONLY",
    "OPEN",
    "OPTIMIZE",
    "OPTIMIZER_COSTS",
    "OPTION",
    "OPTIONALLY",
    "OR",
    "ORDER",
    "ORDINALITY",
    "OUT",
    "OUTER",
    "OUTFILE",
    "OVER",
    "OWNER",
    "PACK_KEYS",
    "PAGE",
    "PARSER",
    "PARTIAL",
    "PARTITION",
    "PARTITIONING",
    "PARTITIONS",
    "PASSWORD",
    "PASSWORD_LOCK_TIME",
    "PAUSE",
    "PERCENT_RANK",
    "PERSIST",
    "PERSIST_ONLY",
    "PHASE",
    "PLUGIN",
    "PLUGINS",
    "POINT",
    "POLYGON",
    "PORT",
    "POSITION",
    "PRECEDING",
    "PRECISION",
    "PREPARE",
    "PRESERVE",
    "PREV",
    "PRIMARY",
    "PRIVILEGES",
    "PROCEDURE",
    "PROCESS",
    "PROCESSLIST",
    "PROFILE",
    "PROFILES",
    "PROXY",
    "PURGE",
    "QUARTER",
    "QUERY",
    "QUICK",
    "RANK",
    "READ",
    "READ_ONLY",
    "READ_WRITE",
    "REAL",
    "REBUILD",
    "RECOVER",
    "RECURSIVE",
    "REDOFILE",
    "REDO_BUFFER_SIZE",
    "REDUNDANT",
    "REFERENCE",
    "REFERENCES",
    "REGEXP",
    "RELAY",
    "RELAYLOG",
    "RELAY_LOG_FILE",
    "RELAY_LOG_POS",
    "RELAY_THREAD",
    "RELEASE",
    "RELOAD",
    "REMOVE",
    "RENAME",
    "REORGANIZE",
    "REPAIR",
    "REPEAT",
    "REPEATABLE",
    "REPLACE",
    "REPLICATE_DO_DB",
    "REPLICATE_DO_TABLE",
    "REPLICATE_IGNORE_DB",
    "REPLICATE_IGNORE_TABLE",
    "REPLICATE_REWRITE_DB",
    "REPLICATE_WILD_DO_TABLE",
    "REPLICATE_WILD_IGNORE_TABLE",
    "REPLICATION",
    "REQUIRE",
    "RESET",
    "RESIGNAL",
    "RESOURCE",
    "RESPECT",
    "RESTART",
    "RESTORE",
    "RESTRICT",
    "RESULT",
    "RESUME",
    "RETURN",
    "RETURNED_SQLSTATE",
    "RETURNS",
    "REUSE",
    "REVERSE",
    "REVOKE",
    "RIGHT",
    "RLIKE",
    "ROLE",
    "ROLLBACK",
    "ROLLUP",
    "ROTATE",
    "ROUTINE",
    "ROW",
    "ROW_COUNT",
    "ROW_FORMAT",
    "ROWS",
    "ROW_NUMBER",
    "RTREE",
    "SAVEPOINT",
    "SCHEMA",
    "SCHEMAS",
    "SECOND",
    "SECOND_MICROSECOND",
    "SECURITY",
    "SELECT",
    "SEPARATOR",
    "SERIAL",
    "SERIALIZABLE",
    "SERVER",
    "SESSION",
    "SET",
    "SHARE",
    "SHOW",
    "SHUTDOWN",
    "SIGNAL",
    "SIGNED",
    "SIMPLE",
    "SKIP",
    "SLAVE",
    "SLOW",
    "SMALLINT",
    "SNAPSHOT",
    "SOME",
    "SONAME",
    "SOUNDS",
    "SOURCE",
    "SPATIAL",
    "SPECIFIC",
    "SQL",
    "SQLEXCEPTION",
    "SQLSTATE",
    "SQLWARNING",
    "SQL_BIG_RESULT",
    "SQL_BUFFER_RESULT",
    "SQL_CACHE",
    "SQL_CALC_FOUND_ROWS",
    "SQL_NO_CACHE",
    "SQL_SMALL_RESULT",
    "SQL_THREAD",
    "SQL_TSI_DAY",
    "SQL_TSI_HOUR",
    "SQL_TSI_MINUTE",
    "SQL_TSI_MONTH",
    "SQL_TSI_QUARTER",
    "SQL_TSI_SECOND",
    "SQL_TSI_WEEK",
    "SQL_TSI_YEAR",
    "SSL",
    "STACKED",
    "START",
    "STARTING",
    "STATS",
    "STATS_AUTO_RECALC",
    "STATS_PERSISTENT",
    "STATS_SAMPLE_PAGES",
    "STATUS",
    "STDDEV",
    "STDDEV_POP",
    "STDDEV_SAMP",
    "STOP",
    "STORAGE",
    "STORED",
    "STRAIGHT_JOIN",
    "STRING",
    "SUBCLASS_ORIGIN",
    "SUBJECT",
    "SUBPARTITION",
    "SUBPARTITIONS",
    "SUBSTRING",
    "SUM",
    "SUPER",
    "SUSPEND",
    "SWAPS",
    "SWITCHES",
    "SYSDATE",
    "SYSTEM",
    "TABLE",
    "TABLES",
    "TABLESPACE",
    "TABLE_CHECKSUM",
    "TABLE_NAME",
    "TEMPORARY",
    "TEMPTABLE",
    "TERMINATED",
    "TEXT",
    "THAN",
    "THEN",
    "TIDB",
    "TIDB_CURRENT_TSO",
    "TIFLASH",
    "TIKV_IMPORTER",
    "TIME",
    "TIMESTAMP",
    "TINYBLOB",
    "TINYINT",
    "TINYTEXT",
    "TO",
    "TOKEN_ISSUER",
    "TOPN",
    "TPCC",
    "TPCH_10",
    "TRACE",
    "TRADITIONAL",
    "TRAILING",
    "TRANSACTION",
    "TRIGGER",
    "TRIGGERS",
    "TRUE",
    "TRUNCATE",
    "TSO",
    "TTL",
    "TTL_ENABLE",
    "TTL_JOB_INTERVAL",
    "TYPE",
    "TYPES",
    "UBOUNDED",
    "UNCOMMITTED",
    "UNDEFINED",
    "UNICODE",
    "UNION",
    "UNIQUE",
    "UNKNOWN",
    "UNLOCK",
    "UNSET",
    "UNSIGNED",
    "UNTIL",
    "UPDATE",
    "UPGRADE",
    "USAGE",
    "USE",
    "USER",
    "USING",
    "UTC_DATE",
    "UTC_TIME",
    "UTC_TIMESTAMP",
    "VALIDATION",
    "VALUE",
    "VALUES",
    "VARBINARY",
    "VARCHAR",
    "VARCHARACTER",
    "VARIABLES",
    "VARYING",
    "VECTOR",
    "VIEW",
    "VIRTUAL",
    "VISIBLE",
    "WAIT",
    "WAIT_TIFLASH_READY",
    "WEEK",
    "WEIGHT_STRING",
    "WHEN",
    "WHERE",
    "WHILE",
    "WIDTH",
    "WINDOW",
    "WITH",
    "WITH_SYS_TABLE",
    "WITHOUT",
    "WORKLOAD",
    "WRITE",
    "X509",
    "XOR",
    "YEAR",
    "YEAR_MONTH",
    "ZEROFILL",
];
