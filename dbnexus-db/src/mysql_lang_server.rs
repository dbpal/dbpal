use sqlx::{AssertSqlSafe, Row, SqlSafeStr};
use std::borrow::Cow;
use std::ops::DerefMut;

use crate::db::{
    AutoCompletionUuid, FormatSqlOpts, GetBackendKind, name_start_ascii_alphabetic,
    not_support_feature,
};
use crate::helper::sqlformat_format_sql;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, DBExecutable, LangServerCommon,
    LanguageServer, LexerCompletionTrait, try_parser_and_lexer,
};
use crate::mysql::MysqlDB;
use crate::sqlparser_completion::TempTableColumn;

use sqlx::{Column, Executor, Statement};
#[async_trait::async_trait]
impl LexerCompletionTrait for MysqlDB {
    type Dialect = sqlparser::dialect::MySqlDialect;

    const TYPES: &'static [&'static str] = TYPES;

    async fn list_columns(&self, table_name: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        AstCompletionHelper::list_column(self, None, table_name, false).await
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_procedure(self, None).await
    }

    async fn extra_completion_item(&self) -> anyhow::Result<Vec<CompletionItem>> {
        self.build_fn_completion_items().await
    }

    async fn get_key_words_fn(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut lock = self.key_word_completion.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql("SELECT  * from information_schema.KEYWORDS")
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = vec![];
        for row in rows.iter() {
            let name: Option<String> = row.try_get(0)?;
            if let Some(name) = name {
                if !name_start_ascii_alphabetic(&name) {
                    continue;
                }
                ret.push(CompletionItem {
                    item: name.into(),
                    kind: CompletionKind::KeyWord,
                    desc: None,
                });
            }
        }
        *lock = ret;
        return anyhow::Ok(lock.clone());
    }

    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        // when suggest on table or view using lexer I don't know the table schema
        // we just select all from the database

        let sql = r#"
        
        SELECT table_name 
        FROM information_schema.tables  ORDER BY  table_name;
        "#;
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get_unchecked(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self, None, true).await
    }
}

impl MysqlDB {
    async fn build_fn_completion_items(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut conn = self.get_conn().await?;

        let rows = sqlx::raw_sql("SHOW FUNCTION STATUS;")
            .fetch_all(conn.deref_mut())
            .await
            .unwrap();
        let mut ret: Vec<_> = Vec::with_capacity(rows.len());

        for r in rows.iter() {
            // let db: String = r.get(0);
            let name: String = r.get(1);
            ret.push(CompletionItem {
                item: Cow::Owned(name),
                kind: crate::language_server::CompletionKind::Function,
                desc: None,
            });
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl LanguageServer for MysqlDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        try_parser_and_lexer(self, sql, line, col, use_lexer).await
    }

    fn format_sql(&self, sql: &str, ops: Option<FormatSqlOpts>) -> anyhow::Result<String> {
        anyhow::Ok(sqlformat_format_sql(sql, ops.map(|x| x.into())))
    }
}

impl MysqlDB {
    pub(crate) fn merge_schema<'a, 'b>(&'a self, schema: Option<&'b str>) -> Option<&'a str>
    where
        'b: 'a,
    {
        if schema.is_some() {
            return schema;
        }
        self.config.db.as_deref()
    }

