use crate::cassandra::CassandraDB;
use crate::db::{CreatedTypeDef, DBTrait};
use crate::language_server::{
    CompletionItem, CompletionKind, CompletionList, GetKeyWords, LangServerCommon, LanguageServer,
    LexerCompletionTrait,
};
use crate::sqlparser_completion::TempTableColumn;
use cql3_parser::cassandra_ast::CassandraAST;
use cql3_parser::cassandra_statement::CassandraStatement;
use futures_util::StreamExt;
use scylla::client::session::Session;
use std::sync::Arc;
#[async_trait::async_trait]
impl GetKeyWords for CassandraDB {
    async fn key_words(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(CASSANDRA_KEYWORDS.iter().map(|x| x.to_string()).collect())
    }
}

#[async_trait::async_trait]
impl LanguageServer for CassandraDB {
    async fn ai_text_completion(
        &self,
        sql: &str,
        line: u32,
        col: u32,
    ) -> anyhow::Result<CompletionList> {
        LexerCompletionTrait::ai_text_completion(self, sql, line, col).await
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
        _use_lexer: bool,
    ) -> anyhow::Result<CompletionList> {
        self.completion_id.set_optional(query_id);
        let dot = self.try_completion_dot(sql, line, col).await;
        match dot {
            Ok(x) => return anyhow::Ok(x),
            Err(err) => {
                log::error!("try dot failed: {:?}", err);
            }
        }
        return LexerCompletionTrait::do_auto_completion(self, sql, line, col).await;
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        let ast = CassandraAST::new(sql);
        for stmt in ast.statements.iter() {
            let x = match &stmt.statement {
                CassandraStatement::AlterKeyspace(..) => true,
                CassandraStatement::AlterMaterializedView(..) => true,
                CassandraStatement::AlterRole(..) => true,
                CassandraStatement::AlterTable(..) => true,
                CassandraStatement::AlterType(..) => true,
                CassandraStatement::AlterUser(..) => true,
                CassandraStatement::ApplyBatch => false,
                CassandraStatement::CreateAggregate(..) => false,
                CassandraStatement::CreateFunction(..) => false,
                CassandraStatement::CreateIndex(..) => false,
                CassandraStatement::CreateKeyspace(..) => false,
                CassandraStatement::CreateMaterializedView(..) => false,
                CassandraStatement::CreateRole(..) => false,
                CassandraStatement::CreateTable(..) => false,
                CassandraStatement::CreateTrigger(..) => false,
                CassandraStatement::CreateType(..) => false,
                CassandraStatement::CreateUser(..) => false,
                CassandraStatement::Delete(..) => false,
                CassandraStatement::DropAggregate(..) => true,
                CassandraStatement::DropFunction(..) => true,
                CassandraStatement::DropIndex(..) => true,
                CassandraStatement::DropKeyspace(..) => true,
                CassandraStatement::DropMaterializedView(..) => true,
                CassandraStatement::DropRole(..) => true,
                CassandraStatement::DropTable(..) => true,
                CassandraStatement::DropTrigger(..) => true,
                CassandraStatement::DropType(..) => true,
                CassandraStatement::DropUser(..) => true,
                CassandraStatement::Grant(..) => true,
                CassandraStatement::Insert(..) => false,
                CassandraStatement::ListPermissions(..) => false,
                CassandraStatement::ListRoles(..) => false,
                CassandraStatement::Revoke(..) => false,
                CassandraStatement::Select(..) => false,
                CassandraStatement::Truncate(..) => true,
                CassandraStatement::Update(..) => false,
                CassandraStatement::Use(..) => false,
                CassandraStatement::Unknown(_) => false,
            };
            if x {
                return anyhow::Ok(x);
            }
        }
        anyhow::Ok(false)
    }
}

#[async_trait::async_trait]
impl LexerCompletionTrait for CassandraDB {
    async fn get_types(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(CASSANDRA_TYPES.iter().map(|x| x.to_string()).collect())
    }
    async fn get_text(&self) -> anyhow::Result<Vec<String>> {
        let r = ["true", "false"].iter().map(|x| x.to_string()).collect();
        anyhow::Ok(r)
    }
}

impl CassandraDB {
    fn get_query_session(&self) -> anyhow::Result<Arc<Session>> {
        match self.query_connection.get(&self.completion_id.get()) {
            Some(x) => anyhow::Ok(x.clone()),
            None => anyhow::bail!("connection closed???"),
        }
    }
}

#[async_trait::async_trait]
impl LangServerCommon for CassandraDB {
    async fn list_index(&self, key_space: Option<&str>) -> anyhow::Result<Vec<String>> {
        let key_space = self.need_key_space(key_space).await?;

        self.get_index_name(&key_space).await
    }

    async fn list_created_type(
        &self,
        key_space: Option<&str>,
    ) -> anyhow::Result<Vec<CreatedTypeDef>> {
        let key_space = self.need_key_space(key_space).await?;
        self.list_created_types_impl(&key_space).await
    }

