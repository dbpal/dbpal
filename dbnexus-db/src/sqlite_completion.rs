use std::{borrow::Cow, ops::DerefMut, sync::Arc};

use crate::{
    completion::{
        try_completion_on_syntax, AutoCompletionTrait, CompletionItem, CompletionKind,
        CompletionOnSynTaxHelper, LexerCompletionTrait,
    },
    db::{Field, SchemaTrait},
    sqlite::SqliteDB,
};
use sqlparser::ast::ObjectType;
use sqlx::Row;

#[async_trait::async_trait]

impl crate::completion::LexerCompletionTrait for SqliteDB {
    type Dialect = sqlparser::dialect::SQLiteDialect;

    const BUILD_IN_FUNCTIONS: &'static [&'static str] = SQLITE_FUNTIONS;
    // https://sqlite.org/lang_keywords.html
    const KEY_WORDS: &'static [&'static str] = SQLITE_KEY_WORDS;
    const TEXTS: &'static [&'static str] = &["true"];
    const TYPES: &'static [&'static str] = SQLITE_TYPES;

    async fn extra_completion_item(&self) -> Vec<CompletionItem> {
        PRAGMA_ITEMS
            .iter()
            .map(|x| CompletionItem {
                item: Cow::Borrowed(*x),
                kind: CompletionKind::Text,
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl AutoCompletionTrait for SqliteDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            &sql,
            line,
            col,
            sqlparser::dialect::SQLiteDialect {},
            Arc::new(Clone::clone(self)),
        )
        .await;
        match r {
            Ok(xs) => return Ok(xs),
            Err(err) => {
                log::error!("completion on ast failed,err:{:?}", err);
            }
        };
        return LexerCompletionTrait::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::SQLiteDialect {},
            sqlparser::tokenizer::Location {
                line: line as u64,
                column: col as u64,
            },
        )
        .await;
    }
}

