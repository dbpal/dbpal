use std::borrow::Cow;
use std::collections::HashMap;

use std::fmt::{Debug, Display};
use std::ops::Deref;

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, FixedOffset};

use serde::Serializer;

use tokio::io::AsyncWriteExt;
use typescript_type_def::TypeDef;

use crate::meta_value::MetaValue;
use crate::query_log::{QueryLog, QueryLogEvent};

pub trait BackendName {
    fn backend_name(&self) -> &str;
}

/// A db representation of schema
#[async_trait::async_trait]
pub trait SchemaTrait: Sync + Send + QuerySql + CheckConn + FormatSql + 'static {
    fn as_any(&self) -> &dyn std::any::Any;

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>>;
    async fn show_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>>;
    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp>;
    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>>;
    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet>;
    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet>;

    /// format a value make it suitable for insert into sql.
    /// this method current use in `insert_sql` etc.
    fn put_value_in_sql(&self, _f: &Field, _value: Option<&str>) -> anyhow::Result<String> {
        not_support_features(self.backend_name())
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()>;

    async fn delete(
        &self, _table_name: &str, _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet>;

    async fn list_view(&self) -> anyhow::Result<Vec<NamedStaff>> {
        not_support_features(self.backend_name())
    }
    async fn drop_view(&self, _view_name: &str) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }
    async fn drop_table(&self, table: &str) -> anyhow::Result<()>;
    async fn show_view(
        &self, _view_name: &str, _where_str: Option<&str>, _order_by: Option<&str>, _page: usize,
        _page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        not_support_features(self.backend_name())
    }

    async fn export_table(
        &self, _table_name: &str, _kind: ExportKind,
        _sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }

    async fn export_query_result(
        &self, _sql: &str, _kind: ExportKind, _sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }

    async fn export_query_result_to_file(
        &self, sql: &str, kind: ExportKind, file: &str,
    ) -> anyhow::Result<()> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
        let h1 = {
            let x = self.clone();
            let sql = sql.to_string();
            let h1 = tokio::spawn(async move { x.export_query_result(&sql, kind, sender).await });
            h1
        };
        let h2 = {
            let filepath = file.to_string();
            tokio::spawn(async move {
                let mut file = tokio::fs::File::create(&filepath).await?;
                while let Some(r) = receiver.recv().await {
                    file.write_all(r.as_bytes()).await?;
                }
                anyhow::Ok(())
            })
        };
        h1.await??;
        h2.await??;
        anyhow::Ok(())
    }

    async fn export_table_to_file(
        &self, table_name: &str, filepath: &str, kind: ExportKind,
    ) -> anyhow::Result<()> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
        let h1 = {
            let x = self.clone();
            let table_name = table_name.to_string();
            let h1 = tokio::spawn(async move { x.export_table(&table_name, kind, sender).await });
            h1
        };
        let h2 = {
            let filepath = filepath.to_string();
            tokio::spawn(async move {
                let mut file = tokio::fs::File::create(&filepath).await?;
                while let Some(r) = receiver.recv().await {
                    file.write_all(r.as_bytes()).await?;
                }
                anyhow::Ok(())
            })
        };
        h1.await??;
        h2.await??;
        anyhow::Ok(())
    }

    fn clone(&self) -> Box<dyn SchemaTrait>;
    async fn create_table_and_index_sql(
        &self, _table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        not_support_features(self.backend_name())
    }

    async fn create_view_sql(&self, _view_name: &str) -> anyhow::Result<String> {
        not_support_features(self.backend_name())
    }
    /// if this connection is read only
    fn read_only(&self) -> bool;

    fn can_write(&self) -> anyhow::Result<()> {
        if self.read_only() {
            return Err(anyhow::Error::msg("this source is readonly"));
        }
        anyhow::Ok(())
    }

    async fn list_trigger(&self, _table_name: &str) -> anyhow::Result<Vec<NamedStaff>> {
        not_support_features(self.backend_name())
    }
    async fn show_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<TriggerDetail> {
        not_support_features(self.backend_name())
    }
    async fn drop_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }

    async fn disable_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }
    async fn enable_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }
    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()>;
    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()>;

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        not_support_features(self.backend_name())
    }
    async fn show_udf(&self, _name: &str) -> anyhow::Result<UdfDetail> {
        not_support_features(self.backend_name())
    }

    async fn drop_udf(
        &self, _name: &str, _extra: Option<DropUdfExtraParameter>,
    ) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        not_support_features(self.backend_name())
    }
    async fn show_procedure(&self, _name: &str) -> anyhow::Result<ProcedureDetail> {
        not_support_features(self.backend_name())
    }
    async fn drop_procedure(&self, _name: &str) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }
    async fn table_exists(&self, table: &str) -> anyhow::Result<bool> {
        let e = self
            .list_table()
            .await?
            .iter()
            .any(|x| x.name.eq_ignore_ascii_case(table));
        anyhow::Ok(e)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "drop_udf_extra_paramter", rename_all = "lowercase")]
