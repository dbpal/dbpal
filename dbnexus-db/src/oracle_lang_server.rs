use std::sync::Arc;

use tokio::task::block_in_place;

use crate::db::DBTrait;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, LangServerCommon, LanguageServer,
    LexerCompletionTrait, try_completion_on_syntax,
};
use crate::oracle::OracleDB;

#[async_trait::async_trait]
impl LexerCompletionTrait for OracleDB {
    type Dialect = sqlparser::dialect::GenericDialect;

    const KEY_WORDS: &'static [&'static str] = ORACLE_KEY_WORDS;
    const TYPES: &'static [&'static str] = ORACLE_TYPES;

    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        let sql = format!(r#"SELECT TABLE_NAME FROM DBA_TABLES"#);
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        let result = block_in_place(|| conn.query(&sql, &[]))?;
        let mut ret = Vec::new();
        for r in result {
            let r = r?;
            let name: String = r.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self, None, None, true).await
    }
}

impl OracleDB {
    fn schema_merge_config<'a, 'b>(&'a self, schema: Option<&'b str>) -> Option<&'a str>
    where
        'b: 'a,
    {
        if schema.is_some() {
            return schema;
        }
        return self.config.schema.as_deref();
    }

    fn not_allow_empty_schema<'a>(schema: Option<&'a str>) -> anyhow::Result<&'a str> {
        match schema {
            Some(x) => return anyhow::Ok(x),
            None => anyhow::bail!("empty schema"),
        }
    }
}

