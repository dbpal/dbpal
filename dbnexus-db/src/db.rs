use std::borrow::Cow;
use std::collections::HashMap;

use std::ops::Deref;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, FixedOffset};
use dashmap::DashMap;

use line_index::{LineCol, TextSize};
use serde::ser::SerializeStruct;
use serde::Serializer;
use sqlparser::ast::Statement;
use sqlparser::dialect::Dialect;
use sqlparser::tokenizer::{Location, Token};
use tokio::io::AsyncWriteExt;
use typescript_type_def::TypeDef;

/// A db representation of schema
#[async_trait::async_trait]
pub trait SchemaTrait: Sync + Send + QuerySql + CheckConn + FormatSql + 'static {
    fn as_any(&self) -> &dyn std::any::Any;
    fn backend_name(&self) -> &str;

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

    async fn list_view(&self) -> anyhow::Result<Vec<ListView>> {
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

    async fn list_trigger(&self, _table_name: &str) -> anyhow::Result<Vec<ListTrigger>> {
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

    async fn list_udf(&self) -> anyhow::Result<Vec<ListUdf>> {
        not_support_features(self.backend_name())
    }
    async fn show_udf(&self, _name: &str) -> anyhow::Result<UdfDetail> {
        not_support_features(self.backend_name())
    }
    async fn drop_udf(&self, _name: &str) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<ListProcedure>> {
        not_support_features(self.backend_name())
    }
    async fn show_procedure(&self, _name: &str) -> anyhow::Result<ProcedureDetail> {
        not_support_features(self.backend_name())
    }
    async fn drop_procedure(&self, _name: &str) -> anyhow::Result<()> {
        not_support_features(self.backend_name())
    }
    async fn table_exists(&self, table: &str) -> anyhow::Result<bool> {
        let e = self.list_table().await?.iter().any(|x| &x.name == table);
        anyhow::Ok(e)
    }
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

/// meta value can used in various place.
#[derive()]

pub struct MetaValue {
    name: Cow<'static, str>,
    value: Option<Box<dyn ToString + Send + 'static>>,
    desc: Option<Cow<'static, str>>,
}
#[async_trait::async_trait]
pub trait CatalogTrait: Send + Sync + 'static + QuerySql + FormatSql + CheckConn {
    async fn list_database(&self) -> anyhow::Result<Vec<String>>;
    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>>;
}

impl MetaValue {
    pub(crate) fn new<T: ToString + Send + 'static>(
        name: impl Into<Cow<'static, str>>, value: T,
    ) -> Self {
        Self {
            name: name.into(),
            value: Some(Box::new(value)),
            desc: None,
        }
    }
    pub(crate) fn new_opt<T: ToString + Send + 'static>(
        name: impl Into<Cow<'static, str>>, value: Option<T>,
    ) -> Self {
        Self {
            name: name.into(),
            value: value.map(|x| Box::new(x) as Box<dyn ToString + Send + 'static>),
            desc: None,
        }
    }

    pub(crate) fn with_desc(&mut self, desc: impl Into<Cow<'static, str>>) {
        self.desc = Some(desc.into());
    }
}

#[cfg(test)]
#[test]
fn test_new_meta_value() {
    let x = MetaValue::new("name", 1);
    assert_eq!(
        "MetaValue { name: \"name\", value: Some(\"1\") }",
        format!("{:?}", x)
    );
}
impl typescript_type_def::TypeDef for MetaValue {
    const INFO: typescript_type_def::type_expr::TypeInfo =
        typescript_type_def::type_expr::TypeInfo::Defined(
            typescript_type_def::type_expr::DefinedTypeInfo {
                def: typescript_type_def::type_expr::TypeDefinition {
                    docs: None,
                    path: &[],
                    name: typescript_type_def::type_expr::Ident("MetaValue"),
                    generic_vars: &[],
                    def: typescript_type_def::type_expr::TypeExpr::Object(
                        typescript_type_def::type_expr::TypeObject {
                            docs: None,
                            index_signature: None,
                            fields: &[
                                typescript_type_def::type_expr::ObjectField {
                                    docs: None,
                                    name: typescript_type_def::type_expr::TypeString {
                                        docs: None,
                                        value: "name",
                                    },
                                    optional: false,
                                    r#type: typescript_type_def::type_expr::TypeExpr::Name(
                                        typescript_type_def::type_expr::TypeName::ident(
                                            typescript_type_def::type_expr::Ident("string"),
                                        ),
                                    ),
                                },
                                typescript_type_def::type_expr::ObjectField {
                                    docs: None,
                                    name: typescript_type_def::type_expr::TypeString {
                                        docs: None,
                                        value: "value",
                                    },
                                    optional: false,
                                    r#type: typescript_type_def::type_expr::TypeExpr::Name(
                                        typescript_type_def::type_expr::TypeName::ident(
                                            typescript_type_def::type_expr::Ident("string"),
                                        ),
                                    ),
                                },
                            ],
                        },
                    ),
                },
                generic_args: &[],
            },
        );
}

