use crate::datasource_config::ClickHouseConfig;
use crate::db::{
    BackendKind, CheckConn, ColumnType, CreateSchemaExtra, CreateTableAndIndexSql, DBTrait,
    DefAndMetas, ExportFrame, ExportKind, ExportQueryResult, Field, GetBackendKind, ListTable,
    MetaValue, NamedStaff, OptionMustBeSomeErr, QueryEvent, QueryEventEmitter, QueryResp, QuerySql,
    ReadOnlyTrait, SchemaTrait, ServerMetas, ShowTableParameter, SqlBuilder, SqlExecuteRet,
    UdfOrProcedureExtraParameter, XlsxEdit, create_html_table_data, create_html_table_header,
    create_markdown_data, create_markdown_header, escape_sql_str, ty_is_float, ty_is_int,
};

use crate::helper::separate_sql_by_sqlparser_tokenizer;
use crate::language_server::LexerCompletionTrait;
use futures_core::stream::BoxStream;
use futures_util::StreamExt;
use reqwest::Response;
use sqlparser::ast::{ArrayElemTypeDef, DataType};
use sqlparser::tokenizer::Token;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio_util::io::StreamReader;

impl GetBackendKind for ClickHouseDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::ClickHouse
    }
}

impl ClickHouseDB {
    async fn export_table_as_sql(
        &self, table_name: &str, include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            let x = self.show_create_table_sql(table_name).await?;
            sender.send(x.create_table.into()).await?;
        }

        let sql = format!(
            r#"select * from `{}`.`{}`"#,
            self.config.db_str(),
            table_name
        );
        let mut stream = self.query_row(&sql).await?;
        let mut fs = vec![];
        while let Some(row) = stream.next().await {
            let row = row?;
            match row {
                FieldsOrData::Fields(fields) => {
                    fs = fields;
                }
                FieldsOrData::Data(data) => {
                    let values = {
                        let mut m = HashMap::new();
                        data.into_iter().zip(fs.iter()).for_each(|(value, f)| {
                            m.insert(f.field.clone(), value);
                        });
                        m
                    };
                    let s = self.insert_sql(self.config.db.as_deref(), table_name, &fs, &values)?;
                    let s = format!("{};\n", s);
                    sender.send(s.into()).await?;
                }
                FieldsOrData::ReadingColumns => {}
            }
        }
        anyhow::Ok(())
    }
}

