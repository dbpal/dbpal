use std::{
    borrow::Cow, collections::HashMap, future::Future, ops::Deref, sync::Arc, time::Duration,
};

use bb8::ManageConnection;

use dashmap::DashMap;
use libsql::{Connection, Row, ValueType, params};

use crate::{
    datasource_config::{LibSqlDBConfig, LibSqlDBConfigSouce},
    db::{
        BackendName, CheckConn, ColumnType, CreateTableAndIndexSql, ExportKind, Field, Index,
        IndexKind, ListTable, NamedStaff, QueryResp, QuerySql, QuoteStyle, ReadOnlyTrait,
        SchemaTrait, ServerMetas, ShowExtraArgs, SqlBuilder, SqlExecuteRet, TableListResp,
        TriggerDetail,
    },
    helper::{create_csv, is_dangerous_sql_by_parser},
    meta_value::MetaValue,
    query_log::QueryLog,
    sqlite::{
        encode_bytes, extract_auto_increment_key, extract_trigger_detail_from_sql, gen_show_sql,
        integer_like, numreic_like, real_like, text_like,
    },
};
use std::ops::DerefMut;
#[derive(Clone)]
pub struct LibSqlDB {
    config: LibSqlDBConfig,
    pool: bb8::Pool<PoolManager>,
    query_connections: Arc<DashMap<uuid::Uuid, Connection>>,
}

impl BackendName for LibSqlDB {
    fn backend_name(&self) -> &str {
        "libsql"
    }
}

