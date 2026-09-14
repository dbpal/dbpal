use crate::helper::{is_csv_file, uuid_v4_string};
use crate::language_server::LanguageServer;
use calamine::Reader;
use chrono::{DateTime, FixedOffset};
use core::str;
use std::any::Any;
use std::collections::HashMap;
use std::collections::HashSet;
use std::fmt::{self, Debug};
use std::io::BufRead;
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::task::spawn_blocking;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackendKind {
    Mysql(BackendKindMysql),
    LibSql,
    PostGres(BackendKindPostGres),
    SqlServer,
    Mongo,
    Redis,
    Sqlite,
    Cassandra,
    ClickHouse,
    Firebird,
    Oracle,
    DuckDB,
    BigQuery,
    Db2,
    CouchBase,
}

impl BackendKind {
    pub(crate) fn case_rule(self) -> sql_insight::CaseRule {
        match self {
            BackendKind::Mysql(_) => {
                // MySQL: 在Unix上区分大小写，在Windows上不区分
                // 使用 Sensitive 以保留精确匹配，避免数据正确性问题
                sql_insight::CaseRule::Sensitive
            }
            BackendKind::LibSql => {
                // LibSQL: 继承SQLite行为，不区分大小写
                sql_insight::CaseRule::Insensitive
            }
            BackendKind::PostGres(_) => {
                // PostgreSQL: 未加引号转为小写，加引号保留
                sql_insight::CaseRule::Lower
            }
            BackendKind::SqlServer => {
                // SQL Server: 默认不区分大小写（取决于collation）
                // 使用 Insensitive 作为默认行为
                sql_insight::CaseRule::Insensitive
            }
            BackendKind::Mongo => {
                // MongoDB: 集合名区分大小写
                sql_insight::CaseRule::Sensitive
            }
            BackendKind::Redis => {
                // Redis: key区分大小写
                sql_insight::CaseRule::Sensitive
            }
            BackendKind::Sqlite => {
                // SQLite: 默认不区分大小写（取决于collation）
                sql_insight::CaseRule::Insensitive
            }
            BackendKind::Cassandra => {
                // Cassandra: 引用标识符区分大小写，否则转为小写
                sql_insight::CaseRule::Lower
            }
            BackendKind::ClickHouse => {
                // ClickHouse: 默认标识符区分大小写
                sql_insight::CaseRule::Sensitive
            }
            BackendKind::Firebird => {
                // Firebird: 未加引号转为大写
                sql_insight::CaseRule::Upper
            }
            BackendKind::Oracle => {
                // Oracle: 未加引号转为大写
                sql_insight::CaseRule::Upper
            }
            BackendKind::DuckDB => {
                // DuckDB: 保留大小写，但不区分比较
                sql_insight::CaseRule::Insensitive
            }
            BackendKind::BigQuery => {
                // BigQuery: 不区分大小写
                sql_insight::CaseRule::Insensitive
            }
            BackendKind::Db2 => {
                // DB2: 未加引号转为大写
                sql_insight::CaseRule::Upper
            }
            BackendKind::CouchBase => {
                // Couchbase: 区分大小写
                sql_insight::CaseRule::Sensitive
            }
        }
    }
}

impl fmt::Display for BackendKindMysql {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendKindMysql::Mysql => write!(f, "MySQL"),
            BackendKindMysql::Mariadb => write!(f, "MariaDB"),
            BackendKindMysql::Tidb => write!(f, "TiDB"),
        }
    }
}

impl fmt::Display for BackendKindPostGres {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendKindPostGres::PostGres => write!(f, "PostgreSQL"),
            BackendKindPostGres::CockRoach => write!(f, "CockroachDB"),
        }
    }
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendKind::Mysql(mysql) => fmt::Display::fmt(&mysql, f),
            BackendKind::LibSql => write!(f, "LibSql"),
            BackendKind::PostGres(pg) => fmt::Display::fmt(&pg, f),
            BackendKind::SqlServer => write!(f, "SqlServer"),
            BackendKind::Mongo => write!(f, "Mongo"),
            BackendKind::Redis => write!(f, "Redis"),
            BackendKind::Sqlite => write!(f, "Sqlite"),
            BackendKind::Cassandra => write!(f, "Cassandra"),
            BackendKind::ClickHouse => write!(f, "ClickHouse"),
            BackendKind::Firebird => write!(f, "Firebird"),
            BackendKind::Oracle => write!(f, "Oracle"),
            BackendKind::DuckDB => write!(f, "DuckDB"),
            BackendKind::BigQuery => write!(f, "BigQuery"),
            BackendKind::Db2 => write!(f, "Db2"),
            BackendKind::CouchBase => write!(f, "CouchBase"),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackendKindMysql {
    Mysql,
    Mariadb,
    Tidb,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackendKindPostGres {
    PostGres,
    CockRoach,
}

impl BackendKind {
    pub fn comment(self, comment: &str) -> String {
        let syntax = match self {
            Self::Mongo => "//",
            Self::Redis => "#",
            _ => "--",
        };
        comment
            .lines()
            .map(|x| {
                if x.starts_with(syntax) {
                    format!("{}\n", x)
                } else {
                    format!("{} {}\n", syntax, x)
                }
            })
            .collect()
    }
    pub fn mysql_like(self) -> bool {
        matches!(self, Self::Mysql(..))
    }

    // 支持这样的语法 @@或者@访问一些全局或者局部的变量
    pub fn support_at(self) -> bool {
        self.mysql_like() || self == Self::SqlServer
    }
    pub fn postgres_like(self) -> bool {
        matches!(self, Self::PostGres(..))
    }

    pub fn sqlite_like(&self) -> bool {
        matches!(self, Self::Sqlite | Self::LibSql)
    }

    pub fn quote_style(self) -> &'static str {
        if self.mysql_like() || self == Self::ClickHouse {
            "`"
        } else {
            "\""
        }
    }

    pub fn format_fields(&self, fields: Option<&[Field]>) -> String {
        if let Some(fields) = fields {
            fields
                .iter()
                .map(|x| format!("{}{}{}", self.quote_style(), x.field, self.quote_style()))
                .collect::<Vec<_>>()
                .join(",")
        } else {
            "*".to_owned()
        }
    }
}

pub trait GetBackendKind {
    fn backend_kind(&self) -> BackendKind;
}

// those backend 不支持连接支持设置用这个trait辅助实现呢
pub trait ReadOnlyTrait {
    fn read_only(&self) -> bool;
    fn can_write(&self) -> anyhow::Result<()> {
        if self.read_only() {
            return Err(anyhow::Error::msg("this connection is read only."));
        }
        anyhow::Ok(())
    }
}

pub type ExportedSizeEmitter = Arc<dyn Fn(usize) + Send + Sync>;

#[derive(serde::Deserialize, Clone, Debug, specta::Type)]
pub struct ShowTableParameter {
    #[serde(default)]
    pub where_str: Option<String>,
    #[serde(default)]
    pub order_by: Option<String>,
    #[serde(default)]
    pub page: u32,
    #[serde(default)]
    pub page_size: u32,
    // for backend don't support page like cassandra
    #[serde(default)]
    pub limit: Option<u32>,
    // right now only used for redis zset
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub allow_filtering: bool,

    // couchbase can use this parameter to format
    #[serde(default)]
    pub tab_size: u32,
}

impl Default for ShowTableParameter {
    fn default() -> Self {
        Self {
            where_str: None,
            order_by: None,
            page: 1,
            page_size: 100,
            limit: None,
            min: None,
            max: None,
            allow_filtering: true,
            tab_size: 4,
        }
    }
}

#[derive(serde::Deserialize, Clone, Debug, specta::Type)]
pub struct TableImportPara {
    pub path: String,
    #[serde(default)]
    pub mongo_data: Option<Vec<JsUint8Array>>,
    #[serde(default)]
    pub csv_delimiter: Option<char>,
}

impl ShowTableParameter {
    pub fn offset(&self) -> u32 {
        (self.page - 1) * self.page_size
    }
}

impl ShowTableParameter {
    pub fn new_stupid_test<'a>(order: impl Into<Option<&'a str>>) -> Self {
        let order = order.into();
        Self {
            where_str: Some("1 = 1".to_owned()),
            order_by: order.map(|x| x.to_string()),
            page: 1,
            page_size: 100,
            ..Default::default()
        }
    }
}

#[derive(Debug, Default, Clone)]
struct QueryRespMemoryCollector {
    ret: Arc<Mutex<Option<QueryResp>>>,
}

impl QueryRespMemoryCollector {
    fn result(&self) -> QueryResp {
        self.ret
            .lock()
            .expect("lock failed")
            .clone()
            .expect("should have result")
    }

    fn option_result(&self) -> Option<QueryResp> {
        self.ret.lock().expect("lock failed").clone()
    }
}

impl QueryEventEmit for QueryRespMemoryCollector {
    fn send(&self, event: QueryEvent) -> anyhow::Result<()> {
        let mut lock = self.ret.lock().expect("lock failed");
        match event {
            QueryEvent::Resp(resp) => *lock = Some(resp),
            QueryEvent::Data(data) => {
                use std::ops::DerefMut;
                let x = lock.deref_mut();
                let x = x.as_mut().expect("the structure must be there");
                x.data.extend(data);
            }
            QueryEvent::Status(s) => {
                println!("status:{}", s);
            }
            QueryEvent::Err(err) => println!("err:{}", err),
            QueryEvent::Row(data) => {
                use std::ops::DerefMut;
                let x = lock.deref_mut();
                let x = x.as_mut().expect("must emit structure");
                x.data.push(data);
            }
        };
        anyhow::Ok(())
    }
}

pub(crate) struct DropConn {
    pub(crate) db: Option<Box<dyn QuerySql>>,
    pub(crate) query_id: String,
    pub(crate) try_rollback: TryRollBack,
}

#[derive(Clone, Debug)]
pub(crate) struct TryRollBack {
    rollback: Arc<Mutex<bool>>,
}

impl TryRollBack {
    pub(crate) fn new() -> Self {
        Self {
            rollback: Arc::new(Mutex::new(true)),
        }
    }
    // not rollback when operation success call this function.
    pub(crate) fn do_not(&self) {
        *self.rollback.lock().expect("lock failed") = false;
    }
    pub(crate) fn get(&self) -> bool {
        *self.rollback.lock().expect("lock failed")
    }
}

impl Drop for DropConn {
    fn drop(&mut self) {
        let db = self.db.take();

        let rollback = self.try_rollback.get();

        if let Some(db) = db {
            let query_id = self.query_id.clone();
            tokio::spawn(async move {
                if db.support_tx() && rollback {
                    if let Err(err) = db.rollback(&query_id).await {
                        log::error!("rollback failed:{}", err);
                    }
                }
                if let Err(err) = db.close_connection(&query_id).await {
                    log::error!("close connection failed:{}", err);
                }
            });
        }
    }
}

