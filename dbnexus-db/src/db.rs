use std::borrow::Cow;
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::language_server::LanguageServer;
use chrono::{DateTime, FixedOffset};
use tokio::io::AsyncWriteExt;
use tokio::task::spawn_blocking;
use typescript_type_def::TypeDef;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackendKind {
    Mysql,
    Tidb,
    Mariadb,
    LibSql,
    PostGres,
    CockRoach,
    SqlServer,
    Mongo,
    Redis,
    Sqlite,
    Cassandra,
    ClickHouse,
    Firebird,
    Oracle,
    DuckDB,
}

impl BackendKind {
    pub fn mysql_like(self) -> bool {
        matches!(self, Self::Mariadb | Self::Tidb | Self::Mysql)
    }

    // 支持这样的语法 @@或者@访问一些全局或者局部的变量
    pub(crate) fn support_at(self) -> bool {
        self.mysql_like() || self == Self::SqlServer
    }
    pub(crate) fn postgres_like(self) -> bool {
        matches!(self, Self::PostGres | Self::CockRoach)
    }

    pub(crate) fn quote_style(self) -> &'static str {
        if self.mysql_like() || self == Self::ClickHouse {
            "`"
        } else {
            "\""
        }
    }

    #[cfg(test)]
    pub(crate) fn make_test_xlsx_file(self, name: &str) -> String {
        let env = std::env::temp_dir();
        let filepath = env.join(format!("{}_{}_export.xlsx", self, name));
        let filepath = filepath.as_path().to_str().unwrap();
        filepath.to_string()
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

impl std::fmt::Display for BackendKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub trait GetBackendKind {
    fn backend_name(&self) -> BackendKind;
}
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

#[derive(serde::Deserialize, Clone, Debug, Default, TypeDef)]
pub struct ShowTableParameter {
    #[serde(default)]
    pub where_str: Option<String>,
    #[serde(default)]
    pub order_by: Option<String>,
    #[serde(default)]
    pub page: usize,
    #[serde(default)]
    pub page_size: usize,
    // for backend don't support page like cassandra
    #[serde(default)]
    pub limit: Option<usize>,
    // right now only used for redis zset
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub allow_filtering: bool,
}

impl ShowTableParameter {
    #[cfg(test)]
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
                let x = x.as_mut().unwrap();
                x.data.extend(data.into_iter());
            }
            _ => {
                println!("event:{:?}", event);
            }
        };
        anyhow::Ok(())
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
    + ReadOnlyTrait
    + ExportQueryResult
{
    fn as_any(&self) -> &dyn std::any::Any;

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        self.duplicate_table(old, new).await?;
        self.drop_table(old, false).await?;
        anyhow::Ok(())
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()>;
    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>>;
    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>>;
    async fn show_table(
        &self, table_name: &str, _: ShowTableParameter, _: QueryEventEmitter,
    ) -> anyhow::Result<()>;

    async fn show_table_collect(
        &self, table_name: &str, para: ShowTableParameter,
    ) -> anyhow::Result<QueryResp> {
        let result = QueryRespMemoryCollector::default();
        let result_clone = result.clone();
        self.show_table(table_name, para, Arc::new(result_clone))
            .await?;
        anyhow::Ok(result.result())
    }
    async fn show_view_collect(
        &self, table_name: &str, para: ShowTableParameter,
    ) -> anyhow::Result<QueryResp> {
        let result = QueryRespMemoryCollector::default();
        let result_clone = result.clone();
        self.show_view(table_name, para, Arc::new(result_clone))
            .await?;
        anyhow::Ok(result.result())
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet>;
    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet>;

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()>;

    async fn table_delete(
        &self, _table_name: &str, _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet>;

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        not_support_feature(self.backend_name())
    }
    async fn drop_view(&self, _view_name: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }
    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()>;

    fn clone(&self) -> Box<dyn SchemaTrait>;

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>>;

    async fn drop_table_column(&self, _table: &str, _col: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }
    async fn modify_table_column(
        &self, _table: &str, _old_name: &str, _new_name: &str, _typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }
    async fn add_table_column(
        &self, _table: &str, _name: &str, _col: ColumnType,
    ) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn rename_index(
        &self, _table_name: &str, _oldname: &str, _newname: &str,
    ) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn show_indexes(&self, _table_name: &str) -> anyhow::Result<Vec<Index>> {
        not_support_feature(self.backend_name())
    }

    async fn create_index(
        &self, _table: &str, _name: &str, _kind: IndexKind, _fields: &[String],
    ) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn drop_index(&self, _table_name: &str, _index: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListTable>> {
        not_support_feature(self.backend_name())
    }

    async fn drop_materialized_view(&self, _name: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn export_table_or_view(
        &self, _name: &str, _kind: ExportKind,
        _sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn show_view(
        &self, _view_name: &str, _para: ShowTableParameter, _: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
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
        &self, table_name: &str, filepath: &str, kind: ExportKind, emit: ExportedSizeEmitter,
    ) -> anyhow::Result<()> {
        if !self.table_exists(table_name).await? {
            anyhow::bail!("table not exists")
        }
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        let h1 = {
            let x = SchemaTrait::clone(self);
            let table_name = table_name.to_string();
            let h1 =
                tokio::spawn(
                    async move { x.export_table_or_view(&table_name, kind, sender).await },
                );

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

    async fn create_table_and_index_sql(
        &self, _table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        not_support_feature(self.backend_name())
    }

    async fn create_view_sql(
        &self, _view_name: &str, _materialized: bool,
    ) -> anyhow::Result<String> {
        not_support_feature(self.backend_name())
    }

    async fn list_trigger(&self, _table_name: &str) -> anyhow::Result<Vec<NamedStaff>> {
        not_support_feature(self.backend_name())
    }
    async fn show_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<TriggerDetail> {
        not_support_feature(self.backend_name())
    }
    async fn drop_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn disable_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }
    async fn enable_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        not_support_feature(self.backend_name())
    }

    async fn show_udf(
        &self, _name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<DefAndMetas> {
        not_support_feature(self.backend_name())
    }

    async fn drop_udf(
        &self, _name: &str, _extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        not_support_feature(self.backend_name())
    }
    async fn show_procedure(
        &self, _name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<ProcedureDetail> {
        not_support_feature(self.backend_name())
    }
    async fn drop_procedure(
        &self, _name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
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
        not_support_feature(self.backend_name())
    }

    async fn drop_created_type(&self, _name: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn export_table_to_string(
        &self, table_name: &str, kind: ExportKind,
    ) -> anyhow::Result<String> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
        let h1 = {
            let x = SchemaTrait::clone(self);
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

    async fn list_sequence(&self) -> anyhow::Result<Vec<SequenceDetail>> {
        not_support_feature(self.backend_name())
    }
    async fn rename_sequence(&self, _old: &str, _new: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }

    async fn drop_sequence(&self, _name: &str, _cascade: bool) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }
    fn list_sql_current_values(&self) -> &'static [&'static str] {
        &[]
    }
    async fn list_event(&self) -> anyhow::Result<Vec<SqlEvent>> {
        anyhow::Ok(vec![])
    }
    async fn drop_event(&self, _name: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }
}

#[derive(Debug, typescript_type_def::TypeDef, serde::Serialize)]
pub struct SqlEvent {
    pub name: String,
    pub sql: String,
    pub metas: Vec<MetaValue>,
}
pub trait SqlBuilder: GetBackendKind {
    // like current_timestamp can directly use in sql
    const CURRENT_VALUES: &'static [&'static str] = &[];

    fn hit_current_value(&self, value: Option<&str>) -> bool {
        value
            .map(|x| {
                Self::CURRENT_VALUES
                    .iter()
                    .any(|y| x.eq_ignore_ascii_case(y))
            })
            .unwrap_or(false)
    }

    fn type_support_compare(&self, _typ: &str) -> bool {
        true
    }

    fn quote(&self, name: &str) -> String {
        format!(
            "{}{}{}",
            self.backend_name().quote_style(),
            name,
            self.backend_name().quote_style()
        )
    }

    /// format a value make it suitable for insert into sql.
    /// this method current use in `insert_sql` etc.
    fn put_value_in_sql(
        &self, _typ: &str, _: Option<i64>, _value: Option<&str>,
    ) -> anyhow::Result<String>;

    fn make_where(
        &self, fs: &[Field], wheres: &HashMap<String, Option<String>>,
        field_by_name: &HashMap<&String, &Field>,
    ) -> anyhow::Result<String> {
        // if contains primary key should use primary key to delete
        // otherwise should use all value
        // to do operation.
        let contains_pri = fs.iter().any(|x| x.is_primary);
        let mut sql = String::new();

        let mut first = true;
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
            sql.push_str(
                format!(
                    "{} {} = {}",
                    if !first { " AND " } else { "" },
                    self.quote(k),
                    self.put_value_in_sql(
                        &f.data_type,
                        f.firebird_binary_sub_type,
                        v.as_ref().map(|x| x.as_str())
                    )?
                )
                .as_str(),
            );
            first = false;
        }
        anyhow::Ok(sql)
    }

    fn delete_sql(
        &self, schema: Option<&str>, table_name: &str, fs: &[Field],
        wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
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
            format!("{}", self.quote(table_name))
        );

        sql.push_str(self.make_where(fs, wheres, &field_by_name)?.as_str());
        log::info!("delete sql:{}", sql);
        anyhow::Ok(sql)
    }

    fn update_sql(
        &self, schema: Option<&str>, table_name: &str, fs: &[Field],
        values: &HashMap<String, Option<String>>, wheres: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        //

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
            format!("{}", self.quote(table_name))
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
                        self.quote(k),
                        self.put_value_in_sql(
                            &f.option_must_be_some()?.data_type,
                            f.option_must_be_some()?.firebird_binary_sub_type,
                            v.as_ref().map(|x| x.as_str())
                        )?
                    )
                    .as_str(),
                );
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
        &self, schema: Option<&str>, table_name: &str, fs: &[Field],
        values: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        #[allow(clippy::format_in_format_args)]
        let mut sql = format!(
            "INSERT INTO {}{}(",
            if let Some(schema) = schema.as_ref() {
                format!("{}.", self.quote(schema))
            } else {
                "".to_string()
            },
            format!("{}", self.quote(table_name))
        );
        let skip_null_value = |f: &Field| {
            values.get(&f.field).map(|x| x.is_none()).unwrap_or(true) && f.default_value.is_some()
        };
        //

        {
            let mut first = true;
            for f in fs.iter() {
                if f.is_auto_increment {
                    continue;
                }
                if skip_null_value(f) {
                    // value is null and has default value
                    continue;
                }
                if !first {
                    sql.push(',');
                }
                sql.push_str(self.quote(f.field.as_str()).to_string().as_str());
                first = false;
            }
            sql.push(')');
        }
        // values
        sql.push_str(" VALUES (");
        {
            let mut first = true;
            for f in fs.iter() {
                if f.is_auto_increment {
                    continue;
                }
                if skip_null_value(f) {
                    // value is null and has default value
                    continue;
                }
                if !first {
                    sql.push(',');
                }
                let value = values.get(&f.field).cloned().flatten();
                sql.push_str(&self.put_value_in_sql(
                    &f.data_type,
                    f.firebird_binary_sub_type,
                    value.as_deref(),
                )?);
                first = false;
            }
            sql.push(')');
        }
        anyhow::Ok(sql)
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, typescript_type_def::TypeDef)]
pub struct CreatedTypeDef {
    pub name: String,
    pub sql: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, TypeDef)]
