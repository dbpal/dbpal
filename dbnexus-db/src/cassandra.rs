// 出现错误 InvalidRequest: Error from server: code=2200 [Invalid query]
// message="org.apache.cassandra.auth.CassandraRoleManager doesn't support
// PASSWORD" 的原因是 Cassandra 的身份验证机制未启用，或者配置不支持密码身份验证
// 。
// 要解决这个问题，请按照以下步骤操作：
// 1. 修改 cassandra.yaml 配置文件
// 默认情况下，Cassandra 使用
// AllowAllAuthenticator，这表示禁用了身份验证。你需要将其改为
// PasswordAuthenticator，以启用基于密码的身份验证 。
//
// 打开 cassandra.yaml 文件（通常位于 /etc/cassandra/conf/cassandra.yaml 或
// Cassandra 安装目录的 conf 文件夹中）。 找到以下行：
// yaml复制
//
// authenticator: AllowAllAuthenticator
//
// 将其更改为：
// yaml复制
//
// authenticator: PasswordAuthenticator
//
// 保存文件并重启 Cassandra 服务
//
// InvalidRequest: Error from server: code=2200 [Invalid query]
// message="User-defined functions are disabled in cassandra.yaml - set
// user_defined_functions_enabled=true to enable"
//

use crate::datasource_config::CassandraDBConfig;
use crate::db::{
    AutoCompletionQueryId, BackendKind, CheckConn, ColumnType, CreateSchemaExtra,
    CreateTableAndIndexSql, CreatedTypeDef, DBTrait, DefAndMetas, ExportKind, ExportQueryResult,
    Field, GetBackendKind, Index, IndexKind, ListTable, MetaValue, NamedStaff, OptionMustBeSomeErr,
    QueryEventEmitter, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, ServerMetas,
    ShowTableParameter, SqlBuilder, SqlExecuteRet, TimeElapsed, UdfOrProcedureExtraParameter,
    XlsxEdit, create_html_table_data, create_html_table_header, escape_sql_str,
    export_query_as_xlsx_send_data,
};

use crate::helper::{create_csv, separate_sql_by_sqlparser_tokenizer};
use crate::language_server::LexerCompletionTrait;
use chrono::{DateTime, NaiveDate, NaiveTime};
use cql3_parser::cassandra_ast::CassandraAST;
use cql3_parser::cassandra_statement::CassandraStatement;
use cql3_parser::common::{ColumnDefinition, DataType};
use dashmap::DashMap;
use futures_util::StreamExt;
use num_bigint::BigInt;
use scylla::authentication::PlainTextAuthenticator;

use scylla::client::session::Session;
use scylla::client::session_builder::SessionBuilder;
use scylla::value::{CqlDecimal, CqlValue, CqlVarint, Row};
use sqlx::types::BigDecimal;
use std::borrow::Cow;
use std::collections::HashMap;

use std::sync::Arc;
use std::time::Duration;
#[cfg(test)]
use tokio::sync::Mutex;
#[derive(Clone)]
pub struct CassandraDB {
    pub(crate) session: Arc<Session>,
    pub(crate) query_connection: Arc<DashMap<String, Arc<Session>>>,
    pub(crate) config: CassandraDBConfig,
    pub(crate) completion_id: AutoCompletionQueryId,
}

async fn make_session(config: &CassandraDBConfig) -> anyhow::Result<Session> {
    let mut builder = SessionBuilder::new().known_node(format!(
        "{}:{}",
        config.host,
        config.port.unwrap_or(9042)
    ));
    match (&config.username, &config.password) {
        (Some(user), Some(pass)) => {
            builder = builder.authenticator_provider(Arc::new(PlainTextAuthenticator::new(
                user.clone(),
                pass.clone(),
            )));
        }
        _ => {}
    }
    let session = builder.build().await?;
    if let Some(key_space) = config.db.as_ref() {
        session.use_keyspace(key_space, false).await?;
    }
    anyhow::Ok(session)
}

impl CassandraDB {
    pub async fn new(config: CassandraDBConfig) -> anyhow::Result<Self> {
        let session = tokio::time::timeout(
            config
                .conn_timeout
                .map(|x| x.into())
                .unwrap_or(Duration::from_secs(3)),
            async { make_session(&config).await },
        )
        .await??;
        anyhow::Ok(Self {
            session: Arc::new(session),
            config,
            query_connection: Default::default(),
            completion_id: Default::default(),
        })
    }
}

#[async_trait::async_trait]

impl ServerMetas for CassandraDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        anyhow::Ok(vec![])
    }
}

#[async_trait::async_trait]
impl CheckConn for CassandraDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let now = std::time::Instant::now();
        self.session
            .query_unpaged("SELECT now() FROM system.local;", &[])
            .await?;
        anyhow::Ok(now.elapsed())
    }
}

impl std::fmt::Debug for CassandraDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "host={} port={:?}", self.config.host, self.config.port)
    }
}

impl GetBackendKind for CassandraDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::Cassandra
    }
}

fn encode_row_as_json(cols: &[String], rows: Row) -> anyhow::Result<serde_json::Value> {
    debug_assert_eq!(cols.len(), rows.columns.len());

    let mut m = serde_json::Map::new();
    for (name, value) in cols.iter().zip(rows.columns) {
        let value = match value {
            Some(x) => encode_value_as_json(x)?,
            None => serde_json::Value::Null,
        };
        m.insert(name.clone(), value);
    }
    fn encode_value_as_json(value: CqlValue) -> anyhow::Result<serde_json::Value> {
        let ret = match value {
            CqlValue::Ascii(x) => serde_json::Value::from(x),
            CqlValue::Boolean(x) => serde_json::Value::from(x),
            CqlValue::Blob(items) => {
                let values: Vec<_> = items.into_iter().map(serde_json::Value::from).collect();
                serde_json::Value::Array(values)
            }
            CqlValue::Counter(counter) => serde_json::Value::from(counter.0),
            CqlValue::Decimal(c) => {
                let x = cql_decimal_to_bigdecimal(&c).to_string();
                serde_json::Value::String(x)
            }
            CqlValue::Date(cql_date) => serde_json::Value::from(cql_date.0),
            CqlValue::Double(x) => serde_json::Value::from(x),
            CqlValue::Duration(cql_duration) => {
                let mut obj = serde_json::Map::new();
                obj.insert(
                    "months".to_owned(),
                    serde_json::Value::from(cql_duration.months),
                );
                obj.insert(
                    "days".to_owned(),
                    serde_json::Value::from(cql_duration.days),
                );
                obj.insert(
                    "nanoseconds".to_owned(),
                    serde_json::Value::from(cql_duration.nanoseconds),
                );
                serde_json::Value::Object(obj)
            }
            CqlValue::Empty => serde_json::Value::Null,
            CqlValue::Float(x) => serde_json::Value::from(x),
            CqlValue::Int(x) => serde_json::Value::from(x),
            CqlValue::BigInt(x) => serde_json::Value::from(x),
            CqlValue::Text(x) => serde_json::Value::from(x),
            CqlValue::Timestamp(cql_timestamp) => serde_json::Value::from(cql_timestamp.0),
            CqlValue::Inet(ip_addr) => serde_json::Value::from(ip_addr.to_string()),
            CqlValue::List(cql_values)
            | CqlValue::Set(cql_values)
            | CqlValue::Vector(cql_values) => {
                let mut values = vec![];
                for c in cql_values {
                    let v = encode_value_as_json(c)?;
                    values.push(v);
                }
                serde_json::Value::Array(values)
            }
            CqlValue::Map(fields) => {
                let mut m = serde_json::Map::new();
                for (k, v) in fields {
                    let k = encode_value_as_string(k, false)?;
                    let v = encode_value_as_json(v)?;
                    m.insert(k, v);
                }
                serde_json::Value::Object(m)
            }
            CqlValue::UserDefinedType { fields, .. } => {
                let mut m = serde_json::Map::new();
                for f in fields {
                    let v = match f.1 {
                        Some(x) => encode_value_as_json(x)?,
                        None => serde_json::Value::Null,
                    };
                    m.insert(f.0, v);
                }
                serde_json::Value::Object(m)
            }
            CqlValue::SmallInt(x) => serde_json::Value::from(x),
            CqlValue::TinyInt(x) => serde_json::Value::from(x),
            CqlValue::Time(cql_time) => serde_json::Value::from(cql_time.0),
            CqlValue::Timeuuid(cql_timeuuid) => serde_json::Value::from(cql_timeuuid.to_string()),

            CqlValue::Uuid(uuid) => serde_json::Value::from(uuid.to_string()),
            CqlValue::Varint(cql_varint) => {
                let v = varint_2_bigint(&cql_varint);
                serde_json::Value::from(v.to_string())
            }
            CqlValue::Tuple(cql_values) => {
                let mut values = vec![];
                for v in cql_values {
                    let v = match v {
                        Some(v) => encode_value_as_json(v)?,
                        None => serde_json::Value::Null,
                    };
                    values.push(v);
                }
                serde_json::Value::Array(values)
            }
            _ => anyhow::bail!("unsupported type,{:?}", value),
        };

        anyhow::Ok(ret)
    }
    anyhow::Ok(serde_json::Value::Object(m))
}