pub enum DropUdfExtraParameter {
    PostGres {
        parameters: String, // a integer , b integer
    },
}

fn not_support_features<T>(name: &str) -> anyhow::Result<T> {
    anyhow::bail!(format!(
        "this backend '{}' don't support this feature",
        name
    ))
}

#[derive(Debug, serde::Serialize, TypeDef)]
pub struct ProcedureDetail {
    // definition of the sql.
    def: String,
    metas: Vec<MetaValue>,
}
impl ProcedureDetail {
    pub(crate) fn new(def: String, metas: Vec<MetaValue>) -> Self {
        Self { metas, def }
    }
}

#[derive(Debug, serde::Serialize, TypeDef)]
pub struct UdfDetail {
    // definition of the sql.
    def: String,
    metas: Vec<MetaValue>,
}

impl UdfDetail {
    pub(crate) fn new(def: String, metas: Vec<MetaValue>) -> Self {
        Self { metas, def }
    }
}
#[async_trait::async_trait]
pub trait CatalogTrait: Send + Sync + 'static + QuerySql + FormatSql + CheckConn {
    async fn list_database(&self) -> anyhow::Result<Vec<String>>;
    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>>;
}

/// named staff maybe used for view udf etc...
#[derive(Debug, serde::Serialize, TypeDef, Clone)]
pub struct NamedStaff {
    pub name: String,
}

impl From<String> for NamedStaff {
    fn from(value: String) -> Self {
        NamedStaff { name: value }
    }
}

/// almost copied from https://docs.rs/sqlformat/0.2.4/sqlformat/struct.FormatOptions.html

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TypeDef)]
pub struct FormatSqlOpts {
    pub indent: FormatSqlOptsIndent,
    pub uppercase: Option<bool>,
    pub lines_between_queries: u8,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TypeDef)]
#[serde(tag = "ident", rename_all = "lowercase")]
pub enum FormatSqlOptsIndent {
    Spaces(u8),
    Tabs,
}

impl Into<sqlformat::FormatOptions<'static>> for FormatSqlOpts {
    fn into(self) -> sqlformat::FormatOptions<'static> {
        sqlformat::FormatOptions {
            indent: match self.indent {
                FormatSqlOptsIndent::Spaces(x) => sqlformat::Indent::Spaces(x),
                FormatSqlOptsIndent::Tabs => sqlformat::Indent::Tabs,
            },
            uppercase: self.uppercase,
            lines_between_queries: self.lines_between_queries,
            ignore_case_convert: None,
        }
    }
}

pub trait FormatSql {
    fn format_sql(&self, sql: &str, ops: Option<FormatSqlOpts>) -> anyhow::Result<String>;
}

#[async_trait::async_trait]
pub trait CheckConn {
    /// Duration is basic `select 1` execution time.
    async fn check_conn(&self) -> anyhow::Result<Duration>;
}
/// create table and all index sql.
#[derive(Clone, serde::Serialize)]
pub struct CreateTableAndIndexSql {
    pub create_table: String,

    /// some backend can query create table and index in one sql
    /// this can be empty
    pub indexes: Vec<String>,
}
impl dyn SchemaTrait {
    #[allow(unused)]
    pub async fn export_table_to_string(
        &self, table_name: &str, kind: ExportKind,
    ) -> anyhow::Result<String> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
        let h1 = {
            let x = self.clone();
            let table_name = table_name.to_string();
            let h1 = tokio::spawn(async move { x.export_table(&table_name, kind, sender).await });
            h1
        };
        let h2 = {
            tokio::spawn(async move {
                let mut s = String::new();
                while let Some(r) = receiver.recv().await {
                    s.push_str(r.deref());
                }
                anyhow::Ok(s)
            })
        };
        h1.await??;
        h2.await?
    }

    #[allow(unused)]
    pub async fn export_query_result_to_string(
        &self, sql: &str, kind: ExportKind,
    ) -> anyhow::Result<String> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
        let h1 = {
            let x = self.clone();
            let sql = sql.to_string();
            let h1 = tokio::spawn(async move { x.export_query_result(&sql, kind, sender).await });
            h1
        };
        let h2 = {
            tokio::spawn(async move {
                let mut s = String::new();
                while let Some(r) = receiver.recv().await {
                    s.push_str(r.deref());
                }
                anyhow::Ok(s)
            })
        };
        h1.await??;
        h2.await?
    }
}

