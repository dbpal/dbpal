use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use r2d2::PooledConnection;
use sqlparser::ast::{ArrayElemTypeDef, DataType};
use tokio::task::block_in_place;

use crate::MemoryLimit;
use crate::datasource_config::ClickHouseConfig;
use crate::db::{
    BackendKind, CheckConn, ColumnType, CreateSchemaExtra, CreateTableAndIndexSql, DBTrait,
    ExportKind, ExportQueryResult, Field, GetBackendKind, ListMaterializedView, ListTable,
    NamedStaff, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas, ShowExtraArgs,
    SqlBuilder, SqlExecuteRet, escape_sql_str, ty_is_float, ty_is_int,
};
use crate::helper::{SqlKind, SqlWithKind, create_csv, is_dangerous_sql_by_parser, sql_to_sql_seq};
use crate::language_server::CompletionItem;
use crate::meta_value::MetaValue;
use crate::query_log::{QueryLogEvent, QueryLogMap};

impl GetBackendKind for ClickHouseDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::ClickHouse
    }
}

fn type_support_compare(_typ: &str) -> bool {
    true
}

impl ClickHouseDB {
    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"SELECT * from "{}"."{}""#,
            self.config.db_str(),
            table_name
        );
        log::info!("sql:{}", sql);
        self.export_query_result_as_csv(&sql, include_header, delimiter, sender)
            .await
    }

    async fn export_table_as_json(
        &self, table_name: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"SELECT * from "{}"."{}""#,
            self.config.db_str(),
            table_name
        );
        log::info!("sql:{}", sql);
        self.export_query_result_as_json(sql.as_str(), pretty, sender)
            .await
    }

    async fn export_table_as_json_line(
        &self, table_name: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"SELECT * from "{}"."{}""#,
            self.config.db_str(),
            table_name
        );
        log::info!("sql:{}", sql);
        self.export_query_result_as_json_line(&sql, sender).await
    }

    async fn export_table_as_sql(
        &self, table_name: &str, include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            let x = self.create_table_and_index_sql(table_name).await?;
            sender.send(Cow::Owned(x.create_table)).await?;
        }

        let sql = format!(
            r#"select * from "{}"."{}""#,
            self.config.db_str(),
            table_name
        );
        let result = self.query_as_json(&sql).await?;

        let fs = self.show_table_columns(table_name).await?;

        let datas = result.encode_json_result()?;

        for d in datas.into_iter() {
            let values = {
                let mut m = HashMap::new();
                d.into_iter()
                    .zip(result.meta.iter())
                    .for_each(|(value, f)| {
                        m.insert(f.name.clone(), value);
                    });
                m
            };
            let s = self.insert_sql(
                self.config.db.as_deref(),
                table_name,
                &fs,
                &values,
            )?;
            let s = format!("{};\n", s);
            sender.send(Cow::Owned(s)).await?;
        }

        anyhow::Ok(())
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
    sql.push_str(format!(" LIMIT {} , {}", offset, page_size).as_str());

    sql
}

impl ClickHouseDB {
    pub(crate) async fn list_view_impl(&self, db: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
            SELECT name
            FROM system.tables
            WHERE database = '{}' AND engine = 'View'
        "#,
            db
        );
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;

        let ret: Vec<String> = conn.query(&sql).fetch_all().await?;
        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
    }

    pub(crate) async fn list_materialized_view_impl(
        &self, db: &str,
    ) -> anyhow::Result<Vec<ListMaterializedView>> {
        let sql = format!(
            r#"
            SELECT name
            FROM system.tables
            WHERE database = '{}' AND engine = 'MaterializedView'
        "#,
            db
        );
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;

        let ret: Vec<String> = conn.query(&sql).fetch_all().await?;
        anyhow::Ok(
            ret.into_iter()
                .map(ListMaterializedView::from)
                .collect(),
        )
    }

    pub(crate) async fn list_table_impl(&self, db: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!("show tables from {}", db);
        let conn = self.get_conn()?;
        let ret: Vec<String> = conn.query(&sql).fetch_all().await?;
        // delete materiazlied view

        let mut ret = ret.into_iter().collect::<HashSet<_>>();

        let m = self
            .list_materialized_view_impl(db)
            .await?
            .into_iter()
            .map(|x| x.name)
            .collect::<Vec<_>>();

        for name in m.iter() {
            ret.remove(name);
        }
        let mut ret = ret.into_iter().collect::<Vec<_>>();
        ret.sort();
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }
}

#[async_trait::async_trait]
impl SchemaTrait for ClickHouseDB {
    async fn list_view(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_view_impl(self.config.db_str()).await
    }

    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListMaterializedView>> {
        self.list_materialized_view_impl(self.config.db_str()).await
    }

    async fn drop_materialized_view(&self, name: &str) -> anyhow::Result<()> {
        self.drop_table(name, false).await
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP VIEW "{view_name}""#);
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    async fn create_table_and_index_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let sql = format!(r#"SHOW CREATE TABLE "{table_name}""#);
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        let x: String = conn.query(&sql).fetch_one().await?;
        anyhow::Ok(CreateTableAndIndexSql {
            create_table: x,
            indexes: vec![],
        })
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

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    async fn table_delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;

        let sql = (self).delete_sql(
            self.config.db.as_deref(),
            table_name,
            &fs,
            &wheres,
        )?;
        log::info!("sql:{}", sql);

        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;

        anyhow::Ok(SqlExecuteRet { sql, changes: None })
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"RENAME TABLE "{old}" TO "{new}""#);
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"CREATE TABLE "{dup}" AS "{table}";"#);
        log::info!("sql:{}", sql);
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        // select data and insert.

        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);

        let j1 = {
            let x = Clone::clone(self);
            let table = table.to_owned();
            tokio::spawn(async move {
                x.export_table_or_view(
                    &table,
                    ExportKind::Sql {
                        include_table_schema: false,
                    },
                    sender,
                )
                .await
            })
        };
        let j2 = {
            let table = table.to_owned();
            let dup = dup.to_owned();
            let x = Clone::clone(self);
            tokio::spawn(async move {
                let conn = x.get_conn()?;
                while let Some(sql) = receiver.recv().await {
                    // replace table name.
                    let sql = sql.replacen(table.as_str(), &dup, 1);
                    conn.query(&sql).execute().await?;
                }
                anyhow::Ok(())
            })
        };
        j1.await??;
        j2.await??;
        anyhow::Ok(())
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"TRUNCATE TABLE "{table_name}""#);
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.config.db_str()).await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let conn = self.get_conn()?;
        let primary_keys = {
            let sql = format!(
                "SELECT primary_key FROM system.tables WHERE name = '{}' AND database = '{}'",
                table_name,
                self.config.db_str()
            );
            log::info!("sql:{}", sql);
            let ret: Vec<String> = conn.query(&sql).fetch_all().await?;

            ret.into_iter().collect::<HashSet<_>>()
        };