    /*
        _is_call not used because cassandra have it's own parser
        _is_call mainly use for function call statement like `select abs(-1)`
        in sqlparser completion helper
    */
    async fn list_column(
        &self,
        key_space: Option<&str>,
        table_name: &str,
        _is_call: Option<Vec<String>>,
    ) -> anyhow::Result<Vec<TempTableColumn>> {
        let key_space = self.need_key_space(key_space).await?;
        // this sql can query view columns too.
        let sql = format!(
            "SELECT column_name ,type
                            FROM system_schema.columns 
                            WHERE keyspace_name = '{}' 
                            AND table_name = '{}';",
            key_space, table_name,
        );
        log::info!("sql:{}", sql);
        let session = self.get_query_session()?;
        let pager = session.query_iter(sql, &[]).await?;
        let mut stram = pager.rows_stream::<(String, String)>()?;
        let mut result = Vec::new();
        while let Some(x) = stram.next().await {
            let (name, data_type) = x?;

            result.push(TempTableColumn {
                name,
                data_type: Some(data_type), // Cassandra does not provide data type in this query
            });
        }
        anyhow::Ok(result)
    }
    async fn list_function(
        &self,
        key_space: Option<&str>,
        include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let mut ret = Vec::new();
        let key_space = self.need_key_space(key_space).await?;
        ret.extend(
            self.list_aggregate_impl(&key_space)
                .await?
                .into_iter()
                .map(|x| CompletionItem {
                    kind: CompletionKind::Function,
                    desc: x.cassandra_parameters.clone().map(|x| x.join(",").into()),
                    item: x.name.clone().into(),
                }),
        );
        ret.extend(
            self.list_udf_impl(&key_space)
                .await?
                .into_iter()
                .map(|x| CompletionItem {
                    kind: CompletionKind::Function,
                    desc: x.cassandra_parameters.clone().map(|x| x.join(",").into()),
                    item: x.name.clone().into(),
                }),
        );
        if include_build_in {
            ret.extend(self.cassandra_build_in_fns());
        }
        anyhow::Ok(ret)
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        DBTrait::list_schema(self).await
    }
    fn dialect(&self) -> Box<dyn sqlparser::dialect::Dialect + Send + Sync> {
        Box::new(sqlparser::dialect::GenericDialect {})
    }
    async fn list_table(&self, key_space: Option<&str>) -> anyhow::Result<Vec<String>> {
        let key_space = self.need_key_space(key_space).await?;
        // log::trace!("cassandra list table by space name:{}", key_space);
        CassandraDB::list_table_impl(self, &key_space)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }
    async fn list_view(&self, key_space: Option<&str>) -> anyhow::Result<Vec<String>> {
        let key_space = self.need_key_space(key_space).await?;
        self.list_materialized_view_impl(&key_space)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }
}

impl CassandraDB {
    fn cassandra_build_in_fns(&self) -> Vec<CompletionItem> {
        CASSANDRA_FUNCTIONS
            .iter()
            .map(|x| CompletionItem {
                item: (*x).into(),
                kind: CompletionKind::Function,
                desc: None,
            })
            .collect()
    }
}

impl CassandraDB {
    async fn get_index_name(&self, key_space: &String) -> anyhow::Result<Vec<String>> {
        let sql = format!(
            r#"
            SELECT index_name 
                FROM system_schema.indexes 
                WHERE keyspace_name = '{key_space}'
        "#
        );
        log::info!("sql:{}", sql);
        let session = self.get_query_session()?;
        let rows = session.query_iter(sql, &[]).await?;
        let mut stream = rows.rows_stream::<(String,)>()?;
        let mut ret = Vec::new();
        while let Some(row) = stream.next().await {
            let (name,) = row?;

            ret.push(name);
        }
        anyhow::Ok(ret)
    }
}

impl CassandraDB {
    async fn need_key_space(&self, key_space: Option<&str>) -> anyhow::Result<String> {
        if let Some(key_space) = key_space {
            return anyhow::Ok(key_space.to_string());
        }
        let session = self.get_query_session()?;
        if let Some(x) = session.get_keyspace() {
            return anyhow::Ok(x.as_str().to_string());
        }
        match &self.config.db {
            Some(x) => anyhow::Ok(x.clone()),
            None => Err(anyhow::Error::msg("need provide key space")),
        }
    }
}

