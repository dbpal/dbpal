use bb8::ManageConnection;
use dashmap::DashMap;
use libsql::{params, Connection, Row, ValueType};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::time::Duration;
#[cfg(test)]
use tokio::sync::{Mutex, OnceCell};

use crate::datasource_config::{LibSqlDBConfig, LibSqlDBConfigSource};
use crate::db::{
    create_html_table_data, create_html_table_header, create_markdown_data, create_markdown_header,
    export_query_as_xlsx_send_data, quote_sql_str, AutoCompletionQueryId, BackendKind, CheckConn,
    ColumnType, CreateIndexReq, CreateSchemaArg, CreateTableAndIndexSql, DBTrait,
    EncodeXlsxDatetime, ExportFrame, ExportKind, ExportQueryResult, ExtractSqlValeFromJsonValue,
    Field, FirstTrue, ForeignKeyDef, GetBackendKind, GetTestExportTableKinds, Index, IndexKind,
    ListTable, MetaValue, NameWithParameter, QueryEvent, QueryEventEmitter, QueryResp, QuerySql,
    ReadOnlyTrait, SchemaTrait, ServerMetas, ShowTableParameter, SqlBuilder, TriggerDetail,
    XlsxEdit, XlsxEditContent, POOL_MAX_CONNECTIONS,
};
use crate::helper::create_csv;
use crate::language_server::{get_update_sql_from_result_set_impl, LexerCompletionTrait};
use crate::sqlite::{
    encode_bytes, extract_auto_increment_key, extract_trigger_detail_from_sql, gen_show_sql,
    integer_like, numeric_like, real_like, sqlite_encode_xlsx_datetime,
    sqlite_extract_sql_value_from_json_value, text_like, SqliteDB,
};

#[derive(Clone)]
pub struct LibSqlDB {
    config: LibSqlDBConfig,
    pool: bb8::Pool<PoolManager>,
    pub(crate) query_connection: Arc<DashMap<String, Connection>>,
    query_tx: Arc<DashMap<String, libsql::Transaction>>,
    pub(crate) completion_id: AutoCompletionQueryId,
}

impl GetBackendKind for LibSqlDB {
    fn backend_kind(&self) -> BackendKind {
        BackendKind::LibSql
    }
}

fn get_row_type(r: &Row) -> Vec<Field> {
    let mut ret = Vec::with_capacity(r.column_count() as usize);
    for index in 0..r.column_count() {
        ret.push(Field::from_name_type(
            r.column_name(index).unwrap_or_default(),
            format!("{:?}", r.column_type(index).unwrap_or(ValueType::Null)),
        ));
    }
    ret
}