/// representation of schema
#[async_trait::async_trait]
pub trait SchemaTrait:
    Sync
    + Send
    + QuerySql
    + CheckConn
    + std::fmt::Debug
    + ServerMetas
    + 'static
    + ExportQueryResult
    + SqlBuilder
    + ExtractSqlValeFromJsonValue
    + EncodeXlsxDatetime
{
    async fn list_foreign_key(&self, _table: &str) -> anyhow::Result<Vec<ForeignKeyDef>> {
        self.backend_kind().not_support_feature("list foreign key")
    }
    async fn drop_foreign_key(&self, _table: &str, _name: &str) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature("drop foreign key")
    }

    async fn create_table(&self, req: &CreateTableReq) -> anyhow::Result<String> {
        let ss = SchemaTrait::create_table_sql(self, req)?;
        let mut sc = SqlCollect::default();

        for sql in ss.iter() {
            self.simple_execute_sql(sql).await?;
            sc.collect(sql);
        }
        anyhow::Ok(sc.into())
    }
    fn create_table_sql(&self, req: &CreateTableReq) -> anyhow::Result<Vec<String>> {
        SqlBuilder::create_table_sql(self, req)
    }

    fn create_table_sql_string(&self, req: &CreateTableReq) -> anyhow::Result<String> {
        SchemaTrait::create_table_sql(self, req).map(|x| x.join(";\n"))
    }

    fn as_any(&self) -> &dyn std::any::Any;

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<Option<String>> {
        self.duplicate_table(old, new).await?;
        self.drop_table(old, false).await?;
        anyhow::Ok(None)
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()>;
    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>>;
    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>>;
    async fn show_table(
        &self,
        table_name: &str,
        _: ShowTableParameter,
        _: QueryEventEmitter,
    ) -> anyhow::Result<()>;

    async fn show_table_collect(
        &self,
        table_name: &str,
        para: ShowTableParameter,
    ) -> anyhow::Result<QueryResp> {
        let result = QueryRespMemoryCollector::default();
        let result_clone = result.clone();
        self.show_table(table_name, para, Arc::new(result_clone))
            .await?;
        anyhow::Ok(result.result())
    }

    async fn show_view_collect(
        &self,
        table_name: &str,
        para: ShowTableParameter,
    ) -> anyhow::Result<QueryResp> {
        let result = QueryRespMemoryCollector::default();
        let result_clone = result.clone();
        self.show_view(table_name, para, Arc::new(result_clone))
            .await?;
        anyhow::Ok(result.result())
    }

    async fn table_insert_sql(
        &self,
        table_name: &str,
        values: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = SqlBuilder::insert_sql(self, table_name, &fs, values)?;
        anyhow::Ok(sql)
    }

    async fn table_insert(
        &self,
        table_name: &str,
        values: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        let sql = self.table_insert_sql(table_name, values).await?;
        let r = self.simple_execute_sql(&sql).await?;
        anyhow::Ok(r)
    }

    async fn table_update_sql(
        &self,
        table_name: &str,
        values: &HashMap<String, Option<String>>,
        wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = SqlBuilder::update_sql(self, table_name, &fs, values, wheres)?;
        anyhow::Ok(sql)
    }

    async fn table_update(
        &self,
        table_name: &str,
        values: &HashMap<String, Option<String>>,
        wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        let sql = self.table_update_sql(table_name, values, wheres).await?;
        self.simple_execute_sql(&sql).await
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<Option<String>>;

    async fn table_delete_sql(
        &self,
        table_name: &str,
        wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        let fs = self.show_table_columns(table_name).await?;
        SqlBuilder::delete_sql(self, table_name, &fs, wheres)
    }

    async fn table_delete(
        &self,
        table_name: &str,
        wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = SqlBuilder::delete_sql(self, table_name, &fs, wheres)?;
        let ret = self.simple_execute_sql(&sql).await?;
        anyhow::Ok(ret)
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.backend_kind().not_support_feature("list view")
    }
    async fn drop_view(&self, _view_name: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("drop view")
    }
    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<Option<String>>;

    fn clone(&self) -> Box<dyn SchemaTrait>;

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>>;

    async fn drop_table_column(&self, _table: &str, _col: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("drop table column")
    }
    async fn modify_table_column(
        &self,
        _table: &str,
        _old_name: &str,
        _new_name: &str,
        _typ: Option<ColumnType>,
    ) -> anyhow::Result<String> {
        self.backend_kind()
            .not_support_feature("modify table column")
    }

    async fn table_import_data_from_file_make_data(
        &self,
        table: &str,
        para: TableImportPara,
        sender: tokio::sync::mpsc::Sender<HashMap<String, Option<String>>>,
    ) -> anyhow::Result<()> {
        let cols = self.show_table_columns(table).await?;
        let mut cols_map = HashMap::new();
        cols.iter().for_each(|x| {
            cols_map.insert(x.field.clone(), x.clone());
        });
        let this = SchemaTrait::clone(self);
        spawn_blocking(move || -> anyhow::Result<()> {
            let handle_json_map = |map: &serde_json::Map<String, serde_json::Value>| {
                let mut values = HashMap::new();
                for (k, v) in map.iter() {
                    let field = match cols_map.get(k) {
                        Some(x) => x,
                        None => {
                            continue;
                        }
                    };
                    values.insert(
                        k.clone(),
                        this.extrat_sql_value_from_json_value(v, &field.data_type)?,
                    );
                }
                sender.blocking_send(values)?;

                anyhow::Ok(())
            };

            let path_lower = para.path.to_lowercase();

            match para {
                _ if is_csv_file(&path_lower) => {
                    let mut rb = csv::ReaderBuilder::new();
                    if path_lower.ends_with(".tsv") {
                        rb.delimiter(b'\t');
                    } else if let Some(x) = para.csv_delimiter {
                        rb.delimiter(x as u8);
                    }
                    let mut reader: csv::Reader<std::fs::File> = rb.from_path(&para.path)?;
                    let headers: Vec<_> = reader.headers()?.iter().map(|x| x.to_string()).collect();

                    for csv_line in reader.records() {
                        let csv_line = csv_line?;
                        let mut values = HashMap::new();
                        for (index, h) in headers.iter().enumerate() {
                            values.insert(
                                h.clone(),
                                csv_line.get(index).and_then(|x| {
                                    if x.is_empty() {
                                        None
                                    } else {
                                        Some(x.to_owned())
                                    }
                                }),
                            );
                        }
                        sender.blocking_send(values)?;
                    }
                }
                _ if path_lower.ends_with(".json") => {
                    let f = std::fs::File::open(para.path)?;
                    let values: Vec<serde_json::Map<String, serde_json::Value>> =
                        serde_json::from_reader(f)?;
                    for value in values.iter() {
                        handle_json_map(value)?;
                    }
                }
                _ if path_lower.ends_with(".jsonl") => {
                    //
                    let f = std::fs::File::open(&para.path)?;
                    let reader = std::io::BufReader::new(f);
                    for line in reader.lines() {
                        let line = line?;
                        if line.trim().is_empty() {
                            // skip empty line
                            continue;
                        }
                        let map: serde_json::Map<String, serde_json::Value> =
                            serde_json::from_str(&line)?;
                        handle_json_map(&map)?;
                    }
                }
                _ if crate::helper::is_excel_file(&path_lower) => {
                    let mut wb: calamine::Xlsx<std::io::BufReader<std::fs::File>> =
                        calamine::open_workbook(&para.path)?;
                    if wb.sheet_names().is_empty() {
                        anyhow::bail!("this contains no sheets")
                    }
                    let range = wb.worksheet_range(&wb.sheet_names()[0])?;
                    let mut headers = vec![];

                    for (row_idx, row) in range.rows().enumerate() {
                        if row_idx == 0 {
                            headers = row.iter().map(|x| x.to_string()).collect();
                            continue;
                        }
                        let mut values = HashMap::new();
                        for (index, data) in row.iter().enumerate() {
                            let f = headers.get(index).and_then(|name| cols_map.get(name));
                            if let Some(f) = f {
                                // trying to format date , datetime

                                let value = match data {
                                    calamine::Data::DateTime(x) => {
                                        let x = EncodeXlsxDatetime::encode(
                                            this.as_ref(),
                                            &f.data_type,
                                            x,
                                        )?;
                                        x
                                    }
                                    _ => {
                                        let v = data.to_string();
                                        if v.is_empty() {
                                            None
                                        } else {
                                            Some(v)
                                        }
                                    }
                                };

                                values.insert(f.field.clone(), value);
                            }
                        }
                        sender.blocking_send(values)?;
                    }
                }
                _ => {
                    anyhow::bail!("unsupported file type,only support csv,json,jsonl,xlsx etc.")
                }
            }
            anyhow::Ok(())
        })
        .await?
    }

    async fn table_import_data_from_file(
        &self,
        table: &str,
        para: TableImportPara,
    ) -> anyhow::Result<()> {
        if !self.table_exists(table).await? {
            anyhow::bail!(format!("table '{}' not exists", table))
        }
        let (sender, mut receiver) =
            tokio::sync::mpsc::channel::<HashMap<String, Option<String>>>(10);

        let cols = self.show_table_columns(table).await?;
        let this_make = SchemaTrait::clone(self);
        // read data from the file
        let task_read = this_make.table_import_data_from_file_make_data(table, para, sender);
        let this = SchemaTrait::clone(self);
        let table = table.to_owned();

        let task_write = tokio::spawn(async move {
            // use tx to commit the data

            let query_id = uuid_v4_string();
            this.open_connection(&query_id).await?;
            let try_rollback = TryRollBack::new();

            let _guard = DropConn {
                db: Some(SchemaTrait::clone(this.as_ref())),
                query_id: query_id.clone(),
                try_rollback: try_rollback.clone(),
            };
            if this.support_tx() {
                this.set_auto_commit(&query_id, false).await?;
            }
            while let Some(record) = receiver.recv().await {
                // build the sql
                let sql = SqlBuilder::insert_sql(this.as_ref(), &table, &cols, &record)?;
                log::info!("insert sql:{}", sql);
                this.execute_sql(&query_id, &sql)
                    .await?
                    .map_err(anyhow::Error::msg)?;
            }
            if this.support_tx() {
                this.commit(&query_id).await?;
            }
            try_rollback.do_not();
            anyhow::Ok(())
        });
        let (res1, res2) = tokio::join!(task_read, task_write);

        res2??;
        res1
    }

    async fn table_import_data_from_file_insert_samples(
        &self,
        table: &str,
        para: TableImportPara,
        sample_count: usize,
    ) -> anyhow::Result<Vec<String>> {
        if !self.table_exists(table).await? {
            anyhow::bail!(format!("table '{}' not exists", table))
        }
        let (sender, mut receiver) =
            tokio::sync::mpsc::channel::<HashMap<String, Option<String>>>(10);

        let cols = self.show_table_columns(table).await?;
        let this_make = SchemaTrait::clone(self);
        // read data from the file
        let task_read = this_make.table_import_data_from_file_make_data(table, para, sender);
        let this = SchemaTrait::clone(self);
        let table = table.to_owned();

        let task_write = tokio::spawn(async move {
            let mut ret = Vec::with_capacity(sample_count);
            while let Some(record) = receiver.recv().await {
                let sql = SqlBuilder::insert_sql(this.as_ref(), &table, &cols, &record)?;
                ret.push(sql);
                if ret.len() >= sample_count {
                    break;
                }
            }
            anyhow::Ok(ret)
        });
        let (res1, res2) = tokio::join!(task_read, task_write);
        let _err = res1;
        if let Err(_err) = _err {
            log::error!("this channel may closed this is fine to me,err:{}", _err);
        }

        res2?
    }

    async fn add_table_column(
        &self,
        _table: &str,
        _name: &str,
        _col: ColumnType,
    ) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("add table column")
    }

    async fn rename_index(
        &self,
        _table_name: &str,
        _oldname: &str,
        _newname: &str,
    ) -> anyhow::Result<Option<String>> {
        self.backend_kind().not_support_feature("rename index")
    }

    async fn show_indexes(&self, _table_name: &str) -> anyhow::Result<Vec<Index>> {
        self.backend_kind().not_support_feature("show indexes")
    }

    async fn create_index(&self, _table: &str, _: &CreateIndexReq) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("create index")
    }

    async fn drop_index(&self, _table_name: &str, _index: &str) -> anyhow::Result<Option<String>> {
        self.backend_kind().not_support_feature("drop index")
    }

    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.backend_kind()
            .not_support_feature("list materialized view")
    }

    async fn drop_materialized_view(&self, _name: &str) -> anyhow::Result<Option<String>> {
        self.backend_kind()
            .not_support_feature("drop materialized view")
    }

    async fn export_table_or_view(
        &self,
        _name: &str,
        _: Option<ShowTableParameter>,
        _kind: ExportKind,
        _sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        self.backend_kind()
            .not_support_feature("export table or view")
    }

    async fn show_view(
        &self,
        _view_name: &str,
        _para: ShowTableParameter,
        _: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature("show view")
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        // 有些后端没有数据 也能得到完整的结果所以提供一个默认实现
        // 后端基于sqlx的需要使用prepare_with这种特殊的接口
        let result = QueryRespMemoryCollector::default();
        let result_clone = result.clone();

        self.show_view(
            view_name,
            ShowTableParameter {
                page: 1,
                page_size: 1,
                ..Default::default()
            },
            Arc::new(result_clone),
        )
        .await?;
        let r = result.option_result();
        anyhow::Ok(r.unwrap_or_default().fields)
    }

    /// export as sql only support table so the frontend should only not pass this kind
    async fn export_table_or_view_to_file(
        &self,
        table_name: &str,
        para: Option<ShowTableParameter>,
        filepath: &str,
        kind: ExportKind,
        emit: ExportedSizeEmitter,
    ) -> anyhow::Result<()> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        let h1 = {
            let x = SchemaTrait::clone(self);
            let kind = kind.clone();
            let table_name = table_name.to_string();
            let h1 = tokio::spawn(async move {
                x.export_table_or_view(&table_name, para, kind, sender)
                    .await
            });
            h1
        };
        let h2 = {
            let filepath = filepath.to_string();
            let table_name = table_name.to_string();
            tokio::spawn(async move {
                write_export_file(kind, &filepath, receiver, &table_name, emit).await
            })
        };
        h1.await??;
        h2.await??;
        anyhow::Ok(())
    }

    async fn show_create_table_sql(
        &self,
        _table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        self.backend_kind().not_support_feature("show create sql")
    }

    async fn show_view_sql(&self, _view_name: &str, _materialized: bool) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("show view sql")
    }

    async fn list_trigger(&self, _table_name: &str) -> anyhow::Result<Vec<NamedStaff>> {
        self.backend_kind().not_support_feature("list trigger")
    }
    async fn show_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<TriggerDetail> {
        self.backend_kind().not_support_feature("show trigger")
    }
    async fn drop_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("drop trigger")
    }

    async fn enable_trigger(
        &self,
        _table: &str,
        _name: &str,
        _enable: bool,
    ) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("enable trigger")
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.backend_kind().not_support_feature("list udf")
    }

    async fn show_udf(&self, _name: &NamedStaff) -> anyhow::Result<DefAndMetas> {
        self.backend_kind().not_support_feature("show udf")
    }

    async fn drop_udf(&self, _name: &NamedStaff) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("drop udf")
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.backend_kind().not_support_feature("list procedure")
    }
    async fn show_procedure(&self, _name: &NamedStaff) -> anyhow::Result<ProcedureDetail> {
        self.backend_kind().not_support_feature("show procedure")
    }
    async fn drop_procedure(&self, _name: &NamedStaff) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("drop procedure")
    }

    async fn table_exists(&self, table: &str) -> anyhow::Result<bool> {
        let e = self
            .list_table()
            .await?
            .iter()
            .any(|x| x.name.eq_ignore_ascii_case(table));
        anyhow::Ok(e)
    }
    async fn list_created_types(&self) -> anyhow::Result<Vec<CreatedTypeDef>> {
        self.backend_kind().not_support_feature("list created type")
    }

    async fn drop_created_type(&self, _name: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("drop created type")
    }

    async fn export_table_to_string(
        &self,
        table_name: &str,
        kind: ExportKind,
    ) -> anyhow::Result<String> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        let h1 = {
            let kind = kind.clone();
            let x = SchemaTrait::clone(self);
            let table_name = table_name.to_string();
            let h1 = tokio::spawn(async move {
                x.export_table_or_view(&table_name, None, kind, sender)
                    .await
            });
            h1
        };
        let h2 = { tokio::spawn(async move { collect_export_str(kind, receiver).await }) };
        h1.await??;
        h2.await?
    }

    async fn list_sequence(&self) -> anyhow::Result<Vec<SequenceDetail>> {
        self.backend_kind().not_support_feature("list sequence")
    }
    async fn rename_sequence(&self, _old: &str, _new: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("rename sequence")
    }

    async fn drop_sequence(&self, _name: &str, _cascade: bool) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("drop sequence")
    }
    fn list_sql_current_values(&self) -> &'static [&'static str] {
        &[]
    }
    async fn list_event(&self) -> anyhow::Result<Vec<SqlEvent>> {
        anyhow::Ok(vec![])
    }
    async fn drop_event(&self, _name: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature("drop event")
    }

    #[allow(clippy::unwrap_used)]
    async fn test_table_export_and_import_excel(&self, table: &str) {
        let file = tempfile::NamedTempFile::with_suffix(".xlsx").unwrap();
        let file = file.path().to_str().unwrap();

        // test csv
        let export_kind = ExportKind::Xlsx {
            include_header: true,
            datetime_format: Default::default(),
        };

        self.export_table_or_view_to_file(table, None, file, export_kind.clone(), Arc::new(|_| {}))
            .await
            .unwrap();

        println!("exported excel file:{}", file);

        self.truncate_table(table).await.unwrap();
        self.table_import_data_from_file(
            table,
            TableImportPara {
                path: file.to_string(),
                mongo_data: None,
                csv_delimiter: None,
            },
        )
        .await
        .unwrap();
    }

    #[allow(clippy::unwrap_used)]
    async fn test_table_export_and_import_csv(&self, table: &str) {
        use std::io::Write;

        let mut file = tempfile::NamedTempFile::with_suffix(".csv").unwrap();
        let d = ';';

        // test csv
        let export_kind = ExportKind::Csv {
            include_header: true,
            delimiter: d,
        };

        let csv_str = self
            .export_table_to_string(table, export_kind.clone())
            .await
            .unwrap();
        file.write_all(csv_str.as_bytes()).unwrap();

        println!("csv str:{}", csv_str);
        self.truncate_table(table).await.unwrap();
        self.table_import_data_from_file(
            table,
            TableImportPara {
                path: file.path().to_str().unwrap().to_string(),
                mongo_data: None,
                csv_delimiter: Some(d),
            },
        )
        .await
        .unwrap();
        let csv_str2 = self
            .export_table_to_string(table, export_kind.clone())
            .await
            .unwrap();
        {
            println!("csv1:{}", csv_str);
            println!("csv2:{}", csv_str2);
        }
    }
    #[allow(clippy::unwrap_used)]
    async fn test_table_export_and_import_json(&self, table: &str) {
        use std::io::Write;

        let mut file = tempfile::NamedTempFile::with_suffix(".json").unwrap();
        // test json
        let json_str = self
            .export_table_to_string(table, ExportKind::Json { pretty: true })
            .await
            .unwrap();

        println!("json str:{}", json_str);

        file.write_all(json_str.as_bytes()).unwrap();

        self.truncate_table(table).await.unwrap();
        self.table_import_data_from_file(
            table,
            TableImportPara {
                path: file.path().to_str().unwrap().to_string(),
                mongo_data: None,
                csv_delimiter: None,
            },
        )
        .await
        .unwrap();

        let json_str2 = self
            .export_table_to_string(table, ExportKind::Json { pretty: true })
            .await
            .unwrap();
        {
            println!("json1:{}", json_str);
            println!("json2:{}", json_str2);
        }
    }
    #[allow(clippy::unwrap_used)]
    async fn test_table_export_and_import_jsonline(&self, table: &str) {
        use std::io::Write;

        let mut file = tempfile::NamedTempFile::with_suffix(".jsonl").unwrap();
        let export_kind = ExportKind::JsonLine;

        let jsonline_str = self
            .export_table_to_string(table, export_kind.clone())
            .await
            .unwrap();
        file.write_all(jsonline_str.as_bytes()).unwrap();

        self.truncate_table(table).await.unwrap();
        self.table_import_data_from_file(
            table,
            TableImportPara {
                path: file.path().to_str().unwrap().to_string(),
                mongo_data: None,
                csv_delimiter: None,
            },
        )
        .await
        .unwrap();
        let jsonline_str2 = self
            .export_table_to_string(table, export_kind.clone())
            .await
            .unwrap();
        {
            println!("jsonline1:{}", jsonline_str);
            println!("jsonline2:{}", jsonline_str2);
        }
    }
    #[allow(clippy::unwrap_used)]
    async fn test_table_export_and_import(&self, table: &str) {
        self.test_table_export_and_import_csv(table).await;
        let para = ShowTableParameter::default();

        let x = self.show_table_collect(table, para.clone()).await.unwrap();
        log::info!("after csv import:{:?}", x.data);

        self.test_table_export_and_import_json(table).await;
        let x = self.show_table_collect(table, para.clone()).await.unwrap();
        log::info!("after json import:{:?}", x.data);
        self.test_table_export_and_import_jsonline(table).await;
        let x = self.show_table_collect(table, para.clone()).await.unwrap();
        log::info!("after jsonline import:{:?}", x.data);
        self.test_table_export_and_import_excel(table).await;
        let x = self.show_table_collect(table, para.clone()).await.unwrap();
        log::info!("after excel import:{:?}", x.data);
        log::info!("test export and import OK.")
    }
    #[allow(clippy::unwrap_used)]
    async fn test_table_equal(
        &self,
        table: &str,
        expected: &HashMap<String, Option<String>>,
        skip_fields: &[&'static str],
    ) {
        let skip_fields: HashSet<&'static str> = skip_fields.iter().copied().collect();

        let expected = {
            let values: HashMap<_, _> = expected
                .iter()
                .filter(|(k, _)| !skip_fields.contains(&k.as_str()))
                .collect();
            values
        };
        let r = self
            .show_table_collect(
                table,
                ShowTableParameter {
                    page_size: 1,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let values = r
            .fields
            .iter()
            .zip(r.data.first().unwrap())
            .map(|(f, v)| (f.field.clone(), v.clone()))
            .filter(|(k, _)| !skip_fields.contains(k.as_str()))
            .collect::<HashMap<_, _>>();

        for (k, v) in expected.iter() {
            let v_e = v.as_ref().map(|s| s.as_str()).unwrap();
            let v_a = values
                .get(*k)
                .unwrap()
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or_else(|| panic!("field: {} not found in actual result", *k));

            if v_e != v_a {
                println!("expect:{:?}", v_e.as_bytes());
                println!("actual:{:?}", v_a.as_bytes());
                panic!("field: {} expect: |{}| actual: |{}|", *k, v_e, v_a);
            }
        }
    }
}

#[derive(Debug, serde::Serialize, specta::Type)]
pub struct SqlEvent {
    pub name: String,
    pub sql: String,
    pub metas: Vec<MetaValue>,
}
pub trait SqlBuilder: GetBackendKind {
    fn get_current_values(&self) -> &'static [&'static str] {
        &[]
    }

    fn gen_show_sql(
        &self,
        table_name: &str,
        fields: Option<&[Field]>,
        para: Option<&ShowTableParameter>,
    ) -> String;

    fn hit_current_value(&self, value: &str) -> bool {
        self.get_current_values()
            .iter()
            .any(|cur| cur.eq_ignore_ascii_case(value))
    }

    fn put_value_in_sql_hit_predefine(&self, typ: &str, value: &str) -> anyhow::Result<String> {
        if self.hit_current_value(value) {
            return anyhow::Ok(value.to_string());
        }
        self.put_value_in_sql(typ, value)
    }

    fn type_support_compare(&self, _typ: &str) -> bool {
        true
    }

    fn get_schema(&self) -> Option<&str>;
    fn quote(&self, name: &str) -> String {
        format!(
            "{}{}{}",
            self.backend_kind().quote_style(),
            name,
            self.backend_kind().quote_style()
        )
    }

    /// format a value make it suitable for insert into sql.
    /// this method current use in `insert_sql` etc.
    fn put_value_in_sql(&self, _typ: &str, _value: &str) -> anyhow::Result<String>;

    fn make_where(
        &self,
        fs: &[Field],
        wheres: &HashMap<String, Option<String>>,
        field_by_name: &HashMap<&String, &Field>,
    ) -> anyhow::Result<String> {
        // if contains primary key should use primary key to delete
        // otherwise should use all value
        // to do operation.
        let contains_pri = fs.iter().any(|x| x.is_primary);
        // check all primary key have value

        let mut sql = String::new();

        let first = FirstTrue::default();

        for (k, v) in wheres.iter() {
            let f = field_by_name.get(k);
            let f = f.copied();

            let f = match f {
                Some(x) => x,
                None => continue,
            };
            // some type can't compare
            // so they can't appear in where condition
            if !self.type_support_compare(f.data_type.as_str()) {
                continue;
            }
            if contains_pri && !f.is_primary {
                continue;
            }

            match v {
                Some(value) => {
                    sql.push_str(
                        format!(
                            "{} {} = {}",
                            if !first.get() { " AND " } else { "" },
                            self.quote(k),
                            self.put_value_in_sql(&f.data_type, value)?
                        )
                        .as_str(),
                    );
                }
                None => {
                    sql.push_str(
                        format!(
                            "{} {} IS NULL",
                            if !first.get() { " AND " } else { "" },
                            self.quote(k)
                        )
                        .as_str(),
                    );
                }
            }
        }
        anyhow::Ok(sql)
    }

    fn delete_sql(
        &self,
        table_name: &str,
        fs: &[Field],
        wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        let schema = self.get_schema();
        let mut field_by_name = HashMap::new();
        for f in fs.iter() {
            field_by_name.insert(&f.field, f);
        }
        #[allow(clippy::format_in_format_args)]
        let mut sql = format!(
            "DELETE FROM {}{} WHERE ",
            if let Some(schema) = schema {
                format!("{}.", self.quote(schema))
            } else {
                "".to_string()
            },
            self.quote(table_name)
        );
        sql.push_str(self.make_where(fs, wheres, &field_by_name)?.as_str());
        log::info!("delete sql:{}", sql);
        anyhow::Ok(sql)
    }

    fn update_sql(
        &self,
        table_name: &str,
        fs: &[Field],
        values: &HashMap<String, Option<String>>,
        wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        //
        let schema = self.get_schema();
        let mut field_by_name = HashMap::new();
        for f in fs.iter() {
            field_by_name.insert(&f.field, f);
        }
        #[allow(clippy::format_in_format_args)]
        let mut sql = format!(
            "UPDATE {}{} SET ",
            if let Some(schema) = schema {
                format!("{}.", self.quote(schema))
            } else {
                "".to_string()
            },
            self.quote(table_name)
        );
        {
            let last = values.len() - 1;
            for (index, (k, v)) in values.iter().enumerate() {
                let f = field_by_name.get(k);
                let f = match f {
                    Some(f) => *f,
                    None => continue,
                };
                match v {
                    Some(value) => {
                        sql.push_str(
                            format!(
                                "{} = {}",
                                self.quote(k),
                                self.put_value_in_sql(&f.data_type, value)?
                            )
                            .as_str(),
                        );
                    }
                    None => {
                        sql.push_str(format!("{} = NULL", self.quote(k)).as_str());
                    }
                }

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

    fn insert_sql(
        &self,
        table_name: &str,
        fs: &[Field],
        values: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        println!("insert_sql values:{:?}", values);
        let schema = self.get_schema();
        #[allow(clippy::format_in_format_args)]
        let mut sql = format!(
            "INSERT INTO {}{}(",
            if let Some(schema) = schema.as_ref() {
                format!("{}.", self.quote(schema))
            } else {
                "".to_string()
            },
            self.quote(table_name)
        );
        let skip_null_default_value = |f: &Field| {
            values.get(&f.field).map(|x| x.is_none()).unwrap_or(true) && f.default_value.is_some()
        };
        //

        {
            let first = FirstTrue::default();

            for f in fs.iter() {
                if f.is_auto_increment {
                    continue;
                }
                if skip_null_default_value(f) {
                    // value is null and has default value
                    continue;
                }
                if !first.get() {
                    sql.push(',');
                }
                sql.push_str(self.quote(f.field.as_str()).to_string().as_str());
            }
            sql.push(')');
        }
        // values
        sql.push_str(" VALUES (");
        {
            let first = FirstTrue::default();

            for f in fs.iter() {
                if f.is_auto_increment {
                    continue;
                }
                if skip_null_default_value(f) {
                    // value is null and has default value
                    continue;
                }
                if !first.get() {
                    sql.push(',');
                }
                let value = values.get(&f.field).cloned().flatten();

                match &value {
                    Some(value) => {
                        sql.push_str(self.put_value_in_sql(&f.data_type, value)?.as_str())
                    }
                    None => {
                        log::info!("field '{}' is null", f.field);
                        sql.push_str("NULL")
                    }
                };
            }
            sql.push(')');
        }
        anyhow::Ok(sql)
    }

    // stupid create table sql auto increment must be primary key.
    // and  not primary key must can be in `PRIMARY KEY(aaa , bbb )` syntax
    // this method should be able used most cases
    fn create_table_sql(&self, req: &CreateTableReq) -> anyhow::Result<Vec<String>> {
        let mut sql = format!(
            "CREATE TABLE {}{}(\n",
            if let Some(s) = self.get_schema() {
                format!("{}.", self.quote(s))
            } else {
                "".to_owned()
            },
            self.quote(&req.name)
        );
        let last = req.columns.len() - 1;
        let has_auto_increment = req.has_auto_increment();
        let pks = if has_auto_increment {
            None
        } else {
            req.format_primary_keys(self.backend_kind().quote_style())
        };
        for (index, col) in req.columns.iter().enumerate() {
            if col.auto_increment && col.default_value.is_some() {
                anyhow::bail!("auto increment column can not have default value.");
            }
            let name = self.quote(&col.name);
            let data_type = &col.data_type;
            let nullable = match self.backend_kind() {
                BackendKind::Cassandra => match col.nullable {
                    SqlNullAble::Unspecified => "".to_owned(),
                    SqlNullAble::Null | SqlNullAble::NotNull => {
                        anyhow::bail!("cassandra not support null constraint")
                    }
                },
                _ => col.nullable.to_string(),
            };
            let auto_increment: &str = if col.auto_increment {
                match self.backend_kind() {
                    _ if self.backend_kind().sqlite_like() => " PRIMARY KEY AUTOINCREMENT ",
                    _ if self.backend_kind().mysql_like() => " PRIMARY KEY AUTO_INCREMENT ",
                    _ => {
                        anyhow::bail!(format!(
                            "auto increment not handled for {}",
                            self.backend_kind()
                        ))
                    }
                }
            } else {
                ""
            };
            let dv = match &col.default_value {
                Some(value) => match self.backend_kind() {
                    BackendKind::Cassandra => {
                        anyhow::bail!("cassandra not support default value")
                    }
                    BackendKind::Firebird => {
                        // silently differrent from other database
                        format!(
                            "DEFAULT {}",
                            self.put_value_in_sql_hit_predefine(&col.data_type, value.as_str())?
                        )
                    }
                    _ => format!(
                        "DEFAULT({})",
                        self.put_value_in_sql_hit_predefine(&col.data_type, value.as_str())?
                    ),
                },
                None => "".to_owned(),
            };

            match self.backend_kind() {
                BackendKind::Firebird => {
                    // somehow firebird need put nullable after default value
                    sql.push_str(
                        format!("{name} {data_type} {dv} {nullable}  {auto_increment}").as_str(),
                    );
                }
                _ => {
                    sql.push_str(
                        format!("{name} {data_type} {nullable} {dv} {auto_increment}").as_str(),
                    );
                }
            }

            if index != last || pks.is_some() {
                sql.push(',');
            }
            sql.push('\n');
        }
        if let Some(pks) = pks {
            sql.push_str(format!("PRIMARY KEY ({})", pks).as_str());
            sql.push('\n');
        }
        sql.push(')');
        let mut ret = vec![sql];
        for index in req.indexes.iter() {
            ret.push(index.common_sql(&req.name, true, self.backend_kind().quote_style())?);
        }
        anyhow::Ok(ret)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CreatedTypeDef {
    pub name: String,
    pub sql: Option<String>,
}

impl std::fmt::Display for CreatedTypeDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "type:{} def:{}",
            self.name,
            self.sql.as_ref().unwrap_or(&"".to_string())
        )
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct ColumnType {
    // type string like `varchar`
    pub typ: String,
    pub nullable: Option<SqlNullAble>,
    // mysql add column can have this field
    pub after: Option<String>,
    pub default_value: Option<String>,
}

impl BackendKind {
    pub fn not_support_feature<T>(&self, feature: &str) -> anyhow::Result<T> {
        anyhow::bail!(format!(
            "'{}' don't support this '{}' feature",
            self, feature
        ))
    }
}

#[derive(Debug, serde::Serialize, specta::Type)]
pub struct ProcedureDetail {
    // definition of the sql.
    pub def: String,
    pub metas: Vec<MetaValue>,
}
impl ProcedureDetail {
    pub fn new(def: String, metas: Vec<MetaValue>) -> Self {
        Self { metas, def }
    }
}

#[derive(Debug, serde::Serialize, specta::Type)]
pub struct DefAndMetas {
    // definition of the sql.
    pub def: String,
    pub metas: Vec<MetaValue>,
}

impl DefAndMetas {
    pub(crate) fn new(def: String, metas: Vec<MetaValue>) -> Self {
        Self { metas, def }
    }
}

#[async_trait::async_trait]
pub trait CatalogTrait:
    Send
    + Sync
    + 'static
    + QuerySql
    + CheckConn
    + LanguageServer
    + Debug
    + ServerMetas
    + ExportQueryResult
{
    async fn list_database(&self) -> anyhow::Result<Vec<String>>;
    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>>;
    async fn create_database(
        &self,
        name: &str,
        extra: Option<CreateDatabaseExtra>,
    ) -> anyhow::Result<()>;
    async fn drop_database(&self, name: &str) -> anyhow::Result<()>;

    fn as_any(&self) -> &dyn Any;
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, specta::Type)]
#[serde(tag = "extra", rename_all = "lowercase")]
pub enum CreateDatabaseExtra {
    Couchbase {
        // 在 Couchbase 中，ram_quota_mb 是一个重要的配置参数，用于指定一个桶（bucket）分配的内存大小。这个参数的单位是兆字节（MB），它决定了该桶可以使用的内存量。
        ram_quota_mb: u32,
        // 是否启用 flush 操作。启用 flush 操作可以清空桶中的所有数据。
        flush_enabled: bool,
        // 副本数量，即每个文档的副本数。副本数量必须小于或等于集群中的节点数。
        num_replicas: u32,
        // in seconds
        max_expiry: Option<u32>,
        bucket_type: CouchBaseBucketType,
    },
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum CouchBaseBucketType {
    Couchbase,
    Ephemeral,
}

/// named staff maybe used for view udf etc...
#[derive(Debug, serde::Serialize, specta::Type, serde::Deserialize, Default)]
pub struct NamedStaff {
    // some backend support reload function like postgres
    pub parameters: Option<String>,
    // cassandra support reload and parameter is a `Vec`
    pub cassandra_parameters: Option<Vec<String>>,
    pub name: String,
    pub metas: Vec<MetaValue>,
}

impl From<String> for NamedStaff {
    fn from(value: String) -> Self {
        NamedStaff {
            name: value,
            metas: vec![],
            parameters: None,
            cassandra_parameters: None,
        }
    }
}
#[async_trait::async_trait]
pub trait CheckConn: Send + Sync + 'static {
    /// Duration is basic `select 1` execution time.
    async fn check_conn(&self) -> anyhow::Result<Duration>;
}
/// create table and all index sql.
#[derive(Clone, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct CreateTableAndIndexSql {
    pub create_table: String,
    /// some backend can query create table and index in one sql
    /// this can be empty
    pub indexes: Vec<String>,
}

impl std::fmt::Display for CreateTableAndIndexSql {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut sc = SqlCollect::default();
        sc.collect(&self.create_table);
        for index in self.indexes.iter() {
            sc.collect(index);
        }
        write!(f, "{}", String::from(sc))
    }
}

#[async_trait::async_trait]
pub trait DBTrait:
    Sync + Send + QuerySql + 'static + CheckConn + Debug + ServerMetas + ExportQueryResult
{
    async fn list_schema(&self) -> anyhow::Result<Vec<String>>;
    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>>;
    async fn create_schema(
        &self,
        name: &str,
        extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()>;
    async fn drop_schema(&self, name: &str) -> anyhow::Result<()>;
    fn as_any(&self) -> &dyn std::any::Any;
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(tag = "extra", rename_all = "lowercase")]
pub enum CreateSchemaExtra {
    Cassandra {
        // CREATE KEYSPACE my_keyspace
        // WITH REPLICATION = {
        //     'class': 'SimpleStrategy',
        //     'replication_factor': 3
        // };
        // class is the keyword for frontend
        replication_strategy: String,
        replication_factor: u32,
    },
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, specta::Type)]
pub struct SequenceDetail {
    pub start_value: String,
    pub min_value: String,
    pub max_value: String,
    pub increment_by: String,
    pub last_value: Option<String>,
    pub name: String,
    pub sql: String,
    pub data_type: String,
}

#[derive(Clone, Debug)]
pub struct XlsxEdit {
    pub row: rust_xlsxwriter::RowNum,
    pub column: rust_xlsxwriter::ColNum,
    pub content: XlsxEditContent,
}
pub trait EncodeXlsxDatetime: GetBackendKind {
    // return a option data is because of https://docs.rs/calamine/latest/src/calamine/datatype.rs.html#791-806
    //  returns a option datetime
    fn encode(&self, ty: &str, date: &calamine::ExcelDateTime) -> anyhow::Result<Option<String>> {
        let date = date.as_datetime();
        log::trace!("encode excel datetime type ty:{} date:{:?}", ty, date);

        if self.is_datatime(ty)? {
            return anyhow::Ok(if let Some(x) = date {
                self.format_datetime(x).into()
            } else {
                None
            });
        }
        if self.is_time(ty)? {
            return anyhow::Ok(if let Some(x) = date {
                self.format_time(x.time()).into()
            } else {
                None
            });
        }
        if self.is_date(ty)? {
            return anyhow::Ok(if let Some(x) = date {
                self.format_date(x.date()).into()
            } else {
                None
            });
        }
        anyhow::bail!(format!("can't convert datetime to '{}'", ty))
    }

    fn is_datatime(&self, _ty: &str) -> anyhow::Result<bool> {
        self.backend_kind()
            .not_support_feature("check if a type is datetime")
    }

    fn is_date(&self, _ty: &str) -> anyhow::Result<bool> {
        self.backend_kind()
            .not_support_feature("check if a type is date")
    }
    fn is_time(&self, _ty: &str) -> anyhow::Result<bool> {
        self.backend_kind()
            .not_support_feature("check if a type is time")
    }

    fn format_datetime(&self, x: chrono::NaiveDateTime) -> String {
        x.to_string()
    }

    fn format_time(&self, x: chrono::NaiveTime) -> String {
        x.to_string()
    }

    fn format_date(&self, x: chrono::NaiveDate) -> String {
        x.to_string()
    }
}

#[derive(Clone, Debug)]
pub enum XlsxEditContent {
    Time(chrono::NaiveTime),
    Date(chrono::NaiveDate),
    DateTime(chrono::NaiveDateTime),
    String(String),
    Number(f64),
    Bool(bool),
}

impl From<i64> for XlsxEditContent {
    fn from(value: i64) -> Self {
        Self::number(value as f64)
    }
}
impl From<u64> for XlsxEditContent {
    fn from(value: u64) -> Self {
        Self::number(value as f64)
    }
}

impl From<String> for XlsxEditContent {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<chrono::NaiveTime> for XlsxEditContent {
    fn from(value: chrono::NaiveTime) -> Self {
        Self::Time(value)
    }
}

impl From<chrono::NaiveDate> for XlsxEditContent {
    fn from(value: chrono::NaiveDate) -> Self {
        Self::Date(value)
    }
}
impl From<chrono::DateTime<chrono::Utc>> for XlsxEditContent {
    fn from(value: chrono::DateTime<chrono::Utc>) -> Self {
        Self::DateTime(value.naive_utc())
    }
}

impl From<chrono::NaiveDateTime> for XlsxEditContent {
    fn from(value: chrono::NaiveDateTime) -> Self {
        Self::DateTime(value)
    }
}

impl XlsxEditContent {
    pub fn number<T: Into<f64>>(value: T) -> Self {
        Self::Number(value.into())
    }
}

impl From<bool> for XlsxEditContent {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl std::fmt::Display for XlsxEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}-{}", self.row, self.column, self.content)
    }
}

impl std::fmt::Display for XlsxEditContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XlsxEditContent::Time(naive_time) => write!(f, "time:{}", naive_time),
            XlsxEditContent::Date(naive_date) => write!(f, "date:{}", naive_date),
            XlsxEditContent::DateTime(naive_date_time) => {
                write!(f, "datetime:{}", naive_date_time)
            }
            XlsxEditContent::String(x) => write!(f, "str:{}", x),
            XlsxEditContent::Number(x) => write!(f, "num:{}", x),
            XlsxEditContent::Bool(x) => write!(f, "bool:{}", x),
        }
    }
}

