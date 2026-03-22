use crate::db::{CatalogTrait, SequenceDetail};
use crate::duckdb::{DuckDB, DuckDbConn};
use crate::language_server::{
    try_parser_dot_lexer, AstCompletionHelper, CompletionItem, CompletionKind, CompletionList,
    GetKeyWords, LangServerCommon, LanguageServer, LexerCompletionTrait,
};
use crate::sqlparser_completion::TempTableColumn;
use duckdb::types::Value;
use std::collections::HashSet;
use tokio::task::spawn_blocking;

#[async_trait::async_trait]
impl LanguageServer for DuckDB {
    async fn try_get_max_id(&self, name: &str) -> anyhow::Result<String> {
        let sql = format!(r#"select max(id) from   "{name}" "#,);
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let r = conn.query_row(&sql, [], |row| {
                let r: i128 = row.get(0)?;
                Ok(r)
            })?;
            anyhow::Ok(r.to_string())
        })
        .await?
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

    async fn ai_text_completion(
        &self,
        sql: &str,
        line: u32,
        col: u32,
    ) -> anyhow::Result<CompletionList> {
        LexerCompletionTrait::ai_text_completion(self, sql, line, col).await
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        LangServerCommon::is_dangerous_sql_by_parser(self, sql)
    }

    fn extract_table_select_selection(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_selection(self, sql)
    }

    fn extract_table_select_sort(&self, sql: &str) -> anyhow::Result<Option<String>> {
        LangServerCommon::extract_table_select_sort(self, sql)
    }
}

#[async_trait::async_trait]

impl LangServerCommon for DuckDB {
    async fn list_sequence(&self, schema: Option<&str>) -> anyhow::Result<Vec<SequenceDetail>> {
        let db = self.get_database().await?;
        let schema = self.get_schema(schema).await?;
        self.list_sequence_impl(&db, &schema).await
    }

    async fn duckdb_extensions(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let sql = "SELECT extension_name, installed, description,aliases FROM duckdb_extensions()";
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query([])?;
            let mut ret = Vec::new();
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                let desc: Option<String> = row.get(2)?;
                let alias: duckdb::types::Value = row.get(3)?;
                let mut name_set: HashSet<_> = match alias {
                    Value::List(values) => {
                        let mut set = HashSet::new();
                        for value in values {
                            match value {
                                Value::Text(x) => {
                                    set.insert(x);
                                }
                                _ => {
                                    log::error!(
                                        "duckdb extension alias should be a string but:{:?}",
                                        value
                                    );
                                }
                            }
                        }
                        set
                    }
                    _ => Default::default(),
                };
                name_set.insert(name);
                for name in name_set {
                    ret.push(CompletionItem {
                        item: name.into(),
                        kind: CompletionKind::Extension,
                        desc: desc.clone().map(|x| x.into()),
                    });
                }
            }
            anyhow::Ok(ret)
        })
        .await?
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let db = self.get_database().await?;
        DuckDB::list_schema_impl(self, &db).await
    }
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        CatalogTrait::list_database(self).await
    }
    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let db = self.get_database().await?;
        let schema = self.get_schema(schema).await?;
        let ret = self.list_table_impl(&db, schema.as_str()).await?;
        anyhow::Ok(ret.into_iter().map(|x| x.name).collect())
    }

    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = self.get_schema(schema).await?;
        let db = self.get_database().await?;
        let ret = self.list_view_impl(&db, &schema).await?;
        anyhow::Ok(ret.into_iter().map(|x| x.name).collect())
    }

    async fn list_column(
        &self,
        schema: Option<&str>,
        name: &str,
        is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let this = Clone::clone(self);
        let schema = this.get_schema(schema).await?;
        let sql = if let Some(is_call) = is_call {
            format!(
                r#"select * from "{}"."{}"({}) limit 1"#,
                schema,
                name,
                is_call.join(",")
            )
        } else {
            format!(r#"select * from "{}"."{}" limit 1"#, schema, name)
        };
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            stmt.execute([])?;
            let fs = stmt
                .column_names()
                .into_iter()
                .enumerate()
                .map(|(index, name)| TempTableColumn {
                    name,
                    data_type: Some(stmt.column_type(index).to_string()),
                })
                .collect::<Vec<_>>();
            anyhow::Ok(fs)
        })
        .await?
    }

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = self.get_schema(schema).await?;
        let db = self.get_database().await?;

        let sql = format!(
            r#"
            select index_name  from duckdb_indexes() 
            where database_name='{}' and schema_name='{}'"#,
            db, schema
        );
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            let mut stream = stmt.query([])?;
            let mut ret = vec![];
            while let Some(row) = stream.next()? {
                let name: String = row.get(0)?;
                ret.push(name);
            }

            anyhow::Ok(ret)
        })
        .await?
    }

    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::DuckDbDialect {})
    }

    async fn list_function(
        &self,
        _schema: Option<&str>,
        _include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let this = Clone::clone(self);
        let sql = r#"SELECT function_name ,description  FROM duckdb_functions()"#;
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query([])?;
            let mut ret = Vec::new();
            let mut names = HashSet::new();
            while let Some(row) = rows.next()? {
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
        })
        .await?
    }
}

