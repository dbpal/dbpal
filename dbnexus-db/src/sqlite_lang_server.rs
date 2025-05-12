use std::borrow::Cow;
use std::ops::DerefMut;
use std::sync::Arc;

use sqlx::Row;

use crate::db::{FormatSqlOpts, SchemaTrait};
use crate::helper::sqlformat_format_sql;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, LangServerCommon, LanguageServer,
    LexerCompletionTrait, try_completion_on_syntax,
};
use crate::sqlite::SqliteDB;

#[async_trait::async_trait]
impl LangServerCommon for SqliteDB {}

#[async_trait::async_trait]
impl crate::language_server::LexerCompletionTrait for SqliteDB {
    type Dialect = sqlparser::dialect::SQLiteDialect;

    // https://sqlite.org/lang_keywords.html
    const KEY_WORDS: &'static [&'static str] = SQLITE_KEY_WORDS;
    const TEXTS: &'static [&'static str] = &["true"];
    const TYPES: &'static [&'static str] = SQLITE_TYPES;

    async fn extra_completion_item(&self) -> Vec<CompletionItem> {
        AstCompletionHelper::sqlite_pragma(self)
    }

    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        let mut ret: Vec<_> = SchemaTrait::list_table(self)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())?;
        ret.extend(
            SchemaTrait::list_view(self)
                .await
                .map(|x| x.into_iter().map(|x| x.name).collect::<Vec<_>>())?
                .into_iter(),
        );
        anyhow::Ok(ret)
    }

    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self, None, None, true).await
    }
}

#[async_trait::async_trait]
impl LanguageServer for SqliteDB {
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
            &sqlparser::dialect::SQLiteDialect {},
            sqlparser::tokenizer::Location {
                line: line as u64,
                column: col as u64,
            },
        )
        .await;
    }

    fn format_sql(&self, sql: &str, ops: Option<FormatSqlOpts>) -> anyhow::Result<String> {
        anyhow::Ok(sqlformat_format_sql(sql, ops.map(|x| x.into())))
    }
}

