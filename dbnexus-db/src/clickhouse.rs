use r2d2::PooledConnection;
use sqlparser::ast::{ArrayElemTypeDef, DataType};

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::task::block_in_place;

use crate::{
    datasource_config::ClickHouseConfig,
    db::{
        BackendName, CheckConn, CreateSchemaExtra, CreateTableAndIndexSql, DBTrait, ExportKind,
        Field, ListTable, QueryResp, QuerySql, SchemaTrait, ServerMetas, ShowExtraArgs,
        SqlExecuteRet, TableListResp, ty_is_float, ty_is_int,
    },
    helper::{SqlKind, SqlWithKind, create_csv, sql_to_sql_seq},
    meta_value::MetaValue,
};

impl BackendName for ClickHouseDB {
    fn backend_name(&self) -> &str {
        "clickhouse"
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
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        self.export_query_result_as_csv(&sql, include_header, delimiter, sender)
            .await
    }
    async fn export_table_as_json(
        &self, table_name: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from {}", table_name);
        self.export_query_result_as_json(sql.as_str(), pretty, sender)
            .await
    }
    async fn export_table_as_json_line(
        &self, table_name: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from {}", table_name);

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

        let sql = format!(r#"select * from "{}""#, table_name);
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
            let s = (self as &dyn SchemaTrait).insert_sql(
                self.config.db.as_ref().map(|x| x.as_str()),
                table_name,
                &fs,
                &values,
            )?;
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

#[async_trait::async_trait]
impl SchemaTrait for ClickHouseDB {
    async fn create_table_and_index_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let sql = format!(r#"SHOW CREATE TABLE "{table_name}""#);
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
    fn quote_style(&self) -> &'static str {
        "\""
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(&table_name).await?;

        let sql = (self as &dyn SchemaTrait).delete_sql(
            self.config.db.as_ref().map(|x| x.as_str()),
            &table_name,
            &fs,
            &wheres,
            type_support_compare,
        )?;

        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;

        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: None,
            last_insert_id_string: None,
            last_insert_row_id: None,
        })
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"RENAME TABLE "{old}" TO "{new}""#);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }
    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"CREATE TABLE "{dup}" AS "{table}""#);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }
    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"TRUNCATE TABLE "{table_name}""#);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let sql = "show tables";
        let conn = self.get_conn()?;
        let ret: Vec<String> = conn.query(&sql).fetch_all().await?;
        anyhow::Ok(ret.into_iter().map(|x| ListTable::from(x)).collect())
    }
    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let conn = self.get_conn()?;

        let primary_keys = {
            let sql = format!(
                "SELECT primary_key FROM system.tables WHERE name = '{}' AND database = '{}'",
                table_name,
                self.config.db_str()
            );
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

            ret.push(Field {
                field: column_name,
                typ: column_type.clone(),
                nullable: column_type.to_lowercase().contains("null"),
                default_value: if default_value.is_empty() {
                    None
                } else {
                    Some(default_value)
                },
                is_primary,
                is_auto_increment: false,
                sortable: true,
            });
        }
        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<TableListResp> {
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

        log::info!("query sql:{}", sql);

        let result = self.query_as_json(&sql).await?;

        let mut infered_fs = result.infer_fields();
        infered_fs.iter_mut().for_each(|f| {
            if let Some(x) = fs.get(&f.field) {
                *f = x.clone();
            }
        });
        let data = result.encode_json_result()?;

        anyhow::Ok(TableListResp {
            fields: infered_fs,
            total,
            data,
            sql: Some(sql),
        })
    }
    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;

        let sql = (self as &dyn SchemaTrait).insert_sql(
            self.config.db.as_ref().map(|x| x.as_str()),
            &table_name,
            &fs,
            &values,
        )?;

        let conn = self.get_conn()?;

        conn.query(&sql).execute().await?;

        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: None,
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }
    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fs = self.show_table_columns(table_name).await?;
        let sql = (self as &dyn crate::db::SchemaTrait).update_sql(
            self.config.db.as_ref().map(|x| x.as_str()),
            table_name,
            &fs,
            &values,
            &wheres,
            type_support_compare,
        )?;
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;

        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: None,
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    fn read_only(&self) -> bool {
        self.config.read_only
    }
    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"DROP TABLE "{table}""#);
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    fn put_value_in_sql(&self, typ: &str, value: Option<&str>) -> anyhow::Result<String> {
        anyhow::Ok(put_value_in_sql(&parse_type(typ)?, value))
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
        .map(|x| format!("'{}'", x))
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
        DataType::Datetime64(_, _) => single_quote_str,
        DataType::LowCardinality(x) => put_value_in_sql(x.as_ref(), value),
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
        let _ = conn.query(&sql).fetch::<i32>()?;
        Ok(())
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        self.is_valid(conn).is_ok()
    }
}

