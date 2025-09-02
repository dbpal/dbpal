// docker run -d   --name some-firebird  -e FIREBIRD_DATABASE=test.fdb   -e
// FIREBIRD_ROOT_PASSWORD=root  -e FIREBIRD_USER=alice -e
// FIREBIRD_PASSWORD=root  -e   -e   -p 3050:3050    firebirdsql/firebird

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;

use dashmap::{DashMap, DashSet};
use r2d2::PooledConnection;
use rsfbclient::{Execute, FbError, Queryable, Row, SqlType, builder_pure_rust};
use rsfbclient_rust::RustFbClient;
use tokio::sync::Mutex;
use tokio::task::block_in_place;

use crate::MemoryLimit;
use crate::datasource_config::FireBirdConfig;
use crate::db::{
    BackendKind, CheckConn, ExportKind, ExportQueryResult, Field, GetBackendKind, Index, IndexKind,
    ListTable, NamedStaff, POOL_MAX_CONNECTIONS, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait,
    ServerMetas, ShowExtraArgs, SqlBuilder, SqlExecuteRet, escape_sql_str, not_support_feature,
};
use crate::helper::{create_csv, is_dangerous_sql_by_parser};
use crate::meta_value::MetaValue;
pub struct PoolManager {
    config: FireBirdConfig,
}

pub type Connection = rsfbclient::Connection<RustFbClient>;

impl r2d2::ManageConnection for PoolManager {
    type Connection = Connection;
    type Error = FbError;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let conn = builder_pure_rust()
            .host(&self.config.host)
            .port(self.config.port)
            .user(&self.config.username)
            .pass(&self.config.password)
            .db_name(&self.config.db)
            .connect()?;
        Ok(conn)
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        let _: Option<Row> = conn.query_first("SELECT 1 FROM RDB$DATABASE;", ())?;
        Ok(())
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        self.is_valid(conn).is_err()
    }
}

impl ReadOnlyTrait for FireBirdDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

impl GetBackendKind for FireBirdDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::Firebird
    }
}

fn infer_fields(row: &Row) -> Vec<Field> {
    row.cols
        .iter()
        .map(|x| {
            Field::from_name_type(
                x.name.as_str(),
                match &x.value {
                    SqlType::Text(_) => "TEXT",
                    SqlType::Integer(_) => "INTEGER",
                    SqlType::Floating(_) => "FLOAT",
                    SqlType::Timestamp(_) => "TIMESTAMP",
                    SqlType::Binary(_) => "BLOB",
                    SqlType::Boolean(_) => "BOOLEAN",
                    SqlType::Null => "NULL",
                },
            )
        })
        .collect()
}

fn is_date(x: &Option<&String>) -> bool {
    x.map(|x| x.eq_ignore_ascii_case("date")).unwrap_or(false)
}
fn is_time(x: &Option<&String>) -> bool {
    x.map(|x| x.eq_ignore_ascii_case("time")).unwrap_or(false)
}

fn encode_rows(
    rows: &Vec<Row>, try_blob_2_str: bool, types: Option<HashMap<&String, &Field>>,
) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let get_binary_sub_type = |name: &String| {
        types
            .as_ref()
            .and_then(|x| x.get(name).map(|x| x.firebird_binary_sub_type))
            .flatten()
    };
    let get_type = |name: &String| {
        types
            .as_ref()
            .and_then(|x| x.get(name).map(|x| &x.data_type))
            .map(|x| x.to_uppercase())
    };
    let mut ret = Vec::with_capacity(rows.len());
    for row in rows.iter() {
        let mut data = Vec::with_capacity(row.cols.len());
        for v in row.cols.iter() {
            let value = match &v.value {
                SqlType::Text(x) => Some(x.clone()),
                SqlType::Integer(x) => Some(x.to_string()),
                SqlType::Floating(x) => Some(x.to_string()),
                SqlType::Timestamp(naive_date_time) => {
                    let typ = get_type(&v.name);
                    let typ = typ.as_ref();
                    if is_date(&typ) {
                        Some(naive_date_time.date().to_string())
                    } else if is_time(&typ) {
                        Some(naive_date_time.time().to_string())
                    } else {
                        Some(naive_date_time.to_string())
                    }
                }
                SqlType::Binary(items) => {
                    let sub_type = get_binary_sub_type(&v.name);
                    match sub_type {
                        _ if sub_type_is_text(sub_type) => Some(String::from_utf8(items.clone())?),
                        _ => {
                            if try_blob_2_str {
                                match String::from_utf8(items.clone()) {
                                    Ok(x) => Some(x),
                                    Err(_) => Some(hex::encode(items.as_slice())),
                                }
                            } else {
                                Some(hex::encode(items.as_slice()))
                            }
                        }
                    }
                }
                SqlType::Boolean(x) => Some(x.to_string()),
                SqlType::Null => None,
            };
            data.push(value);
        }

        ret.push(data);
    }
    anyhow::Ok(ret)
}

