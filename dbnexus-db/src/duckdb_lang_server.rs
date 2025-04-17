use crate::db::CatalogTrait;
use crate::duckdb::{DuckDB, use_database};
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, LangServerCommon, LanguageServer,
    LexerCompletionTrait, try_completion_on_syntax,
};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::task::block_in_place;
#[async_trait::async_trait]
impl LanguageServer for DuckDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            &sql,
            line,
            col,
            sqlparser::dialect::DuckDbDialect {},
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
            &sqlparser::dialect::DuckDbDialect {},
            sqlparser::tokenizer::Location {
                line: line as u64,
                column: col as u64,
            },
        )
        .await;
    }
}

#[async_trait::async_trait]

impl AstCompletionHelper for DuckDB {
    async fn duckdb_extensions(
        &self, installed: Option<bool>,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let mut sql =
            "SELECT extension_name, installed, description FROM duckdb_extensions() ".to_string();
        if let Some(installed) = installed {
            sql.push_str(format!("where installed={}", installed).as_str());
        }

        let conn = self.get_conn()?;
        let mut stmt = conn.prepare(&sql)?;
        let mut rows = block_in_place(|| stmt.query([]))?;
        let mut ret = Vec::new();
        while let Some(row) = block_in_place(|| rows.next())? {
            let name: String = row.get(0)?;
            let desc: Option<String> = row.get(2)?;
            ret.push(CompletionItem {
                item: name.into(),
                kind: CompletionKind::Extension,
                desc: desc.map(|x| x.into()),
            });
        }

        anyhow::Ok(ret)
    }

    async fn list_table(
        &self, db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let db = Self::not_allow_empty_db(self.db_merge_config(db))?;
        let schema = Self::not_allow_empty_db(self.schema_merge_config(schema))?;
        let ret = self.list_table_impl(db, schema)?;
        anyhow::Ok(ret.into_iter().map(|x| x.name).collect())
    }

    async fn list_view(
        &self, db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let db = Self::not_allow_empty_db(self.db_merge_config(db))?;
        let schema = Self::not_allow_empty_db(self.schema_merge_config(schema))?;
        let ret = self.list_view_impl(db, schema)?;
        anyhow::Ok(ret.into_iter().map(|x| x.name).collect())
    }
    async fn list_column(
        &self, db: Option<&str>, schema: Option<&str>, name: &str,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let db = Self::not_allow_empty_db(self.db_merge_config(db))?;
        let schema = Self::not_allow_empty_db(self.schema_merge_config(schema))?;
        let sql = format!(r#"select * from "{}"."{}" limit 1"#, schema, name);
        let conn = self.get_conn()?;
        use_database(&conn, db)?;
        let mut stmt = block_in_place(|| conn.prepare(&sql))?;
        block_in_place(|| stmt.execute([]))?;
        let fs = stmt
            .column_names()
            .into_iter()
            .enumerate()
            .map(|(index, name)| CompletionItem {
                item: name.into(),
                desc: Some(stmt.column_type(index).to_string().into()),
                kind: CompletionKind::Column,
            })
            .collect::<Vec<_>>();
        anyhow::Ok(fs)
    }
    async fn list_function(
        &self, _db: Option<&str>, _schema: Option<&str>, _include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let sql = r#"SELECT function_name ,description  FROM duckdb_functions()"#;

        let conn = self.get_conn()?;
        let mut stmt = block_in_place(|| conn.prepare(sql))?;
        let mut rows = block_in_place(|| stmt.query([]))?;
        let mut ret = Vec::new();
        let mut names = HashSet::new();

        while let Some(row) = block_in_place(|| rows.next())? {
            let name: String = row.get(0)?;
            if names.contains(&name) {
                continue;
            }
            let desc: Option<String> = row.get(1)?;
            ret.push(CompletionItem {
                item: name.clone().into(),
                kind: CompletionKind::Function,
                desc: desc.map(|x| x.into()),
            });
            names.insert(name);
        }

        anyhow::Ok(ret)
    }

    async fn list_index(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
}

impl DuckDB {
    fn db_merge_config<'a, 'b>(&'a self, db: Option<&'b str>) -> Option<&'a str>
    where
        'b: 'a,
    {
        if db.is_some() {
            return db;
        }
        return self.config.db.as_ref().map(|x| x.as_str());
    }

    fn schema_merge_config<'a, 'b>(&'a self, schema: Option<&'b str>) -> Option<&'a str>
    where
        'b: 'a,
    {
        if schema.is_some() {
            return schema;
        }
        return self.config.schema.as_deref();
    }

    fn not_allow_empty_db<'a>(db: Option<&'a str>) -> anyhow::Result<&'a str> {
        match db {
            Some(x) => return anyhow::Ok(x),
            None => anyhow::bail!("empty database"),
        }
    }
    fn not_allow_empty_schema<'a>(db: Option<&'a str>) -> anyhow::Result<&'a str> {
        match db {
            Some(x) => return anyhow::Ok(x),
            None => anyhow::bail!("empty schema"),
        }
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for DuckDB {
    type Dialect = sqlparser::dialect::DuckDbDialect;
    const KEY_WORDS: &'static [&'static str] = KEY_WORDS;
    const TYPES: &'static [&'static str] = TYPES;
    const TEXTS: &'static [&'static str] = &["true", "false"];
    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self, None, None, true).await
    }

    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        let db = Self::not_allow_empty_db(self.db_merge_config(None)).unwrap_or_default();
        let schema =
            Self::not_allow_empty_schema(self.schema_merge_config(None)).unwrap_or_default();
        let mut ret: Vec<String> = vec![];
        if !db.is_empty() && !schema.is_empty() {
            ret.extend(
                self.list_table_impl(db, schema)?
                    .into_iter()
                    .map(|x| x.name),
            );
            ret.extend(self.list_view_impl(db, schema)?.into_iter().map(|x| x.name));
        }
        anyhow::Ok(ret)
    }

    // some backend can use a query to query this.
    async fn get_key_words_from_query(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let sql = "select  keyword_name from duckdb_keywords() ";
        let conn = self.get_conn()?;
        let mut stmt = block_in_place(|| conn.prepare(sql))?;
        let mut rows = block_in_place(|| stmt.query([]))?;
        let mut ret = Vec::new();
        while let Some(row) = block_in_place(|| rows.next())? {
            let name: String = row.get(0)?;
            let name = name.to_uppercase();
            ret.push(CompletionItem {
                item: name.into(),
                kind: CompletionKind::KeyWord,
                desc: None,
            });
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl LangServerCommon for DuckDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        CatalogTrait::list_database(self).await
    }
    async fn list_schema(&self, db: Option<&str>) -> anyhow::Result<Vec<String>> {
        let db = self.db_merge_config(db);
        if let Some(db) = db {
            self.list_schema_impl(db)
        } else {
            anyhow::Ok(vec![])
        }
    }
}