#[async_trait::async_trait]
pub trait DBTrait: Sync + Send + QuerySql + 'static + CheckConn + FormatSql {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>>;
    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>>;
}

#[async_trait::async_trait]
pub trait QuerySql: BackendName + Send + Sync + 'static {
    async fn query_sql(
        &self, query_id: uuid::Uuid, _sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>>;
    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid>;

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()>;

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()>;
    async fn get_query_log(&self, id: uuid::Uuid) -> anyhow::Result<Vec<QueryLogEvent>> {
        let logs = QueryLog::get_instance()
            .get(&id)
            .map(|x| x.lock().unwrap().clone().logs)
            .unwrap_or(vec![]);
        anyhow::Ok(logs)
    }
    fn is_dangerous_sql(&self, _sql: &str) -> anyhow::Result<bool> {
        not_support_features(self.backend_name())
    }
}

/// api can be more easy used with frontend,reis backend may impl
/// for more easy usage
#[async_trait::async_trait]
pub trait QuerySqlWithoutOpenConn {
    async fn query_sql(&self, _sql: &str) -> anyhow::Result<Vec<Result<QueryResp, String>>>;
}

#[derive(Debug, serde::Serialize, TypeDef)]
pub struct ListTable {
    pub name: String,
    pub size: Option<HumanSize>,
}

/// humansize formatted string.
pub struct HumanSize(Box<dyn humansize::ToF64 + Send + 'static>);

impl serde::Serialize for HumanSize {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl Display for HumanSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            humansize::format_size_i(
                self.0.as_ref().to_f64(),
                &humansize::FormatSizeOptions::default()
            )
        )
    }
}

impl Debug for HumanSize {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl HumanSize {
    fn new<T: humansize::ToF64 + Send + 'static>(t: T) -> Self {
        Self(Box::new(t) as Box<dyn humansize::ToF64 + Send + 'static>)
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
    pub(crate) fn new<T: humansize::ToF64 + Send + 'static>(
        name: impl Into<String>, t: Option<T>,
    ) -> Self {
        Self {
            name: name.into(),
            size: t.map(|x| HumanSize::new(x)),
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
pub trait ServerMetas {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>>;
}

impl dyn SchemaTrait {
    pub(crate) fn delete_sql(
        &self, schema: Option<&str>, table_name: &str, fs: &Vec<Field>,
        wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        let mut field_by_name = HashMap::new();
        for f in fs.iter() {
            field_by_name.insert(&f.name, f);
        }
        let mut sql = format!(
            "DELETE FROM {}{table_name} WHERE ",
            if let Some(schema) = schema {
                format!("{}.", schema)
            } else {
                "".to_string()
            }
        );
        {
            let last = wheres.len() - 1;
            for (index, (k, v)) in wheres.iter().enumerate() {
                let f = field_by_name.get(k);
                if f.is_none() {
                    anyhow::bail!("field  '{}' not found", k);
                }
                sql.push_str(
                    format!(
                        "{} = {}",
                        k,
                        self.put_value_in_sql(*f.unwrap(), v.as_ref().map(|x| x.as_str()))?
                    )
                    .as_str(),
                );
                if index != last {
                    sql.push_str(" AND ")
                }
            }
        }
        log::info!("delete sql:{}", sql);
        anyhow::Ok(sql)
    }

    pub(crate) fn update_sql(
        &self, schema: Option<&str>, table_name: &str, fs: &Vec<Field>,
        values: &HashMap<String, Option<String>>, wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        //

        let mut field_by_name = HashMap::new();
        for f in fs.iter() {
            field_by_name.insert(&f.name, f);
        }
        let mut sql = format!(
            "UPDATE {}{table_name} SET ",
            if let Some(schema) = schema {
                format!("{}.", schema)
            } else {
                "".to_string()
            }
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
                        k,
                        self.put_value_in_sql(*f.unwrap(), v.as_ref().map(|x| x.as_str()))?
                    )
                    .as_str(),
                );
                if index != last {
                    sql.push_str(",")
                }
            }
        }
        sql.push_str(" WHERE ");
        {
            let last = wheres.len() - 1;
            for (index, (k, v)) in wheres.iter().enumerate() {
                let f = field_by_name.get(k);
                if f.is_none() {
                    anyhow::bail!("field  '{}' not found", k);
                }
                sql.push_str(
                    format!(
                        "{} = {}",
                        k,
                        self.put_value_in_sql(*f.unwrap(), v.as_ref().map(|x| x.as_str()))?
                    )
                    .as_str(),
                );
                if index != last {
                    sql.push_str(" ABD ")
                }
            }
        }
        log::info!("update sql:{}", sql);

        anyhow::Ok(sql)
    }