impl FireBirdDB {
    async fn export_query_result_as_csv(
        &self, sql: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let firebird = Clone::clone(self);
        let sql = sql.to_owned();

        tokio::task::spawn_blocking(move || {
            let mut conn = firebird.get_conn()?;
            let stream: Box<dyn Iterator<Item = Result<Row, FbError>>> =
                conn.query_iter(&sql, ())?;
            let mut first = true;
            for row in stream {
                let row = row?;
                if first && include_header {
                    let fs = row.cols.iter().map(|x| x.name.clone()).map(Some).collect();
                    sender.blocking_send(Cow::Owned(create_csv(&vec![fs], delimiter)))?;

                    first = false;
                }
                let data = encode_rows(&vec![row], true, None)?;
                sender.blocking_send(Cow::Owned(create_csv(&data, delimiter)))?;
            }
            anyhow::Ok(())
        })
        .await??;

        anyhow::Ok(())
    }

    async fn export_query_result_as_json(
        &self, sql: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let firebird = Clone::clone(self);
        let sql = sql.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut conn = firebird.get_conn()?;

            sender.blocking_send("[".into())?;
            let rows: Box<dyn Iterator<Item = Result<Row, FbError>>> = conn.query_iter(&sql, ())?;
            let mut first = true;
            for row in rows {
                let row = row?;
                let value = encode_row_as_json(&row)?;
                if !first {
                    sender.blocking_send(",".into())?;
                }
                if pretty {
                    sender.blocking_send("\n".into())?;
                }

                sender.blocking_send(Cow::Owned(if pretty {
                    serde_json::to_string_pretty(&value)?
                } else {
                    serde_json::to_string(&value)?
                }))?;

                first = false;
            }

            if pretty {
                sender.blocking_send("\n".into())?;
            }
            sender.blocking_send("]".into())?;
            anyhow::Ok(())
        })
        .await??;

        anyhow::Ok(())
    }

    async fn export_query_result_as_json_line(
        &self, sql: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let firebird = Clone::clone(self);
        let sql = sql.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut conn = firebird.get_conn()?;
            let stream: Box<dyn Iterator<Item = Result<Row, FbError>>> =
                conn.query_iter(&sql, ())?;
            for row in stream {
                let row = row?;
                let value = encode_row_as_json(&row)?;
                sender.blocking_send(Cow::Owned(serde_json::to_string(&value)?))?;
                sender.blocking_send("\n".into())?;
            }
            anyhow::Ok(())
        })
        .await??;
        anyhow::Ok(())
    }
}

fn encode_row_as_json(row: &Row) -> anyhow::Result<serde_json::Value> {
    let mut m = serde_json::Map::new();
    for col in row.cols.iter() {
        let value = match &col.value {
            SqlType::Text(x) => Some(serde_json::Value::from(x.clone())),
            SqlType::Integer(x) => Some(serde_json::Value::from(*x)),
            SqlType::Floating(x) => Some(serde_json::Value::from(*x)),
            SqlType::Timestamp(naive_date_time) => {
                Some(serde_json::Value::from(naive_date_time.to_string()))
            }
            SqlType::Binary(items) => {
                let items = items
                    .iter()
                    .map(|x| serde_json::Value::from(*x))
                    .collect::<Vec<_>>();
                Some(serde_json::Value::Array(items))
            }
            SqlType::Boolean(x) => Some(serde_json::Value::from(*x)),
            SqlType::Null => None,
        };
        m.insert(col.name.clone(), value.unwrap_or(serde_json::Value::Null));
    }
    anyhow::Ok(serde_json::Value::Object(m))
}
#[async_trait::async_trait]
impl ExportQueryResult for FireBirdDB {
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
impl QuerySql for FireBirdDB {
    async fn get_auto_commit(&self, query_id: uuid::Uuid) -> anyhow::Result<bool> {
        let tx = self.query_connections_tx.contains(&query_id);
        anyhow::Ok(!tx)
    }

    async fn set_auto_commit(&self, query_id: uuid::Uuid, commit: bool) -> anyhow::Result<()> {
        if commit {
            anyhow::bail!("firebird use commit or rollback to end transaction");
        }
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.begin_transaction()?;
        self.query_connections_tx.insert(query_id);
        anyhow::Ok(())
    }

    async fn commit(&self, query_id: uuid::Uuid) -> anyhow::Result<()> {
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.commit()?;
        self.query_connections_tx.remove(&query_id);

        anyhow::Ok(())
    }