pub(crate) async fn export_query_as_xlsx_send_data(
    row_num: &mut u32,
    data: Vec<Option<XlsxEditContent>>,
    sender: tokio::sync::mpsc::Sender<ExportFrame>,
) -> anyhow::Result<()> {
    for (col, d) in data.into_iter().enumerate() {
        if let Some(d) = d {
            sender
                .send(
                    XlsxEdit {
                        row: *row_num,
                        column: col as rust_xlsxwriter::ColNum,
                        content: d,
                    }
                    .into(),
                )
                .await?;
        }
    }
    *row_num += 1;
    anyhow::Ok(())
}

pub fn export_query_as_xlsx_send_blocking_data(
    row_num: &mut u32,
    data: Vec<Option<XlsxEditContent>>,
    sender: tokio::sync::mpsc::Sender<ExportFrame>,
) -> anyhow::Result<()> {
    for (col, d) in data.into_iter().enumerate() {
        if let Some(d) = d {
            sender.blocking_send(
                XlsxEdit {
                    row: *row_num,
                    column: col as rust_xlsxwriter::ColNum,
                    content: d,
                }
                .into(),
            )?;
        }
    }
    *row_num += 1;
    anyhow::Ok(())
}

#[derive(Debug, Clone)]
pub enum ExportFrame {
    Str(&'static str),
    String(String),
    Xlsx(XlsxEdit),
    Bytes(tokio_util::bytes::Bytes),
    JsonValue(serde_json::Value),
}

impl From<serde_json::Value> for ExportFrame {
    fn from(value: serde_json::Value) -> Self {
        Self::JsonValue(value)
    }
}

impl From<serde_json::Map<String, serde_json::Value>> for ExportFrame {
    fn from(value: serde_json::Map<String, serde_json::Value>) -> Self {
        Self::JsonValue(serde_json::Value::Object(value))
    }
}

impl From<tokio_util::bytes::Bytes> for ExportFrame {
    fn from(value: tokio_util::bytes::Bytes) -> Self {
        Self::Bytes(value)
    }
}

impl std::fmt::Display for ExportFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportFrame::Str(x) => write!(f, "{}", x),
            ExportFrame::String(s) => write!(f, "{}", s),
            ExportFrame::Xlsx(xlsx_edit) => write!(f, "{}", xlsx_edit),
            ExportFrame::Bytes(bytes) => {
                write!(f, "{}", String::from_utf8_lossy(bytes.deref()))
            }
            ExportFrame::JsonValue(value) => write!(f, "{:?}", value),
        }
    }
}