impl serde::Serialize for MetaValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_struct("MetaValue", 2)?;
        s.serialize_field("name", &self.name)?;
        match &self.value {
            Some(value) => s.serialize_field("value", &value.to_string())?,
            None => s.serialize_field("value", "")?,
        }

        s.end()
    }
}

impl std::fmt::Debug for MetaValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[derive(Debug)]
        #[allow(dead_code)]
        pub struct MetaValue {
            name: Cow<'static, str>,
            value: Option<String>,
        }

        let m = MetaValue {
            name: self.name.clone(),
            value: match &self.value {
                Some(x) => Some(x.to_string()),
                None => None,
            },
        };
        write!(f, "{:?}", m)
    }
}
#[derive(Debug, serde::Serialize, TypeDef, Clone)]
pub struct ListProcedure {
    pub(crate) name: String,
}

impl From<String> for ListProcedure {
    fn from(value: String) -> Self {
        ListProcedure { name: value }
    }
}

#[derive(Debug, serde::Serialize, TypeDef, Clone)]
pub struct ListUdf {
    pub(crate) name: String,
}

impl From<String> for ListUdf {
    fn from(value: String) -> Self {
        ListUdf { name: value }
    }
}

/// almost copied from https://docs.rs/sqlformat/0.2.4/sqlformat/struct.FormatOptions.html

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TypeDef)]
pub struct FormatSqlOpts {
    pub indent: FormatSqlOptsIndent,
    pub uppercase: bool,
    pub lines_between_queries: u8,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TypeDef)]
#[serde(tag = "ident", rename_all = "lowercase")]
pub enum FormatSqlOptsIndent {
    Spaces(u8),
    Tabs,
}

impl Into<sqlformat::FormatOptions> for FormatSqlOpts {
    fn into(self) -> sqlformat::FormatOptions {
        sqlformat::FormatOptions {
            indent: match self.indent {
                FormatSqlOptsIndent::Spaces(x) => sqlformat::Indent::Spaces(x),
                FormatSqlOptsIndent::Tabs => sqlformat::Indent::Tabs,
            },
            uppercase: self.uppercase,
            lines_between_queries: self.lines_between_queries,
        }
    }
}

pub trait FormatSql {
    fn format_sql(&self, sql: &str, ops: Option<FormatSqlOpts>) -> anyhow::Result<String>;
}
#[async_trait::async_trait]

pub trait CheckConn {
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
pub trait QuerySql {
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
}

/// api can be more easy used with frontend,reis backend may impl
/// for more easy usage
#[async_trait::async_trait]
pub trait QuerySqlWithoutOpenConn {
    async fn query_sql(&self, _sql: &str) -> anyhow::Result<Vec<Result<QueryResp, String>>>;
}

#[derive(Clone, Debug, serde::Serialize, TypeDef)]
pub struct ListView {
    pub name: String,
}

impl From<String> for ListView {
    fn from(value: String) -> Self {
        let value = value.into();
        Self { name: value }
    }
}
impl From<&str> for ListView {
    fn from(value: &str) -> Self {
        let value = value.to_string();
        Self { name: value }
    }
}

#[derive(Debug, Clone, serde::Serialize, TypeDef)]
pub struct ListTable {
    pub name: String,
}
impl From<String> for ListTable {
    fn from(value: String) -> Self {
        Self { name: value }
    }
}
impl From<&str> for ListTable {
    fn from(value: &str) -> Self {
        Self {
            name: value.to_string(),
        }
    }
}

/// get server metas info
#[async_trait::async_trait]
pub trait ServerMetas {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>>;
}

/// get current timestamp
pub fn timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();
    let timestamp = now
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();
    timestamp
}
#[derive(Clone, Debug, serde::Serialize, TypeDef, Default)]
pub struct QueryLog {
    logs: Vec<QueryLogEvent>,
}

