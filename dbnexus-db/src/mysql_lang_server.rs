/*

MySQL 只允许省略 sys. 前缀直接调用“函数”，对“存储过程（PROCEDURE）”没有开同样的绿灯。
换言之：
对表（table）和视图（view）更没有“绿灯”——
它们必须用“库名.对象名”或先 USE 库名 再裸写对象名，否则就报 ERROR 1146 (42S02): Table 'db.tbl' doesn't exist。
一句话：
函数（function）：sys 独享全局免前缀。
过程（procedure）、表（table）、视图（view）：一律没有特权，想跨库访问就老老实实加前缀或切换默认库。
*/

use crate::db::{name_start_ascii_alphabetic, BackendKindMysql, SqlEvent};
use crate::language_server::{
    try_parser_dot_lexer, AstCompletionHelper, CompletionItem, CompletionKind, CompletionList,
    DBExecutable, ErrAndLoc, GetKeyWords, LangServerCommon, LanguageServer, LexerCompletionTrait,
};
use crate::mariadb::MARIADB_FUNCTIONS;
use crate::mysql::MysqlDB;
use crate::sqlparser_completion::TempTableColumn;
use crate::tidb::TIDB_KEYWORDS;
use sqlx::{AssertSqlSafe, Row, SqlSafeStr};
use std::collections::HashSet;
use std::ops::DerefMut;
use tokio::sync::Mutex;

use sqlx::{Column, Executor, Statement};
#[async_trait::async_trait]
impl LexerCompletionTrait for MysqlDB {
    async fn get_types(&self) -> anyhow::Result<Vec<String>> {
        return anyhow::Ok(MYSQL_TYPES.iter().map(|x| x.to_string()).collect());
    }
}

#[async_trait::async_trait]
impl GetKeyWords for MysqlDB {
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        static KEY_WORDS_CACHE: Mutex<Vec<String>> = Mutex::const_new(Vec::new());
        match self.kind {
            BackendKindMysql::Tidb => {
                let result = TIDB_KEYWORDS
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>();
                anyhow::Ok(result)
            }
            _ => {
                let mut lock = KEY_WORDS_CACHE.lock().await;
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
                        ret.push(name);
                    }
                }
                *lock = ret;
                return anyhow::Ok(lock.clone());
            }
        }
    }
}

#[async_trait::async_trait]
impl LanguageServer for MysqlDB {
    async fn get_search_path(&self, query_id: &str) -> anyhow::Result<HashSet<String>> {
        match self.query_connection.get(&query_id.to_string()) {
            Some(x) => {
                let mut conn = x.lock().await;
                let mut r = HashSet::new();
                if let Some(x) = self.query_select_database(conn.deref_mut()).await? {
                    r.insert(x.to_string());
                }
                anyhow::Ok(r)
            }
            None => anyhow::bail!(format!("query_connection not found , id :{}", query_id)),
        }
    }

    async fn try_get_max_id(&self, name: &str) -> anyhow::Result<String> {
        let sql = format!("SELECT MAX(id) FROM `{name}`");
        let mut conn = self.get_conn().await?;
        let row = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_one(conn.deref_mut())
            .await?;
        let id: Option<i64> = row.try_get_unchecked(0)?;
        anyhow::Ok(id.unwrap_or_default().to_string())
    }
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
        query_id: Option<String>,
        sql: &str,
        line: u32,
        col: u32,
        use_lexer: bool,
    ) -> anyhow::Result<CompletionList> {
        self.completion_id.set_optional(query_id);
        try_parser_dot_lexer(self, sql, line, col, use_lexer).await
    }

    async fn check_sql(&self, sql: &str) -> anyhow::Result<Option<ErrAndLoc>> {
        LangServerCommon::parse_err(self, sql)
    }
    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        LangServerCommon::is_dangerous_sql_by_parser(self, sql)
    }
}

impl MysqlDB {
    const SELECT_DB_SQL: &str = "SELECT DATABASE()";
}

