use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use futures_util::{StreamExt, TryStreamExt};
use sqlx::pool::PoolConnection;
use sqlx::postgres::PgRow;
use sqlx::{Column, Executor, PgConnection, Postgres, Row};
use tokio::sync::Mutex;

use crate::datasource_config::PGDBConfig;
use crate::db::{
    BackendName, CatalogTrait, CheckConn, ColumnType, CreateDatabaseExtra, CreateSchemaExtra,
    CreateTableAndIndexSql, CreatedType, DBTrait, DefAndMetas, ExportKind, Field, HumanSize, Index,
    IndexKind, ListMaterializedView, ListTable, NamedStaff, ProcedureDetail, QueryResp, QuerySql,
    ReadOnlyTrait, SchemaTrait, ServerMetas, ShowExtraArgs, SqlExecuteRet, TableListResp,
    TriggerDetail, TriggerEvent, TriggerTiming, UdfOrProcedureExtraParameter, not_support_features,
};
use crate::helper::{create_csv, is_dangerous_sql_by_parser};
use crate::meta_value::MetaValue;
use crate::pg_format::*;
use crate::query_log::QueryLog;

/// postgres version of pg.
#[derive(Clone)]
pub struct PostGresDB {
    pool: sqlx::PgPool,
    pub(crate) config: PGDBConfig,
    pub(crate) query_connection:
        Arc<DashMap<uuid::Uuid, Arc<Mutex<PoolConnection<sqlx::Postgres>>>>>,
}

#[async_trait::async_trait]
impl CheckConn for PostGresDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let ins = std::time::Instant::now();
        let mut conn = self.get_conn().await?;
        sqlx::query("select 1").fetch_all(conn.deref_mut()).await?;
        anyhow::Ok(ins.elapsed())
    }
}
impl ReadOnlyTrait for PostGresDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}
#[async_trait::async_trait]
impl DBTrait for PostGresDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let sql = format!(
            r#"
        SELECT schema_name 
            FROM information_schema.schemata
            WHERE schema_name NOT IN ('information_schema', 'pg_catalog' , 'pg_toast') and 
            schemata.catalog_name ='{}' order by schema_name
    "#,
            self.config.db_str()
        );
        log::info!("sql:{}", sql);

        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;

        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.try_get(0)?;
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
        &self, name: &str, _extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("CREATE SCHEMA {name};");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("DROP SCHEMA {name};");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }
}

impl PostGresDB {
    async fn export_table_as_sql(
        &self, table_name: &str, include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            // let ss = DBTrait::create_table_and_index_sql(self,
            // table_name).await?; sender.send(Cow::Owned(ss.
            // create_table.clone())).await?;
            // sender.send(";\n".into()).await?;

            // for index in ss.indexes.into_iter() {
            //     sender.send(Cow::Owned(index)).await?;
            //     sender.send(";\n".into()).await?;
            // }
            anyhow::bail!("export create table script not supported now.")
        }

        let mut conn = self.get_conn().await?;
        let cols = SchemaTrait::show_table_columns(self, table_name).await?;
        let sql = format!("SELECT * from {}.{}", self.must_schema(), table_name);
        log::info!("sql:{}", sql);
        let mut stream = sqlx::query(sql.as_str())
            .fetch(conn.deref_mut())
            .try_chunks(100);
        while let Some(rows) = stream.next().await {
            let rows = rows?;

            let data = encode_rows(&rows)?;
            for d in data.into_iter() {
                let mut m = HashMap::new();
                for (d, f) in d.into_iter().zip(cols.iter()) {
                    m.insert(f.field.clone(), d);
                }

                let insert_sql = (self as &dyn SchemaTrait).insert_sql(
                    self.config.schema.as_ref().map(|x| x.as_str()),
                    table_name,
                    &cols,
                    &m,
                )?;
                sender.send(Cow::Owned(insert_sql)).await?;
                sender.send("\n".into()).await?;
            }
        }
        anyhow::Ok(())
    }

    async fn export_query_result_as_json_line(
        &self, sql: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;

        let mut stream = sqlx::query(sql).fetch(conn.deref_mut());

        while let Some(row) = stream.next().await {
            let row = row?;
            let value = encode_row_as_json(&row)?;
            sender
                .send(Cow::Owned(serde_json::to_string(&value)?))
                .await?;
            sender.send("\n".into()).await?;
        }

        anyhow::Ok(())
    }

    async fn export_table_as_json_line(
        &self, table_name: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(
            "SELECT * from \"{}\".\"{}\"",
            self.must_schema(),
            table_name
        );
        log::info!("sql:{}", sql);
        self.export_query_result_as_json_line(&sql, sender).await
    }

    async fn export_query_result_as_json(
        &self, sql: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let mut stream = sqlx::query(sql).fetch(conn.deref_mut());
        sender.send("[".into()).await?;
        let mut first = true;
        while let Some(row) = stream.next().await {
            let row = row?;
            let value = encode_row_as_json(&row)?;
            if !first {
                sender.send(",".into()).await?;
            }
            if pretty {
                sender.send("\n".into()).await?;
            }
            sender
                .send(Cow::Owned(if pretty {
                    serde_json::to_string_pretty(&value)?
                } else {
                    serde_json::to_string(&value)?
                }))
                .await?;
            first = false;
        }
        if pretty {
            sender.send("\n".into()).await?;
        }
        sender.send("]".into()).await?;
        anyhow::Ok(())
    }

    async fn export_table_as_json(
        &self, table_name: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(
            "SELECT * from \"{}\".\"{}\"",
            self.must_schema(),
            table_name
        );
        log::info!("sql:{}", sql);
        self.export_query_result_as_json(&sql, pretty, sender).await
    }

    async fn export_query_result_as_csv(
        &self, sql: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let mut first = true;

        let mut stream = sqlx::query(sql).fetch(conn.deref_mut()).try_chunks(100);

        while let Some(rows) = stream.next().await {
            let rows = rows?;
            if first && rows.len() > 0 && include_header {
                let fs = rows
                    .get(0)
                    .unwrap()
                    .columns()
                    .iter()
                    .map(|x| x.name().to_string())
                    .map(Some)
                    .collect::<Vec<_>>();
                sender
                    .send(Cow::Owned(create_csv(&vec![fs], delimiter)))
                    .await?;
                first = false
            }

            let data = encode_rows(&rows)?;
            sender
                .send(Cow::Owned(create_csv(&data, delimiter)))
                .await?;
        }
        anyhow::Ok(())
    }

    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from {}.{}", self.must_schema(), table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_csv(sql.as_str(), include_header, delimiter, sender)
            .await
    }
}