const CASSANDRA_TYPES: &[&str] = &[
    "CUSTOM",
    "ASCII",
    "BIGINT",
    "BLOB",
    "BOOLEAN",
    "COUNTER",
    "DECIMAL",
    "DOUBLE",
    "FLOAT",
    "INT",
    "TIMESTAMP",
    "UUID",
    "VARCHAR",
    "VARINT",
    "TIMEUUID",
    "INET",
    "DATE",
    "TIME",
    "SMALLINT",
    "TINYINT",
    "DURATION",
    "LIST",
    "MAP",
    "SET",
    "UDT",
    "TUPLE",
];
/// Apache Cassandra 官方 CQL 关键字（CQL 3.4）
pub const CASSANDRA_KEYWORDS: &[&str] = &[
    "ADD",
    "AGGREGATE",
    "ALL",
    "ALLOW",
    "ALTER",
    "AND",
    "ANY",
    "APPLY",
    "AS",
    "ASC",
    "ASCII",
    "AUTHORIZE",
    "BATCH",
    "BEGIN",
    "BIGINT",
    "BLOB",
    "BOOLEAN",
    "BY",
    "CLUSTERING",
    "COLUMNFAMILY",
    "COMPACT",
    "CONSISTENCY",
    "COUNT",
    "COUNTER",
    "CREATE",
    "CUSTOM",
    "DATE",
    "DECIMAL",
    "DELETE",
    "DESC",
    "DISTINCT",
    "DOUBLE",
    "DROP",
    "EACH_QUORUM",
    "ENTRIES",
    "EXISTS",
    "FILTERING",
    "FLOAT",
    "FROZEN",
    "FROM",
    "FULL",
    "GRANT",
    "IF",
    "IN",
    "INDEX",
    "INET",
    "INFINITY",
    "INSERT",
    "INT",
    "INTO",
    "KEY",
    "KEYSPACE",
    "KEYSPACES",
    "LEVEL",
    "LIMIT",
    "LIST",
    "LOCAL_ONE",
    "LOCAL_QUORUM",
    "MAP",
    "MATERIALIZED",
    "MODIFY",
    "NAN",
    "NORECURSIVE",
    "NOSUPERUSER",
    "NOT",
    "OF",
    "ON",
    "ONE",
    "ORDER",
    "PARTITION",
    "PASSWORD",
    "PER",
    "PERMISSION",
    "PERMISSIONS",
    "PRIMARY",
    "QUORUM",
    "RENAME",
    "REVOKE",
    "SCHEMA",
    "SELECT",
    "SET",
    "STATIC",
    "STORAGE",
    "SUPERUSER",
    "TABLE",
    "TEXT",
    "TIME",
    "TIMESTAMP",
    "TIMEUUID",
    "THREE",
    "TO",
    "TOKEN",
    "TRUNCATE",
    "TTL",
    "TUPLE",
    "TWO",
    "TYPE",
    "UNLOGGED",
    "UPDATE",
    "USE",
    "USER",
    "USERS",
    "USING",
    "UUID",
    "VALUES",
    "VARCHAR",
    "VARINT",
    "VIEW",
    "WHERE",
    "WITH",
    "WRITETIME",
];
/// Apache Cassandra 官方内置函数清单（CQL 3.4+）
pub const CASSANDRA_FUNCTIONS: &[&str] = &[
    // 日期/时间
    "current_timestamp",
    "current_date",
    "current_time",
    "current_timeuuid",
    "to_date",
    "to_timestamp",
    "to_unix_timestamp",
    "dateof",
    "unix_timestamp_of",
    // 转换
    "cast",
    "type_as_blob",
    "blob_as_bigint",
    "blob_as_date",
    "blob_as_decimal",
    "blob_as_double",
    "blob_as_float",
    "blob_as_int",
    "blob_as_smallint",
    "blob_as_text",
    "blob_as_time",
    "blob_as_timestamp",
    "blob_as_tinyint",
    "blob_as_uuid",
    "blob_as_varint",
    // 数学
    "abs",
    "acos",
    "asin",
    "atan",
    "atan2",
    "ceil",
    "cos",
    "exp",
    "floor",
    "log",
    "log10",
    "pow",
    "round",
    "sin",
    "sqrt",
    "tan",
    // 聚合
    "avg",
    "count",
    "min",
    "max",
    "sum",
    // 字符串
    "length",
    "char_length",
    "character_length",
    "lower",
    "upper",
    "trim",
    "ltrim",
    "rtrim",
    "replace",
    "reverse",
    "substr",
    "substring",
    // 集合
    "collection_count",
    "collection_size",
    // 标识
    "uuid",
    "now",
    // token 计算
    "token",
    // 类型检查
    "typeof",
    // 其他辅助
    "ttl",
    "writetime",
    // 二进制
    "bigint_as_blob",
    "boolean_as_blob",
    "date_as_blob",
    "decimal_as_blob",
    "double_as_blob",
    "float_as_blob",
    "inet_as_blob",
    "int_as_blob",
    "smallint_as_blob",
    "text_as_blob",
    "time_as_blob",
    "timestamp_as_blob",
    "tinyint_as_blob",
    "uuid_as_blob",
    "varint_as_blob",
];

#[cfg(test)]
mod tests {
    use crate::{
        cassandra::make_test_object, db::QuerySql, enable_test_log, language_server::LanguageServer,
    };

    #[ignore = "run manually"]
    #[tokio::test]
    async fn xxx() {
        enable_test_log();
        let db = make_test_object(Some("system".to_string())).await;
        db.completion_id.set("".to_string());
        db.open_connection("").await.unwrap();
        let x = LanguageServer::auto_completion(
            &db,
            Some("".to_string()),
            include_str!("./xxx.sql"),
            1,
            23,
            true,
        )
        .await
        .unwrap();
        for name in x.items.iter() {
            println!("{}", name.item)
        }
        assert!(x.from_dot);
    }
}