        let sql = format!(
            r#"
        SELECT
            name AS column_name,
            type AS column_type,
            default_kind,
            default_expression AS default_value,
            is_in_partition_key AS is_partition_key,
            is_in_sorting_key AS is_sorting_key
        FROM
            system.columns
        WHERE
            table = '{}' AND
            database = '{}'"#,
            table_name,
            self.config.db_str()
        );
        log::info!("sql:{}", sql);
        #[derive(Clone, Debug, serde::Deserialize, clickhouse::Row)]
        pub struct Row {
            column_name: String,
            column_type: String,
            #[allow(unused)]
            default_kind: String,
            default_value: String,
            #[allow(unused)]
            is_partition_key: bool,
            #[allow(unused)]
            is_in_sorting_key: bool,
        }

        let rows: Vec<Row> = conn.query(&sql).fetch_all().await.unwrap();

        let mut ret = Vec::with_capacity(rows.len());

        for r in rows.into_iter() {
            let Row {
                column_name,
                column_type,
                default_value,
                ..
            } = r;
            let is_primary = primary_keys.contains(&column_name);
            let ty = parse_type(&column_type)?;
            let nullable = matches!(ty, DataType::Nullable(_));
            ret.push(Field {
                field: column_name,
                typ: column_type.clone(),
                nullable,
                default_value: if default_value.is_empty() {
                    None
                } else {
                    Some(default_value)
                },
                is_primary,
                is_auto_increment: false,
                sortable: true,
                firebird_binary_sub_type: None,
            });
        }
        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        let total = self.query_count(table_name, where_str).await?;
        let fs = self.show_table_columns(table_name).await?;
        let fs = {
            let mut m = HashMap::new();
            fs.into_iter().for_each(|x| {
                m.insert(x.field.clone(), x);
            });
            m
        };
        let sql = gen_show_sql(
            self.config.db_str(),
            table_name,
            where_str,
            order_by,
            page,
            page_size,
        );

        log::info!("sql:{}", sql);

        let result = self.query_as_json(&sql).await?;

        let mut infered_fs = result.infer_fields();
        infered_fs.iter_mut().for_each(|f| {
            if let Some(x) = fs.get(&f.field) {
                *f = x.clone();
            }
        });
        let data = result.encode_json_result()?;
        anyhow::Ok(QueryResp::new(infered_fs, total, data, sql)?)
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        let total = self.query_count(view_name, where_str).await?;

        let sql = gen_show_sql(
            self.config.db_str(),
            view_name,
            where_str,
            order_by,
            page,
            page_size,
        );
        log::info!("sql:{}", sql);
        let result = self.query_as_json(&sql).await?;
        let fields = result.infer_fields();
        let data = result.encode_json_result()?;
        anyhow::Ok(QueryResp {
            fields,
            total,
            data,
            sql: Some(sql),
        })
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;

        let sql = self.insert_sql(
            self.config.db.as_deref(),
            table_name,
            &fs,
            &values,
        )?;
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(SqlExecuteRet { sql, changes: None })
    }

    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fs = self.show_table_columns(table_name).await?;
        let sql = self.update_sql(
            self.config.db.as_deref(),
            table_name,
            &fs,
            &values,
            &wheres,
        )?;
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;

        anyhow::Ok(SqlExecuteRet { sql, changes: None })
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"DROP TABLE "{table}""#);
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
       ALTER TABLE "{}" DROP COLUMN "{}";
        "#,
            table, col,
        );
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn()?;
        if old_name != new_name {
            let sql = format!(
                r#"ALTER TABLE "{}"
                    RENAME COLUMN "{}" TO "{}""#,
                table, old_name, new_name
            );
            log::info!("sql:{}", sql);
            conn.query(&sql).execute().await?;
        }

        if let Some(typ) = typ.as_ref() {
            let sql = format!(
                r#"
                ALTER TABLE "{}"
                MODIFY COLUMN "{}" {}
            "#,
                table, new_name, &typ.typ
            );
            log::info!("sql:{}", sql);
            conn.query(&sql).execute().await?;

            if let Some(dv) = typ.default_value.as_ref() {
                let sql = format!(
                    r#"
                    ALTER TABLE {table}
                    MODIFY COLUMN {new_name} DEFAULT {}
                "#,
                    self.put_value_in_sql(&typ.typ, None, Some(dv.as_str()))?
                );
                log::info!("sql:{}", sql);
                conn.query(&sql).execute().await?;
            }
        }
        anyhow::Ok(())
    }

    async fn add_table_column(
        &self, table: &str, name: &str, col: ColumnType,
    ) -> anyhow::Result<()> {
        let mut sql = format!(
            r#"ALTER TABLE "{table}"
                ADD COLUMN "{name}" {} "#,
            col.typ
        );
        if let Some(dv) = col.default_value.as_ref() {
            sql.push_str(
                format!(
                    "DEFAULT {} ",
                    self.put_value_in_sql(&col.typ, None, Some(dv.as_str()))?
                )
                .as_str(),
            );
        }
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }
}