impl SqliteDB {
    pub(crate) async fn list_index_name_impl(&self) -> anyhow::Result<Vec<String>> {
        let sql = r#"
        SELECT name
            FROM sqlite_master
            WHERE type = 'index';"#;
        log::info!("sql:{}", sql);
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
impl AstCompletionHelper for SqliteDB {
    async fn list_view(
        &self, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        SchemaTrait::list_view(self)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn list_table(
        &self, _db: Option<&str>, _schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        SchemaTrait::list_table(self)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    async fn list_column(
        &self, _db: Option<&str>, _schema: Option<&str>, name: &str,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        use sqlx::{Column, Executor, Statement};
        let mut conn = self.get_conn().await?;
        let sql = format!(r#"select * from "{name}" limit 1"#);
        log::info!("sql:{}", sql);
        let s = conn.deref_mut().prepare_with(&sql, &[]).await?;
        let ret = s
            .columns()
            .iter()
            .map(|x| {
                CompletionItem {
                    item: x.name().to_string().into(),
                    desc: Some(x.type_info().to_string().into()),
                    kind: CompletionKind::Column,
                }
            })
            .collect::<Vec<_>>();
        anyhow::Ok(ret)
    }

    async fn list_function(
        &self, _catelog: Option<&str>, _schema: Option<&str>, _include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let vec = SQLITE_FN_AND_DESC
            .iter()
            .map(|x| {
                CompletionItem {
                    item: x[0].into(),
                    kind: CompletionKind::Function,
                    desc: Some(x[1].into()),
                }
            })
            .collect::<Vec<_>>();
        anyhow::Ok(vec)
    }

    async fn list_index(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        self.list_index_name_impl().await
    }

    fn sqlite_pragma(&self) -> Vec<CompletionItem> {
        sqlite_pragma()
    }
}

pub(crate) fn sqlite_pragma() -> Vec<CompletionItem> {
    static SS: &[[&'static str; 2]] = &[
        [
            "analysis_limit",
            r#"PRAGMA analysis_limit;
PRAGMA analysis_limit = N;

Query or change a limit on the approximate ANALYZE setting. This is the approximate number of rows examined in each index by the ANALYZE command. If the argument N is omitted, then the analysis limit is unchanged. If the limit is zero, then the analysis limit is disabled and the ANALYZE command will examine all rows of each index. If N is greater than zero, then the analysis limit is set to N and subsequent ANALYZE commands will stop analyzing each index after it has examined approximately N rows. If N is a negative number or something other than an integer value, then the pragma behaves as if the N argument was omitted. In all cases, the value returned is the new analysis limit used for subsequent ANALYZE commands.

This pragma can be used to help the ANALYZE command run faster on large databases. The results of analysis are not as good when only part of each index is examined, but the results are usually good enough. Setting N to 100 or 1000 allows the ANALYZE command to run quickly, even on enormous database files.

This pragma was added in SQLite version 3.32.0 (2020-05-22). The current implementation only uses the lower 31 bits of the N value - higher order bits are silently ignored. Future versions of SQLite might begin using higher order bits.

Beginning with SQLite version 3.46.0 (2024-05-23), the recommended way of running ANALYZE is with the PRAGMA optimize command. The PRAGMA optimize will automatically set a reasonable, temporary analysis limit that ensures that the PRAGMA optimize command will finish quickly even on enormous databases. Applications that use the PRAGMA optimize instead of running ANALYZE directly do not need to set an analysis limit. "#,
        ],
        [
            "application_id",
            r#"PRAGMA schema.application_id;
PRAGMA schema.application_id = integer ;

The application_id PRAGMA is used to query or set the 32-bit signed big-endian "Application ID" integer located at offset 68 into the database header. Applications that use SQLite as their application file-format should set the Application ID integer to a unique integer so that utilities such as file(1) can determine the specific file type rather than just reporting "SQLite3 Database". A list of assigned application IDs can be seen by consulting the magic.txt file in the SQLite source repository.

See also the user_version pragma. "#,
        ],
        [
            "auto_vacuum",
            r#"PRAGMA schema.auto_vacuum;
PRAGMA schema.auto_vacuum = 0 | NONE | 1 | FULL | 2 | INCREMENTAL;

Query or set the auto-vacuum status in the database.

The default setting for auto-vacuum is 0 or "none", unless the SQLITE_DEFAULT_AUTOVACUUM compile-time option is used. The "none" setting means that auto-vacuum is disabled. When auto-vacuum is disabled and data is deleted data from a database, the database file remains the same size. Unused database file pages are added to a "freelist" and reused for subsequent inserts. So no database file space is lost. However, the database file does not shrink. In this mode the VACUUM command can be used to rebuild the entire database file and thus reclaim unused disk space.

When the auto-vacuum mode is 1 or "full", the freelist pages are moved to the end of the database file and the database file is truncated to remove the freelist pages at every transaction commit. Note, however, that auto-vacuum only truncates the freelist pages from the file. Auto-vacuum does not defragment the database nor repack individual database pages the way that the VACUUM command does. In fact, because it moves pages around within the file, auto-vacuum can actually make fragmentation worse.

Auto-vacuuming is only possible if the database stores some additional information that allows each database page to be traced backwards to its referrer. Therefore, auto-vacuuming must be turned on before any tables are created. It is not possible to enable or disable auto-vacuum after a table has been created.

When the value of auto-vacuum is 2 or "incremental" then the additional information needed to do auto-vacuuming is stored in the database file but auto-vacuuming does not occur automatically at each commit as it does with auto_vacuum=full. In incremental mode, the separate incremental_vacuum pragma must be invoked to cause the auto-vacuum to occur.

The database connection can be changed between full and incremental autovacuum mode at any time. However, changing from "none" to "full" or "incremental" can only occur when the database is new (no tables have yet been created) or by running the VACUUM command. To change auto-vacuum modes, first use the auto_vacuum pragma to set the new desired mode, then invoke the VACUUM command to reorganize the entire database file. To change from "full" or "incremental" back to "none" always requires running VACUUM even on an empty database.

When the auto_vacuum pragma is invoked with no arguments, it returns the current auto_vacuum mode."#,
        ],
        [
            "automatic_index",
            r#"PRAGMA automatic_index;
PRAGMA automatic_index = boolean;

Query, set, or clear the automatic indexing capability.

Automatic indexing is enabled by default as of version 3.7.17 (2013-05-20), but this might change in future releases of SQLite. "#,
        ],
        [
            "busy_timeout",
            r#"PRAGMA busy_timeout;
PRAGMA busy_timeout = milliseconds;

Query or change the setting of the busy timeout. This pragma is an alternative to the sqlite3_busy_timeout() C-language interface which is made available as a pragma for use with language bindings that do not provide direct access to sqlite3_busy_timeout().

Each database connection can only have a single busy handler. This PRAGMA sets the busy handler for the process, possibly overwriting any previously set busy handler. "#,
        ],
        [
            "cache_size",
            r#"PRAGMA schema.cache_size;
PRAGMA schema.cache_size = pages;
PRAGMA schema.cache_size = -kibibytes;

Query or change the suggested maximum number of database disk pages that SQLite will hold in memory at once per open database file. Whether or not this suggestion is honored is at the discretion of the Application Defined Page Cache. The default page cache that is built into SQLite honors the request, however alternative application-defined page cache implementations may choose to interpret the suggested cache size in different ways or to ignore it all together. The default suggested cache size is -2000, which means the cache size is limited to 2048000 bytes of memory. The default suggested cache size can be altered using the SQLITE_DEFAULT_CACHE_SIZE compile-time options. The TEMP database has a default suggested cache size of 0 pages.

If the argument N is positive then the suggested cache size is set to N. If the argument N is negative, then the number of cache pages is adjusted to be a number of pages that would use approximately abs(N*1024) bytes of memory based on the current page size. SQLite remembers the number of pages in the page cache, not the amount of memory used. So if you set the cache size using a negative number and subsequently change the page size (using the PRAGMA page_size command) then the maximum amount of cache memory will go up or down in proportion to the change in page size.

Backwards compatibility note: The behavior of cache_size with a negative N was different prior to version 3.7.10 (2012-01-16). In earlier versions, the number of pages in the cache was set to the absolute value of N.

When you change the cache size using the cache_size pragma, the change only endures for the current session. The cache size reverts to the default value when the database is closed and reopened.

The default page cache implemention does not allocate the full amount of cache memory all at once. Cache memory is allocated in smaller chunks on an as-needed basis. The page_cache setting is a (suggested) upper bound on the amount of memory that the cache can use, not the amount of memory it will use all of the time. This is the behavior of the default page cache implementation, but an application defined page cache is free to behave differently if it wants. "#,
        ],
        [
            "cache_spill]",
            r#"PRAGMA cache_spill;
PRAGMA cache_spill=boolean;
PRAGMA schema.cache_spill=N;

The cache_spill pragma enables or disables the ability of the pager to spill dirty cache pages to the database file in the middle of a transaction. Cache_spill is enabled by default and most applications should leave it that way as cache spilling is usually advantageous. However, a cache spill has the side-effect of acquiring an EXCLUSIVE lock on the database file. Hence, some applications that have large long-running transactions may want to disable cache spilling in order to prevent the application from acquiring an exclusive lock on the database until the moment that the transaction COMMITs.

The "PRAGMA cache_spill=N" form of this pragma sets a minimum cache size threshold required for spilling to occur. The number of pages in cache must exceed both the cache_spill threshold and the maximum cache size set by the PRAGMA cache_size statement in order for spilling to occur.

The "PRAGMA cache_spill=boolean" form of this pragma applies across all databases attached to the database connection. But the "PRAGMA cache_spill=N" form of this statement only applies to the "main" schema or whatever other schema is specified as part of the statement. "#,
        ],
        [
            "case_sensitive_like",
            r#"PRAGMA case_sensitive_like = boolean;

The default behavior of the LIKE operator is to ignore case for ASCII characters. Hence, by default 'a' LIKE 'A' is true. The case_sensitive_like pragma installs a new application-defined LIKE function that is either case sensitive or insensitive depending on the value of the case_sensitive_like pragma. When case_sensitive_like is disabled, the default LIKE behavior is expressed. When case_sensitive_like is enabled, case becomes significant. So, for example, 'a' LIKE 'A' is false but 'a' LIKE 'a' is still true.

This pragma uses sqlite3_create_function() to overload the LIKE and GLOB functions, which may override previous implementations of LIKE and GLOB registered by the application. This pragma only changes the behavior of the SQL LIKE operator. It does not change the behavior of the sqlite3_strlike() C-language interface, which is always case insensitive.

WARNING: If a database uses the LIKE operator anywhere in the schema, such as in a CHECK constraint or in an expression index or in the WHERE clause of a partial index, then changing the definition of the LIKE operator using this PRAGMA can cause the database to appear to be corrupt. PRAGMA integrity_check will report errors. The database is not really corrupt in that changing the behavior of LIKE back to the way it was when the schema was defined and the database was populated will clear the problem. If the use of LIKE occurs only in indexes, then the problem can be cleared by running REINDEX. Nevertheless, the use of the case_sensitive_like pragma is discouraged.

This pragma is deprecated and exists for backwards compatibility only. New applications should avoid using this pragma. Older applications should discontinue use of this pragma at the earliest opportunity. This pragma may be omitted from the build when SQLite is compiled using SQLITE_OMIT_DEPRECATED. "#,
        ],
        [
            "cell_size_check",
            r#"PRAGMA cell_size_check
PRAGMA cell_size_check = boolean;

The cell_size_check pragma enables or disables additional sanity checking on database b-tree pages as they are initially read from disk. With cell size checking enabled, database corruption is detected earlier and is less likely to "spread". However, there is a small performance hit for doing the extra checks and so cell size checking is turned off by default. "#,
        ],
        [
            "checkpoint_fullfsync",
            r#"PRAGMA checkpoint_fullfsync
PRAGMA checkpoint_fullfsync = boolean;

Query or change the fullfsync flag for checkpoint operations. If this flag is set, then the F_FULLFSYNC syncing method is used during checkpoint operations on systems that support F_FULLFSYNC. The default value of the checkpoint_fullfsync flag is off. Only Mac OS-X supports F_FULLFSYNC.

If the fullfsync flag is set, then the F_FULLFSYNC syncing method is used for all sync operations and the checkpoint_fullfsync setting is irrelevant."#,
        ],
        [
            "collation_list",
            r#"PRAGMA collation_list;

Return a list of the collating sequences defined for the current database connection."#,
        ],
        [
            "compile_options",
            r#"

PRAGMA compile_options;

This pragma returns the names of compile-time options used when building SQLite, one option per row. The "SQLITE_" prefix is omitted from the returned option names. See also the sqlite3_compileoption_get() C/C++ interface and the sqlite_compileoption_get() SQL functions."#,
        ],
        [
            "count_changes",
            r#"PRAGMA count_changes;
PRAGMA count_changes = boolean;

Query or change the count-changes flag. Normally, when the count-changes flag is not set, INSERT, UPDATE and DELETE statements return no data. When count-changes is set, each of these commands returns a single row of data consisting of one integer value - the number of rows inserted, modified or deleted by the command. The returned change count does not include any insertions, modifications or deletions performed by triggers, any changes made automatically by foreign key actions, or updates caused by an upsert.

Another way to get the row change counts is to use the sqlite3_changes() or sqlite3_total_changes() interfaces. There is a subtle different, though. When an INSERT, UPDATE, or DELETE is run against a view using an INSTEAD OF trigger, the count_changes pragma reports the number of rows in the view that fired the trigger, whereas sqlite3_changes() and sqlite3_total_changes() do not.

This pragma is deprecated and exists for backwards compatibility only. New applications should avoid using this pragma. Older applications should discontinue use of this pragma at the earliest opportunity. This pragma may be omitted from the build when SQLite is compiled using SQLITE_OMIT_DEPRECATED. "#,
        ],
        [
            "data_store_directory",
            r#"PRAGMA data_store_directory;
PRAGMA data_store_directory = 'directory-name';

Query or change the value of the sqlite3_data_directory global variable, which windows operating-system interface backends use to determine where to store database files specified using a relative pathname.

Changing the data_store_directory setting is not threadsafe. Never change the data_store_directory setting if another thread within the application is running any SQLite interface at the same time. Doing so results in undefined behavior. Changing the data_store_directory setting writes to the sqlite3_data_directory global variable and that global variable is not protected by a mutex.

This facility is provided for WinRT which does not have an OS mechanism for reading or changing the current working directory. The use of this pragma in any other context is discouraged and may be disallowed in future releases.

This pragma is deprecated and exists for backwards compatibility only. New applications should avoid using this pragma. Older applications should discontinue use of this pragma at the earliest opportunity. This pragma may be omitted from the build when SQLite is compiled using SQLITE_OMIT_DEPRECATED. "#,
        ],
        [
            "data_version",
            r#"PRAGMA schema.data_version;

The "PRAGMA data_version" command provides an indication that the database file has been modified. Interactive programs that hold database content in memory or that display database content on-screen can use the PRAGMA data_version command to determine if they need to flush and reload their memory or update the screen display.

The integer values returned by two invocations of "PRAGMA data_version" from the same connection will be different if changes were committed to the database by any other connection in the interim. The "PRAGMA data_version" value is unchanged for commits made on the same database connection. The behavior of "PRAGMA data_version" is the same for all database connections, including database connections in separate processes and shared cache database connections.

The "PRAGMA data_version" value is a local property of each database connection and so values returned by two concurrent invocations of "PRAGMA data_version" on separate database connections are often different even though the underlying database is identical. It is only meaningful to compare the "PRAGMA data_version" values returned by the same database connection at two different points in time. "#,
        ],
        [
            "database_list",
            r#"

PRAGMA database_list;

This pragma works like a query to return one row for each database attached to the current database connection. The second column is "main" for the main database file, "temp" for the database file used to store TEMP objects, or the name of the ATTACHed database for other database files. The third column is the name of the database file itself, or an empty string if the database is not associated with a file."#,
        ],
        [
            "default_cache_size",
            r#"PRAGMA schema.default_cache_size;
PRAGMA schema.default_cache_size = Number-of-pages;

This pragma queries or sets the suggested maximum number of pages of disk cache that will be allocated per open database file. The difference between this pragma and cache_size is that the value set here persists across database connections. The value of the default cache size is stored in the 4-byte big-endian integer located at offset 48 in the header of the database file.

This pragma is deprecated and exists for backwards compatibility only. New applications should avoid using this pragma. Older applications should discontinue use of this pragma at the earliest opportunity. This pragma may be omitted from the build when SQLite is compiled using SQLITE_OMIT_DEPRECATED. "#,
        ],
        [
            "defer_foreign_keys",
            r#"PRAGMA defer_foreign_keys
PRAGMA defer_foreign_keys = boolean;

When the defer_foreign_keys PRAGMA is on, enforcement of all foreign key constraints is delayed until the outermost transaction is committed. The defer_foreign_keys pragma defaults to OFF so that foreign key constraints are only deferred if they are created as "DEFERRABLE INITIALLY DEFERRED". The defer_foreign_keys pragma is automatically switched off at each COMMIT or ROLLBACK. Hence, the defer_foreign_keys pragma must be separately enabled for each transaction. This pragma is only meaningful if foreign key constraints are enabled, of course.

The sqlite3_db_status(db,SQLITE_DBSTATUS_DEFERRED_FKS,...) C-language interface can be used during a transaction to determine if there are deferred and unresolved foreign key constraints."#,
        ],
        [
            "empty_result_callbacks",
            r#"

PRAGMA empty_result_callbacks;
PRAGMA empty_result_callbacks = boolean;

Query or change the empty-result-callbacks flag.

The empty-result-callbacks flag affects the sqlite3_exec() API only. Normally, when the empty-result-callbacks flag is cleared, the callback function supplied to the sqlite3_exec() is not invoked for commands that return zero rows of data. When empty-result-callbacks is set in this situation, the callback function is invoked exactly once, with the third parameter set to 0 (NULL). This is to enable programs that use the sqlite3_exec() API to retrieve column-names even when a query returns no data.

This pragma is deprecated and exists for backwards compatibility only. New applications should avoid using this pragma. Older applications should discontinue use of this pragma at the earliest opportunity. This pragma may be omitted from the build when SQLite is compiled using SQLITE_OMIT_DEPRECATED. "#,
        ],
        [
            "encoding",
            r#"PRAGMA encoding;
PRAGMA encoding = 'UTF-8';
PRAGMA encoding = 'UTF-16';
PRAGMA encoding = 'UTF-16le';
PRAGMA encoding = 'UTF-16be';

In first form, if the main database has already been created, then this pragma returns the text encoding used by the main database, one of 'UTF-8', 'UTF-16le' (little-endian UTF-16 encoding) or 'UTF-16be' (big-endian UTF-16 encoding). If the main database has not already been created, then the value returned is the text encoding that will be used to create the main database, if it is created by this session.

The second through fifth forms of this pragma set the encoding that the main database will be created with if it is created by this session. The string 'UTF-16' is interpreted as "UTF-16 encoding using native machine byte-ordering". It is not possible to change the text encoding of a database after it has been created and any attempt to do so will be silently ignored.

If no encoding is first set with this pragma, then the encoding with which the main database will be created defaults to one determined by the API used to open the connection.

Once an encoding has been set for a database, it cannot be changed.

Databases created by the ATTACH command always use the same encoding as the main database. An attempt to ATTACH a database with a different text encoding from the "main" database will fail."#,
        ],
        [
            "foreign_key_check",
            r#"PRAGMA schema.foreign_key_check;
PRAGMA schema.foreign_key_check(table-name);

The foreign_key_check pragma checks the database, or the table called "table-name", for foreign key constraints that are violated. The foreign_key_check pragma returns one row output for each foreign key violation. There are four columns in each result row. The first column is the name of the table that contains the REFERENCES clause. The second column is the rowid of the row that contains the invalid REFERENCES clause, or NULL if the child table is a WITHOUT ROWID table. The third column is the name of the table that is referred to. The fourth column is the index of the specific foreign key constraint that failed. The fourth column in the output of the foreign_key_check pragma is the same integer as the first column in the output of the foreign_key_list pragma. When a "table-name" is specified, the only foreign key constraints checked are those created by REFERENCES clauses in the CREATE TABLE statement for table-name."#,
        ],
        [
            "foreign_key_list",
            r#"

PRAGMA foreign_key_list(table-name);

This pragma returns one row for each foreign key constraint created by a REFERENCES clause in the CREATE TABLE statement of table "table-name". "#,
        ],
        [
            "foreign_keys",
            r#"PRAGMA foreign_keys;
PRAGMA foreign_keys = boolean;

Query, set, or clear the enforcement of foreign key constraints.

This pragma is a no-op within a transaction; foreign key constraint enforcement may only be enabled or disabled when there is no pending BEGIN or SAVEPOINT.

Changing the foreign_keys setting affects the execution of all statements prepared using the database connection, including those prepared before the setting was changed. Any existing statements prepared using the legacy sqlite3_prepare() interface may fail with an SQLITE_SCHEMA error after the foreign_keys setting is changed.

As of SQLite version 3.6.19, the default setting for foreign key enforcement is OFF. However, that might change in a future release of SQLite. The default setting for foreign key enforcement can be specified at compile-time using the SQLITE_DEFAULT_FOREIGN_KEYS preprocessor macro. To minimize future problems, applications should set the foreign key enforcement flag as required by the application and not depend on the default setting. "#,
        ],
        [
            "freelist_count",
            r#"PRAGMA schema.freelist_count;

Return the number of unused pages in the database file."#,
        ],
        [
            "full_column_names",
            r#"PRAGMA full_column_names;
PRAGMA full_column_names = boolean;

Query or change the full_column_names flag. This flag together with the short_column_names flag determine the way SQLite assigns names to result columns of SELECT statements. Result columns are named by applying the following rules in order:

If there is an AS clause on the result, then the name of the column is the right-hand side of the AS clause.

If the result is a general expression, not a just the name of a source table column, then the name of the result is a copy of the expression text.

If the short_column_names pragma is ON, then the name of the result is the name of the source table column without the source table name prefix: COLUMN.

If both pragmas short_column_names and full_column_names are OFF then case (2) applies.

The name of the result column is a combination of the source table and source column name: TABLE.COLUMN

This pragma is deprecated and exists for backwards compatibility only. New applications should avoid using this pragma. Older applications should discontinue use of this pragma at the earliest opportunity. This pragma may be omitted from the build when SQLite is compiled using SQLITE_OMIT_DEPRECATED. "#,
        ],
        [
            "fullfsync",
            r#"PRAGMA fullfsync
PRAGMA fullfsync = boolean;

Query or change the fullfsync flag. This flag determines whether or not the F_FULLFSYNC syncing method is used on systems that support it. The default value of the fullfsync flag is off. Only Mac OS X supports F_FULLFSYNC.

See also checkpoint_fullfsync."#,
        ],
        [
            "function_list",
            r#"PRAGMA function_list;

This pragma returns a list of SQL functions known to the database connection. Each row of the result describes a single calling signature for a single SQL function. Some SQL functions will have multiple rows in the result set if they can (for example) be invoked with a varying number of arguments or can accept text in various encodings. "#,
        ],
        [
            "hard_heap_limit",
            r#"PRAGMA hard_heap_limit
PRAGMA hard_heap_limit=N

This pragma invokes the sqlite3_hard_heap_limit64() interface with the argument N, if N is specified and N is a positive integer that is less than the current hard heap limit. The hard_heap_limit pragma always returns the same integer that would be returned by the sqlite3_hard_heap_limit64(-1) C-language function. That is to say, it always returns the value of the hard heap limit that is set after any changes imposed by this PRAGMA.

This pragma can only lower the heap limit, never raise it. The C-language interface sqlite3_hard_heap_limit64() must be used to raise the heap limit.

See also the soft_heap_limit pragma. "#,
        ],
        [
            "ignore_check_constraints",
            r#"PRAGMA ignore_check_constraints = boolean;

This pragma enables or disables the enforcement of CHECK constraints. The default setting is off, meaning that CHECK constraints are enforced by default."#,
        ],
        [
            "incremental_vacuum",
            r#"PRAGMA schema.incremental_vacuum(N);
PRAGMA schema.incremental_vacuum;

The incremental_vacuum pragma causes up to N pages to be removed from the freelist. The database file is truncated by the same amount. The incremental_vacuum pragma has no effect if the database is not in auto_vacuum=incremental mode or if there are no pages on the freelist. If there are fewer than N pages on the freelist, or if N is less than 1, or if the "(N)" argument is omitted, then the entire freelist is cleared."#,
        ],
        [
            "index_info",
            r#"PRAGMA schema.index_info(index-name);

This pragma returns one row for each key column in the named index. A key column is a column that is actually named in the CREATE INDEX index statement or UNIQUE constraint or PRIMARY KEY constraint that created the index. Index entries also usually contain auxiliary columns that point back to the table row being indexed. The auxiliary index-columns are not shown by the index_info pragma, but they are listed by the index_xinfo pragma.

Output columns from the index_info pragma are as follows:

The rank of the column within the index. (0 means left-most.)
The rank of the column within the table being indexed. A value of -1 means rowid and a value of -2 means that an expression is being used.
The name of the column being indexed. This columns is NULL if the column is the rowid or an expression. 

If there is no index named index-name but there is a WITHOUT ROWID table with that name, then (as of SQLite version 3.30.0 on 2019-10-04) this pragma returns the PRIMARY KEY columns of the WITHOUT ROWID table as they are used in the records of the underlying b-tree, which is to say with duplicate columns removed. "#,
        ],
        [
            "index_list",
            r#"

PRAGMA schema.index_list(table-name);

This pragma returns one row for each index associated with the given table.

Output columns from the index_list pragma are as follows:

A sequence number assigned to each index for internal tracking purposes.
The name of the index.
"1" if the index is UNIQUE and "0" if not.
"c" if the index was created by a CREATE INDEX statement, "u" if the index was created by a UNIQUE constraint, or "pk" if the index was created by a PRIMARY KEY constraint.
"1" if the index is a partial index and "0" if not. "#,
        ],
        [
            "index_xinfo",
            r#"PRAGMA schema.index_xinfo(index-name);

This pragma returns information about every column in an index. Unlike this index_info pragma, this pragma returns information about every column in the index, not just the key columns. (A key column is a column that is actually named in the CREATE INDEX index statement or UNIQUE constraint or PRIMARY KEY constraint that created the index. Auxiliary columns are additional columns needed to locate the table entry that corresponds to each index entry.)

Output columns from the index_xinfo pragma are as follows:

The rank of the column within the index. (0 means left-most. Key columns come before auxiliary columns.)
The rank of the column within the table being indexed, or -1 if the index-column is the rowid of the table being indexed and -2 if the index is on an expression.
The name of the column being indexed, or NULL if the index-column is the rowid of the table being indexed or an expression.
1 if the index-column is sorted in reverse (DESC) order by the index and 0 otherwise.
The name for the collating sequence used to compare values in the index-column.
1 if the index-column is a key column and 0 if the index-column is an auxiliary column. 

If there is no index named index-name but there is a WITHOUT ROWID table with that name, then (as of SQLite version 3.30.0 on 2019-10-04) this pragma returns the columns of the WITHOUT ROWID table as they are used in the records of the underlying b-tree, which is to say with de-duplicated PRIMARY KEY columns first followed by data columns. "#,
        ],
        [
            "integrity_check",
            r#"

PRAGMA schema.integrity_check;
PRAGMA schema.integrity_check(N)
PRAGMA schema.integrity_check(TABLENAME)

This pragma does a low-level formatting and consistency check of the database. The integrity_check pragma look for:

Table or index entries that are out of sequence
Misformatted records
Missing pages
Missing or surplus index entries
UNIQUE, CHECK, and NOT NULL constraint errors
Integrity of the freelist
Sections of the database that are used more than once, or not at all 

If the integrity_check pragma finds problems, strings are returned (as multiple rows with a single column per row) which describe the problems. Pragma integrity_check will return at most N errors before the analysis quits, with N defaulting to 100. If pragma integrity_check finds no errors, a single row with the value 'ok' is returned.

The usual case is that the entire database file is checked. However, if the argument is TABLENAME, then checking is only performed for the the table named and its associated indexes. This is called a "partial integrity check". Because only a subset of the database is checked, errors such as unused sections of the file or duplication use of the same section of the file by two or more tables cannot be detected. The freelist is only verified on a partial integrity check if TABLENAME is sqlite_schema or one of its aliases. Support for partial integrity checks was added with version 3.33.0 (2020-08-14).

PRAGMA integrity_check does not find FOREIGN KEY errors. Use the PRAGMA foreign_key_check command to find errors in FOREIGN KEY constraints.

See also the PRAGMA quick_check command which does most of the checking of PRAGMA integrity_check but runs much faster."#,
        ],
        [
            "journal_mode",
            r#"PRAGMA schema.journal_mode;
PRAGMA schema.journal_mode = DELETE | TRUNCATE | PERSIST | MEMORY | WAL | OFF

This pragma queries or sets the journal mode for databases associated with the current database connection.

The first form of this pragma queries the current journaling mode for database. When database is omitted, the "main" database is queried.

The second form changes the journaling mode for "database" or for all attached databases if "database" is omitted. The new journal mode is returned. If the journal mode could not be changed, the original journal mode is returned.

The DELETE journaling mode is the normal behavior. In the DELETE mode, the rollback journal is deleted at the conclusion of each transaction. Indeed, the delete operation is the action that causes the transaction to commit. (See the document titled Atomic Commit In SQLite for additional detail.)

The TRUNCATE journaling mode commits transactions by truncating the rollback journal to zero-length instead of deleting it. On many systems, truncating a file is much faster than deleting the file since the containing directory does not need to be changed.

The PERSIST journaling mode prevents the rollback journal from being deleted at the end of each transaction. Instead, the header of the journal is overwritten with zeros. This will prevent other database connections from rolling the journal back. The PERSIST journaling mode is useful as an optimization on platforms where deleting or truncating a file is much more expensive than overwriting the first block of a file with zeros. See also: PRAGMA journal_size_limit and SQLITE_DEFAULT_JOURNAL_SIZE_LIMIT.

The MEMORY journaling mode stores the rollback journal in volatile RAM. This saves disk I/O but at the expense of database safety and integrity. If the application using SQLite crashes in the middle of a transaction when the MEMORY journaling mode is set, then the database file will very likely go corrupt.

The WAL journaling mode uses a write-ahead log instead of a rollback journal to implement transactions. The WAL journaling mode is persistent; after being set it stays in effect across multiple database connections and after closing and reopening the database. A database in WAL journaling mode can only be accessed by SQLite version 3.7.0 (2010-07-21) or later.

The OFF journaling mode disables the rollback journal completely. No rollback journal is ever created and hence there is never a rollback journal to delete. The OFF journaling mode disables the atomic commit and rollback capabilities of SQLite. The ROLLBACK command no longer works; it behaves in an undefined way. Applications must avoid using the ROLLBACK command when the journal mode is OFF. If the application crashes in the middle of a transaction when the OFF journaling mode is set, then the database file will very likely go corrupt. Without a journal, there is no way for a statement to unwind partially completed operations following a constraint error. This might also leave the database in a corrupted state. For example, if a duplicate entry causes a CREATE UNIQUE INDEX statement to fail half-way through, it will leave behind a partially created, and hence corrupt, index. Because OFF journaling mode allows the database file to be corrupted using ordinary SQL, it is disabled when SQLITE_DBCONFIG_DEFENSIVE is enabled.

Note that the journal_mode for an in-memory database is either MEMORY or OFF and can not be changed to a different value. An attempt to change the journal_mode of an in-memory database to any setting other than MEMORY or OFF is ignored. Note also that the journal_mode cannot be changed while a transaction is active."#,
        ],
        [
            "journal_size_limit",
            r#" PRAGMA schema.journal_size_limit
PRAGMA schema.journal_size_limit = N ;

If a database connection is operating in exclusive locking mode or in persistent journal mode (PRAGMA journal_mode=persist) then after committing a transaction the rollback journal file may remain in the file-system. This increases performance for subsequent transactions since overwriting an existing file is faster than append to a file, but it also consumes file-system space. After a large transaction (e.g. a VACUUM), the rollback journal file may consume a very large amount of space.

Similarly, in WAL mode, the write-ahead log file is not truncated following a checkpoint. Instead, SQLite reuses the existing file for subsequent WAL entries since overwriting is faster than appending.

The journal_size_limit pragma may be used to limit the size of rollback-journal and WAL files left in the file-system after transactions or checkpoints. Each time a transaction is committed or a WAL file resets, SQLite compares the size of the rollback journal file or WAL file left in the file-system to the size limit set by this pragma and if the journal or WAL file is larger it is truncated to the limit.

The second form of the pragma listed above is used to set a new limit in bytes for the specified database. A negative number implies no limit. To always truncate rollback journals and WAL files to their minimum size, set the journal_size_limit to zero. Both the first and second forms of the pragma listed above return a single result row containing a single integer column - the value of the journal size limit in bytes. The default journal size limit is -1 (no limit). The SQLITE_DEFAULT_JOURNAL_SIZE_LIMIT preprocessor macro can be used to change the default journal size limit at compile-time.

This pragma only operates on the single database specified prior to the pragma name (or on the "main" database if no database is specified.) There is no way to change the journal size limit on all attached databases using a single PRAGMA statement. The size limit must be set separately for each attached database. "#,
        ],
        [
            "legacy_alter_table",
            r#"PRAGMA legacy_alter_table;
PRAGMA legacy_alter_table = boolean

This pragma sets or queries the value of the legacy_alter_table flag. When this flag is on, the ALTER TABLE RENAME command (for changing the name of a table) works as it did in SQLite 3.24.0 (2018-06-04) and earlier. More specifically, when this flag is on the ALTER TABLE RENAME command only rewrites the initial occurrence of the table name in its CREATE TABLE statement and in any associated CREATE INDEX and CREATE TRIGGER statements. Other references to the table are unmodified, including:

References to the table within the bodies of triggers and views.
References to the table within CHECK constraints in the original CREATE TABLE statement.
References to the table within the WHERE clauses of partial indexes. 

The default setting for this pragma is OFF, which means that all references to the table anywhere in the schema are converted to the new name.

This pragma is provided as a work-around for older programs that contain code that expect the incomplete behavior of ALTER TABLE RENAME found in older versions of SQLite. New applications should leave this flag turned off.

For compatibility with older virtual table implementations, this flag is turned on temporarily while the sqlite3_module.xRename method is being run. The value of this flag is restored after the sqlite3_module.xRename method finishes.

The legacy alter table behavior can also be toggled on and off using the SQLITE_DBCONFIG_LEGACY_ALTER_TABLE option to the sqlite3_db_config() interface.

The legacy alter table behavior is a per-connection setting. Turning this features on or off affects all attached database files within the database connection. The setting does not persist. Changing this setting in one connection does not affect any other connections. "#,
        ],
        [
            "legacy_file_format",
            r#"

PRAGMA legacy_file_format;

This pragma no longer functions. It has become a no-op. The capabilities formerly provided by PRAGMA legacy_file_format are now available using the SQLITE_DBCONFIG_LEGACY_FILE_FORMAT option to the sqlite3_db_config() C-language interface. "#,
        ],
        [
            "locking_mode",
            r#"PRAGMA schema.locking_mode;
PRAGMA schema.locking_mode = NORMAL | EXCLUSIVE

This pragma sets or queries the database connection locking-mode. The locking-mode is either NORMAL or EXCLUSIVE.

In NORMAL locking-mode (the default unless overridden at compile-time using SQLITE_DEFAULT_LOCKING_MODE), a database connection unlocks the database file at the conclusion of each read or write transaction. When the locking-mode is set to EXCLUSIVE, the database connection never releases file-locks. The first time the database is read in EXCLUSIVE mode, a shared lock is obtained and held. The first time the database is written, an exclusive lock is obtained and held.

Database locks obtained by a connection in EXCLUSIVE mode may be released either by closing the database connection, or by setting the locking-mode back to NORMAL using this pragma and then accessing the database file (for read or write). Simply setting the locking-mode to NORMAL is not enough - locks are not released until the next time the database file is accessed.

There are three reasons to set the locking-mode to EXCLUSIVE.

The application wants to prevent other processes from accessing the database file.
The number of system calls for filesystem operations is reduced, possibly resulting in a small performance increase.
WAL databases can be accessed in EXCLUSIVE mode without the use of shared memory. (Additional information) 

When the locking_mode pragma specifies a particular database, for example:

PRAGMA main.locking_mode=EXCLUSIVE; 

then the locking mode applies only to the named database. If no database name qualifier precedes the "locking_mode" keyword then the locking mode is applied to all databases, including any new databases added by subsequent ATTACH commands.

The "temp" database (in which TEMP tables and indices are stored) and in-memory databases always uses exclusive locking mode. The locking mode of temp and in-memory databases cannot be changed. All other databases use the normal locking mode by default and are affected by this pragma.

If the locking mode is EXCLUSIVE when first entering WAL journal mode, then the locking mode cannot be changed to NORMAL until after exiting WAL journal mode. If the locking mode is NORMAL when first entering WAL journal mode, then the locking mode can be changed between NORMAL and EXCLUSIVE and back again at any time and without needing to exit WAL journal mode."#,
        ],
        [
            "max_page_count",
            r#"PRAGMA schema.max_page_count;
PRAGMA schema.max_page_count = N;

Query or set the maximum number of pages in the database file. Both forms of the pragma return the maximum page count. The second form attempts to modify the maximum page count. The maximum page count cannot be reduced below the current database size. "#,
        ],
        [
            "mmap_size",
            r#"
PRAGMA schema.mmap_size;
PRAGMA schema.mmap_size=N

Query or change the maximum number of bytes that are set aside for memory-mapped I/O on a single database. The first form (without an argument) queries the current limit. The second form (with a numeric argument) sets the limit for the specified database, or for all databases if the optional database name is omitted. In the second form, if the database name is omitted, the limit that is set becomes the default limit for all databases that are added to the database connection by subsequent ATTACH statements.

The argument N is the maximum number of bytes of the database file that will be accessed using memory-mapped I/O. If N is zero then memory mapped I/O is disabled. If N is negative, then the limit reverts to the default value determined by the most recent sqlite3_config(SQLITE_CONFIG_MMAP_SIZE), or to the compile time default determined by SQLITE_DEFAULT_MMAP_SIZE if not start-time limit has been set.

The PRAGMA mmap_size statement will never increase the amount of address space used for memory-mapped I/O above the hard limit set by the SQLITE_MAX_MMAP_SIZE compile-time option, nor the hard limit set at startup-time by the second argument to sqlite3_config(SQLITE_CONFIG_MMAP_SIZE)

The size of the memory-mapped I/O region cannot be changed while the memory-mapped I/O region is in active use, to avoid unmapping memory out from under running SQL statements. For this reason, the mmap_size pragma may be a no-op if the prior mmap_size is non-zero and there are other SQL statements running concurrently on the same database connection."#,
        ],
        [
            "module_list",
            r#"PRAGMA module_list;

This pragma returns a list of virtual table modules registered with the database connection. "#,
        ],
        [
            "optimize",
            r#"PRAGMA optimize;
PRAGMA optimize(MASK);
PRAGMA schema.optimize;
PRAGMA schema.optimize(MASK);

Attempt to optimize the database. All schemas are optimized in the first two forms, and only the specified schema is optimized in the latter two.

In most applications, using PRAGMA optimize as follows will help SQLite to achieve the best possible query performance:

Applications with short-lived database connections should run "PRAGMA optimize;" once, just prior to closing each database connection.

Applications that use long-lived database connections should run "PRAGMA optimize=0x10002;" when the connection is first opened, and then also run "PRAGMA optimize;" periodically, perhaps once per day or once per hour.

All applications should run "PRAGMA optimize;" after a schema change, especially after one or more CREATE INDEX statements. 

This pragma is usually a no-op or nearly so and is very fast. On the occasions where it does need to run ANALYZE on one or more tables, it sets a temporary analysis limit, valid for the duration of this pragma only, that prevents the ANALYZE invocations from running for too long.

Recommended practice is that applications with short-lived database connections should run "PRAGMA optimize" once when the database connection closes. Applications with long-lived database connections should run "PRAGMA optimize=0x10002" when the database connection first opens, then run "PRAGMA optimize" again at periodic intervals - perhaps once per day. All applications should run "PRAGMA optimize" after schema changes, especially CREATE INDEX.

The details of optimizations performed by this pragma are expected to change and improve over time. Applications should anticipate that this pragma will perform new optimizations in future releases.

The optional MASK argument is a bitmask of optimizations to perform:
0x00001 	Debugging mode. Do not actually perform any optimizations but instead return one line of text for each optimization that would have been done. Off by default.
0x00002 	Run ANALYZE on tables that might benefit. On by default.
0x00010 	When running ANALYZE, set a temporary PRAGMA analysis_limit to prevent excess run-time. On by default.
0x10000 	Check the size of all tables, not just tables that have not been recently used, to see if any have grown and shrunk significantly and hence might benefit from being re-analyzed. Off by default.

The default MASK is 0xfffe.

To see all optimizations that would have been done without actually doing them, run "PRAGMA optimize(-1)".

Determination Of When To Run Analyze

In the current implementation, a table is analyzed if and only if all of the following are true:

MASK bit 0x02 is set.
The table is an ordinary table, not a view or virtual table.
The table name does not begin with "sqlite_".
One or more of the following are true:
    The 0x10000 bit of MASK is set
    One or more indexes on the table lack entries in the sqlite_stat1 table.
    The query planner used sqlite_stat1 statistics for one or more indexes of this table at some point during the lifetime of the current database connection. 
One or more of the following are true:
    One or more indexes on the table lack entries in the sqlite_stat1 table.
    The number of rows in the table has increased or decreased by 10-fold since the last time ANALYZE was run on the table. 

The rules for when tables are analyzed are likely to change in future releases. New MASK values may be added in the future. Future versions of this pragma might accept a string literal argument instead of a bit mask, though the bit mask argument will continue to be supported for backwards compatibility. "#,
        ],
        [
            "page_count",
            r#"PRAGMA schema.page_count;

Return the total number of pages in the database file."#,
        ],
        [
            "page_size",
            r#"PRAGMA schema.page_count;

Return the total number of pages in the database file."#,
        ],
        [
            "parser_trace",
            r#"PRAGMA parser_trace = boolean;

If SQLite has been compiled with the SQLITE_DEBUG compile-time option, then the parser_trace pragma can be used to turn on tracing for the SQL parser used internally by SQLite. This feature is used for debugging SQLite itself.

This pragma is intended for use when debugging SQLite itself. It is only available when the SQLITE_DEBUG compile-time option is used."#,
        ],
        [
            "pragma_list",
            r#"PRAGMA pragma_list;

This pragma returns a list of PRAGMA commands known to the database connection. "#,
        ],
        [
            "query_only",
            r#"PRAGMA query_only;
PRAGMA query_only = boolean;

The query_only pragma prevents data changes on database files when enabled. When this pragma is enabled, any attempt to CREATE, DELETE, DROP, INSERT, or UPDATE will result in an SQLITE_READONLY error. However, the database is not truly read-only. You can still run a checkpoint or a COMMIT and the return value of the sqlite3_db_readonly() routine is not affected. "#,
        ],
        [
            "quick_check",
            r#"PRAGMA schema.quick_check;
PRAGMA schema.quick_check(N)
PRAGMA schema.quick_check(TABLENAME)

The pragma is like integrity_check except that it does not verify UNIQUE constraints and does not verify that index content matches table content. By skipping UNIQUE and index consistency checks, quick_check is able to run faster. PRAGMA quick_check runs in O(N) time whereas PRAGMA integrity_check requires O(NlogN) time where N is the total number of rows in the database. Otherwise the two pragmas are the same. "#,
        ],
        [
            "read_uncommitted",
            r#"PRAGMA read_uncommitted;
PRAGMA read_uncommitted = boolean;

Query, set, or clear READ UNCOMMITTED isolation. The default isolation level for SQLite is SERIALIZABLE. Any process or thread can select READ UNCOMMITTED isolation, but SERIALIZABLE will still be used except between connections that share a common page and schema cache. Cache sharing is enabled using the sqlite3_enable_shared_cache() API. Cache sharing is disabled by default.

See SQLite Shared-Cache Mode for additional information."#,
        ],
        [
            "recursive_triggers",
            r#"PRAGMA recursive_triggers;
PRAGMA recursive_triggers = boolean;

Query, set, or clear the recursive trigger capability.

Changing the recursive_triggers setting affects the execution of all statements prepared using the database connection, including those prepared before the setting was changed. Any existing statements prepared using the legacy sqlite3_prepare() interface may fail with an SQLITE_SCHEMA error after the recursive_triggers setting is changed.

Prior to SQLite version 3.6.18 (2009-09-11), recursive triggers were not supported. The behavior of SQLite was always as if this pragma was set to OFF. Support for recursive triggers was added in version 3.6.18 but was initially turned OFF by default, for compatibility. Recursive triggers may be turned on by default in future versions of SQLite.

The depth of recursion for triggers has a hard upper limit set by the SQLITE_MAX_TRIGGER_DEPTH compile-time option and a run-time limit set by sqlite3_limit(db,SQLITE_LIMIT_TRIGGER_DEPTH,...)."#,
        ],
        [
            "reverse_unordered_selects",
            r#"

PRAGMA reverse_unordered_selects;
PRAGMA reverse_unordered_selects = boolean;

When enabled, this PRAGMA causes many SELECT statements without an ORDER BY clause to emit their results in the reverse order from what they normally would. This can help debug applications that are making invalid assumptions about the result order. The reverse_unordered_selects pragma works for most SELECT statements, however the query planner may sometimes choose an algorithm that is not easily reversed, in which case the output will appear in the same order regardless of the reverse_unordered_selects setting.

SQLite makes no guarantees about the order of results if a SELECT omits the ORDER BY clause. Even so, the order of results does not change from one run to the next, and so many applications mistakenly come to depend on the arbitrary output order whatever that order happens to be. However, sometimes new versions of SQLite will contain optimizer enhancements that will cause the output order of queries without ORDER BY clauses to shift. When that happens, applications that depend on a certain output order might malfunction. By running the application multiple times with this pragma both disabled and enabled, cases where the application makes faulty assumptions about output order can be identified and fixed early, reducing problems that might be caused by linking against a different version of SQLite. "#,
        ],
        [
            "schema_version",
            r#"PRAGMA schema.schema_version;
PRAGMA schema.schema_version = integer ;

The schema_version pragma will get or set the value of the schema-version integer at offset 40 in the database header.

SQLite automatically increments the schema-version whenever the schema changes. As each SQL statement runs, the schema version is checked to ensure that the schema has not changed since the SQL statement was prepared. Subverting this mechanism by using "PRAGMA schema_version=N" to change the value of the schema_version may cause SQL statement to run using an obsolete schema, which can lead to incorrect answers and/or database corruption. It is always safe to read the schema_version, but changing the schema_version can cause problems. For this reason, attempts to change the value of schema_version are a silent no-op when defensive mode is enabled for a database connection.

Warning: Misuse of this pragma can result in database corruption.

For the purposes of this pragma, the VACUUM command is considered a schema change, since VACUUM will usually alter the "rootpage" values for entries in the sqlite_schema table.

See also the application_id pragma and user_version pragma. "#,
        ],
        [
            "secure_delete",
            r#"PRAGMA schema.secure_delete;
PRAGMA schema.secure_delete = boolean|FAST

Query or change the secure-delete setting. When secure_delete is on, SQLite overwrites deleted content with zeros. The default setting for secure_delete is determined by the SQLITE_SECURE_DELETE compile-time option and is normally off. The off setting for secure_delete improves performance by reducing the number of CPU cycles and the amount of disk I/O. Applications that wish to avoid leaving forensic traces after content is deleted or updated should enable the secure_delete pragma prior to performing the delete or update, or else run VACUUM after the delete or update.

The "fast" setting for secure_delete (added circa 2017-08-01) is an intermediate setting in between "on" and "off". When secure_delete is set to "fast", SQLite will overwrite deleted content with zeros only if doing so does not increase the amount of I/O. In other words, the "fast" setting uses more CPU cycles but does not use more I/O. This has the effect of purging all old content from b-tree pages, but leaving forensic traces on freelist pages.

When there are attached databases and no database is specified in the pragma, all databases have their secure-delete setting altered. The secure-delete setting for newly attached databases is the setting of the main database at the time the ATTACH command is evaluated.

When multiple database connections share the same cache, changing the secure-delete flag on one database connection changes it for them all.

Limitation: The secure_delete pragma only causes deleted content to be scrubbed from ordinary tables. If virtual tables store content in shadow tables, then deleting content from the virtual table does not necessarily remove forensic traces from the shadow tables. In particular, the FTS3 and FTS5 virtual tables that come bundled with SQLite might leave forensic traces in their shadow tables even if the secure_delete pragma is enabled. "#,
        ],
        [
            "short_column_names",
            r#"PRAGMA short_column_names;
PRAGMA short_column_names = boolean;

Query or change the short-column-names flag. This flag affects the way SQLite names columns of data returned by SELECT statements. See the full_column_names pragma for full details.

This pragma is deprecated and exists for backwards compatibility only. New applications should avoid using this pragma. Older applications should discontinue use of this pragma at the earliest opportunity. This pragma may be omitted from the build when SQLite is compiled using SQLITE_OMIT_DEPRECATED. "#,
        ],
        [
            "shrink_memory",
            r#"PRAGMA shrink_memory

This pragma causes the database connection on which it is invoked to free up as much memory as it can, by calling sqlite3_db_release_memory(). "#,
        ],
        [
            "soft_heap_limit",
            r#"

PRAGMA soft_heap_limit
PRAGMA soft_heap_limit=N

This pragma invokes the sqlite3_soft_heap_limit64() interface with the argument N, if N is specified and is a non-negative integer. The soft_heap_limit pragma always returns the same integer that would be returned by the sqlite3_soft_heap_limit64(-1) C-language function.

See also the hard_heap_limit pragma. "#,
        ],
        [
            "stats",
            r#"PRAGMA stats;

This pragma returns auxiliary information about tables and indices. The returned information is used during testing to help verify that the query planner is operating correctly. The format and meaning of this pragma will likely change from one release to the next. Because of its volatility, the behavior and output format of this pragma are deliberately undocumented.

The intended use of this pragma is only for testing and validation of SQLite. This pragma is subject to change without notice and is not recommended for use by application programs."#,
        ],
        [
            "synchronous",
            r#"PRAGMA schema.synchronous;
PRAGMA schema.synchronous = 0 | OFF | 1 | NORMAL | 2 | FULL | 3 | EXTRA;

Query or change the setting of the "synchronous" flag. The first (query) form will return the synchronous setting as an integer. The second form changes the synchronous setting. The meanings of the various synchronous settings are as follows:

EXTRA (3)
EXTRA synchronous is like FULL with the addition that the directory containing a rollback journal is synced after that journal is unlinked to commit a transaction in DELETE mode. EXTRA provides additional durability if the commit is followed closely by a power loss.
FULL (2)
When synchronous is FULL (2), the SQLite database engine will use the xSync method of the VFS to ensure that all content is safely written to the disk surface prior to continuing. This ensures that an operating system crash or power failure will not corrupt the database. FULL synchronous is very safe, but it is also slower. FULL is the most commonly used synchronous setting when not in WAL mode.
NORMAL (1)
When synchronous is NORMAL (1), the SQLite database engine will still sync at the most critical moments, but less often than in FULL mode. There is a very small (though non-zero) chance that a power failure at just the wrong time could corrupt the database in journal_mode=DELETE on an older filesystem. WAL mode is safe from corruption with synchronous=NORMAL, and probably DELETE mode is safe too on modern filesystems. WAL mode is always consistent with synchronous=NORMAL, but WAL mode does lose durability. A transaction committed in WAL mode with synchronous=NORMAL might roll back following a power loss or system crash. Transactions are durable across application crashes regardless of the synchronous setting or journal mode. The synchronous=NORMAL setting is a good choice for most applications running in WAL mode.
OFF (0)
With synchronous OFF (0), SQLite continues without syncing as soon as it has handed data off to the operating system. If the application running SQLite crashes, the data will be safe, but the database might become corrupted if the operating system crashes or the computer loses power before that data has been written to the disk surface. On the other hand, commits can be orders of magnitude faster with synchronous OFF. 

In WAL mode when synchronous is NORMAL (1), the WAL file is synchronized before each checkpoint and the database file is synchronized after each completed checkpoint and the WAL file header is synchronized when a WAL file begins to be reused after a checkpoint, but no sync operations occur during most transactions. With synchronous=FULL in WAL mode, an additional sync operation of the WAL file happens after each transaction commit. The extra WAL sync following each transaction helps ensure that transactions are durable across a power loss. Transactions are consistent with or without the extra syncs provided by synchronous=FULL. If durability is not a concern, then synchronous=NORMAL is normally all one needs in WAL mode.

The TEMP schema always has synchronous=OFF since the content of of TEMP is ephemeral and is not expected to survive a power outage. Attempts to change the synchronous setting for TEMP are silently ignored.

See also the fullfsync and checkpoint_fullfsync pragmas."#,
        ],
        [
            "table_info",
            r#"PRAGMA schema.table_info(table-name);

This pragma returns one row for each normal column in the named table. Columns in the result set include: "name" (its name); "type" (data type if given, else ''); "notnull" (whether or not the column can be NULL); "dflt_value" (the default value for the column); and "pk" (either zero for columns that are not part of the primary key, or the 1-based index of the column within the primary key).

The "cid" column should not be taken to mean more than "rank within the current result set".

The table named in the table_info pragma can also be a view.

This pragma does not show information about generated columns or hidden columns. Use PRAGMA table_xinfo to get a more complete list of columns that includes generated and hidden columns. "#,
        ],
        [
            "table_list",
            r#" PRAGMA table_list;
PRAGMA schema.table_list;
PRAGMA table_list(table-name);

This pragma returns information about the tables and views in the schema, one table per row of output. The table_list pragma first appeared in SQLite version 3.37.0 (2021-11-27). As of its initial release the columns returned by the table_list pragma include those listed below. Future versions of SQLite will probably add additional columns of output.

schema: the schema in which the table or view appears (for example "main" or "temp").
name: the name of the table or view.
type: the type of object - one of "table", "view", "shadow" (for shadow tables), or "virtual" for virtual tables.
ncol: the number of columns in the table, including generated columns and hidden columns.
wr: 1 if the table is a WITHOUT ROWID table or 0 if is not.
strict: 1 if the table is a STRICT table or 0 if it is not.
Additional columns will likely be added in future releases. 

The default behavior is to show all tables in all schemas. If the schema. name appears before the pragma, then only tables in that one schema are shown. If a table-name argument is supplied, then only information about that one table is returned. "#,
        ],
        [
            "table_xinfo",
            r#"PRAGMA schema.table_xinfo(table-name);

This pragma returns one row for each column in the named table, including generated columns and hidden columns. The output has the same columns as for PRAGMA table_info plus a column, "hidden", whose value signifies a normal column (0), a dynamic or stored generated column (2 or 3), or a hidden column in a virtual table (1). The rows for which this field is non-zero are those omitted for PRAGMA table_info.
"#,
        ],
        [
            "temp_store",
            r#"PRAGMA temp_store;
PRAGMA temp_store = 0 | DEFAULT | 1 | FILE | 2 | MEMORY;

Query or change the setting of the "temp_store" parameter. When temp_store is DEFAULT (0), the compile-time C preprocessor macro SQLITE_TEMP_STORE is used to determine where temporary tables and indices are stored. When temp_store is MEMORY (2) temporary tables and indices are kept as if they were in pure in-memory databases. When temp_store is FILE (1) temporary tables and indices are stored in a file. The temp_store_directory pragma can be used to specify the directory containing temporary files when FILE is specified. When the temp_store setting is changed, all existing temporary tables, indices, triggers, and views are immediately deleted.

It is possible for the library compile-time C preprocessor symbol SQLITE_TEMP_STORE to override this pragma setting. The following table summarizes the interaction of the SQLITE_TEMP_STORE preprocessor macro and the temp_store pragma:"#,
        ],
        [
            "temp_store_directory",
            r#"

PRAGMA temp_store_directory;
PRAGMA temp_store_directory = 'directory-name';

Query or change the value of the sqlite3_temp_directory global variable, which many operating-system interface backends use to determine where to store temporary tables and indices.

When the temp_store_directory setting is changed, all existing temporary tables, indices, triggers, and viewers in the database connection that issued the pragma are immediately deleted. In practice, temp_store_directory should be set immediately after the first database connection for a process is opened. If the temp_store_directory is changed for one database connection while other database connections are open in the same process, then the behavior is undefined and probably undesirable.

Changing the temp_store_directory setting is not threadsafe. Never change the temp_store_directory setting if another thread within the application is running any SQLite interface at the same time. Doing so results in undefined behavior. Changing the temp_store_directory setting writes to the sqlite3_temp_directory global variable and that global variable is not protected by a mutex.

The value directory-name should be enclosed in single quotes. To revert the directory to the default, set the directory-name to an empty string, e.g., PRAGMA temp_store_directory = ''. An error is raised if directory-name is not found or is not writable.

The default directory for temporary files depends on the OS. Some OS interfaces may choose to ignore this variable and place temporary files in some other directory different from the directory specified here. In that sense, this pragma is only advisory.

This pragma is deprecated and exists for backwards compatibility only. New applications should avoid using this pragma. Older applications should discontinue use of this pragma at the earliest opportunity. This pragma may be omitted from the build when SQLite is compiled using SQLITE_OMIT_DEPRECATED. "#,
        ],
        [
            "threads",
            r#"PRAGMA threads;
PRAGMA threads = N;

Query or change the value of the sqlite3_limit(db,SQLITE_LIMIT_WORKER_THREADS,...) limit for the current database connection. This limit sets an upper bound on the number of auxiliary threads that a prepared statement is allowed to launch to assist with a query. The default limit is 0 unless it is changed using the SQLITE_DEFAULT_WORKER_THREADS compile-time option. When the limit is zero, that means no auxiliary threads will be launched.

This pragma is a thin wrapper around the sqlite3_limit(db,SQLITE_LIMIT_WORKER_THREADS,...) interface. "#,
        ],
        [
            "trusted_schema",
            r#"

PRAGMA trusted_schema;
PRAGMA trusted_schema = boolean;

The trusted_schema setting is a per-connection boolean that determines whether or not SQL functions and virtual tables that have not been security audited are allowed to be run by views, triggers, or in expressions of the schema such as CHECK constraints, DEFAULT clauses, generated columns, expression indexes, and/or partial indexes. This setting can also be controlled using the sqlite3_db_config(db,SQLITE_DBCONFIG_TRUSTED_SCHEMA,...) C-language interface.

In order to maintain backwards compatibility, this setting is ON by default. There are advantages to turning it off, and most applications will be unaffected if it is turned off. For that reason, all applications are encouraged to switch this setting off on every database connection as soon as that connection is opened.

The -DSQLITE_TRUSTED_SCHEMA=0 compile-time option will cause this setting to default to OFF. "#,
        ],
        [
            "user_version",
            r#"PRAGMA schema.user_version;
PRAGMA schema.user_version = integer ;

The user_version pragma will get or set the value of the user-version integer at offset 60 in the database header. The user-version is an integer that is available to applications to use however they want. SQLite makes no use of the user-version itself.

See also the application_id pragma and schema_version pragma. "#,
        ],
        [
            "vdbe_addoptrace",
            r#"PRAGMA vdbe_addoptrace = boolean;

If SQLite has been compiled with the SQLITE_DEBUG compile-time option, then the vdbe_addoptrace pragma can be used to cause a complete VDBE opcodes to be displayed as they are created during code generation. This feature is used for debugging SQLite itself. See the VDBE documentation for more information.

This pragma is intended for use when debugging SQLite itself. It is only available when the SQLITE_DEBUG compile-time option is used."#,
        ],
        [
            "vdbe_debug",
            r#"PRAGMA vdbe_debug = boolean;

If SQLite has been compiled with the SQLITE_DEBUG compile-time option, then the vdbe_debug pragma is a shorthand for three other debug-only pragmas: vdbe_addoptrace, vdbe_listing, and vdbe_trace. This feature is used for debugging SQLite itself. See the VDBE documentation for more information.

This pragma is intended for use when debugging SQLite itself. It is only available when the SQLITE_DEBUG compile-time option is used."#,
        ],
        [
            "vdbe_listing",
            r#"PRAGMA vdbe_listing = boolean;

If SQLite has been compiled with the SQLITE_DEBUG compile-time option, then the vdbe_listing pragma can be used to cause a complete listing of the virtual machine opcodes to appear on standard output as each statement is evaluated. With listing is on, the entire content of a program is printed just prior to beginning execution. The statement executes normally after the listing is printed. This feature is used for debugging SQLite itself. See the VDBE documentation for more information.

This pragma is intended for use when debugging SQLite itself. It is only available when the SQLITE_DEBUG compile-time option is used."#,
        ],
        [
            "vdbe_trace",
            r#"PRAGMA vdbe_trace = boolean;

If SQLite has been compiled with the SQLITE_DEBUG compile-time option, then the vdbe_trace pragma can be used to cause virtual machine opcodes to be printed on standard output as they are evaluated. This feature is used for debugging SQLite. See the VDBE documentation for more information.

This pragma is intended for use when debugging SQLite itself. It is only available when the SQLITE_DEBUG compile-time option is used."#,
        ],
        [
            "wal_autocheckpoint",
            r#"PRAGMA wal_autocheckpoint;
PRAGMA wal_autocheckpoint=N;

This pragma queries or sets the write-ahead log auto-checkpoint interval. When the write-ahead log is enabled (via the journal_mode pragma) a checkpoint will be run automatically whenever the write-ahead log equals or exceeds N pages in length. Setting the auto-checkpoint size to zero or a negative value turns auto-checkpointing off.

This pragma is a wrapper around the sqlite3_wal_autocheckpoint() C interface. All automatic checkpoints are PASSIVE.

Autocheckpointing is enabled by default with an interval of 1000 or SQLITE_DEFAULT_WAL_AUTOCHECKPOINT."#,
        ],
        [
            "wal_checkpoint",
            r#"PRAGMA schema.wal_checkpoint;
PRAGMA schema.wal_checkpoint(PASSIVE);
PRAGMA schema.wal_checkpoint(FULL);
PRAGMA schema.wal_checkpoint(RESTART);
PRAGMA schema.wal_checkpoint(TRUNCATE);

If the write-ahead log is enabled (via the journal_mode pragma), this pragma causes a checkpoint operation to run on database database, or on all attached databases if database is omitted. If write-ahead log mode is disabled, this pragma is a harmless no-op.

Invoking this pragma without an argument is equivalent to calling the sqlite3_wal_checkpoint() C interface.
Invoking this pragma with an argument is equivalent to calling the sqlite3_wal_checkpoint_v2() C interface with a 3rd parameter corresponding to the argument:

PASSIVE
Checkpoint as many frames as possible without waiting for any database readers or writers to finish. Sync the db file if all frames in the log are checkpointed. This mode is the same as calling the sqlite3_wal_checkpoint() C interface. The busy-handler callback is never invoked in this mode. 
FULL
This mode blocks (invokes the busy-handler callback) until there is no database writer and all readers are reading from the most recent database snapshot. It then checkpoints all frames in the log file and syncs the database file. FULL blocks concurrent writers while it is running, but readers can proceed. 
RESTART
This mode works the same way as FULL with the addition that after checkpointing the log file it blocks (calls the busy-handler callback) until all readers are finished with the log file. This ensures that the next client to write to the database file restarts the log file from the beginning. RESTART blocks concurrent writers while it is running, but allowed readers to proceed. 
TRUNCATE
This mode works the same way as RESTART with the addition that the WAL file is truncated to zero bytes upon successful completion. 

The wal_checkpoint pragma returns a single row with three integer columns. The first column is usually 0 but will be 1 if a RESTART or FULL or TRUNCATE checkpoint was blocked from completing, for example because another thread or process was actively using the database. In other words, the first column is 0 if the equivalent call to sqlite3_wal_checkpoint_v2() would have returned SQLITE_OK or 1 if the equivalent call would have returned SQLITE_BUSY. The second column is the number of modified pages that have been written to the write-ahead log file. The third column is the number of pages in the write-ahead log file that have been successfully moved back into the database file at the conclusion of the checkpoint. The second and third column are -1 if there is no write-ahead log, for example if this pragma is invoked on a database connection that is not in WAL mode."#,
        ],
        [
            "writable_schema",
            r#"PRAGMA writable_schema = boolean;
PRAGMA writable_schema = RESET

When this pragma is on, and the SQLITE_DBCONFIG_DEFENSIVE flag is off, then the sqlite_schema table can be changed using ordinary UPDATE, INSERT, and DELETE statements. If the argument is "RESET" then schema writing is disabled (as with "PRAGMA writable_schema=OFF") and, in addition, the schema is reloaded. Warning: misuse of this pragma can easily result in a corrupt database file. "#,
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
mod tests {
    use std::path::Path;

    use crate::db::QuerySql;
    use crate::enable_test_log;
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::sqlite::make_test_object;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_auto_completion() {
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        let _ = db
            .query_sql(
                query_id,
                r#"
            -- 测试用到的表格 和建表语句
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

            -- create view for test
            create view employees_view as select * from employees;
        "#,
            )
            .await;

        let _ = db
            .query_sql(
                query_id,
                r#"
                create index employees_name on employees(name)
        "#,
            )
            .await;

        enable_test_log();
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("sqlite_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();

        test_file_auto_completion(&content, &db, false).await;
    }

    #[ignore = "xxx"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = make_test_object().await;
        test_file_auto_completion(
            r#"SELECT e.id AS EmployeeID,  e.name AS EmployeeName,  d.name AS DepartmentName FROM employees AS e JOIN departments AS d ON e.department_id = d.id;
9 e"#,
            &db,
            false,
        )
        .await;
    }
}

pub(crate) const SQLITE_FN_AND_DESC: &'static [[&'static str; 2]] = &[
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

pub(crate) const SQLITE_TYPES: &[&str] = &[
    "NULL",
    "INTEGER",
    "REAL",
    "TEXT",
    "BLOB",
    "INT",
    "TINYINT",
    "SMALLINT",
    "MEDIUMINT",
    "BIGINT",
    "UNSIGNED BIG INT",
    "INT2",
    "INT8",
    "CHARACTER",
    "VARCHAR",
    "VARYING CHARACTER",
    "NCHAR",
    "NATIVE CHARACTER",
    "NVARCHAR",
    "CLOB",
    "DOUBLE",
    "DOUBLE PRECISION",
    "FLOAT",
    "NUMERIC",
    "DECIMAL",
    "BOOLEAN",
    "DATE",
    "DATETIME",
];

pub(crate) const SQLITE_KEY_WORDS: &[&str] = &[
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