fn format_date(mut days_since_epoch: u32) -> anyhow::Result<String> {
    // see   https://github.com/krojew/cdrs-tokio/issues/202
    days_since_epoch &= !(1 << 31);

    let date = NaiveDate::from_ymd_opt(1970, 1, 1)
        .unwrap()
        .checked_add_days(chrono::Days::new(days_since_epoch as i64 as u64));
    match date {
        Some(x) => anyhow::Ok(x.to_string()),

        None => anyhow::bail!("days is none"),
    }
}

fn format_timestamp(x: i64) -> anyhow::Result<String> {
    let time = DateTime::from_timestamp_millis(x);
    match time {
        Some(x) => anyhow::Ok(x.naive_local().to_string()),
        None => Err(anyhow::Error::msg(format!(""))),
    }
}

fn encode_row(row: Row) -> anyhow::Result<Vec<Option<String>>> {
    let mut values = vec![];
    for v in row.columns {
        let value = match v {
            Some(x) => Some(encode_value_as_string(x, false)?),
            None => None,
        };
        values.push(value);
    }

    anyhow::Ok(values)
}

fn format_time(x: i64) -> String {
    let time = NaiveTime::default() + std::time::Duration::from_micros(x as u64);
    time.to_string()
}

fn varint_2_bigint(c: &CqlVarint) -> BigInt {
    num_bigint::BigInt::from_signed_bytes_be(c.as_signed_bytes_be_slice())
}

fn cql_decimal_to_bigdecimal(cql_decimal: &CqlDecimal) -> BigDecimal {
    let (bytes, scale) = cql_decimal.as_signed_be_bytes_slice_and_exponent();
    if bytes.is_empty() {
        return BigDecimal::new(BigInt::from(0), scale as i64);
    }

    let big_int = BigInt::from_signed_bytes_be(bytes);

    // 注意：BigDecimal::new 的第二个参数是负数
    BigDecimal::new(big_int, scale as i64)
}

fn encode_value_as_string(value: CqlValue, is_nested: bool) -> anyhow::Result<String> {
    let v = match value {
        CqlValue::Ascii(x) | CqlValue::Text(x) => {
            if is_nested {
                format!("'{}'", escape_sql_str(&x))
            } else {
                x
            }
        }
        CqlValue::Boolean(x) => x.to_string(),
        CqlValue::Blob(x) => format!("0x{}", hex::encode(&x)),
        CqlValue::Counter(counter) => counter.0.to_string(),
        CqlValue::Decimal(c) => cql_decimal_to_bigdecimal(&c).to_string(),
        CqlValue::Date(cql_date) => format_date(cql_date.0)?,
        CqlValue::Double(x) => x.to_string(),
        CqlValue::Duration(..) => anyhow::bail!("duration not support"),
        CqlValue::Empty => "null".to_string(),
        CqlValue::Float(x) => x.to_string(),
        CqlValue::Int(x) => x.to_string(),
        CqlValue::BigInt(x) => x.to_string(),
        CqlValue::Timestamp(cql_timestamp) => format_timestamp(cql_timestamp.0)?,
        CqlValue::Inet(ip_addr) => ip_addr.to_string(),
        CqlValue::List(cql_values) => {
            let mut values = vec![];
            for v in cql_values {
                let v = encode_value_as_string(v, true)?;
                values.push(v);
            }
            format!("[{}]", values.join(","))
        }
        CqlValue::Map(m) => {
            let mut values = vec![];
            for (k, v) in m {
                let v = encode_value_as_string(v, true)?;
                let k = encode_value_as_string(k, true)?;
                values.push(format!("{}:{}", k, v));
            }
            format!("{{{}}}", values.join(","))
        }
        CqlValue::Set(cql_values) => {
            let mut values = vec![];
            for v in cql_values {
                let v = encode_value_as_string(v, true)?;
                values.push(v);
            }
            format!("{{{}}}", values.join(","))
        }
        CqlValue::UserDefinedType { fields, .. } => {
            let mut values = vec![];
            for (k, v) in fields {
                let v = match v {
                    Some(x) => encode_value_as_string(x, true)?,
                    None => "null".to_string(),
                };
                values.push(format!("{}:{}", k, v));
            }
            format!("{{{}}}", values.join(","))
        }
        CqlValue::SmallInt(x) => x.to_string(),
        CqlValue::TinyInt(x) => x.to_string(),
        CqlValue::Time(cql_time) => format_time(cql_time.0),
        CqlValue::Timeuuid(cql_timeuuid) => cql_timeuuid.to_string(),
        CqlValue::Tuple(cql_values) => {
            let mut values = vec![];
            for v in cql_values {
                let v = match v {
                    Some(v) => encode_value_as_string(v, true)?,
                    None => "null".to_string(),
                };
                values.push(v);
            }
            format!("({})", values.join(","))
        }
        CqlValue::Uuid(uuid) => uuid.to_string(),
        CqlValue::Varint(x) => varint_2_bigint(&x).to_string(),
        CqlValue::Vector(cql_values) => {
            let mut values = vec![];
            for v in cql_values {
                let v = encode_value_as_string(v, true)?;
                values.push(v);
            }
            format!("[{}]", values.join(","))
        }
        _ => anyhow::bail!(format!("unsupported type:{:?}", value)),
    };
    anyhow::Ok(v)
}
fn encode_rows(rows: Vec<Row>) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let mut ret = vec![];
    for row in rows {
        let v = encode_row(row)?;
        ret.push(v);
    }
    anyhow::Ok(ret)
}

fn sql_to_each(sql: &str) -> Vec<String> {
    separate_sql_by_sqlparser_tokenizer(sql, &sqlparser::dialect::GenericDialect {})
        .map_err(|e| log::error!("try separate sql failed,err:{}", e))
        .unwrap_or(vec![sql.to_string()])
}

impl CassandraDB {
    fn get_session_by_query_id(&self, query_id: Option<&str>) -> anyhow::Result<Arc<Session>> {
        match query_id {
            Some(query_id) => match self.query_connection.get(&query_id.to_string()) {
                Some(x) => anyhow::Ok(x.clone()),
                None => anyhow::bail!("query_id not found"),
            },
            None => anyhow::Ok(self.session.clone()),
        }
    }
    async fn export_as_xlsx(
        &self, query_id: Option<&str>, sql: &str, include_header: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let session = self.get_session_by_query_id(query_id)?;
        let pager = session.query_iter(sql, &[]).await?;
        let mut row_num = 0;
        if include_header {
            for (index, col) in pager.column_specs().iter().enumerate() {
                sender
                    .send(
                        XlsxEdit {
                            row: row_num,
                            column: index as rust_xlsxwriter::ColNum,
                            content: col.name().to_string(),
                        }
                        .to_string()
                        .into(),
                    )
                    .await?;
            }
            row_num += 1;
        }

        let mut stream = pager.rows_stream::<Row>()?;
        while let Some(row) = stream.next().await {
            let row = row?;
            let data = encode_row(row)?;
            export_query_as_xlsx_send_data(&mut row_num, vec![data], sender.clone()).await?;
        }
        anyhow::Ok(())
    }
}

#[async_trait::async_trait]

