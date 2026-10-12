/*
    In SQL Server, only objects in the default dbo schema can be accessed directly without a schema prefix; all other custom schemas must be explicitly qualified as schema.object.
*/

use crate::db::{CatalogTrait, CreatedTypeDef};
use crate::language_server::{
    try_parser_dot_lexer, AstCompletionHelper, CompletionItem, CompletionKind, CompletionList,
    DBExecutable, GetKeyWords, LangServerCommon, LanguageServer, LexerCompletionTrait,
};
use crate::mssql::SqlServerDB;
use crate::sqlparser_completion::TempTableColumn;
use futures_util::StreamExt;
use std::ops::DerefMut;
use tiberius::QueryItem;
#[async_trait::async_trait]
impl LexerCompletionTrait for SqlServerDB {
    async fn get_types(&self) -> anyhow::Result<Vec<String>> {
        let r = SQLSERVER_TYPES.iter().map(|x| x.to_string()).collect();
        anyhow::Ok(r)
    }
}

#[async_trait::async_trait]
impl GetKeyWords for SqlServerDB {
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(SQLSERVER_KEYWORDS.iter().map(|x| x.to_string()).collect())
    }
}

#[async_trait::async_trait]
impl LanguageServer for SqlServerDB {
    async fn ai_text_completion(
        &self,
        text: &str,
        line: u32,
        col: u32,
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
        &self,
        _query_id: Option<String>,
        sql: &str,
        line: u32,
        col: u32,
        use_lexer: bool,
    ) -> anyhow::Result<CompletionList> {
        try_parser_dot_lexer(self, sql, line, col, use_lexer).await
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        LangServerCommon::is_dangerous_sql_by_parser(self, sql)
    }
}

impl SqlServerDB {
    fn schema_merge_config(&self, schema: Option<&str>) -> Option<String> {
        if schema.is_some() {
            return schema.map(|x| x.to_string());
        }
        self.config.schema.as_ref().map(|x| x.to_string())
    }
    fn not_allow_empty_schema(&self, schema: Option<&str>) -> anyhow::Result<String> {
        match self.schema_merge_config(schema) {
            Some(x) => anyhow::Ok(x),
            None => anyhow::Ok(Self::DBO_SCHEMA.to_string()),
        }
    }
}