impl ClickHouseDB {
    async fn query_count(&self, name: &str, where_str: Option<&str>) -> anyhow::Result<usize> {
        let mut sql = format!(r#"SELECT count(1) FROM "{name}" "#);
        if let Some(where_str) = where_str {
            sql.push_str(format!(" where {where_str}").as_str());
        }
        let conn = self.get_conn()?;
        let count: usize = conn.query(&sql).fetch_one().await?;
        anyhow::Ok(count)
    }
}

fn put_value_in_sql(typ: &sqlparser::ast::DataType, value: Option<&str>) -> String {
    use sqlparser::ast::{DataType, ObjectName};

    let default_str = value.map(|x| x.to_string()).unwrap_or("null".to_string());
    let single_quote_str = value
        .map(|x| format!("'{}'", escape_sql_str(x)))
        .unwrap_or("null".to_string());

    fn custon_need_quote(x: &ObjectName) -> bool {
        let x = x.to_string().to_lowercase();
        x == "ipv4" || x == "ipv6"
    }
    match typ {
        DataType::Nullable(x) => put_value_in_sql(x.as_ref(), value),
        DataType::String(_) => single_quote_str,
        DataType::FixedString(_) => single_quote_str,
        DataType::Custom(x, _) if custon_need_quote(x) => single_quote_str,
        DataType::Date => single_quote_str,
        DataType::Date32 => single_quote_str,
        DataType::Datetime(_) => single_quote_str,
        DataType::Datetime64(..) => single_quote_str,
        DataType::LowCardinality(x) => put_value_in_sql(x.as_ref(), value),
        DataType::Uuid => single_quote_str,
        _ => default_str,
    }
}

pub struct PoolManager {
    config: ClickHouseConfig,
}

impl r2d2::ManageConnection for PoolManager {
    type Connection = clickhouse::Client;
    type Error = clickhouse::error::Error;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let mut client = clickhouse::Client::default();
        client = client.with_url(format!(
            "http{}://{}:{}",
            if self.config.https { "s" } else { "" },
            self.config.host,
            self.config.port
        ));
        client = client.with_user(self.config.username.clone());
        if let Some(pass) = self.config.password.as_ref() {
            client = client.with_password(pass);
        }
        if let Some(db) = self.config.db.as_ref() {
            client = client.with_database(db);
        }

        Ok(client)
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        let sql = "select 1";
        let _ = conn.query(sql).fetch::<i32>()?;
        Ok(())
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        self.is_valid(conn).is_ok()
    }
}

#[async_trait::async_trait]
impl ServerMetas for ClickHouseDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let conn = self.get_conn()?;
        let version: String = conn.query("SELECT version()").fetch_one().await?;
        let mut ret = Vec::new();
        ret.push(MetaValue::new("version", version));
        let ops: Vec<(String, String)> = conn
            .query("SELECT * FROM system.build_options")
            .fetch_all()
            .await
            .unwrap();

        ops.into_iter()
            .for_each(|(name, value)| ret.push(MetaValue::new(name, value)));

        anyhow::Ok(ret)
    }
}

fn get_sql_seq(sql: &str) -> anyhow::Result<Vec<SqlWithKind>> {
    let dialect = sqlparser::dialect::ClickHouseDialect {};
    sql_to_sql_seq(
        sql,
        &dialect,
        SqlKind::get_from_statement,
        SqlKind::get_from_sql,
    )
}

impl ClickHouseDB {
    async fn export_query_result_as_csv(
        &self, sql: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let result = self.query_as_json(sql).await?;
        if include_header {
            let x = result
                .infer_fields()
                .into_iter()
                .map(|x| Some(x.field))
                .collect::<Vec<_>>();

            sender
                .send(Cow::Owned(create_csv(&vec![x], delimiter)))
                .await?;
        }

        let datas = result.encode_json_result()?;
        sender
            .send(Cow::Owned(create_csv(&datas, delimiter)))
            .await?;
        anyhow::Ok(())
    }

    async fn export_query_result_as_json(
        &self, sql: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let result = self.query_as_json(sql).await?;
        let s = if pretty {
            serde_json::to_string_pretty(&result.data)
        } else {
            serde_json::to_string(&result.data)
        }?;
        sender.send(Cow::Owned(s)).await?;
        anyhow::Ok(())
    }

    async fn export_query_result_as_json_line(
        &self, sql: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let result = self.query_as_json(sql).await?;
        for d in result.data.iter() {
            let s = serde_json::to_string(d)?;
            sender.send(Cow::Owned(s)).await?;
            sender.send(Cow::Borrowed("\n")).await?;
        }
        anyhow::Ok(())
    }
}
#[async_trait::async_trait]
impl ExportQueryResult for ClickHouseDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
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
}

#[async_trait::async_trait]
impl QuerySql for ClickHouseDB {
    fn get_query_log(&self, id: uuid::Uuid) -> anyhow::Result<Vec<QueryLogEvent>> {
        anyhow::Ok(self.query_logs.get(&id))
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        is_dangerous_sql_by_parser(sql, &sqlparser::dialect::ClickHouseDialect {})
    }

    async fn execute_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let mut query_log = self.query_logs.get_guard(query_id, sql);
        let conn = self.get_conn()?;
        let ret = match conn.query(sql).execute().await {
            Ok(_) => Ok(QueryResp::execute_ok(sql.to_string())),
            Err(err) => Err(err.to_string()),
        };
        query_log.set_success(ret.is_ok());

        anyhow::Ok(ret)
    }

    async fn query_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let sqls = get_sql_seq(sql).unwrap_or(vec![SqlWithKind {
            sql: sql.to_string(),
            kind: SqlKind::Query,
        }]);
        let mut ret = Vec::new();
        let mut query_log = self.query_logs.get_guard(query_id, sql);
        let mut ml = MemoryLimit::new();
        for sql in sqls.iter() {
            ml.exceed_error()?;

            log::info!("query sql kind:{:?} sql:{}", sql.kind, sql.sql);
            match sql.kind {
                SqlKind::Query => {
                    let x = self.query_as_json(&sql.sql).await;
                    ml.exceed_error()?;

                    match x {
                        Ok(x) => {
                            let fields = x.infer_fields();
                            let data = x.encode_json_result()?;
                            ret.push(Ok(QueryResp {
                                fields,
                                data,
                                sql: Some(sql.sql.clone()),
                                total: 0,
                            }));
                        }

                        Err(err) => {
                            ret.push(Err(err.to_string()));
                        }
                    }
                }
                _ => {
                    let conn = self.get_conn()?;
                    match conn.query(&sql.sql).execute().await {
                        Ok(_) => {}
                        Err(err) => ret.push(Err(err.to_string())),
                    }
                }
            }
        }
        query_log.set_success(ret.iter().all(|x| x.is_ok()));
        anyhow::Ok(ret)
    }

    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        anyhow::Ok(uuid::Uuid::new_v4())
    }

    async fn re_connect(&self, _id: uuid::Uuid) -> anyhow::Result<()> {
        anyhow::Ok(())
    }

    async fn close_connection(&self, _id: uuid::Uuid) -> anyhow::Result<()> {
        anyhow::Ok(())
    }
}