    async fn rollback(&self, query_id: uuid::Uuid) -> anyhow::Result<()> {
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        conn.rollback()?;
        self.query_connections_tx.remove(&query_id);

        anyhow::Ok(())
    }

    async fn query_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let mut ret = vec![];
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;
        let conn = conn.deref_mut();
        let mut ml = MemoryLimit::new();
        let rows: Result<Vec<Row>, _> = block_in_place(|| conn.query(sql, ()));
        match rows {
            Ok(rows) => {
                ml.exceed_error()?;
                if !rows.is_empty() {
                    let fields = infer_fields(rows.first().unwrap());
                    let data = encode_rows(&rows, true, None)?;
                    ret.push(Ok(QueryResp {
                        fields,
                        data,
                        sql: Some(sql.to_owned()),
                        total: 0,
                        has_more: false,
                        cache_time: None,
                    }));
                }
            }
            Err(err) => ret.push(Err(err.to_string())),
        }

        anyhow::Ok(ret)
    }
    async fn execute_sql_no_open_conn(
        &self, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let mut conn = self.get_conn()?;
        let resp = block_in_place(|| conn.execute(sql, ()))
            .map(|x| QueryResp::row_affected(x as u64, sql.to_owned()))
            .map_err(|e| e.to_string());
        anyhow::Ok(resp)
    }
    async fn execute_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let conn = match self.query_connections.get(&query_id) {
            Some(x) => x.clone(),
            None => anyhow::bail!("connection closed???"),
        };
        let mut conn = conn.lock().await;

        let conn = conn.deref_mut();

        let resp = block_in_place(|| conn.execute(sql, ()))
            .map(|x| QueryResp::row_affected(x as u64, sql.to_owned()))
            .map_err(|e| e.to_string());

        anyhow::Ok(resp)
    }

    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let id = uuid::Uuid::new_v4();
        let conn = self.get_conn()?;
        self.query_connections
            .insert(id, Arc::new(Mutex::new(conn)));
        anyhow::Ok(id)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        let conn = self.get_conn()?;
        self.query_connections
            .insert(id, Arc::new(Mutex::new(conn)));
        self.query_connections_tx.remove(&id);

        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.query_connections.remove(&id);

        anyhow::Ok(())
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        is_dangerous_sql_by_parser(sql, &sqlparser::dialect::GenericDialect {})
    }
}

#[derive(Clone)]
pub struct FireBirdDB {
    config: FireBirdConfig,
    pool: r2d2::Pool<PoolManager>,
    query_connections: Arc<DashMap<uuid::Uuid, Arc<Mutex<PooledConnection<PoolManager>>>>>,
    query_connections_tx: DashSet<uuid::Uuid>,
}

impl std::fmt::Debug for FireBirdDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "host={} user={}", self.config.host, self.config.username)
    }
}

#[async_trait::async_trait]
impl ServerMetas for FireBirdDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let sql = "SELECT RDB$GET_CONTEXT('SYSTEM', 'ENGINE_VERSION') FROM RDB$DATABASE";
        let mut conn = self.get_conn()?;
        let version: Option<(String,)> = conn.query_first(sql, ())?;
        let mut ret = Vec::new();
        if let Some(version) = version {
            ret.push(MetaValue::new("version", version.0));
        }
        anyhow::Ok(ret)
    }
}

impl FireBirdDB {
    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_csv(&sql, include_header, delimiter, sender)
            .await
    }

    async fn export_table_as_json(
        &self, table_name: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from {}", table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_json(sql.as_str(), pretty, sender)
            .await
    }

    async fn export_table_as_json_line(
        &self, table_name: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let sql = format!("SELECT * from {}", table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_json_line(&sql, sender).await
    }

    async fn export_table_as_sql(
        &self, table_name: &str, include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let table_name = table_name.to_owned();
        let firebird = Clone::clone(self);
        let cols = SchemaTrait::show_table_columns(self, &table_name).await?;
        tokio::task::spawn_blocking(move || {
            let mut types = HashMap::new();

            cols.iter().for_each(|x| {
                types.insert(&x.field, x);
            });

            if include_table_schema {
                anyhow::bail!("include schema not support by firebird");
            }
            let mut cols_map = HashMap::new();
            cols.iter().for_each(|f| {
                cols_map.insert(f.field.as_str(), f);
            });
            let mut conn = firebird.get_conn()?;
            let sql = format!("SELECT * from {}", table_name);
            log::info!("sql:{}", sql);
            let rows: Box<dyn Iterator<Item = Result<Row, FbError>>> = conn.query_iter(&sql, ())?;
            for row in rows {
                let row = row?;
                let data = encode_rows(&vec![row], true, Some(types.clone()))?;
                for d in data.into_iter() {
                    let mut m = HashMap::new();
                    for (d, f) in d.into_iter().zip(cols.iter()) {
                        m.insert(f.field.clone(), d);
                    }
                    let insert_sql = firebird.insert_sql(None, &table_name, &cols, &m)?;
                    sender.blocking_send(Cow::Owned(insert_sql))?;
                    sender.blocking_send(";\n".into())?;
                }
            }
            anyhow::Ok(())
        })
        .await??;

        anyhow::Ok(())
    }
}