impl OracleDB {
    fn extact_names(&self, sql: &str) -> anyhow::Result<Vec<String>> {
        let conn = self.get_conn()?;
        let rows = conn.query(sql, &[])?;
        let mut ret = Vec::new();
        for row in rows {
            let row = row?;

            let name = row.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for OracleDB {
    async fn list_table(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let schema = Self::not_allow_empty_schema(self.schema_merge_config(schema))?;

        self.list_table_impl(schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn list_view(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let schema = Self::not_allow_empty_schema(self.schema_merge_config(schema))?;

        self.list_view_impl(schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn list_column(
        &self, _db: Option<&str>, schema: Option<&str>, name: &str,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let schema = Self::not_allow_empty_schema(self.schema_merge_config(schema))?;
        let sql = format!(r#"select * from "{}"."{}""#, schema, name);
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;

        let row = conn.query_row(&sql, &[])?;

        anyhow::Ok(
            row.column_info()
                .iter()
                .map(|x| CompletionItem {
                    item: x.name().to_string().into(),
                    desc: Some(x.oracle_type().to_string().into()),
                    kind: CompletionKind::Column,
                })
                .collect(),
        )
    }

    async fn list_function(
        &self, _db: Option<&str>, schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let ret = self.extact_names(
            r#"
            SELECT
                object_name
            FROM
                user_objects
            WHERE
                object_type = 'FUNCTION'
        "#,
        )?;

        if let Some(schema) = self.schema_merge_config(schema) {
            self.extact_names(
                format!(
                    r#"
                    SELECT
                        object_name
                    FROM
                        dba_objects
                    WHERE
                        object_type = 'FUNCTION'
                        AND OWNER IN ('{}') 
                "#,
                    schema
                )
                .as_str(),
            )?;
        }
        let mut ret: Vec<_> = ret
            .into_iter()
            .map(|x| CompletionItem {
                item: x.into(),
                kind: CompletionKind::Function,
                desc: None,
            })
            .collect();

        if include_build_in {
            ret.extend(ORACLE_BUILD_IN_FUNCTIONS.iter().map(|x| CompletionItem {
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
        let schema = Self::not_allow_empty_schema(self.schema_merge_config(schema))?;
        self.list_procedure_impl(schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = Self::not_allow_empty_schema(self.schema_merge_config(schema))?;
        let sql = format!(
            "SELECT INDEX_NAME FROM ALL_INDEXES WHERE OWNER='{}'",
            schema
        );
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        let rows = conn.query(&sql, &[])?;
        let mut ret = Vec::new();
        for row in rows {
            let row = row?;

            let name: String = row.get(0)?;

            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl LangServerCommon for OracleDB {
    async fn list_schema(&self, _db: Option<&str>) -> anyhow::Result<Vec<String>> {
        DBTrait::list_schema(self).await
    }
}

#[async_trait::async_trait]
impl LanguageServer for OracleDB {
    async fn auto_completion(
        &self, query_id: uuid::Uuid, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            &sql,
            query_id,
            line,
            col,
            sqlparser::dialect::MySqlDialect {},
            Arc::new(Clone::clone(self)),
        )
        .await;
        match r {
            Ok(xs) => return Ok(xs),
            Err(err) => {
                log::error!("completion on ast failed,err:{:?}", err);
            }
        };
        if !use_lexer {
            log::warn!("disabled use lexer , we will return empty data.");
            return anyhow::Ok(vec![]);
        }
        return LexerCompletionTrait::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::GenericDialect {},
            sqlparser::tokenizer::Location {
                line: line as u64,
                column: col as u64,
            },
        )
        .await;
    }
}
pub(crate) const ORACLE_TYPES: &'static [&'static str] = &[
    "NUMBER",
    "BINARY_FLOAT",
    "BINARY_DOUBLE",
    "BINARY_INTEGER",
    "CHAR",
    "VARCHAR2",
    "NCHAR",
    "NVARCHAR2",
    "DATE",
    "TIMESTAMP",
    "INTERVAL YEAR TO MONTH",
    "INTERVAL DAY TO SECOND",
    "RAW",
    "LONG RAW",
    "CLOB",
    "NCLOB",
    "BLOB",
    "SDO_GEOMETRY",
    "XMLType",
];

const ORACLE_KEY_WORDS: &'static [&'static str] = &[
    "ACCESS",
    "ELSE",
    "MODIFY",
    "START",
    "ADD",
    "EXCLUSIVE",
    "NOAUDIT",
    "SELECT",
    "ALL",
    "EXISTS",
    "NOCOMPRESS",
    "SESSION",
    "ALTER",
    "FILE",
    "NOT",
    "SET",
    "AND",
    "FLOAT",
    "NOTFOUND",
    "SHARE",
    "ANY",
    "FOR",
    "NOWAIT",
    "SIZE",
    "ARRAYLEN",
    "FROM",
    "NULL",
    "SMALLINT",
    "AS",
    "GRANT",
    "NUMBER",
    "SQLBUF",
    "ASC",
    "GROUP",
    "OF",
    "SUCCESSFUL",
    "AUDIT",
    "HAVING",
    "OFFLINE",
    "SYNONYM",
    "BETWEEN",
    "IDENTIFIED",
    "ON",
    "SYSDATE",
    "BY",
    "IMMEDIATE",
    "ONLINE",
    "TABLE",
    "CHAR",
    "IN",
    "OPTION",
    "THEN",
    "CHECK",
    "INCREMENT",
    "OR",
    "TO",
    "CLUSTER",
    "INDEX",
    "ORDER",
    "TRIGGER",
    "COLUMN",
    "INITIAL",
    "PCTFREE",
    "UID",
    "COMMENT",
    "INSERT",
    "PRIOR",
    "UNION",
    "COMPRESS",
    "INTEGER",
    "PRIVILEGES",
    "UNIQUE",
    "CONNECT",
    "INTERSECT",
    "PUBLIC",
    "UPDATE",
    "CREATE",
    "INTO",
    "RAW",
    "USER",
    "CURRENT",
    "IS",
    "RENAME",
    "VALIDATE",
    "DATE",
    "LEVEL",
    "RESOURCE",
    "VALUES",
    "DECIMAL",
    "LIKE",
    "REVOKE",
    "VARCHAR",
    "DEFAULT",
    "LOCK",
    "ROW",
    "VARCHAR2",
    "DELETE",
    "LONG",
    "ROWID",
    "VIEW",
    "DESC",
    "MAXEXTENTS",
    "ROWLABEL",
    "WHENEVER",
    "DISTINCT",
    "MINUS",
    "ROWNUM",
    "WHERE",
    "DROP",
    "MODE",
    "ROWS",
    "WITH",
    "ADMIN",
    "CURSOR",
    "FOUND",
    "MOUNT",
    "AFTER",
    "CYCLE",
    "FUNCTION",
    "NEXT",
    "ALLOCATE",
    "DATABASE",
    "GO",
    "NEW",
    "ANALYZE",
    "DATAFILE",
    "GOTO",
    "NOARCHIVELOG",
    "ARCHIVE",
    "DBA",
    "GROUPS",
    "NOCACHE",
    "ARCHIVELOG",
    "DEC",
    "INCLUDING",
    "NOCYCLE",
    "AUTHORIZATION",
    "DECLARE",
    "INDICATOR",
    "NOMAXVALUE",
    "AVG",
    "DISABLE",
    "INITRANS",
    "NOMINVALUE",
    "BACKUP",
    "DISMOUNT",
    "INSTANCE",
    "NONE",
    "BEGIN",
    "DOUBLE",
    "INT",
    "NOORDER",
    "BECOME",
    "DUMP",
    "KEY",
    "NORESETLOGS",
    "BEFORE",
    "EACH",
    "LANGUAGE",
    "NORMAL",
    "BLOCK",
    "ENABLE",
    "LAYER",
    "NOSORT",
    "BODY",
    "END",
    "LINK",
    "NUMERIC",
    "CACHE",
    "ESCAPE",
    "LISTS",
    "OFF",
    "CANCEL",
    "EVENTS",
    "LOGFILE",
    "OLD",
    "CASCADE",
    "EXCEPT",
    "MANAGE",
    "ONLY",
    "CHANGE",
    "EXCEPTIONS",
    "MANUAL",
    "OPEN",
    "CHARACTER",
    "EXEC",
    "MAX",
    "OPTIMAL",
    "CHECKPOINT",
    "EXPLAIN",
    "MAXDATAFILES",
    "OWN",
    "CLOSE",
    "EXECUTE",
    "MAXINSTANCES",
    "PACKAGE",
    "COBOL",
    "EXTENT",
    "MAXLOGFILES",
    "PARALLEL",
    "COMMIT",
    "EXTERNALLY",
    "MAXLOGHISTORY",
    "PCTINCREASE",
    "COMPILE",
    "FETCH",
    "MAXLOGMEMBERS",
    "PCTUSED",
    "CONSTRAINT",
    "FLUSH",
    "MAXTRANS",
    "PLAN",
    "CONSTRAINTS",
    "FREELIST",
    "MAXVALUE",
    "PLI",
    "CONTENTS",
    "FREELISTS",
    "MIN",
    "PRECISION",
    "CONTINUE",
    "FORCE",
    "MINEXTENTS",
    "PRIMARY",
    "CONTROLFILE",
    "FOREIGN",
    "MINVALUE",
    "PRIVATE",
    "COUNT",
    "FORTRAN",
    "MODULE",
    "PROCEDURE",
    "PROFILE",
    "SAVEPOINT",
    "SQLSTATE",
    "TRACING",
    "QUOTA",
    "SCHEMA",
    "STATEMENT_ID",
    "TRANSACTION",
    "READ",
    "SCN",
    "STATISTICS",
    "TRIGGERS",
    "REAL",
    "SECTION",
    "STOP",
    "TRUNCATE",
    "RECOVER",
    "SEGMENT",
    "STORAGE",
    "UNDER",
    "REFERENCES",
    "SEQUENCE",
    "SUM",
    "UNLIMITED",
    "REFERENCING",
    "SHARED",
    "SWITCH",
    "UNTIL",
    "RESETLOGS",
    "SNAPSHOT",
    "SYSTEM",
    "USE",
    "RESTRICTED",
    "SOME",
    "TABLES",
    "USING",
    "REUSE",
    "SORT",
    "TABLESPACE",
    "WHEN",
    "ROLE",
    "SQL",
    "TEMPORARY",
    "WRITE",
    "ROLES",
    "SQLCODE",
    "THREAD",
    "WORK",
    "ROLLBACK",
    "SQLERROR",
    "TIME",
    "ABORT",
    "BETWEEN",
    "CRASH",
    "DIGITS",
    "ACCEPT",
    "BINARY_INTEGER",
    "CREATE",
    "DISPOSE",
    "ACCESS",
    "BODY",
    "CURRENT",
    "DISTINCT",
    "ADD",
    "BOOLEAN",
    "CURRVAL",
    "DO",
    "ALL",
    "BY",
    "CURSOR",
    "DROP",
    "ALTER",
    "CASE",
    "DATABASE",
    "ELSE",
    "AND",
    "CHAR",
    "DATA_BASE",
    "ELSIF",
    "ANY",
    "CHAR_BASE",
    "DATE",
    "END",
    "ARRAY",
    "CHECK",
    "DBA",
    "ENTRY",
    "ARRAYLEN",
    "CLOSE",
    "DEBUGOFF",
    "EXCEPTION",
    "AS",
    "CLUSTER",
    "DEBUGON",
    "EXCEPTION_INIT",
    "ASC",
    "CLUSTERS",
    "DECLARE",
    "EXISTS",
    "ASSERT",
    "COLAUTH",
    "DECIMAL",
    "EXIT",
    "ASSIGN",
    "COLUMNS",
    "DEFAULT",
    "FALSE",
    "AT",
    "COMMIT",
    "DEFINITION",
    "FETCH",
    "AUTHORIZATION",
    "COMPRESS",
    "DELAY",
    "FLOAT",
    "AVG",
    "CONNECT",
    "DELETE",
    "FOR",
    "BASE_TABLE",
    "CONSTANT",
    "DESC",
    "FORM",
    "FUNCTION",
    "NEW",
    "RELEASE",
    "SUM",
    "GENERIC",
    "NEXTVAL",
    "REMR",
    "TABAUTH",
    "GOTO",
    "NOCOMPRESS",
    "RENAME",
    "TABLE",
    "GRANT",
    "NOT",
    "RESOURCE",
    "TABLES",
    "GROUP",
    "NULL",
    "RETURN",
    "TASK",
    "HAVING",
    "NUMBER",
    "REVERSE",
    "TERMINATE",
    "IDENTIFIED",
    "NUMBER_BASE",
    "REVOKE",
    "THEN",
    "IF",
    "OF",
    "ROLLBACK",
    "TO",
    "IN",
    "ON",
    "ROWID",
    "TRUE",
    "INDEX",
    "OPEN",
    "ROWLABEL",
    "TYPE",
    "INDEXES",
    "OPTION",
    "ROWNUM",
    "UNION",
    "INDICATOR",
    "OR",
    "ROWTYPE",
    "UNIQUE",
    "INSERT",
    "ORDER",
    "RUN",
    "UPDATE",
    "INTEGER",
    "OTHERS",
    "SAVEPOINT",
    "USE",
    "INTERSECT",
    "OUT",
    "SCHEMA",
    "VALUES",
    "INTO",
    "PACKAGE",
    "SELECT",
    "VARCHAR",
    "IS",
    "PARTITION",
    "SEPARATE",
    "VARCHAR2",
    "LEVEL",
    "PCTFREE",
    "SET",
    "VARIANCE",
    "LIKE",
    "POSITIVE",
    "SIZE",
    "VIEW",
    "LIMITED",
    "PRAGMA",
    "SMALLINT",
    "VIEWS",
    "LOOP",
    "PRIOR",
    "SPACE",
    "WHEN",
    "MAX",
    "PRIVATE",
    "SQL",
    "WHERE",
    "MIN",
    "PROCEDURE",
    "SQLCODE",
    "WHILE",
    "MINUS",
    "PUBLIC",
    "SQLERRM",
    "WITH",
    "MLSLABEL",
    "RAISE",
    "START",
    "WORK",
    "MOD",
    "RANGE",
    "STATEMENT",
    "XOR",
    "MODE",
    "REAL",
    "STDDEV",
    "NATURAL",
    "RECORD",
    "SUBTYPE",
];

/// https://docs.oracle.com/en/database/oracle/oracle-database/19/sqlrf/Single-Row-Functions.html#GUID-B93F789D-B486-49FF-B0CD-0C6181C5D85C
///  all function collect from.
const ORACLE_BUILD_IN_FUNCTIONS: &'static [&'static str] = &[
    // Numeric Functions
    "ABS",
    "ACOS",
    "ASIN",
    "ATAN",
    "ATAN2",
    "BITAND",
    "CEIL",
    "COS",
    "COSH",
    "EXP",
    "FLOOR",
    "LN",
    "LOG",
    "MOD",
    "NANVL",
    "POWER",
    "REMAINDER",
    "ROUND (number)",
    "SIGN",
    "SIN",
    "SINH",
    "SQRT",
    "TAN",
    "TANH",
    "TRUNC",
    "WIDTH_BUCKET",
    //  Character Functions Returning Character Values
    "CHR",
    "CONCAT",
    "INITCAP",
    "LOWER",
    "LPAD",
    "LTRIM",
    "NCHR",
    "NLS_INITCAP",
    "NLS_LOWER",
    "NLS_UPPER",
    "NLSSORT",
    "REGEXP_REPLACE",
    "REGEXP_SUBSTR",
    "REPLACE",
    "RPAD",
    "RTRIM",
    "SOUNDEX",
    "SUBSTR",
    "TRANSLATE",
    "TRANSLATE ... USING",
    "TRIM",
    "UPPER",
    // Character Functions Returning Number Values
    "ASCII",
    "INSTR",
    "LENGTH",
    "REGEXP_COUNT",
    "REGEXP_INSTR",
    // Character Set Functions
    "NLS_CHARSET_DECL_LEN",
    "NLS_CHARSET_ID",
    "NLS_CHARSET_NAME",
    // Collation Functions
    "COLLATION",
    "NLS_COLLATION_ID",
    "NLS_COLLATION_NAME",
    // Datetime Functions
    "ADD_MONTHS",
    "CURRENT_DATE",
    "CURRENT_TIMESTAMP",
    "DBTIMEZONE",
    "EXTRACT",
    "FROM_TZ",
    "LAST_DAY",
    "LOCALTIMESTAMP",
    "MONTHS_BETWEEN",
    "NEW_TIME",
    "NEXT_DAY",
    "NUMTODSINTERVAL",
    "NUMTOYMINTERVAL",
    "ORA_DST_AFFECTED",
    "ORA_DST_CONVERT",
    "ORA_DST_ERROR",
    "ROUND",
    "SESSIONTIMEZONE",
    "SYS_EXTRACT_UTC",
    "SYSDATE",
    "SYSTIMESTAMP",
    "TO_CHAR",
    "TO_DSINTERVAL",
    "TO_TIMESTAMP",
    "TO_TIMESTAMP_TZ",
    "TO_YMINTERVAL",
    "TRUNC",
    "TZ_OFFSET",
    // General Comparison Functions
    "GREATEST",
    "LEAST",
    // Conversion Functions
    "ASCIISTR",
    "BIN_TO_NUM",
    "CAST",
    "CHARTOROWID",
    "COMPOSE",
    "CONVERT",
    "DECOMPOSE",
    "HEXTORAW",
    "NUMTODSINTERVAL",
    "NUMTOYMINTERVAL",
    "RAWTOHEX",
    "RAWTONHEX",
    "ROWIDTOCHAR",
    "ROWIDTONCHAR",
    "SCN_TO_TIMESTAMP",
    "TIMESTAMP_TO_SCN",
    "TO_BINARY_DOUBLE",
    "TO_BINARY_FLOAT",
    "TO_BLOB",
    "TO_CHAR",
    "TO_CLOB",
    "TO_DATE",
    "TO_DSINTERVAL",
    "TO_LOB",
    "TO_MULTI_BYTE",
    "TO_NCHAR",
    "TO_NCLOB",
    "TO_NUMBER",
    "TO_SINGLE_BYTE",
    "TO_TIMESTAMP",
    "TO_TIMESTAMP_TZ",
    "TO_YMINTERVAL",
    "TREAT",
    "UNISTR",
    "VALIDATE_CONVERSION",
    // Large Object Functions
    "BFILENAME",
    "EMPTY_BLOB",
    "EMPTY_CLOB",
    // Collection Functions
    "CARDINALITY",
    "COLLECT",
    "POWERMULTISET",
    "POWERMULTISET_BY_CARDINALITY",
    "SET",
    // Hierarchical Functions
    "SYS_CONNECT_BY_PATH",
    // Data Mining Functions
    "CLUSTER_DETAILS",
    "CLUSTER_DISTANCE",
    "CLUSTER_ID",
    "CLUSTER_PROBABILITY",
    "CLUSTER_SET",
    "FEATURE_COMPARE",
    "FEATURE_DETAILS",
    "FEATURE_ID",
    "FEATURE_SET",
    "FEATURE_VALUE",
    "ORA_DM_PARTITION_NAME",
    "PREDICTION",
    "PREDICTION_BOUNDS",
    "PREDICTION_COST",
    "PREDICTION_DETAILS",
    "PREDICTION_PROBABILITY",
    "PREDICTION_SET",
    // The SQL XML functions are:
    "DEPTH",
    "EXISTSNODE",
    "EXTRACT (XML)",
    "EXTRACTVALUE",
    "PATH",
    "SYS_DBURIGEN",
    "SYS_XMLAGG",
    "SYS_XMLGEN",
    "XMLAGG",
    "XMLCAST",
    "XMLCDATA",
    "XMLCOLATTVAL",
    "XMLCOMMENT",
    "XMLCONCAT",
    "XMLDIFF",
    "XMLELEMENT",
    "XMLEXISTS",
    "XMLFOREST",
    "XMLISVALID",
    "XMLPARSE",
    "XMLPATCH",
    "XMLPI",
    "XMLQUERY",
    "XMLROOT",
    "XMLSEQUENCE",
    "XMLSERIALIZE",
    "XMLTABLE",
    "XMLTRANSFORM",
    // JSON Functions
    "JSON_QUERY",
    "JSON_TABLE",
    "JSON_VALUE",
    // The following SQL/JSON functions allow you to generate JSON data:
    "JSON_ARRAY",
    "JSON_ARRAYAGG",
    "JSON_OBJECT",
    "JSON_OBJECTAGG",
    // The following Oracle SQL function creates a JSON data guide:
    "JSON_DATAGUIDE",
    // Encoding and Decoding Functions
    "DECODE",
    "DUMP",
    "ORA_HASH",
    "STANDARD_HASH",
    "VSIZE",
    // NULL-Related Functions
    "COALESCE",
    "LNNVL",
    "NANVL",
    "NULLIF",
    "NVL",
    "NVL2",
    // Environment and Identifier Functions
    "CON_DBID_TO_ID",
    "CON_GUID_TO_ID",
    "CON_NAME_TO_ID",
    "CON_UID_TO_ID",
    "ORA_INVOKING_USER",
    "ORA_INVOKING_USERID",
    "SYS_CONTEXT",
    "SYS_GUID",
    "SYS_TYPEID",
    "UID",
    "USER",
    "USERENV",
    //
    "APPROX_COUNT",
    "APPROX_COUNT_DISTINCT",
    "APPROX_COUNT_DISTINCT_AGG",
    "APPROX_COUNT_DISTINCT_DETAIL",
    "APPROX_MEDIAN",
    "APPROX_PERCENTILE",
    "APPROX_PERCENTILE_AGG",
    "APPROX_PERCENTILE_DETAIL",
    "APPROX_RANK",
    "APPROX_SUM",
    "AVG",
    "COLLECT",
    "CORR",
    "CORR_S",
    "CORR_K",
    "COUNT",
    "COVAR_POP",
    "COVAR_SAMP",
    "CUME_DIST",
    "DENSE_RANK",
    "FIRST",
    "GROUP_ID",
    "GROUPING",
    "GROUPING_ID",
    "JSON_ARRAYAGG",
    "JSON_OBJECTAGG",
    "LAST",
    "LISTAGG",
    "MAX",
    "MEDIAN",
    "MIN",
    "PERCENT_RANK",
    "PERCENTILE_CONT",
    "PERCENTILE_DISC",
    "RANK",
    "REGR_ (Linear Regression) Functions",
    "STATS_BINOMIAL_TEST",
    "STATS_CROSSTAB",
    "STATS_F_TEST",
    "STATS_KS_TEST",
    "STATS_MODE",
    "STATS_MW_TEST",
    "STATS_ONE_WAY_ANOVA",
    "STATS_T_TEST_*",
    "STATS_WSR_TEST",
    "STDDEV",
    "STDDEV_POP",
    "STDDEV_SAMP",
    "SUM",
    "SYS_OP_ZONE_ID",
    "SYS_XMLAGG",
    "TO_APPROX_COUNT_DISTINCT",
    "TO_APPROX_PERCENTILE",
    "VAR_POP",
    "VAR_SAMP",
    "VARIANCE",
    "XMLAGG",
    //  Analytic Functions
    "AVG",
    "CLUSTER_DETAILS",
    "CLUSTER_DISTANCE",
    "CLUSTER_ID",
    "CLUSTER_PROBABILITY",
    "CLUSTER_SET",
    "CORR",
    "COUNT",
    "COVAR_POP",
    "COVAR_SAMP",
    "CUME_DIST",
    "DENSE_RANK",
    "FEATURE_DETAILS",
    "FEATURE_ID",
    "FEATURE_SET",
    "FEATURE_VALUE",
    "FIRST",
    "FIRST_VALUE",
    "LAG",
    "LAST",
    "LAST_VALUE",
    "LEAD",
    "LISTAGG",
    "MAX",
    "MIN",
    "NTH_VALUE",
    "NTILE",
    "PERCENT_RANK",
    "PERCENTILE_CONT",
    "PERCENTILE_DISC",
    "PREDICTION",
    "PREDICTION_COST",
    "PREDICTION_DETAILS",
    "PREDICTION_PROBABILITY",
    "PREDICTION_SET",
    "RANK",
    "RATIO_TO_REPORT",
    "ROW_NUMBER",
    "STDDEV",
    "STDDEV_POP",
    "STDDEV_SAMP",
    "SUM",
    "VAR_POP",
    "VAR_SAMP",
    "VARIANCE",
    "REGR_SLOPE",
    "REGR_INTERCEPT",
    "REGR_COUNT",
    "REGR_R2",
    "REGR_AVGX",
    "REGR_AVGY",
    "REGR_SXX",
    "REGR_SYY",
    "REGR_SXY",
    // Object Reference Functions
    "DEREF",
    "MAKE_REF",
    "REF",
    "REFTOHEX",
    "VALUE",
    // Model Functions
    "CV",
    "ITERATION_NUMBER",
    "PRESENTNNV",
    "PRESENTV",
    "PREVIOUS",
    // OLAP Functions
    "CUBE_TABLE",
    // Data Cartridge Functions
    "DATAOBJ_TO_MAT_PARTITION",
    "DATAOBJ_TO_PARTITION",
];
