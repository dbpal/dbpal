use std::sync::Arc;

use rsfbclient::Queryable;

use crate::db::SchemaTrait;
use crate::firebird::FireBirdDB;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, LangServerCommon, LanguageServer,
    LexerCompletionTrait, try_completion_on_syntax,
};

#[async_trait::async_trait]
impl LanguageServer for FireBirdDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            &sql,
            line,
            col,
            sqlparser::dialect::GenericDialect {},
            Arc::new(Clone::clone(self)),
        )
        .await;

        match r {
            Ok(xs) => return Ok(xs),
            Err(err) => {
                log::error!("completion on ast failed,err:{}", err);
            }
        };
        if !use_lexer {
            log::warn!("disabled use lexer , we can return empty data.");
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

#[async_trait::async_trait]
impl AstCompletionHelper for FireBirdDB {
    async fn list_table(
        &self, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
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

    async fn list_view(
        &self, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let sql = r#"
          SELECT RDB$RELATION_NAME
            FROM RDB$RELATIONS
            WHERE RDB$VIEW_BLR IS NOT NULL;
   "#;
        let mut conn = self.get_conn()?;
        let rows: Vec<(String,)> = conn.query(sql, ())?;
        anyhow::Ok(rows.into_iter().map(|x| x.0.trim().to_owned()).collect())
    }

    async fn list_column(
        &self, _db: Option<&str>, _schema: Option<&str>, name: &str, _is_call: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        SchemaTrait::show_table_columns(self, name).await.map(|x| {
            x.into_iter()
                .map(|x| CompletionItem {
                    item: x.field.clone().into(),
                    kind: CompletionKind::Column,
                    desc: None,
                })
                .collect()
        })
    }

    async fn list_function(
        &self, _db: Option<&str>, _schema: Option<&str>, _include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let sql = r#"
            SELECT 
            RDB$FUNCTION_NAME AS FunctionName 
            FROM 
            RDB$FUNCTIONS
        "#;

        let mut conn = self.get_conn()?;

        let rows: Vec<(String,)> = conn.query(sql, ())?;

        anyhow::Ok(
            rows.into_iter()
                .map(|x| CompletionItem {
                    item: x.0.trim().to_owned().into(),
                    kind: CompletionKind::Function,
                    desc: None,
                })
                .collect(),
        )
    }

    async fn list_index(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let sql = r#"
            SELECT RDB$INDEX_NAME AS IndexName
                FROM RDB$INDICES
        "#;

        let mut conn = self.get_conn()?;
        let rows: Vec<(String,)> = conn.query(sql, ())?;
        anyhow::Ok(rows.into_iter().map(|x| x.0.trim().to_owned()).collect())
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for FireBirdDB {
    type Dialect = sqlparser::dialect::GenericDialect;

    const KEY_WORDS: &'static [&'static str] = KEY_WORDS;
    const TEXTS: &'static [&'static str] = &["true", "false"];
    const TYPES: &'static [&'static str] = TYPES;

    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        let mut ret: Vec<_> = AstCompletionHelper::list_table(self, None, None).await?;

        ret.extend(
            AstCompletionHelper::list_view(self, None, None)
                .await?
                .into_iter(),
        );

        anyhow::Ok(ret)
    }

    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self, None, None, true).await
    }
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

const TYPES: &'static [&'static str] = &[
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
impl LangServerCommon for FireBirdDB {}
//  https://firebirdsql.org/refdocs/langrefupd25-reskeywords-full-keywords.html
const KEY_WORDS: &'static [&'static str] = &[
    "ABS",
    "ACCENT",
    "ACOS",
    "ACTION",
    "ACTIVE",
    "ADD",
    "ADMIN",
    "AFTER",
    "ALL",
    "ALTER",
    "ALWAYS",
    "AND",
    "ANY",
    "AS",
    "ASC",
    "ASCENDING",
    "ASCII_CHAR",
    "ASCII_VAL",
    "ASIN",
    "AT",
    "ATAN",
    "ATAN2",
    "AUTO",
    "AUTONOMOUS",
    "AVG",
    "BACKUP",
    "BEFORE",
    "BEGIN",
    "BETWEEN",
    "BIGINT",
    "BIN_AND",
    "BIN_NOT",
    "BIN_OR",
    "BIN_SHL",
    "BIN_SHR",
    "BIN_XOR",
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
    "CEIL",
    "CEILING",
    "CHAR",
    "CHAR_LENGTH",
    "CHAR_TO_UUID",
    "CHARACTER",
    "CHARACTER_LENGTH",
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
    "COS",
    "COSH",
    "COT",
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
    "DATABASE",
    "DATE",
    "DATEADD",
    "DATEDIFF",
    "DAY",
    "DEC",
    "DECIMAL",
    "DECLARE",
    "DECODE",
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
    "EXIT",
    "EXP",
    "EXTERNAL",
    "EXTRACT",
    "FETCH",
    "FILE",
    "FILTER",
    "FIRST",
    "FIRSTNAME",
    "FLOAT",
    "FLOOR",
    "FOR",
    "FOREIGN",
    "FREE_IT",
    "FROM",
    "FULL",
    "FUNCTION",
    "GDSCODE",
    "GEN_ID",
    "GEN_UUID",
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
    "IIF",
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
    "LAST",
    "LASTNAME",
    "LEADING",
    "LEAVE",
    "LEFT",
    "LENGTH",
    "LEVEL",
    "LIKE",
    "LIMBO",
    "LIST",
    "LN",
    "LOCK",
    "LOG",
    "LOG10",
    "LONG",
    "LOWER",
    "LPAD",
    "MANUAL",
    "MAPPING",
    "MATCHED",
    "MATCHING",
    "MAX",
    "MAXIMUM_SEGMENT",
    "MAXVALUE",
    "MERGE",
    "MIDDLENAME",
    "MILLISECOND",
    "MIN",
    "MINUTE",
    "MINVALUE",
    "MOD",
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
    "RAND",
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
    "ROUND",
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
    "SEQUENCE",
    "SET",
    "SHADOW",
    "SHARED",
    "SIGN",
    "SIMILAR",
    "SIN",
    "SINGULAR",
    "SINH",
    "SIZE",
    "SKIP",
    "SMALLINT",
    "SNAPSHOT",
    "SOME",
    "SORT",
    "SOURCE",
    "SPACE",
    "SQLCODE",
    "SQLSTATE",
    "SQRT",
    "STABILITY",
    "START",
    "STARTING",
    "STARTS",
    "STATEMENT",
    "STATISTICS",
    "SUB_TYPE",
    "SUBSTRING",
    "SUM",
    "SUSPEND",
    "TABLE",
    "TAN",
    "TANH",
    "TEMPORARY",
    "THEN",
    "TIME",
    "TIMEOUT",
    "TIMESTAMP",
    "TO",
    "TRAILING",
    "TRANSACTION",
    "TRIGGER",
    "TRIM",
    "TRUNC",
    "TWO_PHASE",
    "TYPE",
    "UNCOMMITTED",
    "UNDO",
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
