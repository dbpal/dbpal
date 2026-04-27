// docker run -d   --name some-firebird  -e FIREBIRD_DATABASE=test.fdb   -e
// FIREBIRD_ROOT_PASSWORD=root  -e FIREBIRD_USER=alice -e
// FIREBIRD_PASSWORD=root  -e   -e   -p 3050:3050    firebirdsql/firebird
/*
    firebird 为啥 SELECT
                RDB$RELATION_NAME AS TABLE_NAME
            FROM
                RDB$RELATIONS
            WHERE
                RDB$VIEW_BLR IS NULL  -- 排除视图
                AND RDB$SYSTEM_FLAG = 0  -- 排除系统表
            ORDER BY
                RDB$RELATION_NAME 这个语句查出来的名字有空格
复制
原因一句话
RDB$RELATION_NAME 的数据类型是 CHAR(31)，定长字符，不足 31 字节时 Firebird 自动用 空格（0x20） 补齐，所以 TRIM 一下即可。

所以代码里面有很多的trim
*/
use crate::datasource_config::FireBirdConfig;
use crate::db::{
    create_html_table_data, create_html_table_header, create_markdown_data, create_markdown_header,
    escape_sql_str, export_query_as_xlsx_send_blocking_data, BackendKind, CheckConn, ColumnType,
    CreateIndexReq, ExportFrame, ExportKind, ExportQueryResult, Field, FirstTrue, GetBackendKind,
    GetTestExportTableKinds, Index, IndexKind, ListTable, MetaValue, QueryEvent, QueryEventEmitter,
    QueryResp, QuerySql, SchemaTrait, ServerMetas, ShowTableParameter, SqlBuilder, XlsxEdit,
    POOL_MAX_CONNECTIONS,
};

use crate::helper::create_csv;
use crate::language_server::LexerCompletionTrait;
use dashmap::{DashMap, DashSet};
use r2d2::{ManageConnection, Pool, PooledConnection};
use rsfbclient::prelude::{TrDataAccessMode, TransactionConfiguration};
use rsfbclient::{builder_pure_rust, Execute, FbError, Queryable, Row, SqlType};
use rsfbclient_rust::RustFbClient;

use std::collections::{HashMap, HashSet};
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task::spawn_blocking;

pub struct PoolManager {
    config: Arc<FireBirdConfig>,
}

pub type Connection = rsfbclient::Connection<RustFbClient>;

impl r2d2::ManageConnection for PoolManager {
    type Connection = Connection;
    type Error = FbError;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let mut tr = TransactionConfiguration::default();

