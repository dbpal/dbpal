use futures_util::StreamExt;
use serde_json::Value;

use crate::{
    couchbase::CouchBaseDB,
    db::{CatalogTrait, DBTrait},
    language_server::{
        CompletionItem, CompletionKind, GetKeyWords, LangServerCommon, LanguageServer,
        LexerCompletionTrait,
    },
    sqlparser_completion::TempTableColumn,
};

#[async_trait::async_trait]

impl LanguageServer for CouchBaseDB {
    async fn auto_completion(
        &self,
        _query_id: Option<String>,
        sql: &str,
        line: u32,
        col: u32,
        _use_lexer: bool,
    ) -> anyhow::Result<crate::language_server::CompletionList> {
        let dot = LangServerCommon::try_completion_dot(self, sql, line, col).await;
        match dot {
            Ok(r) => return anyhow::Ok(r),
            Err(_) => {}
        }
        LexerCompletionTrait::do_auto_completion(self, sql, line, col).await
    }

    async fn ai_text_completion(
        &self,
        sql: &str,
        line: u32,
        col: u32,
    ) -> anyhow::Result<crate::language_server::CompletionList> {
        LexerCompletionTrait::ai_text_completion(self, sql, line, col).await
    }

    fn is_dangerous_sql(&self, _sql: &str) -> anyhow::Result<bool> {
        anyhow::Ok(false)
    }

    fn extract_table_select_selection(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_selection(self, sql)
    }

    fn extract_table_select_sort(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_sort(self, sql)
    }
}

#[async_trait::async_trait]

impl LexerCompletionTrait for CouchBaseDB {
    async fn get_types(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
}

#[async_trait::async_trait]

impl GetKeyWords for CouchBaseDB {
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(COUCHBASE_KEYWORDS.iter().map(|x| x.to_string()).collect())
    }
}

#[async_trait::async_trait]
impl LangServerCommon for CouchBaseDB {
    async fn list_index(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }

    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::GenericDialect {})
    }

    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let s = self.get_schema(schema);
        self.list_table_impl(&s).await
    }

    async fn list_view(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }

    async fn list_function(
        &self,
        _schema: Option<&str>,
        include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let mut ret = vec![];
        // TODO user define function.
        if include_build_in {
            ret.extend(
                COUCHBASE_ANALYTICS_FUNCTIONS
                    .iter()
                    .map(|x| CompletionItem {
                        item: (*x).into(),
                        kind: CompletionKind::Function,
                        desc: None,
                    }),
            );
        }
        anyhow::Ok(ret)
    }

    async fn list_column(
        &self,
        schema: Option<&str>,
        name: &str,
        _is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let schema = self.get_schema(schema);
        let bucket = self.get_bucket()?;
        let s = bucket.scope(schema);
        let sql = format!("select * from `{name }`");
        let mut resp = s.query(sql, None).await?;
        let mut rows = resp.rows::<'_, serde_json::Map<String, Value>>();
        while let Some(row) = rows.next().await {
            let row = row?;
            return anyhow::Ok(
                row.keys()
                    .map(|x| TempTableColumn {
                        name: x.clone(),
                        data_type: None,
                    })
                    .collect(),
            );
        }
        anyhow::Ok(vec![])
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        DBTrait::list_schema(self).await
    }
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        CatalogTrait::list_database(self).await
    }
}

impl CouchBaseDB {
    fn get_schema(&self, schema: Option<&str>) -> String {
        match schema {
            Some(x) => x.to_owned(),
            None => self.config.scope_str().to_owned(),
        }
    }
}