impl ExportFrame {
    pub(crate) fn as_str(&self) -> anyhow::Result<&str> {
        let r = match self {
            ExportFrame::Str(x) => *x,
            ExportFrame::String(s) => s.as_str(),
            ExportFrame::Xlsx(..) => {
                anyhow::bail!("this is xlsx edit,should be a internal error")
            }
            ExportFrame::Bytes(..) => {
                anyhow::bail!("this is bytes,should be a internal error")
            }
            ExportFrame::JsonValue(..) => {
                anyhow::bail!("this is json value ,should be internal error")
            }
        };
        anyhow::Ok(r)
    }

    pub(crate) fn as_bytes(&self) -> anyhow::Result<&[u8]> {
        let r = match self {
            ExportFrame::Str(x) => x.as_bytes(),
            ExportFrame::String(s) => s.as_bytes(),
            ExportFrame::Xlsx(..) => {
                anyhow::bail!("this is xlsx edit,should be a internal error")
            }
            ExportFrame::Bytes(bytes) => bytes,
            ExportFrame::JsonValue(..) => {
                anyhow::bail!("this is json value,should be a internal error")
            }
        };
        anyhow::Ok(r)
    }
}

impl From<&'static str> for ExportFrame {
    fn from(value: &'static str) -> Self {
        ExportFrame::Str(value)
    }
}