#[async_trait::async_trait]
impl ExportQueryResult for LibSqlDB {
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
            Some(query_id) => match self.query_connection.get(&query_id.to_string()) {
                Some(x) => x.clone(),
                None => {
                    anyhow::bail!("query_id not found ")
                }
            },
            None => {
                let x = PoolManager {
                    config: self.config.clone(),
                };
                x.connect().await?
            }
        };
        let mut stream = conn.query(sql, params!()).await?;
        let mut first = FirstTrue::default();

        let mut row_num = 0;
        while let Some(row) = stream.next().await? {
            match kind {
                ExportKind::Csv {
                    include_header,
                    delimiter,
                } => {
                    if first.get() && include_header {
                        let fs = get_row_type(&row);
                        let fs = fs
                            .into_iter()
                            .map(|x| x.field)
                            .map(Some)
                            .collect::<Vec<_>>();
                        sender.send(create_csv(&fs, delimiter)?.into()).await?;
                    }
                    let data = encode_row(&row)?;
                    sender.send(create_csv(&data, delimiter)?.into()).await?;
                }
                ExportKind::Json { .. } | ExportKind::JsonLine => {
                    let value = encode_row_as_json(&row)?;
                    sender.send(value.into()).await?;
                }
                ExportKind::Sql { .. } => anyhow::bail!("can't export query result to sql"),
                ExportKind::Xlsx { include_header, .. } => {
                    if first.get() && include_header {
                        for index in 0..row.column_count() {
                            sender
                                .send(
                                    XlsxEdit {
                                        row: row_num,
                                        column: index as u16,
                                        content: row
                                            .column_name(index)
                                            .unwrap_or_default()
                                            .to_owned()
                                            .into(),
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
                    if first.get() && include_header {
                        let fs = get_row_type(&row);
                        let fs = fs.into_iter().map(|x| x.field).collect::<Vec<_>>();
                        sender.send(create_html_table_header(&fs).into()).await?;
                    }
                    let data = encode_row(&row)?;
                    sender.send(create_html_table_data(&[data]).into()).await?;
                }
                ExportKind::MarkDown => {
                    if first.get() {
                        let fs = get_row_type(&row);
                        let fs = fs.into_iter().map(|x| x.field).collect::<Vec<_>>();
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

impl QuerySql for LibSqlDB {
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
        anyhow::Ok(!self.query_tx.contains_key(&query_id.to_string()))
    }

    async fn set_auto_commit(&self, query_id: &str, commit: bool) -> anyhow::Result<()> {
        if commit {
            anyhow::bail!("libsql use commit or rollback to end transaction");
        }
        let conn = self.query_connection.get(&query_id.to_string());
        let conn = match &conn {
            Some(x) => x.deref(),
            None => anyhow::bail!("collection closed????"),
        };
        let t = conn.transaction().await?;
        self.query_tx.insert(query_id.to_owned(), t);
        anyhow::Ok(())
    }

    async fn commit(&self, query_id: &str) -> anyhow::Result<()> {
        let t = self.query_tx.remove(&query_id.to_string());
        match t {
            Some(x) => {
                x.1.commit().await?;
                anyhow::Ok(())
            }
            None => {
                anyhow::bail!("transaction not found");
            }
        }
    }

    async fn rollback(&self, query_id: &str) -> anyhow::Result<()> {
        let t = self.query_tx.remove(&query_id.to_string());
        match t {
            Some(x) => {
                x.1.rollback().await?;
                anyhow::Ok(())
            }
            None => {
                anyhow::bail!("transaction not found");
            }
        }
    }

    async fn simple_execute_sql(&self, sql: &str) -> anyhow::Result<Option<usize>> {
        let conn = self.get_conn().await?;
        conn.execute_batch(sql)
            .await
            .map_err(|err| anyhow::Error::msg(format!("sql:{} err:{}", sql, err)))?;
        anyhow::Ok(None)
    }
    async fn execute_sql(
        &self,
        query_id: &str,
        sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let tr = self.query_tx.get(&query_id.to_owned());
        let conn = self.query_connection.get(&query_id.to_string());
        let conn = conn.as_deref();
        let r = match &tr {
            Some(x) => x.execute_batch(sql).await,
            None => match &conn {
                Some(x) => x.execute_batch(sql).await,
                None => anyhow::bail!("collection closed????"),
            },
        };

        println!("!!!!!!!!!!!!!!:{:?}", r);

        let ret: Result<QueryResp, String> = match r {
            Ok(..) => Ok(QueryResp::execute_ok(sql.to_owned())),
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
        let tr = self.query_tx.get(&query_id.to_owned());
        let conn = self.query_connection.get(&query_id.to_string());
        let conn = conn.as_deref();
        let conn = match &tr {
            Some(x) => x.deref().deref(),
            None => match &conn {
                Some(x) => x,
                None => anyhow::bail!("collection closed????"),
            },
        };
        let mut batch_rows = conn.execute_batch(sql).await?;
        while let Some(mut rows) = batch_rows.next_stmt_row().flatten() {
            emit.send(QueryEvent::new_status("getting next result set"))?;
            let mut first = FirstTrue::default();

            while let Some(row) = rows.next().await? {
                if first.get() {
                    emit.send(
                        QueryResp {
                            fields: (0..row.column_count())
                                .map(|index| {
                                    Field::from_name_type(
                                        row.column_name(index).unwrap_or_default().to_owned(),
                                        format!(
                                            "{:?}",
                                            row.column_type(index).unwrap_or(ValueType::Null)
                                        ),
                                    )
                                })
                                .collect(),
                            sql: Some(sql.to_owned()),
                            tables: crate::language_server::LangServerCommon::extract_tables(
                                self, sql,
                            )
                            .unwrap_or_default(),
                            ..Default::default()
                        }
                        .into(),
                    )?;
                }
                let data = encode_row(&row)?;
                emit.send(data.into())?;
            }
        }
        anyhow::Ok(())
    }

    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        let conn = self.connect().await?;
        self.query_connection.insert(id.to_owned(), conn);
        self.query_tx.remove(&id.to_string());
        anyhow::Ok(())
    }
    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        let id = id.to_string();
        self.query_tx.remove(&id);
        self.query_connection.remove(&id);
        anyhow::Ok(())
    }
}

impl LibSqlDB {
    async fn query_auto_increment_key(&self, table_name: &str) -> anyhow::Result<Option<String>> {
        let s = format!(
            r#"SELECT sql from sqlite_master where type='table' and name='{}'"#,
            table_name
        );
        let conn = self.get_conn().await?;
        let row = conn.prepare(&s).await?.query_row(params!()).await?;
        let sql: String = row.get(0)?;
        extract_auto_increment_key(sql.as_str())
    }

    async fn query_count(&self, name: &str, where_str: Option<&str>) -> anyhow::Result<usize> {
        let conn = self.get_conn().await?;
        let mut s = format!(r#"SELECT count(1) from "{name}" "#);
        if let Some(where_str) = where_str {
            s.push_str(format!(" where {where_str}").as_str());
        }
        let row = conn.prepare(&s).await?.query_row(params!()).await?;
        let count: i64 = row.get(0)?;
        anyhow::Ok(count as usize)
    }
}

impl LibSqlDB {
    pub(crate) async fn connect(&self) -> anyhow::Result<Connection> {
        let conn = PoolManager {
            config: self.config.clone(),
        }
        .connect()
        .await?;
        anyhow::Ok(conn)
    }
}

pub struct PoolManager {
    config: LibSqlDBConfig,
}

impl bb8::ManageConnection for PoolManager {
    type Connection = Connection;
    type Error = anyhow::Error;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let replica = self.config.replica;
        let conn = match &self.config.source {
            LibSqlDBConfigSource::Local { .. } => {
                // let b = if replica {
                //     libsql::Builder::new_local_replica(path).build().await?
                // } else {
                //     libsql::Builder::new_local(path).build().await?
                // };
                // b.connect()?
                anyhow::bail!("local db not supported yet");
            }

            LibSqlDBConfigSource::Remote {
                url, auth_token, ..
            } => {
                let conn = if replica {
                    // libsql::Builder::new_remote_replica(path, url.clone(), auth_token.clone())
                    //     .build()
                    //     .await?
                    //     .connect()?
                    anyhow::bail!("remote db not supported yet");
                } else {
                    libsql::Builder::new_remote(url.clone(), auth_token.clone())
                        .build()
                        .await?
                        .connect()?
                };
                conn
            }
        };
        Ok(conn)
    }

    async fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.query("select 1", params![]).await?;
        Ok(())
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        false
    }
}

impl LibSqlDB {
    pub async fn new(config: LibSqlDBConfig) -> anyhow::Result<Self> {
        let manager = PoolManager {
            config: config.clone(),
        };
        let mut builder = bb8::Pool::builder();
        if let Some(x) = config.conn_timeout {
            builder = builder.connection_timeout(x.into());
        }
        builder = builder.max_size(POOL_MAX_CONNECTIONS);
        builder = builder.min_idle(Some(5));
        let pool = builder.build(manager).await?;
        anyhow::Ok(Self {
            config,
            pool,
            query_connection: Default::default(),
            query_tx: Default::default(),
            completion_id: Default::default(),
        })
    }

    pub(crate) async fn get_conn(
        &self,
    ) -> anyhow::Result<bb8::PooledConnection<'static, PoolManager>> {
        let conn = self
            .pool
            .get_owned()
            .await
            .map_err(|e| anyhow::Error::msg(format!("{:?}", e)))?;
        anyhow::Ok(conn)
    }
}

#[async_trait::async_trait]

impl CheckConn for LibSqlDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        let conn = self.get_conn().await?;
        let now = std::time::Instant::now();
        conn.query("select 1", params!()).await?;
        anyhow::Ok(now.elapsed())
    }
}

impl std::fmt::Debug for LibSqlDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "libsql@{:?}", self.config)
    }
}

