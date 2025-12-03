use std::vec;

use crate::clickhouse::{ClickHouseDB, JsonRespMeta};
use crate::db::DBTrait;
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, CompletionList, ErrAndLoc, GetKeyWords,
    LangServerCommon, LanguageServer, LexerCompletionTrait, try_parser_dot_lexer,
};
use crate::sqlparser_completion::TempTableColumn;

#[async_trait::async_trait]
impl GetKeyWords for ClickHouseDB {
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        let mut lock = self.key_words_cache.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        let sql = "select * from system.keywords";
        let conn = self.get_conn()?;
        let ret: Vec<String> = conn.query(sql).fetch_all().await?;
        *lock = ret;
        return anyhow::Ok(lock.clone());
    }
}

#[async_trait::async_trait]

impl LanguageServer for ClickHouseDB {
    async fn try_get_max_id(&self, name: &str) -> anyhow::Result<String> {
        let sql = format!("select max(id) from `{name}`");
        let conn = self.get_conn()?;
        let r: i64 = conn.query(&sql).fetch_one().await?;
        anyhow::Ok(r.to_string())
    }

    async fn ai_text_completion(
        &self, text: &str, line: u32, col: u32,
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
        &self, _query_id: Option<String>, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<CompletionList> {
        // self.completion_id.set_optional(query_id);
        try_parser_dot_lexer(self, sql, line, col, use_lexer).await
    }

    async fn check_sql(&self, sql: &str) -> anyhow::Result<Option<ErrAndLoc>> {
        LangServerCommon::parse_err(self, sql)
    }
    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        LangServerCommon::is_dangerous_sql_by_parser(self, sql)
    }
}

impl ClickHouseDB {
    fn schema_merge_config(&self, schema: Option<&str>) -> Option<String> {
        if schema.is_some() {
            return schema.map(|x| x.to_string());
        }
        self.config.db.clone()
    }

    fn not_allow_none_schema(&self, schema: Option<&str>) -> anyhow::Result<String> {
        match self.schema_merge_config(schema) {
            Some(x) => anyhow::Ok(x),
            None => anyhow::bail!("empty schema"),
        }
    }
}
#[async_trait::async_trait]

impl LangServerCommon for ClickHouseDB {
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
    fn clickhouse_format_clause(&self) -> Vec<String> {
        CLICKHOUSE_FORMATS.iter().map(|x| x.to_string()).collect()
    }
    async fn list_index(&self, _schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        // clickhouse 不支持索引
        anyhow::Ok(vec![])
    }
    async fn list_column(
        &self, schema: Option<&str>, name: &str, is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = if let Some(is_call) = is_call {
            format!(
                r#"select {}{}({})"#,
                match schema {
                    Some(x) => format!("\"{}\".", x),
                    None => "".to_string(),
                },
                name,
                is_call.join(",")
            )
        } else {
            format!(
                r#"select * from {}"{}""#,
                match schema {
                    Some(x) => format!("\"{}\".", x),
                    None => "".to_string(),
                },
                name
            )
        };
        self.get_query_columns(&sql).await
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        DBTrait::list_schema(self).await
    }

    async fn list_function(
        &self, _schema: Option<&str>, _include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        // 在 ClickHouse 中，使用以下查询可以查看所有函数，包括表函数（table functions）：
        // 看起来clickhouse 都有全局的作用域
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
        let db = self.not_allow_none_schema(schema)?;
        let v = self.list_table_impl(&db).await?;
        anyhow::Ok(v.into_iter().map(|x| x.name).collect())
    }

    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let db = self.not_allow_none_schema(schema)?;
        let mut ret = Vec::new();
        ret.extend(self.list_view_impl(&db).await?.into_iter().map(|x| x.name));
        ret.extend(
            self.list_materialized_view_impl(&db)
                .await?
                .into_iter()
                .map(|x| x.name),
        );
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for ClickHouseDB {
    async fn list_clickhouse_table_projects(
        &self, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let conn = self.get_conn()?;
        let db = self.not_allow_none_schema(schema)?;
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
                        data_type: Some(r#type),
                    }
                })
                .collect(),
        )
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for ClickHouseDB {
    async fn get_types(&self) -> anyhow::Result<Vec<String>> {
        let r = ["true", "false"].iter().map(|x| x.to_string()).collect();
        anyhow::Ok(r)
    }

    async fn get_text(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(CLICKHOUSE_TYPES.iter().map(|x| x.to_string()).collect())
    }
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

// clickhouse_formats.rs
pub const CLICKHOUSE_FORMATS: &[&str] = &[
    "Arrow",
    "ArrowStream",
    "Avro",
    "CapnProto",
    "CSV",
    "CSVWithNames",
    "CSVWithNamesAndTypes",
    "CustomSeparated",
    "CustomSeparatedWithNames",
    "CustomSeparatedWithNamesAndTypes",
    "ExcelCSV",
    "HiveText",
    "HTML",
    "JSON",
    "JSONAsString",
    "JSONColumns",
    "JSONColumnsWithMetadata",
    "JSONCompact",
    "JSONCompactColumns",
    "JSONCompactEachRow",
    "JSONCompactEachRowWithNames",
    "JSONCompactEachRowWithNamesAndTypes",
    "JSONCompactStrings",
    "JSONEachRow",
    "JSONEachRowWithProgress",
    "JSONLines",
    "JSONObjectEachRow",
    "JSONStrings",
    "JSONStringsEachRow",
    "JSONStringsEachRowWithProgress",
    "Markdown",
    "MsgPack",
    "MySQLDump",
    "Native",
    "Null",
    "ORC",
    "Parquet",
    "ParquetMetadata",
    "Pretty",
    "PrettyCompact",
    "PrettyCompactMonoBlock",
    "PrettyCompactNoEscapes",
    "PrettyMonoBlock",
    "PrettyNoEscapes",
    "PrettySpace",
    "PrettySpaceNoEscapes",
    "Prometheus",
    "Protobuf",
    "ProtobufList",
    "ProtobufSingle",
    "RawBLOB",
    "RowBinary",
    "RowBinaryWithNames",
    "RowBinaryWithNamesAndTypes",
    "SQLInsert",
    "TSKV",
    "TabSeparated",
    "TabSeparatedRaw",
    "TabSeparatedRawWithNames",
    "TabSeparatedRawWithNamesAndTypes",
    "TabSeparatedWithNames",
    "TabSeparatedWithNamesAndTypes",
    "Template",
    "TemplateIgnoreSpaces",
    "TSV",
    "TSVWithNames",
    "TSVWithNamesAndTypes",
    "Values",
    "Vertical",
    "VerticalRaw",
    "XML",
    "YAML",
];

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::clickhouse::make_test_object;
    use crate::db::{QuerySql, SchemaTrait};
    use crate::enable_test_log;
    use crate::language_server::completion_test::test_file_auto_completion;

    // partition test failed , sqlparser can't parse a valid partition statement.
    #[tokio::test]
    async fn test_auto_completion() {
        let db = make_test_object();
        // enable_test_log();
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("employees", false).await;
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE employees (
                    id UInt32,
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
        .unwrap()
        .unwrap();

        let _ = db
            .execute_sql(
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
    #[tokio::test]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = make_test_object();
        test_file_auto_completion(include_str!("./xxx.sql"), &db, false).await;
    }
}