impl ExportQueryResult for CassandraDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }

    async fn export_query_result(
        &self, query_id: &str, sql: &str, kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::Csv {
                include_header,
                delimiter,
            } => {
                self.export_query_result_as_csv(
                    Some(query_id),
                    sql,
                    include_header,
                    delimiter,
                    sender,
                )
                .await
            }
            ExportKind::Json { pretty } => {
                self.export_query_result_as_json(Some(query_id), sql, pretty, sender)
                    .await
            }
            ExportKind::JsonLine => {
                self.export_query_result_as_json_line(Some(query_id), sql, sender)
                    .await
            }
            ExportKind::Sql {
                include_table_schema: _,
            } => ExportKind::cannot_query_result_export_as_sql(),
            ExportKind::Xlsx { include_header } => {
                self.export_as_xlsx(Some(query_id), sql, include_header, sender)
                    .await
            }
            ExportKind::Html { include_header } => {
                self.export_query_result_as_html(Some(query_id), sql, include_header, sender)
                    .await
            }
        }
    }
}

#[async_trait::async_trait]
impl QuerySql for CassandraDB {
    async fn execute_sql_no_open_conn(
        &self, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let mut ret = vec![];
        for sql in sql_to_each(sql).iter() {
            let resp = self.session.query_unpaged(sql.as_str(), &[]).await;
            match resp {
                Ok(_) => ret.push(Some("execute ok".to_string())),
                Err(err) => ret.push(Some(format!("err:{}", err))),
            }
        }
        anyhow::Ok(Ok(QueryResp {
            fields: vec![Field::from_name_type("execute result", "")],
            data: vec![ret],
            ..Default::default()
        }))
    }

    async fn execute_sql(
        &self, query_id: &str, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let conn = match self.query_connection.get(&query_id.to_string()) {
            Some(x) => x.clone(),
            None => anyhow::bail!("query connection closed???"),
        };
        let mut ret = vec![];
        for sql in sql_to_each(sql).iter() {
            let resp = conn.query_unpaged(sql.as_str(), &[]).await;
            match resp {
                Ok(_) => ret.push(Some("execute ok".to_string())),
                Err(err) => ret.push(Some(format!("err:{}", err))),
            }
        }
        anyhow::Ok(Ok(QueryResp {
            fields: vec![Field::from_name_type("execute result", "")],
            data: vec![ret],
            ..Default::default()
        }))
    }

    async fn query_sql(
        &self, query_id: &str, sql: &str, _max_row: usize, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let conn = match self.query_connection.get(&query_id.to_string()) {
            Some(x) => x.clone(),
            None => anyhow::bail!("query connection closed???"),
        };
        for sql in sql_to_each(sql) {
            let mut rows = vec![];

            let mut time_elapsed = TimeElapsed::new();
            let pager = conn.query_iter(sql.as_str(), &[]).await?;
            emit.send(
                QueryResp {
                    fields: pager
                        .column_specs()
                        .iter()
                        .map(|x| Field::from_name_type(x.name(), format!("{:?}", x.typ())))
                        .collect(),
                    sql: Some(sql.to_string()),
                    ..Default::default()
                }
                .into(),
            )?;
            let mut stream = pager.rows_stream::<Row>()?;
            while let Some(x) = stream.next().await {
                let row = x?;
                rows.push(row);
                if rows.len() >= self.query_buffer_size() || time_elapsed.elapsed() {
                    let d = encode_rows(std::mem::take(&mut rows))?;
                    emit.send(d.into())?;
                }
                let d = encode_rows(std::mem::take(&mut rows))?;
                emit.send(d.into())?;
            }
        }
        anyhow::Ok(())
    }

    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        let session = make_session(&self.config).await?;
        self.query_connection
            .insert(id.to_string(), Arc::new(session));
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        self.query_connection.remove(&id.to_string());
        anyhow::Ok(())
    }
}

impl CassandraDB {
    pub(crate) async fn list_table_impl(&self, keyspace: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            "SELECT table_name FROM system_schema.tables WHERE keyspace_name = '{keyspace}' order by table_name;"
        );
        log::info!("sql:{}", sql);
        let pager = self.session.query_iter(sql, &[]).await?;
        let mut stream = pager.rows_stream::<(String,)>()?;
        let mut ret = Vec::new();

        while let Some(name) = stream.next().await {
            let name = name?;
            ret.push(ListTable::from(name.0));
        }
        anyhow::Ok(ret)
    }
}

fn get_select_sql(
    name: &str, cols: String, where_str: Option<&str>, order_by: Option<&str>,
    allow_filtering: bool,
) -> String {
    format!(
        r#"SELECT {cols} FROM "{name}"  {} {} {}"#,
        if let Some(where_str) = where_str.as_ref() {
            format!(" WHERE {} ", where_str)
        } else {
            "".to_string()
        },
        if let Some(order) = order_by {
            format!(" ORDER BY {} ", order)
        } else {
            "".to_string()
        },
        if allow_filtering {
            "ALLOW FILTERING"
        } else {
            ""
        }
    )
}

impl CassandraDB {
    async fn export_query_result_as_json(
        &self, query_id: Option<&str>, sql: &str, pretty: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let session = self.get_session_by_query_id(query_id)?;
        let pager = session.query_iter(sql, &[]).await?;
        let cols: Vec<_> = pager
            .column_specs()
            .iter()
            .map(|x| x.name().to_owned())
            .collect();
        sender.send("[".into()).await?;
        let mut first = true;
        let mut stream = pager.rows_stream::<Row>()?;
        while let Some(row) = stream.next().await {
            let row = row?;
            let value = encode_row_as_json(&cols, row)?;
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

    async fn export_query_result_as_csv(
        &self, query_id: Option<&str>, sql: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let session = self.get_session_by_query_id(query_id)?;
        let pager = session.query_iter(sql, &[]).await?;
        if include_header {
            let cols: Vec<_> = pager
                .column_specs()
                .iter()
                .map(|x| Some(x.name().to_owned()))
                .collect();
            sender.send(create_csv(&[cols], delimiter).into()).await?;
        }
        let mut stream = pager.rows_stream::<Row>()?;
        while let Some(row) = stream.next().await {
            let row = row?;
            let d = encode_row(row)?;
            sender.send(create_csv(&[d], delimiter).into()).await?;
        }
        anyhow::Ok(())
    }

    async fn export_query_result_as_html(
        &self, query_id: Option<&str>, sql: &str, include_header: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let session = self.get_session_by_query_id(query_id)?;
        let pager = session.query_iter(sql, &[]).await?;
        if include_header {
            let cols: Vec<_> = pager
                .column_specs()
                .iter()
                .map(|x| x.name().to_owned())
                .collect();
            sender.send(create_html_table_header(&cols).into()).await?;
        }
        let mut stream = pager.rows_stream::<Row>()?;
        while let Some(row) = stream.next().await {
            let row = row?;
            let d = encode_row(row)?;
            sender.send(create_html_table_data(&[d]).into()).await?;
        }
        anyhow::Ok(())
    }

    async fn export_query_result_as_json_line(
        &self, query_id: Option<&str>, sql: &str,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let session = self.get_session_by_query_id(query_id)?;
        let pager = session.query_iter(sql, &[]).await?;
        let cols: Vec<_> = pager
            .column_specs()
            .iter()
            .map(|x| x.name().to_owned())
            .collect();
        sender.send("[".into()).await?;
        let mut stream = pager.rows_stream::<Row>()?;
        while let Some(row) = stream.next().await {
            let row = row?;
            let value = encode_row_as_json(&cols, row)?;
            sender
                .send(Cow::Owned(serde_json::to_string(&value)?))
                .await?;
        }
        anyhow::Ok(())
    }
}

impl ReadOnlyTrait for CassandraDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}
impl CassandraDB {
    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_csv(None, sql.as_str(), include_header, delimiter, sender)
            .await
    }
    async fn export_table_as_html(
        &self, table_name: &str, include_header: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_html(None, sql.as_str(), include_header, sender)
            .await
    }

    async fn export_table_as_json(
        &self, table_name: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_json(None, sql.as_str(), pretty, sender)
            .await
    }

    async fn export_table_as_json_line(
        &self, table_name: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_json_line(None, sql.as_str(), sender)
            .await
    }

    async fn export_table_as_xlsx(
        &self, table_name: &str, include_header: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        log::info!("sql:{}", sql);
        self.export_as_xlsx(None, sql.as_str(), include_header, sender)
            .await
    }