#[async_trait::async_trait]
impl SchemaTrait for FireBirdDB {
    fn list_sql_current_values(&self) -> &'static [&'static str] {
        <Self as SqlBuilder>::CURRENT_VALUES
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn duplicate_table(&self, _table: &str, _dup: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let sql = "
        
            SELECT 
                RDB$RELATION_NAME AS TABLE_NAME
            FROM 
                RDB$RELATIONS
            WHERE 
                RDB$VIEW_BLR IS NULL  -- 排除视图
                AND RDB$SYSTEM_FLAG = 0  -- 排除系统表
            ORDER BY 
                RDB$RELATION_NAME
        "
        .to_string();
        let mut conn = self.get_conn()?;
        let rows: Vec<(String,)> = block_in_place(|| conn.query(&sql, ()))?;
        anyhow::Ok(
            rows.into_iter()
                .map(|x| ListTable::from(x.0.trim().to_owned()))
                .collect(),
        )
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        self.show_table_columns(view_name).await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let mut conn = self.get_conn()?;

        let pri_keys = {
            let sql = format!(
                r#"
                SELECT 
                    s.RDB$FIELD_NAME AS COLUMN_NAME
                FROM 
                    RDB$RELATION_CONSTRAINTS rc
                JOIN 
                    RDB$INDEX_SEGMENTS s ON rc.RDB$INDEX_NAME = s.RDB$INDEX_NAME
                WHERE 
                    rc.RDB$CONSTRAINT_TYPE = 'PRIMARY KEY'
                    AND rc.RDB$RELATION_NAME = '{table_name}'
                ORDER BY 
                    s.RDB$FIELD_POSITION;
            "#
            );
            let x: Vec<(String,)> = block_in_place(|| conn.query(&sql, ()))?;
            x.into_iter()
                .map(|x| x.0.trim().to_owned())
                .collect::<HashSet<_>>()
        };

        let sql = format!(
            r#"
            SELECT 
            rf.RDB$FIELD_NAME AS COLUMN_NAME, 
            t.RDB$TYPE_NAME AS DATA_TYPE,
            f.RDB$FIELD_SUB_TYPE AS SUB_TYPE,
            CASE WHEN f.RDB$NULL_FLAG = 1 THEN 'NO' ELSE 'YES' END AS IS_NULLABLE,
            f.RDB$DEFAULT_SOURCE AS DEFAULT_VALUE
        FROM 
            RDB$RELATION_FIELDS rf
        JOIN 
            RDB$FIELDS f ON rf.RDB$FIELD_SOURCE = f.RDB$FIELD_NAME
        JOIN 
            RDB$TYPES t ON f.RDB$FIELD_TYPE = t.RDB$TYPE
        WHERE 
            rf.RDB$RELATION_NAME = '{}'
            AND t.RDB$FIELD_NAME = 'RDB$FIELD_TYPE';
        
        "#,
            table_name
        );

        let rows: Vec<(String, String, Option<i64>, String, Option<String>)> =
            block_in_place(|| conn.query(&sql, ()))?;
        let mut ret = Vec::new();
        for (name, typ, sub_type, nullable, default_value) in rows {
            let name = name.trim().to_owned();
            let data_type = typ.trim().to_owned();
            let is_primary = pri_keys.contains(&name);
            ret.push(Field {
                field: name,
                data_type,
                nullable: nullable == "YES",
                default_value,
                is_primary,
                is_auto_increment: false,
                sortable: true,
                firebird_binary_sub_type: sub_type,
            });
        }

        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        let sql = gen_show_sql(table_name, where_str, order_by, page, page_size);
        log::info!("sql:{}", sql);
        let fs = self.show_table_columns(table_name).await?;
        let mut fields_map = HashMap::new();

        fs.iter().for_each(|x| {
            fields_map.insert(&x.field, x);
        });

        let mut conn = self.get_conn()?;
        let rows: Vec<rsfbclient::Row> = block_in_place(|| conn.query(&sql, ()))?;

        if rows.is_empty() {
            return anyhow::Ok(QueryResp {
                fields: fs,
                total: 0,
                data: vec![],
                sql: Some(sql),
                cache_time: None,
                has_more: false,
            });
        }
        let mut fields = infer_fields(&rows[0]);
        fields.iter_mut().for_each(|x| {
            if let Some(f) = fields_map.get(&x.field) {
                *x = (*f).clone();
            }
        });
        let data = encode_rows(&rows, false, Some(fields_map))?;
        let total = self.query_count(table_name, where_str)?;

        anyhow::Ok(QueryResp {
            fields,
            cache_time: None,
            total,
            data,
            sql: Some(sql),
            has_more: false,
        })
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;

        let sql = self.insert_sql(None, table_name, &fs, &values)?;
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn()?;
        let changes = conn.execute(&sql, ())?;

        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(changes as u64),
        })
    }

    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = (self).update_sql(None, table_name, &fs, &values, &wheres)?;
        let mut conn = self.get_conn()?;
        let changes = block_in_place(|| conn.execute(&sql, ()))?;
        anyhow::Ok(SqlExecuteRet {
            changes: Some(changes as u64),
            sql,
        })
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        // firebird have no truncate statement.
        let sql = format!(r#"DELETE FROM "{}""#, table_name);
        let mut conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, ()))?;
        anyhow::Ok(())
    }

    async fn table_delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = (self).delete_sql(None, table_name, &fs, &wheres)?;
        let mut conn = self.get_conn()?;
        let changes = block_in_place(|| conn.execute(&sql, ()))?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(changes as u64),
        })
    }

    async fn list_view(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let sql = r#"
            SELECT RDB$RELATION_NAME
            FROM RDB$RELATIONS
            WHERE RDB$VIEW_BLR IS NOT NULL;
        "#;
        let mut conn = self.get_conn()?;
        let rows: Vec<(String,)> = block_in_place(|| conn.query(sql, ()))?;
        anyhow::Ok(
            rows.into_iter()
                .map(|x| NamedStaff::from(x.0.trim().to_owned()))
                .collect(),
        )
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP VIEW "{}""#, view_name);
        let mut conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, ()))?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<()> {
        let sql = format!(r#"DROP TABLE "{}""#, table);
        let mut conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, ()))?;
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
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
        let mut conn = self.get_conn()?;
        block_in_place(|| conn.execute(&sql, ()))?;
        anyhow::Ok(())
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let sql = format!(
            r#"
        SELECT 
            i.RDB$INDEX_NAME AS INDEX_NAME, 
            i.RDB$UNIQUE_FLAG AS IS_UNIQUE, 
            s.RDB$FIELD_NAME AS FIELD_NAME
        FROM 
            RDB$INDICES i
        LEFT JOIN 
            RDB$INDEX_SEGMENTS s ON s.RDB$INDEX_NAME = i.RDB$INDEX_NAME
        WHERE 
            i.RDB$SYSTEM_FLAG = 0 
            AND i.RDB$RELATION_NAME = '{}'  -- 替换为你的表名
        ORDER BY 
            i.RDB$INDEX_NAME, s.RDB$FIELD_POSITION 

        "#,
            table_name
        );
        let mut conn = self.get_conn()?;
        let rows: Vec<(String, i64, String)> = block_in_place(|| conn.query(&sql, ()))?;
        let mut m: HashMap<String, Index> = HashMap::new();
        for (name, unique, col) in rows.into_iter() {
            let name = name.trim().to_owned();
            let col = col.trim().to_owned();
            let x = m.get_mut(&name);
            match x {
                Some(x) => {
                    x.columns.push(col);
                }
                None => {
                    m.insert(
                        name.clone(),
                        Index {
                            name: Some(name),
                            kind: if unique == 1 {
                                IndexKind::Unique
                            } else {
                                IndexKind::Index
                            },
                            columns: vec![col],
                        },
                    );
                }
            };
        }
        anyhow::Ok(m.values().cloned().collect())
    }

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP INDEX "{}";"#, index);
        let mut conn = self.get_conn()?;
        conn.execute(&sql, ())?;
        anyhow::Ok(())
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        let sql = gen_show_sql(view_name, where_str, order_by, page, page_size);
        log::info!("sql:{}", sql);
        let mut conn = self.get_conn()?;
        let rows: Vec<rsfbclient::Row> = block_in_place(|| conn.query(&sql, ()))?;

        if rows.is_empty() {
            let fields = self.show_view_columns(view_name).await?;

            return anyhow::Ok(QueryResp {
                fields,
                total: 0,
                data: vec![],
                sql: Some(sql),
                cache_time: None,
                has_more: false,
            });
        }
        let data = encode_rows(&rows, true, None)?;
        let total = self.query_count(view_name, where_str)?;
        let fields = match rows.first() {
            Some(x) => infer_fields(x),
            None => {
                vec![]
            }
        };
        anyhow::Ok(QueryResp {
            fields,
            total,
            data,
            sql: Some(sql),
            cache_time: None,
            has_more: false,
        })
    }
}

