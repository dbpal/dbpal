use dashmap::DashMap;
use futures_util::StreamExt;
use sqlx::pool::{PoolConnection, PoolOptions};
use sqlx::postgres::PgRow;
use sqlx::{AssertSqlSafe, Column, Executor, PgConnection, Postgres, Row, SqlSafeStr};

use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

use crate::datasource_config::{PGDBConfig, SslFilesWithStrModel};
use crate::db::{
    create_html_table_data, create_html_table_header, create_markdown_data, create_markdown_header,
    export_query_as_xlsx_send_data, AutoCompletionQueryId, BackendKind, BackendKindPostGres,
    CatalogTrait, CheckConn, ColumnType, CreateDatabaseExtra, CreateIndexReq, CreateSchemaExtra,
    CreateTableAndIndexSql, CreatedTypeDef, DBTrait, DefAndMetas, ExportFrame, ExportKind,
    ExportQueryResult, ExtractSqlValeFromJsonValue, Field, FirstTrue, ForeignKeyDef,
    GetBackendKind, GetTestExportTableKinds, Index, IndexKind, ListTable, MetaValue, NamedStaff,
    OptionMustBeSomeErr, ProcedureDetail, QueryEvent, QueryEventEmitter, QueryResp, QuerySql,
    SchemaTrait, SequenceDetail, ServerMetas, ShowTableParameter, SqlBuilder, SqlCollect,
    TriggerDetail, TriggerEvent, TriggerTiming, XlsxEdit, POOL_MAX_CONNECTIONS,
};

use crate::helper::create_csv;
use crate::language_server::{
    get_update_sql_from_result_set_impl, LangServerCommon, LexerCompletionTrait,
};
use crate::pg_format::*;

/// postgres version of pg.
#[derive(Clone)]
pub struct PostGresDB {
    pool: sqlx::PgPool,
    pub(crate) config: PGDBConfig,
    pub(crate) query_connection: Arc<DashMap<String, Arc<Mutex<sqlx::PgConnection>>>>,
    pub(crate) completion_id: AutoCompletionQueryId,
    pub(crate) kind: BackendKindPostGres,
}

#[async_trait::async_trait]
impl CheckConn for PostGresDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        let mut conn = self.get_conn().await?;
        sqlx::raw_sql("select 1")
            .fetch_one(conn.deref_mut())
            .await?;
        anyhow::Ok(ins.elapsed())
    }
}

impl PostGresDB {
    async fn infer_fields_by_sql(&self, sql: &str) -> anyhow::Result<Vec<Field>> {
        use sqlx::Statement;
        let mut conn = self.get_conn().await?;
        let r = conn
            .deref_mut()
            .prepare_with(AssertSqlSafe(sql).into_sql_str(), &[])
            .await
            .map(|s| {
                s.columns()
                    .iter()
                    .map(|x| Field::from_name_type(x.name(), x.type_info().to_string()))
                    .collect::<Vec<_>>()
            })?;

        anyhow::Ok(r)
    }
}

#[async_trait::async_trait]
impl DBTrait for PostGresDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let sql = format!(
            r#"
        SELECT schema_name 
            FROM information_schema.schemata
            WHERE 
            schemata.catalog_name ='{}' order by schema_name"#,
            self.config.db_str()
        );
        log::info!("sql:{}", sql);
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            if self.kind == BackendKindPostGres::CockRoach && name == "crdb_internal" {
                continue;
            }
            ret.push(name)
        }
        anyhow::Ok(ret)
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut x = Clone::clone(self);
        x.config.schema = Some(name.to_owned());
        x.query_connection = Default::default();
        Ok(Arc::new(x))
    }

    async fn create_schema(
        &self,
        name: &str,
        _extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"CREATE SCHEMA "{name}";"#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP SCHEMA "{name}"   CASCADE ;"#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(())
    }
}

impl PostGresDB {
    async fn export_table_as_sql(
        &self,
        table_name: &str,
        para: Option<ShowTableParameter>,
        include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            anyhow::bail!("export create table sql not supported now.")
        }
        let mut conn = self.get_conn().await?;
        let cols = SchemaTrait::show_table_columns(self, table_name).await?;
        let sql = self.gen_show_sql(table_name, Some(&cols), para.as_ref());
        log::info!("sql:{}", sql);
        let mut stream = sqlx::query(AssertSqlSafe(sql.as_str())).fetch(conn.deref_mut());
        while let Some(row) = stream.next().await {
            let row = row?;
            let data = encode_row(&row)?;
            let mut m = HashMap::new();
            for (d, f) in data.into_iter().zip(cols.iter()) {
                m.insert(f.field.clone(), d);
            }
            let insert_sql = self.insert_sql(table_name, &cols, &m)?;
            sender.send(insert_sql.into()).await?;
            sender.send(";\n".into()).await?;
        }
        anyhow::Ok(())
    }
}

/// AI help me impl this.
// 当然可以,下面是一个 Rust 函数,用于解析 PostgreSQL 触发器的 tgtype 值:
//
// rust
// 复制
// fn parse_trigger_type(tgtype: i32) -> String {
// let mut result = String::new();
//
// if (tgtype & 0b1) == 0b1 {
// result.push_str("INSERT AFTER, ");
// } else if (tgtype & 0b10) == 0b10 {
// result.push_str("INSERT BEFORE, ");
// }
//
// if (tgtype & 0b100) == 0b100 {
// result.push_str("DELETE AFTER, ");
// } else if (tgtype & 0b1000) == 0b1000 {
// result.push_str("DELETE BEFORE, ");
// }
//
// if (tgtype & 0b10000) == 0b10000 {
// result.push_str("UPDATE AFTER, ");
// } else if (tgtype & 0b100000) == 0b100000 {
// result.push_str("UPDATE BEFORE, ");
// }
//
// if result.is_empty() {
// "UNKNOWN".to_string()
// } else {
// result.trim_end_matches(", ").to_string()
// }
// }
// 这个函数接受一个 i32 类型的 tgtype
// 值,并返回一个描述触发器类型的字符串。它检查 tgtype 的每个位,并根据触发器的
// timing 和 event 类型构建结果字符串。
//
// 如果没有找到任何匹配的位模式,它将返回 "UNKNOWN"
// 字符串。否则,它将返回一个以逗号和空格分隔的触发器类型列表。
//
// 您可以像这样使用这个函数:
fn parse_trigger_type(tgtype: i16) -> anyhow::Result<(TriggerEvent, TriggerTiming)> {
    if (tgtype & 0b1) == 0b1 {
        return anyhow::Ok((TriggerEvent::Insert, TriggerTiming::After));
    } else if (tgtype & 0b10) == 0b10 {
        return anyhow::Ok((TriggerEvent::Insert, TriggerTiming::Before));
    }

    if (tgtype & 0b100) == 0b100 {
        return anyhow::Ok((TriggerEvent::Delete, TriggerTiming::After));
    } else if (tgtype & 0b1000) == 0b1000 {
        return anyhow::Ok((TriggerEvent::Delete, TriggerTiming::Before));
    }

    if (tgtype & 0b10000) == 0b10000 {
        return anyhow::Ok((TriggerEvent::Update, TriggerTiming::After));
    } else if (tgtype & 0b100000) == 0b100000 {
        return anyhow::Ok((TriggerEvent::Update, TriggerTiming::Before));
    }
    anyhow::bail!(format!("unknown tgtype:{}", tgtype))
}

#[async_trait::async_trait]

impl ServerMetas for PostGresDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let mut conn = self.get_conn().await?;
        let mut ret = Vec::new();
        {
            let row = sqlx::raw_sql("SELECT version()")
                .fetch_one(conn.deref_mut())
                .await?;
            let version: String = row.try_get(0)?;
            ret.push(MetaValue::new("server version", version));
        }
        {
            let row = sqlx::raw_sql("SELECT current_user;")
                .fetch_one(conn.deref_mut())
                .await?;
            let version: String = row.try_get(0)?;
            ret.push(MetaValue::new("current user", version));
        }
        anyhow::Ok(ret)
    }
}

impl PostGresDB {
    pub(crate) async fn list_sequences_impl(
        &self,
        schema: &str,
    ) -> anyhow::Result<Vec<SequenceDetail>> {
        let sql = format!(
            r#"
            SELECT  
            sequence_name , data_type     FROM information_schema.sequences 
            where sequence_catalog = '{}' and sequence_schema ='{}' 
        "#,
            self.config.db_str(),
            schema
        );
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            let data_type: String = row.try_get(1)?;

            let create_sql = {
                let sql = format!(
                    r#"

                SELECT format(
                    'CREATE SEQUENCE %I.%I' || chr(10) ||
                    '    START WITH %s' || chr(10) ||
                    '    INCREMENT BY %s' || chr(10) ||
                    '    MINVALUE %s' || chr(10) ||
                    '    MAXVALUE %s' || chr(10) ||
                    '    CACHE %s%s;',
                    schemaname, sequencename,
                    start_value, increment_by,
                    min_value, max_value,
                    cache_size,
                    CASE WHEN cycle THEN chr(10) || '    CYCLE' ELSE '' END
                ) AS create_sql
                FROM pg_sequences
                WHERE schemaname = '{}'
                AND sequencename = '{}';


            "#,
                    schema, name
                );

                let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                    .fetch_one(conn.deref_mut())
                    .await?;

                let create_sql: String = row.try_get(0)?;
                create_sql
            };

            let (start_value, min_value, max_value, increment_by, last_value) = {
                let sql = format!(
                    r#"
                    select start_value, min_value , max_value , increment_by ,  last_value 
                    FROM pg_sequences where schemaname = '{}'
                "#,
                    schema
                );
                let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                    .fetch_one(conn.deref_mut())
                    .await?;
                let start_value: i64 = row.try_get(0)?;
                let min_value: i64 = row.try_get(1)?;
                let max_value: i64 = row.try_get(2)?;
                let increment_by: i64 = row.try_get(3)?;
                let last_value: Option<i64> = row.try_get(4)?;

                (start_value, min_value, max_value, increment_by, last_value)
            };
            ret.push(SequenceDetail {
                start_value: start_value.to_string(),
                min_value: min_value.to_string(),
                max_value: max_value.to_string(),
                increment_by: increment_by.to_string(),
                last_value: last_value.map(|x| x.to_string()),
                name,
                sql: create_sql,
                data_type,
            });
        }
        anyhow::Ok(ret)
    }
}
#[async_trait::async_trait]
impl SchemaTrait for PostGresDB {
    async fn drop_foreign_key(&self, table: &str, name: &str) -> anyhow::Result<()> {
        // 1. 删除外键约束 (Postgres 使用 DROP CONSTRAINT，且不需要删除索引)
        let drop_sql = format!(r#"ALTER TABLE "{}" DROP CONSTRAINT "{}""#, table, name);
        self.simple_execute_sql(&drop_sql).await?;
        anyhow::Ok(())
    }

    async fn list_foreign_key(&self, table: &str) -> anyhow::Result<Vec<ForeignKeyDef>> {
        let mut conn = self.get_conn().await?;

        // PostgreSQL 查询外键：必须使用 pg_catalog 系统表
        let sql = format!(
            r#"
        SELECT
            con.conname AS constraint_name,
            -- 获取被引用的父表名 (排除 schema 前缀)
            rel_ns.nspname || '.' || rel.relname AS referenced_table_name,
            -- 获取子表上的外键列名
            a.attname AS column_name,
            -- 获取父表上被引用的列名
            af.attname AS referenced_column_name,
            -- 处理更新和删除规则 (c = CASCADE, n = SET NULL, r = RESTRICT, a = NO ACTION)
            CASE confupdtype WHEN 'c' THEN 'CASCADE' WHEN 'n' THEN 'SET NULL' WHEN 'r' THEN 'RESTRICT' WHEN 'a' THEN 'NO ACTION' ELSE 'NO ACTION' END AS update_rule,
            CASE confdeltype WHEN 'c' THEN 'CASCADE' WHEN 'n' THEN 'SET NULL' WHEN 'r' THEN 'RESTRICT' WHEN 'a' THEN 'NO ACTION' ELSE 'NO ACTION' END AS delete_rule
        FROM
            pg_constraint con
        JOIN
            pg_class rel ON rel.oid = con.confrelid
        JOIN
            pg_namespace rel_ns ON rel_ns.oid = rel.relnamespace
        JOIN
            pg_attribute a ON a.attrelid = con.conrelid AND a.attnum = ANY(con.conkey)
        JOIN
            pg_attribute af ON af.attrelid = con.confrelid AND af.attnum = ANY(con.confkey)
        WHERE
            con.contype = 'f'
            AND con.conrelid = '{}'::regclass
        "#,
            table
        );

        let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;

        log::info!("sql:{}", sql);

        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            // 提取外键的名字 (第0列)
            let constraint_name: Option<String> = r.try_get(0)?;
            // 提取引用的目标表 (第1列)
            let referenced_table: String = r.try_get(1)?;
            // 提取本地列名 (第2列) -> 对应 SQLite 的 "from"
            let from: String = r.try_get(2)?;
            // 提取被引用的列名 (第3列) -> 对应 SQLite 的 "to"
            let to: String = r.try_get(3)?;
            // 更新规则 (第4列)
            let on_update: String = r.try_get(4)?;
            // 删除规则 (第5列)
            let on_delete: String = r.try_get(5)?;

            ret.push(ForeignKeyDef {
                name: constraint_name,
                table: referenced_table,
                from,
                to,
                on_update,
                on_delete,
            });
        }
        anyhow::Ok(ret)
    }