fn get_row_types(r: &Row) -> Vec<Field> {
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

impl QuerySql for LibSqlDB {
    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        is_dangerous_sql_by_parser(sql, &sqlparser::dialect::SQLiteDialect {})
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
    async fn execute_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        let mut query_log = QueryLog::get_guard(query_id, sql);

        let conn = self.query_connections.get(&query_id);
        let conn = match &conn {
            Some(x) => x.deref(),
            None => anyhow::bail!("collection closed????"),
        };

        let ret = match conn.execute(sql, params!()).await {
            Ok(x) => Ok(QueryResp::row_affected(x)),
            Err(err) => Err(err.to_string()),
        };
        query_log.set_success(ret.is_ok());
        anyhow::Ok(ret)
    }

    async fn query_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        let mut query_log = QueryLog::get_guard(query_id, sql);

        let conn = self.query_connections.get(&query_id);
        let conn = match &conn {
            Some(x) => x.deref(),
            None => anyhow::bail!("collection closed????"),
        };

        let mut batch_rows = conn.execute_batch(sql).await?;
        let mut ret = Vec::new();

        while let Some(mut rows_) = batch_rows.next_stmt_row().flatten() {
            let mut rows = Vec::new();
            while let Some(row) = rows_.next().await? {
                rows.push(row);
            }
            if rows.len() > 0 {
                let fields = get_row_types(rows.get(0).unwrap());
                let data = encode_rows(&rows)?;
                ret.push(Ok(QueryResp { fields, data }));
            }
        }
        query_log.set_success(ret.iter().all(|x| x.is_ok()));

        anyhow::Ok(ret)
    }
    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        let conn = self.connect().await?;
        let id = uuid::Uuid::new_v4();
        self.query_connections.insert(id, conn);
        anyhow::Ok(id)
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        let conn = self.connect().await?;
        self.query_connections.insert(id, conn);
        anyhow::Ok(())
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.query_connections.remove(&id);
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn QuerySql> {
        Box::new(Clone::clone(self))
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
    type Error = libsql::Error;

    fn connect(&self) -> impl Future<Output = Result<Self::Connection, Self::Error>> + Send {
        async {
            let replica = self.config.replica;
            let conn = match &self.config.source {
                LibSqlDBConfigSouce::Local { path } => {
                    let b = if replica {
                        libsql::Builder::new_local_replica(path).build().await?
                    } else {
                        libsql::Builder::new_local(path).build().await?
                    };
                    let conn = b.connect()?;
                    conn
                }
                LibSqlDBConfigSouce::Remote {
                    url,
                    auth_token,
                    path,
                } => {
                    let conn = if replica {
                        libsql::Builder::new_remote_replica(path, url.clone(), auth_token.clone())
                            .build()
                            .await?
                            .connect()?
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
    }

    fn is_valid(
        &self, conn: &mut Self::Connection,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async {
            conn.query("select 1", params![]).await?;
            Ok(())
        }
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
        let pool = bb8::Pool::builder().max_size(15).build(manager).await?;
        anyhow::Ok(Self {
            config,
            pool,
            query_connections: Default::default(),
        })
    }
    pub(crate) async fn get_conn(&self) -> anyhow::Result<bb8::PooledConnection<'_, PoolManager>> {
        let conn = self.pool.get().await?;
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

impl QuoteStyle for LibSqlDB {
    fn quote_style(&self) -> &'static str {
        "\""
    }
}

#[async_trait::async_trait]
impl SchemaTrait for LibSqlDB {
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
        let conn = self.get_conn().await?;
        conn.execute(&sql, params!()).await?;
        anyhow::Ok(())
    }

    async fn drop_index(&self, _table_name: &str, index: &str) -> anyhow::Result<()> {
        let sql = format!(r#"DROP INDEX "{index}";"#);
        log::info!("sql:{}", sql);
        let conn = self.get_conn().await?;
        conn.execute(&sql, params!()).await?;
        anyhow::Ok(())
    }

    async fn add_table_column(
        &self, table: &str, name: &str, col: ColumnType,
    ) -> anyhow::Result<()> {
        let sql = format!(
            r#"ALTER TABLE "{table}" ADD COLUMN "{name}" {} {} {}"#,
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
        let conn = self.get_conn().await?;
        conn.execute(&sql, params!()).await?;
        anyhow::Ok(())
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
        let conn = self.get_conn().await?;
        conn.execute(&sql, params!()).await?;
        anyhow::Ok(())
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let conn = self.get_conn().await?;
        let sql = format!(r#"ALTER TABLE "{old}" RENAME TO "{new}""#);
        log::info!("sql:{}", sql);
        conn.execute(&sql, params!()).await?;
        anyhow::Ok(())
    }

    async fn list_trigger(&self, table_name: &str) -> anyhow::Result<Vec<NamedStaff>> {
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
        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
    }

    async fn show_trigger(&self, _table: &str, name: &str) -> anyhow::Result<TriggerDetail> {
        let mut conn = self.get_conn().await?;
        query_trigger_detail(conn.deref_mut(), &name).await
    }

    async fn drop_trigger(&self, _table: &str, name: &str) -> anyhow::Result<()> {
        let sql = format!(
            r#"
         DROP TRIGGER "{name}"
        "#
        );
        log::info!("sql:{}", sql);
        let conn = self.get_conn().await?;
        conn.execute(&sql, params!()).await?;
        anyhow::Ok(())
    }

    async fn create_table_and_index_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        let table = self.create_table_or_view_sql(table_name, "table").await?;
        let indexes = self.get_table_indexes_sql(table_name).await?;

        anyhow::Ok(CreateTableAndIndexSql {
            create_table: table,
            indexes,
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

    async fn create_view_sql(&self, view_name: &str, _: bool) -> anyhow::Result<String> {
        self.create_table_or_view_sql(view_name, "view").await
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_view(&self) -> anyhow::Result<Vec<NamedStaff>> {
        let conn = self.get_conn().await?;
        let mut rows = conn
            .query(
                r#"
            SELECT name
            FROM sqlite_master
            WHERE type = 'view';
        "#,
                params!(),
            )
            .await?;
        let mut ret = Vec::new();
        while let Some(r) = rows.next().await? {
            let name: String = r.get(0)?;
            ret.push(name);
        }

        anyhow::Ok(ret.into_iter().map(NamedStaff::from).collect())
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.can_write()?;

        let sql = format!(r#"DROP VIEW "{view_name}""#);
        let conn = self.get_conn().await?;
        conn.execute(&sql, params!()).await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        self.can_write()?;
        let s = format!(
            r#"DROP TABLE "{table}" {}"#,
            if cascade { " CASCADE " } else { "" }
        );
        let conn = self.get_conn().await?;
        conn.execute(&s, params!()).await?;

        anyhow::Ok(())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_table_columns(table_name).await?;
        let sql =
            (self as &dyn crate::db::SchemaTrait).delete_sql(None, table_name, &fields, &wheres)?;
        log::info!("sql:{}", sql);
        let conn = self.get_conn().await?;
        let changes = conn.execute(&sql, params!()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(changes),
        })
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let sql = format!(
            r#"
            DELETE FROM "{table_name}";
            DELETE FROM sqlite_sequence WHERE name = '{table_name}';
        "#
        );
        let conn = self.get_conn().await?;
        conn.execute(&sql, params!()).await?;

        anyhow::Ok(())
    }

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_table_columns(table_name).await?;

        let conn = self.get_conn().await?;
        let sql = (self as &dyn crate::db::SchemaTrait)
            .update_sql(None, table_name, &fields, &values, &wheres)?;
        log::info!("sql:{}", sql);
        let changes = conn.execute(&sql, params!()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: Some(changes),
        })
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_table_columns(table_name).await?;
        let sql =
            (self as &dyn crate::db::SchemaTrait).insert_sql(None, table_name, &fields, &values)?;
        log::trace!("insert sql:{}", sql);
        let conn = self.get_conn().await?;
        let changes = conn.execute(&sql, params!()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,

            changes: Some(changes),
        })
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let conn = self.get_conn().await?;
        let  mut  rows = conn.query ("SELECT name FROM sqlite_master WHERE type='table' AND name not in ('sqlite_sequence' , 'sqlite_stat1' , 'sqlite_stat4') ORDER BY name " , params!( )).await?;
        let mut ret = Vec::new();
        while let Some(r) = rows.next().await? {
            let table_name: String = r.get(0)?;
            ret.push(table_name);
        }

        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let increment_key = self.query_auto_increment_key(table_name).await?;
        let increment_key = increment_key.as_ref().map(|x| x.as_str());

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
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<TableListResp> {
        let total = self.query_count(table_name, where_str.clone()).await?;
        let sql = gen_show_sql(table_name, where_str, order_by, page, page_size);
        log::info!("show table sql:{}", sql);
        let conn = self.get_conn().await?;
        let mut query_rows = conn.query(&sql, params!()).await?;
        let mut rows = Vec::with_capacity(page_size);
        while let Some(r) = query_rows.next().await? {
            rows.push(r);
        }
        let data = encode_rows(&rows)?;
        let fields = self.show_table_columns(table_name).await?;
        anyhow::Ok(TableListResp::new(fields, total, data, sql))
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<TableListResp> {
        let conn = self.get_conn().await?;
        let total = self.query_count(view_name, where_str.clone()).await?;
        let sql = gen_show_sql(view_name, where_str, order_by, page, page_size);
        log::info!("show table sql:{}", sql);
        let mut raw_rows = conn.query(&sql, params!()).await?;
        let mut rows = Vec::new();
        while let Some(x) = raw_rows.next().await? {
            rows.push(x);
        }
        if rows.len() == 0 {
            let f = self.show_view_columns(view_name).await?;
            return anyhow::Ok(TableListResp::new(f, 0, vec![], sql));
        }
        let data = encode_rows(&rows)?;
        let fields = match rows.get(0) {
            Some(x) => get_row_types(x),
            None => vec![],
        };
        anyhow::Ok(TableListResp::new(fields, total, data, sql))
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
    fn put_value_in_sql(
        &self, typ: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        let typ = typ.to_uppercase();

        let v = match typ.as_str() {
            "" | "TEXT" | "DATE" | "TIME" | "DATETIME" | "NULL" => match value {
                Some(x) => format!("'{}'", x),
                None => "null".to_string(),
            },
            _ if text_like(typ.as_str()) => match value {
                Some(x) => format!("'{}'", x),
                None => "null".to_string(),
            },
            "REAL" | "INTEGER" | "NUMERIC" | "BOOLEAN" => value.unwrap_or("null").to_string(),
            _ if integer_like(typ.as_str()) || numreic_like(typ.as_str()) || real_like(&typ) => {
                value.unwrap_or("null").to_string()
            }
            "BLOB" => value.unwrap_or("null").to_string(),
            _ => {
                let e = format!("type '{}' not handled", typ);
                log::error!("{}", e);
                anyhow::bail!(e)
            }
        };

        anyhow::Ok(v)
    }
}
impl LibSqlDB {
    async fn create_table_or_view_sql(
        &self, table_name: &str, kind: &str,
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
        &self, table_name: &str, include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            let ss = SchemaTrait::create_table_and_index_sql(self, table_name).await?;
            sender.send(Cow::Owned(ss.create_table.clone())).await?;
            sender.send("\n".into()).await?;
            for index in ss.indexes.into_iter() {
                sender.send(Cow::Owned(index)).await?;
                sender.send("\n".into()).await?;
            }
        }
        let conn = self.get_conn().await?;
        let cols = SchemaTrait::show_table_columns(self, table_name).await?;
        let sql = format!("SELECT * from {}", table_name);
        log::info!("sql:{}", sql);
        let mut rows = conn.query(&sql, params!()).await?;

        while let Some(row) = rows.next().await? {
            let data = encode_rows(&vec![row])?;
            for d in data.into_iter() {
                let mut m = HashMap::new();
                for (d, f) in d.into_iter().zip(cols.iter()) {
                    m.insert(f.field.clone(), d);
                }

                let insert_sql = (self).insert_sql(None, table_name, &cols, &m)?;
                sender.send(Cow::Owned(insert_sql)).await?;
                sender.send("\n".into()).await?;
            }
        }
        anyhow::Ok(())
    }

    async fn export_query_result_as_json_line(
        &self, sql: &str, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn().await?;
        let mut stream = conn.query(sql, params!()).await?;
        while let Some(row) = stream.next().await? {
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
        let sql = format!("SELECT * from {}", table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_json_line(&sql, sender).await
    }

    async fn export_query_result_as_json(
        &self, sql: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn().await?;

        let mut stream = conn.query(&sql, params!()).await?;

        sender.send("[".into()).await?;
        let mut first = true;
        while let Some(row) = stream.next().await? {
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
        let sql = format!("SELECT * from {}", table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_json(sql.as_str(), pretty, sender)
            .await
    }

    async fn export_query_result_as_csv(
        &self, sql: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let conn = self.get_conn().await?;

        let mut stream = conn.query(&sql, params!()).await?;
        let mut first = true;
        while let Some(row) = stream.next().await? {
            if first && include_header {
                let fs = get_row_types(&row);
                let fs = fs
                    .into_iter()
                    .map(|x| x.field)
                    .map(Some)
                    .collect::<Vec<_>>();
                sender
                    .send(Cow::Owned(create_csv(&vec![fs], delimiter)))
                    .await?;
                first = false;
            }
            let data = encode_rows(&vec![row])?;

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
        let sql = format!(r#"SELECT * from "{}""#, table_name);
        log::info!("sql:{}", sql);
        self.export_query_result_as_csv(&sql, include_header, delimiter, sender)
            .await
    }
}

async fn query_trigger_detail(conn: &mut Connection, name: &str) -> anyhow::Result<TriggerDetail> {
    let sql = format!("SELECT sql from sqlite_master WHERE type = 'trigger' and name='{name}'");
    let row = conn.prepare(&sql).await?.query_row(params!()).await?;
    let sql: String = row.get(0)?;
    extract_trigger_detail_from_sql(name, sql)
}

fn encode_rows(rows: &Vec<Row>) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let mut ret = Vec::with_capacity(rows.len());

    for r in rows.iter() {
        let mut values = Vec::with_capacity(r.column_count() as usize);
        for c in 0..r.column_count() {
            let ty = r.column_type(c).unwrap_or(ValueType::Null);
            let v = match ty {
                ValueType::Null => {
                    if let Result::Ok(s) = r.get::<f64>(c) {
                        Some(s.to_string())
                    } else if let Result::Ok(s) = r.get::<i64>(c) {
                        Some(s.to_string())
                    } else if let Result::Ok(s) = r.get::<String>(c) {
                        Some(s)
                    } else if let Result::Ok(s) = r.get::<bool>(c) {
                        Some(s.to_string())
                    } else if let Result::Ok(s) = r.get::<Vec<u8>>(c) {
                        Some(encode_bytes(s.as_slice()))
                    } else {
                        None
                    }
                }
                ValueType::Text => {
                    let v: Option<String> = r.get(c)?;
                    v
                }
                ValueType::Integer => {
                    let v: Option<i64> = r.get(c)?;
                    v.map(|x| x.to_string())
                }
                ValueType::Real => {
                    let v: Option<f64> = r.get(c)?;
                    v.map(|x| format!("{}", x))
                }
                ValueType::Blob => {
                    let v: Option<Vec<u8>> = r.get(c)?;
                    v.map(|x| encode_bytes(x.as_slice()))
                }
            };

            values.push(v);
        }
        ret.push(values);
    }
    anyhow::Ok(ret)
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
                v.map(|x| serde_json::Value::String(x))
                    .unwrap_or(null_value.clone())
            }
            ValueType::Real => {
                let v: Option<f64> = r.get(c)?;
                v.map(|x| serde_json::Value::from(x))
                    .unwrap_or(null_value.clone())
            }
            ValueType::Integer => {
                let v: Option<i64> = r.get(c)?;
                v.map(|x| serde_json::Value::from(x))
                    .unwrap_or(null_value.clone())
            }
            ValueType::Blob => {
                let v: Option<Vec<u8>> = r.get(c)?;
                v.map(|x| serde_json::Value::from(x))
                    .unwrap_or(null_value.clone())
            }
        };
        values.insert(r.column_name(c).unwrap_or_default().to_owned(), v);
    }
    anyhow::Ok(serde_json::Value::Object(values))
}

#[cfg(test)]

// this test no need clean up, we use memory sqlite db.
pub(crate) async fn make_test_object() -> LibSqlDB {
    LibSqlDB::new(LibSqlDBConfig {
        replica: false,
        source: LibSqlDBConfigSouce::Remote {
            url: "http://localhost:8080".to_string(),
            auth_token: "".to_string(),
            path: "".to_string(),
        },
        read_only: false,
    })
    .await
    .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]

    async fn check_conn() {
        let db = make_test_object().await;
        let d = db.check_conn().await.unwrap();
        println!("{:?}", d);
    }

    #[tokio::test]
    async fn test_server_metas() {
        let db = make_test_object().await;
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }
    #[tokio::test]
    async fn test_server_select() {
        let db = make_test_object().await;
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
        let db = make_test_object().await;

        let _ = db.drop_table("test_rename", false).await;
        let _ = db.drop_table("test_rename2", false).await;
        let query_id = db.open_connection().await.unwrap();
        db.query_sql(
            query_id,
            r#"
                CREATE TABLE IF NOT EXISTS test_rename (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,  
                    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
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

        db.show_table("test_duplicate2", None, None, 1, 100, None)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_triggers() {
        // enable_test_log();
        let db = make_test_object().await;
        let _ = db.drop_table("test_create_trigger", false).await;
        let _ = db.drop_table("test_create_trigger_update", false).await;
        let query_id = db.open_connection().await.unwrap();

        db.query_sql(
            query_id,
            r#"
            CREATE TABLE test_create_trigger (
                id INTEGER PRIMARY KEY AUTOINCREMENT, -- 自动递增的ID作为主键
                info TEXT NOT NULL 
            );
        "#,
        )
        .await
        .unwrap();
        db.query_sql(
            query_id,
            r#"
        CREATE TABLE test_create_trigger_update (
            id INTEGER PRIMARY KEY AUTOINCREMENT, -- 自动递增的ID作为主键
            info TEXT NOT NULL
        );
    "#,
        )
        .await
        .unwrap();

        db.query_sql(
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
        let db = make_test_object().await;
        let table_name = "test_index";
        let index_name = "name_index";
        let _ = db.drop_index(table_name, index_name).await;
        let _err = db.drop_table(table_name, false).await;
        if let Err(err) = _err {
            println!("drop table failed,err:{}", err.to_string());
        }
        let conn = db.get_conn().await.unwrap();
        conn.execute(
            r#"
                CREATE TABLE test_index (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,  -- 主键，自动增长
                    name TEXT NOT NULL,                    -- 名称，非j空
                    age INTEGER,                           -- 年龄
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP  -- 创建时间，默认为当前时间
                ); "#,
            params!(),
        )
        .await
        .unwrap();

        db.create_index(
            table_name,
            index_name,
            IndexKind::Unique,
            &["name".to_string()],
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
        let db = make_test_object().await;

        let table_name = "Persons";
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();

        db.query_sql(
            query_id,
            r#"
        -- 创建表
        CREATE TABLE Persons (
            ID INTEGER PRIMARY KEY,
            Name TEXT,
            Age INTEGER
        );
        -- 插入测试数据
        INSERT INTO Persons (Name, Age) VALUES ('John', 25);
        INSERT INTO Persons (Name, Age) VALUES ('Jane', 30);
        INSERT INTO Persons (Name, Age) VALUES ('Mike', 35);
        "#,
        )
        .await
        .unwrap();
        let db = db;
        {
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", s);
        }

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
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", s);
        }
        {
            let s = db
                .export_table_to_string(table_name, ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json_line:{}", s);
        }
        println!("########################################");

        {
            let s = db
                .export_query_result_to_string(
                    "select * from persons",
                    ExportKind::Json { pretty: true },
                )
                .await
                .unwrap();
            println!("json:{}", s);
            let _: serde_json::Value = serde_json::from_str(s.as_str()).unwrap();
        }
        {
            let s = db
                .export_query_result_to_string(
                    "select * from persons",
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", s);
        }
        {
            let s = db
                .export_query_result_to_string("select * from persons", ExportKind::JsonLine)
                .await
                .unwrap();
            println!("json_line:{}", s);
        }
    }

    #[tokio::test]
    async fn test_querylog() {
        let db = make_test_object().await;
        let query_id = db.open_connection().await.unwrap();
        db.query_sql(query_id, "select 1").await.unwrap();
        let event = db.get_query_log(query_id).await.unwrap();
        println!("{:?}", event);
        assert_eq!(event.len(), 1);
    }

    #[tokio::test]
    async fn test_all_types() {
        // enable_test_log();
        let db = make_test_object().await;
        let table_name = "test_table";
        let _ = db.drop_table(table_name, false).await;
        let query_id = db.open_connection().await.unwrap();

        db.query_sql(
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
        .unwrap();

        let db = &db as &dyn crate::db::SchemaTrait;
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

        db.insert(table_name, values.clone()).await.unwrap();
        let mut wheres = HashMap::new();
        wheres.insert("id".to_string(), Some(1.to_string()));

        let r = db.update(table_name, values.clone(), wheres).await.unwrap();

        println!("{:?}", r);

        let cols = db
            .show_table(table_name, None, None, 1, 100, None)
            .await
            .unwrap();
        println!("{:?}", cols);

        let indexes = db.show_indexes(table_name).await.unwrap();
        println!("indexes{:?}", indexes);
        let id = db.open_connection().await.unwrap();
        let _ = db.drop_view("test_view").await;
        {
            let r = db
                .query_sql(
                    id.clone(),
                    r#"
            SELECT * from test_table;
            PRAGMA table_info('test_table')

        "#,
                )
                .await
                .unwrap();

            for (index, x) in r.iter().enumerate() {
                println!("result {} r:{:?}", index, x);
            }
        }
        db.query_sql(
            query_id,
            format!("create view test_view as SELECT * from {table_name}").as_str(),
        )
        .await
        .unwrap();
        println!("\nafetr here are view test.\n");

        let views = db.list_view().await.unwrap();
        for v in views.iter() {
            let list = db
                .show_view(&v.name.as_str(), None, None, 1, 100, None)
                .await
                .unwrap();
            println!("view value:{:?}", list);
        }
    }
}