/// use single instance to manage this.
impl QueryLog {
    fn get_instance() -> Arc<DashMap<uuid::Uuid, Arc<Mutex<QueryLog>>>> {
        static LOGS: std::sync::OnceLock<Arc<DashMap<uuid::Uuid, Arc<Mutex<QueryLog>>>>> =
            std::sync::OnceLock::new();
        LOGS.get_or_init(|| Default::default()).clone()
    }

    pub(crate) fn get_guard(id: uuid::Uuid, sql: impl Into<String>) -> QueryLogGuard {
        let log = QueryLog::get_instance().entry(id).or_default().clone();
        QueryLogGuard::new(sql, log)
    }

    pub(crate) fn remove(id: &uuid::Uuid) {
        QueryLog::get_instance().remove(id);
    }
}

impl QueryLog {
    pub(crate) fn append(&mut self, sql: impl Into<String>, success: bool) {
        self.logs.push(QueryLogEvent::new(sql, success));
    }
}

#[derive(Clone, Debug, serde::Serialize, TypeDef)]
pub struct QueryLogEvent {
    timestamp: u64,
    sql: String,
    success: bool,
}

impl QueryLogEvent {
    pub(crate) fn new(sql: impl Into<String>, success: bool) -> Self {
        Self {
            timestamp: timestamp(),
            sql: sql.into(),
            success,
        }
    }
}

pub struct QueryLogGuard {
    success: bool,
    sql: String,
    dest: Arc<std::sync::Mutex<QueryLog>>,
}

impl QueryLogGuard {
    fn new(sql: impl Into<String>, dest: Arc<std::sync::Mutex<QueryLog>>) -> Self {
        Self {
            success: Default::default(),
            sql: sql.into(),
            dest,
        }
    }
    pub(crate) fn set_success(&mut self, success: bool) {
        self.success = success;
    }
}
impl Drop for QueryLogGuard {
    fn drop(&mut self) {
        self.dest
            .lock()
            .unwrap()
            .append(self.sql.clone(), self.success)
    }
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

/// front can execute a sequence of sql.
/// this represent a single query and a kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlSeq {
    pub sql: String,
    pub kind: SqlKind,
}
impl SqlSeq {
    pub fn new(sql: impl Into<String>, kind: SqlKind) -> Self {
        let sql = sql.into();
        Self { sql, kind }
    }
}

impl std::fmt::Display for SqlSeq {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "kind:{:?} sql:{}", self.kind, self.sql)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SqlKind {
    Query,
    Execution,
    /// start transaction.
    Begin,
    /// commit
    Commit,
    /// rollback
    Rollback,
}

impl SqlKind {
    pub(crate) fn get_from_statement(s: &Statement) -> anyhow::Result<Self> {
        let x = if is_statement_begin(s) {
            SqlKind::Begin
        } else if is_statement_commit(s) {
            SqlKind::Commit
        } else if is_statement_rollback(s)? {
            SqlKind::Rollback
        } else if is_statement_absolute_query(s) {
            SqlKind::Query
        } else {
            SqlKind::Execution
        };

        anyhow::Ok(x)
    }

    pub(crate) fn get_from_sql(sql: &str) -> anyhow::Result<Self> {
        let t = sql.to_uppercase();
        if t.starts_with("BEGIN") || (t.starts_with("START") && t.contains("TRANSACTION")) {
            return anyhow::Ok(SqlKind::Begin);
        }
        if t.starts_with("COMMIT") {
            return anyhow::Ok(SqlKind::Commit);
        }
        if t.starts_with("ROLLBACK") {
            return anyhow::Ok(SqlKind::Rollback);
        }

        anyhow::Ok(
            if t.starts_with("SELECT") || t.starts_with("SHOW") || t.starts_with("EXPLAIN") {
                SqlKind::Query
            } else {
                SqlKind::Execution
            },
        )
    }
}