#[async_trait::async_trait]

impl AstCompletionHelper for DuckDB {
    async fn get_query_columns(
        &self,
        sql: &str,
    ) -> anyhow::Result<Vec<crate::sqlparser_completion::TempTableColumn>> {
        let sql = format!("select * from ({sql})  limit 1");

        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(&sql)?;
            stmt.execute([])?;
            let mut ret = vec![];
            for index in 0..stmt.column_count() {
                ret.push(TempTableColumn {
                    name: stmt.column_name(index)?.to_string(),
                    data_type: format!("{:?}", stmt.column_type(index)).into(),
                });
            }
            anyhow::Ok(ret)
        })
        .await?
    }
}

impl DuckDB {
    async fn get_schema(&self, schema: Option<&str>) -> anyhow::Result<String> {
        match schema {
            Some(x) => anyhow::Ok(x.to_string()),
            None => {
                let this = Clone::clone(self);
                spawn_blocking(move || {
                    let conn = this.get_query_conn()?;
                    let conn = conn.lock().expect("lock failed");
                    let r = conn.query_row("SELECT current_schema()", [], |row| {
                        let r: Option<String> = row.get(0)?;
                        Ok(r)
                    })?;
                    match r {
                        Some(x) => anyhow::Ok(x),
                        None => match &this.config.schema {
                            Some(x) => anyhow::Ok(x.to_owned()),
                            None => anyhow::Ok("main".to_string()),
                        },
                    }
                })
                .await?
            }
        }
    }

    // get `use xxxx`
    async fn get_database(&self) -> anyhow::Result<String> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.get_query_conn()?;
            let conn = conn.lock().expect("lock failed");
            let r = conn.query_row("SELECT current_database()", [], |row| {
                let r: String = row.get(0)?;
                Ok(r)
            })?;
            anyhow::Ok(r)
        })
        .await?
    }

    fn get_query_conn(&self) -> anyhow::Result<DuckDbConn> {
        match self.query_connection.get(&self.completion_id.get()) {
            Some(x) => anyhow::Ok(x.clone()),
            None => anyhow::bail!(format!("query id '{}' not found", self.completion_id.get())),
        }
    }
}

#[async_trait::async_trait]

impl GetKeyWords for DuckDB {
    // some backend can use a query to query this.
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let sql = "select keyword_name from duckdb_keywords() ";
            let conn = this.get_conn()?;
            let mut stmt = conn.prepare(sql)?;
            let mut rows = stmt.query([])?;
            let mut ret = Vec::new();
            while let Some(row) = rows.next()? {
                let name: String = row.get(0)?;
                let name = name.to_uppercase();
                ret.push(name);
            }
            anyhow::Ok(ret)
        })
        .await?
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for DuckDB {
    async fn get_types(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(DUCKDB_TYPES.iter().map(|x| x.to_string()).collect())
    }

    async fn get_text(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
}

pub(crate) const DUCKDB_TYPES: &[&str] = &[
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
    use crate::db::{QuerySql, SchemaTrait};
    use crate::duckdb::{make_test_object, test_lock};
    use crate::enable_test_log;
    use crate::language_server::completion_test::test_file_auto_completion;
    use crate::language_server::LanguageServer;
    use std::path::Path;

    // partition test failed , sqlparser can't parse a valid partition statement.
    #[tokio::test]
    async fn test_auto_completion() {
        let _gurad = test_lock().lock().await;

        let db = make_test_object(Some("main".to_string())).await;

        enable_test_log();
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("employees", false).await;
        db.execute_sql(
            query_id,
            r#"
                    CREATE TABLE employees (
                        id INTEGER PRIMARY KEY,
                        first_name TEXT NOT NULL,        
                        last_name TEXT NOT NULL,         
                        job_title TEXT NOT NULL,         
                        hire_date DATE NOT NULL,          
                        salary DECIMAL(10, 2) NOT NULL  
                    )#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
            INSERT INTO employees (id, first_name, last_name, job_title, hire_date, salary)
                VALUES (1, 'San', 'Zhang', 'Software Engineer', '2023-08-15', 15000.00);
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let _ = db.drop_sequence("seq_completion", false).await;

        db.execute_sql(query_id, r#"CREATE SEQUENCE  seq_completion"#)
            .await
            .unwrap()
            .unwrap();
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("duckdb_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        test_file_auto_completion(&content, &db, None, false).await;
    }

    #[ignore = "run manually"]
    #[tokio::test]

    async fn test_try_get_max_id() {
        let _gurad = test_lock().lock().await;
        let db = make_test_object(Some("main".to_string())).await;
        let max_id = db.try_get_max_id("employees").await.unwrap();
        println!("max id is:{}", max_id);
    }
}