        if self.config.read_only {
            tr.data_access = TrDataAccessMode::ReadOnly;
        }
        let conn = builder_pure_rust()
            .host(&self.config.host)
            .port(self.config.port)
            .user(&self.config.username)
            .pass(&self.config.password)
            .db_name(&self.config.db)
            .transaction(tr)
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

impl GetBackendKind for FireBirdDB {
    fn backend_kind(&self) -> BackendKind {
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
    x.map(|x| x.eq_ignore_ascii_case("date"))
        .unwrap_or_default()
}
fn is_time(x: &Option<&String>) -> bool {
    x.map(|x| x.eq_ignore_ascii_case("time"))
        .unwrap_or_default()
}

fn encode_row(
    row: &Row,
    types: Option<HashMap<&String, &Field>>,
) -> anyhow::Result<Vec<Option<String>>> {
    let get_type = |name: &String| {
        types
            .as_ref()
            .and_then(|x| x.get(name).map(|x| &x.data_type))
            .map(|x| x.to_uppercase())
    };
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
            SqlType::Binary(items) => Some(format!("x'{}'", hex::encode(items.as_slice()))),
            SqlType::Boolean(x) => Some(x.to_string()),
            SqlType::Null => None,
        };
        data.push(value);
    }
    anyhow::Ok(data)
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
        &self,
        _query_id: Option<&str>,
        sql: &str,
        kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let sql = sql.to_string();
        spawn_blocking(move || {
            let mut conn = this.get_conn()?;
            let stream: Box<dyn Iterator<Item = Result<Row, FbError>>> =
                conn.query_iter(&sql, ())?;
            let first = FirstTrue::default();

            let mut row_num = 0;
            for row in stream {
                let row = row?;
                match kind {
                    ExportKind::Csv {
                        include_header,
                        delimiter,
                    } => {
                        if first.get() && include_header {
                            let fs: Vec<_> =
                                row.cols.iter().map(|x| x.name.clone()).map(Some).collect();
                            sender.blocking_send(create_csv(&fs, delimiter)?.into())?;
                        }
                        let data = encode_row(&row, None)?;
                        sender.blocking_send(create_csv(&data, delimiter)?.into())?;
                    }
                    ExportKind::Json { .. } | ExportKind::JsonLine => {
                        let value = encode_row_as_json(&row)?;
                        sender.blocking_send(value.into())?;
                    }
                    ExportKind::Sql { .. } => anyhow::bail!("can't export query result as sql"),
                    ExportKind::Xlsx { include_header } => {
                        if first.get() && include_header {
                            for (index, col) in row.cols.iter().enumerate() {
                                sender.blocking_send(
                                    XlsxEdit {
                                        row: row_num,
                                        column: index as rust_xlsxwriter::ColNum,
                                        content: col.name.clone(),
                                    }
                                    .into(),
                                )?;
                            }
                        }
                        let data = encode_row(&row, None)?;
                        export_query_as_xlsx_send_blocking_data(
                            &mut row_num,
                            data,
                            sender.clone(),
                        )?;
                    }
                    ExportKind::Html { include_header } => {
                        if first.get() && include_header {
                            let fs: Vec<_> = row.cols.iter().map(|x| x.name.clone()).collect();
                            sender.blocking_send(create_html_table_header(&fs).into())?;
                        }
                        let data = encode_row(&row, None)?;
                        sender.blocking_send(create_html_table_data(&[data]).into())?;
                    }
                    ExportKind::MarkDown => {
                        if first.get() {
                            let fs: Vec<_> = row.cols.iter().map(|x| x.name.clone()).collect();
                            sender.blocking_send(create_markdown_header(&fs).into())?;
                        }
                        let data = encode_row(&row, None)?;
                        sender.blocking_send(create_markdown_data(&data).into())?;
                    }
                }
            }
            anyhow::Ok(())
        })
        .await?
    }
}

#[async_trait::async_trait]
impl QuerySql for FireBirdDB {
    async fn get_auto_commit(&self, query_id: &str) -> anyhow::Result<bool> {
        let tx = self.query_connection_tx.contains(&query_id.to_string());
        anyhow::Ok(!tx)
    }

    async fn set_auto_commit(&self, query_id: &str, commit: bool) -> anyhow::Result<()> {
        if commit {
            anyhow::bail!("firebird use commit or rollback to end transaction");
        }
        let this = Clone::clone(self);
        let query_id = query_id.to_string();
        spawn_blocking(move || {
            let conn = match this.query_connection.get(&query_id.to_string()) {
                Some(x) => x.clone(),
                None => anyhow::bail!("connection closed???"),
            };
            let mut conn = conn.lock().expect("lock failed");
            let conn = conn.deref_mut();
            conn.begin_transaction()?;
            this.query_connection_tx.insert(query_id.to_string());
            anyhow::Ok(())
        })
        .await?
    }