fn gen_show_sql(
    schema: &str, table_name: &str, fields: Option<String>, where_str: Option<&str>,
    order_by: Option<&str>, page: usize, page_size: usize,
) -> String {
    let mut sql = format!(
        "SELECT {} from `{schema}`.`{table_name}`",
        if let Some(x) = fields {
            x
        } else {
            "*".to_owned()
        }
    );
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
    pub(crate) async fn list_view_impl(&self, db: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"
            SELECT name
            FROM system.tables
            WHERE database = '{}' AND engine = 'View'
        "#,
            db
        );
        log::info!("sql:{}", sql);

        let ret: Vec<String> = self.client.query(&sql).fetch_all().await?;
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    pub(crate) async fn list_materialized_view_impl(
        &self, db: &str,
    ) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"
            SELECT name
            FROM system.tables
            WHERE database = '{}' AND engine = 'MaterializedView'
        "#,
            db
        );
        log::info!("sql:{}", sql);

        let ret: Vec<String> = self.client.query(&sql).fetch_all().await?;
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    pub(crate) async fn list_table_impl(&self, db: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!("show tables from `{}`", db);
        let ret: Vec<String> = self.client.query(&sql).fetch_all().await?;
        // delete materialized view
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
    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        LexerCompletionTrait::get_types(self).await
    }
    fn list_sql_current_values(&self) -> &'static [&'static str] {
        <Self as SqlBuilder>::CURRENT_VALUES
    }
    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_view_impl(self.config.db_str()).await
    }

    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_materialized_view_impl(self.config.db_str()).await
    }

    async fn drop_materialized_view(&self, name: &str) -> anyhow::Result<()> {
        self.drop_table(name, false).await
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP VIEW `{view_name}`"#);
        log::info!("sql:{}", sql);

        self.client.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    async fn show_create_table_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let sql = format!(r#"SHOW CREATE TABLE `{table_name}`"#);
        log::info!("sql:{}", sql);

        let x: String = self.client.query(&sql).fetch_one().await?;
        anyhow::Ok(CreateTableAndIndexSql {
            create_table: x,
            indexes: vec![],
        })
    }

    async fn export_table_or_view(
        &self, name: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::Sql {
                include_table_schema,
            } => {
                self.export_table_as_sql(name, include_table_schema, sender)
                    .await
            }
            _ => {
                let sql = format!(r#"SELECT * from `{}`.`{}`"#, self.config.db_str(), name);
                log::info!("sql:{}", sql);
                self.export_query_result(None, &sql, kind, sender).await
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
        let sql = self.delete_sql(self.config.db.as_deref(), table_name, &fs, &wheres)?;
        log::info!("sql:{}", sql);

        self.client.query(&sql).execute().await?;

        anyhow::Ok(SqlExecuteRet { sql, changes: None })
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"RENAME TABLE `{old}` TO `{new}`"#);
        log::info!("sql:{}", sql);

        self.client.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"CREATE TABLE `{dup}` AS `{table}`"#);
        log::info!("sql:{}", sql);
        log::info!("sql:{}", sql);

        self.client.query(&sql).execute().await?;
        // select data and insert.

        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);

        let jh1 = {
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
        let jh2 = {
            let table = table.to_owned();
            let dup = dup.to_owned();
            let this = Clone::clone(self);
            tokio::spawn(async move {
                while let Some(sql) = receiver.recv().await {
                    // replace table name.
                    let sql = sql.as_str()?.replacen(table.as_str(), &dup, 1);
                    this.client.query(&sql).execute().await?;
                }
                anyhow::Ok(())
            })
        };
        jh1.await??;
        jh2.await??;
        anyhow::Ok(())
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"TRUNCATE TABLE `{table_name}`"#);
        log::info!("sql:{}", sql);
        self.client.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.config.db_str()).await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let primary_keys = {
            let sql = format!(
                "SELECT primary_key FROM system.tables WHERE name = '{}' AND database = '{}'",
                table_name,
                self.config.db_str()
            );
            log::info!("sql:{}", sql);
            let ret: Vec<String> = self.client.query(&sql).fetch_all().await?;

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
            // “不是排序键” ≠ 不能排序，只是会走额外排序算子，更慢、更耗资源；
            // 把最常用的排序/过滤列放进排序键，就是为了省掉这一步。
            #[allow(unused)]
            is_sorting_key: u8,
        }
        let rows: Vec<Row> = self.client.query(&sql).fetch_all().await?;
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
                data_type: column_type.clone(),
                nullable,
                default_value: if default_value.is_empty() {
                    None
                } else {
                    Some(default_value)
                },
                is_primary,
                ..Default::default()
            });
        }
        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, para: ShowTableParameter, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let ShowTableParameter {
            where_str,
            order_by,
            page,
            page_size,
            ..
        } = &para;
        emit.send("trying to query total count.".into())?;
        let total = self
            .query_count(self.config.db_str(), table_name, where_str.as_deref())
            .await?;
        emit.send("trying to query columns.".into())?;
        let fs = self.show_table_columns(table_name).await?;
        let sql = gen_show_sql(
            self.config.db_str(),
            table_name,
            Some(self.backend_name().format_fields(Some(&fs))),
            where_str.as_deref(),
            order_by.as_deref(),
            *page,
            *page_size,
        );
        log::info!("sql:{}", sql);

        emit.send(
            QueryResp {
                fields: fs,
                total,
                ..Default::default()
            }
            .into(),
        )?;
        emit.send("trying to query data.".into())?;
        let mut stream = self.query_row(&sql).await?;
        while let Some(row) = stream.next().await {
            let row = row?;
            match row {
                FieldsOrData::Fields(_) => {}
                FieldsOrData::Data(row) => {
                    emit.send(row.into())?;
                }
                FieldsOrData::ReadingColumns => {}
            }
        }
        anyhow::Ok(())
    }

    async fn show_view(
        &self, view_name: &str, para: ShowTableParameter, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let ShowTableParameter {
            where_str,
            order_by,
            page,
            page_size,
            ..
        } = &para;
        emit.send("trying to query total count.".into())?;
        let total = self
            .query_count(self.config.db_str(), view_name, where_str.as_deref())
            .await?;
        let sql = gen_show_sql(
            self.config.db_str(),
            view_name,
            None,
            where_str.as_deref(),
            order_by.as_deref(),
            *page,
            *page_size,
        );
        log::info!("sql:{}", sql);
        emit.send("trying to query data.".into())?;
        let mut stream = self.query_row(&sql).await?;
        while let Some(row) = stream.next().await {
            let row = row?;
            match row {
                FieldsOrData::Fields(fs) => {
                    emit.send(
                        QueryResp {
                            fields: fs,
                            total,
                            ..Default::default()
                        }
                        .into(),
                    )?;
                }
                FieldsOrData::Data(row) => {
                    emit.send(row.into())?;
                }
                FieldsOrData::ReadingColumns => {}
            }
        }
        anyhow::Ok(())
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;

        let sql = self.insert_sql(self.config.db.as_deref(), table_name, &fs, &values)?;
        log::info!("sql:{}", sql);

        self.client.query(&sql).execute().await?;
        anyhow::Ok(SqlExecuteRet { sql, changes: None })
    }

    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;
        let sql = self.update_sql(self.config.db.as_deref(), table_name, &fs, &values, &wheres)?;
        log::info!("sql:{}", sql);

        self.client.query(&sql).execute().await?;
        anyhow::Ok(SqlExecuteRet { sql, changes: None })
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"DROP TABLE `{table}`"#);
        log::info!("sql:{}", sql);

        self.client.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
        ALTER TABLE `{}` DROP COLUMN `{}`;
        "#,
            table, col,
        );
        log::info!("sql:{}", sql);

        self.client.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        if old_name != new_name {
            let sql = format!(
                r#"ALTER TABLE `{}`
                    RENAME COLUMN `{}` TO `{}`"#,
                table, old_name, new_name
            );
            log::info!("sql:{}", sql);
            self.client.query(&sql).execute().await?;
        }

        if let Some(typ) = typ.as_ref() {
            let sql = format!(
                r#"
                ALTER TABLE `{}`
                MODIFY COLUMN `{}` {}
            "#,
                table, new_name, &typ.typ
            );
            log::info!("sql:{}", sql);
            self.client.query(&sql).execute().await?;

            if let Some(dv) = typ.default_value.as_ref() {
                let sql = format!(
                    r#"
                    ALTER TABLE {table}
                    MODIFY COLUMN {new_name} DEFAULT {}
                "#,
                    self.put_value_in_sql(&typ.typ, Some(dv.as_str()))?
                );
                log::info!("sql:{}", sql);
                self.client.query(&sql).execute().await?;
            }
        }
        anyhow::Ok(())
    }

    async fn add_table_column(
        &self, table: &str, name: &str, col: ColumnType,
    ) -> anyhow::Result<()> {
        let mut sql = format!(
            r#"ALTER TABLE `{table}`
                ADD COLUMN `{name}` {} "#,
            col.typ
        );
        if let Some(dv) = col.default_value.as_ref() {
            sql.push_str(
                format!(
                    "DEFAULT {} ",
                    self.put_value_in_sql(&col.typ, Some(dv.as_str()))?
                )
                .as_str(),
            );
        }

        self.client.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = "SELECT name  from system.functions where origin = 'SQLUserDefined'";

        let rows: Vec<String> = self.client.query(sql).fetch_all().await?;
        anyhow::Ok(rows.into_iter().map(NamedStaff::from).collect())
    }

    async fn show_udf(
        &self, name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<DefAndMetas> {
        let sql = format!(
            r#"SELECT create_query from system.functions where origin = 'SQLUserDefined' and  name='{name}'"#
        );

        let create_query: String = self.client.query(&sql).fetch_one().await?;
        anyhow::Ok(DefAndMetas {
            def: create_query,
            metas: vec![],
        })
    }

    async fn drop_udf(
        &self, name: &str, _extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let sql = format!("DROP FUNCTION `{name}`",);

        self.client.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    fn create_table_sql(&self, req: &crate::db::CreateTableReq) -> anyhow::Result<Vec<String>> {
        SqlBuilder::create_table_sql(self, req)
    }
}

impl ClickHouseDB {
    async fn query_count(
        &self, db: &str, name: &str, where_str: Option<&str>,
    ) -> anyhow::Result<usize> {
        let mut sql = format!(r#"SELECT count(1) FROM `{db}`.`{name}` "#);
        if let Some(where_str) = where_str {
            sql.push_str(format!(" where {where_str}").as_str());
        }

        let count: usize = self.client.query(&sql).fetch_one().await?;
        anyhow::Ok(count)
    }
}

fn put_value_in_sql(typ: &sqlparser::ast::DataType, value: Option<&str>) -> String {
    use sqlparser::ast::{DataType, ObjectName};

    let default_str = value.map(|x| x.to_string()).unwrap_or("null".to_string());
    let single_quote_str = value
        .map(|x| format!("'{}'", escape_sql_str(x)))
        .unwrap_or("null".to_string());

    fn custom_need_quote(x: &ObjectName) -> bool {
        let x = x.to_string().to_lowercase();
        x == "ipv4" || x == "ipv6"
    }
    match typ {
        DataType::Nullable(x) => put_value_in_sql(x.as_ref(), value),
        DataType::String(_) => single_quote_str,
        DataType::FixedString(_) => single_quote_str,
        DataType::Custom(x, _) if custom_need_quote(x) => single_quote_str,
        DataType::Date => single_quote_str,
        DataType::Date32 => single_quote_str,
        DataType::Datetime(_) => single_quote_str,
        DataType::Datetime64(..) => single_quote_str,
        DataType::LowCardinality(x) => put_value_in_sql(x.as_ref(), value),
        DataType::Uuid => single_quote_str,
        _ => default_str,
    }
}

#[async_trait::async_trait]
impl ServerMetas for ClickHouseDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let version: String = self.client.query("SELECT version()").fetch_one().await?;
        let mut ret = Vec::new();
        ret.push(MetaValue::new("version", version));
        let ops: Vec<(String, String)> = self
            .client
            .query("SELECT * FROM system.build_options")
            .fetch_all()
            .await?;

        ops.into_iter()
            .for_each(|(name, value)| ret.push(MetaValue::new(name, value)));

        anyhow::Ok(ret)
    }
}