    async fn export_table_as_sql(
        &self, table_name: &str, include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            let ss = SchemaTrait::create_table_and_index_sql(self, table_name).await?;
            sender.send(Cow::Owned(ss.create_table.clone())).await?;
            sender.send(";\n".into()).await?;
            for index in ss.indexes.into_iter() {
                sender.send(Cow::Owned(index)).await?;
                sender.send(";\n".into()).await?;
            }
        }
        let cols = self.show_table_columns(table_name).await?;
        let sql = format!(
            r#"SELECT {} from "{}""#,
            self.backend_name().format_fields(Some(&cols)),
            table_name
        );
        log::info!("sql:{}", sql);
        let pager = self.session.query_iter(sql.as_str(), &[]).await?;

        let mut stream = pager.rows_stream::<Row>()?;
        while let Some(row) = stream.next().await {
            let row = row?;
            let data = encode_row(row)?;
            let mut m = HashMap::new();
            cols.iter().zip(data.iter()).for_each(|(f, v)| {
                m.insert(f.field.clone(), v.clone());
            });
            let sql = self.insert_sql(self.config.db.as_deref(), table_name, &cols, &m)?;
            let sql = format!("{sql};\n");
            sender.send(Cow::Owned(sql)).await?;
        }
        anyhow::Ok(())
    }
}

impl CassandraDB {
    pub(crate) async fn list_materialized_view_impl(
        &self, key_space: &str,
    ) -> anyhow::Result<Vec<ListTable>> {
        let sql =
            format!("select view_name from system_schema.views where keyspace_name='{key_space}'");
        log::info!("sql:{}", sql);
        let rows = self.session.query_iter(sql, &[]).await?;
        let mut stream = rows.rows_stream::<(String,)>()?;
        let mut ret = Vec::new();
        while let Some(name) = stream.next().await {
            let name = name?;
            ret.push(ListTable::from(name.0));
        }

        anyhow::Ok(ret)
    }
}

/// Cassandra / CQL 里可直接写在语句中的 CURRENT_* 字面量
pub const CQL_CURRENT_LITERALS: &[&str] = &[
    "CURRENT_TIMESTAMP", // 微秒时间戳，如 1692962187000000
    "CURRENT_TIME",      // 当前 64-bit 时间值（time）
    "CURRENT_DATE",      // 当前日期（date）
];

impl SqlBuilder for CassandraDB {
    const CURRENT_VALUES: &'static [&'static str] = CQL_CURRENT_LITERALS;
    fn put_value_in_sql(
        &self, typ: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        if self.hit_current_value(value) {
            return anyhow::Result::Ok(value.option_must_be_some()?.to_string());
        }
        put_value_in_sql(typ, value)
    }
}