    async fn commit(&self, query_id: &str) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let query_id = query_id.to_string();
        spawn_blocking(move || {
            let conn = match this.query_connection.get(&query_id.to_string()) {
                Some(x) => x.clone(),
                None => anyhow::bail!("connection closed???"),
            };
            let mut conn = conn.lock().expect("lock failed");
            let conn = conn.deref_mut();
            conn.commit()?;
            this.query_connection_tx.remove(&query_id.to_string());
            anyhow::Ok(())
        })
        .await?
    }

    async fn rollback(&self, query_id: &str) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let query_id = query_id.to_string();
        spawn_blocking(move || {
            let conn = match this.query_connection.get(&query_id) {
                Some(x) => x.clone(),
                None => anyhow::bail!("connection closed???"),
            };
            let mut conn = conn.lock().expect("lock failed");
            let conn = conn.deref_mut();
            conn.rollback()?;
            this.query_connection_tx.remove(&query_id.to_string());
            anyhow::Ok(())
        })
        .await?
    }

    async fn query_sql(
        &self,
        query_id: &str,
        sql: &str,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        crate::language_server::LangServerCommon::contains_tx_operation_error(self, sql)?;
        let this = Clone::clone(self);
        let query_id = query_id.to_string();
        let sql = sql.to_owned();
        spawn_blocking(move || {
            emit.send(QueryEvent::new_status("trying to lock connection"))?;
            let conn = match this.query_connection.get(&query_id) {
                Some(x) => x.clone(),
                None => anyhow::bail!("connection closed???"),
            };
            let mut conn = conn.lock().expect("lock failed");
            let conn = conn.deref_mut();
            emit.send(QueryEvent::new_status("lock connection ok."))?;
            let stream: Box<dyn Iterator<Item = Result<Row, FbError>>> =
                conn.query_iter(&sql, ())?;
            let first = FirstTrue::default();

            for row in stream {
                let item = row?;
                if first.get() {
                    emit.send(
                        QueryResp {
                            fields: infer_fields(&item),
                            ..Default::default()
                        }
                        .into(),
                    )?;
                }
                let d = encode_row(&item, None)?;
                emit.send(d.into())?;
            }
            anyhow::Ok(())
        })
        .await?
    }
    async fn simple_execute_sql(&self, sql: &str) -> anyhow::Result<Option<usize>> {
        let sql = sql.to_owned();
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let mut conn = this.get_conn()?;
            let r = conn
                .execute(&sql, ())
                .map_err(|err| anyhow::Error::msg(format!("Sql:{} Err:{}", sql, err)))?;
            anyhow::Ok(Some(r))
        })
        .await?
    }
    async fn execute_sql(
        &self,
        query_id: &str,
        sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        crate::language_server::LangServerCommon::contains_tx_operation_error(self, sql)?;
        let this = Clone::clone(self);
        let query_id = query_id.to_string();
        let sql = sql.to_string();
        spawn_blocking(move || {
            let conn = match this.query_connection.get(&query_id.to_string()) {
                Some(x) => x.clone(),
                None => anyhow::bail!("connection closed???"),
            };
            let mut conn = conn.lock().expect("lock failed");
            let conn = conn.deref_mut();
            let resp = conn
                .execute(&sql, ())
                .map(|x| QueryResp::row_affected(x as u64, sql.to_owned()))
                .map_err(|e| e.to_string());
            anyhow::Ok(resp)
        })
        .await?
    }

    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let id = id.to_string();
        spawn_blocking(move || {
            let conn = PoolManager {
                config: this.config.clone(),
            };
            let conn = conn.connect()?;
            this.query_connection.insert(id, Arc::new(Mutex::new(conn)));
            anyhow::Ok(())
        })
        .await?
    }
    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        self.query_connection.remove(&id.to_string());
        anyhow::Ok(())
    }
}

#[derive(Clone)]
pub struct FireBirdDB {
    config: Arc<FireBirdConfig>,
    pool: Pool<PoolManager>,
    query_connection: Arc<DashMap<String, Arc<Mutex<Connection>>>>,
    query_connection_tx: Arc<DashSet<String>>,
}

impl std::fmt::Debug for FireBirdDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "firebird host={} user={}",
            self.config.host, self.config.username
        )
    }
}

#[async_trait::async_trait]
impl ServerMetas for FireBirdDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let sql = "SELECT RDB$GET_CONTEXT('SYSTEM', 'ENGINE_VERSION') FROM RDB$DATABASE";
            let mut conn = this.get_conn()?;
            let version: Option<(String,)> = conn.query_first(sql, ())?;
            let mut ret = Vec::new();
            if let Some(version) = version {
                ret.push(MetaValue::new("version", version.0));
            }
            anyhow::Ok(ret)
        })
        .await?
    }
}

impl FireBirdDB {
    async fn export_table_as_sql(
        &self,
        table_name: &str,
        para: Option<ShowTableParameter>,
        include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            anyhow::bail!("include schema not support by firebird");
        }
        let table_name = table_name.to_owned();
        let this = Clone::clone(self);
        let cols = self.show_table_columns(&table_name).await?;
        spawn_blocking(move || {
            let mut types = HashMap::new();
            cols.iter().for_each(|x| {
                types.insert(&x.field, x);
            });
            let mut conn = this.get_conn()?;
            let sql = this.gen_show_sql(&table_name, Some(&cols), para.as_ref());
            log::info!("sql:{}", sql);
            let rows: Box<dyn Iterator<Item = Result<Row, FbError>>> = conn.query_iter(&sql, ())?;
            for row in rows {
                let row = row?;
                let data = encode_row(&row, Some(types.clone()))?;
                let mut m = HashMap::new();
                for (d, f) in data.into_iter().zip(cols.iter()) {
                    m.insert(f.field.clone(), d);
                }
                let insert_sql = this.insert_sql(&table_name, &cols, &m)?;
                sender.blocking_send(insert_sql.into())?;
                sender.blocking_send(";\n".into())?;
            }
            anyhow::Ok(())
        })
        .await??;
        anyhow::Ok(())
    }
}