/// Firebird 里可直接写在 SQL 语句中的 CURRENT_* 字面量
pub const FB_CURRENT_LITERALS: &[&str] = &[
    "CURRENT_DATE",      //  2025-08-25
    "CURRENT_TIME",      //  14:36:07.1230
    "CURRENT_TIMESTAMP", //  2025-08-25 14:36:07.1230
];

impl SqlBuilder for FireBirdDB {
    const CURRENT_VALUES: &'static [&'static str] = FB_CURRENT_LITERALS;
    fn put_value_in_sql(
        &self, typ: &str, firebird_blob_sub_type: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        if self.hit_current_value(value) {
            return anyhow::Result::Ok(value.unwrap().to_string());
        }
        put_value_in_sql(typ, firebird_blob_sub_type, value)
    }
}

impl FireBirdDB {
    fn query_count(&self, name: &str, where_str: Option<&str>) -> anyhow::Result<usize> {
        let mut conn = self.get_conn()?;
        let mut s = format!(r#"SELECT count(1) from "{name}" "#);
        if let Some(where_str) = where_str {
            s.push_str(format!(" where {where_str}").as_str());
        }
        let row: Option<(i64,)> = conn.query_first(&s, ())?;
        anyhow::Ok(row.map(|x| x.0 as usize).unwrap_or_default())
    }
}

fn sub_type_is_text(sub_type: Option<i64>) -> bool {
    sub_type.map(|x| x == 1).unwrap_or_default()
}

fn put_value_in_sql(
    typ: &str, sub_type: Option<i64>, value: Option<&str>,
) -> anyhow::Result<String> {
    let typ = typ.to_lowercase();
    log::trace!("encode value '{:?}' on type '{}'", value, typ);
    match typ.as_str() {
        "text" | "varying" => anyhow::Ok(
            value
                .map(|x| format!("'{}'", escape_sql_str(x)))
                .unwrap_or("null".to_string()),
        ),
        "date" | "time" | "timestamp" => anyhow::Ok(
            value
                .map(|x| format!("'{}'", escape_sql_str(x)))
                .unwrap_or("null".to_string()),
        ),
        // binary is blob
        "blob" if sub_type_is_text(sub_type) => anyhow::Ok(
            value
                .map(|x| format!("'{}'", x))
                .unwrap_or("null".to_string()),
        ),
        "blob" => match sub_type {
            Some(sub_type) => anyhow::Ok(
                value
                    .map(|x| format!("CAST(HEX_DECODE('{}') AS BLOB SUB_TYPE {})", x, sub_type))
                    .unwrap_or("null".to_string()),
            ),
            None => anyhow::bail!("sub type shoule some."),
        },
        _ => anyhow::Ok(value.map(|x| x.to_string()).unwrap_or("null".to_string())),
    }
}

#[async_trait::async_trait]
impl CheckConn for FireBirdDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let mut conn = self.get_conn()?;
        let now = std::time::Instant::now();
        let _: Option<Row> =
            block_in_place(|| conn.query_first("SELECT 1 FROM RDB$DATABASE;", ()))?;