impl ClickHouseDB {
    async fn export_query_result_as_csv(
        &self, sql: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let resp = self
            .query_as_response(
                sql,
                if include_header {
                    "CSVWithNames"
                } else {
                    "CSV"
                },
                Some(format!("format_csv_delimiter={}", delimiter)),
            )
            .await?;
        let mut stream = resp.bytes_stream();
        while let Some(data) = stream.next().await {
            let data = data?;
            sender.send(data.into()).await?;
            sender.send("\n".into()).await?;
        }
        anyhow::Ok(())
    }

    async fn export_query_result_as_html(
        &self, sql: &str, include_header: bool, sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let mut stream = self.query_row(sql).await?;
        while let Some(row) = stream.next().await {
            let row = row?;
            match row {
                FieldsOrData::Fields(fields) => {
                    if include_header {
                        let x = fields
                            .into_iter()
                            .map(|x| x.field.clone())
                            .collect::<Vec<_>>();
                        sender.send(create_html_table_header(&x[..]).into()).await?;
                    }
                }
                FieldsOrData::Data(data) => {
                    sender.send(create_html_table_data(&[data]).into()).await?;
                }
                FieldsOrData::ReadingColumns => {}
            }
        }

        anyhow::Ok(())
    }

    async fn export_query_result_as_md(
        &self, sql: &str, sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let mut stream = self.query_row(sql).await?;
        while let Some(row) = stream.next().await {
            let row = row?;
            match row {
                FieldsOrData::Fields(fields) => {
                    let x = fields
                        .into_iter()
                        .map(|x| x.field.clone())
                        .collect::<Vec<_>>();
                    sender.send(create_markdown_header(&x[..]).into()).await?;
                }
                FieldsOrData::Data(data) => {
                    sender.send(create_markdown_data(&data).into()).await?;
                }
                FieldsOrData::ReadingColumns => {}
            }
        }
        anyhow::Ok(())
    }

