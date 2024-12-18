use std::{borrow::Cow, ops::DerefMut, sync::Arc};

use futures_util::StreamExt;
use tiberius::QueryItem;

use crate::{
    completion::{
        AutoCompletionTrait, CompletionItem, CompletionKind, CompletionOnSynTaxHelper,
        LexerCompletionTrait, try_completion_on_syntax,
    },
    db::{CatalogTrait, SchemaTrait},
    mssql::MsSqlServer,
};

#[async_trait::async_trait]
impl LexerCompletionTrait for MsSqlServer {
    type Dialect = sqlparser::dialect::MsSqlDialect;

    const KEY_WORDS: &'static [&'static str] = KEYWORDS;
    const TYPES: &'static [&'static str] = TYPES;
}

impl MsSqlServer {
    fn db_merge_config<'a, 'b>(&'a self, db: Option<&'b str>) -> Option<&'a str>
    where
        'b: 'a,
    {
        if db.is_some() {
            return db;
        }
        return self.config.db_name.as_deref();
    }
    fn not_allow_empty_db<'a>(db: Option<&'a str>) -> anyhow::Result<&'a str> {
        match db {
            Some(x) => return anyhow::Ok(x),
            None => anyhow::bail!("empty database"),
        }
    }
}
#[async_trait::async_trait]
impl AutoCompletionTrait for MsSqlServer {
    async fn auto_completion(
        &self, query_id: uuid::Uuid, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            &sql,
            query_id,
            line,
            col,
            sqlparser::dialect::MsSqlDialect {},
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
            &sqlparser::dialect::MsSqlDialect {},
            sqlparser::tokenizer::Location {
                line: line as u64,
                column: col as u64,
            },
        )
        .await;
    }
}

