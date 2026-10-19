#![allow(unused)]

use tokio::task::spawn_blocking;

use crate::{
    db::GetBackendKind,
    db2::Db2Db,
    language_server::{
        AstCompletionHelper, GetKeyWords, LangServerCommon, LanguageServer, LexerCompletionTrait,
        try_parser_dot_lexer,
    },
};

impl Db2Db {
    async fn get_current_schema(&self, schema: Option<&str>) -> anyhow::Result<String> {
        match schema {
            Some(x) => anyhow::Ok(x.to_owned()),
            None => {
                let s = self
                    .query_something("SELECT CURRENT SCHEMA FROM SYSIBM.SYSDUMMY1", |b| {
                        let x: Option<String> = b.get_data(1)?;
                        anyhow::Ok(x)
                    })
                    .await?;
                let r = s.into_iter().flatten().next();
                match r {
                    Some(s) => anyhow::Ok(s),
                    None => anyhow::bail!("no current schema select"),
                }
            }
        }
    }
}

#[async_trait::async_trait]

impl LanguageServer for Db2Db {
    async fn auto_completion(
        &self, query_id: Option<String>, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<crate::language_server::CompletionList> {
        try_parser_dot_lexer(self, sql, line, col, use_lexer).await
    }

    async fn ai_text_completion(
        &self, sql: &str, line: u32, col: u32,
    ) -> anyhow::Result<crate::language_server::CompletionList> {
        anyhow::Ok(Default::default())
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        LangServerCommon::is_dangerous_sql_by_parser(self, sql)
    }

    fn extract_table_select_selection(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_selection(self, sql)
    }

    fn extract_table_select_sort(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_sort(self, sql)
    }
}

#[async_trait::async_trait]

impl GetKeyWords for Db2Db {
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(DB2_KEYWORDS.iter().map(|x| x.to_string()).collect())
    }
}

#[async_trait::async_trait]

impl LexerCompletionTrait for Db2Db {
    async fn get_types(&self) -> anyhow::Result<Vec<String>> {
        let sql =
            "SELECT TYPENAME FROM SYSCAT.DATATYPES WHERE TYPESCHEMA = 'SYSIBM' ORDER BY TYPEID;";
        let r = self
            .query_something(sql, |b| {
                let x: Option<String> = b.get_data(1)?;
                anyhow::Ok(x)
            })
            .await?;
        let r = r.into_iter().flatten().collect();
        anyhow::Ok(r)
    }
}

#[async_trait::async_trait]