impl From<String> for ExportFrame {
    fn from(value: String) -> Self {
        ExportFrame::String(value)
    }
}

impl From<XlsxEdit> for ExportFrame {
    fn from(value: XlsxEdit) -> Self {
        ExportFrame::Xlsx(value)
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]

pub struct CreateTableReq {
    pub name: String,
    pub columns: Vec<CreateTableReqColumn>,
    pub indexes: Vec<CreateIndexReq>,
}

impl CreateTableReq {
    // format primary keys to `aaa,bbb`
    pub(crate) fn format_primary_keys(&self, quote_str: &str) -> Option<String> {
        let cols = self
            .columns
            .iter()
            .filter(|x| x.primary_key)
            .map(|x| format!("{}{}{}", quote_str, x.name.clone(), quote_str))
            .collect::<Vec<_>>();
        if cols.is_empty() {
            None
        } else {
            Some(cols.join(","))
        }
    }

    pub(crate) fn has_auto_increment(&self) -> bool {
        self.columns.iter().any(|x| x.auto_increment)
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default, specta::Type)]
pub struct CreateTableReqColumn {
    pub name: String,
    pub data_type: String,
    pub auto_increment: bool,
    pub primary_key: bool,
    pub default_value: Option<String>,
    pub nullable: SqlNullAble,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, Default, Copy, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum SqlNullAble {
    #[default]
    Unspecified,
    Null,
    NotNull,
}

impl std::fmt::Display for SqlNullAble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                SqlNullAble::Unspecified => "",
                SqlNullAble::Null => "NULL",
                SqlNullAble::NotNull => "NOT NULL",
            }
        )
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]
pub struct CreateIndexReq {
    pub name: String,
    pub columns: Vec<CreateIndexColumn>,
    pub kind: IndexKind,
}