    fn list_sql_current_values(&self) -> &'static [&'static str] {
        <Self as SqlBuilder>::get_current_values(self)
    }
    async fn list_sequence(&self) -> anyhow::Result<Vec<SequenceDetail>> {
        self.list_sequences_impl(self.config.schema_str()).await
    }

    async fn rename_sequence(&self, old: &str, new: &str) -> anyhow::Result<String> {
        let sql = format!(r#"ALTER SEQUENCE "{old}" RENAME TO "{new}";"#);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn drop_sequence(&self, name: &str, cascade: bool) -> anyhow::Result<String> {
        let sql = format!(
            r#"DROP SEQUENCE IF EXISTS "{name}" {};"#,
            if cascade { "CASCADE" } else { "" }
        );
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn list_created_types(&self) -> anyhow::Result<Vec<CreatedTypeDef>> {
        self.list_created_types_impl(self.config.schema_str(), true)
            .await
    }
    async fn drop_created_type(&self, name: &str) -> anyhow::Result<String> {
        let sql = format!(r#"DROP TYPE  "{}"."{name}" "#, self.config.schema_str());
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn create_index(&self, table: &str, req: &CreateIndexReq) -> anyhow::Result<String> {
        let sql = req.common_sql(table, true, self.backend_kind().quote_style())?;
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<Option<String>> {
        let sql = format!("DROP INDEX \"{index}\";");
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn rename_index(
        &self,
        _table: &str,
        old_name: &str,
        new_name: &str,
    ) -> anyhow::Result<Option<String>> {
        let sql = format!("ALTER INDEX \"{old_name}\" RENAME TO \"{new_name}\";");
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn add_table_column(
        &self,
        table: &str,
        name: &str,
        col: ColumnType,
    ) -> anyhow::Result<String> {
        let sql = format!(
            "ALTER TABLE {table} ADD COLUMN \"{name}\" {} {} {}",
            col.typ,
            col.nullable.unwrap_or_default(),
            if let Some(dv) = col.default_value.as_ref() {
                format!(
                    "DEFAULT {}",
                    if self.hit_current_value(dv) {
                        dv.clone()
                    } else {
                        self.put_value_in_sql(&col.typ, dv.as_str())?
                    }
                )
            } else {
                "".to_owned()
            }
        );
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        LexerCompletionTrait::get_types(self).await
    }

    async fn modify_table_column(
        &self,
        table: &str,
        old_name: &str,
        new_name: &str,
        typ: Option<ColumnType>,
    ) -> anyhow::Result<String> {
        let mut sc = SqlCollect::default();
        if old_name != new_name {
            let sql =
                format!(r#"ALTER TABLE "{table}" RENAME COLUMN "{old_name}" TO "{new_name}""#);

            self.simple_execute_sql(&sql).await?;
            log::info!("sql:{}", sql);
            sc.collect(sql);
        }
        if let Some(typ) = typ.as_ref() {
            let sql = format!(
                r#"ALTER TABLE {table} ALTER COLUMN "{new_name}" SET DATA TYPE {}"#,
                typ.typ
            );
            log::info!("sql:{}", sql);
            self.simple_execute_sql(&sql).await?;
            sc.collect(sql);
            if let Some(null) = typ.nullable.as_ref() {
                let sql = match null {
                    crate::db::SqlNullAble::Unspecified => None,
                    crate::db::SqlNullAble::Null => {
                        let sql = format!(
                            r#"ALTER TABLE  "{table}" ALTER COLUMN "{new_name}" DROP NOT NULL"#
                        );
                        Some(sql)
                    }
                    crate::db::SqlNullAble::NotNull => {
                        let sql = format!(
                            r#"ALTER TABLE "{table}" ALTER COLUMN  "{new_name}" SET NOT NULL"#
                        );
                        Some(sql)
                    }
                };
                if let Some(sql) = sql {
                    self.simple_execute_sql(&sql).await?;
                    sc.collect(sql);
                }
            }
            if let Some(dv) = typ.default_value.as_ref() {
                let sql = format!(
                    r#"ALTER TABLE "{table}" ALTER COLUMN "{new_name}" SET DEFAULT {}"#,
                    if self.hit_current_value(dv) {
                        dv.clone()
                    } else {
                        self.put_value_in_sql(&typ.typ, dv.as_str())?
                    }
                );
                self.simple_execute_sql(&sql).await?;
                sc.collect(sql);
            }
        }
        anyhow::Ok(sc.into())
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<String> {
        let sql = format!(r#"ALTER TABLE "{table}" DROP COLUMN "{col}""#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        use sqlx::{Executor, Statement};
        let mut conn = self.get_conn().await?;
        let sql = format!(r#"select * from "{view_name}" limit 1"#);
        log::info!("sql:{}", sql);
        let s = conn
            .deref_mut()
            .prepare_with(AssertSqlSafe(sql.as_str()).into_sql_str(), &[])
            .await?;
        let ret = s
            .columns()
            .iter()
            .map(|x| Field::from_name_type(x.name(), x.type_info().to_string()))
            .collect::<Vec<_>>();
        anyhow::Ok(ret)
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
        SELECT  
        p.proname AS procedure_name,
        pg_catalog.pg_get_function_arguments(p.oid) AS arguments
        FROM pg_catalog.pg_proc p
            JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
        WHERE n.nspname = '{}'  -- 替换为目标模式名称
        AND p.prokind = 'p'  -- 仅选择存储过程
        ORDER BY p.proname;"#,
            self.config.schema_str()
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            let p: String = r.try_get(1)?;
            ret.push(NamedStaff {
                name,
                parameters: Some(p),
                metas: vec![],
                cassandra_parameters: None,
            });
        }

        anyhow::Ok(ret)
    }

    async fn show_procedure(&self, name: &NamedStaff) -> anyhow::Result<ProcedureDetail> {
        let sql = format!(
            r#"
                SELECT 
                    pg_catalog.pg_get_function_result(p.oid) AS return_type,
                    pg_catalog.pg_get_function_arguments(p.oid) AS arguments,
                    pg_get_functiondef(p.oid) AS source_code
                FROM pg_catalog.pg_proc p
                    JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
                WHERE n.nspname = '{}'  -- 指定目标模式
                AND p.proname = '{}'  -- 指定存储过程名称
                AND p.prokind = 'p'
                AND pg_catalog.pg_get_function_arguments(p.oid) = '{}'  -- 指定参数类型"#,
            self.config.schema_str(),
            name.name,
            name.parameters.as_deref().unwrap_or_default(),
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_one(conn.deref_mut())
            .await?;
        let return_type: Option<String> = row.try_get(0)?;
        let parameters: String = row.try_get(1)?;
        let code: String = row.try_get(2)?;

        anyhow::Ok(ProcedureDetail {
            def: code,
            metas: vec![
                MetaValue::new("return_type", return_type),
                MetaValue::new("parameters", parameters),
            ],
        })
    }

    async fn drop_procedure(&self, name: &NamedStaff) -> anyhow::Result<String> {
        let sql = format!(
            "DROP PROCEDURE {}({})",
            name.name,
            name.parameters.as_deref().unwrap_or_default()
        );
        log::error!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
            SELECT  
                p.proname AS function_name,
                pg_catalog.pg_get_function_result(p.oid) AS return_type,
                pg_catalog.pg_get_function_arguments(p.oid) AS arguments
            FROM pg_catalog.pg_proc p
            JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            WHERE n.nspname = '{}'  -- 替换为您要查询的模式名称
            AND p.prokind = 'f'  -- 只选择普通函数
            ORDER BY p.proname;
        "#,
            self.config.schema_str()
        );
        log::info!("sql:{}", sql);
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            let return_type: Option<String> = r.get(1);
            let arguments: Option<String> = r.get(2);
            ret.push(NamedStaff {
                name,
                parameters: arguments,
                metas: vec![MetaValue::new("return_type", return_type)],
                cassandra_parameters: None,
            });
        }
        anyhow::Ok(ret)
    }

    async fn show_udf(&self, name: &NamedStaff) -> anyhow::Result<DefAndMetas> {
        let sql = format!(
            r#"
            SELECT  
                pg_catalog.pg_get_function_result(p.oid) AS result_type,
                pg_catalog.pg_get_function_arguments(p.oid) AS arguments,
                pg_get_functiondef(p.oid) AS function_definition
            FROM pg_catalog.pg_proc p
                JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            WHERE p.proname = '{}' and n.nspname ='{}'  -- 替换为您要查询的函数名称
                AND pg_catalog.pg_get_function_arguments(p.oid) = '{}'
        "#,
            name.name,
            self.config.schema_str(),
            name.parameters.clone().unwrap_or_default()
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_one(conn.deref_mut())
            .await?;
        let result_type: Option<String> = row.try_get(0)?;
        let arguments: Option<String> = row.try_get(1)?;
        let function_definition: String = row.try_get(2)?;
        anyhow::Ok(DefAndMetas::new(
            function_definition,
            vec![
                MetaValue::new("result_type", result_type),
                MetaValue::new("parameters", arguments),
            ],
        ))
    }

    async fn drop_udf(&self, name: &NamedStaff) -> anyhow::Result<String> {
        let sql = format!(
            "DROP FUNCTION IF EXISTS {}.{}({})",
            self.config.schema_str(),
            name.name,
            name.parameters.clone().unwrap_or_default()
        );
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn enable_trigger(
        &self,
        table: &str,
        name: &str,
        enable: bool,
    ) -> anyhow::Result<String> {
        let sql = format!(
            r#"ALTER TABLE "{table}" {} TRIGGER "{name}""#,
            if enable { "ENABLE" } else { "DISABLE" }
        );
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        if self.table_exists(dup).await? {
            anyhow::bail!(format!("table '{}' already exists", dup));
        }
        let schema = self.config.schema_str();

        let sql = format!(r#"CREATE TABLE "{schema}"."{dup}" AS TABLE "{schema}"."{table}""#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<Option<String>> {
        let schema = self.config.schema_str();
        let sql = format!(r#"ALTER TABLE "{schema}"."{old}" RENAME TO "{new}""#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn list_trigger(&self, table_name: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
                SELECT tgname 
                FROM pg_trigger 
                WHERE tgrelid = '{table_name}'::regclass;
        "#
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            ret.push(name);
        }
        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
    }

    async fn show_trigger(&self, table: &str, name: &str) -> anyhow::Result<TriggerDetail> {
        let sql = format!(
            r#"
            SELECT
                pg_get_triggerdef(oid)  AS trigger_definition ,
                    tgtype , tgenabled
                FROM
                    pg_trigger
                WHERE
                    tgrelid = '{table}'::regclass
                    AND tgname = '{name}';
        "#
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_one(conn.deref_mut())
            .await?;
        let definition: String = row.get(0);
        let tgtype: i16 = row.get(1);
        let enable: Option<i8> = row.get(2);

        let (event, timing) = parse_trigger_type(tgtype)?;
        anyhow::Ok(TriggerDetail::new(
            name,
            timing,
            event,
            definition,
            Some(crate::db::TriggerEnable::PostGres {
                enable: enable.map(|x| x as u8 as char),
            }),
        ))
    }

    async fn drop_trigger(&self, table: &str, name: &str) -> anyhow::Result<String> {
        let schema = self.config.schema_str();
        let sql = format!(
            r#"
            DROP TRIGGER "{name}" ON "{schema}"."{table}" 
        "#
        );
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn export_table_or_view(
        &self,
        name: &str,
        para: Option<ShowTableParameter>,
        kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::Sql {
                include_table_schema,
            } => {
                self.export_table_as_sql(name, para, include_table_schema, sender)
                    .await
            }
            _ => {
                let sql = self.gen_show_sql(name, None, para.as_ref());
                self.export_query_result(None, &sql, kind, sender).await
            }
        }
    }

    /// FIXME 如果一个主键是自增的只能得到类型
    async fn show_create_table_sql(
        &self,
        table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        // https://stackoverflow.com/questions/2593803/how-to-generate-the-create-table-sql-statement-for-an-existing-table-in-postgr
        // sql coped from
        // notice
        let sql = format!(
            r#"
        SELECT                                          
        'CREATE TABLE ' || relname || E'\n(\n' ||
        array_to_string(
        array_agg(
            '    ' || column_name || ' ' ||  type || ' '|| not_null
        )
        , E',\n'
        ) || E'\n);\n'
        from
        (
        SELECT 
            c.relname, a.attname AS column_name,
            pg_catalog.format_type(a.atttypid, a.atttypmod) as type,
            case 
            when a.attnotnull
            then 'NOT NULL' 
            else 'NULL' 
            END as not_null 
        FROM pg_class c,
            pg_attribute a,
            pg_type t
            WHERE c.relname = '{table_name}'
            AND a.attnum > 0
            AND a.attrelid = c.oid
            AND a.atttypid = t.oid
        ORDER BY a.attnum
        ) as tabledefinition
        group by relname;
        "#
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_one(conn.deref_mut())
            .await?;
        let create_sql: String = row.try_get(0)?;
        let indexes = self.show_indexes(table_name).await?;

        let mut sqls = Vec::with_capacity(indexes.len());
        for index in indexes.iter() {
            let index_sql = get_index_def(
                &mut conn,
                table_name,
                index.name.as_ref().option_must_be_some()?,
            )
            .await?;
            sqls.push(index_sql);
        }
        anyhow::Ok(CreateTableAndIndexSql {
            create_table: create_sql,
            indexes: sqls,
        })
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn show_view(
        &self,
        view_name: &str,
        para: ShowTableParameter,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        emit.send("trying to query total amount.".into())?;
        let total = self
            .query_count(view_name, para.where_str.as_deref())
            .await?;
        let sql = self.gen_show_sql(view_name, None, Some(&para));
        log::info!("sql:{}", sql);
        emit.send("trying to get connection.".into())?;
        let mut conn = self.get_conn().await?;
        emit.send("trying to query data.".into())?;
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql.as_str())).fetch(conn.deref_mut());
        let first = FirstTrue::default();

        while let Some(row) = stream.next().await {
            let row = row?;
            if first.get() {
                emit.send(
                    QueryResp {
                        fields: get_row_fields(&row),
                        total: total as f64,
                        ..Default::default()
                    }
                    .into(),
                )?;
            }
            emit.send(encode_row(&row)?.into())?;
        }

        if first.get() {
            let fields = self.infer_fields_by_sql(&sql).await?;
            emit.send(
                QueryResp {
                    fields,
                    sql: Some(sql),
                    ..Default::default()
                }
                .into(),
            )?;
        }
        anyhow::Ok(())
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<String> {
        let schema = self.config.schema_str();
        let sql = format!(r#"DROP VIEW "{schema}"."{view_name}""#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<Option<String>> {
        let schema = self.config.schema_str();
        let sql = format!(
            r#"DROP TABLE "{schema}"."{table}" {}"#,
            if cascade { " CASCADE " } else { "" }
        );
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        let sql = match self.kind {
            BackendKindPostGres::CockRoach => {
                let sql = format!(
                    r#"
            SELECT  table_name 
            FROM information_schema.tables
            WHERE table_type = 'VIEW'
                AND table_schema = '{}' and table_name not in (SELECT 
                    matviewname
            FROM 
                pg_matviews where schemaname = '{}' ) 
        "#,
                    self.config.schema_str(),
                    self.config.schema_str(),
                );
                sql
            }
            _ => {
                let sql = format!(
                    r#"
        SELECT table_name
        FROM information_schema.tables
        WHERE table_type = 'VIEW'
            AND table_schema = '{}';
        "#,
                    self.config.schema_str()
                );
                sql
            }
        };
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql))
            .fetch_all(conn.deref_mut())
            .await?;

        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let view_name: String = r.get(0);
            ret.push(view_name);
        }
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<Option<String>> {
        let schema = self.config.schema_str();
        let sql = format!(
            r#"
            TRUNCATE TABLE "{schema}"."{table_name}";
        "#
        );
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"SELECT table_name
                FROM information_schema.tables
                WHERE table_type = 'BASE TABLE' AND table_schema = '{}' and table_catalog='{}' ORDER BY table_name
            "#,
            self.config.schema_str(),
            self.config.db_str()
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;

        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let table: String = r.get(0);
            let sql = format!(
                "SELECT pg_relation_size('{}') AS table_size;",
                table.as_str()
            );
            log::info!("sql:{}", sql);
            let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .fetch_one(conn.deref_mut())
                .await;
            let size: Option<i64> = match row {
                Ok(row) => row.get(0),
                Err(_) => None,
            };
            let t = ListTable::new(table, size.map(|x| x as f64));
            ret.push(t);
        }

        anyhow::Ok(ret)
    }

    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListTable>> {
        let schema = self.config.schema_str();

        let sql = format!(
            r#"
            SELECT 
                matviewname
            FROM 
                pg_matviews where schemaname = '{}';
        "#,
            schema,
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            ret.push(ListTable::from(name));
        }
        anyhow::Ok(ret)
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let sql = format!(
            r#"
            SELECT  column_name,is_nullable,column_default
            FROM information_schema.columns where table_catalog='{}' and table_schema='{}' and table_name = '{}' ORDER BY ordinal_position
        "#,
            self.config.db_str(),
            self.config.schema_str(),
            table_name
        );

        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        let primary_keys = {
            let sql: String = format!(
                r#"
                -- 查表的主键详情
                SELECT  kcu.column_name
                    FROM information_schema.table_constraints AS tc
                    JOIN information_schema.key_column_usage  AS kcu
                    ON tc.constraint_name = kcu.constraint_name
                    WHERE tc.constraint_type = 'PRIMARY KEY'
                    AND tc.table_name      = '{table_name}';
        "#
            );
            log::info!("sql:{}", sql);

            let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                .fetch_all(conn.deref_mut())
                .await?;

            let mut keys = HashSet::new();
            for r in rows.iter() {
                let key: String = r.get(0);
                keys.insert(key);
            }
            keys
        };
        let types = self
            .infer_fields_by_sql(&SqlBuilder::gen_show_sql(self, table_name, None, None))
            .await?;
        let mut types_map = HashMap::new();
        for t in types.iter() {
            types_map.insert(t.field.clone(), t);
        }

        for r in rows.iter() {
            let name: String = r.get(0);
            let nullable: String = r.get(1);
            let default_value: Option<String> = r.get(2);
            let is_primary = primary_keys.contains(&name);
            let data_type = match types_map.get(&name) {
                Some(x) => x.data_type.clone(),
                None => anyhow::bail!(format!("filed '{}' should have type", name)),
            };
            let is_auto_increment = data_type.to_lowercase().contains("int")
                && default_value
                    .as_ref()
                    .map(|x| x.contains("nextval"))
                    .unwrap_or(false);

            ret.push(Field::new(
                name,
                data_type,
                nullable.to_lowercase() != "no",
                default_value,
                is_primary,
                is_auto_increment && is_primary,
            ));
        }

        anyhow::Ok(ret)
    }

    async fn show_table(
        &self,
        table_name: &str,
        para: ShowTableParameter,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        emit.send("trying to query total amount.".into())?;
        let total = self
            .query_count(table_name, para.where_str.as_deref())
            .await?;
        let fields = self.show_table_columns(table_name).await?;
        let sql = self.gen_show_sql(table_name, Some(&fields), Some(&para));
        log::info!("sql:{}", sql);
        emit.send(
            QueryResp {
                fields,
                total: total as f64,
                sql: sql.clone().into(),
                ..Default::default()
            }
            .into(),
        )?;
        emit.send("trying to get connection.".into())?;
        let mut conn = self.get_conn().await?;
        emit.send("trying to query data.".into())?;
        let mut stream = sqlx::query(AssertSqlSafe(sql.as_str())).fetch(conn.deref_mut());
        while let Some(row) = stream.next().await {
            let row = row?;
            let d = encode_row(&row)?;
            emit.send(d.into())?;
        }
        anyhow::Ok(())
    }

    async fn drop_materialized_view(&self, name: &str) -> anyhow::Result<Option<String>> {
        let schema = self.config.schema_str();
        let sql = format!(
            r#"DROP MATERIALIZED VIEW  "{}"."{}" RESTRICT"#,
            schema, name
        );
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let mut conn = self.get_conn().await?;

        // this sql copied from https://stackoverflow.com/questions/2204058/list-columns-with-indexes-in-postgresql
        let rows = sqlx::raw_sql(AssertSqlSafe(
            format!(
                r#"
            SELECT
                t.relname as table_name,
                i.relname as index_name,
                array_agg(a.attname) as column_names
            from
                pg_class t,
                pg_class i,
                pg_index ix,
                pg_attribute a
            where
                t.oid = ix.indrelid
                and i.oid = ix.indexrelid
                and a.attrelid = t.oid
                and a.attnum = ANY(ix.indkey)
                and t.relkind = 'r'
                and t.relname = '{table_name}'
            group by
                t.relname,
                i.relname
                ORDER BY
                t.relname,
                i.relname;
        "#
            )
            .as_str(),
        ))
        .fetch_all(conn.deref_mut())
        .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let index_name: String = r.get(1);
            let column_names: Option<Vec<String>> = r.get(2);
            let index_kind = self
                .get_index_kind(&mut conn, table_name, &index_name)
                .await?;
            ret.push(Index::new(
                index_name,
                index_kind,
                column_names.unwrap_or_default(),
            ))
        }
        anyhow::Ok(ret)
    }
}

impl PostGresDB {
    pub(crate) async fn list_created_types_impl(
        &self,
        schema: &str,
        skip_null_create_sql: bool,
    ) -> anyhow::Result<Vec<CreatedTypeDef>> {
        /*在
        pg_type.typtype  里：
        c = composite（复合类型 / 行类型）
        d = domain（域类型，基于已有类型的约束包装）
        e = enum（枚举类型）
        r = range（范围类型，如 int4range、daterange）

        */
        let sql = format!(
            r#"
        SELECT
        t.typname AS type_name,
        CASE
            WHEN t.typtype = 'e' THEN
                'CREATE TYPE ' || quote_ident(n.nspname) || '.' || quote_ident(t.typname) ||
                ' AS ENUM (' ||
                string_agg(quote_literal(e.enumlabel), ', ' ORDER BY e.enumsortorder) ||
                ');'

            WHEN t.typtype = 'c' THEN
                'CREATE TYPE ' || quote_ident(n.nspname) || '.' || quote_ident(t.typname) ||
                ' AS (' ||
                string_agg(
                    quote_ident(a.attname) || ' ' ||
                    pg_catalog.format_type(a.atttypid, a.atttypmod) ||
                    CASE WHEN a.attnotnull THEN ' NOT NULL' ELSE '' END,
                    ', ' ORDER BY a.attnum
                ) ||
                ');'

            WHEN t.typtype = 'd' THEN
                'CREATE DOMAIN ' || quote_ident(n.nspname) || '.' || quote_ident(t.typname) ||
                ' AS ' ||
                pg_catalog.format_type(t.typbasetype, -1) ||
                CASE WHEN t.typnotnull THEN ' NOT NULL' ELSE '' END ||
                COALESCE(
                    (SELECT ' CHECK (' || pg_get_constraintdef(con.oid) || ')'
                    FROM pg_constraint con
                    WHERE con.contypid = t.oid),
                    ''
                ) || ';'

            WHEN t.typtype = 'r' THEN
                'CREATE TYPE ' || quote_ident(n.nspname) || '.' || quote_ident(t.typname) ||
                ' AS RANGE (subtype = ' ||
                (SELECT pg_catalog.format_type(rngsubtype, -1)
                FROM pg_range WHERE rngtypid = t.oid) ||
                (SELECT COALESCE(' collation = ' || quote_ident(collname), '')
                FROM pg_range
                LEFT JOIN pg_collation c ON c.oid = rngcollation
                WHERE rngtypid = t.oid) ||
                (SELECT CASE WHEN rngcanonical::text <> '-' THEN ' canonical = ' || rngcanonical::text ELSE '' END
                FROM pg_range WHERE rngtypid = t.oid) ||
                (SELECT CASE WHEN rngsubdiff::text   <> '-' THEN ' subtype_diff = ' || rngsubdiff::text   ELSE '' END
                FROM pg_range WHERE rngtypid = t.oid) ||
                ');'

            ELSE
                '-- unsupported type --'
                END AS create_sql
            FROM pg_catalog.pg_type t
            JOIN pg_catalog.pg_namespace n ON n.oid = t.typnamespace
            LEFT JOIN pg_catalog.pg_enum e ON t.oid = e.enumtypid
            LEFT JOIN pg_catalog.pg_class c ON c.oid = t.typrelid AND c.relkind = 'c'
            LEFT JOIN pg_catalog.pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
            WHERE t.typtype IN ('c', 'd', 'e', 'r')
            AND n.nspname = '{}'  
            AND t.typisdefined
            GROUP BY n.nspname, t.typname, t.oid, t.typowner, t.typtype,
                t.typbasetype,  -- 添加这个
                t.typnotnull    -- 建议连这个也加上，防止下一个报错
            ORDER BY type_name
        "#,
            schema
        );

        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;

        let mut ret = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            let def: Option<String> = row.try_get(1)?;
            if skip_null_create_sql
                && (def.is_none() || def.as_ref().option_must_be_some()?.trim().is_empty())
            {
                continue;
            }
            ret.push(CreatedTypeDef { name, sql: def });
        }
        anyhow::Ok(ret)
    }
}
impl PostGresDB {
    pub(crate) async fn query_count(
        &self,
        name: &str,
        where_str: Option<&str>,
    ) -> anyhow::Result<usize> {
        let mut conn = self.get_conn().await?;
        let schema = self.config.schema_str();
        let mut sql = format!(r#"SELECT count(1) from "{schema}"."{name}" "#);
        if let Some(where_str) = where_str {
            sql.push_str(format!(" where {where_str}").as_str());
        }
        let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_one(conn.deref_mut())
            .await
            .map_err(|e| anyhow::Error::msg(format!("err:'{}' sql:'{}'", e, sql)))?;
        let count: i64 = row.get(0);
        anyhow::Ok(count as usize)
    }
}

impl std::fmt::Debug for PostGresDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "postgres on:{:?} {:?}", self.config, self.kind)
    }
}

impl PostGresDB {
    pub async fn new(config: PGDBConfig, kind: BackendKindPostGres) -> anyhow::Result<Self> {
        let mut opts = sqlx::postgres::PgConnectOptions::new();
        opts = opts
            .host(&config.host)
            .port(config.port)
            .username(&config.username);
        if let Some(pass) = &config.password {
            opts = opts.password(pass);
        }
        if let Some(db) = config.db.as_ref() {
            opts = opts.database(db);
        }

        opts = opts.ssl_mode(FromStr::from_str(&config.ssl.ssl_mode)?);
        if let Some(ca) = config.ssl.ssl_files.ssl_ca.as_ref() {
            opts = opts.ssl_root_cert(ca);
        }
        if let Some(cert) = config.ssl.ssl_files.ssl_cert.as_ref() {
            opts = opts.ssl_client_cert(cert);
        }
        if let Some(key) = config.ssl.ssl_files.ssl_key.as_ref() {
            opts = opts.ssl_client_key(key);
        }
        let opts = opts.options(config.options.iter());
        let mut pool_opts = PoolOptions::new();
        if let Some(x) = config.conn_timeout {
            pool_opts = pool_opts.acquire_timeout(x.into());
        }
        pool_opts = pool_opts.max_connections(POOL_MAX_CONNECTIONS);
        let pool = pool_opts.connect_with(opts).await?;
        anyhow::Ok(Self {
            pool,
            config,
            query_connection: Default::default(),
            completion_id: Default::default(),
            kind,
        })
    }

    pub(crate) async fn get_conn(&self) -> anyhow::Result<PoolConnection<Postgres>> {
        let mut conn = self.pool.acquire().await?;
        if let Some(s) = &self.config.schema {
            set_current_schema(conn.deref_mut(), s).await?;
        }
        if self.config.read_only {
            let sql = "SET default_transaction_read_only = on;";
            conn.execute(sql).await?;
        }
        anyhow::Ok(conn)
    }
}

fn get_row_fields(row: &PgRow) -> Vec<Field> {
    log::trace!("infer type by:{:?}", row.columns());
    let mut ret = Vec::with_capacity(row.columns().len());
    for c in row.columns().iter() {
        ret.push(Field {
            field: c.name().to_string(),
            data_type: c.type_info().to_string(),
            ..Default::default()
        });
    }
    ret
}

async fn set_current_schema(conn: &mut PgConnection, schema: &str) -> anyhow::Result<()> {
    sqlx::raw_sql(AssertSqlSafe(
        format!("SET search_path TO {schema}").as_str(),
    ))
    .execute(conn)
    .await?;
    anyhow::Ok(())
}

#[async_trait::async_trait]

impl ExportQueryResult for PostGresDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }

    async fn export_query_result(
        &self,
        query_id: Option<&str>,
        sql: &str,
        kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let conn = match query_id {
            Some(id) => match self.query_connection.get(&id.to_string()) {
                Some(x) => x.clone(),
                None => anyhow::bail!("should be able to get connection"),
            },
            None => {
                let conn = self.get_conn().await?.detach();
                Arc::new(Mutex::new(conn))
            }
        };
        let mut conn = conn.lock().await;
        let first = FirstTrue::default();

        let mut stream = sqlx::query(AssertSqlSafe(sql)).fetch(conn.deref_mut());
        let mut row_num = 0;

        while let Some(row) = stream.next().await {
            let row: PgRow = row?;
            match kind {
                ExportKind::Csv {
                    include_header,
                    delimiter,
                } => {
                    if first.get() && include_header {
                        let fs = row
                            .columns()
                            .iter()
                            .map(|x| x.name().to_string())
                            .map(Some)
                            .collect::<Vec<_>>();
                        sender.send(create_csv(&fs, delimiter)?.into()).await?;
                    }
                    let data = encode_row(&row)?;
                    sender.send(create_csv(&data, delimiter)?.into()).await?
                }
                ExportKind::Json { .. } | ExportKind::JsonLine => {
                    let value = encode_row_as_json(&row)?;
                    sender.send(value.into()).await?;
                }
                ExportKind::Sql { .. } => {
                    anyhow::bail!("can't export query result as sql")
                }
                ExportKind::Xlsx { include_header, .. } => {
                    if first.get() && include_header {
                        for (index, col) in row.columns().iter().enumerate() {
                            sender
                                .send(
                                    XlsxEdit {
                                        row: row_num,
                                        column: index as rust_xlsxwriter::ColNum,
                                        content: col.name().to_string().into(),
                                    }
                                    .into(),
                                )
                                .await?;
                        }
                        row_num += 1;
                    }
                    let data = encode_row_as_xlsx(&row)?;
                    export_query_as_xlsx_send_data(&mut row_num, data, sender.clone()).await?;
                }
                ExportKind::Html { include_header } => {
                    if first.get() && !row.is_empty() && include_header {
                        let fs = row
                            .columns()
                            .iter()
                            .map(|x| x.name().to_string())
                            .collect::<Vec<_>>();
                        sender.send(create_html_table_header(&fs).into()).await?;
                    }
                    let data = encode_row(&row)?;
                    sender.send(create_html_table_data(&[data]).into()).await?;
                }
                ExportKind::MarkDown => {
                    if first.get() {
                        let fs = row
                            .columns()
                            .iter()
                            .map(|x| x.name().to_string())
                            .collect::<Vec<_>>();
                        sender.send(create_markdown_header(&fs).into()).await?;
                    }
                    let data = encode_row(&row)?;
                    sender.send(create_markdown_data(&data).into()).await?;
                }
            }
        }
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl QuerySql for PostGresDB {
    fn clone(&self) -> Box<dyn QuerySql> {
        Box::new(Clone::clone(self))
    }
    async fn get_update_sql_from_result_set(
        &self,
        sql: &str,
        old_values: &[Option<String>],
        new_values: &[Option<String>],
    ) -> anyhow::Result<Vec<String>> {
        get_update_sql_from_result_set_impl(self, sql, old_values, new_values).await
    }
    fn support_tx(&self) -> bool {
        true
    }
    async fn get_auto_commit(&self, query_id: &str) -> anyhow::Result<bool> {
        use sqlx_core::any::AnyConnectionBackend;

        let conn = self.query_connection.get(&query_id.to_string());
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let depth = conn.get_transaction_depth();
        anyhow::Ok(depth == 0)
    }

    async fn set_auto_commit(&self, query_id: &str, commit: bool) -> anyhow::Result<()> {
        if commit {
            anyhow::bail!("sqlite use  commit or rollback to end transaction");
        }
        use sqlx_core::any::AnyConnectionBackend;
        let conn = self.query_connection.get(&query_id.to_string());
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };

        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.begin(None).await?;
        anyhow::Ok(())
    }

    async fn commit(&self, query_id: &str) -> anyhow::Result<()> {
        use sqlx_core::any::AnyConnectionBackend;
        let conn = self.query_connection.get(&query_id.to_string());
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.commit().await?;
        anyhow::Ok(())
    }

    async fn rollback(&self, query_id: &str) -> anyhow::Result<()> {
        use sqlx_core::any::AnyConnectionBackend;
        let conn = self.query_connection.get(&query_id.to_string());
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.rollback().await?;

        anyhow::Ok(())
    }

    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.get_conn().await?.detach();
        self.query_connection
            .insert(id.to_owned(), Arc::new(Mutex::new(conn)));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        self.query_connection.remove(&id.to_owned());
        anyhow::Ok(())
    }
    async fn simple_execute_sql(&self, sql: &str) -> anyhow::Result<Option<usize>> {
        let mut conn = self.get_conn().await?;
        let ret = conn
            .execute(sqlx::raw_sql(AssertSqlSafe(sql)))
            .await
            .map_err(|err| anyhow::Error::msg(format!("Sql:{} Err:{}", sql, err)))?;
        anyhow::Ok(Some(ret.rows_affected() as usize))
    }

    async fn execute_sql(
        &self,
        query_id: &str,
        sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        crate::language_server::LangServerCommon::contains_tx_operation_error(self, sql)?;
        let conn = self.query_connection.get(&query_id.to_string());
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let ret = match conn.execute(sqlx::raw_sql(AssertSqlSafe(sql))).await {
            Ok(x) => Ok(QueryResp::row_affected(x.rows_affected(), sql.to_owned())),
            Err(err) => Err(err.to_string()),
        };
        anyhow::Ok(ret)
    }

    async fn query_sql(
        &self,
        query_id: &str,
        sql: &str,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        crate::language_server::LangServerCommon::contains_tx_operation_error(self, sql)?;
        emit.send(QueryEvent::new_status("trying to lock connection."))?;
        let conn = self.query_connection.get(&query_id.to_string());
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        emit.send(QueryEvent::new_status("locked connection."))?;
        emit.send(QueryEvent::new_status("trying to query."))?;
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql)).fetch_many(conn.deref_mut());
        let mut has_result = false;
        let mut emit_fields = vec![];
        while let Some(item) = stream.next().await {
            let item = match item {
                Ok(x) => x,
                Err(err) => {
                    emit.send(QueryEvent::Err(err.to_string()))?;
                    continue;
                }
            };
            match item {
                sqlx::Either::Left(row_aff) => {
                    println!("row aff:{:?}", row_aff);
                }
                sqlx::Either::Right(row) => {
                    has_result = true;
                    let fields = get_row_fields(&row);
                    if emit_fields.is_empty() || !emit_fields.eq(&fields) {
                        emit.send(
                            QueryResp {
                                fields: fields.clone(),
                                sql: Some(sql.to_owned()),
                                tables: crate::language_server::LangServerCommon::extract_tables(
                                    self, sql,
                                )
                                .unwrap_or_default(),
                                ..Default::default()
                            }
                            .into(),
                        )?;
                        emit_fields = fields;
                    }
                    let data = encode_row(&row)?;
                    emit.send(data.into())?;
                }
            }
        }

        drop(stream);
        if !has_result {
            let fields = self.infer_fields_by_sql(sql).await?;
            emit.send(QueryResp::new(fields, 0f64, vec![], sql.to_owned())?.into())?;
        }
        anyhow::Ok(())
    }
}

async fn get_index_def(
    conn: &mut PoolConnection<Postgres>,
    table_name: &str,
    index_name: &str,
) -> anyhow::Result<String> {
    let sql = format!(
        "SELECT indexdef from pg_indexes where tablename='{table_name}' and indexname='{index_name}'"
    );
    log::info!("sql:{}", sql);
    let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
        .fetch_one(conn.deref_mut())
        .await?;

    let sql: String = row.get(0);

    anyhow::Ok(sql)
}

impl PostGresDB {
    async fn get_index_kind(
        &self,
        conn: &mut PoolConnection<Postgres>,
        table_name: &str,
        index_name: &str,
    ) -> anyhow::Result<IndexKind> {
        let sql: String = get_index_def(conn, table_name, index_name).await?;
        let statements = self.parse_statements(&sql)?;
        if statements.len() != 1 {
            anyhow::bail!("parse create index should contains one statement");
        }
        let stmt = &statements[0];
        use sqlparser::ast::Statement;

        match stmt {
            Statement::CreateIndex(create_index) => {
                if create_index.unique {
                    anyhow::Ok(IndexKind::Unique)
                } else {
                    anyhow::Ok(IndexKind::Index)
                }
            }

            _ => {
                anyhow::bail!("the sql is not a create index".to_string())
            }
        }
    }
}

impl GetBackendKind for PostGresDB {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::PostGres(self.kind)
    }
}

#[async_trait::async_trait]

impl CatalogTrait for PostGresDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(
            "SELECT datname FROM pg_database where datname not in ('template0' , 'template1') order by datname",
        )
        .fetch_all(conn.deref_mut())
        .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        let mut config = self.config.clone();
        config.db = Some(name.to_string());
        let x = Self::new(config, self.kind).await?;
        anyhow::Ok(Arc::new(x))
    }

    async fn create_database(
        &self,
        name: &str,
        _extra: Option<CreateDatabaseExtra>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"CREATE DATABASE "{name}""#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(())
    }

    async fn drop_database(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP DATABASE "{name}""#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl SslFilesWithStrModel {
    #[cfg(test)]
    pub(crate) fn pg_default() -> Self {
        SslFilesWithStrModel {
            ssl_mode: "disable".into(),
            ssl_files: crate::datasource_config::SSLFiles::default(),
        }
    }
}

#[cfg(test)]
pub(crate) async fn make_test_object() -> PostGresDB {
    use crate::datasource_config::SslFilesWithStrModel;
    let pg = PostGresDB::new(
        PGDBConfig {
            host: crate::TEST_HOST.to_string(),
            port: 5432,
            username: "postgres".to_string(),
            password: Some("root".to_string()),
            db: Some("test".into()),
            schema: Some("xxx".to_string()),
            read_only: false,
            ssl: SslFilesWithStrModel::pg_default(),
            conn_timeout: Some(5000.into()),
            options: Default::default(),
        },
        BackendKindPostGres::PostGres,
    )
    .await
    .unwrap();
    pg
}

/// postgres有哪些值可以直接放在sql语句里面的想current timestamp给我完成的rust列表
/// PostgreSQL 里可以直接写在 SQL 语句中的 CURRENT_* 字面量
pub const PG_CURRENT_LITERALS: &[&str] = &[
    "CURRENT_DATE",      //  2025-08-25
    "CURRENT_TIME",      //  14:36:07.123+02
    "CURRENT_TIMESTAMP", //  2025-08-25 14:36:07.123+02
    "CURRENT_USER",      //  alice
    "CURRENT_ROLE",      //  alice（或 SET ROLE 后的角色）
];

impl ExtractSqlValeFromJsonValue for PostGresDB {
    fn extrat_sql_value_from_json_value(
        &self,
        value: &serde_json::Value,
        ty: &str,
    ) -> anyhow::Result<Option<String>> {
        let ty = ty.trim().to_uppercase();
        let ty = ty.as_str();
        let err = anyhow::Error::msg(format!(
            "not valid value,type '{}',value:{}",
            ty,
            serde_json::to_string(value)?
        ));

        if ty.eq_ignore_ascii_case("JSON") {
            match value {
                serde_json::Value::Null => return anyhow::Ok(None),
                _ => return anyhow::Ok(Some(serde_json::to_string(value)?)),
            }
        }
        if is_text_array_ty(ty) {
            match value {
                serde_json::Value::Null => return anyhow::Ok(None),
                serde_json::Value::Array(values) => {
                    let mut results = Vec::with_capacity(values.len());
                    for value in values.iter() {
                        let value = match value {
                            serde_json::Value::String(x) => x,
                            _ => return Err(err),
                        };
                        results.push(escape_double_quote(value));
                    }

                    return anyhow::Ok(Some(format!("{{{}}}", results.join(","))));
                }
                _ => return Err(err),
            }
        }
        if ty.eq_ignore_ascii_case("JSON[]") {
            anyhow::bail!("JSON[] not supported")
        }

        if ty.eq_ignore_ascii_case("INT4RANGE[]")
            || ty.eq_ignore_ascii_case("TSRANGE[]")
            || ty.eq_ignore_ascii_case("TSTZRANGE[]")
            || ty.eq_ignore_ascii_case("DATERANGE[]")
            || ty.eq_ignore_ascii_case("INT8RANGE[]")
        {
            match value {
                serde_json::Value::Null => return anyhow::Ok(None),
                serde_json::Value::Array(values) => {
                    let mut results = Vec::with_capacity(values.len());
                    for value in values.iter() {
                        let value = match value {
                            serde_json::Value::String(x) => x,
                            _ => return Err(err),
                        };
                        results.push(format!("\"{}\"", value));
                    }

                    return anyhow::Ok(Some(format!("{{{}}}", results.join(","))));
                }
                _ => return Err(err),
            }
        }

        fn encode_value(v: &serde_json::Value) -> anyhow::Result<Option<String>> {
            let r = match v {
                serde_json::Value::Null => None,
                serde_json::Value::Bool(x) => Some(x.to_string()),
                serde_json::Value::Number(number) => Some(number.to_string()),
                serde_json::Value::String(x) => Some(x.clone()),
                serde_json::Value::Array(values) => {
                    let mut result = Vec::with_capacity(values.len());
                    for v in values.iter() {
                        result.push(encode_value(v)?.unwrap_or("null".to_owned()));
                    }
                    Some(format!("{{{}}}", result.join(",")))
                }
                serde_json::Value::Object(map) => Some(serde_json::to_string(map)?),
            };
            anyhow::Ok(r)
        }
        anyhow::Ok(encode_value(value)?)
    }
}

impl SqlBuilder for PostGresDB {
    fn get_schema(&self) -> Option<&str> {
        self.config.schema.as_deref()
    }

    fn get_current_values(&self) -> &'static [&'static str] {
        PG_CURRENT_LITERALS
    }

    fn put_value_in_sql(&self, typ: &str, value: &str) -> anyhow::Result<String> {
        put_value_in_sql(typ, value)
    }

    fn type_support_compare(&self, typ: &str) -> bool {
        !matches!(
            typ.to_uppercase().as_str(),
            "JSON" | "JSON[]" | "JSONPATH" | "JSONPATH[]"
        )
    }

    fn gen_show_sql(
        &self,
        table_name: &str,
        fields: Option<&[Field]>,
        para: Option<&ShowTableParameter>,
    ) -> String {
        let schema = self.config.schema_str();
        #[allow(clippy::format_in_format_args)]
        let mut sql = format!(
            r#"SELECT {} from "{schema}"."{table_name}""#,
            self.backend_kind().format_fields(fields)
        );
        if let Some(para) = para {
            let ShowTableParameter {
                where_str,
                order_by,
                page_size,
                ..
            } = &para;
            if let Some(where_str) = where_str {
                sql.push_str(format!(" WHERE {where_str} ").as_str());
            }
            if let Some(order_by) = order_by {
                sql.push_str(format!(" ORDER BY {order_by} ").as_str());
            }
            let offset = para.offset();
            sql.push_str(format!(" OFFSET {} LIMIT {}", offset, page_size).as_str());
        }
        sql
    }
}