#[async_trait::async_trait]
impl CompletionOnSynTaxHelper for MsSqlServer {
    async fn list_role(&self) -> anyhow::Result<Vec<String>> {
        let sql = r#"
        SELECT name AS RoleName
            FROM sys.database_principals
            WHERE type = 'R';  -- 'R' 表示角色
        "#;
        let mut conn = self.get_conn().await?;
        let mut stream = conn.deref_mut().simple_query(sql).await?;
        let mut ret = Vec::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                QueryItem::Row(row) => {
                    let name: Option<&str> = row.get(0);
                    ret.push(name.unwrap_or_default().to_string());
                }
                QueryItem::Metadata(_) => {}
            }
        }
        anyhow::Ok(ret)
    }

    async fn list_table(
        &self, db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let db = Self::not_allow_empty_db(self.db_merge_config(db))?;
        self.list_table_impl(db, Self::ONLY_SCHEMA)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }
    async fn list_constraint(
        &self, db: Option<&str>, _schema: Option<&str>, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        let db = Self::not_allow_empty_db(self.db_merge_config(db))?;
        let sql = format!(
            r#"
            SELECT CONSTRAINT_NAME
            FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS
            WHERE   CONSTRAINT_CATALOG='{}'  AND CONSTRAINT_SCHEMA ='{}' AND TABLE_NAME = '{}';
        "#,
            db,
            Self::ONLY_SCHEMA,
            table_name
        );
        let mut conn = self.get_conn().await?;
        let mut stream = conn.deref_mut().simple_query(&sql).await?;
        let mut ret = Vec::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                QueryItem::Row(row) => {
                    let name: Option<&str> = row.get(0);
                    ret.push(name.unwrap_or_default().to_string());
                }
                QueryItem::Metadata(_) => {}
            }
        }
        anyhow::Ok(ret)
    }

    async fn list_view(
        &self, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        SchemaTrait::list_view(self)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        CatalogTrait::list_database(self).await
    }

    async fn list_column(
        &self, db: Option<&str>, _schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let db = Self::not_allow_empty_db(self.db_merge_config(db))?;
        let sql = format!(
            r#"
            SELECT  
                COLUMN_NAME  from 
                INFORMATION_SCHEMA.COLUMNS  
            WHERE  TABLE_CATALOG  = '{}' AND TABLE_SCHEMA  = '{}' AND TABLE_NAME = '{}'"#,
            db,
            Self::ONLY_SCHEMA,
            table,
        );
        let mut conn = self.get_conn().await?;
        let mut stream = conn.deref_mut().simple_query(&sql).await?;
        let mut ret = Vec::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                QueryItem::Row(row) => {
                    let name: Option<&str> = row.get(0);
                    ret.push(name.unwrap_or_default().to_string());
                }
                QueryItem::Metadata(_) => {}
            }
        }
        anyhow::Ok(ret)
    }
    async fn list_function(
        &self, _db: Option<&str>, _schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let mut ret = SchemaTrait::list_udf(self)
            .await
            .map(|x| {
                x.into_iter().map(|x| CompletionItem {
                    item: x.name.into(),
                    desc: None,
                    kind: CompletionKind::Function,
                })
            })?
            .collect::<Vec<_>>();
        if include_build_in {
            ret.extend(mssql_build_in().into_iter());
        }
        anyhow::Ok(ret)
    }

    async fn list_procedure(
        &self, db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let db = Self::not_allow_empty_db(self.db_merge_config(db))?;
        self.list_procedure_impl(db, Self::ONLY_SCHEMA)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn list_index(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        unimplemented!()
    }
    async fn list_schema(&self, _db: Option<&str>) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![MsSqlServer::ONLY_SCHEMA.to_string()])
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{
        completion::completion_test::test_file_auto_completion, db::QuerySql, enable_test_log,
        mssql::make_test_object,
    };

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_auto_completion() {
        let db = make_test_object().await;
        enable_test_log();
        let query_id = db.open_connection().await.unwrap();
        let _ = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE departments (
                    id INT IDENTITY(1,1) PRIMARY KEY,  -- 自增列
                    name VARCHAR(100) NOT NULL
                );

                CREATE TABLE employees (
                    id INT IDENTITY(1,1) PRIMARY KEY,  -- 自增列
                    name VARCHAR(50) NOT NULL,
                    department_id INT,
                    FOREIGN KEY (department_id) REFERENCES departments(id)  -- 添加外键约束
                );
        "#,
            )
            .await;

        let manifest_dir =
            std::env::var("CARGO_MANIFEST_DIR").expect("无法获取 CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir.as_str()).to_path_buf();
        file_path.push("tests");
        file_path.push("mssql_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        test_file_auto_completion(&content, &db, uuid::Uuid::new_v4(), false).await;
    }
    #[ignore = "xxx"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = make_test_object().await;
        test_file_auto_completion(
            r#"select * from test.dbo.employees
19 test ; 23 dbo ; 31 employees"#,
            &db,
            uuid::Uuid::new_v4(),
            false,
        )
        .await;
    }
}

