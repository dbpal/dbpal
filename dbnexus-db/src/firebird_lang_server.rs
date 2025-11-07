use rsfbclient::{Queryable, Row};

use crate::db::SchemaTrait;
use crate::firebird::FireBirdDB;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, CompletionList, GetKeyWords,
    LangServerCommon, LanguageServer, LexerCompletionTrait, try_parser_dot_lexer,
};
use crate::sqlparser_completion::TempTableColumn;

#[async_trait::async_trait]
impl GetKeyWords for FireBirdDB {
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        let mut lock = self.key_words_completion.lock().await;
        if !lock.is_empty() {
            return Ok(lock.clone());
        }
        *lock = FIREBIRD_KEYWORDS.iter().map(|x| x.to_string()).collect();
        anyhow::Ok(lock.clone())
    }
}

#[async_trait::async_trait]
impl LanguageServer for FireBirdDB {
    async fn ai_text_completion(
        &self, text: &str, line: u32, col: u32,
    ) -> anyhow::Result<CompletionList> {
        LexerCompletionTrait::ai_text_completion(self, text, line, col).await
    }
    fn extract_table_select_selection(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_selection(self, sql)
    }
    fn extract_table_select_sort(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_sort(self, sql)
    }

    async fn auto_completion(
        &self, _query_id: Option<u32>, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<CompletionList> {
        try_parser_dot_lexer(self, sql, line, col, use_lexer)
            .await
    }
    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        LangServerCommon::is_dangerous_sql_by_parser(self, sql)
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for FireBirdDB {
    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        let mut conn = self.get_conn()?;
        let row: Option<Row> = conn.query_first(sql, ())?;
        if let Some(row) = row {
            let ret: Vec<_> = row
                .cols
                .iter()
                .map(|x| TempTableColumn {
                    name: x.name.clone(),
                    data_type: None,
                })
                .collect();
            anyhow::Ok(ret)
        } else {
            log::error!("no results");
            anyhow::Ok(vec![])
        }
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for FireBirdDB {
    const TEXTS: &'static [&'static str] = &["true", "false"];
    const TYPES: &'static [&'static str] = FIREBIRD_TYPES;
}

#[cfg(test)]

mod tests {
    use std::path::Path;

    use crate::db::QuerySql;
    use crate::enable_test_log;
    use crate::firebird::make_test_object;
    use crate::language_server::completion_test::test_file_auto_completion;

    // partition test failed , sqlparser can't parse a valid partition statement.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_auto_completion() {
        let db = make_test_object();
        enable_test_log();
        let query_id = db.open_connection().await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
                    CREATE TABLE EMPLOYEE (
                        EMPLOYEEID INTEGER NOT NULL PRIMARY KEY, -- 雇员编号，主键
                        FIRSTNAME VARCHAR(50) NOT NULL, -- 雇员名字
                        LASTNAME VARCHAR(50) NOT NULL, -- 雇员姓氏
                        GENDER CHAR(1) CHECK (GENDER IN ('M', 'F')), -- 性别，M 表示男性，F 表示女性
                        BIRTHDATE DATE, -- 出生日期
                        HIREDATE DATE -- 入职日期
                    );
                "#,
            )
            .await
            .unwrap();

        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("firebird_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        test_file_auto_completion(&content, &db, false).await;
    }
}

const FIREBIRD_TYPES: &[&str] = &[
    "INT64",
    "CHAR",
    "TIMESTAMP",
    "DECIMAL",
    "FLOAT",
    "BLOB",
    "INTEGER",
    "SMALLINT",
    "NUMERIC",
    "VARCHAR",
    "DOUBLE",
    "SMALLINT",
];

#[async_trait::async_trait]
impl LangServerCommon for FireBirdDB {
    async fn list_index(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let sql = r#"
            SELECT RDB$INDEX_NAME AS IndexName
                FROM RDB$INDICES
        "#;

        let mut conn = self.get_conn()?;
        let rows: Vec<(String,)> = conn.query(sql, ())?;
        anyhow::Ok(rows.into_iter().map(|x| x.0.trim().to_owned()).collect())
    }
    async fn list_column(
        &self, _schema: Option<&str>, name: &str, is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        /*
                    -- 直接写
        SELECT ABS(-1)  FROM RDB$DATABASE;   -- 返回 1
        SELECT SUBSTRING('abc' FROM 2 FOR 2) FROM RDB$DATABASE;
        */
        if let Some(is_call) = is_call {
            let sql = format!("SELECT {}({}) FROM RDB$DATABASE", name, is_call.concat());
            self.get_query_columns(&sql).await
        } else {
            SchemaTrait::show_table_columns(self, name).await.map(|x| {
                x.into_iter()
                    .map(|x| TempTableColumn {
                        name: x.field.clone(),
                        data_type: Some(x.data_type.clone()),
                    })
                    .collect()
            })
        }
    }

