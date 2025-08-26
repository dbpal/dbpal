use std::vec;

use crate::clickhouse::{ClickHouseDB, JsonRespMeta};
use crate::db::DBTrait;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, ErrAndLoc, LangServerCommon,
    LanguageServer, LexerCompletionTrait, try_parser_and_lexer,
};
use crate::sqlparser_completion::TempTableColumn;

#[async_trait::async_trait]

impl LanguageServer for ClickHouseDB {
    async fn auto_completion(
        &self, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        try_parser_and_lexer(self, sql, line, col, use_lexer).await
    }
    fn parse_error(&self, sql: &str) -> anyhow::Result<Option<ErrAndLoc>> {
        LangServerCommon::parse_error(self, sql)
    }
}

impl ClickHouseDB {
    fn schema_merge_config<'a, 'b>(&'a self, schema: Option<&'b str>) -> Option<&'a str>
    where
        'b: 'a,
    {
        if schema.is_some() {
            return schema;
        }
        self.config.db.as_deref()
    }

    fn not_allow_none_schema(schema: Option<&str>) -> anyhow::Result<&str> {
        match schema {
            Some(x) => anyhow::Ok(x),
            None => anyhow::bail!("empty schema"),
        }
    }
}
#[async_trait::async_trait]

impl LangServerCommon for ClickHouseDB {
    async fn list_column(
        &self, schema: Option<&str>, name: &str, _is_call: bool,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = format!(
            r#"select * from {}"{}""#,
            match schema {
                Some(x) => format!("\"{}\".", x),
                None => "".to_string(),
            },
            name
        );
        self.get_query_columns(&sql).await
    }
    async fn get_key_words_fn(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let mut lock = self.key_word_completion.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }

        let sql = "select * from system.keywords";
        let conn = self.get_conn()?;
        let ret: Vec<String> = conn.query(sql).fetch_all().await?;
        lock.extend(ret.into_iter().map(|x| CompletionItem {
            item: x.into(),
            kind: CompletionKind::KeyWord,
            desc: None,
        }));
        return anyhow::Ok(lock.clone());
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        DBTrait::list_schema(self).await
    }

    async fn list_function(
        &self, _schema: Option<&str>, _include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        // 在 ClickHouse 中，使用以下查询可以查看所有函数，包括表函数（table functions）：
        let sql = "select name , description from system.functions";
        let conn = self.get_conn()?;
        let result: Vec<(String, String)> = conn.query(sql).fetch_all().await?;

        anyhow::Ok(
            result
                .into_iter()
                .map(|(name, desc)| CompletionItem {
                    item: name.into(),
                    kind: CompletionKind::Function,
                    desc: Some(desc.into()),
                })
                .collect::<Vec<_>>(),
        )
    }

    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::ClickHouseDialect {})
    }
    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let db = Self::not_allow_none_schema(self.schema_merge_config(schema))?;
        let v = self.list_table_impl(db).await?;
        anyhow::Ok(v.into_iter().map(|x| x.name).collect())
    }

    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let db = Self::not_allow_none_schema(self.schema_merge_config(schema))?;
        let mut ret = Vec::new();
        ret.extend(self.list_view_impl(db).await?.into_iter().map(|x| x.name));
        ret.extend(
            self.list_materialized_view_impl(db)
                .await?
                .into_iter()
                .map(|x| x.name),
        );
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for ClickHouseDB {
    async fn list_cluster(&self) -> anyhow::Result<Vec<String>> {
        let sql = r#"
         SELECT
            cluster 
        FROM
            system.clusters

        "#;
        let conn = self.get_conn()?;
        let ret: Vec<String> = conn.query(sql).fetch_all().await?;
        anyhow::Ok(ret)
    }
    async fn list_clickhouse_table_projects(
        &self, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let conn = self.get_conn()?;
        let db = Self::not_allow_none_schema(self.schema_merge_config(schema))?;
        let sql = format!(
            r#"
            SELECT name 
                FROM system.projections 
                WHERE "table"  = '{table}' and database = '{db}'
        "#
        );
        let result: Vec<String> = conn.query(&sql).fetch_all().await?;
        anyhow::Ok(result)
    }

    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = format!("select * from({}) limit 1", sql);
        let meta = self.query_as_json(&sql).await?.meta;
        anyhow::Ok(
            meta.into_iter()
                .map(|x| {
                    let JsonRespMeta { name, r#type } = x;
                    TempTableColumn {
                        name,
                        typ: Some(r#type),
                    }
                })
                .collect(),
        )
    }

    async fn list_index(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec![])
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for ClickHouseDB {
    const TEXTS: &'static [&'static str] = &["true", "false"];
    const TYPES: &'static [&'static str] = CLICKHOUSE_TYPES;
}