        anyhow::Ok(now.elapsed())
    }
}

impl FireBirdDB {
    pub fn new(config: FireBirdConfig) -> anyhow::Result<Self> {
        let mut builder = r2d2::Pool::builder();
        builder = builder.max_size(POOL_MAX_CONNECTIONS);
        if let Some(x) = config.connection_timeout {
            builder = builder.connection_timeout(Duration::from_millis(x as u64));
        }
        let pool = builder.build(PoolManager {
            config: config.clone(),
        })?;
        anyhow::Ok(Self {
            config,
            pool,
            query_connections: Default::default(),
            query_connections_tx: Default::default(),
        })
    }

    pub(crate) fn get_conn(&self) -> anyhow::Result<r2d2::PooledConnection<PoolManager>> {
        let conn = block_in_place(|| self.pool.get())?;
        anyhow::Ok(conn)
    }
}

pub(crate) fn gen_show_sql(
    table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize,
) -> String {
    let mut sql = format!(r#"SELECT * FROM "{table_name}""#);
    if let Some(where_str) = where_str {
        sql.push_str(format!(" WHERE {where_str} ").as_str());
    }
    if let Some(order_by) = order_by {
        sql.push_str(format!(" ORDER BY {order_by} ").as_str());
    }
    let offset = (page - 1) * page_size;
    sql.push_str(format!(" ROWS {} TO {} ", offset + 1, offset + page_size + 1).as_str());
    sql
}

#[cfg(test)]
pub(crate) fn make_test_object() -> FireBirdDB {
    FireBirdDB::new(FireBirdConfig {
        host: "localhost".to_string(),
        port: 3050,
        username: "SYSDBA".to_string(),
        password: "root".to_string(),
        db: "/var/lib/firebird/data/test.fdb".to_string(),
        read_only: false,
        connection_timeout: None,
        save_password: true,
    })
    .unwrap()
}

#[cfg(test)]

mod tests {
    use super::*;
    use crate::db::IndexKind;
    use crate::enable_test_log;
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_check_conn() {
        let db = make_test_object();
        let d = crate::db::CheckConn::check_conn(&db).await.unwrap();
        println!("ping time:{:?}", d);
    }

    fn test_lock() -> &'static Mutex<()> {
        static MUTEX: Mutex<()> = Mutex::const_new(());
        &MUTEX
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_transaction() {
        let _gurad = test_lock().lock().await;
        let db = make_test_object();

        let table_name = "TEST_COMMIT";
        let _ = db.drop_table(table_name, false).await;

        let query_id = db.open_connection().await.unwrap();
        db.execute_sql(
            query_id,
            r#"
             CREATE TABLE TEST_COMMIT (
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
            INSERT INTO TEST_COMMIT (ID, NAME) VALUES (1, 'Test');
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.rollback(query_id).await.unwrap();

        assert!(
            db.show_table(table_name, None, None, 1, 100, None)
                .await
                .unwrap()
                .total
                == 0
        );

        db.execute_sql(
            query_id,
            r#"
            INSERT INTO TEST_COMMIT (ID, NAME) VALUES (1, 'Test');
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.commit(query_id).await.unwrap();
        assert!(
            db.show_table(table_name, None, None, 1, 100, None)
                .await
                .unwrap()
                .total
                == 1
        );
        db.re_connect(query_id).await.unwrap();

        assert!(db.get_auto_commit(query_id).await.unwrap());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_view() {
        let _gurad = test_lock().lock().await;

        enable_test_log();

        let db = make_test_object();
        let table_name = "TEST_VIEW_TABLE";
        let view_name = "TEST_VIEW";
        let _ = db.drop_view(view_name).await;

        let _ = db.drop_table(table_name, false).await;

        let query_id = db.open_connection().await.unwrap();

        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE TEST_VIEW_TABLE (
                id INTEGER PRIMARY KEY,
                name VARCHAR(50),
                age INTEGER,
                salary DECIMAL(10, 2)
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
            INSERT INTO TEST_VIEW_TABLE (id, name, age, salary) VALUES (1, 'Alice', 30, 50000.00);
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.execute_sql(
            query_id,
            r#"
        CREATE VIEW TEST_VIEW AS
        SELECT id, name, age, salary
        FROM TEST_VIEW_TABLE;
    "#,
        )
        .await
        .unwrap()
        .unwrap();

        let resp = db
            .show_view(view_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("{:?}", resp);

        let columns = db.show_view_columns(view_name).await.unwrap();

        for col in columns.iter() {
            println!("col:{:?}", col);
        }

        let views = db.list_view().await.unwrap();
        for view in views.iter() {
            println!("view:{:?}", view);
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_index() {
        let _gurad = test_lock().lock().await;
        enable_test_log();
        let table_name = "TEST_INDEX";
        let index_name = "TEST_INDEX_NAME_INDEX";
        let db = make_test_object();
        let _err = db.drop_index(table_name, index_name).await;
        if let Err(err) = _err {
            println!("drop index err:{}", err.to_string());
        }
        let _err = db.drop_table(table_name, false).await;
        if let Err(err) = _err {
            println!("drop table err:{}", err.to_string());
        }
        let query_id = db.open_connection().await.unwrap();

        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE TEST_INDEX (
                ID INTEGER NOT NULL PRIMARY KEY,
                NAME VARCHAR(100),
                CREATED_DATE TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                STATUS CHAR(1) CHECK (STATUS IN ('A', 'I'))  -- 'A' for Active, 'I' for Inactive
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.create_index(
            table_name,
            index_name,
            IndexKind::Unique,
            &["NAME".to_string(), "STATUS".to_string()],
        )
        .await
        .unwrap();
        let indexes = db.show_indexes(table_name).await.unwrap();

        for index in indexes.iter() {
            println!("index:{:?}", index);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_list_table() {
        let _gurad = test_lock().lock().await;
        let db = make_test_object();
        let tables = db.list_table().await.unwrap();
        for table in tables {
            println!("table:{:?}", table);
        }
    }
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_truncate() {
        let _gurad = test_lock().lock().await;
        let table_name = "TEST_TRUNCATE";
        let db = make_test_object();
        let _err = db.drop_table(table_name, false).await;
        if let Err(err) = _err {
            println!("drop table err:{}", err.to_string());
        }

        let query_id = db.open_connection().await.unwrap();

        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_truncate (
                id INTEGER PRIMARY KEY,
                name VARCHAR(50),
                age INTEGER,
                salary DECIMAL(10, 2)
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
            INSERT INTO test_truncate (id, name, age, salary) VALUES (1, 'Alice', 30, 50000.00)
            
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.truncate_table(table_name).await.unwrap();
        assert!(
            db.show_table(table_name, None, None, 1, 100, None)
                .await
                .unwrap()
                .total
                == 0
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_table() {
        let _gurad = test_lock().lock().await;
        // enable_test_log();
        let table_name = "TEST_TABLE";
        let db = make_test_object();
        let _err = db.drop_table(table_name, false).await;
        if let Err(err) = _err {
            println!("drop table err:{}", err.to_string());
        }
        let query_id = db.open_connection().await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE TEST_TABLE (
                ID  INTEGER primary key,
                COL_INTEGER INTEGER,
                col_smallint SMALLINT,
                col_bigint BIGINT,
                col_float FLOAT,
                col_double_precision DOUBLE PRECISION,
                col_decimal DECIMAL(10, 2),
                col_numeric NUMERIC(10, 2)
            )
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let mut values = HashMap::new();
        values.insert("ID".to_string(), Some("1".to_string()));
        values.insert("COL_INTEGER".to_string(), Some("123".to_string()));
        values.insert("COL_SMALLINT".to_string(), Some("45".to_string()));
        values.insert("COL_BIGINT".to_string(), Some("9876543210".to_string()));
        values.insert("COL_FLOAT".to_string(), Some("3.14".to_string()));
        values.insert(
            "COL_DOUBLE_PRECISION".to_string(),
            Some("2.718281828459045".to_string()),
        );

        values.insert("COL_DECIMAL".to_string(), Some("123.45".to_string()));
        values.insert("COL_NUMERIC".to_string(), Some("678.90".to_string()));
        db.table_insert(table_name, values).await.unwrap();

        {
            let resp = db
                .show_table(table_name, Some("1 =1 "), Some("ID asc "), 1, 100, None)
                .await
                .unwrap();
            println!("resp:{:?}", resp);
        }
        {
            // export test
            let db = db;
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: false,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_table2() {
        let _gurad = test_lock().lock().await;

        enable_test_log();
        let table_name = "TEST_TABLE2";
        let db = make_test_object();
        let _err = db.drop_table(table_name, false).await;
        if let Err(err) = _err {
            println!("drop table err:{}", err.to_string());
        }
        let query_id = db.open_connection().await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE TEST_TABLE2 (
                ID INTEGER PRIMARY KEY,
                COL_CHAR CHAR(10),
                COL_VARCHAR VARCHAR(50),
                COL_BLOB_TEXT BLOB SUB_TYPE TEXT CHARACTER SET UTF8,
                COL_BLOB_BINARY BLOB SUB_TYPE BINARY,
                COL_DATE DATE,
                COL_TIME TIME,
                COL_TIMESTAMP TIMESTAMP,
                COL_BOOLEAN BOOLEAN
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let mut values = HashMap::new();
        values.insert("ID".to_string(), Some("1".to_string()));
        values.insert("COL_CHAR".to_string(), Some("FixedText".to_string()));
        values.insert(
            "COL_VARCHAR".to_string(),
            Some("Variable Length Text".to_string()),
        );
        values.insert(
            "COL_BLOB_TEXT".to_string(),
            Some("This is a text BLOB".to_string()),
        );
        values.insert(
            "COL_BLOB_BINARY".to_string(),
            Some("48656C6C6F".to_string()),
        );
        values.insert("COL_DATE".to_string(), Some("2025-03-17".to_string()));
        values.insert("COL_TIME".to_string(), Some("14:30:00".to_string()));
        values.insert(
            "COL_TIMESTAMP".to_string(),
            Some("2025-03-17 14:30:00".to_string()),
        );
        values.insert("COL_BOOLEAN".to_string(), Some("TRUE".to_string()));
        db.table_insert(table_name, values).await.unwrap();

        {
            let resp = db
                .show_table(table_name, Some("1 = 1"), Some("ID asc "), 1, 100, None)
                .await
                .unwrap();
            println!("resp:{:?}", resp);
        }
        {
            // export test
            let db = db;
            let ret = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("jsonline:{}", ret);

            let ret = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", ret);
            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", ret);

            let ret = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: false,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", ret);
        }
    }
}