impl std::fmt::Debug for ClickHouseDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "host={} port={} username={}",
            self.config.host, self.config.port, self.config.username
        )
    }
}

#[derive(Clone)]
pub struct ClickHouseDB {
    pub(crate) config: ClickHouseConfig,
    pool: r2d2::Pool<PoolManager>,
    client: reqwest::Client,
    query_logs: QueryLogMap,
    pub(crate) key_word_completion: Arc<tokio::sync::Mutex<Vec<CompletionItem>>>,
}

impl ReadOnlyTrait for ClickHouseDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}
#[async_trait::async_trait]

impl CheckConn for ClickHouseDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let conn = self.get_conn()?;
        let now = std::time::Instant::now();
        let _: Option<u8> = conn.query("select 1").fetch_optional().await?;
        anyhow::Ok(now.elapsed())
    }
}

impl ClickHouseDB {
    pub fn new(config: ClickHouseConfig) -> anyhow::Result<Self> {
        let pool = r2d2::Builder::<PoolManager>::new().build(PoolManager {
            config: config.clone(),
        })?;

        let client = {
            let mut b = reqwest::Client::builder();
            b = b.connect_timeout(Duration::from_secs(10));
            b = b.pool_max_idle_per_host(10);
            b.build()?
        };
        anyhow::Ok(Self {
            config,
            pool,
            client,
            query_logs: Default::default(),
            key_word_completion: Default::default(),
        })
    }

    pub(crate) fn get_conn(&self) -> anyhow::Result<PooledConnection<PoolManager>> {
        let conn = block_in_place(|| self.pool.get())?;
        anyhow::Ok(conn)
    }
}

#[async_trait::async_trait]
impl DBTrait for ClickHouseDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let conn = self.get_conn()?;
        let databases: Vec<String> = conn.query("show databases").fetch_all().await.unwrap();
        anyhow::Ok(databases)
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut config = self.config.clone();
        config.db = Some(name.to_owned());
        let x = Self::new(config)?;
        anyhow::Ok(Arc::new(x))
    }

    async fn create_schema(
        &self, name: &str, _extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("CREATE DATABASE IF NOT EXISTS {name};");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("DROP DATABASE IF EXISTS {name};");
        log::info!("sql:{}", sql);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }
}
#[derive(Clone, Debug, serde::Deserialize)]
pub struct JsonResp {
    pub(crate) meta: Vec<JsonRespMeta>,
    pub(crate) data: Vec<HashMap<String, serde_json::Value>>,
    #[allow(unused)]
    rows: usize,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct JsonRespMeta {
    pub(crate) name: String,
    pub(crate) r#type: String,
}

impl JsonResp {
    fn infer_fields(&self) -> Vec<Field> {
        self.meta.iter().map(Field::from).collect()
    }