const CLICKHOUSE_TYPES: &[&str] = &[
    "UInt8",
    "UInt16",
    "UInt32",
    "UInt64",
    "UInt128",
    "UInt256",
    "Int8",
    "Int16",
    "Int32",
    "Int64",
    "Int128",
    "Int256",
    "Float32",
    "Float64",
    "Decimal",
    "String",
    "FixedString",
    "UUID",
    "IPv6",
    "IPv4",
    "Date",
    "Date32",
    "DateTime",
    "DateTime64",
    "Tuple",
    "Map",
    "LowCardinality",
    "Nullable",
    "JSON",
];

#[cfg(test)]

mod tests {
    use std::collections::HashSet;
    use std::path::Path;

    use crate::clickhouse::{ClickHouseDB, make_test_object};
    use crate::db::{QuerySql, monaco_editor_keyword_regexp};
    use crate::enable_test_log;
    use crate::language_server::LangServerCommon;
    use crate::language_server::completion_test::test_file_auto_completion;

    // partition test failed , sqlparser can't parse a valid partition statement.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_auto_completion() {
        let db = make_test_object();
        // enable_test_log();
        let query_id = db.open_connection().await.unwrap();
        let _ = db
            .query_sql(
                query_id,
                r#"
                 CREATE TABLE employees (
                    employee_id UInt32,
                    first_name String,
                    last_name String,
                    position String,
                    department String,
                    hire_date Date,
                    salary Float64
                ) ENGINE = MergeTree()
                ORDER BY employee_id;
         "#,
            )
            .await
            .unwrap();

        let _ = db
            .query_sql(
                query_id,
                r#"
                 ALTER TABLE employees 
                    ADD PROJECTION employee_salary_projection 
                    (
                        SELECT 
                            department, 
                            AVG(salary) AS avg_salary, 
                            COUNT(*) AS employee_count 
                        GROUP BY 
                            department
                    );
         "#,
            )
            .await
            .unwrap();

        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir).to_path_buf();
        file_path.push("tests");
        file_path.push("clickhouse_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();
        test_file_auto_completion(&content, &db, false).await;
    }
    #[ignore = "手动跑跑的测试"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = make_test_object();
        test_file_auto_completion(
            r#"ALTER TABLE employees ADD PROJECTION employee_salary_projection(SELECT department, AVG(salary) AS avg_salary, COUNT(*) AS employee_count GROUP BY department);
82 department "#,
            &db,
            false,
        )
        .await;
    }

    // clickhouse is a little different keyword can select from database
    // but frontend still need this definition.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn print_key_words() {
        let db = make_test_object();

        let keywords = <ClickHouseDB as LangServerCommon>::get_key_words_fn(&db)
            .await
            .unwrap()
            .into_iter()
            .map(|x| x.item.to_string())
            .collect::<Vec<_>>();
        // `ADMIN OPTION FOR` clickhouse can have this kind of result.

        let keywords = keywords
            .into_iter()
            .map(|x| x.split(" ").map(|x| x.to_uppercase()).collect::<Vec<_>>())
            .flat_map(|x| x.into_iter())
            .collect::<HashSet<_>>();

        let keywords = keywords.into_iter().collect::<Vec<_>>();

        let keywords: Vec<&str> = keywords.iter().map(|x| x.as_str()).collect();

        let s = monaco_editor_keyword_regexp(&keywords[..]);
        println!("{}", s);
    }
}