pub struct ColumnType {
    // type string like `varchar`
    pub(crate) typ: String,
    pub(crate) nullness: Option<String>,
    // mysql add column can have this field
    pub(crate) after: Option<String>,

    pub(crate) default_value: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, TypeDef)]
#[serde(tag = "extra", rename_all = "lowercase")]
pub enum UdfOrProcedureExtraParameter {
    PostGres {
        parameters: String, // a integer , b integer
    },
    Cassandra {
        parameters: Vec<String>, //
    },
}

impl UdfOrProcedureExtraParameter {
    pub(crate) fn wrong_type(&self) -> anyhow::Error {
        anyhow::Error::msg(format!("wrong type of parameter passed:{:?}", self))
    }
}

pub fn not_support_feature<T>(name: BackendKind) -> anyhow::Result<T> {
    anyhow::bail!(format!("'{}' don't support this feature", name))
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
    + ReadOnlyTrait
    + ExportQueryResult
{
    async fn list_database(&self) -> anyhow::Result<Vec<String>>;
    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>>;
    async fn create_database(
        &self, name: &str, extra: Option<CreateDatabaseExtra>,
    ) -> anyhow::Result<()>;
    async fn drop_database(&self, name: &str) -> anyhow::Result<()>;
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, TypeDef)]
#[serde(tag = "extra", rename_all = "lowercase")]
pub enum CreateDatabaseExtra {}

