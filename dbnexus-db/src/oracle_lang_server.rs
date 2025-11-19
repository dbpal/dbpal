use tokio::task::block_in_place;

use crate::db::{DBTrait, name_start_ascii_alphabetic};
use crate::language_server::{
    AstCompletionHelper, CompletionItem, CompletionKind, CompletionList, GetKeyWords,
    LangServerCommon, LanguageServer, LexerCompletionTrait, try_parser_dot_lexer,
};
use crate::oracle::OracleDB;
use crate::sqlparser_completion::TempTableColumn;

#[async_trait::async_trait]
impl LexerCompletionTrait for OracleDB {
    const TYPES: &'static [&'static str] = ORACLE_TYPES;
}

#[async_trait::async_trait]

impl GetKeyWords for OracleDB {
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        let mut lock = self.key_words_cache.lock().await;
        if !lock.is_empty() {
            return anyhow::Ok(lock.clone());
        }
        let conn = self.get_conn().await?;
        let sql = r#"
            SELECT  keyword 
                FROM V$RESERVED_WORDS
        "#;
        let result = block_in_place(|| conn.query(sql, &[]))?;
        let mut ret = Vec::new();
        for r in result {
            let r = r?;
            let name: Option<String> = r.get(0)?;
            if let Some(name) = name {
                let name = name.trim().to_string();
                if name.is_empty() {
                    continue;
                }
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

impl OracleDB {
    fn schema_merge_config(&self, schema: Option<&str>) -> Option<String> {
        if schema.is_some() {
            return schema.map(|x| x.to_string());
        }
        self.config.schema.as_ref().map(|x| x.to_string())
    }
    fn not_allow_empty_schema(&self, schema: Option<&str>) -> anyhow::Result<String> {
        match self.schema_merge_config(schema) {
            Some(x) => anyhow::Ok(x),
            None => anyhow::bail!("empty schema"),
        }
    }
}

impl OracleDB {
    async fn extact_names(&self, sql: &str) -> anyhow::Result<Vec<String>> {
        let conn = self.get_conn().await?;
        let rows = conn.query(sql, &[])?;
        let mut ret = Vec::new();
        for row in rows {
            let row = row?;

            let name = row.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl AstCompletionHelper for OracleDB {
    async fn get_query_columns(&self, sql: &str) -> anyhow::Result<Vec<TempTableColumn>> {
        let conn = self.get_conn().await?;

        let row = conn.query_row(sql, &[])?;

        anyhow::Ok(
            row.column_info()
                .iter()
                .map(|x| TempTableColumn {
                    name: x.name().to_string(),
                    data_type: Some(x.oracle_type().to_string()),
                })
                .collect(),
        )
    }
}

#[async_trait::async_trait]
impl LangServerCommon for OracleDB {
    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = self.not_allow_empty_schema(schema)?;
        let sql = format!(
            "SELECT INDEX_NAME FROM ALL_INDEXES WHERE OWNER='{}'",
            schema
        );
        log::info!("sql:{}", sql);
        let conn = self.get_conn().await?;
        let rows = conn.query(&sql, &[])?;
        let mut ret = Vec::new();
        for row in rows {
            let row = row?;

            let name: String = row.get(0)?;

            ret.push(name);
        }
        anyhow::Ok(ret)
    }
    /*
    -- 最简单的调用
    SELECT scott.函数名(参数) AS 别名 FROM dual;

    -- 例子
    SELECT scott.f_get_emp_name(7839) AS ename FROM dual;
    调用函数的例子和调用一个表一样 所以is_call不需要单独处理
    */
    async fn list_column(
        &self, schema: Option<&str>, name: &str, _is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let sql = format!(
            r#"select * from {}"{}""#,
            match schema {
                Some(x) => format!("\"{}\".", x),
                None => "".to_string(),
            },
            name
        );
        log::info!("sql:{}", sql);
        self.get_query_columns(&sql).await
    }

    async fn list_function(
        &self, schema: Option<&str>, include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let ret = self
            .extact_names(
                r#"
            SELECT
                object_name
            FROM
                user_objects
            WHERE
                object_type = 'FUNCTION'
        "#,
            )
            .await?;

        if let Some(schema) = self.schema_merge_config(schema) {
            self.extact_names(
                format!(
                    r#"
                    SELECT
                        object_name
                    FROM
                        dba_objects
                    WHERE
                        object_type = 'FUNCTION'
                        AND OWNER IN ('{}') 
                "#,
                    schema
                )
                .as_str(),
            )
            .await?;
        }
        let mut ret: Vec<_> = ret
            .into_iter()
            .map(|x| CompletionItem {
                item: x.into(),
                kind: CompletionKind::Function,
                desc: None,
            })
            .collect();

        if include_build_in {
            ret.extend(ORACLE_BUILD_IN_FUNCTIONS.iter().map(|x| CompletionItem {
                item: (*x).into(),
                kind: CompletionKind::Function,
                desc: None,
            }));
        }
        anyhow::Ok(ret)
    }

    async fn list_procedure(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = self.not_allow_empty_schema(schema)?;
        self.list_procedure_impl(&schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }

    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::GenericDialect {})
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        DBTrait::list_schema(self).await
    }
    async fn list_table(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = self.not_allow_empty_schema(schema)?;

        self.list_table_impl(&schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }
    async fn list_view(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let schema = self.not_allow_empty_schema(schema)?;

        self.list_view_impl(&schema)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }
}

#[async_trait::async_trait]
impl LanguageServer for OracleDB {
    async fn try_get_max_id(&self, name: &str) -> anyhow::Result<String> {
        let sql = format!("SELECT MAX(id) FROM \"{name}\"");
        let conn = self.get_conn().await?;
        let row = block_in_place(|| conn.query_row(&sql, &[]))?;

        if row.sql_values().len() > 0 {
            return anyhow::Ok("".to_string());
        }
        anyhow::Ok(row.sql_values()[0].to_string())
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
        try_parser_dot_lexer(self, sql, line, col, use_lexer).await
    }
    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        LangServerCommon::is_dangerous_sql_by_parser(self, sql)
    }
}

pub(crate) const ORACLE_TYPES: &[&str] = &[
    "NUMBER",
    "BINARY_FLOAT",
    "BINARY_DOUBLE",
    "BINARY_INTEGER",
    "CHAR",
    "VARCHAR2",
    "NCHAR",
    "NVARCHAR2",
    "DATE",
    "TIMESTAMP",
    "INTERVAL YEAR TO MONTH",
    "INTERVAL DAY TO SECOND",
    "RAW",
    "LONG RAW",
    "CLOB",
    "NCLOB",
    "BLOB",
    "SDO_GEOMETRY",
    "XMLType",
];

/// https://docs.oracle.com/en/database/oracle/oracle-database/19/sqlrf/Single-Row-Functions.html#GUID-B93F789D-B486-49FF-B0CD-0C6181C5D85C
///  all function collect from.
const ORACLE_BUILD_IN_FUNCTIONS: &[&str] = &[
    // Numeric Functions
    "ABS",
    "ACOS",
    "ASIN",
    "ATAN",
    "ATAN2",
    "BITAND",
    "CEIL",
    "COS",
    "COSH",
    "EXP",
    "FLOOR",
    "LN",
    "LOG",
    "MOD",
    "NANVL",
    "POWER",
    "REMAINDER",
    "ROUND (number)",
    "SIGN",
    "SIN",
    "SINH",
    "SQRT",
    "TAN",
    "TANH",
    "TRUNC",
    "WIDTH_BUCKET",
    //  Character Functions Returning Character Values
    "CHR",
    "CONCAT",
    "INITCAP",
    "LOWER",
    "LPAD",
    "LTRIM",
    "NCHR",
    "NLS_INITCAP",
    "NLS_LOWER",
    "NLS_UPPER",
    "NLSSORT",
    "REGEXP_REPLACE",
    "REGEXP_SUBSTR",
    "REPLACE",
    "RPAD",
    "RTRIM",
    "SOUNDEX",
    "SUBSTR",
    "TRANSLATE",
    "TRANSLATE ... USING",
    "TRIM",
    "UPPER",
    // Character Functions Returning Number Values
    "ASCII",
    "INSTR",
    "LENGTH",
    "REGEXP_COUNT",
    "REGEXP_INSTR",
    // Character Set Functions
    "NLS_CHARSET_DECL_LEN",
    "NLS_CHARSET_ID",
    "NLS_CHARSET_NAME",
    // Collation Functions
    "COLLATION",
    "NLS_COLLATION_ID",
    "NLS_COLLATION_NAME",
    // Datetime Functions
    "ADD_MONTHS",
    "CURRENT_DATE",
    "CURRENT_TIMESTAMP",
    "DBTIMEZONE",
    "EXTRACT",
    "FROM_TZ",
    "LAST_DAY",
    "LOCALTIMESTAMP",
    "MONTHS_BETWEEN",
    "NEW_TIME",
    "NEXT_DAY",
    "NUMTODSINTERVAL",
    "NUMTOYMINTERVAL",
    "ORA_DST_AFFECTED",
    "ORA_DST_CONVERT",
    "ORA_DST_ERROR",
    "ROUND",
    "SESSIONTIMEZONE",
    "SYS_EXTRACT_UTC",
    "SYSDATE",
    "SYSTIMESTAMP",
    "TO_CHAR",
    "TO_DSINTERVAL",
    "TO_TIMESTAMP",
    "TO_TIMESTAMP_TZ",
    "TO_YMINTERVAL",
    "TRUNC",
    "TZ_OFFSET",
    // General Comparison Functions
    "GREATEST",
    "LEAST",
    // Conversion Functions
    "ASCIISTR",
    "BIN_TO_NUM",
    "CAST",
    "CHARTOROWID",
    "COMPOSE",
    "CONVERT",
    "DECOMPOSE",
    "HEXTORAW",
    "NUMTODSINTERVAL",
    "NUMTOYMINTERVAL",
    "RAWTOHEX",
    "RAWTONHEX",
    "ROWIDTOCHAR",
    "ROWIDTONCHAR",
    "SCN_TO_TIMESTAMP",
    "TIMESTAMP_TO_SCN",
    "TO_BINARY_DOUBLE",
    "TO_BINARY_FLOAT",
    "TO_BLOB",
    "TO_CHAR",
    "TO_CLOB",
    "TO_DATE",
    "TO_DSINTERVAL",
    "TO_LOB",
    "TO_MULTI_BYTE",
    "TO_NCHAR",
    "TO_NCLOB",
    "TO_NUMBER",
    "TO_SINGLE_BYTE",
    "TO_TIMESTAMP",
    "TO_TIMESTAMP_TZ",
    "TO_YMINTERVAL",
    "TREAT",
    "UNISTR",
    "VALIDATE_CONVERSION",
    // Large Object Functions
    "BFILENAME",
    "EMPTY_BLOB",
    "EMPTY_CLOB",
    // Collection Functions
    "CARDINALITY",
    "COLLECT",
    "POWERMULTISET",
    "POWERMULTISET_BY_CARDINALITY",
    "SET",
    // Hierarchical Functions
    "SYS_CONNECT_BY_PATH",
    // Data Mining Functions
    "CLUSTER_DETAILS",
    "CLUSTER_DISTANCE",
    "CLUSTER_ID",
    "CLUSTER_PROBABILITY",
    "CLUSTER_SET",
    "FEATURE_COMPARE",
    "FEATURE_DETAILS",
    "FEATURE_ID",
    "FEATURE_SET",
    "FEATURE_VALUE",
    "ORA_DM_PARTITION_NAME",
    "PREDICTION",
    "PREDICTION_BOUNDS",
    "PREDICTION_COST",
    "PREDICTION_DETAILS",
    "PREDICTION_PROBABILITY",
    "PREDICTION_SET",
    // The SQL XML functions are:
    "DEPTH",
    "EXISTSNODE",
    "EXTRACT (XML)",
    "EXTRACTVALUE",
    "PATH",
    "SYS_DBURIGEN",
    "SYS_XMLAGG",
    "SYS_XMLGEN",
    "XMLAGG",
    "XMLCAST",
    "XMLCDATA",
    "XMLCOLATTVAL",
    "XMLCOMMENT",
    "XMLCONCAT",
    "XMLDIFF",
    "XMLELEMENT",
    "XMLEXISTS",
    "XMLFOREST",
    "XMLISVALID",
    "XMLPARSE",
    "XMLPATCH",
    "XMLPI",
    "XMLQUERY",
    "XMLROOT",
    "XMLSEQUENCE",
    "XMLSERIALIZE",
    "XMLTABLE",
    "XMLTRANSFORM",
    // JSON Functions
    "JSON_QUERY",
    "JSON_TABLE",
    "JSON_VALUE",
    // The following SQL/JSON functions allow you to generate JSON data:
    "JSON_ARRAY",
    "JSON_ARRAYAGG",
    "JSON_OBJECT",
    "JSON_OBJECTAGG",
    // The following Oracle SQL function creates a JSON data guide:
    "JSON_DATAGUIDE",
    // Encoding and Decoding Functions
    "DECODE",
    "DUMP",
    "ORA_HASH",
    "STANDARD_HASH",
    "VSIZE",
    // NULL-Related Functions
    "COALESCE",
    "LNNVL",
    "NANVL",
    "NULLIF",
    "NVL",
    "NVL2",
    // Environment and Identifier Functions
    "CON_DBID_TO_ID",
    "CON_GUID_TO_ID",
    "CON_NAME_TO_ID",
    "CON_UID_TO_ID",
    "ORA_INVOKING_USER",
    "ORA_INVOKING_USERID",
    "SYS_CONTEXT",
    "SYS_GUID",
    "SYS_TYPEID",
    "UID",
    "USER",
    "USERENV",
    //
    "APPROX_COUNT",
    "APPROX_COUNT_DISTINCT",
    "APPROX_COUNT_DISTINCT_AGG",
    "APPROX_COUNT_DISTINCT_DETAIL",
    "APPROX_MEDIAN",
    "APPROX_PERCENTILE",
    "APPROX_PERCENTILE_AGG",
    "APPROX_PERCENTILE_DETAIL",
    "APPROX_RANK",
    "APPROX_SUM",
    "AVG",
    "COLLECT",
    "CORR",
    "CORR_S",
    "CORR_K",
    "COUNT",
    "COVAR_POP",
    "COVAR_SAMP",
    "CUME_DIST",
    "DENSE_RANK",
    "FIRST",
    "GROUP_ID",
    "GROUPING",
    "GROUPING_ID",
    "JSON_ARRAYAGG",
    "JSON_OBJECTAGG",
    "LAST",
    "LISTAGG",
    "MAX",
    "MEDIAN",
    "MIN",
    "PERCENT_RANK",
    "PERCENTILE_CONT",
    "PERCENTILE_DISC",
    "RANK",
    "REGR_ (Linear Regression) Functions",
    "STATS_BINOMIAL_TEST",
    "STATS_CROSSTAB",
    "STATS_F_TEST",
    "STATS_KS_TEST",
    "STATS_MODE",
    "STATS_MW_TEST",
    "STATS_ONE_WAY_ANOVA",
    "STATS_T_TEST_*",
    "STATS_WSR_TEST",
    "STDDEV",
    "STDDEV_POP",
    "STDDEV_SAMP",
    "SUM",
    "SYS_OP_ZONE_ID",
    "SYS_XMLAGG",
    "TO_APPROX_COUNT_DISTINCT",
    "TO_APPROX_PERCENTILE",
    "VAR_POP",
    "VAR_SAMP",
    "VARIANCE",
    "XMLAGG",
    //  Analytic Functions
    "AVG",
    "CLUSTER_DETAILS",
    "CLUSTER_DISTANCE",
    "CLUSTER_ID",
    "CLUSTER_PROBABILITY",
    "CLUSTER_SET",
    "CORR",
    "COUNT",
    "COVAR_POP",
    "COVAR_SAMP",
    "CUME_DIST",
    "DENSE_RANK",
    "FEATURE_DETAILS",
    "FEATURE_ID",
    "FEATURE_SET",
    "FEATURE_VALUE",
    "FIRST",
    "FIRST_VALUE",
    "LAG",
    "LAST",
    "LAST_VALUE",
    "LEAD",
    "LISTAGG",
    "MAX",
    "MIN",
    "NTH_VALUE",
    "NTILE",
    "PERCENT_RANK",
    "PERCENTILE_CONT",
    "PERCENTILE_DISC",
    "PREDICTION",
    "PREDICTION_COST",
    "PREDICTION_DETAILS",
    "PREDICTION_PROBABILITY",
    "PREDICTION_SET",
    "RANK",
    "RATIO_TO_REPORT",
    "ROW_NUMBER",
    "STDDEV",
    "STDDEV_POP",
    "STDDEV_SAMP",
    "SUM",
    "VAR_POP",
    "VAR_SAMP",
    "VARIANCE",
    "REGR_SLOPE",
    "REGR_INTERCEPT",
    "REGR_COUNT",
    "REGR_R2",
    "REGR_AVGX",
    "REGR_AVGY",
    "REGR_SXX",
    "REGR_SYY",
    "REGR_SXY",
    // Object Reference Functions
    "DEREF",
    "MAKE_REF",
    "REF",
    "REFTOHEX",
    "VALUE",
    // Model Functions
    "CV",
    "ITERATION_NUMBER",
    "PRESENTNNV",
    "PRESENTV",
    "PREVIOUS",
    // OLAP Functions
    "CUBE_TABLE",
    // Data Cartridge Functions
    "DATAOBJ_TO_MAT_PARTITION",
    "DATAOBJ_TO_PARTITION",
];