    fn encode_json_result(&self) -> anyhow::Result<Vec<Vec<Option<String>>>> {
        let mut ret = Vec::with_capacity(self.data.len());
        fn encode_nested(value: &serde_json::Value, typ: &DataType) -> anyhow::Result<String> {
            let x = match value {
                serde_json::Value::Null => "null".to_string(),
                serde_json::Value::Bool(x) => x.to_string(),
                serde_json::Value::Number(number) => number.to_string(),
                serde_json::Value::String(x) => format!("'{}'", x),
                serde_json::Value::Array(values) => {
                    match typ {
                        DataType::Tuple(tys) => {
                            // a check
                            if values.len() != tys.len() {
                                anyhow::bail!(
                                    "we are dealing with tuple values length and types length shold be equal ,values len:{} tys:{:?}",
                                    values.len(),
                                    tys
                                )
                            }
                            let mut xs = Vec::with_capacity(values.len());
                            for (value, ty) in values.iter().zip(tys.iter()) {
                                let v = encode_nested(value, &ty.field_type)?;
                                xs.push(v);
                            }
                            format!("({})", xs.join(","))
                        }

                        DataType::Array(ty) => {
                            let mut xs = Vec::with_capacity(values.len());
                            for e in values.iter() {
                                xs.push(encode_nested(
                                    e,
                                    match ty {
                                        ArrayElemTypeDef::Parenthesis(x) => x.as_ref(),
                                        _ => {
                                            anyhow::bail!(
                                                "clickhouse accpet Parenthesis type parameter."
                                            )
                                        }
                                    },
                                )?);
                            }
                            format!("[{}]", xs.join(","))
                        }
                        _ => {
                            anyhow::bail!("datatype should array but :{:?}", typ)
                        }
                    }
                }
                serde_json::Value::Object(map) => {
                    let (kty, vty) = match typ {
                        DataType::Map(k, v) => (k.as_ref(), v.as_ref()),
                        _ => anyhow::bail!("we dealing with map ty should be a map but:{:?}", typ),
                    };
                    let mut xs = Vec::with_capacity(map.len());
                    for (k, v) in map.iter() {
                        let no_need_quote = ty_is_int(kty) || ty_is_float(kty);
                        xs.push(format!(
                            "{}:{}",
                            if no_need_quote {
                                k.to_string()
                            } else {
                                format!("'{}'", k)
                            },
                            encode_nested(v, vty)?
                        ));
                    }
                    format!("{{{}}}", xs.join(","))
                }
            };
            anyhow::Ok(x)
        }

        for d in self.data.iter() {
            let mut data = Vec::with_capacity(self.meta.len());
            for meta in self.meta.iter() {
                let value: Option<String> = match d.get(&meta.name) {
                    Some(value) => match value {
                        serde_json::Value::Null => None,
                        serde_json::Value::Bool(x) => Some(x.to_string()),
                        serde_json::Value::Number(number) => Some(number.to_string()),
                        serde_json::Value::String(x) => Some(x.clone()),
                        serde_json::Value::Array(_) => {
                            let ty = parse_type(&meta.r#type)?;
                            Some(encode_nested(value, &ty)?)
                        }
                        serde_json::Value::Object(_) => {
                            let ty = parse_type(&meta.r#type)?;
                            Some(encode_nested(value, &ty)?)
                        }
                    },
                    None => None,
                };

                data.push(value);
            }
            ret.push(data);
        }

        anyhow::Ok(ret)
    }
}

impl From<&JsonRespMeta> for Field {
    fn from(value: &JsonRespMeta) -> Self {
        Field::from_name_type(&value.name, &value.r#type)
    }
}

impl ClickHouseDB {
    pub(crate) async fn query_as_json(&self, query: &str) -> anyhow::Result<JsonResp> {
        let host = format!(
            "http{}://{}:{}",
            if self.config.https { "s" } else { "" },
            self.config.host,
            self.config.port
        );
        let uri = format!(
            "{}/?query={} format json{}",
            host,
            query,
            match &self.config.db {
                Some(x) => format!("&database={}", x),
                None => "".to_string(),
            }
        );
        log::info!("url:{}", uri);

        let response = self
            .client
            .get(uri)
            .basic_auth(&self.config.username, self.config.password.as_ref())
            .send()
            .await?;
        if !response.status().is_success() {
            anyhow::bail!("response not ok:body:{}", response.text().await?)
        }
        let ret = response.json::<JsonResp>().await?;
        anyhow::Ok(ret)
    }
}

fn parse_type(typ: &str) -> anyhow::Result<DataType> {
    let d = sqlparser::dialect::ClickHouseDialect {};
    let mut p = sqlparser::parser::Parser::new(&d);
    p = p.try_with_sql(typ)?;
    let ret = p.parse_data_type()?;
    anyhow::Ok(ret)
}

impl SqlBuilder for ClickHouseDB {
    fn update_sql(
        &self, schema: Option<&str>, table_name: &str, fs: &Vec<Field>,
        values: &HashMap<String, Option<String>>, wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        //

        let mut field_by_name = HashMap::new();
        for f in fs.iter() {
            field_by_name.insert(&f.field, f);
        }

        let mut sql = format!(
            "ALTER TABLE {}{} UPDATE ",
            if let Some(schema) = schema {
                format!("{}{}{}.", self.quote_style(), schema, self.quote_style())
            } else {
                "".to_string()
            },
            format!("{}{}{}", self.quote_style(), table_name, self.quote_style())
        );
        {
            let last = values.len() - 1;
            for (index, (k, v)) in values.iter().enumerate() {
                let f = field_by_name.get(k);
                if f.is_none() {
                    anyhow::bail!("field '{}' not found", k);
                }

                sql.push_str(
                    format!(
                        "{}{}{} = {}",
                        self.quote_style(),
                        k,
                        self.quote_style(),
                        self.put_value_in_sql(
                            &f.unwrap().typ,
                            f.unwrap().firebird_binary_sub_type,
                            v.as_ref().map(|x| x.as_str())
                        )?
                    )
                    .as_str(),
                );
                if index != last {
                    sql.push(',')
                }
            }
        }
        sql.push_str(" WHERE ");
        sql.push_str(self.make_where(fs, wheres, &field_by_name)?.as_str());
        log::info!("update sql:{}", sql);
        anyhow::Ok(sql)
    }

    fn type_support_compare(&self, typ: &str) -> bool {
        type_support_compare(typ)
    }

    fn put_value_in_sql(
        &self, typ: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        anyhow::Ok(put_value_in_sql(&parse_type(typ)?, value))
    }
}

#[cfg(test)]
pub(crate) fn make_test_object() -> ClickHouseDB {
    ClickHouseDB::new(ClickHouseConfig {
        host: "127.0.0.1".to_string(),
        port: 8123,
        read_only: false,
        db: Some("default".to_string()),
        username: "default".to_string(),
        password: Some("root".to_string()),
        https: false,
        save_password: true,
    })
    .unwrap()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::{get_sql_seq, make_test_object, parse_type};
    use crate::db::{ColumnType, DBTrait, ExportKind, QuerySql, SchemaTrait, ServerMetas};
    use crate::enable_test_log;
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_show_metas() {
        let db = make_test_object();
        let metas = db.show_metas().await.unwrap();
        for meta in metas.iter() {
            println!("meta:{:?}", meta);
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_table_column() {
        let db = make_test_object();
        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;

        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .query_sql(
                query_id,
                r#"
                CREATE TABLE test_column
                (
                    id UInt32                       -- 主键，无符号32位整数
                )
                ENGINE = MergeTree()
                ORDER BY id;       
                        "#,
            )
            .await
            .unwrap();

        db.add_table_column(
            table_name,
            "name",
            ColumnType {
                typ: "String".to_string(),
                nullness: None,
                after: None,
                default_value: Some("abc".to_string()),
            },
        )
        .await
        .unwrap();

        db.modify_table_column(
            table_name,
            "name",
            "name2",
            Some(ColumnType {
                typ: "Nullable(String)".to_string(),
                nullness: None,
                after: None,
                default_value: Some("abc111".to_string()),
            }),
        )
        .await
        .unwrap();

        db.drop_table_column(table_name, "name2").await.unwrap();
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_list_databases() {
        let db = make_test_object();
        let name = "abc";
        let _ = db.drop_schema(name).await;
        db.create_schema(&name, None).await.unwrap();

        let schemas = db.list_schema().await.unwrap();
        println!("schemas :{:?}", schemas);
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_materialized_view() {
        let table_name = "test_materialized_view_table";
        let view_name = "test_materialized_view";

        let db = make_test_object();
        let _ = db.drop_materialized_view(view_name).await;
        let _ = db.drop_table(table_name, false).await;

        let query_id = db.open_connection().await.unwrap();

        let _resp = db
            .query_sql(
                query_id,
                r#"
        
                CREATE TABLE test_materialized_view_table (
                    id UInt32,
                    name String,
                    value Float32
                ) ENGINE = MergeTree()
                ORDER BY id;
                

"#,
            )
            .await
            .unwrap();
        assert!(_resp.iter().all(|x| x.is_ok()));

        let _resp = db
            .query_sql(
                query_id,
                r#"
            CREATE MATERIALIZED VIEW test_materialized_view
            ENGINE = SummingMergeTree()
            ORDER BY id AS
            SELECT
                id,
                name,
                sum(value) AS total_value
            FROM test_materialized_view_table
            GROUP BY id, name;
"#,
            )
            .await
            .unwrap();

        assert!(_resp.iter().all(|x| x.is_ok()));

        let _resp = db
            .query_sql(
                query_id,
                r#"
    
             
            INSERT INTO test_materialized_view_table (id, name, value) VALUES
            (1, 'Test A', 10.5),
            (2, 'Test B', 20.0),
            (3, 'Test C', 30.2),
            (4, 'Test D', 40.7),
            (5, 'Test E', 50.1);

"#,
            )
            .await
            .unwrap();
        assert!(_resp.iter().all(|x| x.is_ok()));

        let list = db.list_materialized_view().await.unwrap();
        println!("materialized view lists:{:?}", list);

        let _resp = db
            .show_view(view_name, Some("1  = 1 "), Some("id asc"), 1, 100, None)
            .await
            .unwrap();

        println!("view data:{:?}", _resp);
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_view() {
        let table_name = "test_view_table";
        let view_name = "test_view";

        let db = make_test_object();
        let _ = db.drop_view(view_name).await;
        let _ = db.drop_table(table_name, false).await;

        let query_id = db.open_connection().await.unwrap();

        let _resp = db
            .query_sql(
                query_id,
                r#"
        
                CREATE TABLE test_view_table
                (
                    id UInt32,
                    name String,
                    amount Int32,
                    created_at DateTime DEFAULT now()
                )
                ENGINE = MergeTree()
                ORDER BY id;

                INSERT INTO test_view_table (id, name, amount, created_at) VALUES
                (1, 'Alice', 100, now()),
                (2, 'Bob', 200, now()),
                (3, 'Charlie', 150, now()),
                (4, 'David', 300, now()),
                (5, 'Eve', 50, now());

"#,
            )
            .await
            .unwrap();
        assert!(_resp.iter().all(|x| x.is_ok()));

        let _resp = db
            .query_sql(
                query_id,
                r#"
            CREATE VIEW test_view AS
            SELECT id, name, amount
            FROM test_view_table
"#,
            )
            .await
            .unwrap();

        assert!(_resp.iter().all(|x| x.is_ok()));

        let list = db.list_view().await.unwrap();
        println!("view lists:{:?}", list);

        let _resp = db
            .show_view(view_name, Some("1  = 1 "), Some("id asc"), 1, 100, None)
            .await
            .unwrap();

        println!("view data:{:?}", _resp);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_rename_truncate() {
        let db = make_test_object();
        let name = "test_rename";
        let _ = db.drop_table(name, false).await;

        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_rename (
                id UInt32,
                name String,
                created_at DateTime DEFAULT now()
            ) ENGINE = MergeTree()
            ORDER BY id;
        "#,
            )
            .await
            .unwrap();
        assert!(_resp.iter().all(|x| x.is_ok()));
        let name2 = format!("{}2", name);
        db.truncate_table(name).await.unwrap();

        db.rename_table(name, &name2).await.unwrap();
        db.rename_table(&name2, name).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_dup() {
        // enable_test_log();
        let db = make_test_object();
        let name = "test_dup";
        let name2 = format!("{}2", name);
        let _ = db.drop_table(name, false).await;
        let _ = db.drop_table(&name2, false).await;

        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_dup (
                id UInt32,
                name String,
                created_at DateTime DEFAULT now()
            ) ENGINE = MergeTree()
            ORDER BY id;
        "#,
            )
            .await
            .unwrap();
        assert!(_resp.iter().all(|x| x.is_ok()));

        let _resp = db
            .query_sql(
                query_id,
                r#"
        INSERT INTO test_dup (id, name, created_at) VALUES
            (1, 'Alice', now()),
            (2, 'Bob', now()),
            (3, 'Charlie', now()),
            (4, 'David', now()),
            (5, 'Eve', now()),
            (6, 'Frank', now());
    "#,
            )
            .await
            .unwrap();
        assert!(_resp.iter().all(|x| x.is_ok()));

        db.duplicate_table(name, &name2).await.unwrap();

        let _resp = db
            .show_table(&name2, None, None, 1, 100, None)
            .await
            .unwrap();
        assert_eq!(_resp.data.len(), 6);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_list_tables() {
        let db = make_test_object();

        let ret = db.list_table().await.unwrap();
        println!("{:?}", ret);

        let cols = db.show_table_columns("employees").await.unwrap();

        println!("columns:{:?}", cols);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_query() {
        let db = make_test_object();
        let ret = db.query_as_json("select 1").await.unwrap();
        println!("{:?}", ret);
    }

    #[ignore = "example_table may not exists"]
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_show_table() {
        let db = make_test_object();
        let resp = db
            .show_table("example_table", None, None, 1, 100, None)
            .await
            .unwrap();

        for d in resp.data.iter() {
            println!("{:?}", d);
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_types1() {
        enable_test_log();
        let db = make_test_object();
        let table_name = "test_types1";
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .execute_sql(
                query_id,
                r#"
            CREATE TABLE test_types1
            (
                id UInt64,                 
                value_uint8 UInt8,      
                value_uint16 UInt16,       
                value_uint32 UInt32,      
                value_uint64 UInt64,     
                value_uint128 UInt128 
            ) ENGINE = MergeTree()
            ORDER BY id;
        "#,
            )
            .await
            .unwrap();

        assert!(_resp.is_ok(), "{:?}", _resp);
        {
            let mut values = HashMap::new();
            values.insert("value_uint8".to_string(), Some(1.to_string()));
            values.insert("value_uint16".to_string(), Some(2.to_string()));
            values.insert("value_uint32".to_string(), Some(3.to_string()));
            values.insert("value_uint64".to_string(), Some(4.to_string()));
            values.insert("value_uint128".to_string(), Some(5.to_string()));
            db.table_insert(&table_name, values).await.unwrap();
        }
        {
            let resp = db
                .show_table(table_name, Some("1 = 1"), Some("id asc"), 1, 100, None)
                .await
                .unwrap();

            println!("resp:{:?}", resp);
        }
        {
            let sql = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();

            println!("sql:{}", sql);
            let csv = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();

            println!("csv:{}", csv);
            let jsonline = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();

            println!("jsonline:{}", jsonline);
            let json = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            println!("json:{}", json);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_types2() {
        enable_test_log();
        let db = make_test_object();
        let table_name = "test_types2";
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_types2
            (
                id UInt64,                 
                value_int8 Int8,      
                value_int16 Int16,       
                value_int32 Int32,      
                value_int64 Int64,     
                value_int128 Int128 
            ) ENGINE = MergeTree()
            ORDER BY id;
        "#,
            )
            .await
            .unwrap();

        assert!(_resp.iter().all(|x| x.is_ok()), "{:?}", _resp);
        {
            let mut values = HashMap::new();
            values.insert("value_int8".to_string(), Some(1.to_string()));
            values.insert("value_int16".to_string(), Some(2.to_string()));
            values.insert("value_int32".to_string(), Some(3.to_string()));
            values.insert("value_int64".to_string(), Some(4.to_string()));
            values.insert("value_int128".to_string(), Some(5.to_string()));
            db.table_insert(&table_name, values).await.unwrap();
        }
        {
            let resp = db
                .show_table(table_name, Some("1 = 1"), Some("id asc"), 1, 100, None)
                .await
                .unwrap();

            println!("resp:{:?}", resp);
        }
        {
            let sql = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();

            println!("sql:{}", sql);
            let csv = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();

            println!("csv:{}", csv);
            let jsonline = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();

            println!("jsonline:{}", jsonline);
            let json = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            println!("json:{}", json);
        }
    }

    #[test]
    fn test_get_sql_seq() {
        let ss = get_sql_seq(
            r#"CREATE TABLE test_types3
            (
                id UInt64,  
                float32_field Float32,
                float64_field Float64,
                decimal32_field Decimal(10, 2), -- Decimal(精度, 小数点后位数)，这里使用 32 位存储
                decimal64_field Decimal(18, 4), -- 64 位存储
                decimal128_field Decimal(38, 8), -- 128 位存储
                boolean_field UInt8 -- ClickHouse 中没有原生 Boolean 类型，通常用 UInt8 表示（0 表示 false，非 0 表示 true）
            )
            ENGINE = MergeTree()
            ORDER BY id; -- 根据需要选择排序键
        "#,
        ) .unwrap();
        if ss.len() != 1 {
            for s in ss.iter() {
                println!("{:?}", s);
            }
        }
        assert!(ss.len() == 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_types3() {
        enable_test_log();
        let db = make_test_object();
        let table_name = "test_types3";
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_types3
            (
                id UInt64,  
                float32_field Float32,
                float64_field Float64,
                decimal32_field Decimal(10, 2), -- Decimal(精度, 小数点后位数)，这里使用 32 位存储
                decimal64_field Decimal(18, 4), -- 64 位存储
                decimal128_field Decimal(38, 8), -- 128 位存储
                boolean_field UInt8 -- ClickHouse 中没有原生 Boolean 类型，通常用 UInt8 表示（0 表示 false，非 0 表示 true）
            )
            ENGINE = MergeTree()
            ORDER BY id; -- 根据需要选择排序键
        "#,
            )
            .await
            .unwrap();

        assert!(_resp.iter().all(|x| x.is_ok()), "{:?}", _resp);
        {
            let mut values = HashMap::new();
            values.insert("id".to_string(), Some(100.to_string()));
            values.insert("float32_field".to_string(), Some(1.to_string()));
            values.insert("float64_field".to_string(), Some(2.to_string()));
            values.insert("decimal32_field".to_string(), Some(3.to_string()));
            values.insert("decimal64_field".to_string(), Some(4.to_string()));
            values.insert("decimal128_field".to_string(), Some(5.to_string()));
            values.insert("boolean_field".to_string(), Some(true.to_string()));
            db.table_insert(&table_name, values).await.unwrap();
        }
        {
            let resp = db
                .show_table(table_name, Some("1 = 1"), None, 1, 100, None)
                .await
                .unwrap();

            println!("resp:{:?}", resp);
        }
        {
            let sql = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();

            println!("sql:{}", sql);
            let csv = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();

            println!("csv:{}", csv);
            let jsonline = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();

            println!("jsonline:{}", jsonline);
            let json = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            println!("json:{}", json);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_types4() {
        enable_test_log();
        let db = make_test_object();
        let table_name = "test_types4";
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_types4
            (
                string_field String,            -- 动态长度的字符串
                fixed_string_field FixedString(16), -- 固定长度的字符串，这里长度为 16 字节
                uuid_field UUID,                -- UUID 类型
                ipv6_field IPv6,                -- IPv6 地址
                ipv4_field IPv4,                -- IPv4 地址
                date_field Date                 -- 日期类型
            )
            ENGINE = MergeTree()
            ORDER BY date_field; -- 根据日期字段排序
        "#,
            )
            .await
            .unwrap();

        assert!(_resp.iter().all(|x| x.is_ok()), "{:?}", _resp);
        {
            let mut values = HashMap::new();

            values.insert(
                "string_field".to_string(),
                Some("Hello, ClickHouse!".to_string()),
            );
            values.insert(
                "fixed_string_field".to_string(),
                Some("FixedString".to_string()),
            );
            values.insert(
                "uuid_field".to_string(),
                Some("6f9619ff-8b86-d011-b42d-00c04fc964ff".to_string()),
            );
            values.insert(
                "ipv6_field".to_string(),
                Some("2001:0db8:85a3:0000:0000:8a2e:0370:7334".to_string()),
            );
            values.insert("ipv4_field".to_string(), Some("192.168.1.1".to_string()));
            values.insert("date_field".to_string(), Some("2025-03-06".to_string()));
            db.table_insert(&table_name, values).await.unwrap();
        }
        {
            let resp = db
                .show_table(table_name, Some("1 = 1"), None, 1, 100, None)
                .await
                .unwrap();

            println!("resp:{:?}", resp);
        }
        {
            let sql = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();

            println!("sql:{}", sql);
            let csv = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();

            println!("csv:{}", csv);
            let jsonline = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();

            println!("jsonline:{}", jsonline);
            let json = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            println!("json:{}", json);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_types5() {
        enable_test_log();
        let db = make_test_object();
        let table_name = "test_types5";
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_types5
            (
                date32_field Date32,                     -- Date32 类型，支持更广泛的日期范围
                datetime_field DateTime,                 -- DateTime 类型，表示日期和时间
                datetime64_field DateTime64(3),          -- DateTime64 类型，支持小数秒，精度为 3 位小数
                tuple_field Tuple(Int32, String, Float64), -- Tuple 类型，包含多个不同类型的字段
                array_field Array(String),               -- Array 类型，存储字符串数组
                map_field Map(String, Int32)             -- Map 类型，键为字符串，值为 Int32
            )
            ENGINE = MergeTree()
            ORDER BY date32_field;  
        "#,
            )
            .await
            .unwrap();

        assert!(_resp.iter().all(|x| x.is_ok()), "{:?}", _resp);
        {
            let mut values = HashMap::new();

            values.insert("date32_field".to_string(), Some("2025-03-06".to_string()));
            values.insert(
                "datetime_field".to_string(),
                Some("2025-03-06 12:34:56".to_string()),
            );
            values.insert(
                "datetime64_field".to_string(),
                Some("2025-03-06 12:34:56.789".to_string()),
            );
            values.insert(
                "tuple_field".to_string(),
                Some("(123, 'example', 456.789)".to_string()),
            );
            values.insert(
                "array_field".to_string(),
                Some("['item1', 'item2', 'item3']".to_string()),
            );
            values.insert(
                "map_field".to_string(),
                Some("map('key1', 10, 'key2', 20)".to_string()),
            );
            db.table_insert(&table_name, values).await.unwrap();
        }
        {
            let resp = db
                .show_table(table_name, Some("1 = 1"), None, 1, 100, None)
                .await
                .unwrap();

            println!("resp:{:?}", resp);
        }
        {
            let sql = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();

            println!("sql:{}", sql);
            let csv = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();

            println!("csv:{}", csv);
            let jsonline = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();

            println!("jsonline:{}", jsonline);
            let json = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            println!("json:{}", json);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_types6() {
        // enable_test_log();
        let db = make_test_object();
        let table_name = "test_types6";
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();
        let _resp = db
            .query_sql(
                query_id,
                r#"
            CREATE TABLE test_types6
            (
                low_cardinality_field LowCardinality(String),  -- 使用 LowCardinality 优化存储和查询性能[^4^]
                nullable_field Nullable(String),               -- 可以存储 NULL 值的字段[^2^]
                 -- json_field JSON                                -- JSON 类型字段，用于存储 JSON 数据[^3^]
            )
            ENGINE = MergeTree()
            ORDER BY low_cardinality_field;
        "#,
            )
            .await
            .unwrap();

        assert!(_resp.iter().all(|x| x.is_ok()), "{:?}", _resp);
        {
            let mut values = HashMap::new();

            values.insert("common_value".to_string(), Some("common_value".to_string()));
            values.insert("some_string".to_string(), Some("hello world".to_string()));

            db.table_insert(&table_name, values).await.unwrap();
        }
        {
            let resp = db
                .show_table(table_name, Some("1 = 1"), None, 1, 100, None)
                .await
                .unwrap();

            println!("resp:{:?}", resp);
        }
        {
            let sql = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();

            println!("sql:{}", sql);
            let csv = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();

            println!("csv:{}", csv);
            let jsonline = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();

            println!("jsonline:{}", jsonline);
            let json = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            println!("json:{}", json);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_error_query() {
        // enable_test_log();
        let db = make_test_object();
        let query_id = db.open_connection().await.unwrap();

        let x = db
            .query_sql(query_id, "SELECT * fro test_types4")
            .await
            .unwrap();
        assert!(x.iter().any(|x| x.is_err()));
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_query_empty() {
        let db = make_test_object();
        let query_id = db.open_connection().await.unwrap();
        let x = db
            .query_sql(query_id, " --   this is comment.")
            .await
            .unwrap();
        assert!(x.iter().all(|x| x.is_ok()));
    }
    #[test]
    fn test_parse_type() {
        let x = parse_type("Nullable(String) ").unwrap();
        println!("{:?}", x);
        let x = parse_type("FixedString(10) ").unwrap();
        println!("{:?}", x);
        let x = parse_type("IPv4 ").unwrap();
        println!("{:?}", x);
        let x = parse_type("IPv6 ").unwrap();
        println!("{:?}", x);
        let x = parse_type("Date ").unwrap();
        println!("{:?}", x);
        let x = parse_type("Date32 ").unwrap();
        println!("{:?}", x);
        let x = parse_type("DateTime ").unwrap();
        println!("{:?}", x);
        let x = parse_type("DateTime64(1) ").unwrap();
        println!("{:?}", x);
        let x = parse_type("Tuple(Int32, String)").unwrap();
        println!("{:?}", x);
        let x = parse_type("Map(String, Int32)").unwrap();
        println!("{:?}", x);
        let x = parse_type("LowCardinality(String)").unwrap();
        println!("{:?}", x);
        let x = parse_type("Array(Tuple(Int32, String))").unwrap();
        println!("{:?}", x);
    }
}