    async fn list_function(
        &self, _schema: Option<&str>, _include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let sql = r#"
            SELECT 
            RDB$FUNCTION_NAME AS FunctionName 
            FROM 
            RDB$FUNCTIONS
        "#;
        let mut conn = self.get_conn()?;
        let rows: Vec<(String,)> = conn.query(sql, ())?;
        let mut result: Vec<_> = rows
            .into_iter()
            .map(|x| CompletionItem {
                item: x.0.trim().to_owned().into(),
                kind: CompletionKind::Function,
                desc: None,
            })
            .collect();
        result.extend(FB_BUILTIN_FUNCTIONS.iter().map(|x| CompletionItem {
            item: (*x).into(),
            kind: CompletionKind::Function,
            desc: None,
        }));
        anyhow::Ok(result)
    }
    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::GenericDialect {})
    }
    async fn list_table(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let sql = r#"
            SELECT 
                RDB$RELATION_NAME AS TABLE_NAME
            FROM 
                RDB$RELATIONS
            WHERE 
                RDB$VIEW_BLR IS NULL  -- 排除视图
        "#;
        let mut conn = self.get_conn()?;
        let rows: Vec<(String,)> = conn.query(sql, ())?;
        anyhow::Ok(rows.into_iter().map(|x| x.0.trim().to_owned()).collect())
    }
    async fn list_view(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let sql = r#"
        SELECT RDB$RELATION_NAME
            FROM RDB$RELATIONS
            WHERE RDB$VIEW_BLR IS NOT NULL;
        "#;
        let mut conn = self.get_conn()?;
        let rows: Vec<(String,)> = conn.query(sql, ())?;
        anyhow::Ok(rows.into_iter().map(|x| x.0.trim().to_owned()).collect())
    }
}
/// 给我firebird的完整的关键字列表rust数组 并且去掉了内置函数
/// Firebird 4.0 **保留关键字**（不含内置函数）
pub const FIREBIRD_KEYWORDS: &[&str] = &[
    "ACTIVE",
    "ADD",
    "ADMIN",
    "AFTER",
    "ALL",
    "ALTER",
    "AND",
    "ANY",
    "AS",
    "ASC",
    "ASCENDING",
    "AT",
    "AUTO",
    "AUTONOMOUS",
    "AVG",
    "BACKUP",
    "BEFORE",
    "BEGIN",
    "BETWEEN",
    "BIGINT",
    "BIT_LENGTH",
    "BLOB",
    "BLOCK",
    "BOTH",
    "BREAK",
    "BY",
    "CALLER",
    "CASCADE",
    "CASE",
    "CAST",
    "CHAR",
    "CHARACTER",
    "CHECK",
    "CLOSE",
    "COALESCE",
    "COLLATE",
    "COLLATION",
    "COLUMN",
    "COMMENT",
    "COMMIT",
    "COMMITTED",
    "COMMON",
    "COMPUTED",
    "CONDITIONAL",
    "CONNECT",
    "CONSTRAINT",
    "CONTAINING",
    "COUNT",
    "CREATE",
    "CROSS",
    "CSTRING",
    "CURRENT",
    "CURRENT_CONNECTION",
    "CURRENT_DATE",
    "CURRENT_ROLE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "CURRENT_TRANSACTION",
    "CURRENT_USER",
    "CURSOR",
    "DATA",
    "DATE",
    "DAY",
    "DEC",
    "DECIMAL",
    "DECLARE",
    "DEFAULT",
    "DELETE",
    "DELETING",
    "DESC",
    "DESCENDING",
    "DESCRIPTOR",
    "DIFFERENCE",
    "DISCONNECT",
    "DISTINCT",
    "DO",
    "DOMAIN",
    "DOUBLE",
    "DROP",
    "ELSE",
    "END",
    "ENTRY_POINT",
    "ESCAPE",
    "EXCEPTION",
    "EXECUTE",
    "EXISTS",
    "EXTERNAL",
    "EXTRACT",
    "FETCH",
    "FILE",
    "FILTER",
    "FIRST",
    "FLOAT",
    "FOR",
    "FOREIGN",
    "FREE_IT",
    "FROM",
    "FULL",
    "FUNCTION",
    "GDSCODE",
    "GEN_ID",
    "GENERATED",
    "GENERATOR",
    "GLOBAL",
    "GRANT",
    "GRANTED",
    "GROUP",
    "HASH",
    "HAVING",
    "HOUR",
    "IF",
    "IGNORE",
    "IN",
    "INACTIVE",
    "INDEX",
    "INNER",
    "INPUT_TYPE",
    "INSENSITIVE",
    "INSERT",
    "INSERTING",
    "INT",
    "INTEGER",
    "INTO",
    "IS",
    "ISOLATION",
    "JOIN",
    "KEY",
    "LAG",
    "LAST",
    "LEADING",
    "LEAVE",
    "LEFT",
    "LENGTH",
    "LEVEL",
    "LIKE",
    "LIMBO",
    "LIST",
    "LOCAL",
    "LOCALTIME",
    "LOCALTIMESTAMP",
    "LOCK",
    "LOG",
    "LONG",
    "LOWER",
    "LPAD",
    "MANUAL",
    "MAPPING",
    "MATCH",
    "MATCHED",
    "MAX",
    "MAXIMUM",
    "MAXIMUM_SEGMENT",
    "MERGE",
    "MIN",
    "MINUTE",
    "MODULE_NAME",
    "MONTH",
    "NAMES",
    "NATIONAL",
    "NATURAL",
    "NCHAR",
    "NEXT",
    "NO",
    "NOT",
    "NULL",
    "NULLIF",
    "NULLS",
    "NUMERIC",
    "OCTET_LENGTH",
    "OF",
    "ON",
    "ONLY",
    "OPEN",
    "OPTION",
    "OR",
    "ORDER",
    "OS_NAME",
    "OUTER",
    "OUTPUT_TYPE",
    "OVER",
    "OVERFLOW",
    "OVERLAY",
    "PAD",
    "PAGE",
    "PAGE_SIZE",
    "PAGES",
    "PARAMETER",
    "PASSWORD",
    "PI",
    "PLACING",
    "PLAN",
    "POSITION",
    "POST_EVENT",
    "POWER",
    "PRECISION",
    "PRESERVE",
    "PRIMARY",
    "PRIVILEGES",
    "PROCEDURE",
    "PROTECTED",
    "RDB$DB_KEY",
    "READ",
    "REAL",
    "RECORD_VERSION",
    "RECREATE",
    "RECURSIVE",
    "REFERENCES",
    "RELEASE",
    "REPLACE",
    "REQUESTS",
    "RESERV",
    "RESERVING",
    "RESTART",
    "RESTRICT",
    "RETAIN",
    "RETURNING",
    "RETURNING_VALUES",
    "RETURNS",
    "REVERSE",
    "REVOKE",
    "RIGHT",
    "ROLE",
    "ROLLBACK",
    "ROW",
    "ROW_COUNT",
    "ROWS",
    "RPAD",
    "SAVEPOINT",
    "SCALAR_ARRAY",
    "SCHEMA",
    "SECOND",
    "SEGMENT",
    "SELECT",
    "SENSITIVE",
    "SET",
    "SHADOW",
    "SHARED",
    "SINGULAR",
    "SIZE",
    "SKIP",
    "SMALLINT",
    "SNAPSHOT",
    "SOME",
    "SORT",
    "SOURCE",
    "SPACE",
    "SQL",
    "SQLCODE",
    "SQLSTATE",
    "START",
    "STARTING",
    "STATEMENT",
    "STATIC",
    "STATISTICS",
    "SUB_TYPE",
    "SUM",
    "SUSPEND",
    "TABLE",
    "TEMPORARY",
    "THEN",
    "TIME",
    "TIMESTAMP",
    "TO",
    "TRAILING",
    "TRANSACTION",
    "TRIGGER",
    "TRIM",
    "TYPE",
    "UNCOMMITTED",
    "UNION",
    "UNIQUE",
    "UPDATE",
    "UPDATING",
    "UPPER",
    "USER",
    "USING",
    "UUID_TO_CHAR",
    "VALUE",
    "VALUES",
    "VARCHAR",
    "VARIABLE",
    "VARYING",
    "VIEW",
    "WAIT",
    "WEEK",
    "WEEKDAY",
    "WHEN",
    "WHERE",
    "WHILE",
    "WITH",
    "WORK",
    "WRITE",
    "YEAR",
    "YEARDAY",
];