const KEYWORDS: &'static [&'static str] = &[
    "ABSOLUTE",
    "EXEC",
    "OVERLAPS",
    "ACTION",
    "EXECUTE",
    "PAD",
    "ADA",
    "EXISTS",
    "PARTIAL",
    "ADD",
    "EXTERNAL",
    "PASCAL",
    "ALL",
    "EXTRACT",
    "POSITION",
    "ALLOCATE",
    "FALSE",
    "PRECISION",
    "ALTER",
    "FETCH",
    "PREPARE",
    "AND",
    "FIRST",
    "PRESERVE",
    "ANY",
    "FLOAT",
    "PRIMARY",
    "ARE",
    "FOR",
    "PRIOR",
    "AS",
    "FOREIGN",
    "PRIVILEGES",
    "ASC",
    "FORTRAN",
    "PROCEDURE",
    "ASSERTION",
    "FOUND",
    "PUBLIC",
    "AT",
    "FROM",
    "READ",
    "AUTHORIZATION",
    "FULL",
    "REAL",
    "AVG",
    "GET",
    "REFERENCES",
    "BEGIN",
    "GLOBAL",
    "RELATIVE",
    "BETWEEN",
    "GO",
    "RESTRICT",
    "BIT",
    "GOTO",
    "REVOKE",
    "BIT_LENGTH",
    "GRANT",
    "RIGHT",
    "BOTH",
    "GROUP",
    "ROLLBACK",
    "BY",
    "HAVING",
    "ROWS",
    "CASCADE",
    "HOUR",
    "SCHEMA",
    "CASCADED",
    "IDENTITY",
    "SCROLL",
    "CASE",
    "IMMEDIATE",
    "SECOND",
    "CAST",
    "IN",
    "SECTION",
    "CATALOG",
    "INCLUDE",
    "SELECT",
    "CHAR",
    "INDEX",
    "SESSION",
    "CHAR_LENGTH",
    "INDICATOR",
    "SESSION_USER",
    "CHARACTER",
    "INITIALLY",
    "SET",
    "CHARACTER_LENGTH",
    "INNER",
    "SIZE",
    "CHECK",
    "INPUT",
    "SMALLINT",
    "CLOSE",
    "INSENSITIVE",
    "SOME",
    "COALESCE",
    "INSERT",
    "SPACE",
    "COLLATE",
    "INT",
    "SQL",
    "COLLATION",
    "INTEGER",
    "SQLCA",
    "COLUMN",
    "INTERSECT",
    "SQLCODE",
    "COMMIT",
    "INTERVAL",
    "SQLERROR",
    "CONNECT",
    "INTO",
    "SQLSTATE",
    "CONNECTION",
    "IS",
    "SQLWARNING",
    "CONSTRAINT",
    "ISOLATION",
    "SUBSTRING",
    "CONSTRAINTS",
    "JOIN",
    "SUM",
    "CONTINUE",
    "KEY",
    "SYSTEM_USER",
    "CONVERT",
    "LANGUAGE",
    "TABLE",
    "CORRESPONDING",
    "LAST",
    "TEMPORARY",
    "COUNT",
    "LEADING",
    "THEN",
    "CREATE",
    "LEFT",
    "TIME",
    "CROSS",
    "LEVEL",
    "TIMESTAMP",
    "CURRENT",
    "LIKE",
    "TIMEZONE_HOUR",
    "CURRENT_DATE",
    "LOCAL",
    "TIMEZONE_MINUTE",
    "CURRENT_TIME",
    "LOWER",
    "TO",
    "CURRENT_TIMESTAMP",
    "MATCH",
    "TRAILING",
    "CURRENT_USER",
    "MAX",
    "TRANSACTION",
    "CURSOR",
    "MIN",
    "TRANSLATE",
    "DATE",
    "MINUTE",
    "TRANSLATION",
    "DAY",
    "MODULE",
    "TRIM",
    "DEALLOCATE",
    "MONTH",
    "TRUE",
    "DEC",
    "NAMES",
    "UNION",
    "DECIMAL",
    "NATIONAL",
    "UNIQUE",
    "DECLARE",
    "NATURAL",
    "UNKNOWN",
    "DEFAULT",
    "NCHAR",
    "UPDATE",
    "DEFERRABLE",
    "NEXT",
    "UPPER",
    "DEFERRED",
    "NO",
    "USAGE",
    "DELETE",
    "NONE",
    "USER",
    "DESC",
    "NOT",
    "USING",
    "DESCRIBE",
    "NULL",
    "VALUE",
    "DESCRIPTOR",
    "NULLIF",
    "VALUES",
    "DIAGNOSTICS",
    "NUMERIC",
    "VARCHAR",
    "DISCONNECT",
    "OCTET_LENGTH",
    "VARYING",
    "DISTINCT",
    "OF",
    "VIEW",
    "DOMAIN",
    "ON",
    "WHEN",
    "DOUBLE",
    "ONLY",
    "WHENEVER",
    "DROP",
    "OPEN",
    "WHERE",
    "ELSE",
    "OPTION",
    "WITH",
    "END",
    "OR",
    "WORK",
    "END-EXEC",
    "ORDER",
    "WRITE",
    "ESCAPE",
    "OUTER",
    "YEAR",
    "EXCEPT",
    "OUTPUT",
    "ZONE",
    "EXCEPTION",
];