// can use a query to query all keywords
const KEY_WORDS: &'static [&'static str] = &[];

const TYPES: &'static [&'static str] = &[
    "BIGINT",
    "INT8",
    "LONG",
    "BIT",
    "BITSTRING",
    "BLOB",
    "BYTEA",
    "BINARY",
    "VARBINARY",
    "BOOLEAN",
    "BOOL",
    "LOGICAL",
    "DATE",
    "DECIMAL",
    "NUMERIC",
    "DOUBLE",
    "FLOAT8",
    "FLOAT",
    "FLOAT4",
    "REAL",
    "HUGEINT",
    "INTEGER",
    "INT4",
    "INT",
    "SIGNED",
    "INTERVAL",
    "JSON",
    "SMALLINT",
    "INT2",
    "SHORT",
    "TIME",
    "TIMESTAMP WITH TIME ZONE",
    "TIMESTAMPTZ",
    "TIMESTAMP",
    "DATETIME",
    "TINYINT",
    "INT1",
    "UBIGINT",
    "UHUGEINT",
    "UINTEGER",
    "USMALLINT",
    "UTINYINT",
    "UUID",
    "VARCHAR",
    "MAP",
    "STRUCT",
    "UNION",
];

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{
        db::{QuerySql, monaco_editor_keyword_regexp},
        duckdb::make_test_object,
        enable_test_log,
        language_server::{LexerCompletionTrait, completion_test::test_file_auto_completion},
    };

    // partition test failed , sqlparser can't parse a valid partition statement.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_auto_completion() {
        let db = make_test_object(Some("main".to_string()));
        enable_test_log();
        let query_id = db.open_connection().await.unwrap();
        let _err = db
            .execute_sql(
                query_id,
                r#"
                    CREATE TABLE employees (
                        employee_id INTEGER PRIMARY KEY,  
                        first_name TEXT NOT NULL,        
                        last_name TEXT NOT NULL,         
                        job_title TEXT NOT NULL,         
                        hire_date DATE NOT NULL,          
                        salary DECIMAL(10, 2) NOT NULL  
                    );
                r#"
                 "#,
            )
            .await
            .unwrap();
        if let Err(err) = _err {
            println!("create employees error failed:{:?}", err)
        }
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("duckdb_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        test_file_auto_completion(&content, &db, false).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn key_words() {
        use std::borrow::Borrow;
        let db = make_test_object(None);

        let fs = LexerCompletionTrait::get_key_words_from_query(&db)
            .await
            .unwrap();

        let fs: Vec<&str> = fs.iter().map(|x| x.item.borrow()).collect();

        let r = monaco_editor_keyword_regexp(&fs[..]);
        println!("{}", r);
    }
}