    async fn export_query_result_as_json(
        &self, sql: &str, sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let mut stream = self.query_as_text_line(sql, "JSONEachRow", None).await?;
        while let Some(data) = stream.next().await {
            let data = data?;
            let v: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&data)?;
            sender.send(v.into()).await?;
        }
        anyhow::Ok(())
    }

    async fn export_query_result_as_json_line(
        &self, sql: &str, sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let mut stream = self
            .query_as_response(sql, "JSONEachRow", None)
            .await?
            .bytes_stream();
        while let Some(data) = stream.next().await {
            let data = data?;
            sender.send(data.into()).await?;
        }
        anyhow::Ok(())
    }
}

impl ClickHouseDB {
    async fn export_query_result_as_xlsx(
        &self, sql: &str, include_header: bool, sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let mut stream = self.query_row(sql).await?;
        let mut row_number = 0;
        while let Some(row) = stream.next().await {
            let row = row?;
            match row {
                FieldsOrData::Fields(fields) => {
                    if include_header {
                        for (index, c) in fields.iter().enumerate() {
                            sender
                                .send(
                                    XlsxEdit {
                                        row: row_number,
                                        column: index as rust_xlsxwriter::ColNum,
                                        content: c.field.clone(),
                                    }
                                    .into(),
                                )
                                .await?;
                        }
                        row_number += 1;
                    }
                }
                FieldsOrData::Data(data) => {
                    for (col, d) in data.into_iter().enumerate() {
                        if let Some(d) = d {
                            sender
                                .send(
                                    XlsxEdit {
                                        row: row_number,
                                        column: col as rust_xlsxwriter::ColNum,
                                        content: d,
                                    }
                                    .into(),
                                )
                                .await?;
                        }
                    }
                    row_number += 1;
                }
                FieldsOrData::ReadingColumns => {}
            }
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
        &self, _query_id: Option<&str>, sql: &str, kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::Csv {
                include_header,
                delimiter,
            } => {
                self.export_query_result_as_csv(sql, include_header, delimiter, sender)
                    .await
            }
            ExportKind::Json { .. } => self.export_query_result_as_json(sql, sender).await,
            ExportKind::JsonLine => self.export_query_result_as_json_line(sql, sender).await,
            ExportKind::Sql {
                include_table_schema: _,
            } => ExportKind::cannot_query_result_export_as_sql(),
            ExportKind::Xlsx { include_header } => {
                self.export_query_result_as_xlsx(sql, include_header, sender)
                    .await
            }
            ExportKind::Html { include_header } => {
                self.export_query_result_as_html(sql, include_header, sender)
                    .await
            }
            ExportKind::MarkDown => self.export_query_result_as_md(sql, sender).await,
        }
    }
}