/// Couchbase SQL++ (N1QL) 保留关键字列表
pub const COUCHBASE_KEYWORDS: &[&str] = &[
    "_INDEX_CONDITION",
    "_INDEX_KEY",
    "ADVISE",
    "ALL",
    "ALTER",
    "ANALYZE",
    "AND",
    "ANY",
    "ARRAY",
    "AS",
    "ASC",
    "AT",
    "BEGIN",
    "BETWEEN",
    "BINARY",
    "BOOLEAN",
    "BREAK",
    "BUCKET",
    "BUILD",
    "BY",
    "CACHE",
    "CALL",
    "CASE",
    "CAST",
    "CLUSTER",
    "COLLATE",
    "COLLECTION",
    "COMMIT",
    "COMMITTED",
    "CONNECT",
    "CONTINUE",
    "CORRELATED",
    "COVER",
    "CREATE",
    "CURRENT",
    "CYCLE",
    "DATABASE",
    "DATASET",
    "DATASTORE",
    "DECLARE",
    "DECREMENT",
    "DEFAULT",
    "DELETE",
    "DERIVED",
    "DESC",
    "DESCRIBE",
    "DISTINCT",
    "DO",
    "DROP",
    "EACH",
    "ELEMENT",
    "ELSE",
    "END",
    "ESCAPE",
    "EVERY",
    "EXCEPT",
    "EXCLUDE",
    "EXECUTE",
    "EXISTS",
    "EXPLAIN",
    "FALSE",
    "FETCH",
    "FILTER",
    "FIRST",
    "FLATTEN",
    "FLATTEN_KEYS",
    "FLUSH",
    "FOLLOWING",
    "FOR",
    "FORCE",
    "FROM",
    "FTS",
    "FUNCTION",
    "GOLANG",
    "GRANT",
    "GROUP",
    "GROUPS",
    "GSI",
    "HASH",
    "HAVING",
    "IF",
    "IGNORE",
    "ILIKE",
    "IN",
    "INCLUDE",
    "INCREMENT",
    "INDEX",
    "INFER",
    "INLINE",
    "INNER",
    "INSERT",
    "INTERSECT",
    "INTO",
    "IS",
    "ISOLATION",
    "JAVASCRIPT",
    "JOIN",
    "KEY",
    "KEYS",
    "KEYSPACE",
    "KNOWN",
    "LANGUAGE",
    "LAST",
    "LATERAL",
    "LEFT",
    "LET",
    "LETTING",
    "LEVEL",
    "LIKE",
    "LIMIT",
    "LSM",
    "MAP",
    "MAPPING",
    "MATCHED",
    "MATERIALIZED",
    "MAXVALUE",
    "MERGE",
    "MINUS",
    "MINVALUE",
    "MISSING",
    "NAMESPACE",
    "NAMESPACE_ID",
    "NEST",
    "NEXT",
    "NEXTVAL",
    "NL",
    "NO",
    "NOT",
    "NOT_A_TOKEN",
    "NTH_VALUE",
    "NULL",
    "NULLS",
    "NUMBER",
    "OBJECT",
    "OFFSET",
    "ON",
    "OPTION",
    "OPTIONS",
    "OR",
    "ORDER",
    "OTHERS",
    "OUTER",
    "OVER",
    "PARSE",
    "PARTITION",
    "PASSWORD",
    "PATH",
    "POOL",
    "PRECEDING",
    "PREPARE",
    "PREV",
    "PREVIOUS",
    "PREVVAL",
    "PRIMARY",
    "PRIVATE",
    "PRIVILEGE",
    "PROBE",
    "PROCEDURE",
    "PUBLIC",
    "RANGE",
    "RAW",
    "READ",
    "REALM",
    "RECURSIVE",
    "REDUCE",
    "RENAME",
    "REPLACE",
    "RESPECT",
    "RESTART",
    "RESTRICT",
    "RETURN",
    "RETURNING",
    "REVOKE",
    "RIGHT",
    "ROLE",
    "ROLES",
    "ROLLBACK",
    "ROW",
    "ROWS",
    "SATISFIES",
    "SAVEPOINT",
    "SCHEMA",
    "SCOPE",
    "SELECT",
    "SELF",
    "SEQUENCE",
    "SEMI",
    "SET",
    "SHOW",
    "SOME",
    "START",
    "STATISTICS",
    "STRING",
    "SYSTEM",
    "THEN",
    "TIES",
    "TO",
    "TRAN",
    "TRANSACTION",
    "TRIGGER",
    "TRUE",
    "TRUNCATE",
    "UNBOUNDED",
    "UNDER",
    "UNION",
    "UNIQUE",
    "UNKNOWN",
    "UNNEST",
    "UNSET",
    "UPDATE",
    "UPSERT",
    "USE",
    "USER",
    "USERS",
    "USING",
    "VALIDATE",
    "VALUE",
    "VALUED",
    "VALUES",
    "VECTOR",
    "VIA",
    "VIEW",
    "WHEN",
    "WHERE",
    "WHILE",
    "WINDOW",
    "WITH",
    "WITHIN",
    "WORK",
    "XOR",
];