impl FireBirdDB {
    fn show_table_columns_impl(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
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
            let x: Vec<(String,)> = conn.query(&sql, ())?;
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
            CASE WHEN COALESCE(rf.RDB$NULL_FLAG, f.RDB$NULL_FLAG) = 1 THEN 'NO' ELSE 'YES' END AS IS_NULLABLE,
            COALESCE(rf.RDB$DEFAULT_SOURCE, f.RDB$DEFAULT_SOURCE) AS DEFAULT_VALUE
        FROM RDB$RELATION_FIELDS rf
        JOIN RDB$FIELDS f ON rf.RDB$FIELD_SOURCE = f.RDB$FIELD_NAME
        JOIN RDB$TYPES t ON f.RDB$FIELD_TYPE = t.RDB$TYPE
        WHERE rf.RDB$RELATION_NAME = '{}'
        AND t.RDB$FIELD_NAME = 'RDB$FIELD_TYPE';
    
        "#,
            table_name
        );
        let rows: Vec<(String, String, Option<i64>, String, Option<String>)> =
            conn.query(&sql, ())?;
        let mut ret = Vec::new();
        for (name, typ, _sub_type, nullable, default_value) in rows {
            let name = name.trim().to_owned();
            let data_type = typ.trim().to_owned();
            let is_primary = pri_keys.contains(&name);
            ret.push(Field {
                field: name,
                data_type,
                nullable: nullable == "YES",
                default_value,
                is_primary,
                ..Default::default()
            });
        }
        anyhow::Ok(ret)
    }
}