/// named staff maybe used for view udf etc...
#[derive(Debug, serde::Serialize, TypeDef)]
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
#[derive(Clone, serde::Serialize, TypeDef, serde::Deserialize)]
pub struct CreateTableAndIndexSql {
    pub create_table: String,

    /// some backend can query create table and index in one sql
    /// this can be empty
    pub indexes: Vec<String>,
}

#[async_trait::async_trait]
pub trait DBTrait:
    Sync
    + Send
    + QuerySql
    + 'static
    + CheckConn
    + Debug
    + ServerMetas
    + ReadOnlyTrait
    + ExportQueryResult
{
    async fn list_schema(&self) -> anyhow::Result<Vec<String>>;
    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>>;
    async fn create_schema(
        &self, name: &str, extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()>;
    async fn drop_schema(&self, name: &str) -> anyhow::Result<()>;
    fn as_any(&self) -> &dyn std::any::Any;
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "extra", rename_all = "lowercase")]
pub enum CreateSchemaExtra {
    // CREATE KEYSPACE my_keyspace
    // WITH REPLICATION = {
    //     'class': 'SimpleStrategy',
    //     'replication_factor': 3
    // };
    Cassandra {
        // class is the keyword for frontend
        replication_strategy: String,
        replication_factor: u32,
    },
}