impl CassandraDB {
    pub(crate) async fn list_created_types_impl(
        &self, schema: &str,
    ) -> anyhow::Result<Vec<CreatedTypeDef>> {
        let sql = format!(
            "SELECT type_name FROM system_schema.types where  keyspace_name = '{}'",
            schema
        );
        log::info!("sql:{}", sql);
        let pager = self.session.query_iter(sql, &[]).await?;
        let mut stream = pager.rows_stream::<(String,)>()?;
        let mut ret = Vec::new();
        while let Some(row) = stream.next().await {
            let row = row?;
            let sql = format!("DESCRIBE TYPE {}", row.0);
            log::info!("sql:{}", sql);
            let r = self
                .session
                .query_unpaged(sql.as_str(), &[])
                .await?
                .into_rows_result()?;
            let mut pager = r.rows::<(String, String, String, String)>()?;
            let mut create_statement = "".to_string();
            while let Some(row) = pager.next() {
                let row = row?;
                create_statement = row.3;
                break;
            }
            ret.push(CreatedTypeDef {
                name: row.0,
                sql: create_statement.into(),
            });
        }
        anyhow::Ok(ret)
    }
}
#[async_trait::async_trait]
impl SchemaTrait for CassandraDB {
    fn list_sql_current_values(&self) -> &'static [&'static str] {
        <Self as SqlBuilder>::CURRENT_VALUES
    }

    async fn drop_created_type(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP TYPE  "{name}""#);
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql, &[]).await?;
        anyhow::Ok(())
    }

    async fn list_created_types(&self) -> anyhow::Result<Vec<CreatedTypeDef>> {
        self.list_created_types_impl(self.config.key_space_str()?)
            .await
    }

    async fn drop_materialized_view(&self, name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP MATERIALIZED VIEW "{name}";"#);
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql, &[]).await?;
        anyhow::Ok(())
    }

    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_materialized_view_impl(self.config.key_space_str()?)
            .await
    }

    async fn export_table_or_view(
        &self, name: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::Csv {
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
            ExportKind::Xlsx { include_header } => {
                self.export_table_as_xlsx(name, include_header, sender)
                    .await
            }
            ExportKind::Html { include_header } => {
                self.export_table_as_html(name, include_header, sender)
                    .await
            }
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn create_index(
        &self, table: &str, name: &str, _kind: IndexKind, fields: &[String],
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"CREATE INDEX "{name}" ON "{table}" ({});"#,
            fields.join(",")
        );
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP INDEX IF EXISTS "{index}";"#);
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql, &[]).await?;
        anyhow::Ok(())
    }

    async fn create_table_and_index_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let sql = format!(r#"DESCRIBE TABLE "{table_name}""#);
        log::info!("sql:{}", sql);
        let rows = self.session.query_iter(sql.as_ref(), &[]).await?;

        let mut stream = rows.rows_stream::<(String, String, String, String)>()?;

        while let Some(row) = stream.next().await {
            let row = row?;
            return anyhow::Ok(CreateTableAndIndexSql {
                create_table: row.3,
                indexes: vec![],
            });
        }

        anyhow::bail!("not found create table sql")
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        let sql = self.create_table_and_index_sql(table).await?.create_table;
        // replace table name.
        let sql = sql.replacen(table, dup, 1);
        log::info!("sql:{}", sql);
        log::info!("sql:{}", sql);
        log::info!("dup table create table sql:{}", sql);
        // here we create the table
        self.session.query_unpaged(sql, &[]).await?;
        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
        let j1 = {
            // load all data
            let x = Clone::clone(self);
            let table = table.to_owned();
            tokio::spawn(async move {
                x.export_table_or_view(
                    &table,
                    ExportKind::Sql {
                        // we only need data now.
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
                while let Some(sql) = receiver.recv().await {
                    // replace table name.
                    let sql = sql.replacen(table.as_str(), &dup, 1);
                    x.session.query_unpaged(sql, &[]).await?;
                }
                anyhow::Ok(())
            })
        };
        j1.await??;
        j2.await??;
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(self.config.key_space_str()?).await
    }

    // cassandra can use this function for materialized view too.
    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let sql = format!(
            r#" SELECT column_name, type, kind
        FROM system_schema.columns
        WHERE keyspace_name = '{}' AND table_name = '{}';"#,
            self.config.key_space_str()?,
            table_name
        );
        log::info!("sql:{}", sql);
        let pager = self.session.query_iter(sql.as_str(), &[]).await?;
        let mut stream = pager.rows_stream::<(String, String, String)>()?;
        let mut ret = Vec::new();
        while let Some(row) = stream.next().await {
            let (name, data_type, kind) = row?;
            let is_primary = kind == "partition_key" || kind == "clustering";
            ret.push(Field {
                field: name,
                data_type,
                nullable: !is_primary,
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
            allow_filtering,
            ..
        } = &para;
        let cols = self.show_table_columns(table_name).await?;
        let cols_string = self.backend_name().format_fields(Some(&cols));
        let cql = get_select_sql(
            table_name,
            cols_string,
            where_str.as_deref(),
            order_by.as_deref(),
            *allow_filtering,
        );
        emit.send(
            QueryResp {
                fields: cols,
                sql: Some(cql.clone()),
                ..Default::default()
            }
            .into(),
        )?;

        let pager = self.session.query_iter(cql.as_str(), &[]).await?;
        let mut stream = pager.rows_stream::<Row>()?;
        let mut time_elapsed = TimeElapsed::new();
        let mut rows = vec![];
        while let Some(row) = stream.next().await {
            let row = row?;
            rows.push(row);
            if rows.len() >= self.query_buffer_size() || time_elapsed.elapsed() {
                let d = encode_rows(std::mem::take(&mut rows))?;

                emit.send(d.into())?;
            }
        }
        let d = encode_rows(std::mem::take(&mut rows))?;
        emit.send(d.into())?;
        anyhow::Ok(())
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let sql = format!(
            "SELECT index_name from system_schema.indexes WHERE  table_name = '{table_name}' AND keyspace_name = '{}'",
            self.config.key_space_str()?
        );
        log::info!("sql:{}", sql);
        let pager = self.session.query_iter(sql, &[]).await?;
        let mut stream = pager.rows_stream::<(String,)>()?;

        let mut ret = Vec::new();
        while let Some(row) = stream.next().await {
            let row = row?;
            ret.push(Index {
                name: row.0.into(),
                kind: IndexKind::Index,
                columns: vec![],
            });
        }
        anyhow::Ok(ret)
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;
        let sql = self.insert_sql(None, table_name, &fs, &values)?;
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql.as_str(), &[]).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(1),
        })
    }

    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fs = self.show_table_columns(table_name).await?;
        let sql = self.update_sql(self.config.db.as_deref(), table_name, &fs, &values, &wheres)?;

        log::info!("update sql:{}", sql);
        self.session.query_unpaged(sql.as_str(), &[]).await?;
        anyhow::Ok(SqlExecuteRet::from((sql, 1)))
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"TRUNCATE "{table_name}""#);
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn table_delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_table_columns(table_name).await?;

        let sql = self.delete_sql(self.config.db.as_deref(), table_name, &fs, &wheres)?;
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql.as_str(), &[]).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(1),
        })
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(r#"DROP TABLE "{}";"#, table);
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn add_table_column(
        &self, table: &str, name: &str, col: ColumnType,
    ) -> anyhow::Result<()> {
        let sql = format!("ALTER TABLE {table} ADD {name} {}", col.typ);
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, _typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        let sql = format!("ALTER TABLE {table} RENAME {old_name} TO {new_name}");
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        let sql = format!("ALTER TABLE {table} DROP {col};");
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn show_view(
        &self, view_name: &str, para: ShowTableParameter, status_emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        self.show_table(view_name, para, status_emit).await
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        self.show_table_columns(view_name).await
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_udf_impl(self.config.key_space_str()?).await
    }

    async fn show_udf(
        &self, name: &str, extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<DefAndMetas> {
        // add parameter to udf have this error
        // cassandra@cqlsh:test> DESCRIBE FUNCTION  test.test_udf(int);
        // Improper DESCRIBE command.
        let sql = format!(r#" DESCRIBE FUNCTION  "{}""#, name);
        log::info!("sql:{}", sql);
        let body = self.session.query_iter(sql, &[]).await?;
        let mut stream = body.rows_stream::<(String, Option<String>, String, String)>()?;
        let parameters = match &extra {
            Some(extra) => match &extra {
                UdfOrProcedureExtraParameter::Cassandra { parameters } => parameters,
                _ => {
                    anyhow::bail!("show udf for cassandra need extra parameter.");
                }
            },
            None => anyhow::bail!("show udf for cassandra need extra parameter."),
        };
        while let Some(row) = stream.next().await {
            let (_, typ, _, create_statement) = row?;
            let parameters_try_match = extract_parameter_for_function(&create_statement)?;
            if parameters.len() != parameters_try_match.len() {
                continue;
            }
            if parameters
                .iter()
                .zip(parameters_try_match.iter())
                .any(|(x, y)| {
                    let x_type = x.replace(" ", "");
                    let y_type = y.data_type.to_string().replace(" ", "");
                    !x_type.eq_ignore_ascii_case(&y_type)
                })
            {
                continue;
            }
            return anyhow::Ok(DefAndMetas {
                def: create_statement,
                metas: vec![MetaValue::new("type", typ)],
            });
        }

        anyhow::bail!(format!(
            "no match for parameters:{:?},this should be a bug",
            parameters
        ));
    }

    async fn drop_udf(
        &self, name: &str, extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        let parameters = match &extra {
            Some(extra) => match &extra {
                UdfOrProcedureExtraParameter::Cassandra { parameters } => parameters,
                _ => {
                    anyhow::bail!("drop udf for cassandra need extra parameter.");
                }
            },
            None => anyhow::bail!("drop udf for cassandra need extra parameter."),
        };

        let p = format!("({})", parameters.join(","));

        let sql = format!(r#"DROP FUNCTION "{name}"{p} "#);

        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql, &[]).await?;
        anyhow::Ok(())
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        LexerCompletionTrait::get_types(self).await
    }
}

fn put_value_in_sql(typ: &str, value: Option<&str>) -> anyhow::Result<String> {
    let typ = typ.to_lowercase();
    log::trace!("encode value '{:?}' on type '{}'", value, typ);
    match typ.as_str() {
        "ascii" | "varchar" | "text" => anyhow::Ok(
            value
                .map(|x| format!("'{}'", escape_sql_str(x)))
                .unwrap_or("null".to_string()),
        ),
        "timestamp" | "date" | "time" | "inet" => anyhow::Ok(
            value
                .map(|x| format!("'{}'", x))
                .unwrap_or("null".to_string()),
        ),
        _ => anyhow::Ok(value.map(|x| x.to_string()).unwrap_or("null".to_string())),
    }
}

// TODO this parse only support one parameter or `cassandra`
// support multi parameter for aggregate function.
fn extract_parameter_for_aggregate(sql: &str) -> anyhow::Result<DataType> {
    let x = CassandraAST::new(sql);

    if x.statements.len() != 1 {
        anyhow::bail!("should contains one statement.")
    }

    let s = &x.statements[0];

    match &s.statement {
        CassandraStatement::CreateAggregate(x) => anyhow::Ok(x.data_type.clone()),
        _ => {
            anyhow::bail!("this should be aggregate function statement.")
        }
    }
}

fn extract_parameter_for_function(sql: &str) -> anyhow::Result<Vec<ColumnDefinition>> {
    let x = CassandraAST::new(sql);

    if x.statements.len() != 1 {
        anyhow::bail!("should contains one statement.")
    }

    let s = &x.statements[0];

    match &s.statement {
        CassandraStatement::CreateFunction(x) => anyhow::Ok(x.params.clone()),
        _ => {
            anyhow::bail!("this should be create function statement.")
        }
    }
}

#[async_trait::async_trait]
impl DBTrait for CassandraDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        let pager = self.session.query_iter("DESCRIBE KEYSPACES", &[]).await?;
        let mut stream = pager.rows_stream::<(String, String, String)>()?;
        let mut ret = Vec::new();
        while let Some(row) = stream.next().await {
            let row = row?;
            ret.push(row.0);
        }
        anyhow::Ok(ret)
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let mut config = self.config.clone();
        config.db = Some(name.to_string());
        let x = CassandraDB::new(config).await?;
        anyhow::Ok(Arc::new(x))
    }

    async fn create_schema(
        &self, name: &str, extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.can_write()?;

        let extra = match &extra {
            Some(x) => x,
            None => anyhow::bail!("cassandra need extra parameter."),
        };
        let (replication_strategy, replication_factor) = match extra {
            CreateSchemaExtra::Cassandra {
                replication_strategy,
                replication_factor,
            } => (replication_strategy, replication_factor),
        };

        let sql = format!(
            r#"
            CREATE KEYSPACE {}
            WITH REPLICATION = {{
                'class': '{}',
                'replication_factor': {}
            }};
        "#,
            name, replication_strategy, replication_factor
        );
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql, &[]).await?;
        anyhow::Ok(())
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!("DROP KEYSPACE {name}");
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql.as_str(), &[]).await?;
        anyhow::Ok(())
    }
}

impl CassandraDB {
    pub(crate) async fn list_udf_impl(&self, key_space: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"SELECT function_name, argument_types, return_type, language 
                FROM system_schema.functions 
                WHERE keyspace_name = '{key_space}';"#
        );
        log::info!("sql:{}", sql);
        let pager = self.session.query_iter(sql, &[]).await?;
        let mut stream =
            pager.rows_stream::<(String, Vec<String>, Option<String>, Option<String>)>()?;
        let mut ret = Vec::new();
        while let Some(row) = stream.next().await {
            let (name, argument_types, return_type, language) = row?;
            ret.push(NamedStaff {
                name,
                parameters: None,
                cassandra_parameters: Some(argument_types),
                metas: vec![
                    MetaValue::new("return_type", return_type),
                    MetaValue::new("language", language),
                ],
            });
        }
        anyhow::Ok(ret)
    }
}