const TYPES: &'static [&'static str] = &[
    "tinyint",
    "smallint",
    "int",
    "bigint",
    "float",
    "bit",
    "nvarchar",
    "varchar",
    "nchar",
    "char",
    "ntext",
    "text",
    "binary",
    "varbinary",
    "image",
    "uniqueidentifier",
    "numeric",
    "decimal",
    "xml",
    "datetime",
    "datetime2",
    "smalldatetime",
    "date",
    "time",
    "datetimeoffset",
];

fn mssql_build_in() -> Vec<CompletionItem> {
    static SS: &'static [[&'static str; 2]] = &[
        [
            "ASCII",
            "Returns the ASCII value for the specific character",
        ],
        ["CHAR", "Returns the character based on the ASCII code"],
        [
            "CHARINDEX",
            "Returns the position of a substring in a string",
        ],
        ["CONCAT", "Adds two or more strings together"],
        [
            "CONCAT_WS",
            "Adds two or more strings together with a separator",
        ],
        [
            "DATALENGTH",
            "Returns the number of bytes used to represent an expression",
        ],
        [
            "DIFFERENCE",
            "Compares two SOUNDEX values, and returns an integer value",
        ],
        ["FORMAT", "Formats a value with the specified format"],
        [
            "LEFT",
            "Extracts a number of characters from a string (starting from left)",
        ],
        ["LEN", "Returns the length of a string"],
        ["LOWER", "Converts a string to lower-case"],
        ["LTRIM", "Removes leading spaces from a string"],
        [
            "NCHAR",
            "Returns the Unicode character based on the number code",
        ],
        ["PATINDEX", "Returns the position of a pattern in a string"],
        [
            "QUOTENAME",
            "Returns a Unicode string with delimiters added to make the string a valid SQL Server delimited identifier",
        ],
        [
            "REPLACE",
            "Replaces all occurrences of a substring within a string, with a new substring",
        ],
        ["REPLICATE", "Repeats a string a specified number of times"],
        ["REVERSE", "Reverses a string and returns the result"],
        [
            "RIGHT",
            "Extracts a number of characters from a string (starting from right)",
        ],
        ["RTRIM", "Removes trailing spaces from a string"],
        [
            "SOUNDEX",
            "Returns a four-character code to evaluate the similarity of two strings",
        ],
        [
            "SPACE",
            "Returns a string of the specified number of space characters",
        ],
        ["STR", "Returns a number as string"],
        [
            "STUFF",
            "Deletes a part of a string and then inserts another part into the string, starting at a specified position",
        ],
        ["SUBSTRING", "Extracts some characters from a string"],
        [
            "TRANSLATE",
            "Returns the string from the first argument after the characters specified in the second argument are translated into the characters specified in the third argument.",
        ],
        [
            "TRIM",
            "Removes leading and trailing spaces (or other specified characters) from a string",
        ],
        [
            "UNICODE",
            "Returns the Unicode value for the first character of the input expression",
        ],
        ["UPPER", "Converts a string to upper-case"],
        ["ABS", "Returns the absolute value of a number"],
        ["ACOS", "Returns the arc cosine of a number"],
        ["ASIN", "Returns the arc sine of a number"],
        ["ATAN", "Returns the arc tangent of a number"],
        ["ATN2", "Returns the arc tangent of two numbers"],
        ["AVG", "Returns the average value of an expression"],
        [
            "CEILING",
            "Returns the smallest integer value that is >= a number",
        ],
        [
            "COUNT",
            "Returns the number of records returned by a select query",
        ],
        ["COS", "Returns the cosine of a number"],
        ["COT", "Returns the cotangent of a number"],
        ["DEGREES", "Converts a value in radians to degrees"],
        ["EXP", "Returns e raised to the power of a specified number"],
        [
            "FLOOR",
            "Returns the largest integer value that is <= to a number",
        ],
        [
            "LOG",
            "Returns the natural logarithm of a number, or the logarithm of a number to a specified base",
        ],
        [
            "LOG10",
            "Returns the natural logarithm of a number to base 10",
        ],
        ["MAX", "Returns the maximum value in a set of values"],
        ["MIN", "Returns the minimum value in a set of values"],
        ["PI", "Returns the value of PI"],
        [
            "POWER",
            "Returns the value of a number raised to the power of another number",
        ],
        ["RADIANS", "Converts a degree value into radians"],
        ["RAND", "Returns a random number"],
        [
            "ROUND",
            "Rounds a number to a specified number of decimal places",
        ],
        ["SIGN", "Returns the sign of a number"],
        ["SIN", "Returns the sine of a number"],
        ["SQRT", "Returns the square root of a number"],
        ["SQUARE", "Returns the square of a number"],
        ["SUM", "Calculates the sum of a set of values"],
        ["TAN", "Returns the tangent of a number"],
        ["CURRENT_TIMESTAMP", "Returns the current date and time"],
        [
            "DATEADD",
            "Adds a time/date interval to a date and then returns the date",
        ],
        ["DATEDIFF", "Returns the difference between two dates"],
        [
            "DATEFROMPARTS",
            "Returns a date from the specified parts (year, month, and day values)",
        ],
        ["DATENAME", "Returns a specified part of a date (as string)"],
        [
            "DATEPART",
            "Returns a specified part of a date (as integer)",
        ],
        ["DAY", "Returns the day of the month for a specified date"],
        [
            "GETDATE",
            "Returns the current database system date and time",
        ],
        [
            "GETUTCDATE",
            "Returns the current database system UTC date and time",
        ],
        [
            "ISDATE",
            "Checks an expression and returns 1 if it is a valid date, otherwise 0",
        ],
        [
            "MONTH",
            "Returns the month part for a specified date (a number from 1 to 12)",
        ],
        ["SYSDATETIME", "Returns the date and time of the SQL Server"],
        ["YEAR", "Returns the year part for a specified date"],
        [
            "CAST",
            "Converts a value (of any type) into a specified datatype",
        ],
        ["COALESCE", "Returns the first non-null value in a list"],
        [
            "CONVERT",
            "Converts a value (of any type) into a specified datatype",
        ],
        [
            "CURRENT_USER",
            "Returns the name of the current user in the SQL Server database",
        ],
        [
            "IIF",
            "Returns a value if a condition is TRUE, or another value if a condition is FALSE",
        ],
        [
            "ISNULL",
            "Return a specified value if the expression is NULL, otherwise return the expression",
        ],
        ["ISNUMERIC", "Tests whether an expression is numeric"],
        ["NULLIF", "Returns NULL if two expressions are equal"],
        [
            "SESSION_USER",
            "Returns the name of the current user in the SQL Server database",
        ],
        [
            "SESSIONPROPERTY",
            "Returns the session settings for a specified option",
        ],
        ["SYSTEM_USER", "Returns the login name for the current user"],
        [
            "USER_NAME",
            "Returns the database user name based on the specified id",
        ],
    ];

    lazy_static::lazy_static! {
        static ref CS : Vec<CompletionItem> = SS.iter().map(|x| CompletionItem {
            item : Cow::Borrowed(x[0].into()),
            kind : CompletionKind::Pragma,
            desc: Some( x[1].into()),
        }).collect();
    }
    return CS.clone();
}

#[cfg(test)]
#[test]
fn mysql_fn() {
    let s = include_str!("./mssql_fn_str");
    for line in s.lines() {
        if line.trim() == "" {
            continue;
        }
        let (cmd, desc) = line.split_once(&[' ', '\t']).unwrap();
        let cmd = cmd.trim();
        let desc = desc.trim();
        let cmds: Vec<_> = cmd.split(',').collect();

        for cmd in cmds.iter() {
            println!("[\"{}\" , \"{}\" ],", cmd, desc);
        }
    }
}