impl ReadOnlyTrait for LibSqlDB {
    fn read_only(&self) -> bool {
        self.config.read_only
    }
}

impl LibSqlDB {
    pub(crate) async fn list_table_impl(&self, db: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql =
            format!(r#"SELECT name FROM "{db}".sqlite_master WHERE type='table' ORDER BY name "#);
        log::info!("sql:{}", sql);
        let mut rows: libsql::Rows = match self.query_connection.get(&self.completion_id.get()) {
            Some(conn) => {
                let conn = conn.deref();
                conn.query(&sql, params!()).await?
            }
            None => {
                let conn = self.get_conn().await?;
                conn.query(&sql, params!()).await?
            }
        };

        let mut ret = Vec::new();
        while let Some(r) = rows.next().await? {
            let table_name: String = r.get(0)?;
            ret.push(table_name);
        }

        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    pub(crate) async fn list_view_impl(&self, db: &str) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"
            SELECT name
            FROM "{db}".sqlite_master
            WHERE type = 'view'; "#
        );
        log::info!("sql:{}", sql);
        let mut rows = match self.query_connection.get(&self.completion_id.get()) {
            Some(conn) => {
                let conn = conn.deref();
                conn.query(&sql, params!()).await?
            }
            None => {
                let conn = self.get_conn().await?;
                conn.query(&sql, params!()).await?
            }
        };
        let mut ret = Vec::new();
        while let Some(r) = rows.next().await? {
            let name: String = r.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }
}

#[async_trait::async_trait]
impl ServerMetas for LibSqlDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        let sql = "SELECT sqlite_version()";
        let conn = self.get_conn().await?;
        let row = conn.prepare(sql).await?.query_row(params!()).await?;
        let version: String = row.get(0)?;
        anyhow::Ok(vec![MetaValue::new("version", version)])
    }
}
#[async_trait::async_trait]

