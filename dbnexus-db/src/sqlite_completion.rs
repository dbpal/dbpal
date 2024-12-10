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
                desc: None,
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl AutoCompletionTrait for SqliteDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
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
        if !use_lexer {
            return anyhow::Ok(vec![]);
        }

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
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let vec = SQLITE_FN_AND_DESC
            .iter()
            .map(|x| CompletionItem {
                item: x[0].into(),
                kind: CompletionKind::Function,
                desc: Some(x[1].into()),
            })
            .collect::<Vec<_>>();
        anyhow::Ok(vec)
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
                kind : CompletionKind::Pragma, desc: None,
            }).collect();
        }
        return CS.clone();
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        completion::completion_test::test_file_auto_completion, enable_test_log, sqlite::SqliteDB,
    };
    use std::path::Path;
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
        test_file_auto_completion(&content, &db, false).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "should run by hand."]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = SqliteDB::new("/Users/yuyang/tests/test.sqlite.db".to_string(), false)
            .await
            .unwrap();
        test_file_auto_completion(
            r#"update employees set name = 1 where id = 10000
25 name ; 25 id "#,
            &db,
            false,
        )
        .await;
    }
}

const SQLITE_FN_AND_DESC: &'static [[&'static str; 2]] = &[
    [
        "abs",
        r#"The abs(X) function returns the absolute value of the numeric argument X. Abs(X) returns NULL if X is NULL. Abs(X) returns 0.0 if X is a string or blob that cannot be converted to a numeric value. If X is the integer -9223372036854775808 then abs(X) throws an integer overflow error since there is no equivalent positive 64-bit two complement value. "#,
    ],
    [
        "changes",
        r#"The changes() function returns the number of database rows that were changed or inserted or deleted by the most recently completed INSERT, DELETE, or UPDATE statement, exclusive of statements in lower-level triggers. The changes() SQL function is a wrapper around the sqlite3_changes64() C/C++ function and hence follows the same rules for counting changes. "#,
    ],
    [
        "char",
        r#"The char(X1,X2,...,XN) function returns a string composed of characters having the unicode code point values of integers X1 through XN, respectively."#,
    ],
    [
        "coalesce",
        r#"The coalesce() function returns a copy of its first non-NULL argument, or NULL if all arguments are NULL. Coalesce() must have at least 2 arguments."#,
    ],
    [
        "concat",
        r#"The concat(...) function returns a string which is the concatenation of the string representation of all of its non-NULL arguments. If all arguments are NULL, then concat() returns an empty string. "#,
    ],
    [
        "concat_ws",
        r#"The concat_ws(SEP,...) function returns a string that is the concatenation of all non-null arguments beyond the first argument, using the text value of the first argument as a separator. If the first argument is NULL, then concat_ws() returns NULL. If all arguments other than the first are NULL, then concat_ws() returns an empty string."#,
    ],
    [
        "format",
        r#"The format(FORMAT,...) SQL function works like the sqlite3_mprintf() C-language function and the printf() function from the standard C library. The first argument is a format string that specifies how to construct the output string using values taken from subsequent arguments. If the FORMAT argument is missing or NULL then the result is NULL. The %n format is silently ignored and does not consume an argument. The %p format is an alias for %X. The %z format is interchangeable with %s. If there are too few arguments in the argument list, missing arguments are assumed to have a NULL value, which is translated into 0 or 0.0 for numeric formats or an empty string for %s. See the built-in printf() documentation for additional information."#,
    ],
    [
        "glob",
        r#"The glob(X,Y) function is equivalent to the expression "Y GLOB X". Note that the X and Y arguments are reversed in the glob() function relative to the infix GLOB operator. Y is the string and X is the pattern. So, for example, the following expressions are equivalent:

             name GLOB '*helium*'
             glob('*helium*',name)
          

    If the sqlite3_create_function() interface is used to override the glob(X,Y) function with an alternative implementation then the GLOB operator will invoke the alternative implementation. "#,
    ],
    [
        "hex",
        r#"The hex() function interprets its argument as a BLOB and returns a string which is the upper-case hexadecimal rendering of the content of that blob.

    If the argument X in "hex(X)" is an integer or floating point number, then "interprets its argument as a BLOB" means that the binary number is first converted into a UTF8 text representation, then that text is interpreted as a BLOB. Hence, "hex(12345678)" renders as "3132333435363738" not the binary representation of the integer value "0000000000BC614E".

    See also: unhex() "#,
    ],
    [
        "ifnull",
        r#"The ifnull() function returns a copy of its first non-NULL argument, or NULL if both arguments are NULL. Ifnull() must have exactly 2 arguments. The ifnull() function is equivalent to coalesce() with two arguments."#,
    ],
    [
        "iif",
        r#"The iif(X,Y,Z) function returns the value Y if X is true, and Z otherwise. The iif(X,Y,Z) function is logically equivalent to and generates the same bytecode as the CASE expression "CASE WHEN X THEN Y ELSE Z END"."#,
    ],
    [
        "instr",
        r#"The instr(X,Y) function finds the first occurrence of string Y within string X and returns the number of prior characters plus 1, or 0 if Y is nowhere found within X. Or, if X and Y are both BLOBs, then instr(X,Y) returns one more than the number bytes prior to the first occurrence of Y, or 0 if Y does not occur anywhere within X. If both arguments X and Y to instr(X,Y) are non-NULL and are not BLOBs then both are interpreted as strings. If either X or Y are NULL in instr(X,Y) then the result is NULL."#,
    ],
    [
        "last_insert_rowid",
        r#"The last_insert_rowid() function returns the ROWID of the last row insert from the database connection which invoked the function. The last_insert_rowid() SQL function is a wrapper around the sqlite3_last_insert_rowid() C/C++ interface function."#,
    ],
    [
        "length",
        r#"For a string value X, the length(X) function returns the number of Unicode code points (not bytes) in input string X prior to the first U+0000 character. Since SQLite strings do not normally contain NUL characters, the length(X) function will usually return the total number of characters in the string X. For a blob value X, length(X) returns the number of bytes in the blob. If X is NULL then length(X) is NULL. If X is numeric then length(X) returns the length of a string representation of X.

    Note that for strings, the length(X) function returns the character or code-point length of the string, not the byte length. The character length is the number of characters in the string. The character length is always different from the byte length for UTF-16 strings, and can be different from the byte length for UTF-8 strings if the string contains multi-byte characters. Use the octet_length() function to find the byte length of a string.

    For BLOB values, length(X) always returns the byte-length of the BLOB.

    For string values, length(X) must read the entire string into memory in order to compute the character length. But for BLOB values, reading the whole string into memory is not necessary as SQLite already knows how many bytes are in the BLOB. Hence, for multi-megabyte values, the length(X) function is usually much faster for BLOBs than for strings, since it does not need to load the value into memory. 

like(X,Y)"#,
    ],
    [
        "like",
        r#"The like() function is used to implement the "Y LIKE X [ESCAPE Z]" expression. If the optional ESCAPE clause is present, then the like() function is invoked with three arguments. Otherwise, it is invoked with two arguments only. Note that the X and Y parameters are reversed in the like() function relative to the infix LIKE operator. X is the pattern and Y is the string to match against that pattern. Hence, the following expressions are equivalent:

             name LIKE '%neon%'
             like('%neon%',name)
          

    The sqlite3_create_function() interface can be used to override the like() function and thereby change the operation of the LIKE operator. When overriding the like() function, it may be important to override both the two and three argument versions of the like() function. Otherwise, different code may be called to implement the LIKE operator depending on whether or not an ESCAPE clause was specified. "#,
    ],
    [
        "likelihood",
        r#"The likelihood(X,Y) function returns argument X unchanged. The value Y in likelihood(X,Y) must be a floating point constant between 0.0 and 1.0, inclusive. The likelihood(X) function is a no-op that the code generator optimizes away so that it consumes no CPU cycles during run-time (that is, during calls to sqlite3_step()). The purpose of the likelihood(X,Y) function is to provide a hint to the query planner that the argument X is a boolean that is true with a probability of approximately Y. The unlikely(X) function is short-hand for likelihood(X,0.0625). The likely(X) function is short-hand for likelihood(X,0.9375). "#,
    ],
    [
        "likely",
        r#"The likely(X) function returns the argument X unchanged. The likely(X) function is a no-op that the code generator optimizes away so that it consumes no CPU cycles at run-time (that is, during calls to sqlite3_step()). The purpose of the likely(X) function is to provide a hint to the query planner that the argument X is a boolean value that is usually true. The likely(X) function is equivalent to likelihood(X,0.9375). See also: unlikely(X). "#,
    ],
    [
        "load_extension",
        r#"The load_extension(X,Y) function loads SQLite extensions out of the shared library file named X using the entry point Y. The result of load_extension() is always a NULL. If Y is omitted then the default entry point name is used. The load_extension() function raises an exception if the extension fails to load or initialize correctly.

    The load_extension() function will fail if the extension attempts to modify or delete an SQL function or collating sequence. The extension can add new functions or collating sequences, but cannot modify or delete existing functions or collating sequences because those functions and/or collating sequences might be used elsewhere in the currently running SQL statement. To load an extension that changes or deletes functions or collating sequences, use the sqlite3_load_extension() C-language API.

    For security reasons, extension loading is disabled by default and must be enabled by a prior call to sqlite3_enable_load_extension().
"#,
    ],
    [
        "lower",
        r#"The lower(X) function returns a copy of string X with all ASCII characters converted to lower case. The default built-in lower() function works for ASCII characters only. To do case conversions on non-ASCII characters, load the ICU extension."#,
    ],
    [
        "ltrim",
        r#"The ltrim(X,Y) function returns a string formed by removing any and all characters that appear in Y from the left side of X. If the Y argument is omitted, ltrim(X) removes spaces from the left side of X. "#,
    ],
    [
        "max",
        r#"The multi-argument max() function returns the argument with the maximum value, or return NULL if any argument is NULL. The multi-argument max() function searches its arguments from left to right for an argument that defines a collating function and uses that collating function for all string comparisons. If none of the arguments to max() define a collating function, then the BINARY collating function is used. Note that max() is a simple function when it has 2 or more arguments but operates as an aggregate function if given only a single argument. "#,
    ],
    [
        "min",
        r#"The multi-argument min() function returns the argument with the minimum value. The multi-argument min() function searches its arguments from left to right for an argument that defines a collating function and uses that collating function for all string comparisons. If none of the arguments to min() define a collating function, then the BINARY collating function is used. Note that min() is a simple function when it has 2 or more arguments but operates as an aggregate function if given only a single argument."#,
    ],
    [
        "nullif",
        r#"The nullif(X,Y) function returns its first argument if the arguments are different and NULL if the arguments are the same. The nullif(X,Y) function searches its arguments from left to right for an argument that defines a collating function and uses that collating function for all string comparisons. If neither argument to nullif() defines a collating function then the BINARY collating function is used. "#,
    ],
    [
        "octet_length",
        r#"     The octet_length(X) function returns the number of bytes in the encoding of text string X. If X is NULL then octet_length(X) returns NULL. If X is a BLOB value, then octet_length(X) is the same as length(X). If X is a numeric value, then octet_length(X) returns the number of bytes in a text rendering of that number.

    Because octet_length(X) returns the number of bytes in X, not the number of characters or code-points, the value returned depends on the database encoding. The octet_length() function can return different answers for the same input string if the database encoding is UTF16 instead of UTF8.

    If argument X is a table column and the value is of type text or blob, then octet_length(X) avoids reading the content of X from disk, as the byte length can be computed from metadata. Thus, octet_length(X) is efficient even if X is a column containing a multi-megabyte text or blob value. "#,
    ],
    [
        "printf",
        r#"The printf() SQL function is an alias for the format() SQL function. The format() SQL function was originally named printf(). But the name was later changed to format() for compatibility with other database engines. The printf() name is retained as an alias so as not to break legacy code. "#,
    ],
    [
        "quote",
        r#"The quote(X) function returns the text of an SQL literal which is the value of its argument suitable for inclusion into an SQL statement. Strings are surrounded by single-quotes with escapes on interior quotes as needed. BLOBs are encoded as hexadecimal literals. Strings with embedded NUL characters cannot be represented as string literals in SQL and hence the returned string literal is truncated prior to the first NUL. "#,
    ],
    [
        "random",
        r#"The random() function returns a pseudo-random integer between -9223372036854775808 and +9223372036854775807. "#,
    ],
    [
        "randomblob",
        r#"The randomblob(N) function return an N-byte blob containing pseudo-random bytes. If N is less than 1 then a 1-byte random blob is returned.

    Hint: applications can generate globally unique identifiers using this function together with hex() and/or lower() like this:

        hex(randomblob(16))

        lower(hex(randomblob(16))) 

"#,
    ],
    [
        "replace",
        r#"The replace(X,Y,Z) function returns a string formed by substituting string Z for every occurrence of string Y in string X. The BINARY collating sequence is used for comparisons. If Y is an empty string then return X unchanged. If Z is not initially a string, it is cast to a UTF-8 string prior to processing. "#,
    ],
    [
        "round",
        r#"The round(X,Y) function returns a floating-point value X rounded to Y digits to the right of the decimal point. If the Y argument is omitted or negative, it is taken to be 0. "#,
    ],
    [
        "rtrim",
        r#"The rtrim(X,Y) function returns a string formed by removing any and all characters that appear in Y from the right side of X. If the Y argument is omitted, rtrim(X) removes spaces from the right side of X. "#,
    ],
    [
        "soundex",
        r#"The soundex(X) function returns a string that is the soundex encoding of the string X. The string "?000" is returned if the argument is NULL or contains no ASCII alphabetic characters. This function is omitted from SQLite by default. It is only available if the SQLITE_SOUNDEX compile-time option is used when SQLite is built. "#,
    ],
    [
        "sqlite_compileoption_get",
        r#" The sqlite_compileoption_get() SQL function is a wrapper around the sqlite3_compileoption_get() C/C++ function. This routine returns the N-th compile-time option used to build SQLite or NULL if N is out of range. See also the compile_options pragma. "#,
    ],
    [
        "sqlite_compileoption_used",
        r#"The sqlite_compileoption_used() SQL function is a wrapper around the sqlite3_compileoption_used() C/C++ function. When the argument X to sqlite_compileoption_used(X) is a string which is the name of a compile-time option, this routine returns true (1) or false (0) depending on whether or not that option was used during the build."#,
    ],
    [
        "sqlite_offset",
        r#"The sqlite_offset(X) function returns the byte offset in the database file for the beginning of the record from which value would be read. If X is not a column in an ordinary table, then sqlite_offset(X) returns NULL. The value returned by sqlite_offset(X) might reference either the original table or an index, depending on the query. If the value X would normally be extracted from an index, the sqlite_offset(X) returns the offset to the corresponding index record. If the value X would be extracted from the original table, then sqlite_offset(X) returns the offset to the table record.

    The sqlite_offset(X) SQL function is only available if SQLite is built using the -DSQLITE_ENABLE_OFFSET_SQL_FUNC compile-time option."#,
    ],
    [
        "sqlite_source_id",
        r#"The sqlite_source_id() function returns a string that identifies the specific version of the source code that was used to build the SQLite library. The string returned by sqlite_source_id() is the date and time that the source code was checked in followed by the SHA3-256 hash for that check-in. This function is an SQL wrapper around the sqlite3_sourceid() C interface. "#,
    ],
    [
        "sqlite_version",
        r#"The sqlite_version() function returns the version string for the SQLite library that is running. This function is an SQL wrapper around the sqlite3_libversion() C-interface. "#,
    ],
    [
        "substr",
        r#"The substr(X,Y,Z) function returns a substring of input string X that begins with the Y-th character and which is Z characters long. If Z is omitted then substr(X,Y) returns all characters through the end of the string X beginning with the Y-th. The left-most character of X is number 1. If Y is negative then the first character of the substring is found by counting from the right rather than the left. If Z is negative then the abs(Z) characters preceding the Y-th character are returned. If X is a string then characters indices refer to actual UTF-8 characters. If X is a BLOB then the indices refer to bytes.

    "substring()" is an alias for "substr()" beginning with SQLite version 3.34."#,
    ],
    [
        "substring",
        r#"The substr(X,Y,Z) function returns a substring of input string X that begins with the Y-th character and which is Z characters long. If Z is omitted then substr(X,Y) returns all characters through the end of the string X beginning with the Y-th. The left-most character of X is number 1. If Y is negative then the first character of the substring is found by counting from the right rather than the left. If Z is negative then the abs(Z) characters preceding the Y-th character are returned. If X is a string then characters indices refer to actual UTF-8 characters. If X is a BLOB then the indices refer to bytes.

    "substring()" is an alias for "substr()" beginning with SQLite version 3.34."#,
    ],
    [
        "total_changes",
        r#"The total_changes() function returns the number of row changes caused by INSERT, UPDATE or DELETE statements since the current database connection was opened. This function is a wrapper around the sqlite3_total_changes64() C/C++ interface. "#,
    ],
    [
        "trim",
        r#"The trim(X,Y) function returns a string formed by removing any and all characters that appear in Y from both ends of X. If the Y argument is omitted, trim(X) removes spaces from both ends of X. "#,
    ],
    [
        "typeof",
        r#"The typeof(X) function returns a string that indicates the datatype of the expression X: "null", "integer", "real", "text", or "blob". "#,
    ],
    [
        "unhex",
        r#"The unhex(X,Y) function returns a BLOB value which is the decoding of the hexadecimal string X. If X contains any characters that are not hexadecimal digits and which are not in Y, then unhex(X,Y) returns NULL. If Y is omitted, it is understood to be an empty string and hence X must be a pure hexadecimal string. All hexadecimal digits in X must occur in pairs, with both digits of each pair beginning immediately adjacent to one another, or else unhex(X,Y) returns NULL. If either parameter X or Y is NULL, then unhex(X,Y) returns NULL. The X input may contain an arbitrary mix of upper and lower case hexadecimal digits. Hexadecimal digits in Y have no affect on the translation of X. Only characters in Y that are not hexadecimal digits are ignored in X.

    See also: hex() "#,
    ],
    [
        "unicode",
        r#"The unicode(X) function returns the numeric unicode code point corresponding to the first character of the string X. If the argument to unicode(X) is not a string then the result is undefined. "#,
    ],
    [
        "unlikely",
        r#"The unlikely(X) function returns the argument X unchanged. The unlikely(X) function is a no-op that the code generator optimizes away so that it consumes no CPU cycles at run-time (that is, during calls to sqlite3_step()). The purpose of the unlikely(X) function is to provide a hint to the query planner that the argument X is a boolean value that is usually not true. The unlikely(X) function is equivalent to likelihood(X, 0.0625). "#,
    ],
    [
        "upper",
        r#"The upper(X) function returns a copy of input string X in which all lower-case ASCII characters are converted to their upper-case equivalent. "#,
    ],
    [
        "zeroblob",
        r#"The zeroblob(N) function returns a BLOB consisting of N bytes of 0x00. SQLite manages these zeroblobs very efficiently. Zeroblobs can be used to reserve space for a BLOB that is later written using incremental BLOB I/O. This SQL function is implemented using the sqlite3_result_zeroblob() routine from the C/C++ interface. "#,
    ],
    [
        "date",
        r#"The date() function returns the date as text in this format: YYYY-MM-DD."#,
    ],
    [
        "time",
        r#"The time() function returns the time as text in formatted as HH:MM:SS or as HH:MM:SS.SSS if the subsec modifier is used."#,
    ],
    [
        "datetime",
        r#"The datetime() function returns the date and time formatted as YYYY-MM-DD HH:MM:SS or as YYYY-MM-DD HH:MM:SS.SSS if the subsec modifier is used."#,
    ],
    [
        "julianday",
        r#"The julianday() function returns the Julian day - the fractional number of days since noon in Greenwich on November 24, 4714 B.C. (Proleptic Gregorian calendar)."#,
    ],
    [
        "unixepoch",
        r#"The unixepoch() function returns a unix timestamp - the number of seconds since 1970-01-01 00:00:00 UTC. The unixepoch() function normally returns an integer number of seconds, but with the optional subsec modifier it will return a floating point number which is the fractional number of seconds. "#,
    ],
    [
        "strftime",
        r#"The strftime() function returns the date formatted according to the format string specified as the first argument. The format string supports the most common substitutions found in the strftime() function from the standard C library plus two new substitutions, %f and %J. The following is a complete list of valid strftime() substitutions as of version 3.46.0 (2024-05-23). Earlier versions of SQLite might not support all substitutions. If an undefined or unsupported substitution is seen, the result is NULL. "#,
    ],
    [
        "timediff",
        r#" The timediff(A,B) function returns a string that describes the amount of time that must be added to B in order to reach time A. The format of the timediff() result is designed to be human-readable. The format is:

    (+|-)YYYY-MM-DD HH:MM:SS.SSS 

This time difference string is also an allowed modifier for the other date/time functions. The following invariant holds for time-values A and B:

    datetime(A) = datetime(B, timediff(A,B)) 

The length of months and years vary. February is shorter than March. Leap years are longer than non-leap years. The output from timediff() takes this all into account. The timediff() function is intended to provide a human-friendly description of the time span. If you want to know the number of days or seconds between two dates, A and B, then you can always do one of these:

    SELECT julianday(B) - julianday(A);
    SELECT unixepoch(B) - unixepoch(A); 

The timediff(A,B) might return the same result even for values A and B that span a different number of days - depending on the starting date. For example, both of the following two timediff() calls return the same result ("-0000-01-00 00:00:00.000") even though the first timespan is 28 days and the seconds is 31 days:

    SELECT timediff('2023-02-15','2023-03-15');
    SELECT timediff('2023-03-15','2023-04-15'); 

Summary: If you want a human-friendly time span, use timediff(). If you want a precise time difference (in days or seconds) use the difference between two julianday() or unixepoch() calls. "#,
    ],
    [
        "avg",
        r#"The avg() function returns the average value of all non-NULL X within a group. String and BLOB values that do not look like numbers are interpreted as 0. The result of avg() is always a floating point value whenever there is at least one non-NULL input even if all inputs are integers. The result of avg() is NULL if there are no non-NULL inputs. The result of avg() is computed as total()/count() so all of the constraints that apply to total() also apply to avg()."#,
    ],
    [
        "count",
        r#"The count(X) function returns a count of the number of times that X is not NULL in a group. The count(*) function (with no arguments) returns the total number of rows in the group. "#,
    ],
    [
        "group_concat",
        r#"The group_concat() function returns a string which is the concatenation of all non-NULL values of X. If parameter Y is present then it is used as the separator between instances of X.A comma (",") is used as the separator if Y is omitted.

    The string_agg(X,Y) function is an alias for group_concat(X,Y). String_agg() is compatible with PostgreSQL and SQL-Server and group_concat() is compatible with MySQL.

    The order of the concatenated elements is arbitrary unless an ORDER BY argument is included immediately after the last parameter. "#,
    ],
    [
        "string_agg",
        r#"The group_concat() function returns a string which is the concatenation of all non-NULL values of X. If parameter Y is present then it is used as the separator between instances of X.A comma (",") is used as the separator if Y is omitted.

    The string_agg(X,Y) function is an alias for group_concat(X,Y). String_agg() is compatible with PostgreSQL and SQL-Server and group_concat() is compatible with MySQL.

    The order of the concatenated elements is arbitrary unless an ORDER BY argument is included immediately after the last parameter. "#,
    ],
    [
        "max",
        r#"The max() aggregate function returns the maximum value of all values in the group. The maximum value is the value that would be returned last in an ORDER BY on the same column. Aggregate max() returns NULL if and only if there are no non-NULL values in the group."#,
    ],
    [
        "min",
        r#"The min() aggregate function returns the minimum non-NULL value of all values in the group. The minimum value is the first non-NULL value that would appear in an ORDER BY of the column. Aggregate min() returns NULL if and only if there are no non-NULL values in the group. "#,
    ],
    [
        "sum",
        r#" The sum() and total() aggregate functions return the sum of all non-NULL values in the group. If there are no non-NULL input rows then sum() returns NULL but total() returns 0.0. NULL is not normally a helpful result for the sum of no rows but the SQL standard requires it and most other SQL database engines implement sum() that way so SQLite does it in the same way in order to be compatible. The non-standard total() function is provided as a convenient way to work around this design problem in the SQL language.

The result of total() is always a floating point value. The result of sum() is an integer value if all non-NULL inputs are integers. If any input to sum() is neither an integer nor a NULL, then sum() returns a floating point value which is an approximation of the mathematical sum.

Sum() will throw an "integer overflow" exception if all inputs are integers or NULL and an integer overflow occurs at any point during the computation. No overflow error is ever raised if any prior input was a floating point value. Total() never throws an integer overflow.

When summing floating-point values, if the magnitudes of the values differ wildly then the resulting sum might be imprecise due to the fact that IEEE 754 floating point values are approximations. Use the decimal_sum(X) aggregate in the decimal extension to obtain an exact summation of floating point numbers. Consider this test case:

    CREATE TABLE t1(x REAL);
    INSERT INTO t1 VALUES(1.55e+308),(1.23),(3.2e-16),(-1.23),(-1.55e308);
    SELECT sum(x), decimal_sum(x) FROM t1;

The large values ±1.55e+308 cancel each other out, but the cancellation does not occur until the end of the sum and in the meantime the large +1.55e+308 swamps the tiny 3.2e-16 value. The end result is an imprecise result for the sum(). The decimal_sum() aggregate generates an exact answer, at the cost of additional CPU and memory usage. Note also that decimal_sum() is not built into the SQLite core; it is a loadable extension.

If sum of inputs is too large to represent as a IEEE 754 floating point value, then a +Infinity or -Infinity result may be returned. If very large values with differing signs are used such that the SUM() or TOTAL() function is unable to determine if the correct result is +Infinity or -Infinity or some other value in between, then the result is NULL. Hence, for example, the following query returns NULL:

    WITH t1(x) AS (VALUES(1.0),(-9e+999),(2.0),(+9e+999),(3.0))
     SELECT sum(x) FROM t1;
"#,
    ],
    [
        "total",
        r#" The sum() and total() aggregate functions return the sum of all non-NULL values in the group. If there are no non-NULL input rows then sum() returns NULL but total() returns 0.0. NULL is not normally a helpful result for the sum of no rows but the SQL standard requires it and most other SQL database engines implement sum() that way so SQLite does it in the same way in order to be compatible. The non-standard total() function is provided as a convenient way to work around this design problem in the SQL language.

The result of total() is always a floating point value. The result of sum() is an integer value if all non-NULL inputs are integers. If any input to sum() is neither an integer nor a NULL, then sum() returns a floating point value which is an approximation of the mathematical sum.

Sum() will throw an "integer overflow" exception if all inputs are integers or NULL and an integer overflow occurs at any point during the computation. No overflow error is ever raised if any prior input was a floating point value. Total() never throws an integer overflow.

When summing floating-point values, if the magnitudes of the values differ wildly then the resulting sum might be imprecise due to the fact that IEEE 754 floating point values are approximations. Use the decimal_sum(X) aggregate in the decimal extension to obtain an exact summation of floating point numbers. Consider this test case:

    CREATE TABLE t1(x REAL);
    INSERT INTO t1 VALUES(1.55e+308),(1.23),(3.2e-16),(-1.23),(-1.55e308);
    SELECT sum(x), decimal_sum(x) FROM t1;

The large values ±1.55e+308 cancel each other out, but the cancellation does not occur until the end of the sum and in the meantime the large +1.55e+308 swamps the tiny 3.2e-16 value. The end result is an imprecise result for the sum(). The decimal_sum() aggregate generates an exact answer, at the cost of additional CPU and memory usage. Note also that decimal_sum() is not built into the SQLite core; it is a loadable extension.

If sum of inputs is too large to represent as a IEEE 754 floating point value, then a +Infinity or -Infinity result may be returned. If very large values with differing signs are used such that the SUM() or TOTAL() function is unable to determine if the correct result is +Infinity or -Infinity or some other value in between, then the result is NULL. Hence, for example, the following query returns NULL:

    WITH t1(x) AS (VALUES(1.0),(-9e+999),(2.0),(+9e+999),(3.0))
     SELECT sum(x) FROM t1;
"#,
    ],
    [
        "acos",
        r#"Return the arccosine of X. The result is in radians."#,
    ],
    ["acosh", r#"Return the hyperbolic arccosine of X."#],
    [
        "asin",
        r#"Return the arcsine of X. The result is in radians. "#,
    ],
    ["asinh", r#"Return the hyperbolic arcsine of X."#],
    [
        "atan",
        r#"Return the arctangent of X. The result is in radians."#,
    ],
    [
        "atan2",
        r#"Return the arctangent of Y/X. The result is in radians. The result is placed into correct quadrant depending on the signs of X and Y."#,
    ],
    ["atanh", r#"Return the hyperbolic arctangent of X. ]"#],
    [
        "ceil",
        r#"Return the first representable integer value greater than or equal to X. For positive values of X, this routine rounds away from zero. For negative values of X, this routine rounds toward zero."#,
    ],
    [
        "ceiling",
        r#"Return the first representable integer value greater than or equal to X. For positive values of X, this routine rounds away from zero. For negative values of X, this routine rounds toward zero."#,
    ],
    ["cos", r#"Return the cosine of X. X is in radians."#],
    ["cosh", r#"Return the hyperbolic cosine of X."#],
    ["degrees", r#"Convert value X from radians into degrees."#],
    [
        "exp",
        r#"Compute e (Euler's number, approximately 2.71828182845905) raised to the power X."#,
    ],
    [
        "floor",
        r#"Return the first representable integer value less than or equal to X. For positive numbers, this function rounds toward zero. For negative numbers, this function rounds away from zero."#,
    ],
    ["ln", r#"Return the natural logarithm of X."#],
    [
        "log",
        r#"Return the base-10 logarithm for X. Or, for the two-argument version, return the base-B logarithm of X.

    Compatibility note: SQLite works like PostgreSQL in that the log() function computes a base-10 logarithm. Most other SQL database engines compute a natural logarithm for log(). In the two-argument version of log(B,X), the first argument is the base and the second argument is the operand. This is the same as in PostgreSQL and MySQL, but is reversed from SQL Server which uses the second argument as the base and the first argument as the operand. "#,
    ],
    [
        "log10",
        r#"Return the base-10 logarithm for X. Or, for the two-argument version, return the base-B logarithm of X.

    Compatibility note: SQLite works like PostgreSQL in that the log() function computes a base-10 logarithm. Most other SQL database engines compute a natural logarithm for log(). In the two-argument version of log(B,X), the first argument is the base and the second argument is the operand. This is the same as in PostgreSQL and MySQL, but is reversed from SQL Server which uses the second argument as the base and the first argument as the operand. "#,
    ],
    [
        "log",
        r#"Return the base-10 logarithm for X. Or, for the two-argument version, return the base-B logarithm of X.

    Compatibility note: SQLite works like PostgreSQL in that the log() function computes a base-10 logarithm. Most other SQL database engines compute a natural logarithm for log(). In the two-argument version of log(B,X), the first argument is the base and the second argument is the operand. This is the same as in PostgreSQL and MySQL, but is reversed from SQL Server which uses the second argument as the base and the first argument as the operand. "#,
    ],
    ["log2", r#"Return the logarithm base-2 for the number X."#],
    [
        "mod",
        r#"Return the remainder after dividing X by Y. This is similar to the '%' operator, except that it works for non-integer arguments."#,
    ],
    ["pi", r#"Return an approximation for π."#],
    ["pow", r#"Compute X raised to the power Y."#],
    ["power", r#"Compute X raised to the power Y."#],
    ["radians", r#"Convert X from degrees into radians."#],
    ["sin", r#"Return the sine of X. X is in radians."#],
    ["sinh", r#"Return the hyperbolic sine of X."#],
    [
        "sqrt",
        r#"Return the square root of X. NULL is returned if X is negative."#,
    ],
    ["tan", r#"Return the tangent of X. X is in radians."#],
    ["tanh", r#"Return the hyperbolic tangent of X."#],
    [
        "trunc",
        r#"Return the representable integer in between X and 0 (inclusive) that is furthest away from zero. Or, in other words, return the integer part of X, rounding toward zero. The trunc() function is similar to ceiling(X) and floor(X) except that it always rounds toward zero whereas ceiling(X) and floor(X) round up and down, respectively."#,
    ],
    [
        "json",
        r#"The json(X) function verifies that its argument X is a valid JSON string or JSONB blob and returns a minified version of that JSON string with all unnecessary whitespace removed. If X is not a well-formed JSON string or JSONB blob, then this routine throws an error.

If the input is JSON5 text, then it is converted into canonical RFC-8259 text prior to being returned.

If the argument X to json(X) contains JSON objects with duplicate labels, then it is undefined whether or not the duplicates are preserved. The current implementation preserves duplicates. However, future enhancements to this routine may choose to silently remove duplicates.

Example:

    json(' { "this" : "is", "a": [ "test" ] } ') → '{"this":"is","a":["test"]}'
"#,
    ],
    [
        "jsonb",
        r#"The jsonb(X) function returns the binary JSONB representation of the JSON provided as argument X. An error is raised if X is TEXT that does not have valid JSON syntax.

If X is a BLOB and appears to be JSONB, then this routine simply returns a copy of X. Only the outer-most element of the JSONB input is examined, however. The deep structure of the JSONB is not validated."#,
    ],
    [
        "json_array",
        r#"The json_array() SQL function accepts zero or more arguments and returns a well-formed JSON array that is composed from those arguments. If any argument to json_array() is a BLOB then an error is thrown.

An argument with SQL type TEXT is normally converted into a quoted JSON string. However, if the argument is the output from another json1 function, then it is stored as JSON. This allows calls to json_array() and json_object() to be nested. The json() function can also be used to force strings to be recognized as JSON.

Examples:

    json_array(1,2,'3',4) → '[1,2,"3",4]'
    json_array('[1,2]') → '["[1,2]"]'
    json_array(json_array(1,2)) → '[[1,2]]'
    json_array(1,null,'3','[4,5]','{"six":7.7}') → '[1,null,"3","[4,5]","{\"six\":7.7}"]'
    json_array(1,null,'3',json('[4,5]'),json('{"six":7.7}')) → '[1,null,"3",[4,5],{"six":7.7}]'
"#,
    ],
    [
        "jsonb_array",
        r#"The jsonb_array() SQL function works just like the json_array() function except that it returns the constructed JSON array in the SQLite's private JSONB format rather than in the standard RFC 8259 text format. "#,
    ],
    [
        "json_array_length",
        r#"The json_array_length(X) function returns the number of elements in the JSON array X, or 0 if X is some kind of JSON value other than an array. The json_array_length(X,P) locates the array at path P within X and returns the length of that array, or 0 if path P locates an element in X that is not a JSON array, and NULL if path P does not locate any element of X. Errors are thrown if either X is not well-formed JSON or if P is not a well-formed path.

Examples:

    json_array_length('[1,2,3,4]') → 4
    json_array_length('[1,2,3,4]', '$') → 4
    json_array_length('[1,2,3,4]', '$[2]') → 0
    json_array_length('{"one":[1,2,3]}') → 0
    json_array_length('{"one":[1,2,3]}', '$.one') → 3
    json_array_length('{"one":[1,2,3]}', '$.two') → NULL
"#,
    ],
    [
        "json_error_position",
        r#"he json_error_position(X) function returns 0 if the input X is a well-formed JSON or JSON5 string. If the input X contains one or more syntax errors, then this function returns the character position of the first syntax error. The left-most character is position 1.

If the input X is a BLOB, then this routine returns 0 if X is a well-formed JSONB blob. If the return value is positive, then it represents the approximate 1-based position in the BLOB of the first detected error.

The json_error_position() function was added with SQLite version 3.42.0 (2023-05-16). "#,
    ],
    [
        "json_extract",
        r#"The json_extract(X,P1,P2,...) extracts and returns one or more values from the well-formed JSON at X. If only a single path P1 is provided, then the SQL datatype of the result is NULL for a JSON null, INTEGER or REAL for a JSON numeric value, an INTEGER zero for a JSON false value, an INTEGER one for a JSON true value, the dequoted text for a JSON string value, and a text representation for JSON object and array values. If there are multiple path arguments (P1, P2, and so forth) then this routine returns SQLite text which is a well-formed JSON array holding the various values.

Examples:

    json_extract('{"a":2,"c":[4,5,{"f":7}]}', '$') → '{"a":2,"c":[4,5,{"f":7}]}'
    json_extract('{"a":2,"c":[4,5,{"f":7}]}', '$.c') → '[4,5,{"f":7}]'
    json_extract('{"a":2,"c":[4,5,{"f":7}]}', '$.c[2]') → '{"f":7}'
    json_extract('{"a":2,"c":[4,5,{"f":7}]}', '$.c[2].f') → 7
    json_extract('{"a":2,"c":[4,5],"f":7}','$.c','$.a') → '[[4,5],2]'
    json_extract('{"a":2,"c":[4,5],"f":7}','$.c[#-1]') → 5
    json_extract('{"a":2,"c":[4,5,{"f":7}]}', '$.x') → NULL
    json_extract('{"a":2,"c":[4,5,{"f":7}]}', '$.x', '$.a') → '[null,2]'
    json_extract('{"a":"xyz"}', '$.a') → 'xyz'
    json_extract('{"a":null}', '$.a') → NULL

There is a subtle incompatibility between the json_extract() function in SQLite and the json_extract() function in MySQL. The MySQL version of json_extract() always returns JSON. The SQLite version of json_extract() only returns JSON if there are two or more PATH arguments (because the result is then a JSON array) or if the single PATH argument references an array or object. In SQLite, if json_extract() has only a single PATH argument and that PATH references a JSON null or a string or a numeric value, then json_extract() returns the corresponding SQL NULL, TEXT, INTEGER, or REAL value.

The difference between MySQL json_extract() and SQLite json_extract() really only stands out when accessing individual values within the JSON that are strings or NULLs. The following table demonstrates the difference: "#,
    ],
    [
        "jsonb_extract",
        r#" The jsonb_extract() function works the same as the json_extract() function, except in cases where json_extract() would normally return a text JSON array object, this routine returns the array or object in the JSONB format. For the common case where a text, numeric, null, or boolean JSON element is returned, this routine works exactly the same as json_extract()."#,
    ],
    [
        "json_insert",
        r#"The json_insert(), json_replace, and json_set() functions all take a single JSON value as their first argument followed by zero or more pairs of path and value arguments, and return a new JSON string formed by updating the input JSON by the path/value pairs. The functions differ only in how they deal with creating new values and overwriting preexisting values. "#,
    ],
    [
        "json_replace",
        r#"The json_insert(), json_replace, and json_set() functions all take a single JSON value as their first argument followed by zero or more pairs of path and value arguments, and return a new JSON string formed by updating the input JSON by the path/value pairs. The functions differ only in how they deal with creating new values and overwriting preexisting values. "#,
    ],
    [
        "json_set",
        r#"The json_insert(), json_replace, and json_set() functions all take a single JSON value as their first argument followed by zero or more pairs of path and value arguments, and return a new JSON string formed by updating the input JSON by the path/value pairs. The functions differ only in how they deal with creating new values and overwriting preexisting values. "#,
    ],
    [
        "jsonb_insert",
        r#"The jsonb_insert(), jsonb_replace(), and jsonb_set() functions work the same as json_insert(), json_replace(), and json_set(), respectively, except that "jsonb_" versions return their result in the binary JSONB format. "#,
    ],
    [
        "jsonb_replace",
        r#"The jsonb_insert(), jsonb_replace(), and jsonb_set() functions work the same as json_insert(), json_replace(), and json_set(), respectively, except that "jsonb_" versions return their result in the binary JSONB format. "#,
    ],
    [
        "jsonb_set",
        r#"The jsonb_insert(), jsonb_replace(), and jsonb_set() functions work the same as json_insert(), json_replace(), and json_set(), respectively, except that "jsonb_" versions return their result in the binary JSONB format. "#,
    ],
    [
        "json_object",
        r#"The json_object() SQL function accepts zero or more pairs of arguments and returns a well-formed JSON object that is composed from those arguments. The first argument of each pair is the label and the second argument of each pair is the value. If any argument to json_object() is a BLOB then an error is thrown."#,
    ],
    [
        "jsonb_object",
        r#"The jsonb_object() function works just like the json_object() function except that the generated object is returned in the binary JSONB format."#,
    ],
    [
        "json_patch",
        r#"The json_patch(T,P) SQL function runs the RFC-7396 MergePatch algorithm to apply patch P against input T. The patched copy of T is returned."#,
    ],
    [
        "jsonb_patch",
        r#"The jsonb_patch() function works just like the json_patch() function except that the patched JSON is returned in the binary JSONB format."#,
    ],
    [
        "json_pretty",
        r#" The json_pretty() function works like json() except that it adds extra whitespace to make the JSON result easier for humans to read. The first argument is the JSON or JSONB that is to be pretty-printed. The optional second argument is a text string that is used for indentation. If the second argument is omitted or is NULL, then indentation is four spaces per level.

The json_pretty() function was added with SQLite version 3.46.0 (2024-05-23). "#,
    ],
    [
        "json_remove",
        r#"The json_remove(X,P,...) function takes a single JSON value as its first argument followed by zero or more path arguments. The json_remove(X,P,...) function returns a copy of the X parameter with all the elements identified by path arguments removed. Paths that select elements not found in X are silently ignored.

Removals occurs sequentially from left to right. Changes caused by prior removals can affect the path search for subsequent arguments.

If the json_remove(X) function is called with no path arguments, then it returns the input X reformatted, with excess whitespace removed.

The json_remove() function throws an error if the first argument is not well-formed JSON or if any later argument is not a well-formed path.

Examples:

    json_remove('[0,1,2,3,4]','$[2]') → '[0,1,3,4]'
    json_remove('[0,1,2,3,4]','$[2]','$[0]') → '[1,3,4]'
    json_remove('[0,1,2,3,4]','$[0]','$[2]') → '[1,2,4]'
    json_remove('[0,1,2,3,4]','$[#-1]','$[0]') → '[1,2,3]'
    json_remove('{"x":25,"y":42}') → '{"x":25,"y":42}'
    json_remove('{"x":25,"y":42}','$.z') → '{"x":25,"y":42}'
    json_remove('{"x":25,"y":42}','$.y') → '{"x":25}'
    json_remove('{"x":25,"y":42}','$') → NULL
"#,
    ],
    [
        "jsonb_remove",
        r#"The jsonb_remove() function works just like the json_remove() function except that the edited JSON result is returned in the binary JSONB format. "#,
    ],
    [
        "json_type",
        r#"The json_type(X) function returns the "type" of the outermost element of X. The json_type(X,P) function returns the "type" of the element in X that is selected by path P. The "type" returned by json_type() is one of the following SQL text values: 'null', 'true', 'false', 'integer', 'real', 'text', 'array', or 'object'. If the path P in json_type(X,P) selects an element that does not exist in X, then this function returns NULL.

The json_type() function throws an error if its first argument is not well-formed JSON or JSONB or if its second argument is not a well-formed JSON path.

Examples:

    json_type('{"a":[2,3.5,true,false,null,"x"]}') → 'object'
    json_type('{"a":[2,3.5,true,false,null,"x"]}','$') → 'object'
    json_type('{"a":[2,3.5,true,false,null,"x"]}','$.a') → 'array'
    json_type('{"a":[2,3.5,true,false,null,"x"]}','$.a[0]') → 'integer'
    json_type('{"a":[2,3.5,true,false,null,"x"]}','$.a[1]') → 'real'
    json_type('{"a":[2,3.5,true,false,null,"x"]}','$.a[2]') → 'true'
    json_type('{"a":[2,3.5,true,false,null,"x"]}','$.a[3]') → 'false'
    json_type('{"a":[2,3.5,true,false,null,"x"]}','$.a[4]') → 'null'
    json_type('{"a":[2,3.5,true,false,null,"x"]}','$.a[5]') → 'text'
    json_type('{"a":[2,3.5,true,false,null,"x"]}','$.a[6]') → NULL
"#,
    ],
    [
        "json_valid",
        r#"The json_valid(X,Y) function return 1 if the argument X is well-formed JSON, or returns 0 if X is not well-formed. The Y parameter is an integer bitmask that defines what is meant by "well-formed". The following bits of Y are currently defined: "#,
    ],
    [
        "json_quote",
        r#"The json_quote(X) function converts the SQL value X (a number or a string) into its corresponding JSON representation. If X is a JSON value returned by another JSON function, then this function is a no-op. "#,
    ],
];

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