/// cut statement into smaller base on statement or semi_colon.
pub fn sql_to_sql_seq(
    sql: &str, dialect: &dyn Dialect,
    statement_kind_stmt: impl Fn(&Statement) -> anyhow::Result<SqlKind>,
    statement_kind_sql: impl Fn(&str) -> anyhow::Result<SqlKind>,
) -> anyhow::Result<Vec<SqlSeq>> {
    fn get_sql(
        lineindex: &line_index::LineIndex, sql: &str, start: Location, end: Option<Location>,
    ) -> String {
        let start_offset = lineindex
            .offset(LineCol {
                line: (start.line - 1) as u32,
                col: (start.column - 1) as u32,
            })
            .unwrap();

        let start_offset = usize::from(start_offset);

        let end_offset = end
            .map(|x| {
                {
                    lineindex.offset(LineCol {
                        line: (x.line - 1) as u32,
                        col: (x.column - 1) as u32,
                    })
                }
                .unwrap()
            })
            .unwrap_or(TextSize::from(sql.len() as u32));
        let end_offset = usize::from(end_offset);

        let s = sql[start_offset..end_offset].to_string();
        s.trim().to_string()
    }

    /// cut statement into smaller base on statement.
    fn cut_sql_to_each_statement_ast(
        sql: &str, dialect: &dyn Dialect,
        statement_kind: impl Fn(&Statement) -> anyhow::Result<SqlKind>,
    ) -> anyhow::Result<Vec<SqlSeq>> {
        let lines = line_index::LineIndex::new(sql);
        let parser = sqlparser::parser::Parser::new(dialect);
        let mut parser = parser.try_with_sql(sql)?;
        let mut ret = Vec::new();
        while parser.peek_token().token != Token::EOF {
            while Token::SemiColon == parser.peek_token() {
                parser.next_token();
            }
            if parser.peek_token() == Token::EOF {
                break;
            }
            let start = parser.peek_token();
            let stmt = parser.parse_statement()?;
            let kind = statement_kind(&stmt)?;
            let sql = get_sql(&lines, sql, start.location, {
                let t = parser.peek_token();
                if t.token == Token::EOF {
                    None
                } else {
                    Some(t.location)
                }
            });
            ret.push(SqlSeq::new(sql, kind));
        }
        anyhow::Ok(ret)
    }
    match cut_sql_to_each_statement_ast(sql, dialect, statement_kind_stmt) {
        Ok(x) => return Ok(x),
        Err(_) => {}
    }
    // cut sql by semicolon
    fn cut_sql_to_each_statement_semi_colon(
        sql: &str, dialect: &dyn Dialect, statement_kind: impl Fn(&str) -> anyhow::Result<SqlKind>,
    ) -> anyhow::Result<Vec<SqlSeq>> {
        let lines = line_index::LineIndex::new(sql);
        let tokens = sqlparser::tokenizer::Tokenizer::new(dialect, sql).tokenize_with_location()?;
        if tokens.len() == 0 {
            anyhow::bail!("empty sql.");
        }

        let mut start = &tokens[0];
        let mut ret = Vec::new();
        for index in 1..tokens.len() {
            let t = &tokens[index];
            if t.token == Token::SemiColon || t.token == Token::EOF {
                let sql = get_sql(
                    &lines,
                    sql,
                    start.location,
                    if t.token == Token::EOF {
                        None
                    } else {
                        Some(t.location)
                    },
                );
                let kind = statement_kind(sql.as_str())?;
                if sql != ";" && !sql.is_empty() {
                    ret.push(SqlSeq::new(sql, kind));
                }
                if index + 1 < tokens.len() {
                    start = &tokens[index + 1];
                }
            }
        }
        anyhow::Ok(ret)
    }
    cut_sql_to_each_statement_semi_colon(sql, dialect, statement_kind_sql)
}

fn is_statement_absolute_query(s: &Statement) -> bool {
    match s {
        Statement::Query(_) => true,
        Statement::Fetch {
            name: _,
            direction: _,
            into: _,
        } => true,
        Statement::ShowFunctions { filter: _ } => true,
        Statement::ShowVariable { variable: _ } => true,
        Statement::ShowVariables {
            filter: _,
            global: _,
            session: _,
        } => true,
        Statement::ShowCreate {
            obj_type: _,
            obj_name: _,
        } => true,
        Statement::ShowColumns {
            extended: _,
            full: _,
            table_name: _,
            filter: _,
        } => true,
        Statement::ShowTables {
            extended: _,
            full: _,
            db_name: _,
            filter: _,
        } => true,
        Statement::ShowCollation { filter: _ } => true,
        Statement::ExplainTable {
            describe_alias: _,
            table_name: _,
            hive_format: _,
        } => true,
        Statement::Explain {
            describe_alias: _,
            analyze: _,
            verbose: _,
            statement: _,
            format: _,
        } => true,
        _ => false,
    }
}