#[async_trait::async_trait]
impl SchemaTrait for FireBirdDB {
    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        LexerCompletionTrait::get_types(self).await
    }

    fn list_sql_current_values(&self) -> &'static [&'static str] {
        <Self as SqlBuilder>::get_current_values(self)
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

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn duplicate_table(&self, _table: &str, _dup: &str) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature()
    }
    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
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
        ";
            let mut conn = this.get_conn()?;
            let rows: Vec<(String,)> = conn.query(sql, ())?;
            anyhow::Ok(
                rows.into_iter()
                    .map(|x| ListTable::from(x.0.trim().to_owned()))
                    .collect(),
            )
        })
        .await?
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        self.show_table_columns(view_name).await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let this = Clone::clone(self);
        let table_name = table_name.to_owned();
        spawn_blocking(move || this.show_table_columns_impl(&table_name)).await?
    }

    async fn show_table(
        &self,
        table_name: &str,
        para: ShowTableParameter,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let fs = self.show_table_columns(table_name).await?;
        let table_name = table_name.to_owned();
        let this = Clone::clone(self);
        spawn_blocking(move || {
            emit.send("trying to get connection".into())?;
            let mut conn = this.get_conn()?;
            let total = Self::query_count(&mut conn, &table_name, para.where_str.as_deref())?;
            emit.send(
                QueryResp {
                    fields: fs.clone(),
                    total,
                    ..Default::default()
                }
                .into(),
            )?;
            let sql = this.gen_show_sql(&table_name, Some(&fs), Some(&para));
            let mut fs_map = HashMap::new();
            for f in fs.iter() {
                fs_map.insert(&f.field, f);
            }
            log::info!("sql:{}", sql);
            emit.send("trying to query data".into())?;
            let stream: Box<dyn Iterator<Item = Result<Row, FbError>>> =
                conn.query_iter(&sql, ())?;
            for row in stream {
                let row = row?;
                emit.send(encode_row(&row, Some(fs_map.clone()))?.into())?;
            }
            anyhow::Ok(())
        })
        .await?
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<Option<String>> {
        // firebird have no truncate statement.
        let sql = format!(r#"DELETE FROM "{}""#, table_name);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.clone().into())
    }

    // not tested...
    async fn modify_table_column(
        &self,
        _table: &str,
        _old_name: &str,
        _new_name: &str,
        _typ: Option<ColumnType>,
    ) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }

    // not tested...
    async fn add_table_column(
        &self,
        table: &str,
        name: &str,
        col: ColumnType,
    ) -> anyhow::Result<String> {
        let sql = format!(
            r#"ALTER TABLE "{table}" ADD "{name}" {} {} {}"#,
            col.typ,
            if let Some(dv) = col.default_value.as_ref() {
                format!(" DEFAULT {}", self.put_value_in_sql(&col.typ, dv)?)
            } else {
                "".to_string()
            },
            col.nullable.map(|x| x.to_string()).unwrap_or_default(),
        );
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    // not tested...
    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<String> {
        let sql = format!(r#"ALTER TABLE "{table}" DROP "{col}""#);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let sql = r#"
            SELECT RDB$RELATION_NAME
            FROM RDB$RELATIONS
            WHERE RDB$VIEW_BLR IS NOT NULL;
        "#;
            let mut conn = this.get_conn()?;
            let rows: Vec<(String,)> = conn.query(sql, ())?;
            anyhow::Ok(
                rows.into_iter()
                    .map(|x| ListTable::from(x.0.trim().to_owned()))
                    .collect(),
            )
        })
        .await?
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<String> {
        let sql = format!(r#"DROP VIEW "{}""#, view_name);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<Option<String>> {
        let sql = format!(r#"DROP TABLE "{}""#, table);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    async fn create_index(&self, table: &str, req: &CreateIndexReq) -> anyhow::Result<String> {
        let sql = req.common_sql(table, true, self.backend_kind().quote_style())?;
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let this = Clone::clone(self);
        let table_name = table_name.to_owned();
        spawn_blocking(move || {
            let sql = format!(
                r#"
        SELECT
            i.RDB$INDEX_NAME AS INDEX_NAME,
            i.RDB$UNIQUE_FLAG AS IS_UNIQUE,
            s.RDB$FIELD_NAME AS FIELD_NAME,
            CASE 
                WHEN rc.RDB$CONSTRAINT_TYPE = 'PRIMARY KEY' THEN 1 
                ELSE 0 
            END AS IS_PRIMARY_KEY
        FROM RDB$INDICES i
        LEFT JOIN RDB$INDEX_SEGMENTS s 
            ON s.RDB$INDEX_NAME = i.RDB$INDEX_NAME
        LEFT JOIN RDB$RELATION_CONSTRAINTS rc 
            ON rc.RDB$INDEX_NAME = i.RDB$INDEX_NAME
        WHERE i.RDB$SYSTEM_FLAG = 0
        AND i.RDB$RELATION_NAME = '{}'  -- 大写表名
        ORDER BY i.RDB$INDEX_NAME, s.RDB$FIELD_POSITION;

        "#,
                table_name
            );
            let mut conn = this.get_conn()?;
            let rows: Vec<(String, i64, String, i32)> = conn.query(&sql, ())?;
            let mut m: HashMap<String, Index> = HashMap::new();
            for (name, unique, col, is_primary) in rows.into_iter() {
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
                                kind: if is_primary != 0 {
                                    IndexKind::Primary
                                } else if unique == 1 {
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
        })
        .await?
    }

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<Option<String>> {
        let sql = format!(r#"DROP INDEX "{}";"#, index);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn show_view(
        &self,
        view_name: &str,
        para: ShowTableParameter,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let view_name = view_name.to_owned();
        spawn_blocking(move || {
            emit.send("trying to get view columns".into())?;
            let fields = this.show_table_columns_impl(&view_name)?;
            let sql = this.gen_show_sql(&view_name, None, Some(&para));
            emit.send("trying to get connection.".into())?;
            let mut conn = this.get_conn()?;
            log::info!("sql:{}", sql);
            emit.send("trying to query total count.".into())?;
            let total = Self::query_count(&mut conn, &view_name, para.where_str.as_deref())?;
            emit.send(
                QueryResp {
                    fields,
                    total,
                    sql: sql.clone().into(),
                    ..Default::default()
                }
                .into(),
            )?;
            emit.send("trying to query.".into())?;
            let stream: Box<dyn Iterator<Item = Result<Row, FbError>>> =
                conn.query_iter(&sql, ())?;
            for row in stream {
                let row = row?;
                let d = encode_row(&row, None)?;
                emit.send(d.into())?;
            }
            anyhow::Ok(())
        })
        .await?
    }
}

/// Firebird 里可直接写在 SQL 语句中的 CURRENT_* 字面量
pub const FB_CURRENT_LITERALS: &[&str] = &[
    "CURRENT_DATE",      //  2025-08-25
    "CURRENT_TIME",      //  14:36:07.1230
    "CURRENT_TIMESTAMP", //  2025-08-25 14:36:07.1230
];

impl SqlBuilder for FireBirdDB {
    fn get_current_values(&self) -> &'static [&'static str] {
        FB_CURRENT_LITERALS
    }

    fn put_value_in_sql(&self, typ: &str, value: &str) -> anyhow::Result<String> {
        let typ = typ.to_lowercase();
        log::trace!("encode value '{:?}' on type '{}'", value, typ);
        let r = match typ.as_str() {
            "text" | "varying" => anyhow::Ok(format!("'{}'", escape_sql_str(value))),
            "date" | "time" | "timestamp" => anyhow::Ok(format!("'{}'", escape_sql_str(value))),
            "blob" => anyhow::Ok(value.to_string()),
            _ => anyhow::Ok(value.to_string()),
        };
        r
    }
    fn get_schema(&self) -> Option<&str> {
        None
    }

    fn gen_show_sql(
        &self,
        table_name: &str,
        fields: Option<&[Field]>,
        para: Option<&ShowTableParameter>,
    ) -> String {
        let mut sql = format!(
            r#"SELECT  {}  FROM "{table_name}""#,
            self.backend_kind().format_fields(fields)
        );
        if let Some(para) = para {
            let ShowTableParameter {
                where_str,
                order_by,
                page,
                page_size,
                ..
            } = para;
            if let Some(where_str) = where_str {
                sql.push_str(format!(" WHERE {where_str} ").as_str());
            }
            if let Some(order_by) = order_by {
                sql.push_str(format!(" ORDER BY {order_by} ").as_str());
            }
            let offset = (page - 1) * page_size;
            sql.push_str(format!(" ROWS {} TO {} ", offset + 1, offset + page_size).as_str());
        }
        sql
    }
}

impl FireBirdDB {
    fn query_count(
        conn: &mut PooledConnection<PoolManager>,
        name: &str,
        where_str: Option<&str>,
    ) -> anyhow::Result<usize> {
        let mut s = format!(r#"SELECT count(1) from "{name}" "#);
        if let Some(where_str) = where_str {
            s.push_str(format!(" where {where_str}").as_str());
        }
        let row: Option<(i64,)> = conn.query_first(&s, ())?;
        anyhow::Ok(row.map(|x| x.0 as usize).unwrap_or_default())
    }
}

#[async_trait::async_trait]
impl CheckConn for FireBirdDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let mut conn = this.get_conn()?;
            let now = std::time::Instant::now();
            let _: Option<Row> = conn.query_first("SELECT 1 FROM RDB$DATABASE;", ())?;
            anyhow::Ok(now.elapsed())
        })
        .await?
    }
}

impl FireBirdDB {
    pub async fn new(config: FireBirdConfig) -> anyhow::Result<Self> {
        spawn_blocking(move || {
            let mut builder = r2d2::Pool::builder();
            builder = builder.max_size(POOL_MAX_CONNECTIONS);
            builder = builder.min_idle(Some(2));
            if let Some(x) = config.conn_timeout {
                builder = builder.connection_timeout(x.into());
            }
            let pool = builder.build(PoolManager {
                config: Arc::new(config.clone()),
            })?;
            anyhow::Ok(Self {
                config: Arc::new(config),
                pool,
                query_connection: Default::default(),
                query_connection_tx: Default::default(),
            })
        })
        .await?
    }

    pub(crate) fn get_conn(&self) -> anyhow::Result<PooledConnection<PoolManager>> {
        let conn = match self.config.conn_timeout {
            Some(timeout) => self.pool.get_timeout(Duration::from(timeout)),
            None => self.pool.get(),
        }?;
        anyhow::Ok(conn)
    }
}

#[cfg(test)]
pub(crate) async fn make_test_object() -> FireBirdDB {
    FireBirdDB::new(FireBirdConfig {
        host: "localhost".to_string(),
        port: 3050,
        username: "SYSDBA".to_string(),
        password: "root".to_string(),
        db: "/var/lib/firebird/data/test.fdb".to_string(),
        read_only: false,
        conn_timeout: Some(500000.into()),
    })
    .await
    .unwrap()
}

#[cfg(test)]
pub(crate) fn test_lock() -> &'static tokio::sync::Mutex<()> {
    static MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    &MUTEX
}

impl GetTestExportTableKinds for FireBirdDB {
    fn support_table_schema(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        CreateIndexColumn, CreateTableReq, CreateTableReqColumn, IndexKind, SqlNullAble,
    };
    #[allow(unused)]
    use crate::enable_test_log;

    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object().await;
        let d = crate::db::CheckConn::check_conn(&db).await.unwrap();
        println!("ping time:{:?}", d);
    }

    #[tokio::test]
    async fn test_read_only() {
        let _guard = test_lock().lock().await;

        let db = FireBirdDB::new(FireBirdConfig {
            host: "localhost".to_string(),
            port: 3050,
            username: "SYSDBA".to_string(),
            password: "root".to_string(),
            db: "/var/lib/firebird/data/test.fdb".to_string(),
            read_only: true,
            conn_timeout: Some(500000.into()),
        })
        .await
        .unwrap();

        let err = db
            .simple_execute_sql(
                r#"
                CREATE TABLE USERS (
                    USER_ID      INTEGER GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
                    USERNAME     VARCHAR(50) NOT NULL UNIQUE,
                    EMAIL        VARCHAR(100),
                    SALARY       NUMERIC(15, 2) DEFAULT 0.00,
                    IS_ACTIVE    SMALLINT DEFAULT 1 CHECK (IS_ACTIVE IN (0, 1)), -- Boolean workaround
                    BIO          BLOB SUB_TYPE TEXT,                             -- For large text
                    CREATED_AT   TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                );

        "#,
            )
            .await;
        assert!(err.is_err());
        if let Err(err) = err {
            let x = format!("{:?}", err);
            assert!(x.contains("read-only transaction"));
        }
    }

    #[tokio::test]
    async fn test_table_pagination() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let table_name = "TEST_TABLE_PAGINATION";
        let _ = db.drop_table(table_name, false).await;
        db.create_table(&CreateTableReq {
            name: table_name.into(),
            columns: vec![
                CreateTableReqColumn {
                    name: "ID".into(),
                    data_type: "SMALLINT".into(),
                    auto_increment: false,
                    primary_key: true,
                    default_value: None,
                    nullable: SqlNullAble::None,
                },
                CreateTableReqColumn {
                    name: "NAME".into(),
                    data_type: "varchar(255)".into(),
                    auto_increment: false,
                    primary_key: true,
                    default_value: None,
                    nullable: SqlNullAble::None,
                },
            ],
            indexes: vec![],
        })
        .await
        .unwrap();

        for i in 0..155 {
            let mut values = HashMap::new();
            values.insert("ID".to_owned(), Some((i + 1).to_string()));
            values.insert("NAME".to_owned(), Some(format!("name{}", i + 1)));
            db.table_insert(table_name, values).await.unwrap();
            if (i + 1) % 10 == 0 {
                println!("insert 10 records");
            }
        }
    }

    #[tokio::test]
    async fn test_query_multi_should_be_error() {
        let db = make_test_object().await;
        let mut conn = db.get_conn().unwrap();
        let stream: Result<Box<dyn Iterator<Item = Result<Row, FbError>>>, FbError> = conn
            .query_iter(
                "SELECT 1 FROM RDB$DATABASE;SELECT 2 FROM RDB$DATABASE; ",
                (),
            );
        assert!(stream.is_err());
    }

    #[tokio::test]
    async fn test_transaction() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;

        let table_name = "TEST_COMMIT";
        let _ = db.drop_table(table_name, false).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
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
            db.show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"),)
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
            db.show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"),)
                .await
                .unwrap()
                .total
                == 1
        );
    }

    #[tokio::test]
    async fn test_view() {
        let _guard = test_lock().lock().await;

        let db = make_test_object().await;
        let table_name = "TEST_VIEW_TABLE";
        let view_name = "TEST_VIEW";
        let _ = db.drop_view(view_name).await;

        let _ = db.drop_table(table_name, false).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();

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
            .show_view_collect(view_name, ShowTableParameter::new_stupid_test("id asc"))
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

    #[ignore = "not support yet"]
    #[tokio::test]
    async fn test_table_column() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let table_name = "TEST_COLUMN";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";

        db.open_connection(query_id).await.unwrap();
        let _resp = db
            .execute_sql(
                query_id,
                r#"
            CREATE TABLE TEST_COLUMN
            (
                ID INTEGER GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY -- 主键，无符号32位整数
            )
            "#,
            )
            .await
            .unwrap()
            .unwrap();

        let sql = db
            .add_table_column(
                table_name,
                "NAME",
                ColumnType {
                    typ: "text".to_string(),
                    nullable: Some(SqlNullAble::NotNull),
                    default_value: Some("abc".to_string()),
                    after: None,
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
                    typ: "VARCHAR(100)".to_string(),
                    nullable: SqlNullAble::Null.into(),
                    after: None,
                    default_value: Some("abc111".to_string()),
                }),
            )
            .await
            .unwrap();

        println!("modify table column sql:{}", sql);

        let sql = db.drop_table_column(table_name, "name2").await.unwrap();
        println!("drop table column sql:{}", sql);
    }

    #[tokio::test]
    async fn test_index() {
        let _guard = test_lock().lock().await;

        let table_name = "TEST_INDEX";
        let index_name = "TEST_INDEX_NAME_INDEX";
        let db = make_test_object().await;
        let _err = db.drop_index(table_name, index_name).await;
        if let Err(err) = _err {
            println!("drop index err:{}", err);
        }
        let _err = db.drop_table(table_name, false).await;
        if let Err(err) = _err {
            println!("drop table err:{}", err);
        }
        let req = CreateTableReq {
            name: table_name.to_owned(),
            columns: vec![
                CreateTableReqColumn {
                    name: "ID".into(),
                    data_type: "INTEGER".into(),
                    auto_increment: false,
                    primary_key: true,
                    default_value: None,
                    nullable: SqlNullAble::NotNull,
                },
                CreateTableReqColumn {
                    name: "NAME".into(),
                    data_type: "VARCHAR(100)".into(),
                    ..Default::default()
                },
                CreateTableReqColumn {
                    name: "CREATED_DATE".into(),
                    data_type: "TIMESTAMP".into(),
                    default_value: Some("CURRENT_TIMESTAMP".into()),
                    nullable: SqlNullAble::NotNull,
                    ..Default::default()
                },
                CreateTableReqColumn {
                    name: "STATUS".into(),
                    data_type: "CHAR(1)".into(),
                    ..Default::default()
                },
            ],
            indexes: vec![],
        };

        {
            let sql = db.create_table_sql_string(&req).unwrap();
            println!("create table sql:{}", sql);
        }

        db.create_table(&req).await.unwrap();

        db.create_index(
            table_name,
            &CreateIndexReq {
                name: index_name.to_owned(),
                columns: vec![
                    CreateIndexColumn {
                        col: "NAME".to_string(),
                        asc: None,
                    },
                    CreateIndexColumn {
                        col: "STATUS".to_string(),
                        asc: None,
                    },
                ],
                kind: IndexKind::Unique,
            },
        )
        .await
        .unwrap();
        let indexes = db.show_indexes(table_name).await.unwrap();

        for index in indexes.iter() {
            println!("index:{:?}", index);
        }
    }

    #[tokio::test]
    async fn test_list_table() {
        let _guard = test_lock().lock().await;
        let db = make_test_object().await;
        let tables = db.list_table().await.unwrap();
        for table in tables {
            println!("table:{:?}", table);
        }
    }
    #[tokio::test]
    async fn test_truncate() {
        let _guard = test_lock().lock().await;
        let table_name = "TEST_TRUNCATE";
        let db = make_test_object().await;
        let _err = db.drop_table(table_name, false).await;
        if let Err(err) = _err {
            println!("drop table err:{}", err);
        }

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();

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
            db.show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"),)
                .await
                .unwrap()
                .total
                == 0
        );
    }

    #[tokio::test]
    async fn test_table() {
        let _guard = test_lock().lock().await;

        let table_name = "TEST_TABLE";
        let db = make_test_object().await;
        let _err = db.drop_table(table_name, false).await;
        if let Err(err) = _err {
            println!("drop table err:{}", err);
        }
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
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
                .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
                .await
                .unwrap();
            println!("resp:{:?}", resp);
        }
        GetTestExportTableKinds::test_export(&db, table_name).await;
    }

    #[tokio::test]
    async fn test_table2() {
        let _guard = test_lock().lock().await;

        let table_name = "TEST_TABLE2";
        let db = make_test_object().await;
        let _err = db.drop_table(table_name, false).await;
        if let Err(err) = _err {
            println!("drop table err:{}", err);
        }
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
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
        values.insert("COL_BLOB_TEXT".to_string(), Some("x'616263'".to_string()));
        values.insert(
            "COL_BLOB_BINARY".to_string(),
            Some("x'48656C6C6F'".to_string()),
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
                .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
                .await
                .unwrap();
            println!("resp:{:?}", resp);
        }
        GetTestExportTableKinds::test_export(&db, table_name).await;
    }
}