#[async_trait::async_trait]
impl LangServerCommon for SqlServerDB {
    async fn list_index(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let sql = r#"
        SELECT 
            i.name AS IndexName  FROM 
            sys.indexes AS i
        INNER JOIN 
            sys.index_columns AS ic 
            ON i.object_id = ic.object_id 
            AND i.index_id = ic.index_id
        INNER JOIN 
            sys.columns AS c 
            ON ic.object_id = c.object_id 
            AND ic.column_id = c.column_id
        INNER JOIN 
            sys.tables AS t 
            ON i.object_id = t.object_id
        WHERE 
            t.is_ms_shipped = 0 -- 过滤掉系统表
        ORDER BY 
            t.name, i.name, ic.key_ordinal;
            "#;
        let mut conn = self.get_conn().await?;

        let mut ret = Vec::new();

        let mut stream = conn.simple_query(sql).await?;
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                QueryItem::Row(row) => {
                    let name: Option<&str> = row.try_get(0)?;
                    if let Some(name) = name {
                        ret.push(name.to_owned());
                    }
                }
                QueryItem::Metadata(..) => {}
            }
        }

        anyhow::Ok(ret)
    }

    async fn list_variable(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(
            SQL_SERVER_GLOBAL_VARS
                .iter()
                .map(|x| x.to_string())
                .collect(),
        )
    }

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

    async fn list_created_type(&self, schema: Option<&str>) -> anyhow::Result<Vec<CreatedTypeDef>> {
        let schema = self.not_allow_empty_schema(schema)?;
        let sql = format!(
            r#"
            SELECT
                s.name AS schema_name,
                t.name AS type_name
            FROM sys.types AS t
            JOIN sys.schemas AS s ON t.schema_id = s.schema_id
            WHERE t.is_user_defined = 1
            AND s.name = '{}';   -- 把 dbo 换成目标 schema

        "#,
            schema
        );
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream = conn.simple_query(&sql).await?;

        let mut ret = vec![];
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                QueryItem::Row(row) => {
                    let name: Option<&str> = row.try_get(0)?;
                    if let Some(name) = name {
                        ret.push(CreatedTypeDef {
                            name: name.to_string(),
                            sql: None,
                        });
                    }
                }
                QueryItem::Metadata(..) => {}
            }
        }
        anyhow::Ok(ret)
    }
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        CatalogTrait::list_database(self).await
    }

    async fn list_column(
        &self,
        schema: Option<&str>,
        name: &str,
        is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = if let Some(is_call) = is_call {
            format!(
                r#"  select  {}{}({}) "#,
                match schema {
                    Some(x) => format!("\"{}\".", x),
                    None => "".to_string(),
                },
                name,
                is_call.join(",")
            )
        } else {
            format!(
                r#"  select TOP 1 * from  {}"{}" "#,
                match schema {
                    Some(x) => format!("\"{}\".", x),
                    None => "".to_string(),
                },
                name
            )
        };
        self.get_query_columns(&sql).await
    }

    async fn list_procedure(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_execute(self, schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }
    async fn list_function(
        &self,
        schema: Option<&str>,
        include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let schema = self.schema_merge_config(schema);
        let mut ret = if let Some(schema) = &schema {
            self.list_udf_impl(schema)
                .await
                .map(|x| {
                    x.into_iter().map(|x| CompletionItem {
                        item: x.name.into(),
                        desc: None,
                        kind: CompletionKind::Function,
                    })
                })?
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        if include_build_in {
            ret.extend(SQLSERVER_FUNCTIONS.iter().map(|x| CompletionItem {
                item: (*x).into(),
                kind: CompletionKind::Function,
                desc: None,
            }));
        }
        anyhow::Ok(ret)
    }
    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::MsSqlDialect {})
    }

    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let db = self.config.db_str()?;
        let schema = self.not_allow_empty_schema(schema)?;
        self.list_table_impl(db, &schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let sql = r#"
            SELECT 
                s.name AS SchemaName
            FROM 
                sys.schemas s
            ORDER BY 
                s.name;
        "#;
        let mut conn = self.get_conn().await?;
        let conn = conn.deref_mut();
        let mut stream = conn.simple_query(sql).await?;
        let mut ret = Vec::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                QueryItem::Row(row) => {
                    let name: Option<&str> = row.try_get(0)?;
                    if let Some(name) = name {
                        ret.push(name.to_string());
                    }
                }
                QueryItem::Metadata(..) => {}
            }
        }
        anyhow::Ok(ret)
    }

    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = self.not_allow_empty_schema(schema)?;
        self.list_view_impl(&schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn list_execute(&self, schema: Option<&str>) -> anyhow::Result<Vec<DBExecutable>> {
        let call_schema = schema;
        let schema = self.not_allow_empty_schema(schema)?;
        let mut ret: Vec<crate::db::NamedStaff> = self.list_procedure_impl(&schema).await?;
        if call_schema.is_none() && schema != Self::SYS_SCHEMA {
            ret.extend(self.list_procedure_impl(Self::SYS_SCHEMA).await?);
        }
        anyhow::Ok(
            ret.into_iter()
                .map(|x| DBExecutable {
                    name: x.name,
                    statement: None,
                })
                .collect(),
        )
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for SqlServerDB {
    async fn list_constraint(
        &self,
        _schema: Option<&str>,
        table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        let db: &String = self.config.db_str()?;
        let sql = format!(
            r#"
            SELECT CONSTRAINT_NAME
            FROM INFORMATION_SCHEMA.TABLE_CONSTRAINTS
            WHERE   CONSTRAINT_CATALOG='{}'  AND CONSTRAINT_SCHEMA ='{}' AND TABLE_NAME = '{}';
        "#,
            db,
            Self::DBO_SCHEMA,
            table_name
        );
        log::info!("sql:{}", sql);
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

    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        let mut conn = self.get_conn().await?;
        let mut s = conn.simple_query(sql).await?;
        while let Some(s) = s.next().await {
            let s = s?;
            match s {
                QueryItem::Row(_) => {}
                QueryItem::Metadata(result_metadata) => {
                    return anyhow::Ok(
                        result_metadata
                            .columns()
                            .iter()
                            .map(|x| TempTableColumn {
                                name: x.name().to_string(),
                                data_type: Some(format!("{:?}", x.column_type())),
                            })
                            .collect(),
                    );
                }
            }
        }
        log::error!("no result found");
        anyhow::Ok(vec![])
    }
}

/// All SQL Server 2022 built-in function names.
pub const SQLSERVER_FUNCTIONS: &[&str] = &[
    // Math
    "ABS",
    "ACOS",
    "ASIN",
    "ATAN",
    "ATN2",
    "CEILING",
    "COS",
    "COT",
    "DEGREES",
    "EXP",
    "FLOOR",
    "LOG",
    "LOG10",
    "PI",
    "POWER",
    "RADIANS",
    "RAND",
    "ROUND",
    "SIGN",
    "SIN",
    "SQRT",
    "SQUARE",
    "TAN",
    // String
    "ASCII",
    "CHAR",
    "CHARINDEX",
    "CONCAT",
    "CONCAT_WS",
    "DATALENGTH",
    "DIFFERENCE",
    "FORMAT",
    "LEFT",
    "LEN",
    "LOWER",
    "LTRIM",
    "NCHAR",
    "PATINDEX",
    "QUOTENAME",
    "REPLACE",
    "REPLICATE",
    "REVERSE",
    "RIGHT",
    "RTRIM",
    "SOUNDEX",
    "SPACE",
    "STR",
    "STUFF",
    "SUBSTRING",
    "TRANSLATE",
    "TRIM",
    "UNICODE",
    "UPPER",
    // Date/time
    "CURRENT_TIMESTAMP",
    "CURRENT_TIMEZONE",
    "CURRENT_TIMEZONE_ID",
    "DATEADD",
    "DATEDIFF",
    "DATENAME",
    "DATEPART",
    "DAY",
    "EOMONTH",
    "GETDATE",
    "GETUTCDATE",
    "ISDATE",
    "MONTH",
    "SMALLDATETIMEFROMPARTS",
    "SWITCHOFFSET",
    "SYSDATETIME",
    "SYSDATETIMEOFFSET",
    "SYSUTCDATETIME",
    "TIMEFROMPARTS",
    "TODATETIMEOFFSET",
    "YEAR",
    // JSON
    "ISJSON",
    "JSON_MODIFY",
    "JSON_QUERY",
    "JSON_VALUE",
    // Aggregate/analytic
    "AVG",
    "CHECKSUM_AGG",
    "COUNT",
    "COUNT_BIG",
    "CUME_DIST",
    "DENSE_RANK",
    "FIRST_VALUE",
    "LAG",
    "LAST_VALUE",
    "LEAD",
    "MAX",
    "MIN",
    "NTILE",
    "PERCENTILE_CONT",
    "PERCENTILE_DISC",
    "PERCENT_RANK",
    "RANK",
    "ROW_NUMBER",
    "STDEV",
    "STDEVP",
    "SUM",
    "VAR",
    "VARP",
    // Encryption/hash
    "CERTENCODED",
    "CERTPRIVATEKEY",
    "DECRYPTBYASYMKEY",
    "DECRYPTBYCERT",
    "DECRYPTBYKEY",
    "DECRYPTBYKEYAUTOASYMKEY",
    "DECRYPTBYKEYAUTOCERT",
    "DECRYPTBYPASSPHRASE",
    "ENCRYPTBYASYMKEY",
    "ENCRYPTBYCERT",
    "ENCRYPTBYKEY",
    "ENCRYPTBYPASSPHRASE",
    "HASHBYTES",
    "IS_OBJECTSIGNED",
    "KEY_GUID",
    "KEY_ID",
    "SIGNBYASYMKEY",
    "SIGNBYCERT",
    "VERIFYSIGNEDBYCERT",
    "VERIFYSIGNEDBYASYMKEY",
    // Conversion
    "CAST",
    "CONVERT",
    "COALESCE",
    "ISNULL",
    "NULLIF",
    // System information
    "APP_NAME",
    "CURRENT_USER",
    "DATABASEPROPERTYEX",
    "DB_ID",
    "DB_NAME",
    "HOST_ID",
    "HOST_NAME",
    "IS_MEMBER",
    "IS_SRVROLEMEMBER",
    "ORIGINAL_DB_NAME",
    "ORIGINAL_LOGIN",
    "SERVERPROPERTY",
    "SESSION_USER",
    "SYSTEM_USER",
    "USER_ID",
    "USER_NAME",
    // Other common functions
    "BINARY_CHECKSUM",
    "CHECKSUM",
    "ERROR_LINE",
    "ERROR_MESSAGE",
    "ERROR_NUMBER",
    "ERROR_PROCEDURE",
    "ERROR_SEVERITY",
    "ERROR_STATE",
    "FORMATMESSAGE",
    "GETANSINULL",
    "HOST_NAME",
    "IDENT_CURRENT",
    "IDENT_INCR",
    "IDENT_SEED",
    "IDENTITY",
    "ISNUMERIC",
    "NEWID",
    "NEWSEQUENTIALID",
    "PARSENAME",
    "ROWCOUNT_BIG",
    "SCOPE_IDENTITY",
    "SERVERPROPERTY",
    "SESSIONPROPERTY",
    "STATS_DATE",
    "XACT_STATE", // To extend this list, run:
                  // SELECT name FROM master.sys.all_objects
                  // WHERE type IN ('FN','IF','TF','FS','FT','AF') AND is_ms_shipped = 1 ORDER BY name;
];

/// SQL Server 2022 reserved keywords (official list).
pub const SQLSERVER_KEYWORDS: &[&str] = &[
    "ADD",
    "ALL",
    "ALTER",
    "AND",
    "ANY",
    "AS",
    "ASC",
    "AUTHORIZATION",
    "BACKUP",
    "BEGIN",
    "BETWEEN",
    "BREAK",
    "BROWSE",
    "BULK",
    "BY",
    "CASCADE",
    "CASE",
    "CHECK",
    "CHECKPOINT",
    "CLOSE",
    "CLUSTERED",
    "COALESCE",
    "COLLATE",
    "COLUMN",
    "COMMIT",
    "COMPUTE",
    "CONSTRAINT",
    "CONTAINS",
    "CONTAINSTABLE",
    "CONTINUE",
    "CONVERT",
    "CREATE",
    "CROSS",
    "CURRENT",
    "CURRENT_DATE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "CURRENT_USER",
    "CURSOR",
    "DATABASE",
    "DBCC",
    "DEALLOCATE",
    "DECLARE",
    "DEFAULT",
    "DELETE",
    "DENY",
    "DESC",
    "DISK",
    "DISTINCT",
    "DISTRIBUTED",
    "DOUBLE",
    "DROP",
    "DUMP",
    "ELSE",
    "END",
    "ERRLVL",
    "ESCAPE",
    "EXCEPT",
    "EXEC",
    "EXECUTE",
    "EXISTS",
    "EXIT",
    "EXTERNAL",
    "FETCH",
    "FILE",
    "FILLFACTOR",
    "FOR",
    "FOREIGN",
    "FREETEXT",
    "FREETEXTTABLE",
    "FROM",
    "FULL",
    "FUNCTION",
    "GOTO",
    "GRANT",
    "GROUP",
    "HAVING",
    "HOLDLOCK",
    "IDENTITY",
    "IDENTITY_INSERT",
    "IDENTITYCOL",
    "IF",
    "IN",
    "INDEX",
    "INNER",
    "INSERT",
    "INTERSECT",
    "INTO",
    "IS",
    "JOIN",
    "KEY",
    "KILL",
    "LEFT",
    "LIKE",
    "LINENO",
    "LOAD",
    "MERGE",
    "NATIONAL",
    "NOCHECK",
    "NONCLUSTERED",
    "NOT",
    "NULL",
    "NULLIF",
    "OF",
    "OFF",
    "OFFSETS",
    "ON",
    "OPEN",
    "OPENDATASOURCE",
    "OPENQUERY",
    "OPENROWSET",
    "OPENXML",
    "OPTION",
    "OR",
    "ORDER",
    "OUTER",
    "OVER",
    "PERCENT",
    "PIVOT",
    "PLAN",
    "PRECISION",
    "PRIMARY",
    "PRINT",
    "PROC",
    "PROCEDURE",
    "PUBLIC",
    "RAISERROR",
    "READ",
    "READTEXT",
    "RECONFIGURE",
    "REFERENCES",
    "REPLICATION",
    "RESTORE",
    "RESTRICT",
    "RETURN",
    "REVERT",
    "REVOKE",
    "RIGHT",
    "ROLLBACK",
    "ROWCOUNT",
    "ROWGUIDCOL",
    "RULE",
    "SAVE",
    "SCHEMA",
    "SECURITYAUDIT",
    "SELECT",
    "SEMANTICKEYPHRASETABLE",
    "SEMANTICSIMILARITYDETAILSTABLE",
    "SEMANTICSIMILARITYTABLE",
    "SESSION_USER",
    "SET",
    "SETUSER",
    "SHUTDOWN",
    "SOME",
    "STATISTICS",
    "SYSTEM_USER",
    "TABLE",
    "TABLESAMPLE",
    "TEXTSIZE",
    "THEN",
    "TO",
    "TOP",
    "TRAN",
    "TRANSACTION",
    "TRIGGER",
    "TRUNCATE",
    "TSEQUAL",
    "UNION",
    "UNIQUE",
    "UNPIVOT",
    "UPDATE",
    "UPDATETEXT",
    "USE",
    "USER",
    "VALUES",
    "VARYING",
    "VIEW",
    "WAITFOR",
    "WHEN",
    "WHERE",
    "WHILE",
    "WITH",
    "WRITETEXT",
];

/// All SQL Server 2022 system data types.
pub const SQLSERVER_TYPES: &[&str] = &[
    // Exact numeric
    "BIGINT",
    "INT",
    "SMALLINT",
    "TINYINT",
    "BIT",
    "DECIMAL",
    "NUMERIC",
    "MONEY",
    "SMALLMONEY",
    // Approximate numeric
    "FLOAT",
    "REAL",
    // Date/time
    "DATE",
    "TIME",
    "DATETIME",
    "DATETIME2",
    "DATETIMEOFFSET",
    "SMALLDATETIME",
    // Character
    "CHAR",
    "VARCHAR",
    "TEXT",
    "NCHAR",
    "NVARCHAR",
    "NTEXT",
    // Binary
    "BINARY",
    "VARBINARY",
    "IMAGE",
    // Other
    "CURSOR",
    "SQL_VARIANT",
    "TABLE",
    "TIMESTAMP",
    "UNIQUEIDENTIFIER",
    "XML",
    "GEOMETRY",
    "GEOGRAPHY",
    "HIERARCHYID",
    // Variable-length MAX
    "VARCHAR(MAX)",
    "NVARCHAR(MAX)",
    "VARBINARY(MAX)",
];

/// All SQL Server @@ global variable names (system functions).
/// Usage: format!("@@{}", VAR[i])
pub const SQL_SERVER_GLOBAL_VARS: &[&str] = &[
    "DATEFIRST",
    "DBTS",
    "LANGID",
    "LANGUAGE",
    "LOCK_TIMEOUT",
    "MAX_CONNECTIONS",
    "MAX_PRECISION",
    "NESTLEVEL",
    "OPTIONS",
    "REMSERVER",
    "SERVERNAME",
    "SERVICENAME",
    "SPID",
    "TEXTSIZE",
    "VERSION",
    "CURSOR_ROWS",
    "FETCH_STATUS",
    "PROCID",
    "ERROR",
    "ROWCOUNT",
    "TRANCOUNT",
    "IDENTITY",
    "PACK_RECEIVED",
    "PACK_SENT",
    "PACKET_ERRORS",
    "TOTAL_READ",
    "TOTAL_WRITE",
    "TOTAL_ERRORS",
    "CONNECTIONS",
    "CPU_BUSY",
    "IDLE",
    "IO_BUSY",
    "PACKET_ERRORS",
    "TIMETICKS",
    "TOTAL_ERRORS",
    "TOTAL_READ",
    "TOTAL_WRITE",
    "BUSY",
    "IDLE",
    "IO_BUSY",
    "PACK_RECEIVED",
    "PACK_SENT",
    "PACKET_ERRORS",
    "TIMETICKS",
    "TOTAL_ERRORS",
    "TOTAL_READ",
    "TOTAL_WRITE",
];

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::db::QuerySql;
    use crate::enable_test_log;
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::language_server::LangServerCommon;
    use crate::mssql::{make_test_object, test_lock};

    #[tokio::test]
    async fn test_auto_completion() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        enable_test_log();
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _ = db
            .execute_sql(
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

        db.execute_sql(
            query_id,
            "INSERT INTO departments (name) VALUES ('人力资源'), ('财务部'), ('市场部');",
        )
        .await
        .unwrap()
        .unwrap();
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("mssql_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        test_file_auto_completion(&content, &db, None, false).await;
    }

    #[ignore = "手动跑跑的测试"]
    #[tokio::test]
    async fn test_completion_xxx() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object().await;
        test_file_auto_completion(
            r#"GRANT SELECT ON dbo.employees TO public;
20 dbo ; 30 employees"#,
            &db,
            None,
            false,
        )
        .await;
    }
    #[ignore = "手动跑跑的测试"]
    #[tokio::test]
    async fn test_completion_execute() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let x = db.list_execute(None).await;
        println!("###########:{:?}", x);
    }
}
