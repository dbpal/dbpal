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

use crate::language_server::LanguageServer;
use crate::meta_value::MetaValue;
use crate::query_log::{QueryLog, QueryLogEvent};

pub trait BackendName {
    fn backend_name(&self) -> &str;
}

/// A db representation of schema
#[async_trait::async_trait]
pub trait SchemaTrait:
    Sync + Send + QuerySql + CheckConn + std::fmt::Debug + ServerMetas + 'static
{
    fn as_any(&self) -> &dyn std::any::Any;

    fn quote_style(&self) -> &'static str {
        ""
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>>;
    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>>;
    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp>;
    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>>;

    async fn rename_index(
        &self, _table_name: &str, _oldname: &str, _newname: &str,
    ) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }

    async fn drop_index(&self, _table_name: &str, _index: &str) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }

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

    async fn drop_table_column(&self, _table: &str, _col: &str) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }
    fn list_data_types(&self) -> anyhow::Result<&'static [&'static str]> {
        not_support_features(self.backend_name())
    }
    async fn modify_table_column(
        &self, _table: &str, _old_name: &str, _new_name: &str, _typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }
    async fn add_table_column(
        &self, _table: &str, _name: &str, _col: ColumnType,
    ) -> anyhow::Result<()> {
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
    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()>;
    async fn show_view(
        &self, _view_name: &str, _where_str: Option<&str>, _order_by: Option<&str>, _page: usize,
        _page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        not_support_features(self.backend_name())
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        // 有些后端没有数据 也能得到完整的结果所以提供一个默认实现
        // 后端基于sqlx的需要使用prepare_with这种特殊的接口
        self.show_view(view_name, None, None, 1, 1)
            .await
            .map(|x| x.fields)
    }

    async fn export_table_or_view(
        &self, _name: &str, _kind: ExportKind,
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
            let h1 =
                tokio::spawn(
                    async move { x.export_table_or_view(&table_name, kind, sender).await },
                );
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
        anyhow::bail!(format!(
            "sql server don't support create_table_and_index_sql"
        ))
    }

    async fn create_view_sql(&self, _view_name: &str) -> anyhow::Result<String> {
        anyhow::bail!(format!("sql server don't support create_view_sql"))
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

    async fn show_udf(
        &self, _name: &str, _arg: Option<UdfOrPrecedureExtraParameter>,
    ) -> anyhow::Result<UdfDetail> {
        not_support_features(self.backend_name())
    }

    async fn drop_udf(
        &self, _name: &str, _extra: Option<UdfOrPrecedureExtraParameter>,
    ) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        not_support_features(self.backend_name())
    }
    async fn show_procedure(
        &self, _name: &str, _arg: Option<UdfOrPrecedureExtraParameter>,
    ) -> anyhow::Result<ProcedureDetail> {
        not_support_features(self.backend_name())
    }
    async fn drop_procedure(
        &self, _name: &str, _arg: Option<UdfOrPrecedureExtraParameter>,
    ) -> anyhow::Result<()> {
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
pub struct ColumnType {
    // type string like `varchar`
    pub(crate) typ: String,
    pub(crate) nullness: Option<String>,
    // mysql add column can have this field
    pub(crate) after: Option<String>,

    pub(crate) default_value: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "extra", rename_all = "lowercase")]
pub enum UdfOrPrecedureExtraParameter {
    PostGres {
        parameters: String, // a integer , b integer
    },
}

pub(crate) fn not_support_features<T>(name: &str) -> anyhow::Result<T> {
    anyhow::bail!(format!(
        "this backend '{}' don't support this feature",
        name
    ))
}

#[derive(Debug, serde::Serialize, TypeDef)]
pub struct ProcedureDetail {
    // definition of the sql.
    pub def: String,
    pub metas: Vec<MetaValue>,
}
impl ProcedureDetail {
    pub(crate) fn new(def: String, metas: Vec<MetaValue>) -> Self {
        Self { metas, def }
    }
}

#[derive(Debug, serde::Serialize, TypeDef)]
pub struct UdfDetail {
    // definition of the sql.
    pub def: String,
    pub metas: Vec<MetaValue>,
}

impl UdfDetail {
    pub(crate) fn new(def: String, metas: Vec<MetaValue>) -> Self {
        Self { metas, def }
    }
}

#[async_trait::async_trait]
pub trait CatalogTrait:
    Send + Sync + 'static + QuerySql + CheckConn + LanguageServer + Debug + ServerMetas
{
    async fn list_database(&self) -> anyhow::Result<Vec<String>>;
    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>>;
    async fn create_database(&self, name: &str) -> anyhow::Result<()>;
    async fn drop_database(&self, name: &str) -> anyhow::Result<()>;
}

/// named staff maybe used for view udf etc...
#[derive(Debug, serde::Serialize, TypeDef)]
pub struct NamedStaff {
    pub name: String,
    pub metas: Vec<MetaValue>,
}

impl From<String> for NamedStaff {
    fn from(value: String) -> Self {
        NamedStaff {
            name: value,
            metas: vec![],
        }
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
            let h1 =
                tokio::spawn(
                    async move { x.export_table_or_view(&table_name, kind, sender).await },
                );
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
pub trait DBTrait: Sync + Send + QuerySql + 'static + CheckConn + Debug + ServerMetas {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>>;
    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>>;
    async fn create_schema(&self, name: &str) -> anyhow::Result<()>;
    async fn drop_schema(&self, name: &str) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
pub trait QuerySql: BackendName + Send + Sync + 'static + LanguageServer {
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
pub struct HumanSize(pub(crate) Box<dyn humansize::ToF64 + Send + 'static>);

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
    pub(crate) fn make_where(
        &self, fs: &Vec<Field>, wheres: &HashMap<String, Option<String>>,
        field_by_name: &HashMap<&String, &Field>, type_support_compare: impl Fn(&str) -> bool,
    ) -> anyhow::Result<String> {
        // if contains primary key should use primary key to delete
        // otherwise should use all value
        // to do operation.
        let contains_pri = fs.iter().any(|x| x.is_primary);
        let mut sql = String::new();

        let mut first = true;
        for (k, v) in wheres.iter() {
            let f = field_by_name.get(k);
            let f = f.map(|x| *x);

            if f.is_none() {
                anyhow::bail!("field  '{}' not found", k);
            }
            let f = f.unwrap();
            // some type can't compare
            // so they can't appear in where condition
            if !type_support_compare(f.typ.as_str()) {
                continue;
            }
            if contains_pri && !f.is_primary {
                continue;
            }
            sql.push_str(
                format!(
                    "{} {}{}{} = {}",
                    if !first { " AND " } else { "" },
                    self.quote_style(),
                    k,
                    self.quote_style(),
                    self.put_value_in_sql(f, v.as_ref().map(|x| x.as_str()))?
                )
                .as_str(),
            );
            first = false;
        }
        anyhow::Ok(sql)
    }

    pub(crate) fn delete_sql(
        &self, schema: Option<&str>, table_name: &str, fs: &Vec<Field>,
        wheres: &HashMap<String, Option<String>>, type_support_compare: impl Fn(&str) -> bool,
    ) -> anyhow::Result<String> {
        let mut field_by_name = HashMap::new();
        for f in fs.iter() {
            field_by_name.insert(&f.field, f);
        }

        let mut sql = format!(
            "DELETE FROM {}{table_name} WHERE ",
            if let Some(schema) = schema {
                format!("{}{}{}.", self.quote_style(), schema, self.quote_style())
            } else {
                "".to_string()
            }
        );

        sql.push_str(
            self.make_where(fs, wheres, &field_by_name, type_support_compare)?
                .as_str(),
        );
        log::info!("delete sql:{}", sql);
        anyhow::Ok(sql)
    }

    pub(crate) fn update_sql(
        &self, schema: Option<&str>, table_name: &str, fs: &Vec<Field>,
        values: &HashMap<String, Option<String>>, wheres: &HashMap<String, Option<String>>,
        type_support_compare: impl Fn(&str) -> bool,
    ) -> anyhow::Result<String> {
        //

        let mut field_by_name = HashMap::new();
        for f in fs.iter() {
            field_by_name.insert(&f.field, f);
        }

        let mut sql = format!(
            "UPDATE {}{table_name} SET ",
            if let Some(schema) = schema {
                format!("{}{}{}.", self.quote_style(), schema, self.quote_style())
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
                        "{}{}{} = {}",
                        self.quote_style(),
                        k,
                        self.quote_style(),
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
        sql.push_str(
            self.make_where(fs, wheres, &field_by_name, type_support_compare)?
                .as_str(),
        );
        log::info!("update sql:{}", sql);
        anyhow::Ok(sql)
    }

    pub(crate) fn insert_sql(
        &self, schema: Option<&str>, table_name: &str, fs: &Vec<Field>,
        values: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        let mut field_by_name = HashMap::new();
        for f in fs.iter() {
            field_by_name.insert(&f.field, f);
        }
        // check
        for col in values.keys() {
            if field_by_name.get(col).is_none() {
                anyhow::bail!(format!("col '{}' not found", col))
            }
        }
        let mut sql = format!(
            "INSERT INTO {}{table_name}(",
            if let Some(schema) = schema {
                format!("{}{}{}.", self.quote_style(), schema, self.quote_style())
            } else {
                "".to_string()
            }
        );
        // fields
        {
            let last = values.len() - 1;
            for (index, f) in values.iter().enumerate() {
                if field_by_name.get(f.0).map(|x| x.is_auto_increment).unwrap() {
                    continue;
                }
                sql.push_str(
                    format!(
                        "{}{}{}",
                        self.quote_style(),
                        f.0.as_str(),
                        self.quote_style()
                    )
                    .as_str(),
                );
                if index != last {
                    sql.push_str(",");
                }
            }
            sql.push_str(")");
        }
        // values
        sql.push_str(" VALUES (");
        {
            let last = values.len() - 1;
            for (index, f) in values.iter().enumerate() {
                if field_by_name.get(f.0).map(|x| x.is_auto_increment).unwrap() {
                    continue;
                }
                let value = f.1.as_ref().map(|x| x.as_str());
                let f = field_by_name.get(f.0).unwrap();
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
    pub name: String,
    pub kind: IndexKind,
    pub on: Vec<String>,
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
    pub fields: Vec<Field>,
    pub data: Vec<Vec<Option<String>>>,
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
                field: col_name,
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
                field: "row affected".to_string(),
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
    pub fields: Vec<Field>,
    pub total: usize,
    pub data: Vec<Vec<Option<String>>>,
    pub sql: Option<String>,
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
            debug_assert_eq!(
                x.len(),
                self.fields.len(),
                "fileds:{:?},data:{:?}",
                self.fields,
                self.data
            );
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
                field: col_name,
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
    pub field: String,
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
            field: name,
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
            field: name,
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
    pub timing: TriggerTiming,
    pub event: TriggerEvent,
    pub sql: String,
    pub enable: Option<TriggerEnable>,
    pub metas: Vec<MetaValue>,
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

pub fn monaco_editor_keyword_regexp(keywords: &[&str]) -> String {
    let mut ret = "/\\b(".to_string();
    let last = keywords.len() - 1;
    for (index, c) in keywords.iter().enumerate() {
        ret.push_str(*c);
        if index != last {
            ret.push_str("|");
        }
    }
    ret.push_str(")\\b/");

    return ret;
}

#[cfg(test)]
mod tests {
    use super::UdfOrPrecedureExtraParameter;

    #[test]
    fn test_serde_udf_or_precedure_extra_parameter() {
        let x = Some(UdfOrPrecedureExtraParameter::PostGres {
            parameters: "int".to_string(),
        });
        let s = serde_json::to_string_pretty(&x).unwrap();
        println!("{:?}", s);
    }

    #[test]
    fn test_sql_parse() {
        let s = r#"
        CREATE OR REPLACE FUNCTION test.factorial(n integer)
            RETURNS integer
            LANGUAGE plpgsql
            AS $function$
            DECLARE
                result INT := 1;
            BEGIN
                IF n < 0 THEN
                    RETURN NULL;  -- 负数的阶乘未定义
                ELSIF n = 0 THEN
                    RETURN 1;  -- 0 的阶乘为 1
                ELSE
                    FOR i IN 1..n LOOP
                        result := result * i;
                    END LOOP;
                    RETURN result;
                END IF;
            END;
            $function$
"#;

        let r = sqlformat::format(
            s,
            &sqlformat::QueryParams::None,
            &sqlformat::FormatOptions {
                indent: sqlformat::Indent::Tabs,
                uppercase: None,
                lines_between_queries: 1,
                ignore_case_convert: None,
            },
        );
        println!("{}", r)
    }
}