impl ClickHouseDB {
    fn not_allow_switch_db(&self, sql: &str) -> anyhow::Result<()> {
        let ss = crate::language_server::LangServerCommon::parse_statements(self, sql)
            .unwrap_or_default();

        for s in ss.iter() {
            if let sqlparser::ast::Statement::Use(_) = s {
                anyhow::bail!("due to internal reason, you can not switch database")
            }
        }
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl QuerySql for ClickHouseDB {
    async fn execute_sql_no_open_conn(
        &self, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        self.not_allow_switch_db(sql)?;

        let ret = match self.client.query(sql).execute().await {
            Ok(_) => Ok(QueryResp::execute_ok(sql.to_string())),
            Err(err) => Err(err.to_string()),
        };
        anyhow::Ok(ret)
    }
    async fn execute_sql(
        &self, _query_id: &str, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        self.not_allow_switch_db(sql)?;

        let ret = match self.client.query(sql).execute().await {
            Ok(_) => Ok(QueryResp::execute_ok(sql.to_string())),
            Err(err) => Err(err.to_string()),
        };
        anyhow::Ok(ret)
    }

    async fn query_sql(
        &self, _query_id: &str, sql: &str, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        self.not_allow_switch_db(sql)?;
        let dialect = sqlparser::dialect::ClickHouseDialect {};
        let sqls = separate_sql_by_sqlparser_tokenizer(sql, &dialect)?;
        for sql in sqls.iter() {
            emit.send(QueryEvent::new_status(format!(
                "try to execute sql: {}",
                sql
            )))?;
            let mut stream = self.query_row(sql).await?;
            while let Some(row) = stream.next().await {
                let row = row?;
                match row {
                    FieldsOrData::Fields(fields) => emit.send(
                        QueryResp {
                            fields,
                            ..Default::default()
                        }
                        .into(),
                    )?,
                    FieldsOrData::Data(items) => {
                        emit.send(vec![items].into())?;
                    }
                    FieldsOrData::ReadingColumns => {}
                }
            }
        }
        anyhow::Ok(())
    }
    async fn open_connection(&self, _id: &str) -> anyhow::Result<()> {
        anyhow::Ok(())
    }
    async fn close_connection(&self, _id: &str) -> anyhow::Result<()> {
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
    pub(crate) client: Arc<clickhouse::Client>,
    pub(crate) http_client: Arc<reqwest::Client>,
    pub(crate) key_words_cache: Arc<tokio::sync::Mutex<Vec<String>>>,
}

impl ReadOnlyTrait for ClickHouseDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}
#[async_trait::async_trait]

impl CheckConn for ClickHouseDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let now = std::time::Instant::now();
        let _: Option<u8> = self.client.query("select 1").fetch_optional().await?;
        anyhow::Ok(now.elapsed())
    }
}

impl ClickHouseDB {
    pub fn new(config: ClickHouseConfig) -> anyhow::Result<Self> {
        let mut client = clickhouse::Client::default();
        client = client.with_url(format!(
            "http{}://{}:{}",
            if config.https { "s" } else { "" },
            config.host,
            config.port
        ));
        client = client.with_user(config.username.clone());
        if let Some(pass) = config.password.as_ref() {
            client = client.with_password(pass);
        }
        if let Some(db) = config.db.as_ref() {
            client = client.with_database(db);
        }

        let http_client = {
            let mut b = reqwest::Client::builder();
            if let Some(x) = config.conn_timeout {
                b = b.connect_timeout(x.into());
            }
            b = b.pool_max_idle_per_host(10);
            b.build()?
        };
        anyhow::Ok(Self {
            config,
            http_client: Arc::new(http_client),
            client: Arc::new(client),
            key_words_cache: Default::default(),
        })
    }
}

#[async_trait::async_trait]
impl DBTrait for ClickHouseDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let databases: Vec<String> = self.client.query("show databases").fetch_all().await?;
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

        self.client.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("DROP DATABASE IF EXISTS {name};");
        log::info!("sql:{}", sql);

        self.client.query(&sql).execute().await?;
        anyhow::Ok(())
    }
}