impl CreateIndexReq {
    pub(crate) fn format_cols(&self, can_have_order: bool, quote_str: &str) -> String {
        self.columns
            .iter()
            .map(|col| match (can_have_order, col.asc) {
                (true, Some(order)) => {
                    format!(
                        "{} {}",
                        format_args!("{}{}{}", quote_str, col.col, quote_str),
                        if order { "asc" } else { "desc" }
                    )
                }
                _ => col.col.clone(),
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    pub fn common_sql(
        &self,
        table: &str,
        can_have_order: bool,
        quote_str: &str,
    ) -> anyhow::Result<String> {
        let sql = format!(
            "CREATE {} INDEX {} ON {table}({})",
            match self.kind {
                IndexKind::Index => "",
                IndexKind::Primary => anyhow::bail!("can't create primary index"),
                IndexKind::Unique => "Unique",
            },
            self.name,
            self.format_cols(can_have_order, quote_str)
        );
        anyhow::Ok(sql)
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, specta::Type)]

pub struct CreateIndexColumn {
    pub col: String,
    pub asc: Option<bool>,
}

async fn collect_export_str(
    kind: ExportKind,
    mut receiver: tokio::sync::mpsc::Receiver<ExportFrame>,
) -> anyhow::Result<String> {
    match kind {
        ExportKind::Json { pretty } => {
            let mut ret = "[".to_string();
            let first = FirstTrue::default();

            while let Some(r) = receiver.recv().await {
                let value = match r {
                    ExportFrame::JsonValue(x) => x,
                    _ => {
                        unreachable!()
                    }
                };
                if !first.get() {
                    ret.push(',');
                }
                if pretty {
                    ret.push('\n');
                }
                ret.push_str(
                    if pretty {
                        serde_json::to_string_pretty(&value)
                    } else {
                        serde_json::to_string(&value)
                    }?
                    .as_str(),
                );
            }
            if pretty {
                ret.push('\n');
            }
            ret.push(']');
            anyhow::Ok(ret)
        }
        ExportKind::JsonLine => {
            let mut ret = String::new();
            while let Some(x) = receiver.recv().await {
                match x {
                    ExportFrame::Bytes(x) => {
                        ret.push_str(&String::from_utf8_lossy(&x));
                    }
                    ExportFrame::JsonValue(x) => {
                        let x = serde_json::to_string(&x)?;
                        ret.push_str(&x);
                    }
                    _ => unreachable!("kind:{:?} data:{:?}", kind, x),
                };

                ret.push('\n');
            }

            anyhow::Ok(ret)
        }
        ExportKind::Xlsx { .. } => {
            let mut ret = String::new();
            let mut last: Option<XlsxEdit> = None;
            while let Some(r) = receiver.recv().await {
                // check something

                match &r {
                    ExportFrame::Xlsx(data) => {
                        if let Some(last) = &last {
                            assert!(
                                (last.row < data.row) || (last.column < data.column),
                                "lat{:?} this:{}",
                                last,
                                data
                            );
                        }
                        last = Some(data.clone());
                    }

                    _ => {
                        unreachable!()
                    }
                }

                ret.push_str(r.to_string().as_str());
                ret.push('\n');
            }
            anyhow::Ok(ret)
        }
        _ => {
            let mut ret = String::new();
            while let Some(r) = receiver.recv().await {
                ret.push_str(r.to_string().as_str());
                ret.push('\n');
            }
            anyhow::Ok(ret)
        }
    }
}
#[async_trait::async_trait]
pub trait ExportQueryResult: GetBackendKind + Send + Sync + 'static {
    fn clone(&self) -> Box<dyn ExportQueryResult>;

    async fn export_query_result(
        &self,
        query_id: Option<&str>,
        _sql: &str,
        _kind: ExportKind,
        _sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()>;

    async fn export_query_result_collect(
        &self,
        query_id: Option<&str>,
        sql: &str,
        kind: ExportKind,
        max: usize,
    ) -> anyhow::Result<(bool, Vec<ExportFrame>)> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
        let j1 = {
            let x = self.clone();
            let sql = sql.to_string();
            let query_id = query_id.map(|x| x.to_owned());
            tokio::spawn(async move {
                x.export_query_result(query_id.as_deref(), &sql, kind, sender)
                    .await
            })
        };
        let j2 = {
            let mut exceed = false;
            tokio::spawn(async move {
                let mut ret = vec![];
                while let Some(x) = receiver.recv().await {
                    ret.push(x);
                    if ret.len() > max {
                        exceed = true;
                        break;
                    }
                }

                anyhow::Ok((exceed, ret))
            })
        };
        if let Err(err) = j1.await? {
            log::error!(
                "query failed. if channel is close this should be fine:{}",
                err
            );
        }
        j2.await?
    }

    async fn export_query_result_to_string(
        &self,
        query_id: &str,
        sql: &str,
        kind: ExportKind,
    ) -> anyhow::Result<String> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        let j1 = {
            let x = ExportQueryResult::clone(self);
            let sql = sql.to_string();
            let kind = kind.clone();

            let query_id = query_id.to_string();
            let h1 = tokio::spawn(async move {
                x.export_query_result(Some(&query_id), &sql, kind, sender)
                    .await
            });
            h1
        };
        let j2 = { tokio::spawn(async move { collect_export_str(kind, receiver).await }) };
        j1.await??;
        j2.await?
    }
    async fn export_query_result_to_file(
        &self,
        query_id: &str,
        sql: &str,
        kind: ExportKind,
        file: &str,
        emit: ExportedSizeEmitter,
    ) -> anyhow::Result<()> {
        log::info!("sql:{}", sql);
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        let j1 = {
            let x = self.clone();
            let sql = sql.to_string();
            let kind = kind.clone();
            let query_id = query_id.to_string();
            tokio::spawn(async move {
                x.export_query_result(Some(&query_id), &sql, kind, sender)
                    .await
            })
        };
        let j2 = {
            let sql = sql.to_string();
            let filepath = file.to_string();
            tokio::spawn(
                async move { write_export_file(kind, &filepath, receiver, &sql, emit).await },
            )
        };
        j1.await??;
        j2.await??;
        anyhow::Ok(())
    }
}

pub(crate) async fn write_export_file(
    kind: ExportKind,
    filepath: &str,
    mut receiver: tokio::sync::mpsc::Receiver<ExportFrame>,
    title: &str,
    emit: ExportedSizeEmitter,
) -> anyhow::Result<()> {
    let mut time_elapsed = TimeElapsed::default();
    let mut total = 0;
    let r = match kind {
        ExportKind::Json { pretty } => {
            let mut file = tokio::fs::File::create(&filepath).await?;
            file.write_all("[".as_bytes()).await?;
            let first = FirstTrue::default();

            while let Some(e) = receiver.recv().await {
                let value = match e {
                    ExportFrame::JsonValue(x) => x,
                    _ => {
                        anyhow::bail!("not json value but:{:?}", e);
                    }
                };
                if !first.get() {
                    file.write_all(",".as_bytes()).await?;
                }
                if pretty {
                    file.write_all("\n".as_bytes()).await?;
                }
                let value = if pretty {
                    serde_json::to_string_pretty(&value)
                } else {
                    serde_json::to_string(&value)
                }?;
                total += value.len();
                if time_elapsed.elapsed() {
                    emit(total);
                }
                file.write_all(value.as_bytes()).await?;
            }
            if pretty {
                file.write_all("\n".as_bytes()).await?;
            }
            file.write_all("]".as_bytes()).await?;
            emit(total);
            anyhow::Ok(())
        }
        ExportKind::JsonLine => {
            let mut file = tokio::fs::File::create(&filepath).await?;
            while let Some(e) = receiver.recv().await {
                match e {
                    ExportFrame::Bytes(x) => {
                        file.write_all(&x).await?;
                    }
                    ExportFrame::JsonValue(x) => {
                        let value = serde_json::to_string(&x)?;
                        total += value.len();
                        if time_elapsed.elapsed() {
                            emit(total);
                        }
                        file.write_all(value.as_bytes()).await?;
                        file.write_all("\n".as_bytes()).await?;
                    }
                    _ => {
                        anyhow::bail!("wrong export frame data:{:?}", e);
                    }
                }
            }
            emit(total);
            anyhow::Ok(())
        }
        ExportKind::Html { .. } => {
            let mut file = tokio::fs::File::create(&filepath).await?;
            // Add a worksheet to the workbook.
            let template = include_str!("./export_html_template.html");
            {
                let t = "<table>";
                let index = template.find(t).option_must_be_some()?;
                let x = &template[..index + t.len()];
                let x = x.replacen("THIS_IS_TITLE", title, 1);
                file.write_all(x.as_bytes()).await?;
            }

            while let Some(data) = receiver.recv().await {
                file.write_all(data.as_str()?.as_bytes()).await?;
                total += data.as_str()?.len();
                if time_elapsed.elapsed() {
                    emit(total)
                }
            }
            {
                let t = "</table>";
                let index = template.find(t).option_must_be_some()?;
                file.write_all(&template.as_bytes()[index..]).await?;
            }
            emit(total);
            anyhow::Ok(())
        }
        ExportKind::Xlsx {
            include_header,
            datetime_format,
        } => {
            let filepath = filepath.to_string();
            let emit = emit.clone();
            spawn_blocking(move || {
                let mut workbook = rust_xlsxwriter::workbook::Workbook::new();
                // Add a worksheet to the workbook.
                let worksheet = workbook.add_worksheet_with_constant_memory();
                while let Some(edit) = receiver.blocking_recv() {
                    let edit = match edit {
                        ExportFrame::Xlsx(xlsx_edit) => xlsx_edit,
                        _ => {
                            anyhow::bail!(format!("this should be xlsx edit,but:{:?}", edit))
                        }
                    };
                    total += 1;
                    if time_elapsed.elapsed() {
                        emit(total)
                    }
                    if include_header && edit.row == 0 {
                        let header_format = rust_xlsxwriter::Format::new().set_bold();
                        worksheet.write_string_with_format(
                            edit.row,
                            edit.column,
                            match edit.content {
                                XlsxEditContent::String(x) => x,
                                _ => {
                                    anyhow::bail!(format!(
                                        "this is internal error,should be string value but:{:?}",
                                        edit.content
                                    ))
                                }
                            }, // header must  be  a string data
                            &header_format,
                        )?;
                    } else {
                        match edit.content {
                            XlsxEditContent::Time(naive_time) => {
                                let mut f = rust_xlsxwriter::Format::new();
                                f = f.set_num_format(&datetime_format.time);
                                worksheet.write_datetime_with_format(
                                    edit.row,
                                    edit.column,
                                    naive_time,
                                    &f,
                                )?;
                            }
                            XlsxEditContent::Date(naive_date) => {
                                let mut f = rust_xlsxwriter::Format::new();

                                f = f.set_num_format(&datetime_format.date);
                                worksheet.write_date_with_format(
                                    edit.row,
                                    edit.column,
                                    naive_date,
                                    &f,
                                )?;
                            }
                            XlsxEditContent::DateTime(naive_date_time) => {
                                let mut f = rust_xlsxwriter::Format::new();

                                f = f.set_num_format(&datetime_format.datatime);
                                worksheet.write_datetime_with_format(
                                    edit.row,
                                    edit.column,
                                    naive_date_time,
                                    &f,
                                )?;
                            }
                            XlsxEditContent::String(x) => {
                                worksheet.write_string(edit.row, edit.column, x)?;
                            }
                            XlsxEditContent::Number(f) => {
                                worksheet.write_number(edit.row, edit.column, f)?;
                            }
                            XlsxEditContent::Bool(x) => {
                                worksheet.write_boolean(edit.row, edit.column, x)?;
                            }
                        }
                    }
                }
                workbook.save(filepath)?;
                emit(total);
                anyhow::Ok(())
            })
            .await?
        }
        _ => {
            let mut file = tokio::fs::File::create(&filepath).await?;
            while let Some(data) = receiver.recv().await {
                file.write_all(data.as_bytes()?).await?;
                total += data.as_bytes()?.len();
                if time_elapsed.elapsed() {
                    emit(total)
                }
            }
            emit(total);
            anyhow::Ok(())
        }
    };
    r
}

#[async_trait::async_trait]
pub trait QuerySql: GetBackendKind + Send + Sync + 'static + LanguageServer {
    async fn get_auto_commit(&self, _query_id: &str) -> anyhow::Result<bool> {
        self.backend_kind().not_support_feature("transaction")
    }