    pub(crate) fn insert_sql(
        &self, schema: Option<&str>, table_name: &str, fs: &Vec<Field>,
        values: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        // fields
        let mut sql = format!(
            "INSERT INTO {}{table_name}(",
            if let Some(schema) = schema {
                format!("{}.", schema)
            } else {
                "".to_string()
            }
        );
        {
            let last = fs.len() - 1;
            for (index, f) in fs.iter().enumerate() {
                if f.is_auto_increment {
                    continue;
                }
                sql.push_str(&f.name);
                if index != last {
                    sql.push_str(",");
                }
            }
            sql.push_str(")");
        }
        // values
        sql.push_str(" VALUES (");
        {
            let last = fs.len() - 1;
            for (index, f) in fs.iter().enumerate() {
                if f.is_auto_increment {
                    continue;
                }
                let value = values
                    .get(&f.name)
                    .map(|x| x.as_ref().map(|x| x.as_str()))
                    .flatten();

                sql.push_str(&self.put_value_in_sql(f, value)?);
                if index != last {
                    sql.push_str(",");
                }
            }
            sql.push_str(")");
        }
        anyhow::Ok(sql)
    }
}

#[derive(Clone, Debug, serde::Serialize, TypeDef)]
pub struct SqlExecuteRet {
    pub sql: String,
    pub changes: u64,
    pub last_insert_row_id: Option<i64>,
    /// most of the database have id int but mongo have string
    pub last_insert_id_string: Option<String>,
}

impl From<(String, u64)> for SqlExecuteRet {
    fn from((sql, changes): (String, u64)) -> Self {
        Self {
            sql,
            changes,
            last_insert_id_string: None,
            last_insert_row_id: None,
        }
    }
}
impl From<u64> for SqlExecuteRet {
    fn from(changes: u64) -> Self {
        Self {
            sql: "".to_string(),
            changes,
            last_insert_id_string: None,
            last_insert_row_id: None,
        }
    }
}
#[derive(Debug, Clone, serde::Serialize, TypeDef)]
pub struct Index {
    pub(crate) name: String,
    pub(crate) kind: IndexKind,
    pub(crate) on: Vec<String>,
}

impl Index {
    pub fn new(name: impl Into<String>, kind: IndexKind, on: Vec<String>) -> Self {
        let name = name.into();
        Self { name, kind, on }
    }
}

#[derive(Clone, serde::Serialize, Copy, Debug, TypeDef)]
pub enum IndexKind {
    Normal,
    Unique,
    Primary,
}
#[derive(Clone, Debug, serde::Serialize, TypeDef, PartialEq, Eq)]
pub struct QueryResp {
    pub(crate) fields: Vec<Field>,
    pub(crate) data: Vec<Vec<Option<String>>>,
}
impl QueryResp {
    pub(crate) fn new(fields: Vec<Field>, data: Vec<Vec<Option<String>>>) -> Self {
        Self { fields, data }
    }

    pub fn result(col_name: impl Into<String>, result: Vec<Option<String>>) -> Self {
        let col_name = col_name.into();

        Self {
            data: vec![result],
            fields: vec![Field {
                name: col_name,
                typ: "".to_string(),
                nullable: false,
                default_value: None,
                is_primary: false,
                is_auto_increment: false,
            }],
        }
    }

