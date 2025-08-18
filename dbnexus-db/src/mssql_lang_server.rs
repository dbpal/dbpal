use std::ops::DerefMut;

use crate::db::FormatSqlOpts;
use crate::helper::sqlformat_format_sql;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, DBExecutable, LangServerCommon,
    LanguageServer, LexerCompletionTrait, try_completion_on_syntax,
};
use crate::mssql::MsSqlServerDB;
use crate::sqlparser_completion::TempTableColumn;
use futures_util::StreamExt;

use tiberius::QueryItem;

#[async_trait::async_trait]
impl LexerCompletionTrait for MsSqlServerDB {
    type Dialect = sqlparser::dialect::MsSqlDialect;

    async fn list_columns(&self, table_name: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        AstCompletionHelper::list_column(self, None, table_name, false).await
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_procedure(self, None).await
    }

    const TYPES: &'static [&'static str] = SQLSERVER_TYPES;

    async fn get_key_words_fn(&self) -> anyhow::Result<Vec<CompletionItem>> {
        anyhow::Ok(
            SQLSERVER_KEYWORDS
                .iter()
                .map(|x| CompletionItem {
                    item: (*x).into(),
                    desc: None,
                    kind: CompletionKind::KeyWord,
                })
                .collect(),
        )
    }

    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        let mut ret = AstCompletionHelper::list_table(self, None)
            .await
            .unwrap_or_default();
        ret.extend(
            AstCompletionHelper::list_view(self, None)
                .await
                .unwrap_or_default()
                .into_iter(),
        );
        anyhow::Ok(ret)
    }

    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self, None, true).await
    }
}

#[async_trait::async_trait]
impl LanguageServer for MsSqlServerDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(sql, line, col, sqlparser::dialect::MsSqlDialect {}, self)
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
            &sqlparser::dialect::MsSqlDialect {},
        )
        .await;
    }

    fn format_sql(&self, sql: &str, ops: Option<FormatSqlOpts>) -> anyhow::Result<String> {
        anyhow::Ok(sqlformat_format_sql(sql, ops.map(|x| x.into())))
    }
}

#[async_trait::async_trait]
impl LangServerCommon for MsSqlServerDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![
            MsSqlServerDB::DBO_SCHEMA.to_string(),
            MsSqlServerDB::SYS_SCHEMA.to_string(),
        ])
    }

    async fn list_execute(&self, schema: Option<&str>) -> anyhow::Result<Vec<DBExecutable>> {
        let mut x = self.list_procedure_impl(Self::DBO_SCHEMA).await?;
        let b = self.list_procedure_impl(Self::SYS_SCHEMA).await?;
        match schema {
            Some(schema) => match schema {
                Self::SYS_SCHEMA => x = b,
                _ => {
                    x.extend(b.into_iter());
                }
            },
            None => {
                x.extend(b.into_iter());
            }
        };
        anyhow::Ok(
            x.into_iter()
                .map(|x| DBExecutable {
                    name: x.name.into(),
                    statement: None,
                })
                .collect(),
        )
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for MsSqlServerDB {
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

    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let db = self.config.db_str();
        let schema = schema.unwrap_or(Self::DBO_SCHEMA);
        self.list_table_impl(db, schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn list_constraint(
        &self, _schema: Option<&str>, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        let db: &String = self.config.db_str();
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

    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = schema.unwrap_or(Self::DBO_SCHEMA);
        self.list_view_impl(schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
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
                                typ: Some(format!("{:?}", x.column_type())),
                            })
                            .collect(),
                    );
                }
            }
        }
        log::error!("no reuslt found");
        anyhow::Ok(vec![])
    }

    async fn list_column(
        &self, schema: Option<&str>, name: &str, _is_call: bool,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = format!(
            r#"  select TOP 1 * from  {}"{}" "#,
            match schema {
                Some(x) => format!("\"{}\".", x),
                None => "".to_string(),
            },
            name
        );
        self.get_query_columns(&sql).await
    }

    async fn list_function(
        &self, schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        // 在 SQL Server 中，sys 模式下并没有直接的用户定义函数 (UDF)，而是包含了一系列的系统视图和存储过程。不过，你可以使用一些系统函数来获取数据库元数据。以下是一些常用的系统函数，这些可以帮助你查询数据库的信息：
        let schema = schema.unwrap_or(Self::DBO_SCHEMA);
        let mut ret = self
            .list_udf_impl(schema)
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
            ret.extend(SQLSERVER_FUNCTIONS.iter().map(|x| CompletionItem {
                item: (*x).into(),
                kind: CompletionKind::Function,
                desc: None,
            }));
        }
        anyhow::Ok(ret)
    }

    async fn list_procedure(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_execute(self, schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

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
}

#[cfg(test)]

mod tests {
    use std::path::Path;

    use crate::db::QuerySql;
    use crate::enable_test_log;
    use crate::language_server::LangServerCommon;
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::mssql::{make_test_object, test_lock};

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_auto_completion() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        enable_test_log();
        let query_id = db.open_connection().await.unwrap();
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
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("mssql_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        test_file_auto_completion(&content, &db, false).await;
    }

    #[ignore = "手动跑跑的测试"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_completion_xxx() {
        let _guard = test_lock().lock().await;
        enable_test_log();
        let db = make_test_object().await;
        test_file_auto_completion(
            r#"GRANT SELECT ON dbo.employees TO public;
20 dbo ; 30 employees"#,
            &db,
            false,
        )
        .await;
    }
    #[ignore = "手动跑跑的测试"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_completion_execute() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let x = db.list_execute(None).await;
        println!("###########:{:?}", x);
    }
}

/// SQL Server 2022 全部内置函数名称
pub const SQLSERVER_FUNCTIONS: &[&str] = &[
    // 数学
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
    // 字符串
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
    // 日期时间
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
    // 聚合/分析
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
    // 加密/哈希
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
    // 转换
    "CAST",
    "CONVERT",
    "COALESCE",
    "ISNULL",
    "NULLIF",
    // 系统信息
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
    // 其他常用
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
    "XACT_STATE", // 如需继续扩展，可执行：
                  // SELECT name FROM master.sys.all_objects
                  // WHERE type IN ('FN','IF','TF','FS','FT','AF') AND is_ms_shipped = 1 ORDER BY name;
];

/// SQL Server 2022 保留关键字（官方版）
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

/// SQL Server 2022 全部系统数据类型
pub const SQLSERVER_TYPES: &[&str] = &[
    // 精确数值
    "BIGINT",
    "INT",
    "SMALLINT",
    "TINYINT",
    "BIT",
    "DECIMAL",
    "NUMERIC",
    "MONEY",
    "SMALLMONEY",
    // 近似数值
    "FLOAT",
    "REAL",
    // 日期/时间
    "DATE",
    "TIME",
    "DATETIME",
    "DATETIME2",
    "DATETIMEOFFSET",
    "SMALLDATETIME",
    // 字符
    "CHAR",
    "VARCHAR",
    "TEXT",
    "NCHAR",
    "NVARCHAR",
    "NTEXT",
    // 二进制
    "BINARY",
    "VARBINARY",
    "IMAGE",
    // 其他
    "CURSOR",
    "SQL_VARIANT",
    "TABLE",
    "TIMESTAMP",
    "UNIQUEIDENTIFIER",
    "XML",
    "GEOMETRY",
    "GEOGRAPHY",
    "HIERARCHYID",
    // 变长 MAX
    "VARCHAR(MAX)",
    "NVARCHAR(MAX)",
    "VARBINARY(MAX)",
];