    pub(crate) fn not_allow_empty_schema(schema: Option<&str>) -> anyhow::Result<&str> {
        match schema {
            Some(x) => anyhow::Ok(x),
            None => anyhow::bail!("empty schema"),
        }
    }
}

#[async_trait::async_trait]
impl LangServerCommon for MysqlDB {
    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::MySqlDialect {})
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        MysqlDB::list_schema_impl(self).await
    }
    async fn list_execute(&self, _: Option<&str>) -> anyhow::Result<Vec<DBExecutable>> {
        /*
            TODO should execute this sql but somehow owner_thread_id = connection_id()  match not working
            why????
            SELECT statement_id, sql_text ,statement_name   FROM performance_schema.prepared_statements_instances WHERE owner_thread_id = connection_id() and  statement_name is not null;
        */

        let conn = self.query_connection.get(&AutoCompletionUuid::get());
        if conn.is_none() {
            anyhow::bail!("should be able to get connection")
        }
        let conn = conn.unwrap();
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let sql = r#"
            SELECT  sql_text ,statement_name   
                FROM performance_schema.prepared_statements_instances WHERE 
                 statement_name is not null;
        "#;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let text: String = row.try_get(0)?;
            let name: String = row.try_get(1)?;
            ret.push(DBExecutable {
                name,
                statement: Some(text),
            });
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for MysqlDB {
    async fn keywords(&self) -> anyhow::Result<Vec<CompletionItem>> {
        LexerCompletionTrait::get_key_words_fn(self).await
    }

    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = MysqlDB::not_allow_empty_schema(self.merge_schema(schema))?;
        self.list_table_impl(schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn mysql_show_create_trigger(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let schema = MysqlDB::not_allow_empty_schema(self.merge_schema(None))?;
        let sql = format!(
            r#"
            SELECT TRIGGER_NAME 
                FROM INFORMATION_SCHEMA.TRIGGERS
                WHERE TRIGGER_SCHEMA = '{}'
         "#,
            schema
        );
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());

        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            ret.push(CompletionItem {
                item: name.into(),
                kind: CompletionKind::Trigger,
                desc: None,
            });
        }

        anyhow::Ok(ret)
    }

    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = MysqlDB::not_allow_empty_schema(self.merge_schema(schema))?;
        self.list_view_impl(schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        let mut conn = self.get_conn().await?;
        let s = conn
            .deref_mut()
            .prepare_with(AssertSqlSafe(sql).into_sql_str(), &[])
            .await?;
        let ret = s
            .columns()
            .iter()
            .map(|x| TempTableColumn {
                name: x.name().to_string(),
                typ: Some(x.type_info().to_string()),
            })
            .collect::<Vec<_>>();
        anyhow::Ok(ret)
    }
    async fn list_column(
        &self, schema: Option<&str>, name: &str, _is_call: bool,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = format!(
            r#"select * from {}`{name}` limit 1"#,
            match schema {
                Some(x) => format!("`{}`.", x),
                None => "".to_string(),
            }
        );
        log::info!("sql:{}", sql);
        self.get_query_columns(&sql).await
    }

    async fn list_function(
        &self, schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let schema = self.merge_schema(schema);
        let mut conn = self.get_conn().await?;
        let mut ret = if let Some(schema) = schema {
            crate::mysql::FunctionOrProcedure::query_list(
                crate::mysql::FunctionOrProcedure::Function,
                conn.deref_mut(),
                schema,
            )
            .await
            .map(|x| {
                x.into_iter()
                    .map(|x| CompletionItem {
                        item: x.into(),
                        kind: CompletionKind::Function,
                        desc: None,
                    })
                    .collect::<Vec<_>>()
            })?
        } else {
            Default::default()
        };

        if include_build_in {
            ret.extend(MYSQL_FUNCTIONS.iter().map(|x| CompletionItem {
                item: (*x).into(),
                kind: CompletionKind::Function,
                desc: None,
            }));
        }
        anyhow::Ok(ret)
    }

    async fn list_procedure(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = MysqlDB::not_allow_empty_schema(self.merge_schema(schema))?;
        let mut conn = self.get_conn().await?;
        crate::mysql::FunctionOrProcedure::query_list(
            crate::mysql::FunctionOrProcedure::Procedure,
            conn.deref_mut(),
            schema,
        )
        .await
    }

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = MysqlDB::not_allow_empty_schema(self.merge_schema(schema))?;
        let sql = format!(
            r#"
        SELECT   INDEX_NAME 
        FROM information_schema.STATISTICS
        WHERE TABLE_SCHEMA = '{}'"#,
            schema
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_variable(&self) -> anyhow::Result<Vec<String>> {
        let sql = "show variables";
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_partition(
        &self, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let schema = MysqlDB::not_allow_empty_schema(self.merge_schema(schema))?;
        let sql = format!(
            r#"
        SELECT PARTITION_NAME
            FROM information_schema.PARTITIONS
            WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}';"#,
            schema, table
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    // TODO sqlpaser支持解析mysql 的role语句吗
    async fn list_role(&self) -> anyhow::Result<Vec<String>> {
        not_support_feature(self.backend_name())
    }

    async fn list_constraint(
        &self, schema: Option<&str>, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        let schema = MysqlDB::not_allow_empty_schema(self.merge_schema(schema))?;
        let sql = format!(
            r#"SELECT CONSTRAINT_NAME
                FROM information_schema.TABLE_CONSTRAINTS
                WHERE TABLE_SCHEMA = '{}' AND TABLE_NAME = '{}'"#,
            schema, table_name
        );
        log::info!("sql:{}", sql);

        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_table_trigger(
        &self, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let schema = MysqlDB::not_allow_empty_schema(self.merge_schema(schema))?;
        let sql = format!(
            r#"SELECT TRIGGER_NAME
                FROM information_schema.TRIGGERS
                WHERE EVENT_OBJECT_TABLE = '{}' AND TRIGGER_SCHEMA = '{}';"#,
            table, schema
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::datasource_config::MysqlDBConfig;
    use crate::db::{AutoCompletionUuid, QuerySql, monaco_editor_keyword_regexp};
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::language_server::{LangServerCommon, LexerCompletionTrait};
    use crate::mysql::MysqlDB;
    use crate::{TEST_HOST, enable_test_log};

    async fn make_test_object(config: MysqlDBConfig) -> MysqlDB {
        let db = MysqlDB::new(config).await.unwrap();
        db
    }
    fn make_config() -> MysqlDBConfig {
        MysqlDBConfig {
            connection_timeout: None,
            host: TEST_HOST.to_string(),
            username: "root".to_string(),
            password: Some("root".to_string()),
            port: 3306, // use mariadb to test mysql have bugs.
            db: Some("test".to_string()),
            read_only: false,
            save_password: true,
            ssl: Default::default(),
        }
    }

    #[tokio::test]
    async fn print_key_words() {
        let db = make_test_object(make_config()).await;

        let keywords = <MysqlDB as LexerCompletionTrait>::get_key_words_fn(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|x| x.item.to_string())
            .collect::<Vec<_>>();

        let keywords: Vec<&str> = keywords.iter().map(|x| x.as_str()).collect();

        let s = monaco_editor_keyword_regexp(&keywords[..]);
        println!("{}", s);
    }

    // partition test failed , sqlparser can't parse a valid partition statement.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_auto_completion() {
        let db = make_test_object(make_config()).await;
        enable_test_log();
        let query_id = db.open_connection().await.unwrap();
        let _ = db
            .execute_sql(
                query_id,
                r#"
                CREATE TABLE departments (
                    id INT AUTO_INCREMENT PRIMARY KEY,
                    name VARCHAR(100) NOT NULL
                );
                CREATE TABLE employees (
                    id INT AUTO_INCREMENT PRIMARY KEY,
                    name VARCHAR(50) NOT NULL,
                    department_id INT
                )
                PARTITION BY RANGE (id) (
                    PARTITION p1 VALUES LESS THAN (10000),
                    PARTITION p2 VALUES LESS THAN (MAXVALUE)
                );    
                create view employees_view as select * from employees;
        "#,
            )
            .await
            .unwrap();
        let _ = db
            .query_sql(
                query_id,
                r#"
            DELIMITER //

            CREATE TRIGGER do_nothing_trigger
            BEFORE INSERT ON employees
            FOR EACH ROW
            BEGIN
                -- 什么都不做
            END;

            //

            DELIMITER ;
    "#,
            )
            .await
            .unwrap();

        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("mysql_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        test_file_auto_completion(&content, &db, false).await;
    }
    #[ignore = "手动跑跑的测试"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_completion_xxx() {
        enable_test_log();
        let mut config = make_config();
        config.db = None;

        let db = make_test_object(config).await;
        test_file_auto_completion(
            r#"SELECT * from test
19 test"#,
            &db,
            false,
        )
        .await;
    }
    #[tokio::test]
    async fn test_prepare() {
        let _guard = AutoCompletionUuid::test_mutex().lock().unwrap();
        let db = make_test_object(make_config()).await;
        let query_id = db.open_connection().await.unwrap();
        AutoCompletionUuid::set(query_id.clone());
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

pub(crate) const TYPES: &[&str] = &[
    "BOOLEAN",
    "BOOL",
    "TINYINT",
    "SMALLINT",
    "INT",
    "BIGINT",
    "TINYINT UNSIGNED",
    "SMALLINT UNSIGNED",
    "INT UNSIGNED",
    "BIGINT UNSIGNED",
    "FLOAT",
    "DOUBLE",
    "VARCHAR",
    "CHAR",
    "TEXT",
    "VARBINARY",
    "BINARY",
    "BLOB",
];

// I said this to AI haha.
// https://dev.mysql.com/doc/refman/8.4/en/built-in-function-reference.html 14.1 Built-In Function and Operator Reference 14.1 Built-In Function and Operator Reference    帮我把这里面所有的函数整理成rust数组
// MySQL 8.4 全部内置函数名称
pub const MYSQL_FUNCTIONS: &[&str] = &[
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
    "FORMAT_BYTES",
    "FORMAT_PICO_TIME",
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
    "RANDOM_BYTES",
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
    "BIN",
    "BIT_LENGTH",
    "CHAR",
    "CHARACTER_LENGTH",
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
