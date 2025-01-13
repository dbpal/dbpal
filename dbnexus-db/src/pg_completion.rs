use std::ops::DerefMut;
use std::sync::Arc;

use sqlx::Row;

use crate::completion::{
    AstCompletionHelper, LanguageServer, CompletionItem, CompletionKind, LexerCompletionTrait,
    try_completion_on_syntax,
};
use crate::db::CatalogTrait;
use crate::pg::PostGresDB;

#[async_trait::async_trait]
impl LexerCompletionTrait for PostGresDB {
    type Dialect = sqlparser::dialect::PostgreSqlDialect;

    const KEY_WORDS: &'static [&'static str] = PG_KEYWORDS;
    const TYPES: &'static [&'static str] = TYPES;
    async fn list_table_or_view(&self) -> anyhow::Result<Vec<String>> {
        let mut ret = AstCompletionHelper::list_table(self, None, None).await?;
        ret.extend(
            AstCompletionHelper::list_view(self, None, None)
                .await?
                .into_iter(),
        );
        anyhow::Ok(ret)
    }
    async fn list_function(&self) -> anyhow::Result<Vec<CompletionItem>> {
        AstCompletionHelper::list_function(self, None, None, true).await
    }
}

impl PostGresDB {
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
}

impl PostGresDB {
    async fn list_build_functions(&self) -> anyhow::Result<Vec<CompletionItem>> {
        let sql = r#"SELECT p.proname AS function_name,
             d.description AS function_description
            FROM pg_catalog.pg_proc p
            JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            LEFT JOIN pg_catalog.pg_description d ON d.objoid = p.oid
            WHERE n.nspname = 'pg_catalog'  -- 只查询内置函数
            ORDER BY p.proname;"#;
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            let desc: Option<String> = r.try_get(1)?;
            ret.push(CompletionItem {
                item: name.into(),
                kind: CompletionKind::Function,
                desc: desc.map(|x| x.into()),
            });
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]

impl AstCompletionHelper for PostGresDB {
    async fn list_table(
        &self, db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let mut sql =
            r#"select table_name FROM information_schema.tables  WHERE table_type = 'BASE TABLE' "#
                .to_string();
        if let Some(db) = self.db_merge_config(db) {
            sql.push_str(format!(" AND table_catalog = '{}'", db).as_str());
        }
        if let Some(schema) = self.schema_merge_config(schema) {
            sql.push_str(format!(" AND table_schema = '{}'", schema).as_str());
        }
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_procedure(
        &self, _db: Option<&str>, schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let sql = if let Some(schema) = self.schema_merge_config(schema) {
            format!(
                r#"
            SELECT 
                p.proname AS procedure_name
            FROM 
                pg_catalog.pg_proc p
            JOIN 
                pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            WHERE 
                n.nspname = '{}'  -- 指定目标模式
                AND p.prokind = 'p'  -- 仅选择存储过程
            ORDER BY 
                p.proname;
            "#,
                schema
            )
        } else {
            format!(
                r#"
            SELECT 
                p.proname AS procedure_name
            FROM 
                pg_catalog.pg_proc p
            JOIN 
                pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            WHERE 
                p.prokind = 'p'  -- 仅选择存储过程
            ORDER BY 
                p.proname;
            "#
            )
        };
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_view(
        &self,
        _db: Option<&str>, // can't filter on catelog 根据表结构
        schema: Option<&str>,
    ) -> anyhow::Result<Vec<String>> {
        let mut sql = "SELECT viewname FROM pg_views ".to_string();
        if let Some(schema) = self.schema_merge_config(schema) {
            sql.push_str(format!(" WHERE schemaname='{}'", schema).as_str());
        }
        // postgres has materilized view in other table
        sql.push_str(format!("UNION SELECT matviewname from pg_matviews").as_str());

        if let Some(schema) = self.schema_merge_config(schema) {
            sql.push_str(format!(" WHERE schemaname='{}'", schema).as_str());
        }

        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }

        anyhow::Ok(ret)
    }

    async fn list_column(
        &self, db: Option<&str>, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let mut sql = "SELECT column_name FROM information_schema.columns".to_string();
        let mut has_where = false;
        if let Some(db) = self.db_merge_config(db) {
            has_where = true;
            sql.push_str(format!(" WHERE table_catalog='{}'", db).as_str());
        }
        if let Some(schema) = self.schema_merge_config(schema) {
            if has_where {
                sql.push_str(" AND ");
            } else {
                sql.push_str(" WHERE ");
            }
            has_where = true;
            sql.push_str(format!("table_schema='{}'", schema).as_str());
        }
        if has_where {
            sql.push_str(" AND ");
        } else {
            sql.push_str(" WHERE ");
        }
        sql.push_str(format!("table_name='{}'", table).as_str());
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_function(
        &self,
        _db: Option<&str>, // how to filter on this???
        schema: Option<&str>,
        include_build_in: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let sql;
        if let Some(schema) = self.schema_merge_config(schema) {
            sql = format!(
                r#"
            SELECT  
                p.proname AS function_name
            FROM pg_catalog.pg_proc p
            JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            WHERE n.nspname = '{}'  -- 替换为你的模式名
            ORDER BY p.proname;
        "#,
                schema
            );
        } else {
            sql = format!(
                r#"
            SELECT 
                p.proname AS function_name
            FROM pg_catalog.pg_proc p
            JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            ORDER BY p.proname;
        "#,
            );
        }
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(CompletionItem {
                item: name.into(),
                desc: None,
                kind: CompletionKind::Function,
            });
        }
        if include_build_in {
            ret.extend(self.list_build_functions().await?.into_iter());
        }
        anyhow::Ok(ret)
    }

    async fn list_index(&self, schema: Option<&str>) -> anyhow::Result<Vec<String>> {
        let sql = if let Some(schema) = self.schema_merge_config(schema) {
            format!(
                r#"
            SELECT indexname AS index_name
                FROM pg_indexes
                WHERE schemaname = '{}';
                "#,
                schema
            )
        } else {
            r#"
            SELECT indexname AS index_name
                FROM pg_indexes
                "#
            .to_string()
        };
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_variable(&self) -> anyhow::Result<Vec<String>> {
        let sql = "show all";
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        CatalogTrait::list_database(self).await
    }

    async fn list_schema(&self, db: Option<&str>) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let db = self.db_merge_config(db);
        if db.is_none() {
            log::error!("list_schema using a empty db:{:?}", db);
        }
        let sql = format!(
            r#"
        SELECT schema_name 
            FROM information_schema.schemata
            WHERE schema_name NOT IN ('information_schema', 'pg_catalog' , 'pg_toast') and 
            schemata.catalog_name ='{}'
    "#,
            db.unwrap_or_default(),
        );
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }

    async fn postgres_table_policy(
        &self, _db: Option<&str>, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let sql = if let Some(schema) = self.schema_merge_config(schema) {
            format!(
                r#"SELECT policyname
                FROM pg_policies
                WHERE tablename = '{}' AND schemaname='{}';  -- 替换为你的表名"#,
                table, schema
            )
        } else {
            format!(
                r#"SELECT policyname
                FROM pg_policies
                WHERE tablename = '{}';  -- 替换为你的表名"#,
                table
            )
        };
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }

    async fn list_role(&self) -> anyhow::Result<Vec<String>> {
        let sql = r#"
        SELECT rolname AS role_name
            FROM pg_roles
        "#;
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }

    async fn list_postgres_rule(
        &self, _db: Option<&str>, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        // 这里schemaname实际上是db的名字 我观察出来的
        // 对吗？？？
        let sql = if let Some(schema) = self.db_merge_config(schema) {
            format!(
                r#"select rulename from pg_rules where tablename='{}' and schemaname='{}'"#,
                table, schema
            )
        } else {
            format!(
                r#"select rulename from pg_rules where tablename='{}'"#,
                table
            )
        };
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }

    async fn list_constraint(
        &self, db: Option<&str>, schema: Option<&str>, table_name: &str,
    ) -> anyhow::Result<Vec<String>> {
        let mut sql =
            "SELECT constraint_name FROM information_schema.table_constraints".to_string();
        let mut has_where = false;
        if let Some(db) = self.db_merge_config(db) {
            has_where = true;
            sql.push_str(format!(" WHERE table_catalog='{}'", db).as_str());
        }
        if let Some(schema) = self.schema_merge_config(schema) {
            if has_where {
                sql.push_str(" AND ");
            } else {
                sql.push_str(" WHERE ");
            }
            has_where = true;
            sql.push_str(format!("table_schema='{}'", schema).as_str());
        }
        if has_where {
            sql.push_str(" AND ");
        } else {
            sql.push_str(" WHERE ");
        }
        sql.push_str(format!(" table_name='{}'", table_name).as_str());
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }

    async fn list_prepare_name(&self, query_id: uuid::Uuid) -> anyhow::Result<Vec<String>> {
        let sql = r#"
        SELECT name
            FROM pg_prepared_statements;
        "#;
        let conn = match self.query_connection.get(&query_id).map(|x| x.clone()) {
            Some(x) => x,
            None => anyhow::bail!(format!("can't find query of {}", query_id)),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }

    async fn list_table_trigger(
        &self, _db: Option<&str>, schema: Option<&str>, table: &str,
    ) -> anyhow::Result<Vec<String>> {
        let sql = if let Some(schema) = self.schema_merge_config(schema) {
            format!(
                r#"
                SELECT tgname AS trigger_name
                    FROM pg_trigger
                    JOIN pg_class ON pg_trigger.tgrelid = pg_class.oid
                    JOIN pg_namespace ON pg_class.relnamespace = pg_namespace.oid
                    WHERE pg_class.relname = '{}'
                    AND pg_namespace.nspname = '{}'
            "#,
                table, schema,
            )
        } else {
            format!(
                r#"
                SELECT tgname AS trigger_name
                    FROM pg_trigger
                    JOIN pg_class ON pg_trigger.tgrelid = pg_class.oid
                    JOIN pg_namespace ON pg_class.relnamespace = pg_namespace.oid
                    WHERE pg_class.relname = '{}'
            "#,
                table,
            )
        };
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name)
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl LanguageServer for PostGresDB {
    async fn auto_completion(
        &self, query_id: uuid::Uuid, sql: &str, line: u32, col: u32, use_lexer: bool,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        let r = try_completion_on_syntax(
            &sql,
            query_id,
            line,
            col,
            sqlparser::dialect::PostgreSqlDialect {},
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
            log::warn!("disabled use lexer , we will return empty data.");
            return anyhow::Ok(vec![]);
        }
        return LexerCompletionTrait::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::PostgreSqlDialect {},
            sqlparser::tokenizer::Location {
                line: line as u64,
                column: col as u64,
            },
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::completion::completion_test::test_file_auto_completion;

    use crate::db::QuerySql;
    use crate::enable_test_log;
    use crate::pg::make_test_object;

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_auto_completion() {
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        let _ = db
            .query_sql(
                query_id,
                r#"
        -- 我们创建一下的表来做测试的

        CREATE TABLE departments (
            id SERIAL PRIMARY KEY,
            name VARCHAR(100) NOT NULL
        );
        CREATE TABLE employees (
            id SERIAL PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            department_id INTEGER REFERENCES departments(id)
        );
        CREATE OR REPLACE RULE do_nothing_rule AS
            ON INSERT TO employees
            DO INSTEAD
            NOTHING;
        CREATE OR REPLACE FUNCTION do_nothing()
        RETURNS TRIGGER AS $$
        BEGIN
            RETURN NULL;  -- 不执行任何操作
        END;
        $$ LANGUAGE plpgsql;

        CREATE TRIGGER do_nothing_trigger
        BEFORE INSERT OR UPDATE OR DELETE ON employees
        FOR EACH ROW
        EXECUTE FUNCTION do_nothing();
        create  view employees_test as select * from employees e;

        ALTER TABLE employees ENABLE ROW LEVEL SECURITY;

        CREATE POLICY no_op_policy
        ON employees
        USING (true)  -- 任何行都可以访问
        WITH CHECK (true);  -- 任何行都可以插入/更新
        "#,
            )
            .await;

        enable_test_log();
        let manifest_dir =
            std::env::var("CARGO_MANIFEST_DIR").expect("无法获取 CARGO_MANIFEST_DIR");
        let mut file_path = Path::new(manifest_dir.as_str()).to_path_buf();
        file_path.push("tests");
        file_path.push("pg_auto_completion_test.sql");
        let content = std::fs::read_to_string(file_path.as_path()).unwrap();

        test_file_auto_completion(&content, &db, uuid::Uuid::new_v4(), false).await;
    }
    #[ignore = "xxx"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_completion_xxx() {
        enable_test_log();
        let db = make_test_object().await;
        test_file_auto_completion(
            r#"DROP PROCEDURE procedure_name() 
30 test_procedure"#,
            &db,
            uuid::Uuid::new_v4(),
            false,
        )
        .await;
    }
}

pub(crate) const TYPES: &[&str] = &[
    "BOOL",
    "BYTEA",
    "NAME",
    "INT8",
    "INT2",
    "INT4",
    "TEXT",
    "OID",
    "JSON",
    "JSON[]",
    "POINT",
    "LSEG",
    "PATH",
    "BOX",
    "POLYGON",
    "LINE",
    "LINE[]",
    "CIDR",
    "CIDR[]",
    "FLOAT4",
    "FLOAT8",
    "UNKNOWN",
    "CIRCLE",
    "CIRCLE[]",
    "MACADDR8",
    "MACADDR8[]",
    "MACADDR",
    "INET",
    "BOOL[]",
    "BYTEA[]",
    "NAME[]",
    "INT2[]",
    "INT4[]",
    "TEXT[]",
    "CHAR[]",
    "VARCHAR[]",
    "INT8[]",
    "POINT[]",
    "LSEG[]",
    "PATH[]",
    "BOX[]",
    "FLOAT4[]",
    "FLOAT8[]",
    "POLYGON[]",
    "OID[]",
    "MACADDR[]",
    "INET[]",
    "CHAR",
    "VARCHAR",
    "DATE",
    "TIME",
    "TIMESTAMP",
    "TIMESTAMP[]",
    "DATE[]",
    "TIME[]",
    "TIMESTAMPTZ",
    "TIMESTAMPTZ[]",
    "INTERVAL",
    "INTERVAL[]",
    "NUMERIC[]",
    "TIMETZ",
    "TIMETZ[]",
    "BIT",
    "BIT[]",
    "VARBIT",
    "VARBIT[]",
    "NUMERIC",
    "RECORD",
    "RECORD[]",
    "UUID",
    "UUID[]",
    "JSONB",
    "JSONB[]",
    "INT4RANGE",
    "INT4RANGE[]",
    "NUMRANGE",
    "NUMRANGE[]",
    "TSRANGE",
    "TSRANGE[]",
    "TSTZRANGE",
    "TSTZRANGE[]",
    "DATERANGE",
    "DATERANGE[]",
    "INT8RANGE",
    "INT8RANGE[]",
    "JSONPATH",
    "JSONPATH[]",
    "MONEY",
    "MONEY[]",
    "VOID",
];

const PG_KEYWORDS: &[&str] = &[
    "ABORT",
    "ABS",
    "ABSENT",
    "ABSOLUTE",
    "ACCESS",
    "ACCORDING",
    "ACOS",
    "ACTION",
    "ADA",
    "ADD",
    "ADMIN",
    "AFTER",
    "AGGREGATE",
    "ALL",
    "ALLOCATE",
    "ALSO",
    "ALTER",
    "ALWAYS",
    "ANALYSE",
    "ANALYZE",
    "AND",
    "ANY",
    "ANY_VALUE",
    "ARE",
    "ARRAY",
    "ARRAY_AGG",
    "ARRAY_​MAX_​CARDINALITY",
    "AS",
    "ASC",
    "ASENSITIVE",
    "ASIN",
    "ASSERTION",
    "ASSIGNMENT",
    "ASYMMETRIC",
    "AT",
    "ATAN",
    "ATOMIC",
    "ATTACH",
    "ATTRIBUTE",
    "ATTRIBUTES",
    "AUTHORIZATION",
    "AVG",
    "BACKWARD",
    "BASE64",
    "BEFORE",
    "BEGIN",
    "BEGIN_FRAME",
    "BEGIN_PARTITION",
    "BERNOULLI",
    "BETWEEN",
    "BIGINT",
    "BINARY",
    "BIT",
    "BIT_LENGTH",
    "BLOB",
    "BLOCKED",
    "BOM",
    "BOOLEAN",
    "BOTH",
    "BREADTH",
    "BTRIM",
    "BY",
    "C",
    "CACHE",
    "CALL",
    "CALLED",
    "CARDINALITY",
    "CASCADE",
    "CASCADED",
    "CASE",
    "CAST",
    "CATALOG",
    "CATALOG_NAME",
    "CEIL",
    "CEILING",
    "CHAIN",
    "CHAINING",
    "CHAR",
    "CHARACTER",
    "CHARACTERISTICS",
    "CHARACTERS",
    "CHARACTER_LENGTH",
    "CHARACTER_​SET_​CATALOG",
    "CHARACTER_SET_NAME",
    "CHARACTER_SET_SCHEMA",
    "CHAR_LENGTH",
    "CHECK",
    "CHECKPOINT",
    "CLASS",
    "CLASSIFIER",
    "CLASS_ORIGIN",
    "CLOB",
    "CLOSE",
    "CLUSTER",
    "COALESCE",
    "COBOL",
    "COLLATE",
    "COLLATION",
    "COLLATION_CATALOG",
    "COLLATION_NAME",
    "COLLATION_SCHEMA",
    "COLLECT",
    "COLUMN",
    "COLUMNS",
    "COLUMN_NAME",
    "COMMAND_FUNCTION",
    "COMMAND_​FUNCTION_​CODE",
    "COMMENT",
    "COMMENTS",
    "COMMIT",
    "COMMITTED",
    "COMPRESSION",
    "CONCURRENTLY",
    "CONDITION",
    "CONDITIONAL",
    "CONDITION_NUMBER",
    "CONFIGURATION",
    "CONFLICT",
    "CONNECT",
    "CONNECTION",
    "CONNECTION_NAME",
    "CONSTRAINT",
    "CONSTRAINTS",
    "CONSTRAINT_CATALOG",
    "CONSTRAINT_NAME",
    "CONSTRAINT_SCHEMA",
    "CONSTRUCTOR",
    "CONTAINS",
    "CONTENT",
    "CONTINUE",
    "CONTROL",
    "CONVERSION",
    "CONVERT",
    "COPARTITION",
    "COPY",
    "CORR",
    "CORRESPONDING",
    "COS",
    "COSH",
    "COST",
    "COUNT",
    "COVAR_POP",
    "COVAR_SAMP",
    "CREATE",
    "CROSS",
    "CSV",
    "CUBE",
    "CUME_DIST",
    "CURRENT",
    "CURRENT_CATALOG",
    "CURRENT_DATE",
    "CURRENT_DEFAULT_TRANSFORM_GROUP",
    "CURRENT_PATH",
    "CURRENT_ROLE",
    "CURRENT_ROW",
    "CURRENT_SCHEMA",
    "CURRENT_TIME",
    "CURRENT_TIMESTAMP",
    "CURRENT_TRANSFORM_GROUP_FOR_TYPE",
    "CURRENT_USER",
    "CURSOR",
    "CURSOR_NAME",
    "CYCLE",
    "DATA",
    "DATABASE",
    "DATALINK",
    "DATE",
    "DATETIME_INTERVAL_CODE",
    "DATETIME_INTERVAL_PRECISION",
    "DAY",
    "DB",
    "DEALLOCATE",
    "DEC",
    "DECFLOAT",
    "DECIMAL",
    "DECLARE",
    "DEFAULT",
    "DEFAULTS",
    "DEFERRABLE",
    "DEFERRED",
    "DEFINE",
    "DEFINED",
    "DEFINER",
    "DEGREE",
    "DELETE",
    "DELIMITER",
    "DELIMITERS",
    "DENSE_RANK",
    "DEPENDS",
    "DEPTH",
    "DEREF",
    "DERIVED",
    "DESC",
    "DESCRIBE",
    "DESCRIPTOR",
    "DETACH",
    "DETERMINISTIC",
    "DIAGNOSTICS",
    "DICTIONARY",
    "DISABLE",
    "DISCARD",
    "DISCONNECT",
    "DISPATCH",
    "DISTINCT",
    "DLNEWCOPY",
    "DLPREVIOUSCOPY",
    "DLURLCOMPLETE",
    "DLURLCOMPLETEONLY",
    "DLURLCOMPLETEWRITE",
    "DLURLPATH",
    "DLURLPATHONLY",
    "DLURLPATHWRITE",
    "DLURLSCHEME",
    "DLURLSERVER",
    "DLVALUE",
    "DO",
    "DOCUMENT",
    "DOMAIN",
    "DOUBLE",
    "DROP",
    "DYNAMIC",
    "DYNAMIC_FUNCTION",
    "DYNAMIC_FUNCTION_CODE",
    "EACH",
    "ELEMENT",
    "ELSE",
    "EMPTY",
    "ENABLE",
    "ENCODING",
    "ENCRYPTED",
    "END",
    "END-EXEC",
    "END_FRAME",
    "END_PARTITION",
    "ENFORCED",
    "ENUM",
    "EQUALS",
    "ERROR",
    "ESCAPE",
    "EVENT",
    "EVERY",
    "EXCEPT",
    "EXCEPTION",
    "EXCLUDE",
    "EXCLUDING",
    "EXCLUSIVE",
    "EXEC",
    "EXECUTE",
    "EXISTS",
    "EXP",
    "EXPLAIN",
    "EXPRESSION",
    "EXTENSION",
    "EXTERNAL",
    "EXTRACT",
    "FALSE",
    "FAMILY",
    "FETCH",
    "FILE",
    "FILTER",
    "FINAL",
    "FINALIZE",
    "FINISH",
    "FIRST",
    "FIRST_VALUE",
    "FLAG",
    "FLOAT",
    "FLOOR",
    "FOLLOWING",
    "FOR",
    "FORCE",
    "FOREIGN",
    "FORMAT",
    "FORTRAN",
    "FORWARD",
    "FOUND",
    "FRAME_ROW",
    "FREE",
    "FREEZE",
    "FROM",
    "FS",
    "FULFILL",
    "FULL",
    "FUNCTION",
    "FUNCTIONS",
    "FUSION",
    "G",
    "GENERAL",
    "GENERATED",
    "GET",
    "GLOBAL",
    "GO",
    "GOTO",
    "GRANT",
    "GRANTED",
    "GREATEST",
    "GROUP",
    "GROUPING",
    "GROUPS",
    "HANDLER",
    "HAVING",
    "HEADER",
    "HEX",
    "HIERARCHY",
    "HOLD",
    "HOUR",
    "ID",
    "IDENTITY",
    "IF",
    "IGNORE",
    "ILIKE",
    "IMMEDIATE",
    "IMMEDIATELY",
    "IMMUTABLE",
    "IMPLEMENTATION",
    "IMPLICIT",
    "IMPORT",
    "IN",
    "INCLUDE",
    "INCLUDING",
    "INCREMENT",
    "INDENT",
    "INDEX",
    "INDEXES",
    "INDICATOR",
    "INHERIT",
    "INHERITS",
    "INITIAL",
    "INITIALLY",
    "INLINE",
    "INNER",
    "INOUT",
    "INPUT",
    "INSENSITIVE",
    "INSERT",
    "INSTANCE",
    "INSTANTIABLE",
    "INSTEAD",
    "INT",
    "INTEGER",
    "INTEGRITY",
    "INTERSECT",
    "INTERSECTION",
    "INTERVAL",
    "INTO",
    "INVOKER",
    "IS",
    "ISNULL",
    "ISOLATION",
    "JOIN",
    "JSON",
    "JSON_ARRAY",
    "JSON_ARRAYAGG",
    "JSON_EXISTS",
    "JSON_OBJECT",
    "JSON_OBJECTAGG",
    "JSON_QUERY",
    "JSON_SCALAR",
    "JSON_SERIALIZE",
    "JSON_TABLE",
    "JSON_TABLE_PRIMITIVE",
    "JSON_VALUE",
    "K",
    "KEEP",
    "KEY",
    "KEYS",
    "KEY_MEMBER",
    "KEY_TYPE",
    "LABEL",
    "LAG",
    "LANGUAGE",
    "LARGE",
    "LAST",
    "LAST_VALUE",
    "LATERAL",
    "LEAD",
    "LEADING",
    "LEAKPROOF",
    "LEAST",
    "LEFT",
    "LENGTH",
    "LEVEL",
    "LIBRARY",
    "LIKE",
    "LIKE_REGEX",
    "LIMIT",
    "LINK",
    "LISTAGG",
    "LISTEN",
    "LN",
    "LOAD",
    "LOCAL",
    "LOCALTIME",
    "LOCALTIMESTAMP",
    "LOCATION",
    "LOCATOR",
    "LOCK",
    "LOCKED",
    "LOG",
    "LOG10",
    "LOGGED",
    "LOWER",
    "LPAD",
    "LTRIM",
    "M",
    "MAP",
    "MAPPING",
    "MATCH",
    "MATCHED",
    "MATCHES",
    "MATCH_NUMBER",
    "MATCH_RECOGNIZE",
    "MATERIALIZED",
    "MAX",
    "MAXVALUE",
    "MEASURES",
    "MEMBER",
    "MERGE",
    "MESSAGE_LENGTH",
    "MESSAGE_OCTET_LENGTH",
    "MESSAGE_TEXT",
    "METHOD",
    "MIN",
    "MINUTE",
    "MINVALUE",
    "MOD",
    "MODE",
    "MODIFIES",
    "MODULE",
    "MONTH",
    "MORE",
    "MOVE",
    "MULTISET",
    "MUMPS",
    "NAME",
    "NAMES",
    "NAMESPACE",
    "NATIONAL",
    "NATURAL",
    "NCHAR",
    "NCLOB",
    "NESTED",
    "NESTING",
    "NEW",
    "NEXT",
    "NFC",
    "NFD",
    "NFKC",
    "NFKD",
    "NIL",
    "NO",
    "NONE",
    "NORMALIZE",
    "NORMALIZED",
    "NOT",
    "NOTHING",
    "NOTIFY",
    "NOTNULL",
    "NOWAIT",
    "NTH_VALUE",
    "NTILE",
    "NULL",
    "NULLABLE",
    "NULLIF",
    "NULLS",
    "NULL_ORDERING",
    "NUMBER",
    "NUMERIC",
    "OBJECT",
    "OCCURRENCE",
    "OCCURRENCES_REGEX",
    "OCTETS",
    "OCTET_LENGTH",
    "OF",
    "OFF",
    "OFFSET",
    "OIDS",
    "OLD",
    "OMIT",
    "ON",
    "ONE",
    "ONLY",
    "OPEN",
    "OPERATOR",
    "OPTION",
    "OPTIONS",
    "OR",
    "ORDER",
    "ORDERING",
    "ORDINALITY",
    "OTHERS",
    "OUT",
    "OUTER",
    "OUTPUT",
    "OVER",
    "OVERFLOW",
    "OVERLAPS",
    "OVERLAY",
    "OVERRIDING",
    "OWNED",
    "OWNER",
    "P",
    "PAD",
    "PARALLEL",
    "PARAMETER",
    "PARAMETER_MODE",
    "PARAMETER_NAME",
    "PARAMETER_​ORDINAL_​POSITION",
    "PARAMETER_​SPECIFIC_​CATALOG",
    "PARAMETER_​SPECIFIC_​NAME",
    "PARAMETER_​SPECIFIC_​SCHEMA",
    "PARSER",
    "PARTIAL",
    "PARTITION",
    "PASCAL",
    "PASS",
    "PASSING",
    "PASSTHROUGH",
    "PASSWORD",
    "PAST",
    "PATH",
    "PATTERN",
    "PER",
    "PERCENT",
    "PERCENTILE_CONT",
    "PERCENTILE_DISC",
    "PERCENT_RANK",
    "PERIOD",
    "PERMISSION",
    "PERMUTE",
    "PIPE",
    "PLACING",
    "PLAN",
    "PLANS",
    "PLI",
    "POLICY",
    "PORTION",
    "POSITION",
    "POSITION_REGEX",
    "POWER",
    "PRECEDES",
    "PRECEDING",
    "PRECISION",
    "PREPARE",
    "PREPARED",
    "PRESERVE",
    "PREV",
    "PRIMARY",
    "PRIOR",
    "PRIVATE",
    "PRIVILEGES",
    "PROCEDURAL",
    "PROCEDURE",
    "PROCEDURES",
    "PROGRAM",
    "PRUNE",
    "PTF",
    "PUBLIC",
    "PUBLICATION",
    "QUOTE",
    "QUOTES",
    "RANGE",
    "RANK",
    "READ",
    "READS",
    "REAL",
    "REASSIGN",
    "RECHECK",
    "RECOVERY",
    "RECURSIVE",
    "REF",
    "REFERENCES",
    "REFERENCING",
    "REFRESH",
    "REGR_AVGX",
    "REGR_AVGY",
    "REGR_COUNT",
    "REGR_INTERCEPT",
    "REGR_R2",
    "REGR_SLOPE",
    "REGR_SXX",
    "REGR_SXY",
    "REGR_SYY",
    "REINDEX",
    "RELATIVE",
    "RELEASE",
    "RENAME",
    "REPEATABLE",
    "REPLACE",
    "REPLICA",
    "REQUIRING",
    "RESET",
    "RESPECT",
    "RESTART",
    "RESTORE",
    "RESTRICT",
    "RESULT",
    "RETURN",
    "RETURNED_CARDINALITY",
    "RETURNED_LENGTH",
    "RETURNED_OCTET_LENGTH",
    "RETURNED_SQLSTATE",
    "RETURNING",
    "RETURNS",
    "REVOKE",
    "RIGHT",
    "ROLE",
    "ROLLBACK",
    "ROLLUP",
    "ROUTINE",
    "ROUTINES",
    "ROUTINE_CATALOG",
    "ROUTINE_NAME",
    "ROUTINE_SCHEMA",
    "ROW",
    "ROWS",
    "ROW_COUNT",
    "ROW_NUMBER",
    "RPAD",
    "RTRIM",
    "RULE",
    "RUNNING",
    "SAVEPOINT",
    "SCALAR",
    "SCALE",
    "SCHEMA",
    "SCHEMAS",
    "SCHEMA_NAME",
    "SCOPE",
    "SCOPE_CATALOG",
    "SCOPE_NAME",
    "SCOPE_SCHEMA",
    "SCROLL",
    "SEARCH",
    "SECOND",
    "SECTION",
    "SECURITY",
    "SEEK",
    "SELECT",
    "SELECTIVE",
    "SELF",
    "SEMANTICS",
    "SENSITIVE",
    "SEQUENCE",
    "SEQUENCES",
    "SERIALIZABLE",
    "SERVER",
    "SERVER_NAME",
    "SESSION",
    "SESSION_USER",
    "SET",
    "SETOF",
    "SETS",
    "SHARE",
    "SHOW",
    "SIMILAR",
    "SIMPLE",
    "SIN",
    "SINH",
    "SIZE",
    "SKIP",
    "SMALLINT",
    "SNAPSHOT",
    "SOME",
    "SORT_DIRECTION",
    "SOURCE",
    "SPACE",
    "SPECIFIC",
    "SPECIFICTYPE",
    "SPECIFIC_NAME",
    "SQL",
    "SQLCODE",
    "SQLERROR",
    "SQLEXCEPTION",
    "SQLSTATE",
    "SQLWARNING",
    "SQRT",
    "STABLE",
    "STANDALONE",
    "START",
    "STATE",
    "STATEMENT",
    "STATIC",
    "STATISTICS",
    "STDDEV_POP",
    "STDDEV_SAMP",
    "STDIN",
    "STDOUT",
    "STORAGE",
    "STORED",
    "STRICT",
    "STRING",
    "STRIP",
    "STRUCTURE",
    "STYLE",
    "SUBCLASS_ORIGIN",
    "SUBMULTISET",
    "SUBSCRIPTION",
    "SUBSET",
    "SUBSTRING",
    "SUBSTRING_REGEX",
    "SUCCEEDS",
    "SUM",
    "SUPPORT",
    "SYMMETRIC",
    "SYSID",
    "SYSTEM",
    "SYSTEM_TIME",
    "SYSTEM_USER",
    "T",
    "TABLE",
    "TABLES",
    "TABLESAMPLE",
    "TABLESPACE",
    "TABLE_NAME",
    "TAN",
    "TANH",
    "TEMP",
    "TEMPLATE",
    "TEMPORARY",
    "TEXT",
    "THEN",
    "THROUGH",
    "TIES",
    "TIME",
    "TIMESTAMP",
    "TIMEZONE_HOUR",
    "TIMEZONE_MINUTE",
    "TO",
    "TOKEN",
    "TOP_LEVEL_COUNT",
    "TRAILING",
    "TRANSACTION",
    "TRANSACTIONS_​COMMITTED",
    "TRANSACTIONS_​ROLLED_​BACK",
    "TRANSACTION_ACTIVE",
    "TRANSFORM",
    "TRANSFORMS",
    "TRANSLATE",
    "TRANSLATE_REGEX",
    "TRANSLATION",
    "TREAT",
    "TRIGGER",
    "TRIGGER_CATALOG",
    "TRIGGER_NAME",
    "TRIGGER_SCHEMA",
    "TRIM",
    "TRIM_ARRAY",
    "TRUE",
    "TRUNCATE",
    "TRUSTED",
    "TYPE",
    "TYPES",
    "UESCAPE",
    "UNBOUNDED",
    "UNCOMMITTED",
    "UNCONDITIONAL",
    "UNDER",
    "UNENCRYPTED",
    "UNION",
    "UNIQUE",
    "UNKNOWN",
    "UNLINK",
    "UNLISTEN",
    "UNLOGGED",
    "UNMATCHED",
    "UNNAMED",
    "UNNEST",
    "UNTIL",
    "UNTYPED",
    "UPDATE",
    "UPPER",
    "URI",
    "USAGE",
    "USER",
    "USER_​DEFINED_​TYPE_​CATALOG",
    "USER_​DEFINED_​TYPE_​CODE",
    "USER_​DEFINED_​TYPE_​NAME",
    "USER_​DEFINED_​TYPE_​SCHEMA",
    "USING",
    "UTF16",
    "UTF32",
    "UTF8",
    "VACUUM",
    "VALID",
    "VALIDATE",
    "VALIDATOR",
    "VALUE",
    "VALUES",
    "VALUE_OF",
    "VARBINARY",
    "VARCHAR",
    "VARIADIC",
    "VARYING",
    "VAR_POP",
    "VAR_SAMP",
    "VERBOSE",
    "VERSION",
    "VERSIONING",
    "VIEW",
    "VIEWS",
    "VOLATILE",
    "WHEN",
    "WHENEVER",
    "WHERE",
    "WHITESPACE",
    "WIDTH_BUCKET",
    "WINDOW",
    "WITH",
    "WITHIN",
    "WITHOUT",
    "WORK",
    "WRAPPER",
    "WRITE",
    "XML",
    "XMLAGG",
    "XMLATTRIBUTES",
    "XMLBINARY",
    "XMLCAST",
    "XMLCOMMENT",
    "XMLCONCAT",
    "XMLDECLARATION",
    "XMLDOCUMENT",
    "XMLELEMENT",
    "XMLEXISTS",
    "XMLFOREST",
    "XMLITERATE",
    "XMLNAMESPACES",
    "XMLPARSE",
    "XMLPI",
    "XMLQUERY",
    "XMLROOT",
    "XMLSCHEMA",
    "XMLSERIALIZE",
    "XMLTABLE",
    "XMLTEXT",
    "XMLVALIDATE",
    "YEAR",
    "YES",
    "ZONE",
];