impl MysqlDB {
    pub(crate) async fn query_select_database(
        &self,
        mut conn: &mut sqlx::MySqlConnection,
    ) -> anyhow::Result<Option<String>> {
        let sql = Self::SELECT_DB_SQL;
        let row = sqlx::raw_sql(sql).fetch_one(conn.deref_mut()).await.ok();
        let r = match row {
            Some(row) => {
                let name: Option<String> = row.try_get(0).ok().flatten();
                name
            }
            None => None,
        };
        anyhow::Ok(r)
    }
    pub(crate) async fn merge_schema(&self, schema: Option<&str>) -> Option<String> {
        if schema.is_some() {
            return schema.map(|x| x.to_string());
        }
        // check if user use `use` to select database
        let used_db = match self.query_connection.get(&self.completion_id.get()) {
            Some(x) => {
                let mut conn = x.lock().await;
                let sql = Self::SELECT_DB_SQL;
                let row = sqlx::raw_sql(sql).fetch_one(conn.deref_mut()).await.ok();
                match row {
                    Some(row) => {
                        let name: Option<String> = row.try_get(0).ok().flatten();
                        name
                    }
                    None => None,
                }
            }
            None => None,
        };
        if used_db.is_some() {
            return used_db;
        }
        self.config.db.as_ref().map(|x| x.to_string())
    }

    pub(crate) async fn not_allow_empty_schema(
        &self,
        schema: Option<&str>,
    ) -> anyhow::Result<String> {
        match self.merge_schema(schema).await {
            Some(x) => anyhow::Ok(x),
            None => anyhow::bail!("empty schema"),
        }
    }
}

#[async_trait::async_trait]
impl LangServerCommon for MysqlDB {
    async fn list_event(&self, schema: Option<&str>) -> anyhow::Result<Vec<SqlEvent>> {
        let schema = self.not_allow_empty_schema(schema).await?;
        self.list_event_impl(&schema).await
    }

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = self.not_allow_empty_schema(schema).await?;
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
        let conn = match self.query_connection.get(&self.completion_id.get()) {
            Some(x) => x.clone(),
            None => anyhow::bail!("should be able to get query connection"),
        };
        let mut conn = conn.lock().await;
        let mut ret = HashSet::new();