#[async_trait::async_trait]
impl ServerMetas for ClickHouseDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        unimplemented!()
    }
}
fn get_sql_seq(sql: &str) -> anyhow::Result<Vec<SqlWithKind>> {
    // TODO use oracle later when sqlparser supported
    let dialect = sqlparser::dialect::ClickHouseDialect {};
    sql_to_sql_seq(
        sql,
        &dialect,
        |s| SqlKind::get_from_statement(s),
        |x| SqlKind::get_from_sql(x),
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
impl QuerySql for ClickHouseDB {
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

    fn clone(&self) -> Box<dyn QuerySql> {
        Box::new(Clone::clone(self))
    }
    async fn query_sql(
        &self, _query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let sqls = get_sql_seq(sql).unwrap_or(vec![SqlWithKind {
            sql: sql.to_string(),
            kind: SqlKind::Query,
        }]);
        let mut ret = Vec::new();

        for sql in sqls.iter() {
            match sql.kind {
                SqlKind::Query => {
                    let x = self.query_as_json(&sql.sql).await;
                    match x {
                        Ok(x) => {
                            let fields = x.infer_fields();
                            let data = x.encode_json_result()?;
                            ret.push(Ok(QueryResp { fields, data }));
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
    config: ClickHouseConfig,
    pool: r2d2::Pool<PoolManager>,
    client: reqwest::Client,
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
        })
    }
    fn get_conn(&self) -> anyhow::Result<PooledConnection<PoolManager>> {
        let conn = block_in_place(|| self.pool.get())?;
        anyhow::Ok(conn)
    }
}

#[async_trait::async_trait]
impl DBTrait for ClickHouseDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let conn = self.get_conn()?;
        let databases: Vec<String> = conn.query("show databases").fetch_all().await.unwrap();
        anyhow::Ok(databases)
    }
    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut x = Clone::clone(self);
        x.config.db = Some(name.to_owned());
        anyhow::Ok(Arc::new(x))
    }

    async fn create_schema(
        &self, name: &str, _extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        let sql = format!("CREATE DATABASE IF NOT EXISTS {name};");
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!("DROP DATABASE IF EXISTS {name};");
        let conn = self.get_conn()?;
        conn.query(&sql).execute().await?;
        anyhow::Ok(())
    }
}
#[derive(Clone, Debug, serde::Deserialize)]
pub struct JsonResp {
    meta: Vec<JsonRespMeta>,
    data: Vec<HashMap<String, serde_json::Value>>,
    #[allow(unused)]
    rows: usize,
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct JsonRespMeta {
    name: String,
    r#type: String,
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
                serde_json::Value::Array(values) => match typ {
                    DataType::Tuple(tys) => {
                        // a check
                        if values.len() != tys.len() {
                            anyhow::bail!(
                                "we are dealing with type values length and types length shold be equal ,values len:{} tys:{:?}",
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
                            xs.push(encode_nested(e, match ty {
                                ArrayElemTypeDef::Parenthesis(x) => x.as_ref(),
                                _ => anyhow::bail!("clickhouse accpet Parenthesis type parameter."),
                            })?);
                        }
                        format!("[{}]", xs.join(","))
                    }
                    _ => {
                        anyhow::bail!("datatype should array but :{:?}", typ)
                    }
                },
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
    async fn query_as_json(&self, query: &str) -> anyhow::Result<JsonResp> {
        let host = format!(
            "http{}://{}:{}",
            if self.config.https { "s" } else { "" },
            self.config.host,
            self.config.port
        );
        let uri = format!("{}/?query={} format json", host, query);

        let response = self
            .client
            .get(uri)
            .basic_auth(&self.config.username, self.config.password.as_ref())
            .send()
            .await?;
        if !response.status().is_success() {
            anyhow::bail!("response not ok:{:?}", response)
        }
        let ret = response.json::<JsonResp>().await?;
        anyhow::Ok(ret)
    }
}

#[cfg(test)]
fn make_test_object() -> ClickHouseDB {
    ClickHouseDB::new(ClickHouseConfig {
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

fn parse_type(typ: &str) -> anyhow::Result<DataType> {
    let d = sqlparser::dialect::ClickHouseDialect {};
    let mut p = sqlparser::parser::Parser::new(&d);
    p = p.try_with_sql(&typ)?;
    let ret = p.parse_data_type()?;
    anyhow::Ok(ret)
}

#[cfg(test)]
mod tests {
    use super::{make_test_object, parse_type};
    use crate::{
        db::{DBTrait, ExportKind, QuerySql, SchemaTrait},
        enable_test_log,
    };
    use std::collections::HashMap;
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
            .query_sql(
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

        assert!(_resp.iter().all(|x| x.is_ok()), "{:?}", _resp);
        {
            let mut values = HashMap::new();
            values.insert("value_uint8".to_string(), Some(1.to_string()));
            values.insert("value_uint16".to_string(), Some(2.to_string()));
            values.insert("value_uint32".to_string(), Some(3.to_string()));
            values.insert("value_uint64".to_string(), Some(4.to_string()));
            values.insert("value_uint128".to_string(), Some(5.to_string()));
            db.insert(&table_name, values).await.unwrap();
        }
        {
            let resp = db
                .show_table(table_name, Some("1 = 1"), Some("id asc"), 1, 100, None)
                .await
                .unwrap();

            println!("resp:{:?}", resp);
        }
        {
            let sql = (&db as &dyn SchemaTrait)
                .export_table_to_string(table_name, ExportKind::Sql {
                    include_table_schema: true,
                })
                .await
                .unwrap();

            println!("sql:{}", sql);
            let csv = (&db as &dyn SchemaTrait)
                .export_table_to_string(table_name, ExportKind::CSV {
                    include_header: true,
                    delimiter: ',',
                })
                .await
                .unwrap();

            println!("csv:{}", csv);
            let jsonline = (&db as &dyn SchemaTrait)
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();

            println!("jsonline:{}", jsonline);
            let json = (&db as &dyn SchemaTrait)
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();

            println!("json:{}", json);
        }
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