// 帮我把这里面所有的内置函数整理成rust数组https://docs.couchbase.com/server/current/analytics/8_builtin.html
/// Couchbase Analytics (SQL++) 内置函数列表
pub const COUCHBASE_ANALYTICS_FUNCTIONS: &[&str] = &[
    // --- 数值函数 (Numeric Functions) ---
    "ABS",
    "ACOS",
    "ASIN",
    "ATAN",
    "ATAN2",
    "CEIL",
    "COS",
    "COT",
    "DEGREES",
    "EXP",
    "FLOOR",
    "LN",
    "LOG",
    "NANVL",
    "PI",
    "POWER",
    "RADIANS",
    "RANDOM",
    "ROUND",
    "SIGN",
    "SIN",
    "SQRT",
    "TAN",
    "TRUNC",
    // --- 字符串函数 (String Functions) ---
    "CONCAT",
    "CONTAINS",
    "DECODE_JSON",
    "ENCODE_JSON",
    "ENDS_WITH",
    "INITCAP",
    "LENGTH",
    "LOWER",
    "LTRIM",
    "POSITION",
    "REGEXP_CONTAINS",
    "REGEXP_LIKE",
    "REGEXP_POSITION",
    "REGEXP_REPLACE",
    "REPEAT",
    "REPLACE",
    "REVERSE",
    "RTRIM",
    "SPLIT",
    "STARTS_WITH",
    "SUBSTR",
    "TITLE",
    "TRIM",
    "UPPER",
    // --- 日期/时间函数 (Date/Time Functions) ---
    "CURRENT_DATE",
    "CURRENT_DATETIME",
    "CURRENT_TIME",
    "DATE_ADD",
    "DATE_DIFF",
    "DATE_PART",
    "DATE_TRUNC",
    "DURATION_TO_STR",
    "STR_TO_DATE",
    "STR_TO_DATETIME",
    "STR_TO_DURATION",
    "STR_TO_MILLIS",
    "STR_TO_TIME",
    "STR_TO_TZ",
    "MILLIS_TO_STR",
    "MILLIS_TO_UTC",
    "MILLIS_TO_TZ",
    // --- 聚合函数 (Aggregate Functions) ---
    "AVG",
    "COUNT",
    "MAX",
    "MIN",
    "SUM",
    "STDDEV",
    "STDDEV_POP",
    "STDDEV_SAMP",
    "VAR_POP",
    "VAR_SAMP",
    "VARIANCE",
    "ARRAY_AGG",
    // --- 数组/对象函数 (Array & Object Functions) ---
    "ARRAY_APPEND",
    "ARRAY_CONCAT",
    "ARRAY_CONTAINS",
    "ARRAY_COUNT",
    "ARRAY_DISTINCT",
    "ARRAY_IFNULL",
    "ARRAY_INSERT",
    "ARRAY_MAX",
    "ARRAY_MIN",
    "ARRAY_POSITION",
    "ARRAY_PREPEND",
    "ARRAY_PUT",
    "ARRAY_RANGE",
    "ARRAY_REMOVE",
    "ARRAY_REPEAT",
    "ARRAY_REPLACE",
    "ARRAY_REVERSE",
    "ARRAY_SLICE",
    "ARRAY_SORT",
    "ARRAY_SUM",
    "OBJECT_ADD",
    "OBJECT_CONCAT",
    "OBJECT_NAMES",
    "OBJECT_PUT",
    "OBJECT_REMOVE",
    "OBJECT_RENAME",
    "OBJECT_REPLACE",
    "OBJECT_VALUES",
    "OBJECT_INNER_VALUES",
    // --- 类型转换/检查函数 (Type Functions) ---
    "TO_ARRAY",
    "TO_ATOMIC",
    "TO_BOOLEAN",
    "TO_BIGINT",
    "TO_DOUBLE",
    "TO_FLOAT",
    "TO_INTEGER",
    "TO_NUMBER",
    "TO_OBJECT",
    "TO_STRING",
    "IS_ARRAY",
    "IS_ATOM",
    "IS_BOOLEAN",
    "IS_BINARY",
    "IS_NUMBER",
    "IS_OBJECT",
    "IS_STRING",
    "TYPE",
    // --- 条件/比较函数 (Comparison & Conditional) ---
    "GREATEST",
    "LEAST",
    "IFNULL",
    "IFMISSING",
    "IFMISSINGORNULL",
    "MISSINGIF",
    "NULLIF",
    // --- 空间函数 (Spatial Functions) ---
    "ST_AREA",
    "ST_DISTANCE",
    "ST_LENGTH",
    "ST_INTERSECTS",
    "ST_CONTAINS",
    "ST_WITHIN",
    "ST_X",
    "ST_Y",
    "ST_Z",
    "ST_MAKE_POINT",
    "ST_GEOM_FROM_TEXT",
    // --- 系统/元数据函数 (System Functions) ---
    "META",
    "UUID",
    "VERSION",
];