impl LangServerCommon for Db2Db {
    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(Default::default())
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        self.list_schema_impl().await
    }

    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::GenericDialect {})
    }

    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = self.get_current_schema(schema).await?;
        let r = self.list_table_impl(&schema).await?;
        anyhow::Ok(r.into_iter().map(|x| x.name).collect())
    }

    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(Default::default())
    }

    async fn list_function(
        &self, schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<crate::language_server::CompletionItem>> {
        anyhow::Ok(Default::default())
    }

    async fn list_column(
        &self, schema: Option<&str>, name: &str, is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<crate::sqlparser_completion::TempTableColumn>> {
        anyhow::Ok(Default::default())
    }
}

#[async_trait::async_trait]

impl AstCompletionHelper for Db2Db {
    async fn get_query_columns(
        &self, _sql: &str,
    ) -> anyhow::Result<Vec<crate::sqlparser_completion::TempTableColumn>> {
        anyhow::Ok(Default::default())
    }
}

// Provide all DB2 keywords in Rust array format.

/// DB2 keywords and reserved words.
/// Source: official IBM DB2 documentation.
/// https://www.ibm.com/docs/zh/db2/11.5.x?topic=sql-reserved-schema-names-reserved-words
/// Note: This list contains the main keywords for DB2 for LUW and may need to be adjusted for specific versions.
pub const DB2_KEYWORDS: &[&str] = &[
    "ABS",
    "ACTIVATE",
    "ADD",
    "AFTER",
    "ALIAS",
    "ALL",
    "ALLOCATE",
    "ALLOW",
    "ALTER",
    "AND",
    "ANY",
    "ARE",
    "ARRAY",
    "AS",
    "ASENSITIVE",
    "ASSOCIATE",
    "ASUTIME",
    "ASYMMETRIC",
    "AT",
    "ATOMIC",
    "ATTRIBUTES",
    "AUDIT",
    "AUTHORIZATION",
    "AUX",
    "AUXILIARY",
    "AVG",
    "BEFORE",
    "BEGIN",
    "BETWEEN",
    "BIGINT",
    "BINARY",
    "BLOB",
    "BOOLEAN",
    "BOTH",
    "BUFFERPOOL",
    "BY",
    "CACHE",
    "CALL",
    "CALLED",
    "CAPTURE",
    "CARDINALITY",
    "CASCADED",
    "CASE",
    "CAST",
    "CCSID",
    "CEIL",
    "CEILING",
    "CHAR",
    "CHAR_LENGTH",
    "CHARACTER",
    "CHARACTER_LENGTH",
    "CHECK",
    "CLOB",
    "CLONE",
    "CLOSE",
    "CLUSTER",
    "COALESCE",
    "COLLATE",
    "COLLECT",
    "COLLECTION",
    "COLLID",
    "COLUMN",
    "COMMENT",
    "COMMIT",
    "CONCAT",
    "CONDITION",
    "CONNECT",
    "CONNECTION",
    "CONSTRAINT",
    "CONTAINS",
    "CONTINUE",
    "CONVERT",
    "CORR",
    "CORRESPONDING",
    "COUNT",
    "COUNT_BIG",
    "COVAR_POP",
    "COVAR_SAMP",
    "CREATE",
    "CROSS",
    "CUBE",
    "CUME_DIST",
    "CURRENT",
    "CURRENT_DATE",
    "CURRENT_DEFAULT_TRANSFORM_GROUP",
    "CURRENT_LC_CTYPE",
    "CURRENT_PATH",
    "CURRENT_ROLE",
    "CURRENT_SCHEMA",
    "CURRENT_SERVER",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "CURRENT_TIMEZONE",
    "CURRENT_TRANSFORM_GROUP_FOR_TYPE",
    "CURRENT_USER",
    "CURSOR",
    "CYCLE",
    "DATA",
    "DATABASE",
    "DATAPARTITIONNAME",
    "DATAPARTITIONNUM",
    "DATE",
    "DAY",
    "DAYS",
    "DB2GENERAL",
    "DB2GENRL",
    "DB2SQL",
    "DBINFO",
    "DBPARTITIONNAME",
    "DBPARTITIONNUM",
    "DEALLOCATE",
    "DEC",
    "DECIMAL",
    "DECLARE",
    "DEFAULT",
    "DEFAULTS",
    "DEFINITION",
    "DELETE",
    "DENSE_RANK",
    "DENSERANK",
    "DEREF",
    "DESCRIBE",
    "DESCRIPTOR",
    "DETERMINISTIC",
    "DIAGNOSTICS",
    "DISABLE",
    "DISALLOW",
    "DISCONNECT",
    "DISTINCT",
    "DO",
    "DOCUMENT",
    "DOUBLE",
    "DROP",
    "DSSIZE",
    "DYNAMIC",
    "EACH",
    "EDITPROC",
    "ELEMENT",
    "ELSE",
    "ELSEIF",
    "ENABLE",
    "ENCODING",
    "ENCRYPTION",
    "END",
    "END-EXEC",
    "ENDING",
    "ERASE",
    "ESCAPE",
    "EVERY",
    "EXCEPT",
    "EXCEPTION",
    "EXCLUDING",
    "EXCLUSIVE",
    "EXEC",
    "EXECUTE",
    "EXISTS",
    "EXIT",
    "EXP",
    "EXPLAIN",
    "EXTENDED",
    "EXTERNAL",
    "EXTRACT",
    "FALSE",
    "FENCED",
    "FETCH",
    "FIELDPROC",
    "FILE",
    "FILTER",
    "FINAL",
    "FIRST",
    "FLOAT",
    "FLOOR",
    "FOR",
    "FOREIGN",
    "FREE",
    "FROM",
    "FULL",
    "FUNCTION",
    "FUSION",
    "GENERAL",
    "GENERATED",
    "GET",
    "GLOBAL",
    "GO",
    "GOTO",
    "GRANT",
    "GRAPHIC",
    "GROUP",
    "GROUPING",
    "HANDLER",
    "HASH",
    "HASHED_VALUE",
    "HAVING",
    "HINT",
    "HOLD",
    "HOUR",
    "HOURS",
    "IDENTITY",
    "IF",
    "IMMEDIATE",
    "IMPORT",
    "IN",
    "INCLUDING",
    "INCLUSIVE",
    "INCREMENT",
    "INDEX",
    "INDICATOR",
    "INDICATORS",
    "INF",
    "INFINITY",
    "INHERIT",
    "INNER",
    "INOUT",
    "INSENSITIVE",
    "INSERT",
    "INT",
    "INTEGER",
    "INTEGRITY",
    "INTERSECT",
    "INTERSECTION",
    "INTERVAL",
    "INTO",
    "IS",
    "ISNULL",
    "ISOBID",
    "ISOLATION",
    "ITERATE",
    "JAR",
    "JAVA",
    "JOIN",
    "KEEP",
    "KEY",
    "LABEL",
    "LANGUAGE",
    "LARGE",
    "LAST3",
    "LATERAL",
    "LC_CTYPE",
    "LEADING",
    "LEAVE",
    "LEFT",
    "LIKE",
    "LIMIT",
    "LINKTYPE",
    "LN",
    "LOCAL",
    "LOCALDATE",
    "LOCALE",
    "LOCALTIME",
    "LOCALTIMESTAMP",
    "LOCATOR",
    "LOCATORS",
    "LOCK",
    "LOCKMAX",
    "LOCKSIZE",
    "LONG",
    "LOOP",
    "LOWER",
    "MAINTAINED",
    "MATCH",
    "MATERIALIZED",
    "MAX",
    "MAXVALUE",
    "MEMBER",
    "MERGE",
    "METHOD",
    "MICROSECOND",
    "MICROSECONDS",
    "MIN",
    "MINUTE",
    "MINUTES",
    "MINVALUE",
    "MOD",
    "MODE",
    "MODIFIES",
    "MODULE",
    "MONTH",
    "MONTHS",
    "MULTISET",
    "NAN",
    "NATIONAL",
    "NATURAL",
    "NCHAR",
    "NCLOB",
    "NEW",
    "NEW_TABLE",
    "NEXTVAL",
    "NO",
    "NOCACHE",
    "NOCYCLE",
    "NODENAME",
    "NODENUMBER",
    "NOMAXVALUE",
    "NOMINVALUE",
    "NONE",
    "NOORDER",
    "NORMALIZE",
    "NORMALIZED",
    "NOT",
    "NOTNULL",
    "NULL",
    "NULLIF",
    "NULLS",
    "NUMERIC",
    "NUMPARTS",
    "OBID",
    "OCTET_LENGTH",
    "OF",
    "OFF",
    "OFFSET",
    "OLD",
    "OLD_TABLE",
    "ON",
    "ONLY",
    "OPEN",
    "OPTIMIZATION",
    "OPTIMIZE",
    "OPTION",
    "OR",
    "ORDER",
    "OUT",
    "OUTER",
    "OVER",
    "OVERLAPS",
    "OVERLAY",
    "OVERRIDING",
    "PACKAGE",
    "PADDED",
    "PAGESIZE",
    "PARAMETER",
    "PART",
    "PARTITION",
    "PARTITIONED",
    "PARTITIONING",
    "PARTITIONS",
    "PASSWORD",
    "PATH",
    "PERCENT",
    "PERCENT_RANK",
    "PERCENTILE_CONT",
    "PERCENTILE_DISC",
    "PIECESIZE",
    "PLAN",
    "POSITION",
    "POWER",
    "PRECISION",
    "PREPARE",
    "PREVVAL",
    "PRIMARY",
    "PRIQTY",
    "PRIVILEGES",
    "PROCEDURE",
    "PROGRAM",
    "PSID",
    "PUBLIC",
    "QUERY",
    "QUERYNO",
    "RANGE",
    "RANK",
    "READ",
    "READS",
    "REAL",
    "RECOVERY",
    "RECURSIVE",
    "REF",
    "REFERENCES",
    "REFERENCING",
    "REFRESH",
    "RELEASE",
    "RENAME",
    "REPEAT",
    "RESET",
    "RESIGNAL",
    "RESTART",
    "RESTRICT",
    "RESULT",
    "RESULT_SET_LOCATOR",
    "RETURN",
    "RETURNS",
    "REVOKE",
    "RIGHT",
    "ROLE",
    "ROLLBACK",
    "ROLLUP",
    "ROUND_CEILING",
    "ROUND_DOWN",
    "ROUND_FLOOR",
    "ROUND_HALF_DOWN",
    "ROUND_HALF_EVEN",
    "ROUND_HALF_UP",
    "ROUND_UP",
    "ROUTINE",
    "ROW",
    "ROWNUMBER",
    "ROWS",
    "ROWSET",
    "RRN",
    "RUN",
    "SAVEPOINT",
    "SCHEMA",
    "SCRATCHPAD",
    "SCROLL",
    "SEARCH",
    "SECOND",
    "SECONDS",
    "SECQTY",
    "SECURITY",
    "SELECT",
    "SENSITIVE",
    "SEQUENCE",
    "SESSION",
    "SESSION_USER",
    "SET",
    "SIGNAL",
    "SIMPLE",
    "SNAN",
    "SOME",
    "SOURCE",
    "SPECIFIC",
    "SPECIFICTYPE",
    "SQL",
    "SQLEXCEPTION",
    "SQLID",
    "SQLSTATE",
    "SQLWARNING",
    "STACKED",
    "STANDARD",
    "START",
    "STARTING",
    "STATEMENT",
    "STATIC",
    "STATISTICS",
    "STAY",
    "STOGROUP",
    "STORES",
    "STYLE",
    "SUBMULTISET",
    "SUBSTRING",
    "SUM",
    "SUMMARY",
    "SYNONYM",
    "SYSFUN",
    "SYSIBM",
    "SYSPROC",
    "SYSTEM",
    "SYSTEM_USER",
    "TABLE",
    "TABLESPACE",
    "TEMPORARY",
    "THEN",
    "TIME",
    "TIMESTAMP",
    "TO",
    "TRANSACTION",
    "TRIGGER",
    "TRIM",
    "TRUNCATE",
    "TYPE",
    "UNDO",
    "UNION",
    "UNIQUE",
    "UNKNOWN",
    "UNLINK",
    "UNNEST",
    "UNTIL",
    "UPDATE",
    "USAGE",
    "USER",
    "USING",
    "UTC",
    "UTP",
    "VALUE",
    "VALUES",
    "VARIABLE",
    "VARIANT",
    "VCAT",
    "VERSION",
    "VIEW",
    "VOLATILE",
    "VOLUMES",
    "WHEN",
    "WHENEVER",
    "WHERE",
    "WHILE",
    "WITH",
    "WITHOUT",
    "WLM",
    "WRITE",
    "XMLELEMENT",
    "XMLEXISTS",
    "XMLNAMESPACES",
    "YEAR",
    "YEARS",
];
