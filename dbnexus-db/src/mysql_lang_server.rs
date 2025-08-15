use std::borrow::Cow;
use std::ops::DerefMut;
use std::sync::Arc;

use sqlx::{AssertSqlSafe, Row, SqlSafeStr};

use crate::db::{FormatSqlOpts, GetBackendKind, name_start_ascii_alphabetic, not_support_feature};
use crate::helper::sqlformat_format_sql;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, LangServerCommon, LanguageServer,
    LexerCompletionTrait, try_completion_on_syntax,
};
use crate::mysql::MysqlDB;
use crate::sqlparser_completion::TempTableColumn;

use sqlx::{Column, Executor, Statement};
#[async_trait::async_trait]
impl LexerCompletionTrait for MysqlDB {
    type Dialect = sqlparser::dialect::MySqlDialect;

    const TYPES: &'static [&'static str] = TYPES;

    async fn list_columns(&self, table_name: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        AstCompletionHelper::list_column(self, None, None, table_name, false).await
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<String>> {
        AstCompletionHelper::list_procedure(self, None, None).await
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
        AstCompletionHelper::list_function(self, None, None, true).await
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
        &self, _query_id: Option<uuid::Uuid>, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            sql,
            line,
            col,
            sqlparser::dialect::MySqlDialect {},
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
            log::warn!("disabled use lexer , we will return empty data.");
            return anyhow::Ok(vec![]);
        }
        return LexerCompletionTrait::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::MySqlDialect {},
        )
        .await;
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
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }

    async fn list_schema(&self, _db: Option<&str>) -> anyhow::Result<Vec<String>> {
        MysqlDB::list_schema_impl(self).await
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for MysqlDB {
    async fn keywords(&self) -> anyhow::Result<Vec<CompletionItem>> {
        LexerCompletionTrait::get_key_words_fn(self).await
    }
    async fn list_table(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
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

    async fn list_view(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
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
        &self, _db: Option<&str>, schema: Option<&str>, name: &str, _is_call: bool,
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
        &self, _db: Option<&str>, schema: Option<&str>, include_build_in: bool,
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
            ret.extend(mysql_build_in().into_iter());
        }
        anyhow::Ok(ret)
    }

    async fn list_procedure(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
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
        &self, _db: Option<&str>, schema: Option<&str>, table: &str,
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
        &self, _db: Option<&str>, schema: Option<&str>, table_name: &str,
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
        &self, _db: Option<&str>, schema: Option<&str>, table: &str,
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
    use crate::db::{QuerySql, monaco_editor_keyword_regexp};
    use crate::language_server::LexerCompletionTrait;
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::mysql::MysqlDB;
    use crate::{TEST_HOST, enable_test_log};

    async fn make_test_object(config: MysqlDBConfig) -> MysqlDB {
        let db = MysqlDB::new(config).await.unwrap();
        db
    }
    fn make_config() -> MysqlDBConfig {
        MysqlDBConfig {
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

fn mysql_build_in() -> Vec<CompletionItem> {
    static SS: &[[&str; 2]] = &[
        ["ABS", "Return the absolute value"],
        ["ACOS", "Return the arc cosine"],
        ["ADDDATE", "Add time values (intervals) to a date value"],
        ["ADDTIME", "Add time"],
        ["AES_DECRYPT", "Decrypt using AES"],
        ["AES_ENCRYPT", "Encrypt using AES"],
        ["AND", "Logical AND"],
        ["&&", "Logical AND"],
        ["ANY_VALUE", "Suppress ONLY_FULL_GROUP_BY value rejection"],
        ["ASCII", "Return numeric value of left-most character"],
        ["ASIN", "Return the arc sine"],
        [
            "asynchronous_connection_failover_add_managed",
            "Add group member source server configuration information to a replication channel source list",
        ],
        [
            "asynchronous_connection_failover_add_source",
            "Add source server configuration information server to a replication channel source list",
        ],
        [
            "asynchronous_connection_failover_delete_managed",
            "Remove a managed group from a replication channel source list",
        ],
        [
            "asynchronous_connection_failover_delete_source",
            "Remove a source server from a replication channel source list",
        ],
        [
            "asynchronous_connection_failover_reset",
            "Remove all settings relating to group replication asynchronous failover",
        ],
        ["ATAN", "Return the arc tangent"],
        ["ATAN2", "Return the arc tangent of the two arguments"],
        ["ATAN", "Return the arc tangent of the two arguments"],
        ["AVG", "Return the average value of the argument"],
        ["BENCHMARK", "Repeatedly execute an expression"],
        [
            "BETWEEN",
            "... AND ...       Whether a value is within a range of values",
        ],
        [
            "BIN",
            "Return a string containing binary representation of a number",
        ],
        ["BIN_TO_UUID", "Convert binary UUID to string"],
        ["BINARY", "Cast a string to a binary string   Yes"],
        ["BIT_AND", "Return bitwise AND"],
        ["BIT_COUNT", "Return the number of bits that are set"],
        ["BIT_LENGTH", "Return length of argument in bits"],
        ["BIT_OR", "Return bitwise OR"],
        ["BIT_XOR", "Return bitwise XOR"],
        ["CAN_ACCESS_COLUMN", "Internal use only"],
        ["CAN_ACCESS_DATABASE", "Internal use only"],
        ["CAN_ACCESS_TABLE", "Internal use only"],
        ["CAN_ACCESS_USER", "Internal use only"],
        ["CAN_ACCESS_VIEW", "Internal use only"],
        ["CASE", "Case operator"],
        ["CAST", "Cast a value as a certain type"],
        [
            "CEIL",
            "Return the smallest integer value not less than the argument",
        ],
        [
            "CEILING",
            "Return the smallest integer value not less than the argument",
        ],
        ["CHAR", "Return the character for each integer passed"],
        ["CHAR_LENGTH", "Return number of characters in argument"],
        ["CHARACTER_LENGTH", "Synonym for CHAR_LENGTH()"],
        ["CHARSET", "Return the character set of the argument"],
        ["COALESCE", "Return the first non-NULL argument"],
        [
            "COERCIBILITY",
            "Return the collation coercibility value of the string argument",
        ],
        ["COLLATION", "Return the collation of the string argument"],
        ["COMPRESS", "Return result as a binary string"],
        ["CONCAT", "Return concatenated string"],
        ["CONCAT_WS", "Return concatenate with separator"],
        [
            "CONNECTION_ID",
            "Return the connection ID (thread ID) for the connection",
        ],
        ["CONV", "Convert numbers between different number bases"],
        ["CONVERT", "Cast a value as a certain type"],
        ["CONVERT_TZ", "Convert from one time zone to another"],
        ["COS", "Return the cosine"],
        ["COT", "Return the cotangent"],
        ["COUNT", "Return a count of the number of rows returned"],
        [
            "COUNT(DISTINCT)",
            "Return the count of a number of different values",
        ],
        ["CRC32", "Compute a cyclic redundancy check value"],
        ["CUME_DIST", "Cumulative distribution value"],
        ["CURDATE", "Return the current date"],
        ["CURRENT_DATE", "Synonyms for CURDATE()"],
        ["CURRENT_DATE", "Synonyms for CURDATE()"],
        ["CURRENT_ROLE", "Return the current active roles"],
        ["CURRENT_TIME", "Synonyms for CURTIME()"],
        ["CURRENT_TIME", "Synonyms for CURTIME()"],
        ["CURRENT_TIMESTAMP", "Synonyms for NOW()"],
        ["CURRENT_TIMESTAMP", "Synonyms for NOW()"],
        ["CURRENT_USER", "The authenticated user name and host name"],
        ["CURRENT_USER", "The authenticated user name and host name"],
        ["CURTIME", "Return the current time"],
        ["DATABASE", "Return the default (current) database name"],
        [
            "DATE",
            "Extract the date part of a date or datetime expression",
        ],
        ["DATE_ADD", "Add time values (intervals) to a date value"],
        ["DATE_FORMAT", "Format date as specified"],
        ["DATE_SUB", "Subtract a time value (interval) from a date"],
        ["DATEDIFF", "Subtract two dates"],
        ["DAY", "Synonym for DAYOFMONTH()"],
        ["DAYNAME", "Return the name of the weekday"],
        ["DAYOFMONTH", "Return the day of the month (0-31)"],
        ["DAYOFWEEK", "Return the weekday index of the argument"],
        ["DAYOFYEAR", "Return the day of the year (1-366)"],
        ["DEFAULT", "Return the default value for a table column"],
        ["DEGREES", "Convert radians to degrees"],
        [
            "DENSE_RANK",
            "Rank of current row within its partition,without gaps",
        ],
        ["DIV", "Integer division"],
        ["ELT", "Return string at index number"],
        ["EXISTS", "Whether the result of a query contains any rows"],
        ["EXP", "Raise to the power of"],
        [
            "EXPORT_SET",
            "Return a string such that for every bit set in the value bits,you get an on string and for every unset bit,you get an off string",
        ],
        ["EXTRACT", "Extract part of a date"],
        [
            "ExtractValue",
            "Extract a value from an XML string using XPath notation",
        ],
        [
            "FIELD",
            "Index (position) of first argument in subsequent arguments",
        ],
        [
            "FIND_IN_SET",
            "Index (position) of first argument within second argument",
        ],
        [
            "FIRST_VALUE",
            "Value of argument from first row of window frame",
        ],
        [
            "FLOOR",
            "Return the largest integer value not greater than the argument",
        ],
        [
            "FORMAT",
            "Return a number formatted to specified number of decimal places",
        ],
        ["FORMAT_BYTES", "Convert byte count to value with units"],
        [
            "FORMAT_PICO_TIME",
            "Convert time in picoseconds to value with units",
        ],
        [
            "FOUND_ROWS",
            "For a SELECT with a LIMIT clause,the number of rows that would be returned were there no LIMIT clause",
        ],
        ["FROM_DAYS", "Convert a day number to a date"],
        ["FROM_UNIXTIME", "Format Unix timestamp as a date"],
        [
            "GeomCollection",
            "Construct geometry collection from geometries",
        ],
        [
            "GeometryCollection",
            "Construct geometry collection from geometries",
        ],
        ["GET_DD_COLUMN_PRIVILEGES", "Internal use only"],
        ["GET_DD_CREATE_OPTIONS", "Internal use only"],
        ["GET_DD_INDEX_SUB_PART_LENGTH", "Internal use only"],
        ["GET_FORMAT", "Return a date format string"],
        ["GET_LOCK", "Get a named lock"],
        ["GREATEST", "Return the largest argument"],
        ["GROUP_CONCAT", "Return a concatenated string"],
        [
            "group_replication_disable_member_action",
            "Disable member action for event specified",
        ],
        [
            "group_replication_enable_member_action",
            "Enable member action for event specified",
        ],
        [
            "group_replication_get_communication_protocol",
            "Get version of group replication communication protocol currently in use",
        ],
        [
            "group_replication_get_write_concurrency",
            "Get maximum number of consensus instances currently set for group",
        ],
        [
            "group_replication_reset_member_actions",
            "Reset all member actions to defaults and configuration version number to 1",
        ],
        [
            "group_replication_set_as_primary",
            "Make a specific group member the primary",
        ],
        [
            "group_replication_set_communication_protocol",
            "Set version for group replication communication protocol to use",
        ],
        [
            "group_replication_set_write_concurrency",
            "Set maximum number of consensus instances that can be executed in parallel",
        ],
        [
            "group_replication_switch_to_multi_primary_mode",
            "Changes the mode of a group running in single-primary mode to multi-primary mode",
        ],
        [
            "group_replication_switch_to_single_primary_mode",
            "Changes the mode of a group running in multi-primary mode to single-primary mode",
        ],
        [
            "GROUPING",
            "Distinguish super-aggregate ROLLUP rows from regular rows",
        ],
        [
            "HEX",
            "Hexadecimal representation of decimal or string value",
        ],
        ["HOUR", "Extract the hour"],
        ["ICU_VERSION", "ICU library version"],
        ["IF", "If/else construct"],
        ["IFNULL", "Null if/else construct"],
        ["IN", "Whether a value is within a set of values"],
        ["INET_ATON", "Return the numeric value of an IP address"],
        ["INET_NTOA", "Return the IP address from a numeric value"],
        [
            "INSERT",
            "Insert substring at specified position up to specified number of characters",
        ],
        [
            "INSTR",
            "Return the index of the first occurrence of substring",
        ],
        ["INTERNAL_AUTO_INCREMENT", "Internal use only"],
        ["INTERNAL_AVG_ROW_LENGTH", "Internal use only"],
        ["INTERNAL_CHECK_TIME", "Internal use only"],
        ["INTERNAL_CHECKSUM", "Internal use only"],
        ["INTERNAL_DATA_FREE", "Internal use only"],
        ["INTERNAL_DATA_LENGTH", "Internal use only"],
        ["INTERNAL_DD_CHAR_LENGTH", "Internal use only"],
        ["INTERNAL_GET_COMMENT_OR_ERROR", "Internal use only"],
        ["INTERNAL_GET_ENABLED_ROLE_JSON", "Internal use only"],
        ["INTERNAL_GET_HOSTNAME", "Internal use only"],
        ["INTERNAL_GET_USERNAME", "Internal use only"],
        ["INTERNAL_GET_VIEW_WARNING_OR_ERROR", "Internal use only"],
        ["INTERNAL_INDEX_COLUMN_CARDINALITY", "Internal use only"],
        ["INTERNAL_INDEX_LENGTH", "Internal use only"],
        ["INTERNAL_IS_ENABLED_ROLE", "Internal use only"],
        ["INTERNAL_IS_MANDATORY_ROLE", "Internal use only"],
        ["INTERNAL_KEYS_DISABLED", "Internal use only"],
        ["INTERNAL_MAX_DATA_LENGTH", "Internal use only"],
        ["INTERNAL_TABLE_ROWS", "Internal use only"],
        ["INTERNAL_UPDATE_TIME", "Internal use only"],
        [
            "INTERVAL",
            "Return the index of the argument that is less than the first argument",
        ],
        ["IS", "Test a value against a boolean"],
        ["IS_FREE_LOCK", "Whether the named lock is free"],
        ["IS", "NOT    Test a value against a boolean"],
        ["IS", "NOT NULL       NOT NULL value test"],
        ["IS", "NULL   NULL value test"],
        [
            "IS_USED_LOCK",
            "Whether the named lock is in use; return connection identifier if true",
        ],
        ["IS_UUID", "Whether argument is a valid UUID"],
        ["ISNULL", "Test whether the argument is NULL"],
        ["JSON_ARRAY", "Create JSON array"],
        ["JSON_ARRAY_APPEND", "Append data to JSON document"],
        ["JSON_ARRAY_INSERT", "Insert into JSON array"],
        ["JSON_ARRAYAGG", "Return result set as a single JSON array"],
        [
            "JSON_CONTAINS",
            "Whether JSON document contains specific object at path",
        ],
        [
            "JSON_CONTAINS_PATH",
            "Whether JSON document contains any data at path",
        ],
        ["JSON_DEPTH", "Maximum depth of JSON document"],
        ["JSON_EXTRACT", "Return data from JSON document"],
        ["JSON_INSERT", "Insert data into JSON document"],
        ["JSON_KEYS", "Array of keys from JSON document"],
        ["JSON_LENGTH", "Number of elements in JSON document"],
        [
            "JSON_MERGE",
            "Merge JSON documents,preserving duplicate keys. Deprecated synonym for JSON_MERGE_PRESERVE()   Yes",
        ],
        [
            "JSON_MERGE_PATCH",
            "Merge JSON documents,replacing values of duplicate keys",
        ],
        [
            "JSON_MERGE_PRESERVE",
            "Merge JSON documents,preserving duplicate keys",
        ],
        ["JSON_OBJECT", "Create JSON object"],
        [
            "JSON_OBJECTAGG",
            "Return result set as a single JSON object",
        ],
        [
            "JSON_OVERLAPS",
            "Compares two JSON documents,returns TRUE (1) if these have any key-value pairs or array elements in common,otherwise FALSE (0)",
        ],
        [
            "JSON_PRETTY",
            "Print a JSON document in human-readable format",
        ],
        ["JSON_QUOTE", "Quote JSON document"],
        ["JSON_REMOVE", "Remove data from JSON document"],
        ["JSON_REPLACE", "Replace values in JSON document"],
        [
            "JSON_SCHEMA_VALID",
            "Validate JSON document against JSON schema; returns TRUE/1 if document validates against schema,or FALSE/0 if it does not",
        ],
        [
            "JSON_SCHEMA_VALIDATION_REPORT",
            "Validate JSON document against JSON schema; returns report in JSON format on outcome on validation including success or failure and reasons for failure",
        ],
        ["JSON_SEARCH", "Path to value within JSON document"],
        ["JSON_SET", "Insert data into JSON document"],
        [
            "JSON_STORAGE_FREE",
            "Freed space within binary representation of JSON column value following partial update",
        ],
        [
            "JSON_STORAGE_SIZE",
            "Space used for storage of binary representation of a JSON document",
        ],
        [
            "JSON_TABLE",
            "Return data from a JSON expression as a relational table",
        ],
        ["JSON_TYPE", "Type of JSON value"],
        ["JSON_UNQUOTE", "Unquote JSON value"],
        ["JSON_VALID", "Whether JSON value is valid"],
        [
            "JSON_VALUE",
            "Extract value from JSON document at location pointed to by path provided; return this value as VARCHAR(512) or specified type",
        ],
        [
            "LAG",
            "Value of argument from row lagging current row within partition",
        ],
        [
            "LAST_DAY",
            "Return the last day of the month for the argument",
        ],
        [
            "LAST_INSERT_ID",
            "Value of the AUTOINCREMENT column for the last INSERT",
        ],
        [
            "LAST_VALUE",
            "Value of argument from last row of window frame",
        ],
        ["LCASE", "Synonym for LOWER()"],
        [
            "LEAD",
            "Value of argument from row leading current row within partition",
        ],
        ["LEAST", "Return the smallest argument"],
        [
            "LEFT",
            "Return the leftmost number of characters as specified",
        ],
        ["LENGTH", "Return the length of a string in bytes"],
        ["LIKE", "Simple pattern matching"],
        ["LineString", "Construct LineString from Point values"],
        ["LN", "Return the natural logarithm of the argument"],
        ["LOAD_FILE", "Load the named file"],
        ["LOCALTIME", "Synonym for NOW()"],
        ["LOCALTIME", "Synonym for NOW()"],
        ["LOCALTIMESTAMP", "Synonym for NOW()"],
        ["LOCALTIMESTAMP", "Synonym for NOW()"],
        [
            "LOCATE",
            "Return the position of the first occurrence of substring",
        ],
        ["LOG", "Return the natural logarithm of the first argument"],
        ["LOG10", "Return the base-10 logarithm of the argument"],
        ["LOG2", "Return the base-2 logarithm of the argument"],
        ["LOWER", "Return the argument in lowercase"],
        [
            "LPAD",
            "Return the string argument,left-padded with the specified string",
        ],
        ["LTRIM", "Remove leading spaces"],
        [
            "MAKE_SET",
            "Return a set of comma-separated strings that have the corresponding bit in bits set",
        ],
        ["MAKEDATE", "Create a date from the year and day of year"],
        ["MAKETIME", "Create time from hour,minute,second"],
        [
            "MASTER_POS_WAIT",
            "Block until the replica has read and applied all updates up to the specified position     Yes",
        ],
        ["MATCH", "Perform full-text search"],
        ["MAX", "Return the maximum value"],
        [
            "MBRContains",
            "Whether MBR of one geometry contains MBR of another",
        ],
        ["MBRCoveredBy", "Whether one MBR is covered by another"],
        ["MBRCovers", "Whether one MBR covers another"],
        ["MBRDisjoint", "Whether MBRs of two geometries are disjoint"],
        ["MBREquals", "Whether MBRs of two geometries are equal"],
        ["MBRIntersects", "Whether MBRs of two geometries intersect"],
        ["MBROverlaps", "Whether MBRs of two geometries overlap"],
        ["MBRTouches", "Whether MBRs of two geometries touch"],
        [
            "MBRWithin",
            "Whether MBR of one geometry is within MBR of another",
        ],
        ["MD5", "Calculate MD5 checksum"],
        [
            "MEMBER",
            "OF()       Returns true (1) if first operand matches any element of JSON array passed as second operand,otherwise returns false (0)",
        ],
        ["MICROSECOND", "Return the microseconds from argument"],
        [
            "MID",
            "Return a substring starting from the specified position",
        ],
        ["MIN", "Return the minimum value"],
        ["MINUTE", "Return the minute from the argument"],
        ["MOD", "Return the remainder"],
        ["MONTH", "Return the month from the date passed"],
        ["MONTHNAME", "Return the name of the month"],
        [
            "MultiLineString",
            "Contruct MultiLineString from LineString values",
        ],
        ["MultiPoint", "Construct MultiPoint from Point values"],
        ["MultiPolygon", "Construct MultiPolygon from Polygon values"],
        ["NAME_CONST", "Cause the column to have the given name"],
        ["NOT", "Negates value"],
        ["!", "Negates value"],
        [
            "NOT",
            "BETWEEN ... AND ...   Whether a value is not within a range of values",
        ],
        [
            "NOT",
            "EXISTS()      Whether the result of a query contains no rows",
        ],
        ["NOT", "IN()  Whether a value is not within a set of values"],
        ["NOT", "LIKE  Negation of simple pattern matching"],
        ["NOT", "REGEXP        Negation of REGEXP"],
        ["NOW", "Return the current date and time"],
        [
            "NTH_VALUE",
            "Value of argument from N-th row of window frame",
        ],
        [
            "NTILE",
            "Bucket number of current row within its partition.",
        ],
        ["NULLIF", "Return NULL if expr1 = expr2"],
        [
            "OCT",
            "Return a string containing octal representation of a number",
        ],
        ["OCTET_LENGTH", "Synonym for LENGTH()"],
        ["OR", "Logical OR"],
        ["||", "Logical OR"],
        [
            "ORD",
            "Return character code for leftmost character of the argument",
        ],
        ["PERCENT_RANK", "Percentage rank value"],
        ["PERIOD_ADD", "Add a period to a year-month"],
        ["PERIOD_DIFF", "Return the number of months between periods"],
        ["PI", "Return the value of pi"],
        ["Point", "Construct Point from coordinates"],
        ["Polygon", "Construct Polygon from LineString arguments"],
        ["POSITION", "Synonym for LOCATE()"],
        ["POW", "Return the argument raised to the specified power"],
        ["POWER", "Return the argument raised to the specified power"],
        [
            "PS_CURRENT_THREAD_ID",
            "Performance Schema thread ID for current thread",
        ],
        [
            "PS_THREAD_ID",
            "Performance Schema thread ID for given thread",
        ],
        ["QUARTER", "Return the quarter from a date argument"],
        ["QUOTE", "Escape the argument for use in an SQL statement"],
        ["RADIANS", "Return argument converted to radians"],
        ["RAND", "Return a random floating-point value"],
        ["RANDOM_BYTES", "Return a random byte vector"],
        ["RANK", "Rank of current row within its partition,with gaps"],
        ["REGEXP", "Whether string matches regular expression"],
        [
            "REGEXP_INSTR",
            "Starting index of substring matching regular expression",
        ],
        ["REGEXP_LIKE", "Whether string matches regular expression"],
        [
            "REGEXP_REPLACE",
            "Replace substrings matching regular expression",
        ],
        [
            "REGEXP_SUBSTR",
            "Return substring matching regular expression",
        ],
        ["RELEASE_ALL_LOCKS", "Release all current named locks"],
        ["RELEASE_LOCK", "Release the named lock"],
        ["REPEAT", "Repeat a string the specified number of times"],
        ["REPLACE", "Replace occurrences of a specified string"],
        ["REVERSE", "Reverse the characters in a string"],
        [
            "RIGHT",
            "Return the specified rightmost number of characters",
        ],
        ["RLIKE", "Whether string matches regular expression"],
        [
            "ROLES_GRAPHML",
            "Return a GraphML document representing memory role subgraphs",
        ],
        ["ROUND", "Round the argument"],
        ["ROW_COUNT", "The number of rows updated"],
        ["ROW_NUMBER", "Number of current row within its partition"],
        ["RPAD", "Append string the specified number of times"],
        ["RTRIM", "Remove trailing spaces"],
        ["SCHEMA", "Synonym for DATABASE()"],
        ["SEC_TO_TIME", "Converts seconds to 'hh:mm:ss' format"],
        ["SECOND", "Return the second (0-59)"],
        ["SESSION_USER", "Synonym for USER()"],
        ["SHA1", "Calculate an SHA-1 160-bit checksum"],
        ["SHA", "Calculate an SHA-1 160-bit checksum"],
        ["SHA2", "Calculate an SHA-2 checksum"],
        ["SIGN", "Return the sign of the argument"],
        ["SIN", "Return the sine of the argument"],
        ["SLEEP", "Sleep for a number of seconds"],
        ["SOUNDEX", "Return a soundex string"],
        ["SOUNDS", "LIKE       Compare sounds"],
        [
            "SOURCE_POS_WAIT",
            "Block until the replica has read and applied all updates up to the specified position",
        ],
        ["SPACE", "Return a string of the specified number of spaces"],
        ["SQRT", "Return the square root of the argument"],
        ["ST_Area", "Return Polygon or MultiPolygon area"],
        [
            "ST_AsBinary",
            "Convert from internal geometry format to WKB",
        ],
        ["ST_AsWKB", "Convert from internal geometry format to WKB"],
        ["ST_AsGeoJSON", "Generate GeoJSON object from geometry"],
        ["ST_AsText", "Convert from internal geometry format to WKT"],
        ["ST_AsWKT", "Convert from internal geometry format to WKT"],
        [
            "ST_Buffer",
            "Return geometry of points within given distance from geometry",
        ],
        [
            "ST_Buffer_Strategy",
            "Produce strategy option for ST_Buffer()",
        ],
        ["ST_Centroid", "Return centroid as a point"],
        ["ST_Collect", "Aggregate spatial values into collection"],
        ["ST_Contains", "Whether one geometry contains another"],
        ["ST_ConvexHull", "Return convex hull of geometry"],
        ["ST_Crosses", "Whether one geometry crosses another"],
        [
            "ST_Difference",
            "Return point set difference of two geometries",
        ],
        ["ST_Dimension", "Dimension of geometry"],
        [
            "ST_Disjoint",
            "Whether one geometry is disjoint from another",
        ],
        ["ST_Distance", "The distance of one geometry from another"],
        [
            "ST_Distance_Sphere",
            "Minimum distance on earth between two geometries",
        ],
        ["ST_EndPoint", "End Point of LineString"],
        ["ST_Envelope", "Return MBR of geometry"],
        ["ST_Equals", "Whether one geometry is equal to another"],
        ["ST_ExteriorRing", "Return exterior ring of Polygon"],
        [
            "ST_FrechetDistance",
            "The discrete Fréchet distance of one geometry from another",
        ],
        ["ST_GeoHash", "Produce a geohash value"],
        ["ST_GeomCollFromText", "Return geometry collection from WKT"],
        [
            "ST_GeometryCollectionFromText",
            "Return geometry collection from WKT",
        ],
        ["ST_GeomCollFromTxt", "Return geometry collection from WKT"],
        ["ST_GeomCollFromWKB", "Return geometry collection from WKB"],
        [
            "ST_GeometryCollectionFromWKB",
            "Return geometry collection from WKB",
        ],
        [
            "ST_GeometryN",
            "Return N-th geometry from geometry collection",
        ],
        ["ST_GeometryType", "Return name of geometry type"],
        [
            "ST_GeomFromGeoJSON",
            "Generate geometry from GeoJSON object",
        ],
        ["ST_GeomFromText", "Return geometry from WKT"],
        ["ST_GeometryFromText", "Return geometry from WKT"],
        ["ST_GeomFromWKB", "Return geometry from WKB"],
        ["ST_GeometryFromWKB", "Return geometry from WKB"],
        [
            "ST_HausdorffDistance",
            "The discrete Hausdorff distance of one geometry from another",
        ],
        ["ST_InteriorRingN", "Return N-th interior ring of Polygon"],
        [
            "ST_Intersection",
            "Return point set intersection of two geometries",
        ],
        ["ST_Intersects", "Whether one geometry intersects another"],
        ["ST_IsClosed", "Whether a geometry is closed and simple"],
        ["ST_IsEmpty", "Whether a geometry is empty"],
        ["ST_IsSimple", "Whether a geometry is simple"],
        ["ST_IsValid", "Whether a geometry is valid"],
        ["ST_LatFromGeoHash", "Return latitude from geohash value"],
        ["ST_Latitude", "Return latitude of Point"],
        ["ST_Length", "Return length of LineString"],
        ["ST_LineFromText", "Construct LineString from WKT"],
        ["ST_LineStringFromText", "Construct LineString from WKT"],
        ["ST_LineFromWKB", "Construct LineString from WKB"],
        ["ST_LineStringFromWKB", "Construct LineString from WKB"],
        [
            "ST_LineInterpolatePoint",
            "The point a given percentage along a LineString",
        ],
        [
            "ST_LineInterpolatePoints",
            "The points a given percentage along a LineString",
        ],
        ["ST_LongFromGeoHash", "Return longitude from geohash value"],
        ["ST_Longitude", "Return longitude of Point"],
        ["ST_MakeEnvelope", "Rectangle around two points"],
        ["ST_MLineFromText", "Construct MultiLineString from WKT"],
        [
            "ST_MultiLineStringFromText",
            "Construct MultiLineString from WKT",
        ],
        ["ST_MLineFromWKB", "Construct MultiLineString from WKB"],
        [
            "ST_MultiLineStringFromWKB",
            "Construct MultiLineString from WKB",
        ],
        ["ST_MPointFromText", "Construct MultiPoint from WKT"],
        ["ST_MultiPointFromText", "Construct MultiPoint from WKT"],
        ["ST_MPointFromWKB", "Construct MultiPoint from WKB"],
        ["ST_MultiPointFromWKB", "Construct MultiPoint from WKB"],
        ["ST_MPolyFromText", "Construct MultiPolygon from WKT"],
        ["ST_MultiPolygonFromText", "Construct MultiPolygon from WKT"],
        ["ST_MPolyFromWKB", "Construct MultiPolygon from WKB"],
        ["ST_MultiPolygonFromWKB", "Construct MultiPolygon from WKB"],
        [
            "ST_NumGeometries",
            "Return number of geometries in geometry collection",
        ],
        [
            "ST_NumInteriorRing",
            "Return number of interior rings in Polygon",
        ],
        [
            "ST_NumInteriorRings",
            "Return number of interior rings in Polygon",
        ],
        ["ST_NumPoints", "Return number of points in LineString"],
        ["ST_Overlaps", "Whether one geometry overlaps another"],
        [
            "ST_PointAtDistance",
            "The point a given distance along a LineString",
        ],
        [
            "ST_PointFromGeoHash",
            "Convert geohash value to POINT value",
        ],
        ["ST_PointFromText", "Construct Point from WKT"],
        ["ST_PointFromWKB", "Construct Point from WKB"],
        ["ST_PointN", "Return N-th point from LineString"],
        ["ST_PolyFromText", "Construct Polygon from WKT"],
        ["ST_PolygonFromText", "Construct Polygon from WKT"],
        ["ST_PolyFromWKB", "Construct Polygon from WKB"],
        ["ST_PolygonFromWKB", "Construct Polygon from WKB"],
        ["ST_Simplify", "Return simplified geometry"],
        ["ST_SRID", "Return spatial reference system ID for geometry"],
        ["ST_StartPoint", "Start Point of LineString"],
        ["ST_SwapXY", "Return argument with X/Y coordinates swapped"],
        [
            "ST_SymDifference",
            "Return point set symmetric difference of two geometries",
        ],
        ["ST_Touches", "Whether one geometry touches another"],
        ["ST_Transform", "Transform coordinates of geometry"],
        ["ST_Union", "Return point set union of two geometries"],
        ["ST_Validate", "Return validated geometry"],
        ["ST_Within", "Whether one geometry is within another"],
        ["ST_X", "Return X coordinate of Point"],
        ["ST_Y", "Return Y coordinate of Point"],
        ["STATEMENT_DIGEST", "Compute statement digest hash value"],
        [
            "STATEMENT_DIGEST_TEXT",
            "Compute normalized statement digest",
        ],
        ["STD", "Return the population standard deviation"],
        ["STDDEV", "Return the population standard deviation"],
        ["STDDEV_POP", "Return the population standard deviation"],
        ["STDDEV_SAMP", "Return the sample standard deviation"],
        ["STR_TO_DATE", "Convert a string to a date"],
        ["STRCMP", "Compare two strings"],
        [
            "SUBDATE",
            "Synonym for DATE_SUB() when invoked with three arguments",
        ],
        ["SUBSTR", "Return the substring as specified"],
        ["SUBSTRING", "Return the substring as specified"],
        [
            "SUBSTRING_INDEX",
            "Return a substring from a string before the specified number of occurrences of the delimiter",
        ],
        ["SUBTIME", "Subtract times"],
        ["SUM", "Return the sum"],
        ["SYSDATE", "Return the time at which the function executes"],
        ["SYSTEM_USER", "Synonym for USER()"],
        ["TAN", "Return the tangent of the argument"],
        ["TIME", "Extract the time portion of the expression passed"],
        ["TIME_FORMAT", "Format as time"],
        ["TIME_TO_SEC", "Return the argument converted to seconds"],
        ["TIMEDIFF", "Subtract time"],
        [
            "TIMESTAMP",
            "With a single argument,this function returns the date or datetime expression; with two arguments,the sum of the arguments",
        ],
        ["TIMESTAMPADD", "Add an interval to a datetime expression"],
        [
            "TIMESTAMPDIFF",
            "Return the difference of two datetime expressions,using the units specified",
        ],
        ["TO_DAYS", "Return the date argument converted to days"],
        [
            "TO_SECONDS",
            "Return the date or datetime argument converted to seconds since Year 0",
        ],
        ["TRIM", "Remove leading and trailing spaces"],
        ["TRUNCATE", "Truncate to specified number of decimal places"],
        ["UCASE", "Synonym for UPPER()"],
        ["UNCOMPRESS", "Uncompress a string compressed"],
        [
            "UNCOMPRESSED_LENGTH",
            "Return the length of a string before compression",
        ],
        [
            "UNHEX",
            "Return a string containing hex representation of a number",
        ],
        ["UNIX_TIMESTAMP", "Return a Unix timestamp"],
        ["UpdateXML", "Return replaced XML fragment"],
        ["UPPER", "Convert to uppercase"],
        ["USER", "The user name and host name provided by the client"],
        ["UTC_DATE", "Return the current UTC date"],
        ["UTC_TIME", "Return the current UTC time"],
        ["UTC_TIMESTAMP", "Return the current UTC date and time"],
        ["UUID", "Return a Universal Unique Identifier (UUID)"],
        [
            "UUID_SHORT",
            "Return an integer-valued universal identifier",
        ],
        ["UUID_TO_BIN", "Convert string UUID to binary"],
        [
            "VALIDATE_PASSWORD_STRENGTH",
            "Determine strength of password",
        ],
        ["VALUES", "Define the values to be used during an INSERT"],
        ["VAR_POP", "Return the population standard variance"],
        ["VAR_SAMP", "Return the sample variance"],
        ["VARIANCE", "Return the population standard variance"],
        [
            "VERSION",
            "Return a string that indicates the MySQL server version",
        ],
        [
            "WAIT_FOR_EXECUTED_GTID_SET",
            "Wait until the given GTIDs have executed on the replica.",
        ],
        ["WEEK", "Return the week number"],
        ["WEEKDAY", "Return the weekday index"],
        ["WEEKOFYEAR", "Return the calendar week of the date (1-53)"],
        ["WEIGHT_STRING", "Return the weight string for a string"],
        ["XOR", "Logical XOR"],
        ["YEAR", "Return the year"],
        ["YEARWEEK", "Return the year and week"],
        [
            "asymmetric_decrypt",
            "Decrypt ciphertext using private or public key",
        ],
        [
            "asymmetric_derive",
            "Derive symmetric key from asymmetric keys",
        ],
        [
            "asymmetric_encrypt",
            "Encrypt cleartext using private or public key",
        ],
        ["asymmetric_sign", "Generate signature from digest"],
        ["asymmetric_verify", "Verify that signature matches digest"],
        [
            "asynchronous_connection_failover_add_managed",
            "Add a replication source server in a managed group to the source list",
        ],
        [
            "asynchronous_connection_failover_add_source",
            "Add a replication source server to the source list",
        ],
        [
            "asynchronous_connection_failover_delete_managed",
            "Remove managed group of replication source servers from the source list",
        ],
        [
            "asynchronous_connection_failover_delete_source",
            "Remove a replication source server from the source list",
        ],
        [
            "audit_api_message_emit_udf",
            "Add message event to audit log",
        ],
        [
            "audit_log_encryption_password_get",
            "Fetch audit log encryption password",
        ],
        [
            "audit_log_encryption_password_set",
            "Set audit log encryption password",
        ],
        ["audit_log_filter_flush", "Flush audit log filter tables"],
        ["audit_log_filter_remove_filter", "Remove audit log filter"],
        [
            "audit_log_filter_remove_user",
            "Unassign audit log filter from user",
        ],
        ["audit_log_filter_set_filter", "Define audit log filter"],
        [
            "audit_log_filter_set_user",
            "Assign audit log filter to user",
        ],
        ["audit_log_read", "Return audit log records"],
        [
            "audit_log_read_bookmark",
            "Bookmark for most recent audit log event",
        ],
        ["audit_log_rotate", "Rotate audit log file"],
        ["create_asymmetric_priv_key", "Create private key"],
        ["create_asymmetric_pub_key", "Create public key"],
        ["create_dh_parameters", "Generate shared DH secret"],
        ["create_digest", "Generate digest from string"],
        [
            "firewall_group_delist",
            "Remove account from firewall group profile",
        ],
        [
            "firewall_group_enlist",
            "Add account to firewall group profile",
        ],
        [
            "flush_rewrite_rules",
            "Load rewrite_rules table into Rewriter cache",
        ],
        [
            "gen_blacklist",
            "Perform dictionary term replacement         Yes",
        ],
        ["gen_blocklist", "Perform dictionary term replacement"],
        ["gen_blocklist", "Perform dictionary term replacement"],
        ["gen_dictionary", "Return random term from dictionary"],
        ["gen_dictionary_drop", "Remove dictionary from registry"],
        ["gen_dictionary_load", "Load dictionary into registry"],
        ["gen_dictionary", "Return random term from dictionary"],
        ["gen_range", "Generate random number within range"],
        ["gen_range", "Generate random number within range"],
        [
            "gen_rnd_canada_sin",
            "Generate random Canada Social Insurance Number",
        ],
        ["gen_rnd_email", "Generate random email address"],
        ["gen_rnd_email", "Generate random email address"],
        [
            "gen_rnd_iban",
            "Generate random International Bank Account Number",
        ],
        [
            "gen_rnd_pan",
            "Generate random payment card Primary Account Number",
        ],
        [
            "gen_rnd_pan",
            "Generate random payment card Primary Account Number",
        ],
        ["gen_rnd_ssn", "Generate random US Social Security Number"],
        ["gen_rnd_ssn", "Generate random US Social Security Number"],
        [
            "gen_rnd_uk_nin",
            "Generate random United Kingdom National Insurance Number",
        ],
        ["gen_rnd_us_phone", "Generate random US phone number"],
        ["gen_rnd_us_phone", "Generate random US phone number"],
        [
            "gen_rnd_uuid",
            "Generate random Universally Unique Identifier",
        ],
        [
            "group_replication_disable_member_action",
            "Enable a member action so that the member does not take it in the specified situation",
        ],
        [
            "group_replication_enable_member_action",
            "Enable a member action for the member to take in the specified situation",
        ],
        [
            "group_replication_get_communication_protocol",
            "Return Group Replication protocol version",
        ],
        [
            "group_replication_get_write_concurrency",
            "Return maximum number of consensus instances executable in parallel",
        ],
        [
            "group_replication_reset_member_actions",
            "Reset the member actions configuration to the default settings",
        ],
        [
            "group_replication_set_as_primary",
            "Assign group member as new primary",
        ],
        [
            "group_replication_set_communication_protocol",
            "Set Group Replication protocol version",
        ],
        [
            "group_replication_set_write_concurrency",
            "Set maximum number of consensus instances executable in parallel",
        ],
        [
            "group_replication_switch_to_multi_primary_mode",
            "Change group from single-primary to multi-primary mode",
        ],
        [
            "group_replication_switch_to_single_primary_mode",
            "Change group from multi-primary to single-primary mode",
        ],
        ["keyring_aws_rotate_cmk", "Rotate AWS customer master key"],
        [
            "keyring_aws_rotate_keys",
            "Rotate keys in keyring_aws storage file",
        ],
        [
            "keyring_hashicorp_update_config",
            "Cause runtime keyring_hashicorp reconfiguration",
        ],
        ["keyring_key_fetch", "Fetch keyring key value"],
        ["keyring_key_generate", "Generate random keyring key"],
        ["keyring_key_length_fetch", "Return keyring key length"],
        ["keyring_key_remove", "Remove keyring key"],
        ["keyring_key_store", "Store key in keyring"],
        ["keyring_key_type_fetch", "Return keyring key type"],
        ["load_rewrite_rules", "Rewriter plugin helper routine"],
        ["mask_canada_sin", "Mask Canada Social Insurance Number"],
        ["mask_iban", "Mask International Bank Account Number"],
        ["mask_inner", "Mask interior part of string"],
        ["mask_inner", "Mask interior part of string"],
        ["mask_outer", "Mask left and right parts of string"],
        ["mask_outer", "Mask left and right parts of string"],
        [
            "mask_pan",
            "Mask payment card Primary Account Number part of string",
        ],
        [
            "mask_pan",
            "Mask payment card Primary Account Number part of string",
        ],
        [
            "mask_pan_relaxed",
            "Mask payment card Primary Account Number part of string",
        ],
        [
            "mask_pan_relaxed",
            "Mask payment card Primary Account Number part of string",
        ],
        ["mask_ssn", "Mask US Social Security Number"],
        ["mask_ssn", "Mask US Social Security Number"],
        [
            "mask_uk_nin",
            "Mask United Kingdom National Insurance Number",
        ],
        [
            "mask_uuid",
            "Mask Universally Unique Identifier part of string",
        ],
        [
            "masking_dictionaries_flush",
            "Cause masking_dictionaries cache to be reloaded from table",
        ],
        [
            "masking_dictionary_remove",
            "Remove dictionary from the database table",
        ],
        [
            "masking_dictionary_term_add",
            "Add new term to the dictionary",
        ],
        [
            "masking_dictionary_term_remove",
            "Remove existing term from the dictionary",
        ],
        [
            "mysql_firewall_flush_status",
            "Reset firewall status variables",
        ],
        [
            "mysql_query_attribute_string",
            "Fetch query attribute value",
        ],
        [
            "normalize_statement",
            "Normalize SQL statement to digest form",
        ],
        [
            "read_firewall_group_allowlist",
            "Update firewall group profile recorded-statement cache",
        ],
        [
            "read_firewall_groups",
            "Update firewall group profile cache",
        ],
        [
            "read_firewall_users",
            "Update firewall account profile cache         Yes",
        ],
        [
            "read_firewall_whitelist",
            "Update firewall account profile recorded-statement cache  Yes",
        ],
        [
            "service_get_read_locks",
            "Acquire locking service shared locks",
        ],
        [
            "service_get_write_locks",
            "Acquire locking service exclusive locks",
        ],
        ["service_release_locks", "Release locking service locks"],
        [
            "set_firewall_group_mode",
            "Establish firewall group profile operational mode",
        ],
        [
            "set_firewall_mode",
            "Establish firewall account profile operational mode     Yes",
        ],
        [
            "version_tokens_delete",
            "Delete tokens from version tokens list",
        ],
        ["version_tokens_edit", "Modify version tokens list"],
        [
            "version_tokens_lock_exclusive",
            "Acquire exclusive locks on version tokens",
        ],
        [
            "version_tokens_lock_shared",
            "Acquire shared locks on version tokens",
        ],
        ["version_tokens_set", "Set version tokens list"],
        ["version_tokens_show", "Return version tokens list"],
        ["version_tokens_unlock", "Release version tokens locks"],
    ];

    lazy_static::lazy_static! {
        static ref CS : Vec<CompletionItem> = SS.iter().map(|x| CompletionItem {
            item : Cow::Borrowed(x[0]),
            kind : CompletionKind::Pragma,
            desc: Some( x[1].into()),
        }).collect();
    }
    CS.clone()
}

#[cfg(test)]
#[test]
fn mysql_fn() {
    let s = include_str!("./mysql_fn_str");
    for line in s.lines() {
        if line.trim() == "" {
            continue;
        }
        let (cmd, desc) = line.split_once(&[' ', '\t']).unwrap();
        let cmd = cmd.trim();
        let desc = desc.trim();
        let cmds: Vec<_> = cmd.split(',').collect();

        for cmd in cmds.iter() {
            let c = if cmd.ends_with("()") {
                &cmd[..cmd.len() - 2]
            } else {
                cmd
            };
            println!("[\"{}\" , \"{}\" ],", c, desc);
        }
    }
}