impl SqliteDB {
    async fn list_index_name_impl(&self) -> anyhow::Result<Vec<String>> {
        let sql = r#"
        SELECT name
            FROM sqlite_master
            WHERE type = 'index';"#;
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name = r.get(0);
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl CompletionOnSynTaxHelper for SqliteDB {
    async fn list_table(
        &self, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        SchemaTrait::list_table(self)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }
    async fn list_column(
        &self, _db: Option<&str>, _schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<Field>> {
        SchemaTrait::show_columns(self, &table).await
    }
    async fn list_function(
        &self, _catelog: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        anyhow::bail!("sqlite can't use sql to create function.")
    }
    async fn list_drop_by_type(
        &self, typ: ObjectType, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        match typ {
            ObjectType::Table => CompletionOnSynTaxHelper::list_table(self, None, None).await,
            ObjectType::View => SchemaTrait::list_view(self)
                .await
                .map(|x| x.into_iter().map(|x| x.name).collect::<Vec<_>>()),
            ObjectType::Index => {
                // use by drop index of something
                // in this case we don't know the table
                // so we must can tell which index is base on index's name
                self.list_index_name_impl().await
            }
            _ => {
                anyhow::bail!("not support object type '{}' by sqlte.", typ)
            }
        }
    }
    async fn list_index(&self) -> anyhow::Result<Vec<String>> {
        self.list_index_name_impl().await
    }

    fn sqlite_pragma(&self) -> Vec<CompletionItem> {
        static PS: &[&'static str] = &[
            "analysis_limit",
            "application_id",
            "auto_vacuum",
            "automatic_index",
            "busy_timeout",
            "cache_size",
            "cache_spill",
            "case_sensitive_like¹",
            "cell_size_check",
            "checkpoint_fullfsync",
            "collation_list",
            "compile_options",
            "count_changes¹",
            "data_store_directory¹",
            "data_version",
            "database_list",
            "default_cache_size¹",
            "defer_foreign_keys",
            "empty_result_callbacks¹",
            "encoding",
            "foreign_key_check",
            "foreign_key_list",
            "foreign_keys",
            "freelist_count",
            "full_column_names¹",
            "fullfsync",
            "function_list",
            "hard_heap_limit",
            "ignore_check_constraints",
            "incremental_vacuum",
            "index_info",
            "index_list",
            "index_xinfo",
            "integrity_check",
            "journal_mode",
            "journal_size_limit",
            "legacy_alter_table",
            "legacy_file_format",
            "locking_mode",
            "max_page_count",
            "mmap_size",
            "module_list",
            "optimize",
            "page_count",
            "page_size",
            "parser_trace²",
            "pragma_list",
            "query_only",
            "quick_check",
            "read_uncommitted",
            "recursive_triggers",
            "reverse_unordered_selects",
            "schema_version³",
            "secure_delete",
            "short_column_names",
            "shrink_memory",
            "soft_heap_limit",
            "stats³",
            "synchronous",
            "table_info",
            "table_list",
            "table_xinfo",
            "temp_store",
            "temp_store_directory¹",
            "threads",
            "trusted_schema",
            "user_version",
            "vdbe_addoptrace",
            "vdbe_debug²",
            "vdbe_listing²",
            "vdbe_trace²",
            "wal_autocheckpoint",
            "wal_checkpoint",
            "writable_schema",
        ];
        lazy_static::lazy_static! {
            static ref CS : Vec<CompletionItem> = PS.iter().map(|x| CompletionItem {
                item : Cow::Borrowed(*x),
                kind : CompletionKind::Pragma,
            }).collect();
        }
        return CS.clone();
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{
        completion::completion_test::test_file_auto_completion, enable_test_log, sqlite::SqliteDB,
    };
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "should run by hand."]

    async fn test_auto_completion() {
        /*

        测试用到的表格 和建表语句

           -- 创建 departments 表
           CREATE TABLE departments (
               id INTEGER PRIMARY KEY,
               name TEXT NOT NULL
           );

           -- 创建 employees 表
           CREATE TABLE employees (
               id INTEGER PRIMARY KEY,
               name TEXT NOT NULL,
               department_id INTEGER,
               FOREIGN KEY (department_id) REFERENCES departments (id)
           );

           -- 插入测试数据到 departments 表
           INSERT INTO departments (id, name) VALUES
           (1, 'HR'),
           (2, 'IT'),
           (3, 'Sales');

           -- 插入测试数据到 employees 表
           INSERT INTO employees (id, name, department_id) VALUES
           (1, 'Alice', 1),
           (2, 'Bob', 2),
           (3, 'Charlie', 2),
           (4, 'David', 1),
           (5, 'Eva', 3);

           -- 创建 departments 表
           CREATE TABLE departments (
               id INTEGER PRIMARY KEY,
               name TEXT NOT NULL
           );

           -- 创建 employees 表
           CREATE TABLE employees (
               id INTEGER PRIMARY KEY,
               name TEXT NOT NULL,
               department_id INTEGER,
               FOREIGN KEY (department_id) REFERENCES departments (id)
           );

           -- 插入测试数据到 departments 表
           INSERT INTO departments (id, name) VALUES
           (1, 'HR'),
           (2, 'IT'),
           (3, 'Sales');

           -- 插入测试数据到 employees 表
           INSERT INTO employees (id, name, department_id) VALUES
           (1, 'Alice', 1),
           (2, 'Bob', 2),
           (3, 'Charlie', 2),
           (4, 'David', 1),
           (5, 'Eva', 3);
        */
        enable_test_log();
        let manifest_dir =
            std::env::var("CARGO_MANIFEST_DIR").expect("无法获取 CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir.as_str()).to_path_buf();
        file_path.push("tests");
        file_path.push("sqlite_auto_completion_test.txt");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        let db = SqliteDB::new("/Users/yuyang/tests/test.sqlite.db".to_string(), false)
            .await
            .unwrap();

        test_file_auto_completion(&content, &db).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "should run by hand."]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = SqliteDB::new("/Users/yuyang/tests/test.sqlite.db".to_string(), false)
            .await
            .unwrap();
        test_file_auto_completion(
            r#"select id from (select * from employees)
10 name"#,
            &db,
        )
        .await;
    }
}

const SQLITE_FUNTIONS: &[&str] = &[
    "abs",
    "acos",
    "acosh",
    "asin",
    "asinh",
    "atan",
    "atan2",
    "atanh",
    "avg",
    "bm25",
    "ceil",
    "ceiling",
    "changes",
    "char",
    "coalesce",
    "cos",
    "cosh",
    "count",
    "cume_dist",
    "current_date",
    "current_time",
    "current_timestamp",
    "date",
    "datetime",
    "degrees",
    "dense_rank",
    "exp",
    "first_value",
    "floor",
    "format",
    "fts3_tokenizer",
    "fts5",
    "fts5_source_id",
    "geopoly_area",
    "geopoly_bbox",
    "geopoly_blob",
    "geopoly_ccw",
    "geopoly_contains_point",
    "geopoly_debug",
    "geopoly_group_bbox",
    "geopoly_json",
    "geopoly_overlap",
    "geopoly_regular",
    "geopoly_svg",
    "geopoly_within",
    "geopoly_xform",
    "glob",
    "group_concat",
    "hex",
    "highlight",
    "ifnull",
    "iif",
    "instr",
    "json",
    "json_array",
    "json_array_length",
    "json_extract",
    "json_group_array",
    "json_group_object",
    "json_insert",
    "json_object",
    "json_patch",
    "json_quote",
    "json_remove",
    "json_replace",
    "json_set",
    "json_type",
    "json_valid",
    "julianday",
    "lag",
    "last_insert_rowid",
    "last_value",
    "lead",
    "length",
    "like",
    "likelihood",
    "likely",
    "ln",
    "load_extension",
    "log",
    "log10",
    "log2",
    "lower",
    "ltrim",
    "match",
    "matchinfo",
    "max",
    "min",
    "mod",
    "nth_value",
    "ntile",
    "nullif",
    "offsets",
    "optimize",
    "percent_rank",
    "pi",
    "pow",
    "power",
    "printf",
    "quote",
    "radians",
    "random",
    "randomblob",
    "rank",
    "replace",
    "round",
    "row_number",
    "rtreecheck",
    "rtreedepth",
    "rtreenode",
    "rtrim",
    "sign",
    "sin",
    "sinh",
    "snippet",
    "soundex",
    "sqlite_compileoption_get",
    "sqlite_compileoption_used",
    "sqlite_log",
    "sqlite_source_id",
    "sqlite_version",
    "sqrt",
    "strftime",
    "substr",
    "substring",
    "subtype",
    "sum",
    "tan",
    "tanh",
    "time",
    "total",
    "total_changes",
    "trim",
    "trunc",
    "typeof",
    "unhex",
    "unicode",
    "unixepoch",
    "unlikely",
    "upper",
    "zeroblob",
];
const SQLITE_TYPES: &[&str] = &["NULL", "INTEGER", "REAL", "TEXT", "BLOB"];

const SQLITE_KEY_WORDS: &[&str] = &[
    "ABORT",
    "ACTION",
    "ADD",
    "AFTER",
    "ALL",
    "ALTER",
    "ALWAYS",
    "ANALYZE",
    "AND",
    "AS",
    "ASC",
    "ATTACH",
    "AUTOINCREMENT",
    "BEFORE",
    "BEGIN",
    "BETWEEN",
    "BY",
    "CASCADE",
    "CASE",
    "CAST",
    "CHECK",
    "COLLATE",
    "COLUMN",
    "COMMIT",
    "CONFLICT",
    "CONSTRAINT",
    "CREATE",
    "CROSS",
    "CURRENT",
    "CURRENT_DATE",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "DATABASE",
    "DEFAULT",
    "DEFERRABLE",
    "DEFERRED",
    "DELETE",
    "DESC",
    "DETACH",
    "DISTINCT",
    "DO",
    "DROP",
    "EACH",
    "ELSE",
    "END",
    "ESCAPE",
    "EXCEPT",
    "EXCLUDE",
    "EXCLUSIVE",
    "EXISTS",
    "EXPLAIN",
    "FAIL",
    "FILTER",
    "FIRST",
    "FOLLOWING",
    "FOR",
    "FOREIGN",
    "FROM",
    "FULL",
    "GENERATED",
    "GLOB",
    "GROUP",
    "GROUPS",
    "HAVING",
    "IF",
    "IGNORE",
    "IMMEDIATE",
    "IN",
    "INDEX",
    "INDEXED",
    "INITIALLY",
    "INNER",
    "INSERT",
    "INSTEAD",
    "INTERSECT",
    "INTO",
    "IS",
    "ISNULL",
    "JOIN",
    "KEY",
    "LAST",
    "LEFT",
    "LIKE",
    "LIMIT",
    "MATCH",
    "MATERIALIZED",
    "NATURAL",
    "NO",
    "NOT",
    "NOTHING",
    "NOTNULL",
    "NULL",
    "NULLS",
    "OF",
    "OFFSET",
    "ON",
    "OR",
    "ORDER",
    "OTHERS",
    "OUTER",
    "OVER",
    "PARTITION",
    "PLAN",
    "PRAGMA",
    "PRECEDING",
    "PRIMARY",
    "QUERY",
    "RAISE",
    "RANGE",
    "RECURSIVE",
    "REFERENCES",
    "REGEXP",
    "REINDEX",
    "RELEASE",
    "RENAME",
    "REPLACE",
    "RESTRICT",
    "RETURNING",
    "RIGHT",
    "ROLLBACK",
    "ROW",
    "ROWS",
    "SAVEPOINT",
    "SELECT",
    "SET",
    "TABLE",
    "TEMP",
    "TEMPORARY",
    "THEN",
    "TIES",
    "TO",
    "TRANSACTION",
    "TRIGGER",
    "UNBOUNDED",
    "UNION",
    "UNIQUE",
    "UPDATE",
    "USING",
    "VACUUM",
    "VALUES",
    "VIEW",
    "VIRTUAL",
    "WHEN",
    "WHERE",
    "WINDOW",
    "WITH",
    "WITHOUT",
];

const PRAGMA_ITEMS: &'static [&'static str] = &[
    "analysis_limit",
    "application_id",
    "auto_vacuum",
    "automatic_index",
    "busy_timeout",
    "cache_size",
    "cache_spill",
    "case_sensitive_like",
    "cell_size_check",
    "checkpoint_fullfsync",
    "collation_list",
    "compile_options",
    "count_changes",
    "data_store_directory",
    "data_version",
    "database_list",
    "default_cache_size",
    "defer_foreign_keys",
    "empty_result_callbacks",
    "encoding",
    "foreign_key_check",
    "foreign_key_list",
    "foreign_keys",
    "freelist_count",
    "full_column_names",
    "fullfsync",
    "function_list",
    "hard_heap_limit",
    "ignore_check_constraints",
    "incremental_vacuum",
    "index_info",
    "index_list",
    "index_xinfo",
    "integrity_check",
    "journal_mode",
    "journal_size_limit",
    "legacy_alter_table",
    "legacy_file_format",
    "locking_mode",
    "max_page_count",
    "mmap_size",
    "module_list",
    "optimize",
    "page_count",
    "page_size",
    "parser_trace",
    "pragma_list",
    "query_only",
    "quick_check",
    "read_uncommitted",
    "recursive_triggers",
    "reverse_unordered_selects",
    "schema_version",
    "secure_delete",
    "short_column_names",
    "shrink_memory",
    "soft_heap_limit",
    "stats",
    "synchronous",
    "table_info",
    "table_list",
    "table_xinfo",
    "temp_store",
    "temp_store_directory",
    "threads",
    "trusted_schema",
    "user_version",
    "vdbe_addoptrace",
    "vdbe_debug",
    "vdbe_listing",
    "vdbe_trace",
    "wal_autocheckpoint",
    "wal_checkpoint",
    "writable_schema",
];