#[derive(Clone, Debug, TypeDef, serde::Serialize)]
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
    pub content: String,
}

impl std::fmt::Display for XlsxEdit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.row, self.column, self.content)
    }
}

impl From<Cow<'static, str>> for XlsxEdit {
    fn from(value: Cow<'static, str>) -> Self {
        let v: Vec<_> = value.splitn(3, ':').collect();
        XlsxEdit {
            row: v[0].parse().expect("must be parseable"),
            column: v[1].parse().expect("must be parseable"),
            content: v[2].to_string(),
        }
    }
}

pub(crate) async fn export_query_as_xlsx_send_data(
    row_num: &mut u32, data: Vec<Vec<Option<String>>>,
    sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
) -> anyhow::Result<()> {
    for data in data {
        for (col, d) in data.into_iter().enumerate() {
            if let Some(d) = d {
                sender
                    .send(
                        XlsxEdit {
                            row: *row_num,
                            column: col as rust_xlsxwriter::ColNum,
                            content: d,
                        }
                        .to_string()
                        .into(),
                    )
                    .await?;
            }
        }
        *row_num += 1;
    }
    anyhow::Ok(())
}

pub(crate) fn export_query_as_xlsx_send_blocking_data(
    row_num: &mut u32, data: Vec<Vec<Option<String>>>,
    sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
) -> anyhow::Result<()> {
    for data in data {
        for (col, d) in data.into_iter().enumerate() {
            if let Some(d) = d {
                sender.blocking_send(
                    XlsxEdit {
                        row: *row_num,
                        column: col as rust_xlsxwriter::ColNum,
                        content: d,
                    }
                    .to_string()
                    .into(),
                )?;
            }
        }
        *row_num += 1;
    }
    anyhow::Ok(())
}

