use std::vec;

use futures_util::StreamExt;
use tokio::sync::Mutex;

use crate::clickhouse::{ClickHouseDB, FieldsOrData};
use crate::db::DBTrait;
use crate::language_server::{
    try_parser_dot_lexer, AstCompletionHelper, CompletionItem, CompletionKind, CompletionList,
    GetKeyWords, LangServerCommon, LanguageServer, LexerCompletionTrait,
};
use crate::sqlparser_completion::TempTableColumn;

#[async_trait::async_trait]
impl GetKeyWords for ClickHouseDB {
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        static KEY_WORDS_CACHE: Mutex<Vec<String>> = Mutex::const_new(Vec::new());
        let mut lock = KEY_WORDS_CACHE.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        let sql = "select * from system.keywords";
        let ret: Vec<String> = self.client.query(sql).fetch_all().await?;
        *lock = ret;
        return anyhow::Ok(lock.clone());
    }
}

#[async_trait::async_trait]

impl LanguageServer for ClickHouseDB {
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
        _query_id: Option<String>,
        sql: &str,
        line: u32,
        col: u32,
        use_lexer: bool,
    ) -> anyhow::Result<CompletionList> {
        // self.completion_id.set_optional(query_id);
        try_parser_dot_lexer(self, sql, line, col, use_lexer).await
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

        let ret: Vec<String> = self.client.query(sql).fetch_all().await?;
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
        &self,
        schema: Option<&str>,
        name: &str,
        is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let schema = self.not_allow_none_schema(schema)?;
        let schema_dot = format!("\"{}\".", schema);
        let sql = if let Some(is_call) = is_call {
            format!(r#"select {}{}({})"#, schema_dot, name, is_call.join(","))
        } else {
            format!(r#"select * from {}"{}""#, schema_dot, name)
        };
        self.get_query_columns(&sql).await
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        DBTrait::list_schema(self).await
    }

    async fn list_function(
        &self,
        _schema: Option<&str>,
        _include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        // 在 ClickHouse 中，使用以下查询可以查看所有函数，包括表函数（table functions）：
        // 看起来clickhouse 都有全局的作用域
        let sql = "select name , description from system.functions";

        let result: Vec<(String, String)> = self.client.query(sql).fetch_all().await?;

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
        &self,
        schema: Option<&str>,
        table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let db = self.not_allow_none_schema(schema)?;
        let sql = format!(
            r#"
            SELECT name 
                FROM system.projections 
                WHERE "table"  = '{table}' and database = '{db}'
        "#
        );
        let result: Vec<String> = self.client.query(&sql).fetch_all().await?;
        anyhow::Ok(result)
    }

    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = format!("select * from({}) limit 1", sql);
        let mut stream = self.query_row(&sql).await?;
        log::info!("get query columns:{}", sql);

        while let Some(x) = stream.next().await {
            match x? {
                FieldsOrData::Fields(fields) => {
                    return anyhow::Ok(
                        fields
                            .into_iter()
                            .map(|x| TempTableColumn {
                                name: x.field,
                                data_type: Some(x.data_type),
                            })
                            .collect(),
                    );
                }
                FieldsOrData::Data(..) => {}
                FieldsOrData::ReadingColumns => {}
            }
        }

        anyhow::Ok(vec![])
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for ClickHouseDB {
    async fn get_types(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(CLICKHOUSE_TYPES.iter().map(|x| x.to_string()).collect())
    }

    async fn get_text(&self) -> anyhow::Result<Vec<String>> {
        let r = ["true", "false"].iter().map(|x| x.to_string()).collect();
        anyhow::Ok(r)
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

    use crate::clickhouse::{make_test_object, test_lock, ClickHouseDB};
    use crate::datasource_config::ClickHouseConfig;
    use crate::db::{QuerySql, SchemaTrait};
    use crate::enable_test_log;
    use crate::language_server::completion_test::test_file_auto_completion;

    // partition test failed , sqlparser can't parse a valid partition statement.
    #[tokio::test]
    async fn test_auto_completion() {
        let _guard = test_lock().lock().await;
        let db = make_test_object();
        // enable_test_log();
        let query_id = "aaaa";
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
        test_file_auto_completion(&content, &db, None, false).await;
    }
    #[ignore = "手动跑跑的测试"]
    #[tokio::test]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = ClickHouseDB::new(ClickHouseConfig {
            conn_timeout: Some(5000.into()),
            host: "127.0.0.1".to_string(),
            port: 8123,
            read_only: false,
            db: Some("information_schema".to_string()),
            username: "default".to_string(),
            password: Some("root".to_string()),
            https: false,
        })
        .unwrap();

        test_file_auto_completion(include_str!("./xxx.sql"), &db, None, false).await;
    }
}