fn gen_show_sql(
    schema: &str, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize,
) -> String {
    let mut sql = format!("SELECT * from \"{schema}\".\"{table_name}\"");
    if let Some(where_str) = where_str {
        sql.push_str(format!(" where {where_str} ").as_str());
    }
    if let Some(order_by) = order_by {
        sql.push_str(format!(" ORDER BY {order_by} ").as_str());
    }
    let offset = (page - 1) * page_size;
    sql.push_str(format!(" OFFSET {} LIMIT {}", offset, page_size).as_str());

    sql
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

impl PostGresDB {
    pub(crate) fn must_schema(&self) -> &String {
        self.config.schema.as_ref().unwrap()
    }
}

fn type_support_compare(typ: &str) -> bool {
    match typ.to_uppercase().as_str() {
        "JSON" | "JSON[]" => false,
        "JSONPATH" | "JSONPATH[]" => false,
        _ => true,
    }
}
#[async_trait::async_trait]

impl ServerMetas for PostGresDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let mut conn = self.get_conn().await?;

        let mut ret = Vec::new();

        {
            let row = sqlx::query("SELECT version()")
                .fetch_one(conn.deref_mut())
                .await?;
            let version: String = row.try_get(0)?;
            ret.push(MetaValue::new("server version", version));
        }
        {
            let row = sqlx::query("SELECT current_user;")
                .fetch_one(conn.deref_mut())
                .await?;
            let version: String = row.try_get(0)?;
            ret.push(MetaValue::new("current user", version));
        }
        // TODO I don't know which settings is import
        // show them all
        {
            let sql = r#"
                SELECT name,setting,short_desc 
                FROM pg_settings
                ORDER BY name;
                "#;
            log::info!("sql:{}", sql);
            let rows = sqlx::query(sql).fetch_all(conn.deref_mut()).await?;
            for r in rows.iter() {
                let name: String = r.try_get(0)?;
                let setting: Option<String> = r.try_get(1)?;
                let desc: String = r.try_get(2)?;
                let mut meta = MetaValue::new_opt(name, setting);
                meta.with_desc(desc);
                ret.push(meta);
            }
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl SchemaTrait for PostGresDB {
    async fn list_created_types(&self) -> anyhow::Result<Vec<CreatedType>> {
        // TODO
        not_support_features(self.backend_name())
    }
    async fn drop_created_type(&self, typ: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP TYPE  "{}"."{typ}" "#, self.must_schema());
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;

        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    fn quote_style(&self) -> &'static str {
        "\""
    }

    async fn create_index(
        &self, table: &str, name: &str, kind: IndexKind, fields: &[String],
    ) -> anyhow::Result<()> {
        let sql = format!(
            "CREATE {} INDEX {} ON {table}({})",
            match kind {
                IndexKind::Index => "",
                IndexKind::Primary => anyhow::bail!("can't create primary index"),
                IndexKind::Unique => "Unique",
            },
            name,
            fields.join(",")
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<()> {
        let sql = format!("DROP INDEX \"{index}\";");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn rename_index(
        &self, _table: &str, old_name: &str, new_name: &str,
    ) -> anyhow::Result<()> {
        let sql = format!("ALTER INDEX \"{old_name}\" RENAME TO \"{new_name}\";");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn add_table_column(
        &self, table: &str, name: &str, col: ColumnType,
    ) -> anyhow::Result<()> {
        let sql = format!(
            "ALTER TABLE {table} ADD COLUMN \"{name}\" {} {} {}",
            col.typ,
            if let Some(nullness) = col.nullness.as_ref() {
                nullness.as_str()
            } else {
                ""
            },
            if let Some(d) = col.default_value.as_ref() {
                format!(
                    "DEFAULT {}",
                    self.put_value_in_sql(&col.typ, None, Some(d.as_str()))?
                )
            } else {
                "".to_owned()
            }
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::query(&sql)
            .execute(conn.deref_mut())
            .await
            .map_err(|e| anyhow::Error::msg(format!("err:{} sql:{}", e.to_string(), sql)))?;
        anyhow::Ok(())
    }

    fn list_data_types(&self) -> anyhow::Result<&'static [&'static str]> {
        anyhow::Ok(crate::pg_lang_server::TYPES)
    }

    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        if old_name != new_name {
            let sql = format!(
                r#"
            ALTER TABLE "{table}" RENAME COLUMN "{old_name}" TO "{new_name}";
            "#
            );
            sqlx::query(&sql).execute(conn.deref_mut()).await?;
            log::info!("sql:{}", sql);
        }
        if let Some(typ) = typ.as_ref() {
            let sql = format!(
                r#"
            ALTER TABLE {table} 
                    ALTER COLUMN "{new_name}" SET DATA TYPE {};
            "#,
                typ.typ
            );
            log::info!("sql:{}", sql);
            sqlx::query(&sql).execute(conn.deref_mut()).await?;
            if let Some(null) = typ.nullness.as_ref() {
                let sql = if null.trim().to_lowercase() == "null" {
                    format!(
                        r#"
                        ALTER TABLE {table} 
                            ALTER COLUMN {new_name} DROP NOT NULL;
                    "#
                    )
                } else {
                    format!(
                        r#"
                        ALTER TABLE {table} 
                            ALTER COLUMN {new_name} SET NOT NULL;
                    "#
                    )
                };
                log::info!("sql:{}", sql);
                sqlx::query(&sql).execute(conn.deref_mut()).await?;
            }
        }
        anyhow::Ok(())
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
            ALTER TABLE "{table}"
                DROP COLUMN "{col}";
        "#
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        use sqlx::{Executor, Statement};
        let mut conn = self.get_conn().await?;
        let sql = format!(r#"select * from "{view_name}" limit 1"#);
        log::info!("sql:{}", sql);
        let s = conn.deref_mut().prepare_with(&sql, &[]).await?;
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
            self.must_schema()
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
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

    async fn show_procedure(
        &self, name: &str, arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<ProcedureDetail> {
        let arg: &UdfOrProcedureExtraParameter = match arg.as_ref() {
            Some(arg) => arg,
            None => anyhow::bail!(format!("postgres need extra argument.")),
        };
        let parameters = match arg {
            UdfOrProcedureExtraParameter::PostGres { parameters } => parameters,

            _ => return Err(arg.wrong_type()),
        };
        let sql = format!(
            r#"
                SELECT 
                    pg_catalog.pg_get_function_result(p.oid) AS return_type,
                    pg_catalog.pg_get_function_arguments(p.oid) AS arguments,
                    p.prosrc AS source_code
                FROM pg_catalog.pg_proc p
                    JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
                WHERE n.nspname = '{}'  -- 指定目标模式
                AND p.proname = '{}'  -- 指定存储过程名称
                AND p.prokind = 'p'
                AND pg_catalog.pg_get_function_arguments(p.oid) = '{}'  -- 指定参数类型"#,
            self.must_schema(),
            name,
            parameters,
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let row = sqlx::query(&sql).fetch_one(conn.deref_mut()).await?;
        let return_type: Option<String> = row.try_get(0)?;
        let parameters: String = row.try_get(1)?;
        let code: String = row.try_get(2)?;

        anyhow::Ok(ProcedureDetail {
            def: code,
            metas: vec![
                MetaValue::new_opt("return_type", return_type),
                MetaValue::new("parameters", parameters),
            ],
        })
    }

    async fn drop_procedure(
        &self, name: &str, arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let arg: &UdfOrProcedureExtraParameter = match arg.as_ref() {
            Some(arg) => arg,
            None => anyhow::bail!(format!("postgres need extra argument.")),
        };
        let p = match arg {
            UdfOrProcedureExtraParameter::PostGres { parameters } => parameters,
            _ => return Err(arg.wrong_type()),
        };
        let sql = format!("DROP PROCEDURE {}({})", name, p);
        let mut conn = self.get_conn().await?;
        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
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
            self.must_schema()
        );
        log::info!("sql:{}", sql);
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            let return_type: Option<String> = r.get(1);
            let arguments: Option<String> = r.get(2);
            ret.push(NamedStaff {
                name,
                parameters: arguments,
                metas: vec![MetaValue::new_opt("return_type", return_type)],
                cassandra_parameters: None,
            });
        }
        anyhow::Ok(ret)
    }

    async fn show_udf(
        &self, name: &str, arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<DefAndMetas> {
        let arg = match &arg {
            Some(arg) => match arg {
                UdfOrProcedureExtraParameter::PostGres { parameters } => parameters,
                _ => return Err(arg.wrong_type()),
            },
            None => anyhow::bail!(format!("postgres need extra parameter")),
        };

        let sql = format!(
            r#"
            SELECT  
                pg_catalog.pg_get_function_result(p.oid) AS result_type,
                pg_catalog.pg_get_function_arguments(p.oid) AS arguments,
                pg_get_functiondef(p.oid) AS function_definition
            FROM pg_catalog.pg_proc p
                JOIN pg_catalog.pg_namespace n ON n.oid = p.pronamespace
            WHERE p.proname = '{name}' and n.nspname ='{}'  -- 替换为您要查询的函数名称
                 AND pg_catalog.pg_get_function_arguments(p.oid) = '{}'
       "#,
            self.must_schema(),
            arg
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let row = sqlx::query(&sql).fetch_one(conn.deref_mut()).await?;
        let result_type: Option<String> = row.try_get(0)?;
        let arguments: Option<String> = row.try_get(1)?;
        let function_definition: String = row.try_get(2)?;
        anyhow::Ok(DefAndMetas::new(function_definition, vec![
            MetaValue::new_opt("result_type", result_type),
            MetaValue::new_opt("parameters", arguments),
        ]))
    }

    async fn drop_udf(
        &self, name: &str, extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let parameters = match &extra {
            Some(x) => match x {
                UdfOrProcedureExtraParameter::PostGres { parameters } => parameters,
                #[allow(unreachable_patterns)]
                _ => {
                    anyhow::bail!("must be a postgres parameter")
                }
            },
            None => anyhow::bail!("postgres need extra parameters to drop udf."),
        };

        let sql = format!(
            "DROP FUNCTION IF EXISTS {}.{}({})",
            self.must_schema(),
            name,
            parameters
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn disable_trigger(&self, table: &str, name: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let sql = format!(r#"ALTER TABLE {table} DISABLE TRIGGER "{name}""#);
        log::info!("sql:{}", sql);
        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn enable_trigger(&self, table: &str, name: &str) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        let sql = format!(r#"ALTER TABLE {table} ENABLE TRIGGER "{name}""#);
        log::info!("sql:{}", sql);
        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        if self.table_exists(dup).await? {
            anyhow::bail!(format!("table '{}' already exists", dup));
        }
        let schema = self.must_schema();

        let sql = format!(r#"CREATE TABLE {schema}.{dup} AS TABLE "{schema}"."{table}""#);
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let schema = self.must_schema();
        let sql = format!(r#"ALTER TABLE "{schema}"."{old}" RENAME TO "{new}""#);
        log::info!("sql:{}", sql);

        let mut conn = self.get_conn().await?;
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
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
        let rows = sqlx::query(sql.as_str())
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
        let row = sqlx::query(sql.as_str())
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

    async fn drop_trigger(&self, table: &str, name: &str) -> anyhow::Result<()> {
        let schema = self.must_schema();
        let sql = format!(
            r#"
         DROP TRIGGER "{name}" ON "{schema}"."{table}" 
        "#
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn export_table_or_view(
        &self, name: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::CSV {
                include_header,
                delimiter,
            } => {
                self.export_table_as_csv(name, include_header, delimiter, sender)
                    .await
            }
            ExportKind::Json { pretty } => self.export_table_as_json(name, pretty, sender).await,
            ExportKind::JsonLine => self.export_table_as_json_line(name, sender).await,
            ExportKind::Sql {
                include_table_schema,
            } => {
                self.export_table_as_sql(name, include_table_schema, sender)
                    .await
            }
        }
    }

    /// FIXME 如果一个主键是自增的只能得到类型
    async fn create_table_and_index_sql(
        &self, table_name: &str,
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
        let row = sqlx::query(sql.as_str())
            .fetch_one(conn.deref_mut())
            .await?;
        let create_sql: String = row.try_get(0)?;
        let indexes = self.show_indexes(&table_name).await?;

        let mut sqls = Vec::with_capacity(indexes.len());
        for index in indexes.iter() {
            let index_sql =
                get_index_def(&mut conn, table_name, &index.name.as_ref().unwrap()).await?;
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
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<TableListResp> {
        let total = self.query_count(view_name, where_str).await?;

        let sql = gen_show_sql(
            self.must_schema(),
            view_name,
            where_str,
            order_by,
            page,
            page_size,
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await?;

        if rows.len() == 0 {
            let f = self.show_view_columns(view_name).await?;
            return anyhow::Ok(TableListResp::new(f, 0, vec![], sql));
        }

        let data = encode_rows(&rows)?;
        let fields = get_rows_types(&rows);
        anyhow::Ok(TableListResp::new(fields, total, data, sql))
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let schema = self.must_schema();

        let sql = format!(r#"DROP VIEW "{schema}"."{view_name}""#);
        log::info!("sql:{}", sql);
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let schema = self.must_schema();
        let mut conn = self.get_conn().await?;
        let sql = format!(
            r#"DROP TABLE "{schema}"."{table}" {}"#,
            if cascade { " CASCADE " } else { "" }
        );
        log::info!("sql:{}", sql);
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn list_view(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let mut conn = self.get_conn().await?;
        let schema = self.must_schema();
        let rows = sqlx::query(
            format!(
                r#"
        SELECT table_name
        FROM information_schema.tables
        WHERE table_type = 'VIEW'
          AND table_schema = '{schema}';
        "#
            )
            .as_str(),
        )
        .fetch_all(conn.deref_mut())
        .await?;

        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let view_name: String = r.get(0);
            ret.push(view_name);
        }
        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_table_columns(table_name).await?;
        let sql = (self as &dyn crate::db::SchemaTrait).delete_sql(
            self.config.schema.as_ref().map(|x: &String| x.as_str()),
            table_name,
            &fields,
            &wheres,
            type_support_compare,
        )?;
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let r = sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(r.rows_affected()),
        })
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let schema = self.must_schema();
        let s = format!(
            r#"
            TRUNCATE TABLE {schema}.{table_name};
        "#
        );
        let mut conn = self.get_conn().await?;
        sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    fn put_value_in_sql(
        &self, typ: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        put_value_in_sql(typ, value)
    }

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = (self as &dyn crate::db::SchemaTrait).update_sql(
            self.config.schema.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &values,
            &wheres,
            type_support_compare,
        )?;
        log::info!("update sql:{}", sql);
        let mut conn = self.get_conn().await?;

        let r = sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(r.rows_affected()),
        })
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;
        let s = (self as &dyn crate::db::SchemaTrait).insert_sql(
            self.config.schema.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &values,
        )?;

        let mut conn = self.get_conn().await?;

        let r = sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql: s,
            changes: Some(r.rows_affected()),
        })
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"SELECT table_name
             FROM information_schema.tables
             WHERE table_type = 'BASE TABLE' AND table_schema = '{}' and table_catalog='{}' ORDER BY table_name
     "#,
            self.must_schema(),
            &self.config.db_str()
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(sql.as_str())
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
            let row = sqlx::query(&sql).fetch_one(conn.deref_mut()).await?;
            let size: Option<i64> = row.get(0);
            let t = ListTable::new(table, size);
            ret.push(t);
        }

        anyhow::Ok(ret)
    }

    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListMaterializedView>> {
        let schema = self.must_schema();

        let sql = format!(
            r#"
            SELECT 
                matviewname,
                pg_total_relation_size(quote_ident(schemaname) || '.' || quote_ident(matviewname)) as size 
            FROM 
                pg_matviews where schemaname = '{}';
        "#,
            schema,
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            let size: Option<i64> = r.get(1);
            ret.push(ListMaterializedView {
                name,
                size: size.map(|x| HumanSize(Box::new(x))),
            });
        }
        anyhow::Ok(ret)
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let s = format!(
            r#"

            SELECT  column_name , data_type,is_nullable,column_default,udt_name
            FROM information_schema.columns where table_catalog='{}' and table_schema='{}' and table_name = '{}' ORDER BY ordinal_position
        "#,
            &self.config.db_str(),
            self.must_schema(),
            table_name
        );

        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(s.as_str())
            .fetch_all(conn.deref_mut())
            .await
            .unwrap();
        let mut ret = Vec::with_capacity(rows.len());
        let primary_keys = {
            let sql = format!(
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

            let rows = sqlx::query(sql.as_str())
                .fetch_all(conn.deref_mut())
                .await?;

            let mut keys = HashSet::new();
            for r in rows.iter() {
                let key: String = r.get(0);
                keys.insert(key);
            }
            keys
        };

        for r in rows.iter() {
            let name: String = r.get(0 as usize);
            let data_type: String = r.get(1);
            let data_type = data_type.to_uppercase();
            let nullable: String = r.get(2);
            let default_value: Option<String> = r.get(3);
            let udt_name: Option<String> = r.get(4);
            let is_primary = primary_keys.contains(&name);
            let is_auto_increment = data_type.to_lowercase().contains("int")
                && default_value
                    .as_ref()
                    .map(|x| x.contains("nextval"))
                    .unwrap_or(false);

            ret.push(Field::new(
                name,
                match udt_name {
                    Some(udt_name) => {
                        get_pg_type_base_on_data_type_and_udtname(&data_type, &udt_name)?
                    }
                    None => data_type,
                },
                nullable.to_lowercase() != "no",
                default_value,
                is_primary,
                is_auto_increment && is_primary,
            ));
        }

        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<TableListResp> {
        let total = self.query_count(table_name, where_str.clone()).await?;
        let sql = gen_show_sql(
            self.must_schema(),
            table_name,
            where_str,
            order_by,
            page,
            page_size,
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        let row = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await
            .map_err(|e| anyhow::Error::msg(format!("err:'{}' sql:'{}'", e, sql)))?;
        let data = encode_rows(&row)?;
        let fields = self.show_table_columns(table_name).await?;
        anyhow::Ok(TableListResp::new(fields, total, data, sql))
    }

    async fn drop_materialized_view(&self, name: &str) -> anyhow::Result<()> {
        let schema = self.must_schema();
        let sql = format!(
            r#"DROP MATERIALIZED VIEW  "{}"."{}" RESTRICT"#,
            schema, name
        );
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let mut conn = self.get_conn().await?;

        // this sql copied from https://stackoverflow.com/questions/2204058/list-columns-with-indexes-in-postgresql
        let rows = sqlx::query(
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
        )
        .fetch_all(conn.deref_mut())
        .await
        .unwrap();
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let index_name: String = r.get(1);
            let column_names: Option<Vec<String>> = r.get(2);
            let index_kind = get_index_kind(&mut conn, table_name, &index_name).await?;
            ret.push(Index::new(
                index_name,
                index_kind,
                column_names.unwrap_or(vec![]),
            ))
        }
        anyhow::Ok(ret)
    }
}

impl PostGresDB {
    async fn query_count(&self, name: &str, where_str: Option<&str>) -> anyhow::Result<usize> {
        let mut conn = self.get_conn().await?;
        let schema = self.must_schema();

        let mut sql = format!("SELECT count(1) from {schema}.{name} ");
        if let Some(where_str) = where_str {
            sql.push_str(format!(" where {where_str}").as_str());
        }
        let row = sqlx::query(sql.as_str())
            .fetch_one(conn.deref_mut())
            .await
            .map_err(|e| anyhow::Error::msg(format!("err:'{}' sql:'{}'", e, sql)))?;
        let count: i64 = row.get(0);
        anyhow::Ok(count as usize)
    }
}
impl std::fmt::Debug for PostGresDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "postgres on:{:?}", self.config)
    }
}

impl PostGresDB {
    pub async fn new(conn: PGDBConfig) -> anyhow::Result<Self> {
        let mut opts = sqlx::postgres::PgConnectOptions::new();
        opts = opts
            .host(&conn.host)
            .port(conn.port)
            .username(&conn.username);
        if let Some(pass) = &conn.password {
            opts = opts.password(&pass);
        }
        if let Some(db) = conn.db.as_ref() {
            opts = opts.database(&db);
        }

        let pool = sqlx::postgres::PgPool::connect_with(opts).await?;
        anyhow::Ok(Self {
            pool,
            config: conn,
            query_connection: Default::default(),
        })
    }

    pub(crate) async fn get_conn(&self) -> anyhow::Result<PoolConnection<Postgres>> {
        let mut conn = self.pool.acquire().await?;
        if let Some(s) = &self.config.schema {
            set_current_schema(conn.deref_mut(), s).await?;
        }
        return anyhow::Ok(conn);
    }
}

fn get_rows_types(rows: &Vec<PgRow>) -> Vec<Field> {
    if rows.len() == 0 {
        log::warn!("info type on empty rows");
        return vec![];
    }
    log::trace!("infer type by:{:?}", rows[0].columns());
    let mut ret = Vec::with_capacity(rows[0].columns().len());
    for c in rows[0].columns().iter() {
        ret.push(Field {
            field: c.name().to_string(),
            typ: c.type_info().to_string(),
            nullable: true,
            default_value: None,
            is_primary: false,
            is_auto_increment: false,
            sortable: true,
            firebird_binary_sub_type: None,
        });
    }
    ret
}

#[cfg(test)]
#[test]
fn test_output_bytes() {
    let x = output_bytes(&[98, 126]);
    assert_eq!(x, "\\x627E")
}

///
fn get_pg_type_base_on_data_type_and_udtname(
    data_type: &str, udt_name: &str,
) -> anyhow::Result<String> {
    if data_type.to_uppercase() == "ARRAY" {
        let x = match udt_name {
            "_macaddr8" => "MACADDR8[]".to_string(),
            "_bytea" => "BYTEA[]".to_string(),
            "_bool" => "BOOL[]".to_string(),
            "_name" => "NAME[]".to_string(),
            "_bpchar" => "BPCHAR[]".to_string(),
            "_int2" => "INT2[]".to_string(),
            "_int4" => "INT4[]".to_string(),
            "_text" => "TEXT[]".to_string(),
            "_varchar" => "VARCHAR[]".to_string(),
            "_int8" => "INT8[]".to_string(),
            "_path" => "PATH[]".to_string(),
            "_float4" => "FLOAT4[]".to_string(),
            "_float8" => "FLOAT8[]".to_string(),
            "_macaddr" => "MACADDR[]".to_string(),
            "_inet" => "INET[]".to_string(),
            "_timestamp" => "TIMESTAMP[]".to_string(),
            "_date" => "DATE[]".to_string(),
            "_time" => "TIME[]".to_string(),
            "_timestamptz" => "TIMESTAMPTZ[]".to_string(),
            "_interval" => "INTERVAL[]".to_string(),
            "_numeric" => "NUMERIC[]".to_string(),
            "_timetz" => "TIMETZ[]".to_string(),
            "_bit" => "BIT[]".to_string(),
            "_varbit" => "VARBIT[]".to_string(),
            "_uuid" => "UUID[]".to_string(),
            "_jsonb" => "JSONB[]".to_string(),
            "_int4range" => "INT4RANGE[]".to_string(),
            "_numrange" => "NUMRANGE[]".to_string(),
            "_tsrange" => "TSRANGE[]".to_string(),
            "_tstzrange" => "TSTZRANGE[]".to_string(),
            "_daterange" => "DATERANGE[]".to_string(),
            "_int8range" => "INT8RANGE[]".to_string(),
            "_jsonpath" => "JSONPATH[]".to_string(),
            "_money" => "MONEY[]".to_string(),
            _ => {
                log::error!("udtname '{}' not handled", udt_name);
                anyhow::bail!(format!("udtname '{}' not handled", udt_name));
            }
        };
        return Ok(x);
    }

    return Ok(data_type.to_string());
}

async fn set_current_schema(conn: &mut PgConnection, schema: &str) -> anyhow::Result<()> {
    sqlx::query(format!("SET search_path TO {schema},public").as_str())
        .execute(conn)
        .await?;
    anyhow::Ok(())
}

#[async_trait::async_trait]
impl QuerySql for PostGresDB {
    fn clone(&self) -> Box<dyn QuerySql> {
        Box::new(Clone::clone(self))
    }
    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        is_dangerous_sql_by_parser(sql, &sqlparser::dialect::PostgreSqlDialect {})
    }
    async fn export_query_result(
        &self, sql: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::CSV {
                include_header,
                delimiter,
            } => {
                self.export_query_result_as_csv(sql, include_header, delimiter, sender)
                    .await
            }
            ExportKind::Json { pretty } => {
                self.export_query_result_as_json(sql, pretty, sender).await
            }
            ExportKind::JsonLine => self.export_query_result_as_json_line(sql, sender).await,
            ExportKind::Sql {
                include_table_schema: _,
            } => ExportKind::cannot_query_result_export_as_sql(),
        }
    }

    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        let conn = self.get_conn().await?;

        self.query_connection.insert(id, Arc::new(Mutex::new(conn)));
        anyhow::Ok(id)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        let conn = self.get_conn().await?;
        self.query_connection.insert(id, Arc::new(Mutex::new(conn)));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        QueryLog::remove(&id);

        self.query_connection.remove(&id);
        anyhow::Ok(())
    }
    async fn execute_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let mut query_log = QueryLog::get_guard(query_id, sql);
        let conn = self.query_connection.get(&query_id);
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();

        let ret = match conn.execute(sqlx::raw_sql(sql)).await {
            Ok(x) => Ok(QueryResp::row_affected(x.rows_affected())),
            Err(err) => Err(err.to_string()),
        };
        query_log.set_success(ret.is_ok());
        anyhow::Ok(ret)
    }
    async fn query_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let mut query_log = QueryLog::get_guard(query_id, sql);
        let conn = self.query_connection.get(&query_id);
        let conn = match conn {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();

        let mut ret = Vec::new();
        let mut stream = sqlx::raw_sql(sql).fetch_many(conn.deref_mut());

        let mut rows = Vec::new();
        while let Some(item) = stream.next().await {
            let item = match item {
                Ok(x) => x,
                Err(err) => {
                    ret.push(Err(err.to_string()));
                    continue;
                }
            };
            match item {
                sqlx::Either::Left(row_aff) => {
                    if rows.len() > 0 {
                        let fields = get_rows_types(&rows);
                        let data = encode_rows(&rows)?;
                        ret.push(Ok(QueryResp::new(fields, data)));
                        rows.clear();
                    };
                    if row_aff.rows_affected() > 0 {
                        ret.push(Ok(QueryResp::row_affected(row_aff.rows_affected())));
                    }
                }
                sqlx::Either::Right(row) => {
                    rows.push(row);
                }
            }
        }
        drop(stream);
        query_log.set_success(ret.iter().all(|x| x.is_ok()));
        if ret.len() == 0 {
            use sqlx::Statement;
            let s = conn.deref_mut().prepare_with(&sql, &[]).await?;
            let field = s
                .columns()
                .iter()
                .map(|x| Field::from_name_type(x.name(), x.type_info().to_string()))
                .collect::<Vec<_>>();
            ret = vec![Ok(QueryResp::new(field, vec![]))];
        }
        anyhow::Ok(ret)
    }
}

async fn get_index_def(
    conn: &mut PoolConnection<Postgres>, table_name: &str, index_name: &str,
) -> anyhow::Result<String> {
    let sql = format!(
        "SELECT indexdef from pg_indexes where tablename='{table_name}' and indexname='{index_name}'"
    );
    log::info!("sql:{}", sql);
    let row = sqlx::query(sql.as_str())
        .fetch_one(conn.deref_mut())
        .await?;

    let sql: String = row.get(0);

    anyhow::Ok(sql)
}

async fn get_index_kind(
    conn: &mut PoolConnection<Postgres>, table_name: &str, index_name: &str,
) -> anyhow::Result<IndexKind> {
    let sql: String = get_index_def(conn, table_name, index_name).await?;
    let ss = sqlparser::parser::Parser::parse_sql(
        &sqlparser::dialect::PostgreSqlDialect {},
        sql.as_str(),
    )?;
    if ss.len() != 1 {
        anyhow::bail!("parse create index should contains one statement");
    }
    let s = &ss[0];
    use sqlparser::ast::Statement;

    match s {
        Statement::CreateIndex(create_index) => {
            if create_index.unique {
                anyhow::Ok(IndexKind::Unique)
            } else {
                anyhow::Ok(IndexKind::Index)
            }
        }

        _ => {
            anyhow::bail!(format!("the sql is not a create index"))
        }
    }
}

impl BackendName for PostGresDB {
    fn backend_name(&self) -> &str {
        "postgres"
    }
}

#[async_trait::async_trait]

impl CatalogTrait for PostGresDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(
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
        let x = Self::new(config).await?;
        anyhow::Ok(Arc::new(x))
    }

    async fn create_database(
        &self, name: &str, _extra: Option<CreateDatabaseExtra>,
    ) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("CREATE DATABASE {name}");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn drop_database(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("DROP DATABASE {name}");
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn().await?;
        sqlx::query(&sql).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }
}

#[cfg(test)]
pub(crate) async fn make_test_object() -> PostGresDB {
    let pg = PostGresDB::new(PGDBConfig {
        host: crate::TEST_HOST.to_string(),
        port: 5432,
        username: "postgres".to_string(),
        password: Some("root".to_string()),
        db: Some("test".into()),
        schema: Some("xxx".to_string()),
        read_only: false,
    })
    .await
    .unwrap();
    pg
}
#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::make_test_object;
    use crate::db::{
        CatalogTrait, CheckConn, ColumnType, ExportKind, QuerySql, SchemaTrait, ServerMetas,
        UdfOrProcedureExtraParameter,
    };
    use crate::enable_test_log;
    // #[tokio::test]
    // async fn test_list_created_types() {
    //     let db = make_test_object().await;
    //     let _ = db.drop_created_type("test_type").await;
    //     let query_id = db.open_connection().await.unwrap();

    //     let _resp = db
    //         .query_sql(
    //             query_id,
    //             r#"
    //     CREATE TYPE test_type AS ENUM ('value1', 'value2', 'value3');"#,
    //         )
    //         .await
    //         .unwrap();
    //     assert!(_resp.iter().all(|x| x.is_ok()), "resp:{:?}", _resp);
    //     let list = db.list_created_types().await.unwrap();
    //     for l in list.iter() {
    //         println!("create type:{:?}", l);
    //     }
    // }

    #[tokio::test]
    async fn test_procedure() {
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_procedure("test_procedure", None).await;
        db.query_sql(
            query_id,
            r#"
            CREATE PROCEDURE test_procedure()
            LANGUAGE plpgsql AS
            $$
            BEGIN
                -- procedure logic goes here
            END;
            $$;
        "#,
        )
        .await
        .unwrap();
        let list = db.list_procedure().await.unwrap();
        for l in list.iter() {
            let details = db
                .show_procedure(
                    &l.name,
                    Some(UdfOrProcedureExtraParameter::PostGres {
                        parameters: "".to_string(),
                    }),
                )
                .await
                .unwrap();
            println!("udf:{},details:{:?}", l.name, details);
        }
    }

    #[tokio::test]
    async fn test_list_database() {
        let db = make_test_object().await;
        let dbs = CatalogTrait::list_database(&db).await.unwrap();
        println!("{:?}", dbs);
    }
    #[tokio::test]
    async fn test_materialized_view() {
        enable_test_log();
        let db = make_test_object().await;
        let _ = db.drop_materialized_view("test_material_view").await;
        let _ = db.drop_table("test_material_view_table", true).await;

        let query_id = db.open_connection().await.unwrap();
        let query_resp = db
            .query_sql(
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
            .unwrap();
        assert!(
            query_resp.iter().all(|x| x.is_ok()),
            "result:{:?}",
            query_resp
        );
        let list = db.list_materialized_view().await.unwrap();
        for l in list.iter() {
            let table_resp = db
                .show_view(&l.name, None, None, 1, 100, None)
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

    #[tokio::test]

    async fn test_rename() {
        let db = make_test_object().await;

        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_table("test_rename", false).await;
        let _ = db.drop_table("test_rename2", false).await;
        db.query_sql(
            query_id,
            r#"
                CREATE TABLE test_rename (
                    
                    name VARCHAR(255) NOT NULL,
                    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT CURRENT_TIMESTAMP
                );"#,
        )
        .await
        .unwrap();
        db.rename_table("test_rename", "test_rename2")
            .await
            .unwrap();

        db.show_table("test_rename2", None, None, 1, 100, None)
            .await
            .unwrap();
    }

    #[tokio::test]

    async fn test_duplicate() {
        let db = make_test_object().await;

        let _ = db.drop_table("test_duplicate", false).await;
        let _ = db.drop_table("test_duplicate2", false).await;
        let query_id = db.open_connection().await.unwrap();

        db.query_sql(
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
        .unwrap();
        db.duplicate_table("test_duplicate", "test_duplicate2")
            .await
            .unwrap();

        let resp = db
            .show_table("test_duplicate2", None, None, 1, 100, None)
            .await
            .unwrap();
        assert_eq!(resp.data.len(), 5);
    }

    #[tokio::test]
    async fn test_export() {
        // init_log();
        let table_name = "test_export";
        let db = make_test_object().await;

        let sql = r#"drop table if exists test_export; 
                CREATE TABLE test_export (
                
                name VARCHAR(50),
                age INT,
                email VARCHAR(100)
            ); 
             INSERT INTO test_export (name, age, email) VALUES ('John', 25, 'john@example.com');
             INSERT INTO test_export (name, age, email) VALUES ('Jane', 30, 'jane@example.com');
             INSERT INTO test_export (name, age, email) VALUES ('Bob', 40, 'bob@example.com');
             "#;
        let query_id = db.open_connection().await.unwrap();

        let db = &db as &dyn SchemaTrait;
        db.query_sql(query_id, sql).await.unwrap();

        {
            let s = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", s);
            let _: serde_json::Value = serde_json::from_str(s.as_str()).unwrap();
        }
        {
            let s = db
                .export_table_to_string(table_name, ExportKind::CSV {
                    include_header: true,
                    delimiter: ',',
                })
                .await
                .unwrap();
            println!("csv:{}", s);
        }
        {
            let s = db
                .export_table_to_string(table_name, ExportKind::Sql {
                    include_table_schema: false,
                })
                .await
                .unwrap();
            println!("sql:{}", s);
        }
        // {
        //     let s = db
        //         .export_table_to_string(
        //             table_name,
        //             ExportKind::Sql {
        //                 include_table_schema: true,
        //             },
        //         )
        //         .await
        //         .unwrap();
        //     println!("sql:{}", s);
        // }
    }

    #[tokio::test]
    async fn test_show_index() {
        let table_name = "index_test";
        let db = make_test_object().await;

        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();

        db.query_sql(query_id, format!(
            r#" create table {table_name} (a int, b int, c int, constraint pk_test primary key(a, b)); "#,
        ).as_str()).await .unwrap()
        ;

        let db = &db as &dyn crate::db::SchemaTrait;
        let indexes = db.show_indexes(table_name).await.unwrap();
        println!("{:?}", indexes);
    }

    #[tokio::test]
    async fn test_list_table() {
        // list all table and query
        let pg = make_test_object().await;

        let pg = &pg as &dyn SchemaTrait;
        let table = pg.list_table().await.unwrap();
        println!("{:?}", table);
    }

    #[tokio::test]
    async fn test_udf() {
        let pg = make_test_object().await;
        let query_id = pg.open_connection().await.unwrap();
        pg.query_sql(
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
        .unwrap();
        let list = pg.list_udf().await.unwrap();
        for l in list.iter() {
            // I created some other functions to test.
            // so we only test on `test_udf`.
            if l.name != "test_udf" {
                continue;
            }
            println!("udf:{:?}", l);
            let details = pg
                .show_udf(
                    &l.name,
                    Some(UdfOrProcedureExtraParameter::PostGres {
                        parameters: "a integer, b integer".to_string(),
                    }),
                )
                .await
                .unwrap();
            println!("name:{} , udf:{:?}", l.name, details);
            if l.name == "test_udf" {
                pg.drop_udf(
                    &l.name,
                    Some(UdfOrProcedureExtraParameter::PostGres {
                        parameters: "a integer, b integer".to_string(),
                    }),
                )
                .await
                .unwrap();
            }
        }
    }

    #[tokio::test]
    async fn test_triggers() {
        let db = make_test_object().await;

        //
        let _ = db.drop_table("test_create_trigger", false).await;
        let query_id = db.open_connection().await.unwrap();
        db.query_sql(
            query_id,
            r#"
          CREATE TABLE test_create_trigger (
            
            name VARCHAR(50)
          );
        "#,
        )
        .await
        .unwrap();

        let _ = db.drop_table("test_create_trigger_update", false).await;

        db.query_sql(
            query_id,
            r#"
            CREATE TABLE test_create_trigger_update (
                
                name VARCHAR(50),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            );
    "#,
        )
        .await
        .unwrap();

        db.query_sql(
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
        .unwrap();

        db.query_sql(
            query_id,
            r#"
            CREATE TRIGGER trigger_insert_test_create_trigger
            AFTER INSERT ON test_create_trigger
            FOR EACH ROW
            EXECUTE FUNCTION trigger_insert_test_create_trigger();
    "#,
        )
        .await
        .unwrap();

        let triggers = db.list_trigger("test_create_trigger").await.unwrap();
        for tri in triggers.iter() {
            let detail = db
                .show_trigger("test_create_trigger", tri.name.as_str())
                .await
                .unwrap();

            // disable trigger and enable trigger test.
            db.disable_trigger("test_create_trigger", &tri.name)
                .await
                .unwrap();
            db.enable_trigger("test_create_trigger", &tri.name)
                .await
                .unwrap();

            println!("details:{:?}", detail);
        }
    }
    #[tokio::test]
    async fn test_epxort() {
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        let table_name = "test_export_table";

        let _ = db.drop_table(table_name, false).await;

        db.query_sql(
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
        .unwrap();
        let schema_trait = &db as &dyn SchemaTrait;
        {
            let e = schema_trait
                .export_table_to_string(&table_name, ExportKind::CSV {
                    include_header: true,
                    delimiter: ',',
                })
                .await
                .unwrap();
            println!("csv:{}", e);
        }
        {
            let e = schema_trait
                .export_table_to_string(&table_name, ExportKind::Sql {
                    include_table_schema: false,
                })
                .await
                .unwrap();
            println!("sql:{}", e);
        }
        {
            let e = schema_trait
                .export_table_to_string(&table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            let _: serde_json::value::Value = serde_json::from_str(e.as_str()).unwrap();

            println!("json:{}", e);
        }
        {
            let e = schema_trait
                .export_table_to_string(&table_name, ExportKind::JsonLine)
                .await
                .unwrap();

            println!("json line:{}", e);
        }

        {
            let e = schema_trait
                .export_query_result_to_string("select * from test_export_table", ExportKind::CSV {
                    include_header: true,
                    delimiter: ',',
                })
                .await
                .unwrap();
            println!("csv:{}", e);
        }

        {
            let e = schema_trait
                .export_query_result_to_string(
                    "select * from test_export_table",
                    ExportKind::Json { pretty: true },
                )
                .await
                .unwrap();
            let _: serde_json::value::Value = serde_json::from_str(e.as_str()).unwrap();

            println!("json:{}", e);
        }
        {
            let e = schema_trait
                .export_query_result_to_string(
                    "select * from test_export_table",
                    ExportKind::JsonLine,
                )
                .await
                .unwrap();

            println!("json line:{}", e);
        }
    }

    #[tokio::test]
    async fn test_types1() {
        enable_test_log();
        let table_name = "test_types1";
        let pg = make_test_object().await;

        let _ = pg.drop_table(&table_name, false).await;

        let query_id = pg.open_connection().await.unwrap();
        pg.query_sql(
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
        .unwrap();

        // sqlx::query(
        //     r#"
        //     INSERT INTO test_types1 (bool_column, bytea_column, char_column,
        // name_column, int8_column, int2_column, int4_column)     VALUES
        //         (TRUE, E'\\x0123456789ABCDEF', 'A', 'John', 1000000000, 10, 100),
        //         (FALSE, E'\\x00FF00FF', 'B', 'Jane', -500000000, 20, -200),
        //         (TRUE, E'\\x00', 'C', 'Mike', 0, 30, 0);
        // "#,
        // ).execute(conn.deref_mut() ).await.unwrap();
        let pg = &pg as &dyn SchemaTrait;
        let mut values = HashMap::new();
        values.insert("bool_column".to_string(), Some("true".to_string()));
        values.insert(
            "bytea_column".to_string(),
            Some("\\\\x0123456789ABCDEF".to_string()),
        );
        values.insert("char_column".to_string(), Some("A".to_string()));
        values.insert("name_column".to_string(), Some("John".to_string()));
        values.insert("int8_column".to_string(), Some("1".to_string()));
        values.insert("int2_column".to_string(), Some("2".to_string()));
        values.insert("int4_column".to_string(), Some("100".to_string()));
        pg.insert(table_name, values.clone()).await.unwrap();
        let mut wheres = values.clone();
        wheres.insert("id".to_string(), Some("1".to_string()));

        let json = pg
            .export_table_to_string("test_types1", ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(json.as_str()).unwrap();

        println!("export json:{}", json);
    }

    #[tokio::test]
    async fn test_types2() {
        enable_test_log();
        let table_name = "test_types2";

        let pg = make_test_object().await;

        let _ = pg.drop_table(&table_name, false).await;

        let query_id = pg.open_connection().await.unwrap();
        pg.query_sql(
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
        .unwrap();

        // sqlx::query(
        //     r#"
        //             INSERT INTO test_types2 (text_column, oid_column, json_column,
        // float4_column, float8_column, macaddr8_column, macaddr8_array_column)
        // VALUES
        //     ('Sample text', 12345, '{"key": "value"}', 3.14, 5.6789,
        // '08:00:2b:01:02:03:04:05', ARRAY['08:00:2b:01:02:03:04:06',
        // '08:00:2b:01:02:03:04:07']::MACADDR8[]);         "#,
        // )
        // .execute(conn.deref_mut())
        // .await
        // .unwrap();

        let mut values = HashMap::new();
        values.insert("text_column".to_string(), Some("Sample text".to_string()));
        values.insert("oid_column".to_string(), Some("12345".to_string()));
        values.insert(
            "json_column".to_string(),
            Some("{\"key\": \"value\"}".to_string()),
        );
        values.insert("float4_column".to_string(), Some("3.14".to_string()));
        values.insert("float8_column".to_string(), Some("5.6789".to_string()));
        values.insert(
            "macaddr8_column".to_string(),
            Some("08:00:2b:01:02:03:04:05".to_string()),
        );
        values.insert(
            "macaddr8_array_column".to_string(),
            Some("{08:00:2b:01:02:03:04:06, 08:00:2b:01:02:03:04:07}".to_string()),
        );
        let pg = &pg as &dyn SchemaTrait;
        pg.insert(table_name, values).await.unwrap();

        let x = pg
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("{:?}", x);
        let json = pg
            .export_table_to_string(&table_name, ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(json.as_str()).unwrap();

        println!("export json:{}", json);
    }

    #[tokio::test]
    async fn test_table_column() {
        let db = make_test_object().await;

        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;

        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_column
                (
                    id INT PRIMARY KEY                     -- 主键，无符号32位整数
                )
                "#,
            )
            .await
            .unwrap();

        db.add_table_column(table_name, "name", ColumnType {
            typ: "INT2".to_string(),
            nullness: Some("null".to_string()),
            after: None,
            default_value: Some("1".to_string()),
        })
        .await
        .unwrap();

        db.modify_table_column(
            table_name,
            "name",
            "name2",
            Some(ColumnType {
                typ: "INT4".to_string(),
                nullness: Some("null".to_string()),
                after: None,
                default_value: Some("2".to_string()),
            }),
        )
        .await
        .unwrap();

        db.drop_table_column(table_name, "name2").await.unwrap();
    }

    #[tokio::test]
    async fn test_list_schema() {
        let pg = make_test_object().await;

        let tr = &pg as &dyn crate::db::DBTrait;
        let _ = tr.create_schema("abc", None).await;

        let resp = tr.list_schema().await.unwrap();
        assert_eq!(resp, vec!["abc", "public", "xxx"]);
        tr.drop_schema("abc").await.unwrap();
    }
    #[tokio::test]
    async fn test_types3() {
        enable_test_log();
        let pg = make_test_object().await;
        let table_name = "test_types3";

        let _ = pg.drop_table(&table_name, false).await;

        let query_id = pg.open_connection().await.unwrap();
        pg.query_sql(
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
        .unwrap();

        let pg = &pg as &dyn SchemaTrait;

        let mut values = HashMap::new();
        values.insert(
            "macaddr_column".to_string(),
            Some("08:00:2b:01:02:03".to_string()),
        );
        values.insert("inet_column".to_string(), Some("192.168.0.1".to_string()));
        values.insert(
            "bool_array_column".to_string(),
            Some("{true , false }".to_string()),
        );
        values.insert(
            "bytea_array_column".to_string(),
            Some("{0x12345}".to_string()),
        );
        values.insert(
            "char_array_column".to_string(),
            Some("{a  , b }".to_string()),
        );
        values.insert(
            "name_array_column".to_string(),
            Some("{ a  , b }".to_string()),
        );

        pg.insert(table_name, values).await.unwrap();

        let x = pg
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("{:?}", x);
        let json = pg
            .export_table_to_string(&table_name, ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(json.as_str()).unwrap();

        println!("export json:{}", json);
    }

    #[tokio::test]
    async fn test_types4() {
        enable_test_log();

        let table_name = "test_types4";
        let pg = make_test_object().await;

        let _ = pg.drop_table(&table_name, false).await;

        let query_id = pg.open_connection().await.unwrap();
        pg.query_sql(
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
        .unwrap();

        //         sqlx::query(
        //             r#"

        //             INSERT INTO test_types4 (int2_array_column, int4_array_column,
        // text_array_column, varchar_array_column, int8_array_column) VALUES
        //     (ARRAY[1, 2, 3], ARRAY[4, 5, 6], ARRAY['A', 'B', 'C'], ARRAY['John',
        // 'Jane', 'Bob'], ARRAY[7, 8, 9]);         "#,
        //         ).execute(conn.deref_mut()).await.unwrap();
        let mut values = HashMap::new();
        values.insert("int2_array_column".to_string(), Some("{1,2}".to_string()));
        values.insert("int4_array_column".to_string(), Some("{1,2}".to_string()));
        values.insert("text_array_column".to_string(), Some("{a,b}".to_string()));
        values.insert(
            "varchar_array_column".to_string(),
            Some("{c,d}".to_string()),
        );
        values.insert("int8_array_column".to_string(), Some("{1,100}".to_string()));
        let pg = &pg as &dyn SchemaTrait;
        pg.insert(table_name, values).await.unwrap();

        let x = pg
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("{:?}", x);
        let json = pg
            .export_table_to_string(&table_name, ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(json.as_str()).unwrap();

        println!("export json:{}", json);
    }

    #[tokio::test]
    async fn test_types5() {
        enable_test_log();
        let table_name = "test_types5";
        let pg = make_test_object().await;

        let _ = pg.drop_table(&table_name, false).await;

        let query_id = pg.open_connection().await.unwrap();
        pg.query_sql(
            query_id,
            format!(
                r#"
        CREATE TABLE {table_name} (
            id SERIAL PRIMARY KEY,
            path_column PATH,
            path_array_column PATH[],
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
        .unwrap();

        //         sqlx::query(
        //             r#"

        //             INSERT INTO test_types5 (path_column, path_array_column,
        // float4_array_column, float8_array_column, macaddr_array_column,
        // inet_array_column, date_column) VALUES
        //     ('(1.0,2.0),(3.0,4.0)'::PATH, ARRAY['(5.0,6.0),(7.0,8.0)'::PATH,
        // '(9.0,10.0),(11.0,12.0)'::PATH],         ARRAY[1.23, 4.56, 7.89],
        // ARRAY[12.34, 56.78, 90.12],         ARRAY['08:00:2b:01:02:03'
        // ::MACADDR, '08:00:2b:04:05:06'::MACADDR],         ARRAY['192.168.0.1'
        // ::INET, '10.0.0.1'::INET], '2022-01-01'::DATE);         "#,
        //         ).execute(conn.deref_mut()).await.unwrap();
        let pg = &pg as &dyn SchemaTrait;

        let mut values = HashMap::new();
        values.insert(
            "path_column".to_string(),
            Some("(1.0,2.0),(3.0,4.0)".to_string()),
        );
        values.insert(
            "path_array_column".to_string(),
            Some("{\"((5,6),(7,8))\",\"((9,10),(11,12))\"}".to_string()),
        );
        values.insert("float4_array_column".to_string(), Some("{1,2}".to_string()));
        values.insert("float8_array_column".to_string(), Some("{3,4}".to_string()));

        values.insert(
            "macaddr_array_column".to_string(),
            Some("{01:02:03:04:05:06}".to_string()),
        );
        values.insert(
            "inet_array_column".to_string(),
            Some("{192.168.1.1}".to_string()),
        );

        values.insert("date_column".to_string(), Some("2022-01-01".to_string()));

        pg.insert(table_name, values).await.unwrap();

        let x = pg
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("{:?}", x);
        // let json = pg
        //     .export_table_to_string(&table_name, ExportKind::Json { pretty:
        // true })     .await
        //     .unwrap();
        // let _: serde_json::Value =
        // serde_json::from_str(json.as_str()).unwrap();

        // println!("export json:{}", json);
    }

    #[tokio::test]
    async fn test_types6() {
        enable_test_log();
        let table_name = "test_types6";
        let pg = make_test_object().await;

        let _ = pg.drop_table(&table_name, false).await;

        let query_id = pg.open_connection().await.unwrap();
        pg.query_sql(
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
        .unwrap();

        let pg = &pg as &dyn SchemaTrait;

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
            Some("2024-03-01 10:00:00+00".to_string()),
        );
        values.insert(
            "col_timestamptz_array".to_string(),
            Some("{\"2024-03-01 10:00:00+00\",\"2024-03-02 12:00:00+00\"}".to_string()),
        );

        pg.insert(table_name, values).await.unwrap();

        let x = pg
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("{:?}", x);
        let json = pg
            .export_table_to_string(&table_name, ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(json.as_str()).unwrap();

        println!("export json:{}", json);
    }

    #[tokio::test]
    async fn test_types7() {
        enable_test_log();
        let table_name = "test_types7";
        let pg = make_test_object().await;
        let _ = pg.drop_table(&table_name, false).await;

        let query_id = pg.open_connection().await.unwrap();
        pg.query_sql(
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
        .unwrap();

        let pg = &pg as &dyn SchemaTrait;

        let mut values = HashMap::new();
        values.insert(
            "interval_col".to_string(),
            Some("1 day 02:30:00".to_string()),
        );
        values.insert(
            "interval_array_col".to_string(),
            Some("{\"2 days\",03:00:00,00:04:00}".to_string()),
        );
        values.insert(
            "numeric_array_col".to_string(),
            Some("{1.23,4.56,7.89}".to_string()),
        );
        values.insert("timetz_col".to_string(), Some("12:30:00+03".to_string()));
        values.insert(
            "timetz_array_col".to_string(),
            Some("{10:00:00+03,15:30:00+03}".to_string()),
        );
        pg.insert(table_name, values).await.unwrap();
        let x = pg
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("{:?}", x);

        let json = pg
            .export_table_to_string(&table_name, ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(json.as_str()).unwrap();

        println!("export json:{}", json);
    }

    #[tokio::test]
    async fn test_types8() {
        enable_test_log();
        let table_name = "test_types8";
        let pg = make_test_object().await;

        let _ = pg.drop_table(&table_name, false).await;

        let query_id = pg.open_connection().await.unwrap();
        pg.query_sql(
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
        .unwrap();

        let pg = &pg as &dyn SchemaTrait;

        let mut values = HashMap::new();
        values.insert("bit_column".to_string(), Some("1".to_string()));
        values.insert("bit_array_column".to_string(), Some("{0,1}".to_string()));
        values.insert("varbit_column".to_string(), Some("101010".to_string()));
        values.insert(
            "varbit_array_column".to_string(),
            Some("{101,010}".to_string()),
        );

        values.insert("numeric_column".to_string(), Some("123.45".to_string()));
        pg.insert(table_name, values).await.unwrap();
        let x = pg
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("{:?}", x);
        let json = pg
            .export_table_to_string(&table_name, ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(json.as_str()).unwrap();

        println!("export json:{}", json);
    }

    #[tokio::test]
    async fn test_types9() {
        enable_test_log();
        let table_name = "test_types9";
        let pg = make_test_object().await;
        let _ = pg.drop_table(&table_name, false).await;

        let query_id = pg.open_connection().await.unwrap();
        pg.query_sql(
            query_id,
            format!(
                r#"
            CREATE TABLE {table_name} (
                uuid_column UUID,
                uuid_array_column UUID[],
                jsonb_column JSONB,
                jsonb_array_column JSONB[],
                int4range_column INT4RANGE,
                int4range_array_column INT4RANGE[]
              );
    "#,
            )
            .as_str(),
        )
        .await
        .unwrap();

        let pg = &pg as &dyn SchemaTrait;

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
            Some("{\"key1\": \"value1\", \"key2\": \"value2\"}".to_string()),
        );
        values.insert(
            "jsonb_array_column".to_string(),
            Some(
                "{\"{\\\"key3\\\": \\\"value3\\\"}\",\"{\\\"key4\\\": \\\"value4\\\"}\"}"
                    .to_string(),
            ),
        );
        values.insert("int4range_column".to_string(), Some("[10,20)".to_string()));
        values.insert(
            "int4range_array_column".to_string(),
            Some("{\"[30,40)\",\"[50,60)\"}".to_string()),
        );

        pg.insert(table_name, values).await.unwrap();
        let x = pg
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("{:?}", x);
        let json = pg
            .export_table_to_string(&table_name, ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(json.as_str()).unwrap();

        println!("export json:{}", json);
    }

    #[tokio::test]
    async fn test_types10() {
        enable_test_log();
        let table_name = "test_types10";
        let pg = make_test_object().await;

        let _ = pg.drop_table(&table_name, false).await;

        let query_id = pg.open_connection().await.unwrap();
        pg.query_sql(
            query_id,
            format!(
                r#"
            CREATE TABLE {table_name} (
                num_range NUMRANGE,
                num_range_array NUMRANGE[],
                ts_range TSRANGE,
                ts_range_array TSRANGE[],
                tstz_range TSTZRANGE,
                tstz_range_array TSTZRANGE[]
            );
    "#,
            )
            .as_str(),
        )
        .await
        .unwrap();

        let pg = &pg as &dyn SchemaTrait;

        let mut values = HashMap::new();
        values.insert("num_range".to_string(), Some("[1,10)".to_string()));
        values.insert(
            "num_range_array".to_string(),
            Some("{\"[1,5)\",\"[6,10]\"}".to_string()),
        );
        values.insert(
            "ts_range".to_string(),
            Some("[\"2024-01-01 00:00:00\",\"2024-01-10 00:00:00\")".to_string()),
        );
        values.insert(
            "ts_range_array".to_string(),
            Some("{\"[\\\"2024-01-01 00:00:00\\\",\\\"2024-01-05 00:00:00\\\")\",\"[\\\"2024-01-06 00:00:00\\\",\\\"2024-01-10 00:00:00\\\"]\"}".to_string()),
        );
        values.insert(
            "tstz_range".to_string(),
            Some("[\"2024-01-01 00:00:00+00\",\"2024-01-10 00:00:00+00\")".to_string()),
        );
        values.insert("tstz_range_array".to_string(), Some("{}".to_string()));

        pg.insert(table_name, values).await.unwrap();
        let x = pg
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("{:?}", x);
        let json = pg
            .export_table_to_string(&table_name, ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(json.as_str()).unwrap();

        println!("export json:{}", json);
    }

    #[tokio::test]
    async fn test_types11() {
        enable_test_log();
        let table_name = "test_types11";
        let pg = make_test_object().await;

        let _ = pg.drop_table(&table_name, false).await;

        let query_id = pg.open_connection().await.unwrap();
        pg.query_sql(
            query_id,
            format!(
                r#"
            CREATE TABLE {table_name} (
                id SERIAL PRIMARY KEY,
                date_range DATERANGE,
                date_range_array DATERANGE[],
                int8_range INT8RANGE,
                int8_range_array INT8RANGE[],
                json_path JSONPATH,
                json_path_array JSONPATH[],
                money_value MONEY,
                money_value_array MONEY[]
            );

    "#,
            )
            .as_str(),
        )
        .await
        .unwrap();

        let pg = &pg as &dyn SchemaTrait;

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
        values.insert(
            "json_path".to_string(),
            Some("$.\"store\".\"book\"[0].\"title\"".to_string()),
        );
        values.insert(
            "json_path_array".to_string(),
            Some("{\"$.\\\"store\\\".\\\"book\\\"[0].\\\"title\\\"\"}".to_string()),
        );
        values.insert("money_value".to_string(), Some("$10.50".to_string()));
        values.insert(
            "money_value_array".to_string(),
            Some("{$5.00,$15.75}".to_string()),
        );
        pg.insert(table_name, values).await.unwrap();
        let x = pg
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("{:?}", x);

        let json = pg
            .export_table_to_string(&table_name, ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(json.as_str()).unwrap();

        println!("export json:{}", json);
    }
}