#[cfg(test)]
pub(crate) async fn make_test_object(db: Option<String>) -> CassandraDB {
    use crate::{
        TEST_HOST,
        datasource_config::{CassandraSsl, SSLFiles},
    };
    let ss = CassandraDB::new(CassandraDBConfig {
        host: TEST_HOST.to_string(),
        port: Some(9042),
        username: Some("cassandra".to_string()),
        password: Some("cassandra".to_string()),
        read_only: false,
        db,
        conn_timeout: Some(500000.into()),
        save_password: true,
        ssl: CassandraSsl {
            ssl_files: SSLFiles {
                ssl_ca: None,
                ssl_cert: None,
                ssl_key: None,
            },
        },
    })
    .await
    .unwrap();
    ss
}

#[cfg(test)]
pub(crate) fn test_lock() -> &'static Mutex<()> {
    static LOCK: Mutex<()> = Mutex::const_new(());
    &LOCK
}

impl CassandraDB {
    pub async fn list_aggregate_impl(&self, key_space: &str) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = format!(
            r#"
        SELECT aggregate_name, argument_types, return_type, state_func, final_func, initcond
            FROM system_schema.aggregates
            WHERE keyspace_name = '{key_space}';
        "#
        );
        log::info!("sql:{}", sql);
        let pager = self.session.query_iter(sql.as_str(), &[]).await?;
        let mut stream = pager.rows_stream::<(
            String,
            Option<Vec<String>>,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
        )>()?;
        let mut ret = Vec::new();
        while let Some(row) = stream.next().await {
            let (name, argument_types, return_type, state_func, final_func, initcond) = row?;

            ret.push(NamedStaff {
                parameters: None,
                name,
                metas: vec![
                    MetaValue::new("return_type", return_type),
                    MetaValue::new("state_func", state_func),
                    MetaValue::new("final_func", final_func),
                    MetaValue::new("initcond", initcond),
                ],
                cassandra_parameters: Some(argument_types.unwrap_or_default()),
            });
        }