    fn clone(&self) -> Box<dyn QuerySql>;

    async fn execute_tx_multi_sql(&self, sql: &[String]) -> anyhow::Result<()> {
        let query_id = uuid_v4_string();

        self.open_connection(&query_id).await?;
        let try_rollback = TryRollBack::new();

        let _guard = DropConn {
            query_id: query_id.clone(),
            db: QuerySql::clone(self).into(),
            try_rollback: try_rollback.clone(),
        };
        if self.support_tx() {
            self.set_auto_commit(&query_id, false).await?;
        }

        for sql in sql.iter() {
            self.execute_sql(&query_id, sql.as_str())
                .await?
                .map_err(anyhow::Error::msg)?;
        }
        if self.support_tx() {
            self.commit(&query_id).await?;
        }
        try_rollback.do_not();
        anyhow::Ok(())
    }

    async fn get_update_sql_from_result_set(
        &self,
        sql: &str,
        old_values: &[Option<String>],
        new_values: &[Option<String>],
    ) -> anyhow::Result<Vec<String>>;

    async fn set_auto_commit(&self, _query_id: &str, _commit: bool) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature("transaction")
    }
    async fn commit(&self, _query_id: &str) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature("transaction")
    }
    async fn rollback(&self, _query_id: &str) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature("transaction")
    }

    fn support_tx(&self) -> bool;

    async fn query_sql(
        &self,
        query_id: &str,
        _sql: &str,
        emit: QueryEventEmitter,
    ) -> anyhow::Result<()>;

    async fn execute_sql(
        &self,
        query_id: &str,
        _sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>>;
    async fn simple_execute_sql(&self, sql: &str) -> anyhow::Result<Option<usize>>;

    async fn open_connection(&self, id: &str) -> anyhow::Result<()>;

    async fn close_connection(&self, id: &str) -> anyhow::Result<()>;
}

#[derive(Debug, serde::Serialize, Clone, specta::Type)]
pub struct ListTable {
    pub name: String,
    pub size: Option<f64>,
    // for frontend tree item id
}

impl PartialOrd for ListTable {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ListTable {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for ListTable {}

impl Ord for ListTable {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.name.cmp(&other.name)
    }
}

impl From<String> for ListTable {
    fn from(value: String) -> Self {
        Self {
            name: value,
            size: None,
        }
    }
}

impl ListTable {
    pub(crate) fn new(name: impl Into<String>, size: Option<f64>) -> Self {
        Self {
            name: name.into(),
            size,
        }
    }
}

impl From<&str> for ListTable {
    fn from(value: &str) -> Self {
        Self {
            name: value.to_string(),
            size: None,
        }
    }
}

/// get server metas info
#[async_trait::async_trait]
pub trait ServerMetas: Sync + Send + 'static {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>>;
}

#[derive(Debug, Clone, serde::Serialize, specta::Type)]
pub struct Index {
    pub name: Option<String>,
    pub kind: IndexKind,
    pub columns: Vec<String>,
}

impl Index {
    pub fn new(name: impl Into<Option<String>>, kind: IndexKind, columns: Vec<String>) -> Self {
        let name = name.into();
        Self {
            name,
            kind,
            columns,
        }
    }
}

#[derive(Clone, serde::Serialize, Copy, Debug, serde::Deserialize, specta::Type)]
pub enum IndexKind {
    #[serde(rename = "index")]
    Index,
    #[serde(rename = "unique")]
    Unique,
    #[serde(rename = "primary")]
    Primary,
}

impl TryFrom<&str> for IndexKind {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "primary" => Ok(Self::Primary),
            "normal" => Ok(Self::Index),
            "unique" => Ok(Self::Unique),
            _ => Err(format!(
                "the frontend may pass the wrong parameter of {}",
                value
            )),
        }
    }
}

impl QueryResp {
    pub fn row_affected(affected: u64, sql: impl Into<Option<String>>) -> Self {
        Self {
            data: vec![vec![Some(affected.to_string())]],
            fields: vec![Field {
                field: "row affected".to_string(),
                ..Default::default()
            }],
            sql: sql.into(),
            total: 0f64,
            tables: Default::default(),
        }
    }

    pub fn execute_ok(sql: impl Into<Option<String>>) -> Self {
        Self {
            data: vec![vec![Some("OK".to_string())]],
            fields: vec![Field {
                field: "execute".to_string(),
                ..Default::default()
            }],
            sql: sql.into(),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct QueryResp {
    pub fields: Vec<Field>,
    // only pagination need this parameter.
    pub total: f64,
    pub data: Vec<Vec<Option<String>>>,
    pub sql: Option<String>,
    // tables we are dealing with.can be extract with lib sql_insight
    pub tables: Vec<String>,
}

/// we don't want to check `sql` somehow can be different
impl PartialEq for QueryResp {
    fn eq(&self, other: &Self) -> bool {
        self.fields == other.fields && self.total == other.total && self.data == other.data
    }
}

impl QueryResp {
    fn check(&self) -> anyhow::Result<()> {
        for x in self.data.iter() {
            if x.len() != self.fields.len() {
                anyhow::bail!(
                    "column and data not match {}!={}",
                    self.fields.len(),
                    x.len()
                )
            }
        }
        anyhow::Ok(())
    }
}

impl QueryResp {
    pub fn new(
        fields: Vec<Field>,
        total: f64,
        data: Vec<Vec<Option<String>>>,
        sql: impl Into<Option<String>>,
    ) -> anyhow::Result<Self> {
        let sql = sql.into();
        let ret = Self {
            fields,
            total,
            sql,
            data,
            tables: Default::default(),
        };
        ret.check()?;
        anyhow::Ok(ret)
    }

    pub fn result(sql: impl Into<Option<String>>, result: Vec<Option<String>>) -> Self {
        let sql = sql.into();
        Self {
            total: result.len() as f64,
            sql,
            data: vec![result],
            fields: vec![Field {
                field: "result".to_string(),
                ..Default::default()
            }],
            tables: Default::default(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq, specta::Type)]
pub struct Field {
    pub field: String,

    pub data_type: String,

    pub nullable: bool,
    /// default value of this field
    pub default_value: Option<String>,
    /// is primary key??
    pub is_primary: bool,
    //
    pub is_auto_increment: bool,
    // right now all of implementation are sortable
    pub sortable: bool,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 基础列定义: field_name data_type [NOT NULL] [DEFAULT xxx] [PRIMARY KEY] [AUTO_INCREMENT]
        write!(f, "{} {}", self.field, self.data_type)?;

        if !self.nullable {
            write!(f, " NOT NULL")?;
        }

        if let Some(ref default) = self.default_value {
            write!(f, " DEFAULT {}", default)?;
        }

        if self.is_primary {
            write!(f, " PRIMARY KEY")?;
        }

        if self.is_auto_increment {
            write!(f, " AUTO_INCREMENT")?;
        }

        Ok(())
    }
}

impl Default for Field {
    fn default() -> Self {
        Self {
            field: "".to_string(),
            data_type: "".to_string(),
            nullable: true,
            default_value: None,
            is_primary: false,
            is_auto_increment: false,
            sortable: true,
        }
    }
}

impl Field {
    pub fn from_name_type(name: impl Into<String>, typ: impl Into<String>) -> Self {
        let name = name.into();
        let typ = typ.into();

        Self {
            field: name,
            data_type: typ,
            ..Default::default()
        }
    }
}

impl Field {
    pub fn new(
        name: impl Into<String>,
        typ: impl Into<String>,
        nullable: bool,
        default_value: Option<String>,
        is_primary: bool,
        is_auto_increment: bool,
    ) -> Self {
        let name = name.into();
        let typ = typ.into();
        Self {
            field: name,
            data_type: typ,
            nullable,
            default_value,
            is_primary,
            is_auto_increment,
            sortable: true,
        }
    }
}

pub fn format_data_time_fix_offset(x: &DateTime<FixedOffset>) -> String {
    format!("{} {}", x.naive_local(), x.offset())
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, specta::Type)]
#[serde(tag = "export_kind", rename_all = "lowercase")]
pub enum ExportKind {
    Csv {
        include_header: bool,
        delimiter: char,
    },
    Json {
        pretty: bool,
    },
    JsonLine,
    Sql {
        include_table_schema: bool,
    },
    Xlsx {
        include_header: bool,
        #[serde(flatten)]
        datetime_format: ExportKindXlsDatetimeformat,
    },
    Html {
        include_header: bool,
    },
    MarkDown,
}
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize, specta::Type)]

pub struct ExportKindXlsDatetimeformat {
    datatime: String,
    date: String,
    time: String,
}

impl Default for ExportKindXlsDatetimeformat {
    fn default() -> Self {
        Self {
            datatime: "yyyy-mm-dd hh:mm:ss".into(),
            date: "yyyy-mm-dd".into(),
            time: "hh:mm:ss".into(),
        }
    }
}

impl ExportKind {
    pub(crate) fn cannot_query_result_export_as_sql() -> anyhow::Result<()> {
        anyhow::bail!("can't export sql result as sql")
    }
}

#[derive(Debug, serde::Serialize, specta::Type)]
pub struct TriggerDetail {
    pub timing: TriggerTiming,
    pub event: TriggerEvent,
    pub sql: String,
    pub enable: Option<TriggerEnable>,
    pub metas: Vec<MetaValue>,
    pub name: String,
}

impl PartialEq for TriggerDetail {
    fn eq(&self, other: &Self) -> bool {
        self.timing == other.timing
            && self.event == other.event
            && self.sql == other.sql
            && self.enable == other.enable
    }
}

impl Eq for TriggerDetail {}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq, specta::Type)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TriggerEnable {
    // https://www.postgresql.org/docs/current/catalog-pg-trigger.html
    // Controls in which session_replication_role modes the trigger fires. O = trigger fires in
    // “origin” and “local” modes, D = trigger is disabled, R = trigger fires in “replica” mode, A
    // = trigger fires always.
    PostGres { enable: Option<char> },
    Bool { enable: bool },
}