fn encode_nested(value: &serde_json::Value, typ: &DataType) -> anyhow::Result<String> {
    let x = match value {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(x) => x.to_string(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::String(x) => format!("'{}'", escape_sql_str(x)),
        serde_json::Value::Array(values) => {
            match typ {
                DataType::Tuple(tys) => {
                    // a check
                    if values.len() != tys.len() {
                        anyhow::bail!(
                            "we are dealing with tuple values length and types length should be equal ,values len:{} tys:{:?}",
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
                                    anyhow::bail!("clickhouse accept Parenthesis type parameter.")
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

fn encode_json_result(tys: &[DataType], line: &str) -> anyhow::Result<Vec<Option<String>>> {
    let mut data = Vec::with_capacity(tys.len());
    let values: Vec<serde_json::Value> = serde_json::from_str(line)?;
    for (ty, value) in tys.iter().zip(values.iter()) {
        let value = match value {
            serde_json::Value::Null => None,
            serde_json::Value::Bool(x) => Some(x.to_string()),
            serde_json::Value::Number(number) => Some(number.to_string()),
            serde_json::Value::String(x) => Some(x.clone()),
            serde_json::Value::Array(_) => Some(encode_nested(value, ty)?),
            serde_json::Value::Object(_) => Some(encode_nested(value, ty)?),
        };
        data.push(value);
    }
    anyhow::Ok(data)
}

#[derive(Clone, Debug)]
pub enum FieldsOrData {
    Fields(Vec<Field>),
    Data(Vec<Option<String>>),
    ReadingColumns,
}

impl ClickHouseDB {
    pub(crate) async fn query_row(
        &'_ self, query: &str,
    ) -> anyhow::Result<BoxStream<'_, anyhow::Result<FieldsOrData>>> {
        let stream = self
            .query_as_text_line(query, "JSONCompactEachRowWithNamesAndTypes", None)
            .await?;
        let mut count = 0;
        let mut cols: Vec<String> = vec![];
        let mut ts: Vec<_> = vec![];
        let r = stream.map(move |x| match x {
            Err(err) => Err(anyhow::Error::from(err)),
            Ok(line) => {
                if count == 0 {
                    count += 1;
                    cols = serde_json::from_str(&line)?;
                    return anyhow::Ok(FieldsOrData::ReadingColumns);
                }
                if count == 1 {
                    count += 1;
                    let ts_str: Vec<String> = serde_json::from_str(&line)?;
                    for x in ts_str.iter() {
                        let t = parse_type(x)?;
                        ts.push(t);
                    }
                    return anyhow::Ok(FieldsOrData::Fields(
                        ts_str
                            .iter()
                            .zip(cols.iter())
                            .map(|(ty, name)| Field::from_name_type(name, ty))
                            .collect(),
                    ));
                }
                // now we have data
                let r = encode_json_result(&ts, &line)?;
                anyhow::Ok(FieldsOrData::Data(r))
            }
        });
        anyhow::Ok(Box::pin(r))
    }

    pub(crate) async fn query_as_text_line(
        &'_ self, query: &str, format: &str, http_extra: Option<String>,
    ) -> anyhow::Result<BoxStream<'_, Result<String, std::io::Error>>> {
        use tokio_stream::wrappers::LinesStream;
        let res = self.query_as_response(query, format, http_extra).await?;
        let reader =
            StreamReader::new(res.bytes_stream().map(|x| x.map_err(std::io::Error::other)));
        let reader = BufReader::new(reader);
        let lines = Box::pin(LinesStream::new(reader.lines()));
        anyhow::Ok(lines)
    }

    pub(crate) async fn query_as_response(
        &self, query: &str, format: &str, http_extra: Option<String>,
    ) -> anyhow::Result<Response> {
        let host = format!(
            "http{}://{}:{}",
            if self.config.https { "s" } else { "" },
            self.config.host,
            self.config.port
        );
        let mut uri = format!(
            "{}/?query={} format {format}{}",
            host,
            replace_new_line_to_whitespace(query)?,
            match &self.config.db {
                Some(x) => format!("&database={}", x),
                None => "".to_string(),
            }
        );
        if let Some(extra) = http_extra {
            uri.push('&');
            uri.push_str(&extra);
        }
        log::info!("url:{}", uri);
        let response = self
            .http_client
            .get(uri)
            .basic_auth(&self.config.username, self.config.password.as_ref())
            .send()
            .await?;
        if !response.status().is_success() {
            anyhow::bail!("response not ok:body:{}", response.text().await?)
        }
        anyhow::Ok(response)
    }
}

fn parse_type(typ: &str) -> anyhow::Result<DataType> {
    let d = sqlparser::dialect::ClickHouseDialect {};
    let mut p = sqlparser::parser::Parser::new(&d);
    p = p.try_with_sql(typ)?;
    let ret = p.parse_data_type()?;
    anyhow::Ok(ret)
}

/// ClickHouse 里可直接写在 SQL 语句中的零参常量字面量
pub const CH_CURRENT_LITERALS: &[&str] = &[
    "CURRENT_TIMESTAMP", //  2025-08-25 14:36:07
    "CURRENT_DATE",      //  2025-08-25
];

impl SqlBuilder for ClickHouseDB {
    const CURRENT_VALUES: &'static [&'static str] = CH_CURRENT_LITERALS;
    fn update_sql(
        &self, schema: Option<&str>, table_name: &str, fs: &[Field],
        values: &HashMap<String, Option<String>>, wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        //

        let mut field_by_name = HashMap::new();
        for f in fs.iter() {
            field_by_name.insert(&f.field, f);
        }
        #[allow(clippy::format_in_format_args)]
        let mut sql = format!(
            "ALTER TABLE {}{} UPDATE ",
            if let Some(schema) = schema {
                format!("{}.", self.quote(schema),)
            } else {
                "".to_string()
            },
            format!("{}", self.quote(table_name))
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
                        "{} = {}",
                        self.quote(k),
                        self.put_value_in_sql(
                            &f.option_must_be_some()?.data_type,
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

    fn put_value_in_sql(&self, typ: &str, value: Option<&str>) -> anyhow::Result<String> {
        if self.hit_current_value(value) {
            return anyhow::Result::Ok(value.option_must_be_some()?.to_string());
        }
        anyhow::Ok(put_value_in_sql(&parse_type(typ)?, value))
    }
}

#[cfg(test)]
pub(crate) fn make_test_object() -> ClickHouseDB {
    ClickHouseDB::new(ClickHouseConfig {
        conn_timeout: Some(5000.into()),
        host: "127.0.0.1".to_string(),
        port: 8123,
        read_only: false,
        db: Some("default".to_string()),
        username: "default".to_string(),
        password: Some("root".to_string()),
        https: false,
    })
    .unwrap()
}

/// clickhouse 换行转成空格 不然用url查询会有问题
/// 原因不详
fn replace_new_line_to_whitespace(sql: &str) -> anyhow::Result<String> {
    let dialect = sqlparser::dialect::ClickHouseDialect {};
    let tokens = sqlparser::tokenizer::Tokenizer::new(&dialect, sql)
        .tokenize()?
        .iter()
        .filter(|x| !matches!(x, Token::Whitespace(_)))
        .map(|x| x.to_string())
        .collect::<Vec<_>>();

    anyhow::Ok(tokens.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{make_test_object, parse_type};
    use crate::db::{
        ColumnType, CreateTableReq, CreateTableReqColumn, DBTrait, ExportKind, QuerySql,
        SchemaTrait, ServerMetas, SqlNullAble, TestQueryEventEmit,
    };
    use crate::enable_test_log;
    use std::collections::HashMap;

    #[test]
    fn test_replace_new_line_to_whitespace() {
        {
            let sql = r#"select *
from test"#;
            let x = replace_new_line_to_whitespace(sql).unwrap();
            assert_eq!(x, "select * from test");
        }
        {
            let sql = r#"select *
from test
;"#;
            let x = replace_new_line_to_whitespace(sql).unwrap();
            assert_eq!(x, "select * from test ;");
        }
    }

    #[tokio::test]
    async fn test_udf() {
        let db = make_test_object();
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let name = "test_udf_add";

        let _ = db.drop_udf(name, None).await;

        db.execute_sql(
            query_id,
            r#"CREATE FUNCTION test_udf_add AS (a, b) -> a + b;"#,
        )
        .await
        .unwrap()
        .unwrap();

        for l in db.list_udf().await.unwrap() {
            let detail = db.show_udf(&l.name, None).await.unwrap();
            println!("detail:{:?}", detail);
        }
    }

    #[tokio::test]
    async fn test_show_tables() {
        enable_test_log();
        let db = make_test_object();
        let mut stream = db.query_row("show TABLES from default").await.unwrap();
        while let Some(row) = stream.next().await {
            let row = row.unwrap();
            println!("!!!!!!!!!!!!:{:?}", row);
        }
    }

    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object();
        let ping_time = db.check_conn().await.unwrap();
        println!("{:?}", ping_time);
    }
    #[tokio::test]
    async fn test_show_metas() {
        let db = make_test_object();
        let metas = db.show_metas().await.unwrap();
        for meta in metas.iter() {
            println!("meta:{:?}", meta);
        }
    }
    #[tokio::test]
    async fn test_table_column() {
        let db = make_test_object();
        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();

        db.add_table_column(
            table_name,
            "name",
            ColumnType {
                typ: "String".to_string(),
                nullable: Default::default(),
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
                nullable: Default::default(),
                after: None,
                default_value: Some("abc111".to_string()),
            }),
        )
        .await
        .unwrap();

        db.drop_table_column(table_name, "name2").await.unwrap();
    }
    #[tokio::test]
    async fn test_list_databases() {
        let db = make_test_object();
        let name = "abc";
        let _ = db.drop_schema(name).await;
        db.create_schema(name, None).await.unwrap();

        let schemas = db.list_schema().await.unwrap();
        println!("schemas :{:?}", schemas);
    }
    #[tokio::test]
    async fn test_materialized_view() {
        let table_name = "test_materialized_view_table";
        let view_name = "test_materialized_view";

        let db = make_test_object();
        let _ = db.drop_materialized_view(view_name).await;
        let _ = db.drop_table(table_name, false).await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();

        let req = CreateTableReq {
            name: table_name.to_owned(),
            columns: vec![
                CreateTableReqColumn {
                    name: "id".to_string(),
                    data_type: "UInt32".into(),
                    auto_increment: false,
                    primary_key: true,
                    default_value: Some(1.to_string()),
                    nullable: SqlNullAble::NotNull,
                },
                CreateTableReqColumn {
                    name: "name".to_owned(),
                    data_type: "String".to_owned(),
                    ..Default::default()
                },
                CreateTableReqColumn {
                    name: "value".to_owned(),
                    data_type: "Float32".to_owned(),
                    ..Default::default()
                },
            ],
            indexes: vec![],
        };

        db.create_table(&req).await.unwrap();

        {
            let sql = db.create_table_sql_string(&req).unwrap();
            println!("create table sql:{}", sql);
        }

        db.execute_sql(
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
        .unwrap()
        .unwrap();

        db.execute_sql(
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
        .unwrap()
        .unwrap();

        let list = db.list_materialized_view().await.unwrap();
        println!("materialized view lists:{:?}", list);

        let _resp = db
            .show_view_collect(view_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();

        println!("view data:{:?}", _resp);
    }
    #[tokio::test]
    async fn test_view() {
        let table_name = "test_view_table";
        let view_name = "test_view";

        let db = make_test_object();
        let _ = db.drop_view(view_name).await;
        let _ = db.drop_table(table_name, false).await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();

        db.execute_sql(
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
                ORDER BY id "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
                INSERT INTO test_view_table (id, name, amount, created_at) VALUES
                (1, 'Alice', 100, now()),
                (2, 'Bob', 200, now()),
                (3, 'Charlie', 150, now()),
                (4, 'David', 300, now()),
                (5, 'Eve', 50, now()); "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
            CREATE VIEW test_view AS
            SELECT id, name, amount
            FROM test_view_table
"#,
        )
        .await
        .unwrap()
        .unwrap();

        let list = db.list_view().await.unwrap();
        println!("view lists:{:?}", list);

        let _resp = db
            .show_view_collect(view_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();

        println!("view data:{:?}", _resp);

        {
            let json = db
                .export_table_to_string(view_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            let v: serde_json::Value = serde_json::from_str(&json).unwrap();
            println!("!!!!!!!!!!!!:{:?}", v);
        }
    }

    #[tokio::test]
    async fn test_rename_truncate() {
        let db = make_test_object();
        let name = "test_rename";
        let _ = db.drop_table(name, false).await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();

        let name2 = format!("{}2", name);
        db.truncate_table(name).await.unwrap();

        db.rename_table(name, &name2).await.unwrap();
        db.rename_table(&name2, name).await.unwrap();
    }

    #[tokio::test]
    async fn test_dup() {
        // enable_test_log();
        let db = make_test_object();
        let name = "test_dup";
        let name2 = format!("{}2", name);
        let _ = db.drop_table(name, false).await;
        let _ = db.drop_table(&name2, false).await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();

        let _resp = db
            .execute_sql(
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

        db.duplicate_table(name, &name2).await.unwrap();

        let _resp = db
            .show_table_collect(&name2, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        assert_eq!(_resp.data.len(), 6);
    }

    #[tokio::test]
    async fn test_list_tables() {
        let db = make_test_object();

        let ret = db.list_table().await.unwrap();
        println!("{:?}", ret);

        let cols = db.show_table_columns("employees").await.unwrap();

        println!("columns:{:?}", cols);
    }

    #[tokio::test]
    async fn test_types1() {
        enable_test_log();
        let db = make_test_object();
        let table_name = "test_types1";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
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
            db.table_insert(table_name, values).await.unwrap();
        }
        {
            let resp = db
                .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
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
                    ExportKind::Csv {
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
            let html = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Html {
                        include_header: true,
                    },
                )
                .await
                .unwrap();

            println!("html:{}", html);
            let md = db
                .export_table_to_string(table_name, ExportKind::MarkDown)
                .await
                .unwrap();

            println!("markdown:{}", md);
        }
    }

    #[tokio::test]
    async fn test_types2() {
        enable_test_log();
        let db = make_test_object();
        let table_name = "test_types2";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();

        {
            let mut values = HashMap::new();
            values.insert("value_int8".to_string(), Some(1.to_string()));
            values.insert("value_int16".to_string(), Some(2.to_string()));
            values.insert("value_int32".to_string(), Some(3.to_string()));
            values.insert("value_int64".to_string(), Some(4.to_string()));
            values.insert("value_int128".to_string(), Some(5.to_string()));
            db.table_insert(table_name, values).await.unwrap();
        }
        {
            let resp = db
                .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
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
                    ExportKind::Csv {
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

    #[tokio::test]
    async fn test_types3() {
        enable_test_log();
        let db = make_test_object();
        let table_name = "test_types3";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db
            .execute_sql (
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
            .unwrap() .unwrap() ;

        {
            let mut values = HashMap::new();
            values.insert("id".to_string(), Some(100.to_string()));
            values.insert("float32_field".to_string(), Some(1.to_string()));
            values.insert("float64_field".to_string(), Some(2.to_string()));
            values.insert("decimal32_field".to_string(), Some(3.to_string()));
            values.insert("decimal64_field".to_string(), Some(4.to_string()));
            values.insert("decimal128_field".to_string(), Some(5.to_string()));
            values.insert("boolean_field".to_string(), Some(true.to_string()));
            db.table_insert(table_name, values).await.unwrap();
        }
        {
            let resp = db
                .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
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
                    ExportKind::Csv {
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

    #[tokio::test]
    async fn test_types4() {
        enable_test_log();
        let db = make_test_object();
        let table_name = "test_types4";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
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
        .unwrap()
        .unwrap();

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
            db.table_insert(table_name, values).await.unwrap();
        }
        {
            let resp = db
                .show_table_collect(
                    table_name,
                    ShowTableParameter::new_stupid_test("string_field asc"),
                )
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
                    ExportKind::Csv {
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

    #[tokio::test]
    async fn test_types5() {
        enable_test_log();
        let db = make_test_object();
        let table_name = "test_types5";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db
            .execute_sql (
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
            .unwrap() .unwrap() ;

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
            db.table_insert(table_name, values).await.unwrap();
        }
        {
            let resp = db
                .show_table_collect(
                    table_name,
                    ShowTableParameter::new_stupid_test("date32_field asc"),
                )
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
                    ExportKind::Csv {
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

    #[tokio::test]
    async fn test_types6() {
        enable_test_log();
        let db = make_test_object();
        let table_name = "test_types6";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db
            .execute_sql (
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
            .unwrap(). unwrap();

        {
            let mut values = HashMap::new();

            values.insert(
                "low_cardinality_field".to_string(),
                Some("some stupid value".to_string()),
            );
            values.insert(
                "nullable_field".to_string(),
                Some("hello world".to_string()),
            );

            db.table_insert(table_name, values).await.unwrap();
        }
        {
            let resp = db
                .show_table_collect(
                    table_name,
                    ShowTableParameter::new_stupid_test("nullable_field asc"),
                )
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
                    ExportKind::Csv {
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

    #[tokio::test]
    async fn test_error_query() {
        // enable_test_log();
        let db = make_test_object();
        let query_id = "";
        db.open_connection(query_id).await.unwrap();

        db.query_sql(
            query_id,
            "SELECT * fro test_types4",
            Arc::new(TestQueryEventEmit::default()),
        )
        .await
        .unwrap();
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