pub fn format_data_time_fix_offset(x: &DateTime<FixedOffset>) -> String {
    format!("{} {}", x.naive_local(), x.offset())
}

#[derive(Clone, Debug, Copy, serde::Deserialize, serde::Serialize, TypeDef)]
#[serde(tag = "export_kind", rename_all = "lowercase")]
pub enum ExportKind {
    CSV {
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

/// create csv file list of string
/// each line of line separator
pub(crate) fn create_csv(ds: &Vec<Vec<Option<String>>>, delimiter: char) -> String {
    let mut s = String::new();
    for d in ds.iter() {
        let mut s2 = String::new();
        let last = d.len() - 1;
        for (index, x) in d.iter().enumerate() {
            if let Some(x) = x {
                s2.push_str(&escape_csv(x, delimiter));
            }

            if index != last {
                s2.push(delimiter);
            }
        }
        s.push_str(s2.as_str());
        s.push_str("\n");
    }
    s
}

fn escape_csv(s: &str, delimiter: char) -> Cow<'_, str> {
    if s.contains(delimiter) {
        let s = s.replace("\"", "\\\"");
        Cow::Owned(format!("\"{}\"", s))
    } else {
        Cow::Borrowed(s)
    }
}

#[cfg(test)]
#[test]
fn test_escape_csv() {
    assert_eq!(escape_csv("\" aaaa,", ','), "\"\\\" aaaa,\"");
}

fn is_statement_begin(s: &Statement) -> bool {
    match s {
        Statement::StartTransaction {
            modes: _,
            begin: _,
            modifier: _,
        } => true,
        _ => false,
    }
}

fn is_statement_commit(s: &Statement) -> bool {
    match s {
        Statement::Commit { chain: _ } => true,
        _ => false,
    }
}

fn is_statement_rollback(s: &Statement) -> anyhow::Result<bool> {
    match s {
        Statement::Rollback {
            chain: _,
            savepoint,
        } => {
            if savepoint.is_some() {
                anyhow::bail!("save point is not supported")
            } else {
                anyhow::Ok(true)
            }
        }
        _ => anyhow::Ok(false),
    }
}

pub struct OffsetLimitIter<Index = usize> {
    pub(crate) total: Index,
    pub(crate) offset: Index,
    pub(crate) size: Index,
}

impl<T: Default> OffsetLimitIter<T> {
    pub(crate) fn new(total: T, size: T) -> Self {
        Self {
            offset: Default::default(),
            total,
            size,
        }
    }
}

impl<T: std::ops::Add + std::ops::AddAssign + Copy + std::cmp::Ord> Iterator
    for OffsetLimitIter<T>
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset < self.total {
            let offset = self.offset;
            self.offset += self.size;
            Some(offset)
        } else {
            None
        }
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

impl From<String> for ListTrigger {
    fn from(value: String) -> Self {
        Self { name: value }
    }
}
#[derive(Debug, Clone, serde::Serialize, TypeDef)]
pub struct ListTrigger {
    pub name: String,
}
#[derive(Debug, Clone, Copy, serde::Serialize, TypeDef, PartialEq, Eq)]
pub enum TriggerTiming {
    Before,
    After,
    /// sql server have this timing but I am not quit understand.
    InsteadOf,
}

#[cfg(test)]
#[test]
fn test_off_iter() {
    assert_eq!(
        OffsetLimitIter::new(9, 3).collect::<Vec<_>>(),
        vec![0, 3, 6]
    );
    assert_eq!(
        OffsetLimitIter::new(9, 4).collect::<Vec<_>>(),
        vec![0, 4, 8]
    );
}

/// format sql routine for crate `sqlformat`.
pub(crate) fn sqlformat_format_sql(sql: &str, op: Option<sqlformat::FormatOptions>) -> String {
    sqlformat::format(sql, &sqlformat::QueryParams::None, op.unwrap_or_default())
}

#[cfg(test)]
#[test]
fn test_sqlformat_format_sql() {
    assert_eq!(sqlformat_format_sql("select 1", None), "select\n  1");
}