/*
    firebird所有的内置函数 ABS 之类的给我一个rust数组 我直接处理
*/
/// Firebird 内置函数（标量）名称表，按字母序
pub const FB_BUILTIN_FUNCTIONS: &[&str] = &[
    "ABS",
    "ACOS",
    "ASCII_CHAR",
    "ASCII_VAL",
    "ASIN",
    "ATAN",
    "ATAN2",
    "BIN_AND",
    "BIN_OR",
    "BIN_XOR",
    "BIT_LENGTH",
    "CAST",
    "CEIL",
    "CEILING",
    "CHAR_LENGTH",
    "CHAR_TO_UUID",
    "COS",
    "COT",
    "DATEADD",
    "DATEDIFF",
    "DAY",
    "EXP",
    "FLOOR",
    "GEN_UUID",
    "HASH",
    "HOUR",
    "LEFT",
    "LN",
    "LOG",
    "LOWER",
    "LPAD",
    "MILLISECOND",
    "MINUTE",
    "MONTH",
    "OCTET_LENGTH",
    "PI",
    "POWER",
    "RAND",
    "REPLACE",
    "REVERSE",
    "RIGHT",
    "ROUND",
    "RPAD",
    "SECOND",
    "SIGN",
    "SIN",
    "SQRT",
    "SUBSTRING",
    "TAN",
    "TRIM",
    "UPPER",
    "UUID_TO_CHAR",
    "WEEK",
    "WEEKDAY",
    "YEAR",
];