        {
            let sql = "show variables";
            let rows = sqlx::raw_sql(AssertSqlSafe(sql))
                .fetch_all(conn.deref_mut())
                .await?;

            for r in rows.iter() {
                let name: String = r.try_get(0)?;
                ret.insert(name);
            }
        }
        {
            let sql = "show global variables";
            let rows = sqlx::raw_sql(AssertSqlSafe(sql))
                .fetch_all(conn.deref_mut())
                .await?;
            for r in rows.iter() {
                let name: String = r.try_get(0)?;
                ret.insert(name);
            }
        }
        anyhow::Ok(ret.into_iter().collect())
    }

    async fn list_user(&self) -> anyhow::Result<Vec<String>> {
        let sql = "SELECT  DISTINCT  user FROM mysql.user";
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());

        for r in rows {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_role(&self) -> anyhow::Result<Vec<String>> {
        LangServerCommon::list_user(self).await
    }

    async fn list_column(
        &self,
        schema: Option<&str>,
        name: &str,
        is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = if let Some(args) = is_call {
            format!(
                r#"select {}`{}`({})"#,
                match schema {
                    Some(x) => format!("`{}`.", x),
                    None => "".to_string(),
                },
                name,
                args.join(","),
            )
        } else {
            format!(
                r#"select * from {}`{}` limit 1"#,
                match schema {
                    Some(x) => format!("`{}`.", x),
                    None => "".to_string(),
                },
                name,
            )
        };
        log::info!("sql:{}", sql);
        self.get_query_columns(&sql).await
    }

    async fn list_function(
        &self,
        schema: Option<&str>,
        include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        match self.kind {
            BackendKindMysql::Tidb => {
                if !include_build_in {
                    return anyhow::Ok(vec![]);
                }
                let mut ret = vec![];
                let mut conn = self.get_conn().await?;
                let rows = sqlx::raw_sql("SHOW BUILTINS;")
                    .fetch_all(conn.deref_mut())
                    .await?;
                for row in rows.iter() {
                    let name: Option<String> = row.try_get(0)?;
                    if let Some(name) = name {
                        ret.push(CompletionItem {
                            item: name.into(),
                            kind: CompletionKind::Function,
                            desc: None,
                        });
                    }
                }
                anyhow::Ok(ret)
            }
            _ => {
                let schema = self.merge_schema(schema).await;
                let mut conn = self.get_conn().await?;
                let mut ret = if let Some(schema) = &schema {
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
                    ret.extend(
                        if self.kind == BackendKindMysql::Mysql {
                            MYSQL_FUNCTIONS
                        } else {
                            MARIADB_FUNCTIONS
                        }
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
        }
    }

    async fn list_procedure(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        /*
        mysql在数据库test可以直接调用sys的procedure 不使用sys这个前缀
        编辑
        复制
        不行。
        MySQL 只允许省略 sys. 前缀直接调用“函数”，对“存储过程（PROCEDURE）”没有开同样的绿灯。
        换言之：
        */
        let schema = self.not_allow_empty_schema(schema).await?;
        let mut conn = self.get_conn().await?;
        crate::mysql::FunctionOrProcedure::query_list(
            crate::mysql::FunctionOrProcedure::Procedure,
            conn.deref_mut(),
            &schema,
        )
        .await
    }
    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = self.not_allow_empty_schema(schema).await?;
        let mut ret: Vec<String> = self
            .list_table_impl(&schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())?;

        if self.kind == BackendKindMysql::Mariadb {
            let sql = format!(
        "select SEQUENCE_NAME  from information_schema.SEQUENCES where SEQUENCE_SCHEMA = '{}'",schema );
            let mut conn = self.get_conn().await?;
            let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .fetch_all(conn.deref_mut())
                .await?;
            for row in rows.iter() {
                let name: String = row.try_get(0)?;
                ret.push(name);
            }
        }
        anyhow::Ok(ret)
    }

    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = self.not_allow_empty_schema(schema).await?;
        self.list_view_impl(&schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

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
            SELECT statement_id, sql_text ,statement_name
            FROM performance_schema.prepared_statements_instances WHERE owner_thread_id = connection_id() and  statement_name is not null;
        */

        let conn = self.query_connection.get(&self.completion_id.get());
        let conn = match conn {
            Some(x) => x,
            None => anyhow::bail!("connection not found"),
        };
        let mut conn = conn.lock().await;
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
    async fn mysql_show_create_trigger(
        &self,
        schema: Option<&str>,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let schema = self.not_allow_empty_schema(schema).await?;
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
                data_type: Some(x.type_info().to_string()),
            })
            .collect::<Vec<_>>();
        anyhow::Ok(ret)
    }

    async fn list_partition(
        &self,
        schema: Option<&str>,
        table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let schema = self.not_allow_empty_schema(schema).await?;
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

    async fn list_constraint(
        &self,
        schema: Option<&str>,
        table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        let schema = self.not_allow_empty_schema(schema).await?;
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
        &self,
        schema: Option<&str>,
        table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let schema = self.not_allow_empty_schema(schema).await?;
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

// src/mysql_types.rs
pub const MYSQL_TYPES: &[&str] = &[
    /* ---------- 数值 ---------- */
    "TINYINT",
    "SMALLINT",
    "MEDIUMINT",
    "INT",
    "INTEGER",
    "BIGINT",
    "DECIMAL",
    "NUMERIC",
    "FLOAT",
    "DOUBLE",
    "REAL",
    "BIT",
    /* ---------- 日期/时间 ---------- */
    "DATE",
    "TIME",
    "DATETIME",
    "TIMESTAMP",
    "YEAR",
    /* ---------- 字符串 ---------- */
    "CHAR",
    "VARCHAR",
    "BINARY",
    "VARBINARY",
    "TINYBLOB",
    "BLOB",
    "MEDIUMBLOB",
    "LONGBLOB",
    "TINYTEXT",
    "TEXT",
    "MEDIUMTEXT",
    "LONGTEXT",
    /* ---------- JSON ---------- */
    "JSON",
    /* ---------- 空间/几何 (MySQL & MariaDB) ---------- */
    "GEOMETRY",
    "POINT",
    "LINESTRING",
    "POLYGON",
    "MULTIPOINT",
    "MULTILINESTRING",
    "MULTIPOLYGON",
    "GEOMETRYCOLLECTION",
    /* ---------- 枚举/集合 ---------- */
    "ENUM",
    "SET",
    /* ---------- MariaDB 独有扩展 ---------- */
    "INET4", // MariaDB 10.10+
    "INET6", // MariaDB 10.10+
    /* ---------- 别名（官方文档列出，会出现在 DDL 中） ---------- */
    "BOOL",
    "BOOLEAN",
    "SERIAL",
    "FIXED",
    "FLOAT4",
    "FLOAT8",
    "INT1",
    "INT2",
    "INT3",
    "INT4",
    "INT8",
    "LONG",
    "LONGLONG",
    "MIDDLEINT",
    "LONG VARBINARY",
    "LONG VARCHAR",
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::datasource_config::MysqlDBConfig;
    use crate::db::{BackendKindMysql, QuerySql, SchemaTrait};
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::language_server::LanguageServer;
    use crate::mysql::MysqlDB;
    use crate::{enable_test_log, TEST_HOST};

    async fn make_test_object(config: MysqlDBConfig) -> MysqlDB {
        let db = MysqlDB::new(config, BackendKindMysql::Mysql).await.unwrap();
        db
    }
    fn make_config() -> MysqlDBConfig {
        MysqlDBConfig {
            conn_timeout: Some(5000.into()),
            host: TEST_HOST.to_string(),
            username: "root".to_string(),
            password: Some("root".to_string()),
            port: 3306,
            db: Some("test".to_string()),
            read_only: false,

            ssl: Default::default(),
        }
    }

    // partition test failed , sqlparser can't parse a valid partition statement.
    #[tokio::test]
    async fn test_auto_completion() {
        let db = make_test_object(make_config()).await;
        enable_test_log();
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("departments", true).await;
        let _ = db.drop_table("employees", true).await;
        let _ = db.drop_view("employees_view").await;

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
            .execute_sql(query_id, "replace into departments values (1,'test')")
            .await
            .unwrap();

        let _ = db
            .execute_sql(
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

        let _ = db.drop_event("do_nothing_event_loop").await;
        let _ = db
            .execute_sql(
                query_id,
                r#"
            CREATE EVENT do_nothing_event_loop
            ON SCHEDULE EVERY 10 SECOND
            STARTS CURRENT_TIMESTAMP
            DO
            BEGIN
            /* 空语句块，可永久存在 */
            END;
            "#,
            )
            .await
            .unwrap();

        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("mysql_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        test_file_auto_completion(&content, &db, None, false).await;
    }
    #[ignore = "手动跑跑的测试"]
    #[tokio::test]
    async fn test_completion_xxx() {
        enable_test_log();
        let mut config = make_config();
        config.db = None;

        let db = make_test_object(config).await;
        test_file_auto_completion(include_str!("./xxx.sql"), &db, None, false).await;
    }

    #[tokio::test]

    async fn test_at_variable_lexer() {
        let sql = include_str!("../tests/mysql_at_var_lexer.sql");
        let db = make_test_object(make_config()).await;

        let list = LanguageServer::auto_completion(&db, None, sql, 2, 11, true)
            .await
            .unwrap();
        list.items.iter().for_each(|x| {
            if x.item.contains("my_var") {
                println!("!!!!!:{:?}", x.item);
            }
        });

        assert!(list.items.iter().any(|x| x.item == "my_var"));
    }

    #[tokio::test]

    async fn test_try_get_max_id() {
        let db = make_test_object(make_config()).await;
        let max_id = db.try_get_max_id("departments").await.unwrap();
        println!("max id is:{}", max_id);
    }
}