    pub fn row_affected(affected: u64) -> Self {
        Self {
            data: vec![vec![Some(affected.to_string())]],
            fields: vec![Field {
                name: "row affected".to_string(),
                typ: "int".to_string(),
                nullable: false,
                default_value: None,
                is_primary: false,
                is_auto_increment: false,
            }],
        }
    }
}

#[derive(Clone, Debug, serde::Serialize, TypeDef)]
pub struct TableListResp {
    pub(crate) fields: Vec<Field>,
    pub(crate) total: usize,
    pub(crate) data: Vec<Vec<Option<String>>>,
    pub(crate) sql: Option<String>,
}

/// we don't want to check `sql` somehow can be different
impl PartialEq for TableListResp {
    fn eq(&self, other: &Self) -> bool {
        self.fields == other.fields && self.total == other.total && self.data == other.data
    }
}

impl TableListResp {
    #[allow(dead_code)]
    fn check(&self) {
        for x in self.data.iter() {
            assert_eq!(x.len(), self.fields.len());
        }
    }
}

impl TableListResp {
    pub fn to_query_resp(self) -> QueryResp {
        let TableListResp {
            fields,
            total: _,
            data,
            sql: _,
        } = self;
        QueryResp::new(fields, data)
    }

    pub fn new(
        fields: Vec<Field>, total: usize, data: Vec<Vec<Option<String>>>,
        sql: impl Into<Option<String>>,
    ) -> Self {
        let sql = sql.into();
        let ret = Self {
            fields,
            total,
            sql,
            data,
        };
        #[cfg(debug_assertions)]
        ret.check();
        ret
    }

    pub fn result(
        col_name: impl Into<String>, sql: impl Into<Option<String>>, result: Vec<Option<String>>,
    ) -> Self {
        let sql = sql.into();
        let col_name = col_name.into();

        Self {
            total: result.len(),
            sql,
            data: vec![result],
            fields: vec![Field {
                name: col_name,
                typ: "".to_string(),
                nullable: false,
                default_value: None,
                is_primary: false,
                is_auto_increment: false,
            }],
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, TypeDef, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub typ: String,
    pub nullable: bool,
    /// default value of this field
    pub default_value: Option<String>,
    /// is primary key??
    pub is_primary: bool,
    //
    pub is_auto_increment: bool,
}

impl Field {
    pub fn from_name_type(name: impl Into<String>, typ: impl Into<String>) -> Self {
        let name = name.into();
        let typ = typ.into();

        Self {
            name,
            typ,
            nullable: true,
            default_value: None,
            is_auto_increment: false,
            is_primary: false,
        }
    }
}

impl Field {
    pub fn new(
        name: impl Into<String>, typ: impl Into<String>, nullable: bool,
        default_value: Option<String>, is_primary: bool, is_auto_increment: bool,
    ) -> Self {
        let name = name.into();
        let typ = typ.into();
        Self {
            name,
            typ,
            nullable,
            default_value,
            is_primary,
            is_auto_increment,
        }
    }
}

pub fn format_data_time_fix_offset(x: &DateTime<FixedOffset>) -> String {
    format!("{} {}", x.naive_local(), x.offset())
}

#[derive(Clone, Debug, Copy, serde::Deserialize, serde::Serialize, TypeDef)]
#[serde(tag = "export_kind", rename_all = "lowercase")]
pub enum ExportKind {
    CSV {
        // include
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
}

impl ExportKind {
    pub(crate) fn cannot_query_result_export_as_sql() -> anyhow::Result<()> {
        anyhow::bail!("cannot export sql result as sql")
    }
}

#[derive(Debug, serde::Serialize, TypeDef)]
pub struct TriggerDetail {
    pub(crate) timing: TriggerTiming,
    pub(crate) event: TriggerEvent,
    pub(crate) sql: String,
    pub(crate) enable: Option<TriggerEnable>,
    pub(crate) metas: Vec<MetaValue>,
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

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq, TypeDef)]
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
        timing: TriggerTiming, event: impl Into<TriggerEvent>, sql: impl Into<String>,
        enable: Option<TriggerEnable>,
    ) -> Self {
        Self {
            timing,
            event: event.into(),
            sql: sql.into(),
            enable,
            metas: vec![],
        }
    }

    pub(crate) fn with_metas(&mut self, metas: Vec<MetaValue>) {
        self.metas = metas;
    }
}

#[derive(Debug, Clone, TypeDef, PartialEq, Eq, serde::Serialize)]
pub enum TriggerEvent {
    Insert,
    Update,
    Delete,
}

impl TryFrom<&str> for TriggerEvent {
    type Error = anyhow::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_uppercase().as_str() {
            "INSERT" => Ok(Self::Insert),
            "UPDATE" => Ok(Self::Update),
            "DELETE" => Ok(Self::Delete),
            _ => anyhow::bail!(format!("'{}' not a valid event", value)),
        }
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, TypeDef, PartialEq, Eq)]
pub enum TriggerTiming {
    Before,
    After,
    /// sql server have this timing but I am not quit understand.
    InsteadOf,
}