impl DBTrait for LibSqlDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        anyhow::Ok(vec!["main".to_owned()])
    }
    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        if name != "main" {
            anyhow::bail!("libsql should only open main".to_string())
        }

        anyhow::Ok(Arc::new(Clone::clone(self)))
    }
    async fn create_schema(
        &self,
        _name: &str,
        _extra: Option<CreateSchemaArg>,
    ) -> anyhow::Result<()> {
        self.backend_kind()
            .not_support_feature("can't create schema")
    }
    async fn drop_schema(&self, _name: &str) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature("can't drop schema")
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait::async_trait]
impl SchemaTrait for LibSqlDB {
    async fn list_foreign_key(&self, table: &str) -> anyhow::Result<Vec<ForeignKeyDef>> {
        let conn = self.get_conn().await?;

        // PRAGMA statements in libsql/SQLite do not support parameter binding, so format the values directly.
        let sql = format!(r#"PRAGMA foreign_key_list("{}")"#, table);
        log::info!("sql:{}", sql);

        // 1. Prepare the statement.
        let mut rows = conn.query(&sql, params!()).await?;
        let mut ret = vec![];

        while let Some(row) = rows.next().await? {
            // Note: row.get in libsql/rusqlite uses zero-based indexing.
            // PRAGMA foreign_key_list returns columns in this order:
            // id(0), seq(1), table(2), from(3), to(4), on_update(5), on_delete(6), match(7)
            let table: String = row.get(2)?;
            let from: String = row.get(3)?;
            let to: String = row.get(4)?;
            let on_update: String = row.get(5)?;
            let on_delete: String = row.get(6)?;

            ret.push(ForeignKeyDef {
                name: None, // SQLite/libsql does not provide constraint names, so explicitly set this to None.
                table,
                from,
                to,
                on_update,
                on_delete,
            })
        }

        anyhow::Ok(ret)
    }

    fn list_sql_predefine_values(&self) -> &'static [&'static str] {
        <Self as SqlBuilder>::get_predefine_values(self)
    }

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        LexerCompletionTrait::get_types(self).await
    }

    async fn create_index(&self, table: &str, req: &CreateIndexReq) -> anyhow::Result<String> {
        let sql = req.common_sql(table, true, self.backend_kind().quote_style())?;
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<Option<String>> {
        let sql = format!(r#"DROP INDEX "{index}";"#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }
    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<String> {
        let sql = format!(r#"ALTER TABLE "{table}" DROP COLUMN "{col}";"#);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn add_table_column(
        &self,
        table: &str,
        name: &str,
        col: ColumnType,
    ) -> anyhow::Result<String> {
        let sql = format!(
            r#"ALTER TABLE "{table}" ADD COLUMN "{name}" {} {} {}"#,
            col.typ,
            col.nullable.unwrap_or_default(),
            if let Some(d) = col.default_value.as_ref() {
                format!("DEFAULT {}", self.encode_value(&col.typ, d.as_str())?)
            } else {
                "".to_owned()
            }
        );
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        let mut conn = self.get_conn().await?;
        let sql = format!(r#"select * from "{view_name}" limit 1"#);
        log::info!("sql:{}", sql);
        let s = conn.deref_mut().prepare(&sql).await?;
        let ret = s
            .columns()
            .iter()
            .map(|x| Field::from_name_type(x.name(), x.decl_type().unwrap_or_default()))
            .collect::<Vec<_>>();
        anyhow::Ok(ret)
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        if self.table_exists(dup).await? {
            anyhow::bail!(format!("table '{}' already exists", dup));
        }
        let sql = format!(r#"CREATE TABLE "{dup}" AS SELECT * FROM "{table}""#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<Option<String>> {
        let sql = format!(r#"ALTER TABLE "{old}" RENAME TO "{new}""#);
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn list_trigger(&self, table_name: &str) -> anyhow::Result<Vec<NameWithParameter>> {
        let sql = format!(
            r#"
            SELECT name 
            FROM sqlite_master 
            WHERE type = 'trigger' AND tbl_name = '{table_name}';
        "#
        );
        log::info!("sql:{}", sql);
        let conn = self.get_conn().await?;
        let mut rows = conn.query(&sql, params!()).await?;
        let mut ret = Vec::new();
        while let Some(row) = rows.next().await? {
            let name: String = row.get(0)?;
            ret.push(name);
        }
        anyhow::Ok(ret.into_iter().map(NameWithParameter::from).collect())
    }

    async fn show_trigger(&self, _table: &str, name: &str) -> anyhow::Result<TriggerDetail> {
        let mut conn = self.get_conn().await?;
        query_trigger_detail(conn.deref_mut(), name).await
    }

    async fn drop_trigger(&self, _table: &str, name: &str) -> anyhow::Result<String> {
        let sql = format!(
            r#"
            DROP TRIGGER "{name}"
        "#
        );
        log::info!("sql:{}", sql);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn show_create_table_sql(
        &self,
        table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let table = self.create_table_or_view_sql(table_name, "table").await?;
        let indexes = self.get_table_indexes_sql(table_name).await?;

        anyhow::Ok(CreateTableAndIndexSql {
            create_table: table,
            indexes,
        })
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
                log::info!("sql:{}", sql);
                self.export_query_result(None, &sql, kind, sender).await
            }
        }
    }

    async fn show_view_sql(&self, view_name: &str, _: bool) -> anyhow::Result<String> {
        self.create_table_or_view_sql(view_name, "view").await
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_view_impl(SqliteDB::MAIN_DB).await
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<String> {
        self.can_write()?;
        let sql = format!(r#"DROP VIEW "{view_name}""#);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql)
    }

    async fn drop_table(&self, table: &str, _cascade: bool) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let sql = format!(r#"DROP TABLE "{table}" "#,);
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<Option<String>> {
        self.can_write()?;
        let sql = format!(
            r#"
            DELETE FROM "{table_name}";
            DELETE FROM sqlite_sequence WHERE name = '{table_name}';
        "#
        );
        self.simple_execute_sql(&sql).await?;
        anyhow::Ok(sql.into())
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.list_table_impl(SqliteDB::MAIN_DB).await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let increment_key = self.query_auto_increment_key(table_name).await?;
        let increment_key = increment_key.as_deref();

        let s = format!(r#"pragma table_info("{table_name}")"#);
        let conn = self.get_conn().await?;
        let mut rows = conn.query(&s, params!()).await?;
        let mut ret = Vec::new();
        // cid name type     notnull  delt_value pk
        // 0	id	INTEGER	 0		  null      1
        while let Some(r) = rows.next().await? {
            let name: String = r.get(1)?;
            let ty: String = r.get(2)?;
            let not_null: i64 = r.get(3)?;
            let delt_value: Option<String> = r.get(4)?;
            let pk: Option<i64> = r.get(5)?;
            let is_auto_increment = Some(name.as_str()) == increment_key;

            ret.push(Field::new(
                name,
                ty,
                not_null == 0,
                delt_value,
                pk.map(|x| x == 1).unwrap_or_default(),
                is_auto_increment,
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
        emit.send("trying to query table columns".into())?;
        let fields = self.show_table_columns(table_name).await?;
        emit.send("trying to query total count of this table.".into())?;
        let total = self.query_count(table_name, para.filter.as_deref()).await?;
        let sql = self.gen_show_sql(table_name, Some(&fields), Some(&para));
        log::info!("show table sql:{}", sql);
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
        let conn: bb8::PooledConnection<'_, PoolManager> = self.get_conn().await?;
        emit.send("trying to query data".into())?;
        let mut query_rows = conn.query(&sql, params!()).await?;
        while let Some(row) = query_rows.next().await? {
            let data = encode_row(&row)?;
            emit.send(data.into())?;
        }
        anyhow::Ok(())
    }

    async fn show_view(
        &self,
        view_name: &str,
        para: ShowTableParameter,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        emit.send("trying to get connection.".into())?;
        let conn = self.get_conn().await?;
        emit.send("trying to query total count of this view.".into())?;
        let total = self.query_count(view_name, para.filter.as_deref()).await?;
        let sql = self.gen_show_sql(view_name, None, Some(&para));
        log::info!("show table sql:{}", sql);

        if total == 0 {
            let s = conn.prepare(&sql).await?;
            let fields: Vec<_> = s
                .columns()
                .iter()
                .map(|c| Field::from_name_type(c.name(), c.decl_type().unwrap_or_default()))
                .collect();
            emit.send(
                QueryResp {
                    fields,
                    total: total as f64,
                    sql: sql.clone().into(),
                    ..Default::default()
                }
                .into(),
            )?;
            return anyhow::Ok(());
        }

        emit.send("trying to query data".into())?;
        let mut stream = conn.query(&sql, params!()).await?;
        let mut first = FirstTrue::default();

        while let Some(row) = stream.next().await? {
            if first.get() {
                emit.send(
                    QueryResp {
                        fields: get_row_type(&row),
                        total: total as f64,
                        sql: sql.clone().into(),
                        ..Default::default()
                    }
                    .into(),
                )?;
            }
            let d = encode_row(&row)?;
            emit.send(d.into())?;
        }

        anyhow::Ok(())
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let sql = format!("PRAGMA index_list({table_name})");
        log::info!("sql:{}", sql);
        let conn = self.get_conn().await?;
        let mut rows = conn.query(&sql, params!()).await?;

        // seq name unique origin partial
        // 0   xxx  1      c      0
        let mut ret = Vec::new();
        while let Some(r) = rows.next().await? {
            let name: String = r.get(1)?;
            let unique: i64 = r.get(2)?;
            // query create index sql
            let sql = format!("PRAGMA index_info('{name}')");
            log::info!("sql:{}", sql);
            let mut rows = conn.query(&sql, params!()).await?;
            let mut on = Vec::with_capacity(10);
            while let Some(r) = rows.next().await? {
                let name: String = r.get(2)?;
                on.push(name);
            }
            ret.push(Index::new(
                name,
                if unique == 1 {
                    IndexKind::Unique
                } else {
                    IndexKind::Index
                },
                on,
            ))
        }
        anyhow::Ok(ret)
    }
}

impl SqlBuilder for LibSqlDB {
    fn get_predefine_values(&self) -> &'static [&'static str] {
        crate::sqlite::SQLITE_CURRENT_LITERALS
    }
    fn encode_value(&self, typ: &str, value: &str) -> anyhow::Result<String> {
        let typ = typ.to_uppercase();

        let v = match typ.as_str() {
            "" | "TEXT" | "NULL" => quote_sql_str(value),
            "DATE" | "TIME" | "DATETIME" => format!("'{}'", value),
            _ if text_like(typ.as_str()) => quote_sql_str(value),
            "REAL" | "INTEGER" | "NUMERIC" | "BOOLEAN" => value.to_string(),
            _ if integer_like(typ.as_str()) || numeric_like(typ.as_str()) || real_like(&typ) => {
                value.to_string()
            }
            "BLOB" => value.to_string(),
            _ => {
                let e = format!("type '{}' not handled", typ);
                log::error!("{}", e);
                anyhow::bail!(e)
            }
        };

        anyhow::Ok(v)
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
        gen_show_sql(table_name, fields, para)
    }
}
impl LibSqlDB {
    async fn create_table_or_view_sql(
        &self,
        table_name: &str,
        kind: &str,
    ) -> anyhow::Result<String> {
        let conn = self.get_conn().await?;
        let sql = format!(
            "SELECT sql FROM sqlite_master WHERE type = '{kind}' AND name = '{table_name}';"
        );
        log::info!("sql:{}", sql);
        let row = conn.prepare(&sql).await?.query_row(params!()).await?;
        let sql: String = row.get(0)?;
        anyhow::Ok(sql)
    }

    async fn get_table_indexes_sql(&self, table_name: &str) -> anyhow::Result<Vec<String>> {
        let conn = self.get_conn().await?;
        let sql = format!(
            "SELECT sql FROM sqlite_master WHERE type = 'index' AND tbl_name = '{table_name}';"
        );
        log::info!("sql:{}", sql);
        let mut rows = conn.query(&sql, params!()).await?;

        let mut ret = Vec::new();
        while let Some(r) = rows.next().await? {
            let sql: String = r.get(0)?;
            ret.push(sql);
        }
        anyhow::Ok(ret)
    }

    async fn export_table_as_sql(
        &self,
        table_name: &str,
        para: Option<ShowTableParameter>,
        include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            let create_table = SchemaTrait::show_create_table_sql(self, table_name).await?;
            sender.send(create_table.to_string().into()).await?;
        }
        let conn = self.get_conn().await?;
        let cols = SchemaTrait::show_table_columns(self, table_name).await?;
        let sql = self.gen_show_sql(table_name, Some(&cols), para.as_ref());
        log::info!("sql:{}", sql);
        let mut rows = conn.query(&sql, params!()).await?;

        while let Some(row) = rows.next().await? {
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

async fn query_trigger_detail(conn: &mut Connection, name: &str) -> anyhow::Result<TriggerDetail> {
    let sql = format!("SELECT sql from sqlite_master WHERE type = 'trigger' and name='{name}'");
    let row = conn.prepare(&sql).await?.query_row(params!()).await?;
    let sql: String = row.get(0)?;
    extract_trigger_detail_from_sql(name, sql)
}

impl EncodeXlsxDatetime for LibSqlDB {
    fn encode(&self, ty: &str, data: &calamine::ExcelDateTime) -> anyhow::Result<Option<String>> {
        sqlite_encode_xlsx_datetime(ty, data)
    }

    fn is_datatime(&self, ty: &str) -> anyhow::Result<bool> {
        anyhow::Ok(ty.eq_ignore_ascii_case("DATETIME"))
    }

    fn is_date(&self, ty: &str) -> anyhow::Result<bool> {
        anyhow::Ok(ty.eq_ignore_ascii_case("DATE"))
    }

    fn is_time(&self, ty: &str) -> anyhow::Result<bool> {
        anyhow::Ok(ty.eq_ignore_ascii_case("TIME"))
    }
}

fn encode_row_as_xlsx(row: &Row) -> anyhow::Result<Vec<Option<XlsxEditContent>>> {
    let mut values = Vec::with_capacity(row.column_count() as usize);
    for c in 0..row.column_count() {
        let ty = row.column_type(c).unwrap_or(ValueType::Null);
        let v: Option<XlsxEditContent> = match ty {
            ValueType::Null => {
                if let Result::Ok(s) = row.get::<f64>(c) {
                    Some(XlsxEditContent::number(s))
                } else if let Result::Ok(s) = row.get::<i64>(c) {
                    Some(s.into())
                } else if let Result::Ok(s) = row.get::<String>(c) {
                    Some(s.into())
                } else if let Result::Ok(s) = row.get::<bool>(c) {
                    Some(s.into())
                } else if let Result::Ok(s) = row.get::<Vec<u8>>(c) {
                    Some(encode_bytes(s.as_slice()).into())
                } else {
                    None
                }
            }
            ValueType::Text => {
                let v: Option<String> = row.get(c)?;
                v.map(|x| x.into())
            }
            ValueType::Integer => {
                let v: Option<i64> = row.get(c)?;
                v.map(|x| x.into())
            }
            ValueType::Real => {
                let v: Option<f64> = row.get(c)?;
                v.map(|x| format!("{}", x).into())
            }
            ValueType::Blob => {
                let v: Option<Vec<u8>> = row.get(c)?;
                v.map(|x| encode_bytes(x.as_slice()).into())
            }
        };

        values.push(v);
    }

    anyhow::Ok(values)
}

fn encode_row(row: &Row) -> anyhow::Result<Vec<Option<String>>> {
    let mut values = Vec::with_capacity(row.column_count() as usize);
    for c in 0..row.column_count() {
        let ty = row.column_type(c).unwrap_or(ValueType::Null);
        let v = match ty {
            ValueType::Null => {
                if let Result::Ok(s) = row.get::<f64>(c) {
                    Some(s.to_string())
                } else if let Result::Ok(s) = row.get::<i64>(c) {
                    Some(s.to_string())
                } else if let Result::Ok(s) = row.get::<String>(c) {
                    Some(s)
                } else if let Result::Ok(s) = row.get::<bool>(c) {
                    Some(s.to_string())
                } else if let Result::Ok(s) = row.get::<Vec<u8>>(c) {
                    Some(encode_bytes(s.as_slice()))
                } else {
                    None
                }
            }
            ValueType::Text => {
                let v: Option<String> = row.get(c)?;
                v
            }
            ValueType::Integer => {
                let v: Option<i64> = row.get(c)?;
                v.map(|x| x.to_string())
            }
            ValueType::Real => {
                let v: Option<f64> = row.get(c)?;
                v.map(|x| format!("{}", x))
            }
            ValueType::Blob => {
                let v: Option<Vec<u8>> = row.get(c)?;
                v.map(|x| encode_bytes(x.as_slice()))
            }
        };

        values.push(v);
    }

    anyhow::Ok(values)
}

fn encode_row_as_json(r: &Row) -> anyhow::Result<serde_json::Value> {
    let mut values = serde_json::Map::default();
    let null_value = serde_json::Value::Null;

    for c in 0..r.column_count() {
        let ty = r.column_type(c).unwrap_or(ValueType::Null);

        let v: serde_json::Value = match ty {
            ValueType::Null => {
                if let Result::Ok(s) = r.get::<f64>(c) {
                    serde_json::Value::from(s)
                } else if let Result::Ok(s) = r.get::<Vec<u8>>(c) {
                    serde_json::Value::from(encode_bytes(s.as_slice()))
                } else if let Result::Ok(s) = r.get::<i64>(c) {
                    serde_json::Value::from(s)
                } else if let Result::Ok(s) = r.get::<String>(c) {
                    serde_json::Value::from(s)
                } else {
                    null_value.clone()
                }
            }
            ValueType::Text => {
                let v: Option<String> = r.get(c)?;
                v.map(serde_json::Value::String)
                    .unwrap_or(null_value.clone())
            }
            ValueType::Real => {
                let v: Option<f64> = r.get(c)?;
                v.map(serde_json::Value::from).unwrap_or(null_value.clone())
            }
            ValueType::Integer => {
                let v: Option<i64> = r.get(c)?;
                v.map(serde_json::Value::from).unwrap_or(null_value.clone())
            }
            ValueType::Blob => {
                let v: Option<Vec<u8>> = r.get(c)?;
                v.map(serde_json::Value::from).unwrap_or(null_value.clone())
            }
        };
        values.insert(r.column_name(c).unwrap_or_default().to_owned(), v);
    }
    anyhow::Ok(serde_json::Value::Object(values))
}

// docker run --name libsql-server -d   -p 8111:8080    -e SQLD_NODE=primary ghcr.io/tursodatabase/libsql-server:latest
#[cfg(test)]
// this test no need clean up, we use memory sqlite db.
pub(crate) async fn make_test_object() -> &'static Mutex<LibSqlDB> {
    static DB: OnceCell<Mutex<LibSqlDB>> = OnceCell::const_new();
    DB.get_or_init(|| async {
        let libsql = LibSqlDB::new(LibSqlDBConfig {
            conn_timeout: Some(5000.into()),
            replica: false,
            source: LibSqlDBConfigSource::Remote {
                url: "http://localhost:8111".to_string(),
                auth_token: "".to_string(),
                path: None,
            },
            read_only: false,
        })
        .await
        .unwrap();
        Mutex::new(libsql)
    })
    .await
}

impl ExtractSqlValeFromJsonValue for LibSqlDB {
    fn extrat_sql_value_from_json_value(
        &self,
        value: &serde_json::Value,
        _ty: &str,
    ) -> anyhow::Result<Option<String>> {
        sqlite_extract_sql_value_from_json_value(value)
    }
}

impl GetTestExportTableKinds for LibSqlDB {}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        db::{
            CreateIndexColumn, CreateTableReq, CreateTableReqColumn, SqlNullAble,
            TestQueryEventEmit,
        },
        enable_test_log,
    };

    #[ignore = "this exists because of next test not pass,I can't get foreign keys after create table and I don't know why"]
    #[tokio::test]
    async fn test_foreign_key_get() {
        let db = make_test_object().await.lock().await;

        let f = db.list_foreign_key("test_comments").await.unwrap();
        assert!(f.len() == 1);
        for f in f.iter() {
            println!("foreign key:{:?}", f);
        }
    }

    #[tokio::test]
    async fn test_foreign_key() {
        enable_test_log();
        let db = make_test_object().await.lock().await;

        let table_name_posts = "test_posts";
        let table_name_comments = "test_comments";
        let _ = db.drop_table(table_name_comments, false).await;
        let _ = db.drop_table(table_name_posts, false).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();

        db.execute_sql(
            query_id,
            r#"
        CREATE TABLE test_posts (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            content TEXT
        );"#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
        CREATE TABLE test_comments (
            comment_id INTEGER PRIMARY KEY AUTOINCREMENT, -- 修复了原代码拼写缺失
            author_name TEXT,
            post_id INTEGER,
            FOREIGN KEY (post_id) REFERENCES test_posts(id) 
            ON DELETE CASCADE 
        );"#,
        )
        .await
        .unwrap()
        .unwrap();

        // Note: Do not call close_connection here, or test_foreign_key_get will not be able to access the connection.
        // db.close_connection(query_id).await.unwrap();
    }

    #[tokio::test]
    async fn test_table_pagination() {
        let db = make_test_object().await.lock().await;
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

    async fn check_conn() {
        let db = make_test_object().await.lock().await;
        let d = db.check_conn().await.unwrap();
        println!("{:?}", d);
    }

    #[tokio::test]
    async fn test_server_metas() {
        let db = make_test_object().await.lock().await;
        let metas = ServerMetas::show_metas(db.deref()).await.unwrap();
        println!("{:?}", metas);
    }

    #[tokio::test]
    async fn test_table_column() {
        let db = make_test_object().await.lock().await;
        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
                query_id,
                r#"
                CREATE TABLE test_column
                (
                    id INTEGER PRIMARY KEY AUTOINCREMENT                      -- 主键，无符号32位整数
                )
                "#,
            )
            .await
            .unwrap() .unwrap() ;

        let sql = db
            .add_table_column(
                table_name,
                "name",
                ColumnType {
                    typ: "VARCHAR(255)".to_string(),
                    nullable: SqlNullAble::Null.into(),
                    after: Some("id".to_string()),
                    default_value: Some("abc".to_string()),
                },
            )
            .await
            .unwrap();
        println!("add table column:{}", sql);

        let sql = db.drop_table_column(table_name, "name").await.unwrap();
        println!("drop table column:{}", sql);
    }

    #[tokio::test]
    async fn test_transaction() {
        let db = make_test_object().await.lock().await;

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
        db.set_auto_commit(query_id, false).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            INSERT INTO test_commit (ID, NAME) VALUES (2, 'Test1');
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
    async fn test_server_select() {
        let db = make_test_object().await.lock().await;
        let conn = db.get_conn().await.unwrap();
        let _ = conn.execute("drop table users", params!()).await;
        let mut rows = conn
            .query(
                "select 1  ; CREATE TABLE users (
                                id INTEGER PRIMARY KEY AUTOINCREMENT 
                                );
",
                params!(),
            )
            .await
            .unwrap();

        while let Some(r) = rows.next().await.unwrap() {
            for index in 0..r.column_count() {
                println!("{:?}-{:?}", r.column_name(index), r.column_type(index));
            }
        }
    }

    #[tokio::test]
    async fn test_rename() {
        let db = make_test_object().await.lock().await;

        let _ = db.drop_table("test_rename", false).await;
        let _ = db.drop_table("test_rename2", false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE IF NOT EXISTS test_rename (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,  
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
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
            ShowTableParameter::new_stupid_test("id asc"),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_duplicate() {
        let db = make_test_object().await.lock().await;
        let _ = db.drop_table("test_duplicate", false).await;
        let _ = db.drop_table("test_duplicate2", false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE IF NOT EXISTS test_duplicate (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,  
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
                );"#,
        )
        .await
        .unwrap()
        .unwrap();

        db.rename_table("test_duplicate", "test_duplicate2")
            .await
            .unwrap();

        db.show_table_collect(
            "test_duplicate2",
            ShowTableParameter::new_stupid_test("id asc"),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_triggers() {
        let db = make_test_object().await.lock().await;
        let _ = db.drop_table("test_create_trigger", false).await;
        let _ = db.drop_table("test_create_trigger_update", false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();

        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_create_trigger (
                id INTEGER PRIMARY KEY AUTOINCREMENT, -- 自动递增的ID作为主键
                info TEXT NOT NULL 
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.execute_sql(
            query_id,
            r#"
        CREATE TABLE test_create_trigger_update (
            id INTEGER PRIMARY KEY AUTOINCREMENT, -- 自动递增的ID作为主键
            info TEXT NOT NULL
        );
    "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.execute_sql(
            query_id,
            r#"
        CREATE TRIGGER after_insert_test_create_trigger
        AFTER INSERT ON test_create_trigger
        FOR EACH ROW
        BEGIN
            INSERT INTO test_create_trigger_update (info)
            VALUES (NEW.info);
        END;
    "#,
        )
        .await
        .unwrap()
        .unwrap();
        let table = "test_create_trigger";

        let triggers = db.list_trigger(table).await.unwrap();
        for trigger in triggers.iter() {
            let details = db
                .show_trigger("test_create_trigger", trigger.name.as_str())
                .await
                .unwrap();
            println!("name:{} , detail:{:?}", trigger.name, details);
        }
    }
    #[tokio::test]
    async fn test_index() {
        let db = make_test_object().await.lock().await;
        let table_name = "test_index";
        let index_name = "name_index";
        let _ = db.drop_index(table_name, index_name).await;
        let _err = db.drop_table(table_name, false).await;
        if let Err(err) = _err {
            println!("drop table failed,err:{}", err);
        }
        let req = CreateTableReq {
            name: table_name.to_string(),
            columns: vec![
                CreateTableReqColumn {
                    name: "id".to_string(),
                    data_type: "INTEGER".into(),
                    auto_increment: true,
                    primary_key: true,
                    default_value: None,
                    nullable: Default::default(),
                },
                CreateTableReqColumn {
                    name: "name".into(),
                    data_type: "TEXT".to_string(),
                    auto_increment: false,
                    primary_key: false,
                    default_value: None,
                    nullable: SqlNullAble::NotNull,
                },
                CreateTableReqColumn {
                    name: "age".into(),
                    data_type: "INTEGER".into(),
                    ..Default::default()
                },
                CreateTableReqColumn {
                    name: "created_at".into(),
                    data_type: "DATETIME".to_string(),
                    auto_increment: false,
                    primary_key: false,
                    default_value: Some("CURRENT_TIMESTAMP".into()),
                    nullable: Default::default(),
                },
            ],
            indexes: vec![],
        };
        db.create_table(&req).await.unwrap();
        db.create_index(
            table_name,
            &CreateIndexReq {
                name: index_name.to_owned(),
                columns: vec![CreateIndexColumn {
                    col: "name".to_string(),
                    asc: Some(true),
                }],
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
    async fn test_export_table() {
        let db = make_test_object().await.lock().await;

        let table_name = "Persons";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();

        db.execute_sql(
            query_id,
            r#"
            -- Create the table.
            CREATE TABLE Persons (
                ID INTEGER PRIMARY KEY,
                Name TEXT,
                Age INTEGER
            );
            CREATE INDEX persons_idx_age ON Persons (Age);
            -- Insert test data.
            INSERT INTO Persons (Name, Age) VALUES ('John', 25);
            INSERT INTO Persons (Name, Age) VALUES ('Jane', 30);
            INSERT INTO Persons (Name, Age) VALUES ('Mike', 35);
            "#,
        )
        .await
        .unwrap()
        .unwrap();

        GetTestExportTableKinds::test_export(db.deref(), table_name).await;
    }

    #[tokio::test]
    async fn test_list_table() {
        let db = make_test_object().await.lock().await;
        // db.list_table()
        //     .await
        //     .unwrap()
        //     .into_iter()
        //     .for_each(|x| println!("{:?}", x));

        db.list_view()
            .await
            .unwrap()
            .into_iter()
            .for_each(|x| println!("{:?}", x));
    }
    #[tokio::test]
    async fn test_all_types() {
        let db = make_test_object().await.lock().await;
        let table_name = "test_table";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();

        db.execute_sql(
            query_id,
            r#"
            CREATE table test_table (
                id INTEGER PRIMARY KEY,
                null_col NULL,
                text_col TEXT,
                real_col REAL,
                blob_col BLOB,
                int_col INTEGER,
                numeric_col NUMERIC,
                datetime_col datetime,
                time_col time,
                date_col date
            );
            CREATE INDEX idx ON test_table (real_col , int_col );
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let mut values: HashMap<String, Option<String>> = HashMap::new();

        values.insert("id".to_string(), Some("1".to_string()));
        values.insert("text_col".to_string(), Some("Some text".to_string()));
        values.insert("real_col".to_string(), Some("3.14".to_string()));
        values.insert(
            "blob_col".to_string(),
            Some("X'0123456789ABCDEF'".to_string()),
        );
        values.insert("int_col".to_string(), Some("42".to_string()));
        values.insert("numeric_col".to_string(), Some("123.45".to_string()));
        values.insert(
            "datetime_col".to_string(),
            Some("2024-01-01 12:00:00".to_string()),
        );
        values.insert("time_col".to_string(), Some("12:00:00".to_string()));
        values.insert("date_col".to_string(), Some("2024-01-01".to_string()));

        db.table_insert(table_name, &values).await.unwrap();
        let mut wheres = HashMap::new();
        wheres.insert("id".to_string(), Some(1.to_string()));

        let r = db.table_update(table_name, &values, &wheres).await.unwrap();

        println!("{:?}", r);

        let cols = db
            .show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"))
            .await
            .unwrap();
        println!("{:?}", cols);

        let indexes = db.show_indexes(table_name).await.unwrap();
        println!("indexes{:?}", indexes);
        let id = "";
        db.open_connection(id).await.unwrap();

        let view_name = "test_view";

        let _ = db.drop_view(view_name).await;
        {
            db.query_sql(
                id,
                r#"
            SELECT * from test_table;
            PRAGMA table_info('test_table')

        "#,
                Arc::new(TestQueryEventEmit::default()),
            )
            .await
            .unwrap();
        }
        db.execute_sql(
            query_id,
            format!("create view {view_name} as SELECT * from {table_name}").as_str(),
        )
        .await
        .unwrap()
        .unwrap();

        println!("\n after here are view test.\n");
        db.test_table_export_and_import(table_name).await;
        db.test_table_equal(table_name, &values, &["id"]).await;

        let views = db.list_view().await.unwrap();
        for v in views.iter() {
            println!("view:{:?}", v);
            let list = db
                .show_view_collect(
                    v.name.as_str(),
                    ShowTableParameter::new_stupid_test("id asc"),
                )
                .await
                .unwrap();
            println!("view value:{:?}", list);
        }
    }

    #[tokio::test]

    async fn test_ssl() {
        // I don't know if libsql support https
    }
}