impl TriggerDetail {
    pub fn new(
        name: &str,
        timing: TriggerTiming,
        event: impl Into<TriggerEvent>,
        sql: impl Into<String>,
        enable: Option<TriggerEnable>,
    ) -> Self {
        Self {
            timing,
            event: event.into(),
            sql: sql.into(),
            enable,
            metas: vec![],
            name: name.to_owned(),
        }
    }

    pub(crate) fn with_metas(&mut self, metas: Vec<MetaValue>) {
        self.metas = metas;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub enum TriggerEvent {
    Insert,
    Update,
    Delete,
    InsteadOf,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, specta::Type)]
pub struct ForeignKeyDef {
    pub name: Option<String>,
    pub table: String,
    pub from: String,
    pub to: String,
    pub on_update: String,
    pub on_delete: String,
}

impl TryFrom<&str> for TriggerEvent {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_uppercase().as_str() {
            "INSERT" => Ok(Self::Insert),
            "UPDATE" => Ok(Self::Update),
            "DELETE" => Ok(Self::Delete),
            "INSTEAD OF" => Ok(Self::InsteadOf),
            _ => anyhow::bail!(format!(
                "'{}' not a valid event,this should be a bug.please report,thanks.",
                value
            )),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq, specta::Type)]
pub enum TriggerTiming {
    Before,
    After,
    InsteadOf,
    String(String),
}

/// meta value can used in various place.
#[derive(Debug, serde::Serialize, specta::Type, serde::Deserialize)]
pub struct MetaValue {
    pub name: String,
    pub value: Option<String>,
}

impl MetaValue {
    pub fn new(name: impl Into<String>, value: impl Into<Option<String>>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

pub fn escape_sql_str(s: &str) -> String {
    s.replace("'", "''")
}

pub fn quote_sql_str(s: &str) -> String {
    format!("'{}'", escape_sql_str(s))
}

pub(crate) fn name_start_ascii_alphabetic(name: &str) -> bool {
    name.chars()
        .next()
        .map(|c| c.is_ascii_alphabetic())
        .unwrap_or(false)
}

#[derive(Clone, Default)]
pub struct AutoCompletionQueryId {
    query_id: Arc<Mutex<String>>,
}

impl AutoCompletionQueryId {
    pub fn get(&self) -> String {
        self.query_id.lock().expect("should lock ok").clone()
    }

    pub fn set(&self, x: String) {
        *self.query_id.lock().expect("should lock ok") = x;
    }
    pub fn set_optional(&self, x: Option<String>) {
        if let Some(x) = x {
            self.set(x);
        }
    }
}

pub struct TimeElapsed {
    d: Duration,
    ins: std::time::Instant,
}

impl Default for TimeElapsed {
    fn default() -> Self {
        Self {
            d: Duration::from_millis(100),
            ins: std::time::Instant::now(),
        }
    }
}

impl TimeElapsed {
    pub fn new(dur: Duration) -> Self {
        Self {
            d: dur,
            ins: std::time::Instant::now(),
        }
    }
    pub fn elapsed(&mut self) -> bool {
        let e = self.ins.elapsed() > self.d;
        if e {
            self.ins = std::time::Instant::now();
        }
        e
    }
}

pub fn create_html_table_header(x: &[String]) -> String {
    format!(
        r#"
        <tr>
            {}
        </tr>
    "#,
        x.iter()
            .map(|x| format!("<th>{}</th>", html_escape::encode_text(x)))
            .collect::<Vec<_>>()
            .join("")
    )
}

pub trait OptionMustBeSomeErr<T> {
    fn option_must_be_some(self) -> anyhow::Result<T>;
}

impl<T> OptionMustBeSomeErr<T> for Option<T> {
    fn option_must_be_some(self) -> anyhow::Result<T> {
        match self {
            Some(x) => Ok(x),
            None => Err(anyhow::Error::msg("must be some")),
        }
    }
}

pub fn create_html_table_data(data: &[Vec<Option<String>>]) -> String {
    let mut ret = "".to_owned();
    for d in data.iter() {
        let x = &format!(
            r#"
            <tr>
                {}
            </tr>
        "#,
            d.iter()
                .map(|x| {
                    format!(
                        "<td>{}</td>",
                        html_escape::encode_text(
                            x.as_ref().map(|x| x.as_str()).unwrap_or_default()
                        )
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        );
        ret.push_str(x.as_str());
    }
    ret
}

fn escape_markdown(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            // Escape structural Markdown characters with a backslash
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '.'
            | '!' => {
                escaped.push('\\');
                escaped.push(c);
            }
            _ => escaped.push(c),
        }
    }
    escaped
}

pub fn create_markdown_header(header: &[String]) -> String {
    let header: Vec<_> = header.iter().map(|x| escape_markdown(x)).collect();

    let mut ret = format!("|{}|\n", header.join("|"));
    ret.push_str(
        format!(
            "|{}|\n",
            (0..header.len())
                .map(|_| ":---:")
                .collect::<Vec<_>>()
                .join("|")
        )
        .as_str(),
    );
    ret
}

pub fn create_markdown_data(header: &[Option<String>]) -> String {
    let ret = format!(
        "|{}|\n",
        header
            .iter()
            .map(|x| match x {
                Some(x) => escape_markdown(x.as_str()),
                None => "".to_owned(),
            })
            .collect::<Vec<_>>()
            .join("|")
    );
    ret
}
pub type QueryEventEmitter = Arc<dyn QueryEventEmit>;

pub trait QueryEventEmit: Sync + Send + 'static {
    fn send(&self, event: QueryEvent) -> anyhow::Result<()>;
}

#[derive(Debug, Default)]
pub struct TestQueryEventEmit {}

impl QueryEventEmit for TestQueryEventEmit {
    fn send(&self, event: QueryEvent) -> anyhow::Result<()> {
        if let QueryEvent::Resp(x) = &event {
            println!(
                "#################################################################################"
            );
            for f in x.fields.iter() {
                println!("column:{} data_type:{}", f.field, f.data_type);
            }
        }

        println!("query event:{:?}", event);
        anyhow::Ok(())
    }
}

#[derive(Debug)]
pub enum QueryEvent {
    Resp(QueryResp),
    Data(Vec<Vec<Option<String>>>),
    Status(String),
    Err(String),
    Row(Vec<Option<String>>),
}

impl From<Vec<Option<String>>> for QueryEvent {
    fn from(value: Vec<Option<String>>) -> Self {
        QueryEvent::Row(value)
    }
}

impl QueryEvent {
    pub fn data_len(&self) -> usize {
        match self {
            QueryEvent::Resp(query_resp) => query_resp.data.len(),
            QueryEvent::Data(items) => items.len(),
            QueryEvent::Status(_) => 0,
            QueryEvent::Err(_) => 0,
            QueryEvent::Row(..) => 1,
        }
    }
}

impl QueryEvent {
    pub(crate) fn new_status(s: impl Into<String>) -> Self {
        Self::Status(s.into())
    }
}

impl From<&str> for QueryEvent {
    fn from(value: &str) -> Self {
        Self::Status(value.to_owned())
    }
}

impl From<QueryResp> for QueryEvent {
    fn from(value: QueryResp) -> Self {
        Self::Resp(value)
    }
}

impl From<Vec<Vec<Option<String>>>> for QueryEvent {
    fn from(value: Vec<Vec<Option<String>>>) -> Self {
        Self::Data(value)
    }
}

pub fn need_paid() -> anyhow::Result<()> {
    if cfg!(feature = "paid") {
        anyhow::Ok(())
    } else {
        anyhow::bail!("this feature is paid")
    }
}

#[derive(Clone, Debug, Default)]
pub struct SqlCollect {
    sql: String,
}

impl SqlCollect {
    pub fn collect(&mut self, sql: impl Into<String>) {
        let sql = sql.into();
        self.sql.push_str(format!("{};\n", sql).as_str());
    }
}

impl From<SqlCollect> for String {
    fn from(value: SqlCollect) -> Self {
        value.sql
    }
}

pub const POOL_MAX_CONNECTIONS: u32 = 200;

#[async_trait::async_trait]

pub trait GetTestExportTableKinds: SchemaTrait {
    fn get_kinds(&self) -> Vec<ExportKind> {
        let mut r = vec![
            ExportKind::Sql {
                include_table_schema: false,
            },
            ExportKind::JsonLine,
            ExportKind::Json { pretty: true },
            ExportKind::Csv {
                include_header: true,
                delimiter: ',',
            },
            ExportKind::Csv {
                include_header: false,
                delimiter: ',',
            },
            ExportKind::MarkDown,
            ExportKind::Html {
                include_header: true,
            },
            ExportKind::Html {
                include_header: false,
            },
            ExportKind::Xlsx {
                include_header: true,
                datetime_format: Default::default(),
            },
            ExportKind::Xlsx {
                include_header: false,
                datetime_format: Default::default(),
            },
        ];
        if self.support_table_schema() {
            r.push(ExportKind::Sql {
                include_table_schema: true,
            });
        }
        r
    }

    fn support_table_schema(&self) -> bool {
        true
    }

    async fn test_export(&self, table_name: &str) {
        for kind in self.get_kinds() {
            let str = self
                .export_table_to_string(table_name, kind.clone())
                .await
                .unwrap_or_else(|_| panic!("should export failed:{:?}", kind));
            println!("export:{:?}\n{}", kind, str)
        }
    }
}

#[derive()]
pub struct FirstTrue {
    value: AtomicBool,
}

impl Default for FirstTrue {
    fn default() -> Self {
        Self {
            value: AtomicBool::new(true),
        }
    }
}

impl FirstTrue {
    pub fn get(&self) -> bool {
        let value = self.value.load(Ordering::Relaxed);
        if value {
            self.value.store(false, Ordering::Relaxed);
        }
        value
    }
}

pub trait ExtractSqlValeFromJsonValue {
    fn extrat_sql_value_from_json_value(
        &self,
        value: &serde_json::Value,
        ty: &str,
    ) -> anyhow::Result<Option<String>>;
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(transparent)]
pub struct JsUint8Array(pub Vec<u8>);

impl specta::Type for JsUint8Array {
    fn definition(_: &mut specta::Types) -> specta::datatype::DataType {
        specta::datatype::DataType::Reference(specta_typescript::define("Uint8Array"))
    }
}

impl Deref for JsUint8Array {
    type Target = Vec<u8>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