impl GetTestExportTableKinds for PostGresDB {
    fn support_table_schema(&self) -> bool {
        false
    }
}
#[cfg(test)]
mod tests {
    use super::make_test_object;
    #[allow(unused_imports)]
    use crate::enable_test_log;
    use crate::{
        datasource_config::{PGDBConfig, SSLFiles, SslFilesWithStrModel},
        db::{
            BackendKindPostGres, CheckConn, ColumnType, CreateIndexColumn, CreateIndexReq,
            CreateTableReq, CreateTableReqColumn, DBTrait, GetTestExportTableKinds, IndexKind,
            NamedStaff, QuerySql, SchemaTrait, ServerMetas, ShowTableParameter, SqlNullAble,
            TestQueryEventEmit,
        },
        helper::{cert_files_dir, start_docker_container, DockerContainerDropper},
        pg::PostGresDB,
    };
    use sqlx::{Column, Executor, Row, SqlSafeStr, Statement};
    use std::{collections::HashMap, ops::DerefMut, process::Command, sync::Arc, time::Duration};

    #[test]
    fn test_output_bytes() {
        let x = crate::pg_format::output_bytes(&[98, 126]);
        assert_eq!(x, "\\x627E")
    }

    #[tokio::test]
    async fn test_read_only() {
        let db = PostGresDB::new(
            PGDBConfig {
                host: crate::TEST_HOST.to_string(),
                port: 5432,
                username: "postgres".to_string(),
                password: Some("root".to_string()),
                db: Some("test".into()),
                schema: Some("xxx".to_string()),
                read_only: true,
                ssl: SslFilesWithStrModel::pg_default(),
                conn_timeout: Some(5000.into()),
                options: Default::default(),
            },
            BackendKindPostGres::PostGres,
        )
        .await
        .unwrap();
        let mut conn = db.get_conn().await.unwrap();
        let err = conn
            .deref_mut()
            .execute(
                r#"
        CREATE TABLE users (
            email VARCHAR(255)
        );
        "#,
            )
            .await;
        assert!(err.is_err());
        if let Err(err) = err {
            let x = format!("{:?}", err);
            assert!(x.contains("read-only"));
        }
    }
    #[tokio::test]
    async fn test_table_pagination() {
        let db = make_test_object().await;
        let table_name = "test_table_pagination";
        let _ = db.drop_table(table_name, false).await;
        db.create_table(&CreateTableReq {
            name: table_name.into(),
            columns: vec![
                CreateTableReqColumn {
                    name: "id".into(),
                    data_type: "SMALLINT".into(),
                    auto_increment: false,
                    primary_key: true,
                    default_value: None,
                    nullable: SqlNullAble::Unspecified,
                },
                CreateTableReqColumn {
                    name: "name".into(),
                    data_type: "varchar(255)".into(),
                    auto_increment: false,
                    primary_key: true,
                    default_value: None,
                    nullable: SqlNullAble::Unspecified,
                },
            ],
            indexes: vec![],
        })
        .await
        .unwrap();

        for i in 0..155 {
            let mut values = HashMap::new();
            values.insert("id".to_owned(), Some((i + 1).to_string()));
            values.insert("name".to_owned(), Some(format!("name{}", i + 1)));
            db.table_insert(table_name, &values).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_list_created_types() {
        let db = make_test_object().await;
        let _ = db.drop_created_type("test_type").await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();

        db.execute_sql(
            query_id,
            r#"
            CREATE TYPE test_type AS ENUM ('value1', 'value2', 'value3');"#,
        )
        .await
        .unwrap()
        .unwrap();
        let list = db.list_created_types().await.unwrap();
        for l in list.iter() {
            println!("create type:{:?}", l);
        }
    }

    #[tokio::test]
    async fn test_procedure() {
        let db = make_test_object().await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let parameter = "a integer".to_string();
        let name = "test_procedure";
        let err = db
            .drop_procedure(&NamedStaff {
                name: name.to_string(),
                parameters: Some(parameter.clone()),
                ..Default::default()
            })
            .await;
        if let Err(e) = err {
            log::error!("drop procedure failed: {}", e);
        }
        db.execute_sql(
            query_id,
            r#"
            CREATE PROCEDURE test_procedure(a integer)
            LANGUAGE plpgsql AS
            $$
            BEGIN
                -- procedure logic goes here
            END;
            $$;
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let list = SchemaTrait::list_procedure(&db).await.unwrap();
        for l in list.iter() {
            let details = db.show_procedure(l).await.unwrap();
            println!("udf:{},details:{:?}", l.name, details);
        }
    }

    #[tokio::test]
    async fn test_view() {
        let db = make_test_object().await;
        let _ = db.drop_view("test_view").await;
        let _ = db.drop_table("test_view_table", true).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_view_table (
                
                name TEXT,
                value INT
            );
            INSERT INTO test_view_table (name, value) VALUES
                ('Item A', 10),
                ('Item B', 20),
                ('Item C', 30);

            CREATE VIEW test_view AS
                SELECT * FROM test_view_table;
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let list = db.list_view().await.unwrap();
        for l in list.iter() {
            let table_resp = db
                .show_view_collect(&l.name, ShowTableParameter::new_stupid_test("name asc"))
                .await
                .unwrap();
            println!("resp:{:?}", table_resp);
        }
    }

    #[tokio::test]
    async fn test_materialized_view() {
        let db = make_test_object().await;
        let _ = db.drop_materialized_view("test_material_view").await;
        let _ = db.drop_table("test_material_view_table", true).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_material_view_table (
                
                name TEXT,
                value INT
            );
            INSERT INTO test_material_view_table (name, value) VALUES
                ('Item A', 10),
                ('Item B', 20),
                ('Item C', 30);

            CREATE MATERIALIZED VIEW test_material_view AS
                SELECT * FROM test_material_view_table;
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let list = db.list_materialized_view().await.unwrap();
        for l in list.iter() {
            let table_resp = db
                .show_view_collect(&l.name, ShowTableParameter::new_stupid_test("name asc"))
                .await
                .unwrap();
            println!("resp:{:?}", table_resp);
        }
    }
    #[tokio::test]
    async fn test_server_metas() {
        let db = make_test_object().await;
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }

    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object().await;
        CheckConn::check_conn(&db).await.unwrap();
    }

    // right now we only support receive data from the database
    #[tokio::test]
    async fn test_regproc() {
        let db = make_test_object().await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.query_sql(
            query_id,
            r#"
        select  aggfnoid , aggtransfn  from pg_catalog.pg_aggregate limit 4

        "#,
            Arc::new(TestQueryEventEmit::default()),
        )
        .await
        .unwrap();
    }

    #[tokio::test]

    async fn test_rename() {
        let db = make_test_object().await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("test_rename", false).await;
        let _ = db.drop_table("test_rename2", false).await;
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_rename (
                    
                    name VARCHAR(255) NOT NULL,
                    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT CURRENT_TIMESTAMP
                );"#,
        )
        .await
        .unwrap()
        .unwrap();
        db.rename_table("test_rename", "test_rename2")
            .await
            .unwrap();

        db.show_table_collect(
            "test_rename2",
            ShowTableParameter::new_stupid_test("name asc"),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_duplicate() {
        let db = make_test_object().await;

        let _ = db.drop_table("test_duplicate", false).await;
        let _ = db.drop_table("test_duplicate2", false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();

        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_duplicate (
                    
                    data TEXT,
                    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO test_duplicate (data) VALUES
                ('测试数据1'),
                ('测试数据2'),
                ('测试数据3'),
                ('测试数据4'),
                ('测试数据5');
                "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.duplicate_table("test_duplicate", "test_duplicate2")
            .await
            .unwrap();

        let resp = db
            .show_table_collect(
                "test_duplicate2",
                ShowTableParameter::new_stupid_test("data asc"),
            )
            .await
            .unwrap();
        assert_eq!(resp.data.len(), 5);
    }

    #[tokio::test]
    async fn test_index() {
        let table_name = "index_test";
        let db = make_test_object().await;
        let index_name = "index_test_index_xxx";

        let _ = db.drop_index(table_name, index_name).await;
        let _ = db.drop_table(table_name, false).await;
        let req = CreateTableReq {
            name: table_name.to_string(),
            columns: vec![
                CreateTableReqColumn {
                    name: "a".to_string(),
                    data_type: "int".to_string(),
                    auto_increment: false,
                    primary_key: true,
                    default_value: Some(1.to_string()),
                    nullable: Default::default(),
                },
                CreateTableReqColumn {
                    name: "b".to_string(),
                    data_type: "int".to_string(),
                    auto_increment: false,
                    primary_key: true,
                    default_value: Some(1.to_string()),
                    nullable: Default::default(),
                },
                CreateTableReqColumn {
                    name: "c".to_string(),
                    data_type: "int".to_string(),
                    auto_increment: false,
                    primary_key: false,
                    default_value: Some(1.to_string()),
                    nullable: SqlNullAble::Null,
                },
            ],
            indexes: vec![],
        };
        db.create_table(&req).await.unwrap();

        let sql = db.create_table_sql_string(&req).unwrap();

        println!("create table sql:{}", sql);

        let index_name2 = format!("{}2", index_name);
        db.create_index(
            table_name,
            &CreateIndexReq {
                name: index_name.to_owned(),
                kind: IndexKind::Unique,
                columns: vec![
                    CreateIndexColumn {
                        col: "a".to_string(),
                        asc: Some(true),
                    },
                    CreateIndexColumn {
                        col: "b".to_string(),
                        asc: Some(true),
                    },
                ],
            },
        )
        .await
        .unwrap();
        db.rename_index(table_name, index_name, &index_name2)
            .await
            .unwrap();
        db.rename_index(table_name, &index_name2, index_name)
            .await
            .unwrap();

        let indexes = db.show_indexes(table_name).await.unwrap();
        for index in indexes.iter() {
            println!("index:{:?}", index)
        }
    }

    #[tokio::test]
    async fn test_list_table() {
        // list all table and query
        let pg = make_test_object().await;

        let table = pg.list_table().await.unwrap();
        println!("{:?}", table);
    }

    #[tokio::test]
    async fn test_udf() {
        let pg = make_test_object().await;
        let name = "test_udf";

        let _ = pg
            .drop_udf(&NamedStaff {
                name: name.to_string(),
                parameters: Some("a integer, b integer".to_string()),
                ..Default::default()
            })
            .await;
        let query_id = "aaaa";
        pg.open_connection(query_id).await.unwrap();
        pg.execute_sql(
            query_id,
            r#"
            CREATE OR REPLACE FUNCTION test_udf(a INT, b INT)
            RETURNS INT AS $$
            BEGIN
                RETURN a + b;
            END;
            $$ LANGUAGE plpgsql;
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let list = pg.list_udf().await.unwrap();
        for l in list.iter() {
            // I created some other functions to test.
            // so we only test on `test_udf`.
            if l.name != "test_udf" {
                continue;
            }
            println!("udf:{:?}", l);
            let details = pg.show_udf(l).await.unwrap();
            println!("name:{} , udf:{:?}", l.name, details);
        }
    }

    #[tokio::test]
    async fn test_triggers() {
        let db = make_test_object().await;

        //
        let _ = db.drop_table("test_create_trigger", true).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
        CREATE TABLE test_create_trigger (
            
            name VARCHAR(50)
        );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let _ = db.drop_table("test_create_trigger_update", true).await;

        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_create_trigger_update (
                
                name VARCHAR(50),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
    "#,
        )
        .await
        .unwrap()
        .unwrap();
        let _ = db
            .drop_udf(&NamedStaff {
                name: "trigger_insert_test_create_trigger".to_string(),
                parameters: Some("".into()),
                ..Default::default()
            })
            .await;

        db.execute_sql(
            query_id,
            r#"
            CREATE FUNCTION trigger_insert_test_create_trigger()
            RETURNS TRIGGER AS $$
            BEGIN
                INSERT INTO test_create_trigger_update (name, created_at)
                VALUES (NEW.name, CURRENT_TIMESTAMP);
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;
    "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
            CREATE TRIGGER trigger_insert_test_create_trigger
            AFTER INSERT ON test_create_trigger
            FOR EACH ROW
            EXECUTE FUNCTION trigger_insert_test_create_trigger();
    "#,
        )
        .await
        .unwrap()
        .unwrap();

        let triggers = db.list_trigger("test_create_trigger").await.unwrap();
        for tri in triggers.iter() {
            let detail = db
                .show_trigger("test_create_trigger", tri.name.as_str())
                .await
                .unwrap();

            // disable trigger and enable trigger test.
            db.enable_trigger("test_create_trigger", &tri.name, false)
                .await
                .unwrap();
            db.enable_trigger("test_create_trigger", &tri.name, true)
                .await
                .unwrap();

            println!("details:{:?}", detail);
        }
    }

    #[tokio::test]
    async fn test_transaction() {
        let db = make_test_object().await;

        let table_name = "test_commit";
        let _ = db.drop_table(table_name, false).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_commit (
                ID INTEGER PRIMARY KEY,
                NAME VARCHAR(50)
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.set_auto_commit(query_id, false).await.unwrap();

        db.execute_sql(
            query_id,
            r#"
            INSERT INTO test_commit (ID, NAME) VALUES (1, 'Test');
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.rollback(query_id).await.unwrap();

        assert!(
            db.show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"),)
                .await
                .unwrap()
                .total
                == 0f64
        );
        db.set_auto_commit(query_id, false).await.unwrap();

        db.execute_sql(
            query_id,
            r#"
            INSERT INTO test_commit (ID, NAME) VALUES (1, 'Test');
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.commit(query_id).await.unwrap();
        assert!(
            db.show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"),)
                .await
                .unwrap()
                .total
                == 1f64
        );
    }

    #[tokio::test]
    async fn test_sequence() {
        let db = make_test_object().await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let name = "test_seq";
        let _ = db.drop_sequence(name, true).await;
        db.execute_sql(
            query_id,
            r#"
            CREATE SEQUENCE test_seq AS integer START 1;
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let name2 = format!("{}2", name);

        let seq = db.list_sequence().await.unwrap();
        for seq in seq.iter() {
            if seq.name.eq(name) {
                println!("seq:{:?}", seq);
            }
        }
        db.rename_sequence(name, &name2).await.unwrap();
        db.rename_sequence(&name2, name).await.unwrap();
    }

    #[tokio::test]
    async fn test_export() {
        let db = make_test_object().await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let table_name = "test_export_table";

        let _ = db.drop_table(table_name, false).await;

        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_export_table (
                
                name VARCHAR(100) NOT NULL,
                date DATE
            );
            INSERT INTO test_export_table (name, date) VALUES
            ('Alice', '2024-06-01'),
            ('Bob', '2024-06-02'),
            ('Charlie', '2024-06-03'),
            ('David', '2024-06-04'),
            ('Eve', '2024-06-05');
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        GetTestExportTableKinds::test_export(&db, table_name).await;
    }

    #[tokio::test]
    async fn test_types1() {
        //
        let table_name = "test_types1";
        let pg = make_test_object().await;
        let _ = pg.drop_table(table_name, false).await;
        let query_id = "aaaa";
        pg.open_connection(query_id).await.unwrap();
        pg.execute_sql(
            query_id,
            format!(
                r#"
        CREATE TABLE {table_name} (
            bool_column BOOL,
            bytea_column BYTEA,
            char_column CHAR,
            name_column NAME,
            int8_column INT8,
            int2_column INT2,
            int4_column INT4
        );"#
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert("bool_column".to_string(), Some("true".to_string()));
        values.insert("bytea_column".to_string(), Some("\\x010203".to_string()));
        values.insert("char_column".to_string(), Some("A".to_string()));
        values.insert("name_column".to_string(), Some("John".to_string()));
        values.insert("int8_column".to_string(), Some("1".to_string()));
        values.insert("int2_column".to_string(), Some("2".to_string()));
        values.insert("int4_column".to_string(), Some("100".to_string()));
        pg.table_insert(table_name, &values).await.unwrap();
        let x = pg
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("bool_column asc"),
            )
            .await
            .unwrap();
        println!("{:?}", x);

        GetTestExportTableKinds::test_export(&pg, table_name).await;

        pg.test_table_export_and_import(table_name).await;
        pg.test_table_equal(table_name, &values, &[]).await;
    }

    #[tokio::test]
    async fn test_types2() {
        let table_name = "test_types2";
        let pg = make_test_object().await;
        let _ = pg.drop_table(table_name, false).await;
        let query_id = "aaaa";
        pg.open_connection(query_id).await.unwrap();
        pg.execute_sql(
            query_id,
            format!(
                r#"
        CREATE TABLE {table_name} (
            id SERIAL PRIMARY KEY,
            text_column TEXT,
            oid_column OID,
            json_column JSON,
            float4_column FLOAT4,
            float8_column FLOAT8,
            macaddr8_column MACADDR8,
            macaddr8_array_column MACADDR8[]
        );
    "#
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert("text_column".to_string(), Some("Sample text".to_string()));
        values.insert("oid_column".to_string(), Some("12345".to_string()));
        values.insert(
            "json_column".to_string(),
            Some(r#"{"key":"value"}"#.to_string()),
        );
        values.insert("float4_column".to_string(), Some("3.14".to_string()));
        values.insert("float8_column".to_string(), Some("5.6789".to_string()));
        values.insert(
            "macaddr8_column".to_string(),
            Some("08:00:2B:01:02:03:04:05".to_string()),
        );
        values.insert(
            "macaddr8_array_column".to_string(),
            Some("{08:00:2B:01:02:03:04:06,08:00:2B:01:02:03:04:07}".to_string()),
        );

        pg.table_insert(table_name, &values).await.unwrap();

        let x = pg
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("{:?}", x);

        GetTestExportTableKinds::test_export(&pg, table_name).await;
        pg.test_table_export_and_import(table_name).await;
        pg.test_table_equal(table_name, &values, &["id"]).await;
    }

    #[tokio::test]
    async fn test_table_column() {
        let db = make_test_object().await;
        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_column
                (
                    id INT PRIMARY KEY                     -- 主键，无符号32位整数
                )
                "#,
        )
        .await
        .unwrap()
        .unwrap();

        let sql = db
            .add_table_column(
                table_name,
                "name",
                ColumnType {
                    typ: "INT2".to_string(),
                    nullable: SqlNullAble::Null.into(),
                    after: None,
                    default_value: Some("1".to_string()),
                },
            )
            .await
            .unwrap();

        println!("add table column sql:{}", sql);

        let sql = db
            .modify_table_column(
                table_name,
                "name",
                "name2",
                Some(ColumnType {
                    typ: "INT4".to_string(),
                    nullable: SqlNullAble::Null.into(),
                    after: None,
                    default_value: Some("2".to_string()),
                }),
            )
            .await
            .unwrap();

        println!("modify table column sql:{}", sql);

        let sql = db.drop_table_column(table_name, "name2").await.unwrap();
        println!("drop table column sql:{}", sql);
    }

    #[tokio::test]
    async fn test_schemas() {
        let pg = make_test_object().await;
        let _ = pg.drop_schema("abc").await;
        let _ = pg.create_schema("abc", None).await;
        let resp = pg.list_schema().await.unwrap();
        println!("{:?}", resp);
    }
    #[tokio::test]
    async fn test_types3() {
        let pg = make_test_object().await;
        let table_name = "test_types3";
        let _ = pg.drop_table(table_name, false).await;
        let query_id = "aaaa";
        pg.open_connection(query_id).await.unwrap();
        pg.execute_sql(
            query_id,
            format!(
                r#"
        CREATE TABLE {table_name} (
            macaddr_column MACADDR,
            inet_column INET,
            bool_array_column BOOL[],
            bytea_array_column BYTEA[],
            char_array_column CHAR[],
            name_array_column NAME[]
        );
    "#
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert(
            "macaddr_column".to_string(),
            Some("08:00:2B:01:02:03".to_string()),
        );
        values.insert(
            "inet_column".to_string(),
            Some("192.168.0.1/32".to_string()),
        );
        values.insert("bool_array_column".to_string(), Some("{t,f}".to_string()));
        values.insert(
            "bytea_array_column".to_string(),
            Some(r#"{"\\x010203"}"#.to_string()),
        );
        values.insert(
            "char_array_column".to_string(),
            Some("{\"a\",\"b\"}".to_string()),
        );
        values.insert(
            "name_array_column".to_string(),
            Some("{\"a\",\"b\"}".to_string()),
        );

        pg.table_insert(table_name, &values).await.unwrap();

        let x = pg
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("macaddr_column asc"),
            )
            .await
            .unwrap();
        println!("{:?}", x);

        GetTestExportTableKinds::test_export(&pg, table_name).await;
        pg.test_table_export_and_import(table_name).await;
        pg.test_table_equal(table_name, &values, &[]).await;
    }

    #[tokio::test]
    async fn test_types4() {
        let table_name = "test_types4";
        let pg = make_test_object().await;
        let _ = pg.drop_table(table_name, false).await;
        let query_id = "aaaa";
        pg.open_connection(query_id).await.unwrap();
        pg.execute_sql(
            query_id,
            format!(
                r#"
        CREATE TABLE {table_name} (
            int2_array_column INT2[],
            int4_array_column INT4[],
            text_array_column TEXT[],
            varchar_array_column VARCHAR[],
            int8_array_column INT8[]
        );
    "#
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert("int2_array_column".to_string(), Some("{1,2}".to_string()));
        values.insert("int4_array_column".to_string(), Some("{1,2}".to_string()));
        values.insert(
            "text_array_column".to_string(),
            Some(r#"{"a","b"}"#.to_string()),
        );
        values.insert(
            "varchar_array_column".to_string(),
            Some(r#"{"c\"","d"}"#.to_string()),
        );
        values.insert("int8_array_column".to_string(), Some("{1,100}".to_string()));

        pg.table_insert(table_name, &values).await.unwrap();

        let x = pg
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("int2_array_column asc"),
            )
            .await
            .unwrap();
        println!("{:?}", x);

        GetTestExportTableKinds::test_export(&pg, table_name).await;
        pg.test_table_export_and_import(table_name).await;
        pg.test_table_equal(table_name, &values, &[]).await;
    }

    #[tokio::test]
    async fn test_types5() {
        let table_name = "test_types5";
        let pg = make_test_object().await;
        let _ = pg.drop_table(table_name, false).await;
        let query_id = "aaaa";
        pg.open_connection(query_id).await.unwrap();
        pg.execute_sql(
            query_id,
            format!(
                r#"
        CREATE TABLE {table_name} (
            id SERIAL PRIMARY KEY,
            -- path_column PATH,  
            -- path_array_column PATH[],
            float4_array_column FLOAT4[],
            float8_array_column FLOAT8[],
            macaddr_array_column MACADDR[],
            inet_array_column INET[],
            date_column DATE
        )
    "#,
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();
        let mut values = HashMap::new();
        values.insert("float4_array_column".to_string(), Some("{1,2}".to_string()));
        values.insert("float8_array_column".to_string(), Some("{3,4}".to_string()));

        values.insert(
            "macaddr_array_column".to_string(),
            Some("{01:02:03:04:05:06}".to_string()),
        );
        values.insert(
            "inet_array_column".to_string(),
            Some("{192.168.1.1/32}".to_string()),
        );

        values.insert("date_column".to_string(), Some("2022-01-01".to_string()));

        pg.table_insert(table_name, &values).await.unwrap();

        let x = pg
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("{:?}", x);

        GetTestExportTableKinds::test_export(&pg, table_name).await;
        pg.test_table_export_and_import(table_name).await;
        pg.test_table_equal(table_name, &values, &["id"]).await;
    }

    #[tokio::test]
    async fn test_types6() {
        let table_name = "test_types6";
        let pg = make_test_object().await;

        let _ = pg.drop_table(table_name, false).await;

        let query_id = "aaaa";
        pg.open_connection(query_id).await.unwrap();
        pg.execute_sql(
            query_id,
            format!(
                r#"
        CREATE TABLE {table_name} (
            col_time TIME,
            col_timestamp TIMESTAMP,
            col_timestamp_array TIMESTAMP[],
            col_date_array DATE[],
            col_time_array TIME[],
            col_timestamptz TIMESTAMPTZ,
            col_timestamptz_array TIMESTAMPTZ[]
        );
"#,
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert("col_time".to_string(), Some("10:00:00".to_string()));
        values.insert(
            "col_timestamp".to_string(),
            Some("2024-03-01 10:00:00".to_string()),
        );
        values.insert(
            "col_timestamp_array".to_string(),
            Some("{\"2024-03-01 10:00:00\",\"2024-03-02 12:00:00\"}".to_string()),
        );
        values.insert(
            "col_date_array".to_string(),
            Some("{2024-03-01,2024-03-02}".to_string()),
        );
        values.insert(
            "col_date_array".to_string(),
            Some("{2024-03-01,2024-03-02}".to_string()),
        );
        values.insert(
            "col_time_array".to_string(),
            Some("{10:00:00,12:00:00}".to_string()),
        );
        values.insert(
            "col_timestamptz".to_string(),
            Some("2024-03-01 10:00:00 +00:00".to_string()),
        );
        values.insert(
            "col_timestamptz_array".to_string(),
            Some("{\"2024-03-01 10:00:00 +00:00\",\"2024-03-02 12:00:00 +00:00\"}".to_string()),
        );

        pg.table_insert(table_name, &values).await.unwrap();

        let x = pg
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("col_time asc"),
            )
            .await
            .unwrap();
        println!("{:?}", x);

        GetTestExportTableKinds::test_export(&pg, table_name).await;
        pg.test_table_export_and_import(table_name).await;
        pg.test_table_equal(table_name, &values, &[]).await;
    }

    #[tokio::test]
    async fn test_types7() {
        //
        let table_name = "test_types7";
        let pg = make_test_object().await;
        let _ = pg.drop_table(table_name, false).await;

        let query_id = "aaaa";
        pg.open_connection(query_id).await.unwrap();
        pg.execute_sql(
            query_id,
            format!(
                r#"
        CREATE TABLE {table_name} (
            interval_col INTERVAL,
            interval_array_col INTERVAL[],
            numeric_array_col NUMERIC[],
            timetz_col TIMETZ,
            timetz_array_col TIMETZ[]
        );
"#,
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert(
            "interval_col".to_string(),
            Some("1 day 02:30:00".to_string()),
        );
        values.insert(
            "interval_array_col".to_string(),
            Some("{\"2 days\",\"03:00:00\",\"00:04:00\"}".to_string()),
        );
        values.insert(
            "numeric_array_col".to_string(),
            Some("{1.2300,4.5600,7.8900}".to_string()),
        );
        values.insert("timetz_col".to_string(), Some("12:30:00+03:00".to_string()));
        values.insert(
            "timetz_array_col".to_string(),
            Some("{10:00:00+03:00,15:30:00+03:00}".to_string()),
        );
        pg.table_insert(table_name, &values).await.unwrap();
        let x = pg
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("interval_col asc"),
            )
            .await
            .unwrap();
        println!("{:?}", x);

        GetTestExportTableKinds::test_export(&pg, table_name).await;
        pg.test_table_export_and_import(table_name).await;
        pg.test_table_equal(table_name, &values, &[]).await;
    }

    #[tokio::test]
    async fn test_types8() {
        let table_name = "test_types8";
        let pg = make_test_object().await;

        let _ = pg.drop_table(table_name, false).await;

        let query_id = "aaaa";
        pg.open_connection(query_id).await.unwrap();
        pg.execute_sql(
            query_id,
            format!(
                r#"
        CREATE TABLE {table_name}(
            bit_column BIT,
            bit_array_column BIT[],
            varbit_column VARBIT,
            varbit_array_column VARBIT[],
            numeric_column NUMERIC
        );
"#,
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert("bit_column".to_string(), Some("1".to_string()));
        values.insert("bit_array_column".to_string(), Some("{0,1}".to_string()));
        values.insert("varbit_column".to_string(), Some("101010".to_string()));
        values.insert(
            "varbit_array_column".to_string(),
            Some("{101,010}".to_string()),
        );

        values.insert("numeric_column".to_string(), Some("123.4500".to_string()));
        pg.table_insert(table_name, &values).await.unwrap();
        let x = pg
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test(None))
            .await
            .unwrap();
        println!("{:?}", x);

        GetTestExportTableKinds::test_export(&pg, table_name).await;
        pg.test_table_export_and_import(table_name).await;
        pg.test_table_equal(table_name, &values, &[]).await;
    }

    #[tokio::test]
    async fn test_types9() {
        let table_name = "test_types9";
        let pg = make_test_object().await;
        let _ = pg.drop_table(table_name, false).await;

        let query_id = "aaaa";
        pg.open_connection(query_id).await.unwrap();
        pg.execute_sql(
            query_id,
            format!(
                r#"
            CREATE TABLE {table_name} (
                uuid_column UUID,
                uuid_array_column UUID[],
                jsonb_column JSONB,
                int4range_column INT4RANGE,
                int4range_array_column INT4RANGE[]
            );
    "#,
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert(
            "uuid_column".to_string(),
            Some("550e8400-e29b-41d4-a716-446655440000".to_string()),
        );
        values.insert(
            "uuid_array_column".to_string(),
            Some(
                "{123e4567-e89b-12d3-a456-426655440000,987e6543-c21d-43a1-b876-446655440000}"
                    .to_string(),
            ),
        );
        values.insert(
            "jsonb_column".to_string(),
            Some(r#"{"key1":"value1","key2":"value2"}"#.to_string()),
        );
        values.insert("int4range_column".to_string(), Some("[10,20)".to_string()));
        values.insert(
            "int4range_array_column".to_string(),
            Some("{\"[30,40)\",\"[50,60)\"}".to_string()),
        );

        pg.table_insert(table_name, &values).await.unwrap();
        let x = pg
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("uuid_column asc"),
            )
            .await
            .unwrap();
        println!("{:?}", x);

        GetTestExportTableKinds::test_export(&pg, table_name).await;
        pg.test_table_export_and_import(table_name).await;
        pg.test_table_equal(table_name, &values, &[]).await;
    }

    #[tokio::test]
    async fn test_types10() {
        let table_name = "test_types10";
        let pg = make_test_object().await;
        let _ = pg.drop_table(table_name, false).await;
        let query_id = "aaaa";
        pg.open_connection(query_id).await.unwrap();
        pg.execute_sql(
            query_id,
            format!(
                r#"
            CREATE TABLE {table_name} (
                num_range NUMRANGE,
                num_range_array NUMRANGE[],
                ts_range TSRANGE,
                tstz_range TSTZRANGE
            );
    "#,
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert("num_range".to_string(), Some("[1,10)".to_string()));
        values.insert(
            "num_range_array".to_string(),
            Some(r#"{"[1,5)","[6,10]"}"#.to_string()),
        );
        values.insert(
            "ts_range".to_string(),
            Some(r#"["2024-01-01 00:00:00","2024-01-10 00:00:00")"#.to_string()),
        );
        values.insert(
            "tstz_range".to_string(),
            Some(r#"["2024-01-01 00:00:00 +00:00",2024-01-10 00:00:00 +00:00)"#.to_string()),
        );

        pg.table_insert(table_name, &values).await.unwrap();
        let x = pg
            .show_table_collect(
                table_name,
                ShowTableParameter::new_stupid_test("num_range asc"),
            )
            .await
            .unwrap();
        println!("{:?}", x);

        GetTestExportTableKinds::test_export(&pg, table_name).await;
        pg.test_table_export_and_import(table_name).await;
        pg.test_table_equal(table_name, &values, &[]).await;
    }

    #[tokio::test]
    async fn test_types11() {
        //
        let table_name = "test_types11";
        let pg = make_test_object().await;
        let _ = pg.drop_table(table_name, false).await;
        let query_id = "aaaa";
        pg.open_connection(query_id).await.unwrap();
        pg.execute_sql(
            query_id,
            format!(
                r#"
            CREATE TABLE {table_name} (
                id SERIAL PRIMARY KEY,
                date_range DATERANGE,
                date_range_array DATERANGE[],
                int8_range INT8RANGE,
                int8_range_array INT8RANGE[],
                -- json_path JSONPATH,
                -- json_path_array JSONPATH[],
                money_value MONEY,
                money_value_array MONEY[]
            );

    "#,
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        values.insert(
            "date_range".to_string(),
            Some("[2024-01-01,2024-01-11)".to_string()),
        );
        values.insert(
            "date_range_array".to_string(),
            Some("{\"[2024-01-01,2024-01-06)\",\"[2024-01-07,2024-01-10)\"}".to_string()),
        );
        values.insert("int8_range".to_string(), Some("[1,11)".to_string()));
        values.insert(
            "int8_range_array".to_string(),
            Some("{\"[1,6)\",\"[7,10)\"}".to_string()),
        );
        // values.insert(
        //     "json_path".to_string(),
        //     Some("$.\"store\".\"book\"[0].\"title\"".to_string()),
        // );
        // values.insert(
        //     "json_path_array".to_string(),
        //     Some("{\"$.\\\"store\\\".\\\"book\\\"[0].\\\"title\\\"\"}".to_string()),
        // );
        values.insert("money_value".to_string(), Some("$10.50".to_string()));
        values.insert(
            "money_value_array".to_string(),
            Some("{$5.00,$15.75}".to_string()),
        );
        pg.table_insert(table_name, &values).await.unwrap();
        let x = pg
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("{:?}", x);

        GetTestExportTableKinds::test_export(&pg, table_name).await;
        pg.test_table_export_and_import(table_name).await;
        pg.test_table_equal(table_name, &values, &["id", "money_value_array"])
            .await;
    }

    #[tokio::test]
    async fn test_foreign_key() {
        let db = make_test_object().await;

        let table_name_posts = "test_posts";
        let table_name_comments = "test_comments";

        // PG 默认的外键名格式通常是 {table}_{column}_fkey
        // 但在实际业务中，建议通过系统表查询拿到确切的名字，而不是瞎猜
        let _err = db
            .drop_foreign_key(
                table_name_comments,
                "fk_comments_posts", // PostgreSQL 默认生成的名字
            )
            .await;

        if let Err(_err) = _err {
            println!("drop foreign key failed, err: {}", _err);
        }

        let _ = db.drop_table(table_name_comments, false).await;
        let _ = db.drop_table(table_name_posts, false).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();

        // 1. 创建父表 test_posts
        // PG 使用 SERIAL 代替 AUTO_INCREMENT
        db.execute_sql(
            query_id,
            r#"
        CREATE TABLE IF NOT EXISTS test_posts (
            id SERIAL PRIMARY KEY,
            title VARCHAR(255) NOT NULL,
            content TEXT
        );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        // 2. 创建子表 test_comments
        // PG 的外键约束和 MySQL 类似，写在表级
        db.execute_sql(
            query_id,
            r#"
        CREATE TABLE IF NOT EXISTS test_comments (
            comment_id SERIAL PRIMARY KEY,
            author_name VARCHAR(255),
            post_id INT,
            CONSTRAINT fk_comments_posts FOREIGN KEY (post_id) 
            REFERENCES test_posts(id) 
            ON DELETE CASCADE
        );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.close_connection(query_id).await.unwrap();

        // 3. 列出外键并打印
        let f = db.list_foreign_key(table_name_comments).await.unwrap();

        assert!(f.len() == 1);

        for f in f.iter() {
            println!("foreign key: {:?}", f);
        }
    }

    #[ignore = "run by hand,right now I think linux can pass"]
    #[tokio::test]

    async fn test_ssl() {
        let port = portpicker::pick_unused_port().unwrap();
        println!("test port:{}", port);
        let cert_dir = cert_files_dir();
        // 2. 动态拼接数据卷挂载参数路径
        let ca_volume = format!("{}/ca.crt:/etc/postgresql/ca.crt", cert_dir);
        let cert_volume = format!("{}/server.crt:/etc/postgresql/server.crt", cert_dir);
        let key_volume = format!("{}/server.key:/etc/postgresql/server.key", cert_dir);

        // 3. 使用 std::process::Command 按顺序构建并执行命令
        let output = Command::new("docker")
            .arg("run")
            .arg("-d")
            .args(["--name", "postgres-ssl-test"])
            .args(["-p", format!("{}:5432", port).as_str()])
            .args(["-e", "POSTGRES_USER=testuser"])
            .args(["-e", "POSTGRES_PASSWORD=testpass"])
            .args(["-e", "POSTGRES_DB=testdb"])
            .args(["-v", &ca_volume])
            .args(["-v", &cert_volume])
            .args(["-v", &key_volume])
            .arg("postgres")
            .args(["-c", "ssl=on"])
            .args(["-c", "ssl_cert_file=/etc/postgresql/server.crt"])
            .args(["-c", "ssl_key_file=/etc/postgresql/server.key"])
            .args(["-c", "ssl_ca_file=/etc/postgresql/ca.crt"])
            .args(["-c", "log_connections=on"])
            .output()
            .unwrap(); // 捕获标准输出和错误输出

        // 4. 检查命令执行结果
        let container_id = if output.status.success() {
            let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("PostgreSQL 容器启动成功，ID 为: {}", container_id);
            container_id
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("启动 Docker 容器失败: {}", stderr);
            return;
        };
        let _guard = DockerContainerDropper::from(&container_id);

        tokio::time::sleep(Duration::from_secs(2)).await;

        start_docker_container(&container_id).unwrap();

        tokio::time::sleep(Duration::from_secs(2)).await;
        //  test ssl
        let config = PGDBConfig {
            options: Default::default(),
            host: "127.0.0.1".to_string(),
            port,
            username: "testuser".to_string(),
            password: "testpass".to_string().into(),
            db: Some("postgres".to_string()),
            schema: None,
            read_only: true,
            ssl: SslFilesWithStrModel {
                ssl_mode: "disable".to_string(),
                ssl_files: SSLFiles {
                    ssl_ca: None,
                    ssl_cert: None,
                    ssl_key: None,
                },
            },
            conn_timeout: Some(5000.into()),
        };
        let kind = BackendKindPostGres::PostGres;
        {
            let db = PostGresDB::new(config.clone(), kind).await.unwrap();
            db.check_conn().await.unwrap();
        }

        {
            let mut config = config.clone();
            config.ssl.ssl_mode = "require".to_string();
            let db = PostGresDB::new(config.clone(), kind).await.unwrap();
            db.check_conn().await.unwrap();
        }

        {
            let mut config = config.clone();
            config.ssl.ssl_mode = "verify-ca".to_string();
            config.ssl.ssl_files = SSLFiles::test_cert();
            println!("config:{:?}", config);

            let db = PostGresDB::new(config.clone(), kind).await.unwrap();
            db.check_conn().await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_type_regproc() {
        let db = make_test_object().await;

        let sql = r#"
                SELECT "oid","amname","amhandler","amtype" from "pg_catalog"."pg_am" ORDER BY "amhandler" ASC  OFFSET 0 LIMIT 5
"#;

        let mut conn: sqlx::pool::PoolConnection<sqlx::Postgres> = db.get_conn().await.unwrap();

        let stmt: sqlx::postgres::PgStatement = conn.prepare(sql.into_sql_str()).await.unwrap();

        for col in stmt.columns() {
            println!("!!!:{} {}", col.name(), col.type_info());
        }

        let rows = sqlx::query(sql.into_sql_str())
            .fetch_all(conn.deref_mut())
            .await
            .unwrap();

        for row in rows.iter() {
            for col in row.columns().iter() {
                println!("!!!:{} {}", col.name(), col.type_info());
            }
        }
    }
}