        anyhow::Ok(ret)
    }

    pub async fn list_aggregate(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.list_aggregate_impl(self.config.key_space_str()?).await
    }

    pub async fn show_aggregate(
        &self, name: &str, parameters: Vec<String>,
    ) -> anyhow::Result<DefAndMetas> {
        let sql = format!("DESCRIBE AGGREGATE  {name}");
        log::info!("sql:{}", sql);
        let body = self.session.query_iter(sql.as_str(), &[]).await?;
        let mut steam = body.rows_stream::<(String, Option<String>, String, String)>()?;

        while let Some(row) = steam.next().await {
            let (_, typ, _, create_statement) = row?;
            let parameters_try_match = vec![extract_parameter_for_aggregate(&create_statement)?];
            if parameters.len() != parameters_try_match.len() {
                continue;
            }
            if parameters
                .iter()
                .zip(parameters_try_match.iter())
                .any(|(x, y)| {
                    let x_type = x.replace(" ", "");
                    let y_type = y.to_string().replace(" ", "");
                    !x_type.eq_ignore_ascii_case(&y_type)
                })
            {
                continue;
            }
            return anyhow::Ok(DefAndMetas {
                def: create_statement,
                metas: vec![MetaValue::new("type", typ)],
            });
        }
        anyhow::bail!(format!(
            "no match for parameter:{:?},this should be a bug",
            parameters
        ));
    }

    pub async fn drop_aggregate(&self, name: &str, parameters: Vec<String>) -> anyhow::Result<()> {
        let p = format!("({})", parameters.join(","));
        let sql = format!("DROP AGGREGATE {name}{p} ");
        log::info!("sql:{}", sql);
        self.session.query_unpaged(sql, &[]).await?;
        anyhow::Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enable_test_log;
    #[tokio::test]
    async fn test_list_schema() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(None).await;
        let _ = db
            .create_schema(
                "test",
                Some(CreateSchemaExtra::Cassandra {
                    replication_strategy: "SimpleStrategy".to_string(),
                    replication_factor: 1,
                }),
            )
            .await;

        let schemas = DBTrait::list_schema(&db).await.unwrap();
        println!("list schema:{:?}", schemas)
    }

    #[tokio::test]
    async fn test_aggregate() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        {
            // drop some function.
            let _ = db
                .drop_aggregate("test_aggregate", vec!["int".to_string()])
                .await;
            let _ = db
                .drop_udf(
                    "sum_state_function",
                    Some(UdfOrProcedureExtraParameter::Cassandra {
                        parameters: vec!["int".to_string(), "int".to_string()],
                    }),
                )
                .await;

            let _ = db
                .drop_udf(
                    "sum_final_function",
                    Some(UdfOrProcedureExtraParameter::Cassandra {
                        parameters: vec!["int".to_string()],
                    }),
                )
                .await;
        }
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _resp = db
            .execute_sql(
                query_id,
                r#"
        CREATE OR REPLACE FUNCTION sum_state_function(state int, val int)
            RETURNS NULL ON NULL INPUT
            RETURNS int
            LANGUAGE java
            AS $$
                return state + val; 
            $$;
        "#,
            )
            .await
            .unwrap()
            .unwrap();

        let _resp = db
            .execute_sql(
                query_id,
                r#"
                CREATE OR REPLACE FUNCTION sum_final_function(state int)
                RETURNS NULL ON NULL INPUT
                RETURNS int
                LANGUAGE java
                AS $$
                    return state; // 直接返回累加结果
                $$;
    "#,
            )
            .await
            .unwrap()
            .unwrap();

        let _resp = db
            .execute_sql(
                query_id,
                r#"
            CREATE AGGREGATE test_aggregate(int)
            SFUNC sum_state_function
            STYPE int
            FINALFUNC sum_final_function
            INITCOND 0;
"#,
            )
            .await
            .unwrap();

        let list = db.list_aggregate().await.unwrap();

        list.iter().for_each(|x| println!("aggregate:{:?}", x));

        let details = db
            .show_aggregate("test_aggregate", vec!["int".to_string()])
            .await
            .unwrap();

        println!("details:{:?}", details);
    }

    #[tokio::test]
    async fn test_check_conn() {
        let _guard = test_lock().lock().await;

        let db = make_test_object(None).await;
        let d = crate::db::CheckConn::check_conn(&db).await.unwrap();
        println!("d:{:?}", d);
    }
    fn new_test_show_parameter() -> ShowTableParameter {
        ShowTableParameter {
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_a_lot_data() {
        let _guard = test_lock().lock().await;
        let table_name = "test_a_lot_data";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_a_lot_data (
                    id UUID PRIMARY KEY,
                    ascii_column VARCHAR,
                );
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let size = 10000;
        for k in 0..size {
            let mut values = HashMap::new();
            values.insert("id".to_string(), Some("uuid()".to_string()));
            values.insert(
                "ascii_column".to_string(),
                Some(format!("example_ascii {}", k)),
            );
            db.table_insert(table_name, values).await.unwrap();
            if k % 500 == 0 {
                println!("insert {}", k);
            }
        }
    }

    #[tokio::test]
    async fn test_sort() {
        let _guard = test_lock().lock().await;

        let table_name = "test_sort";

        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_sort (
                id UUID,
                sort_key INT,
                name TEXT,
                value INT,
                PRIMARY KEY (id , sort_key )
            ) ;

        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
        INSERT INTO test_sort (sort_key, id, name, value) VALUES (1, uuid(), 'Alice', 30);
        INSERT INTO test_sort (sort_key, id, name, value) VALUES (1, uuid(), 'Bob', 20);
        INSERT INTO test_sort (sort_key, id, name, value) VALUES (1, uuid(), 'Charlie', 25);
        INSERT INTO test_sort (sort_key, id, name, value) VALUES (2, uuid(), 'Diana', 35);
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let resp = db
            .show_table_collect(table_name, new_test_show_parameter())
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }

    #[tokio::test]
    async fn test_show_table_column() {
        let _guard = test_lock().lock().await;

        let db = make_test_object(Some("test".to_string())).await;
        let fields = db.show_table_columns("example_table").await.unwrap();
        for field in fields {
            println!("field:{:?}", field);
        }
    }

    #[tokio::test]
    async fn test_duplicate() {
        let _guard = test_lock().lock().await;
        let table_name = "test_duplicate_table";
        let table_dup = "test_duplicate";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let _ = db.drop_table(table_dup, false).await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_duplicate_table (
                id UUID PRIMARY KEY,
                name TEXT,
                value INT
            );

        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.execute_sql(
            query_id,
            r#"
        INSERT INTO test_duplicate_table (id, name, value) VALUES (uuid(), 'Alice', 10);
        INSERT INTO test_duplicate_table (id, name, value) VALUES (uuid(), 'Bob', 20);
        INSERT INTO test_duplicate_table (id, name, value) VALUES (uuid(), 'Charlie', 30);
    "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.duplicate_table(table_name, table_dup).await.unwrap();

        let resp = db
            .show_table_collect(table_dup, new_test_show_parameter())
            .await
            .unwrap();
        println!("resp:{:?}", resp);
    }

    #[tokio::test]
    async fn test_list_created_types() {
        let _guard = test_lock().lock().await;

        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_created_type("test_type").await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TYPE test_type (
                id int,
                name text,
                created_at timestamp
            );
        "#,
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
    async fn test_types1() {
        let now = std::time::Instant::now();
        let _guard = test_lock().lock().await;
        let table_name = "test_types1";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;

        println!("drop table:{:?}", now.elapsed());
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_types1 (
                    id UUID PRIMARY KEY,
                    ascii_column ASCII,
                    bigint_column BIGINT,
                    blob_column BLOB,
                    boolean_column BOOLEAN,
                    decimal_column DECIMAL,
                    double_column DOUBLE
                );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        println!("create table:{:?}", now.elapsed());
        let mut values = HashMap::new();
        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert(
            "ascii_column".to_string(),
            Some("example_ascii".to_string()),
        );
        values.insert(
            "bigint_column".to_string(),
            Some("1234567890123456789".to_string()),
        );
        values.insert("blob_column".to_string(), Some("0x48656c6c6f".to_string()));
        values.insert("boolean_column".to_string(), Some("true".to_string()));

        values.insert("decimal_column".to_string(), Some("123.45".to_string()));
        values.insert("double_column".to_string(), Some("3.14159".to_string()));
        db.table_insert(table_name, values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;
            println!("resp:{:?}", resp);
        }
        {
            // export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }
    #[tokio::test]
    async fn test_types1_list() {
        let _guard = test_lock().lock().await;

        let table_name = "test_types_list1";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_types_list1 (
                    id UUID PRIMARY KEY,
                    ascii_column list<ASCII>,
                    bigint_column list<BIGINT>,
                    blob_column list<BLOB>,
                    boolean_column list<BOOLEAN>,
                    decimal_column list<DECIMAL>,
                    double_column list<DOUBLE>
                );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert(
            "ascii_column".to_string(),
            Some("['example_ascii']".to_string()),
        );
        values.insert(
            "bigint_column".to_string(),
            Some("[1234567890123456789]".to_string()),
        );
        values.insert(
            "blob_column".to_string(),
            Some("[0x48656c6c6f]".to_string()),
        );
        values.insert("boolean_column".to_string(), Some("[true]".to_string()));

        values.insert("decimal_column".to_string(), Some("[123.45]".to_string()));
        values.insert("double_column".to_string(), Some("[3.14159]".to_string()));

        db.table_insert(table_name, values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;

            println!("resp:{:?}", resp);
        }
        {
            // export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test]
    async fn test_types2() {
        let _guard = test_lock().lock().await;
        let table_name = "test_types2";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_types2 (
                    id UUID PRIMARY KEY,
                    float_value FLOAT,
                    int_value INT,
                    timestamp_value TIMESTAMP,
                    varchar_value VARCHAR,
                    varint_value VARINT
                );
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        // INSERT INTO test_types2 (id, float_value, int_value, timestamp_value,
        // varchar_value, varint_value) VALUES (uuid(), 3.14, 42, '2023-01-01
        // 10:00:00', 'Sample Text', '123456789012345678901234567890');
        //

        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert("float_value".to_string(), Some("3.14".to_string()));
        values.insert("int_value".to_string(), Some("42".to_string()));
        values.insert(
            "timestamp_value".to_string(),
            Some("2023-01-01 10:00:00".to_string()),
        );
        values.insert("varchar_value".to_string(), Some("Sample Text".to_string()));

        values.insert(
            "varint_value".to_string(),
            Some("123456789012345678901234567890".to_string()),
        );

        db.table_insert(table_name, values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;

            println!("resp:{:?}", resp);
        }
        {
            // export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test]
    async fn test_types2_list() {
        let _guard = test_lock().lock().await;
        let table_name = "test_types_list2";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_types_list2 (
                    id UUID PRIMARY KEY,
                    float_value list<FLOAT>,
                    int_value list<INT>,
                    timestamp_value list<TIMESTAMP>,
                    varchar_value list<VARCHAR>,
                    varint_value list<VARINT>
                );
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        // INSERT INTO test_types2 (id, float_value, int_value, timestamp_value,
        // varchar_value, varint_value) VALUES (uuid(), 3.14, 42, '2023-01-01
        // 10:00:00', 'Sample Text', '123456789012345678901234567890');
        //

        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert("float_value".to_string(), Some("[3.14]".to_string()));
        values.insert("int_value".to_string(), Some("[42]".to_string()));
        values.insert(
            "timestamp_value".to_string(),
            Some("['2023-01-01 10:00:00']".to_string()),
        );
        values.insert(
            "varchar_value".to_string(),
            Some("['Sample Text']".to_string()),
        );

        values.insert(
            "varint_value".to_string(),
            Some("[123456789012345678901234567890]".to_string()),
        );

        db.table_insert(table_name, values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;

            println!("resp:{:?}", resp);
        }
        {
            // export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test]
    async fn test_types3() {
        let _guard = test_lock().lock().await;
        let table_name = "test_types3";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_types3 (
                    id UUID PRIMARY KEY,
                    timeuuid_value TIMEUUID,
                    inet_value INET,
                    date_value DATE,
                    time_value TIME,
                    smallint_value SMALLINT,
                    tinyint_value TINYINT
                );

        "#,
        )
        .await
        .unwrap()
        .unwrap();
        // INSERT INTO test_types3 (id, timeuuid_value, inet_value, date_value,
        // time_value, smallint_value, tinyint_value) VALUES (
        // uuid(),
        // now(),                          -- 生成当前的timeuuid
        // '192.168.1.1',                 -- 示例IP地址
        // '2023-01-01',                  -- 示例日期
        // '10:30:00',                    -- 示例时间
        // 123,                            -- 示例smallint值
        // 5,                              -- 示例tinyint值
        // );

        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert("timeuuid_value".to_string(), Some("now()".to_string()));
        values.insert("inet_value".to_string(), Some("192.168.1.1".to_string()));
        values.insert("date_value".to_string(), Some("2023-01-01".to_string()));
        values.insert("time_value".to_string(), Some("10:30:00".to_string()));

        values.insert("smallint_value".to_string(), Some("123".to_string()));
        values.insert("tinyint_value".to_string(), Some("5".to_string()));

        db.table_insert(table_name, values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;

            println!("resp:{:?}", resp);
        }
        {
            // export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test]
    async fn test_types_list3() {
        let _guard = test_lock().lock().await;
        let table_name = "test_types_list3";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_types_list3 (
                    id UUID PRIMARY KEY,
                    timeuuid_value list<TIMEUUID>,
                    inet_value list<INET>,
                    date_value list<DATE>,
                    time_value list<TIME>,
                    smallint_value list<SMALLINT>,
                    tinyint_value list<TINYINT>
                );

        "#,
        )
        .await
        .unwrap()
        .unwrap();
        // INSERT INTO test_types3 (id, timeuuid_value, inet_value, date_value,
        // time_value, smallint_value, tinyint_value) VALUES (
        // uuid(),
        // now(),                          -- 生成当前的timeuuid
        // '192.168.1.1',                 -- 示例IP地址
        // '2023-01-01',                  -- 示例日期
        // '10:30:00',                    -- 示例时间
        // 123,                            -- 示例smallint值
        // 5,                              -- 示例tinyint值
        // );

        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert("timeuuid_value".to_string(), Some("[now()]".to_string()));
        values.insert(
            "inet_value".to_string(),
            Some("['192.168.1.1']".to_string()),
        );
        values.insert("date_value".to_string(), Some("['2023-01-01']".to_string()));
        values.insert("time_value".to_string(), Some("['10:30:00']".to_string()));

        values.insert("smallint_value".to_string(), Some("[123]".to_string()));
        values.insert("tinyint_value".to_string(), Some("[5]".to_string()));

        db.table_insert(table_name, values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;

            println!("resp:{:?}", resp);
        }
        {
            // export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test]
    async fn test_types4() {
        let _guard = test_lock().lock().await;
        let table_name = "test_types4";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
               CREATE TABLE test_types4 (
                    id UUID PRIMARY KEY,
                    map_field map<int, int>,
                    set_field set<int>,
                    person TypePerson,  -- 确保 TypePerson 已经定义
                    tuple_field tuple<int, int>
                );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();

        values.insert("id".to_string(), Some("uuid()".to_string()));
        values.insert("map_field".to_string(), Some("{1 :2}".to_string()));
        values.insert("set_field".to_string(), Some("{1,2}".to_string()));
        values.insert(
            "person".to_string(),
            Some("{first_name : 'yu' ,last_name : 'yang' , email : 'xxx',age : 100}".to_string()),
        );
        values.insert("tuple_field".to_string(), Some("(1,100)".to_string()));
        db.table_insert(table_name, values).await.unwrap();

        {
            let resp = db
                .show_table_collect(table_name, new_test_show_parameter())
                .await;

            println!("resp:{:?}", resp);
        }
        {
            // export test

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Csv {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);
            // export json

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            // export jsonline
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            // export jsonline
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test]
    async fn test_table_column() {
        let _guard = test_lock().lock().await;

        enable_test_log();
        let db = make_test_object(Some("test".to_string())).await;
        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _resp = db
            .execute_sql(
                query_id,
                r#"
               CREATE TABLE test_column (
                    id1 int,
                    id2 int,
                    value text,
                    PRIMARY KEY (id1, id2)
                );
                        "#,
            )
            .await
            .unwrap()
            .unwrap();

        db.add_table_column(
            table_name,
            "name",
            ColumnType {
                typ: "tinyint".to_string(),
                nullness: None,
                after: None,
                default_value: Some("1".to_string()),
            },
        )
        .await
        .unwrap();

        db.modify_table_column(
            table_name,
            "id2",
            "name2",
            Some(ColumnType {
                typ: "smallint".to_string(),
                nullness: None,
                after: None,
                default_value: Some("2".to_string()),
            }),
        )
        .await
        .unwrap();

        db.drop_table_column(table_name, "value").await.unwrap();
    }
    #[tokio::test]
    async fn test_counter() {
        let _guard = test_lock().lock().await;

        let table_name = "test_counter";
        let db = make_test_object(Some("test".to_string())).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_counter (
                page_id UUID PRIMARY KEY,
                view_count COUNTER
            );
            
                "#,
        )
        .await
        .unwrap()
        .unwrap();

        let mut values = HashMap::new();
        let mut wheres = HashMap::new();
        values.insert("view_count".to_string(), Some("view_count + 1".to_string()));
        wheres.insert("page_id".to_string(), Some("uuid()".to_string()));
        db.table_update(table_name, values, wheres).await.unwrap();

        let resp = db
            .show_table_collect(table_name, new_test_show_parameter())
            .await
            .unwrap();
        println!("xxxx:{:?}", resp);
    }

    #[tokio::test]
    async fn test_udf() {
        let _guard = test_lock().lock().await;

        enable_test_log();
        let db = make_test_object(Some("test".to_string())).await;
        let name = "test_udf";
        {
            let err = db
                .drop_udf(
                    name,
                    Some(UdfOrProcedureExtraParameter::Cassandra {
                        parameters: vec!["text".to_string()],
                    }),
                )
                .await;
            if let Err(err) = err {
                println!("drop udf error:{}", err);
            }
            let err = db
                .drop_udf(
                    name,
                    Some(UdfOrProcedureExtraParameter::Cassandra {
                        parameters: vec!["list<int>".to_string()],
                    }),
                )
                .await;
            if let Err(err) = err {
                println!("drop udf error:{}", err);
            }
        }

        db.session
            .query_unpaged(
                r#"
            CREATE FUNCTION IF NOT EXISTS  test_udf(input text)
            RETURNS NULL ON NULL INPUT
            RETURNS text
            LANGUAGE java
            AS 'return input;';
        "#,
                &[],
            )
            .await
            .unwrap();

        db.session
            .query_unpaged(
                r#"
            CREATE FUNCTION IF NOT EXISTS  test_udf(input list<int>)
            RETURNS NULL ON NULL INPUT
            RETURNS text
            LANGUAGE java
            AS 'return String.valueOf(input);';
        "#,
                &[],
            )
            .await
            .unwrap();
        let list = db.list_udf().await.unwrap();
        list.iter().for_each(|x| println!("udf:{:?}", x));

        let detail = db
            .show_udf(
                name,
                Some(UdfOrProcedureExtraParameter::Cassandra {
                    parameters: vec!["list <int>".to_string()],
                }),
            )
            .await
            .unwrap();
        println!("detail:{:?}", detail);
    }

    #[tokio::test]
    async fn test_create_type() {
        let _guard = test_lock().lock().await;

        let db = make_test_object(Some("test".to_string())).await;
        db.session
            .query_unpaged(
                r#"
            CREATE TYPE IF NOT EXISTS TypePerson (
                first_name text,
                last_name text,
                age int,
                email text
            );
        "#,
                &[],
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_rename() {
        let _guard = test_lock().lock().await;

        let db = make_test_object(Some("test".to_string())).await;
        let table_name = "test_rename";
        let table_name2 = format!("{}2", table_name);
        let _ = db.drop_table(table_name, false).await;
        let _ = db.drop_table(&table_name2, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_rename (
                page_id UUID PRIMARY KEY,
                view_count COUNTER
            );
                "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.rename_table(table_name, &table_name2).await.unwrap();
        db.rename_table(&table_name2, table_name).await.unwrap();
    }

    #[tokio::test]
    async fn test_index() {
        let _guard = test_lock().lock().await;

        let db = make_test_object(Some("test".to_string())).await;
        let table_name = "test_index";
        let _ = db.drop_index(table_name, "test_index_email_index").await;
        let _ = db.drop_table(table_name, false).await;

        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_index (
                    id UUID PRIMARY KEY,
                    name TEXT,
                    age INT,
                    email TEXT
                );
                "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.create_index(
            table_name,
            "test_index_email_index",
            IndexKind::Index,
            &["email".to_string()],
        )
        .await
        .unwrap();

        let list = db.show_indexes(table_name).await.unwrap();
        println!("indexes:{:?}", list);
    }

    #[tokio::test]
    async fn test_view() {
        let _guard = test_lock().lock().await;

        let db = make_test_object(Some("test".to_string())).await;
        let table_name = "test_view_table";
        let view_name = "test_view";
        let _ = db.drop_materialized_view(view_name).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_view_table (
                    id UUID PRIMARY KEY,
                    name TEXT,
                    age INT
                );
                "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.execute_sql(
            query_id,
            r#"
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Alice', 30);
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Bob', 25);
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Charlie', 35);
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Diana', 28);
            INSERT INTO test_view_table (id, name, age) VALUES (uuid(), 'Eve', 22);
            "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE MATERIALIZED VIEW test_view AS
                SELECT * FROM test_view_table
                WHERE age IS NOT NULL AND id IS NOT NULL
                PRIMARY KEY (age, id);
            "#,
        )
        .await
        .unwrap()
        .unwrap();

        let list = db.list_materialized_view().await.unwrap();
        println!("view list:{:?}", list);
        {
            let resp = db
                .show_view_collect(view_name, new_test_show_parameter())
                .await
                .unwrap();
            println!("resp:{:?}", resp);
        }
    }

    #[test]
    fn test_parser_start_bytes() {
        let sql = "select * from test1 ; select * from test2;";
        let ss = sql_to_each(sql);
        println!("{:?}", ss);
        assert!(ss.len() == 2);
    }

    #[test]
    fn test_agg() {
        let sql = r#"CREATE AGGREGATE my_count(text)
                SFUNC count_state_function
                STYPE int
                FINALFUNC count_final_function
                INITCOND 0;"#;
        CassandraAST::new(sql)
            .statements
            .iter()
            .for_each(|x| println!("{:?}", x.statement));
    }
}