#[async_trait::async_trait]
pub trait ExportQueryResult: GetBackendKind + Send + Sync + 'static {
    fn clone(&self) -> Box<dyn ExportQueryResult>;

    async fn export_query_result(
        &self, query_id: &str, _sql: &str, _kind: ExportKind,
        _sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()>;

    async fn export_query_result_to_string(
        &self, query_id: &str, sql: &str, kind: ExportKind,
    ) -> anyhow::Result<String> {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(10);
        let j1 = {
            let x = ExportQueryResult::clone(self);
            let sql = sql.to_string();
            let query_id = query_id.to_string();
            let h1 =
                tokio::spawn(
                    async move { x.export_query_result(&query_id, &sql, kind, sender).await },
                );
            h1
        };
        let j2 = {
            tokio::spawn(async move {
                let mut ret = String::new();
                while let Some(r) = receiver.recv().await {
                    ret.push_str(r.deref());
                }
                anyhow::Ok(ret)
            })
        };
        j1.await??;
        j2.await?
    }
    async fn export_query_result_to_file(
        &self, query_id: &str, sql: &str, kind: ExportKind, file: &str, emit: ExportedSizeEmitter,
    ) -> anyhow::Result<()> {
        log::info!("sql:{}", sql);
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        let j1 = {
            let x = self.clone();
            let sql = sql.to_string();
            let query_id = query_id.to_string();
            tokio::spawn(async move { x.export_query_result(&query_id, &sql, kind, sender).await })
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

async fn write_export_file(
    kind: ExportKind, filepath: &str, mut receiver: tokio::sync::mpsc::Receiver<Cow<'static, str>>,
    title: &str, emit: ExportedSizeEmitter,
) -> anyhow::Result<()> {
    let mut size = TimeElapsed::new();
    let r = match kind {
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
            let mut total = 0;

            while let Some(data) = receiver.recv().await {
                file.write_all(data.as_bytes()).await?;
                total += data.len();
                if size.elapsed() {
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
        ExportKind::Xlsx { .. } => {
            let mut total = 0;
            let filepath = filepath.to_string();
            let emit = emit.clone();
            spawn_blocking(move || {
                let mut workbook = rust_xlsxwriter::workbook::Workbook::new();
                // Add a worksheet to the workbook.
                let worksheet = workbook.add_worksheet_with_constant_memory();
                while let Some(edit) = receiver.blocking_recv() {
                    let edit = XlsxEdit::from(edit);
                    total += edit.content.len();
                    if size.elapsed() {
                        emit(total)
                    }
                    worksheet.write_string(edit.row, edit.column, edit.content)?;
                }
                workbook.save(filepath)?;
                emit(total);
                anyhow::Ok(())
            })
            .await?
        }
        _ => {
            let mut total = 0;
            let mut file = tokio::fs::File::create(&filepath).await?;
            while let Some(data) = receiver.recv().await {
                file.write_all(data.as_bytes()).await?;
                total += data.len();
                if size.elapsed() {
                    emit(total)
                }
            }
            emit(0);
            anyhow::Ok(())
        }
    };
    r
}

#[async_trait::async_trait]
pub trait QuerySql: GetBackendKind + Send + Sync + 'static + LanguageServer {
    async fn get_auto_commit(&self, _query_id: &str) -> anyhow::Result<bool> {
        not_support_feature(self.backend_name())
    }

    fn query_buffer_size(&self) -> usize {
        if cfg!(debug_assertions) { 77 } else { 1000 }
    }

    async fn set_auto_commit(&self, _query_id: &str, _commit: bool) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }
    async fn commit(&self, _query_id: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }
    async fn rollback(&self, _query_id: &str) -> anyhow::Result<()> {
        not_support_feature(self.backend_name())
    }
    // `max_row` max returning row size.
    // this is tricky when this implementation is total async rust
    // u don't this this var `emit` will try limit the returning results and cancel the task
    // and return a error
    async fn query_sql(
        &self, query_id: &str, _sql: &str, max_row: usize, emit: QueryEventEmitter,
    ) -> anyhow::Result<()>;

    async fn execute_sql(
        &self, query_id: &str, _sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>>;
    async fn execute_sql_no_open_conn(
        &self, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>>;

    async fn open_connection(&self, id: &str) -> anyhow::Result<()>;

    async fn close_connection(&self, id: &str) -> anyhow::Result<()>;
}

#[derive(Debug, serde::Serialize, TypeDef)]
pub struct ListTable {
    pub name: String,
    pub size: Option<u64>,
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
    pub(crate) fn new(name: impl Into<String>, size: Option<u64>) -> Self {
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

#[derive(Clone, Debug, serde::Serialize, TypeDef)]
pub struct SqlExecuteRet {
    pub sql: String,
    // some backend can't get the changes result
    pub changes: Option<u64>,
}

impl From<(String, u64)> for SqlExecuteRet {
    fn from((sql, changes): (String, u64)) -> Self {
        Self {
            sql,
            changes: Some(changes),
        }
    }
}
impl From<u64> for SqlExecuteRet {
    fn from(changes: u64) -> Self {
        Self {
            sql: "".to_string(),
            changes: Some(changes),
        }
    }
}
#[derive(Debug, Clone, serde::Serialize, TypeDef)]
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

#[derive(Clone, serde::Serialize, Copy, Debug, TypeDef)]
pub enum IndexKind {
    Index,
    Unique,
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
            total: 0,
            ..Default::default()
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

#[derive(Clone, Debug, Default)]
pub struct QueryResp {
    pub fields: Vec<Field>,
    // only pagination need this parameter.
    pub total: usize,
    pub data: Vec<Vec<Option<String>>>,
    pub sql: Option<String>,
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
        fields: Vec<Field>, total: usize, data: Vec<Vec<Option<String>>>,
        sql: impl Into<Option<String>>,
    ) -> anyhow::Result<Self> {
        let sql = sql.into();
        let ret = Self {
            fields,
            total,
            sql,
            data,
            ..Default::default()
        };
        ret.check()?;
        anyhow::Ok(ret)
    }

    pub fn result(sql: impl Into<Option<String>>, result: Vec<Option<String>>) -> Self {
        let sql = sql.into();
        Self {
            total: result.len(),
            sql,
            data: vec![result],
            fields: vec![Field {
                field: "result".to_string(),
                ..Default::default()
            }],
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, TypeDef, PartialEq, Eq)]
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
    pub firebird_binary_sub_type: Option<i64>,
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
            firebird_binary_sub_type: None,
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
        name: impl Into<String>, typ: impl Into<String>, nullable: bool,
        default_value: Option<String>, is_primary: bool, is_auto_increment: bool,
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
            firebird_binary_sub_type: None,
        }
    }
}

pub fn format_data_time_fix_offset(x: &DateTime<FixedOffset>) -> String {
    format!("{} {}", x.naive_local(), x.offset())
}

#[derive(Clone, Debug, Copy, serde::Deserialize, serde::Serialize, TypeDef)]
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
    },
    Html {
        include_header: bool,
    },
}

impl ExportKind {
    pub(crate) fn cannot_query_result_export_as_sql() -> anyhow::Result<()> {
        anyhow::bail!("can't export sql result as sql")
    }
}

#[derive(Debug, serde::Serialize, TypeDef)]
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
        name: &str, timing: TriggerTiming, event: impl Into<TriggerEvent>, sql: impl Into<String>,
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

#[derive(Debug, Clone, TypeDef, PartialEq, Eq, serde::Serialize)]
pub enum TriggerEvent {
    Insert,
    Update,
    Delete,
    InsteadOf,
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

#[derive(Debug, Clone, serde::Serialize, TypeDef, PartialEq, Eq)]
pub enum TriggerTiming {
    Before,
    After,
    InsteadOf,
    String(String),
}

pub(crate) fn ty_is_int(x: &sqlparser::ast::DataType) -> bool {
    use sqlparser::ast::DataType;
    matches!(
        x,
        DataType::TinyInt(_)
            | DataType::TinyIntUnsigned(_)
            | DataType::Int2(_)
            | DataType::Int2Unsigned(_)
            | DataType::SmallInt(_)
            | DataType::SmallIntUnsigned(_)
            | DataType::MediumInt(_)
            | DataType::MediumIntUnsigned(_)
            | DataType::Int(_)
            | DataType::Int4(_)
            | DataType::Int8(_)
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::Int128
            | DataType::Int256
            | DataType::Integer(_)
            | DataType::IntUnsigned(_)
            | DataType::Int4Unsigned(_)
            | DataType::UnsignedInteger
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::UInt128
            | DataType::UInt256
            | DataType::BigInt(_)
            | DataType::BigIntUnsigned(_)
            | DataType::Int8Unsigned(_)
    )
}

/// meta value can used in various place.
#[derive(Debug, serde::Serialize, TypeDef)]
pub struct MetaValue {
    pub name: String,
    pub value: Option<String>,
    pub desc: Option<String>,
}

impl MetaValue {
    pub(crate) fn new(name: impl Into<String>, value: impl Into<Option<String>>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
            desc: None,
        }
    }

    pub(crate) fn with_desc(&mut self, desc: impl Into<String>) {
        self.desc = Some(desc.into());
    }
}

// float decimal all consider as float number
pub(crate) fn ty_is_float(x: &sqlparser::ast::DataType) -> bool {
    use sqlparser::ast::DataType;

    matches!(
        x,
        DataType::Numeric(_)
            | DataType::Decimal(_)
            | DataType::BigNumeric(_)
            | DataType::BigDecimal(_)
            | DataType::Dec(_)
            | DataType::Float(_)
            | DataType::Float4
            | DataType::Float32
            | DataType::Float64
            | DataType::Real
            | DataType::Float8
            | DataType::Double(_)
            | DataType::DoublePrecision
    )
}

pub fn escape_sql_str(s: &str) -> String {
    s.replace("'", "''")
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
    pub(crate) fn get(&self) -> String {
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

impl TimeElapsed {
    pub(crate) fn new() -> Self {
        Self {
            d: Duration::from_millis(100),
            ins: std::time::Instant::now(),
        }
    }
    pub(crate) fn elapsed(&mut self) -> bool {
        let e = self.ins.elapsed() > self.d;
        if e {
            self.ins = std::time::Instant::now();
        }
        e
    }
}

pub(crate) fn create_html_table_header(x: &[String]) -> String {
    format!(
        r#"
        <tr>
            {}
        </tr>
    "#,
        x.iter()
            .map(|x| format!("<th>{}</th>", v_htmlescape::escape(x)))
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

pub(crate) fn create_html_table_data(data: &[Vec<Option<String>>]) -> String {
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
                        v_htmlescape::escape(x.as_ref().map(|x| x.as_str()).unwrap_or_default())
                    )
                })
                .collect::<Vec<_>>()
                .join("")
        );
        ret.push_str(x.as_str());
    }
    ret
}

pub type QueryEventEmitter = Arc<dyn QueryEventEmit>;

pub trait QueryEventEmit: Sync + Send + 'static {
    fn send(&self, event: QueryEvent) -> anyhow::Result<()>;
}

#[derive(Debug, Default)]
pub struct TestQueryEventEmit {}

impl QueryEventEmit for TestQueryEventEmit {
    fn send(&self, _event: QueryEvent) -> anyhow::Result<()> {
        println!("query event:{:?}", _event);
        anyhow::Ok(())
    }
}

#[derive(Debug)]
pub enum QueryEvent {
    Resp(QueryResp),
    Data(Vec<Vec<Option<String>>>),
    Status(String),
    Err(String),
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

pub fn build_limit_sql(sql: &str, max: usize) -> String {
    format!("select * from ({})  limit {} ", sql, max)
}

pub(crate) const POOL_MAX_CONNECTIONS: u32 = 200;

#[cfg(test)]
mod tests {
    use super::UdfOrProcedureExtraParameter;
    #[test]
    fn test_serde_udf_or_procedure_extra_parameter() {
        let x = Some(UdfOrProcedureExtraParameter::PostGres {
            parameters: "int".to_string(),
        });
        let s = serde_json::to_string_pretty(&x).unwrap();
        println!("{:?}", s);
    }

    #[test]
    fn xxx() {
        let d = anyhow::Error::msg("haha ");

        println!("{}", d.backtrace());
    }
}
