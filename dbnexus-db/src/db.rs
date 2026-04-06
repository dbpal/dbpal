use crate::helper::generate_random_string;
use crate::language_server::LanguageServer;
use chrono::{DateTime, FixedOffset};
use core::str;
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::task::spawn_blocking;
use typescript_type_def::TypeDef;

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
    pub fn mysql_like(self) -> bool {
        matches!(self, Self::Mysql(..))
    }

    // 支持这样的语法 @@或者@访问一些全局或者局部的变量
    pub(crate) fn support_at(self) -> bool {
        self.mysql_like() || self == Self::SqlServer
    }
    pub(crate) fn postgres_like(self) -> bool {
        matches!(self, Self::PostGres(..))
    }

    pub(crate) fn sqlite_like(&self) -> bool {
        matches!(self, Self::Sqlite | Self::LibSql)
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
    #[cfg(test)]
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
{
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

    #[cfg(test)]
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

    #[cfg(test)]
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
        values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = SqlBuilder::insert_sql(self, table_name, &fs, &values)?;
        anyhow::Ok(sql)
    }

    async fn table_insert(
        &self,
        table_name: &str,
        values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        let sql = self.table_insert_sql(table_name, values).await?;
        let r = self.simple_execute_sql(&sql).await?;
        anyhow::Ok(r)
    }

    async fn table_update_sql(
        &self,
        table_name: &str,
        values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = SqlBuilder::update_sql(self, table_name, &fs, &values, &wheres)?;
        anyhow::Ok(sql)
    }

    async fn table_update(
        &self,
        table_name: &str,
        values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        let sql = self.table_update_sql(table_name, values, wheres).await?;
        self.simple_execute_sql(&sql).await
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<Option<String>>;

    async fn table_delete_sql(
        &self,
        table_name: &str,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        let fs = self.show_table_columns(table_name).await?;
        SqlBuilder::delete_sql(self, table_name, &fs, &wheres)
    }

    async fn table_delete(
        &self,
        table_name: &str,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<Option<usize>> {
        let fs = self.show_table_columns(table_name).await?;
        let sql = SqlBuilder::delete_sql(self, table_name, &fs, &wheres)?;
        let ret = self.simple_execute_sql(&sql).await?;
        anyhow::Ok(ret)
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.backend_kind().not_support_feature()
    }
    async fn drop_view(&self, _view_name: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }
    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<Option<String>>;

    fn clone(&self) -> Box<dyn SchemaTrait>;

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>>;

    async fn drop_table_column(&self, _table: &str, _col: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }
    async fn modify_table_column(
        &self,
        _table: &str,
        _old_name: &str,
        _new_name: &str,
        _typ: Option<ColumnType>,
    ) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }
    async fn add_table_column(
        &self,
        _table: &str,
        _name: &str,
        _col: ColumnType,
    ) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }

    async fn rename_index(
        &self,
        _table_name: &str,
        _oldname: &str,
        _newname: &str,
    ) -> anyhow::Result<Option<String>> {
        self.backend_kind().not_support_feature()
    }

    async fn show_indexes(&self, _table_name: &str) -> anyhow::Result<Vec<Index>> {
        self.backend_kind().not_support_feature()
    }

    async fn create_index(&self, _table: &str, _: &CreateIndexReq) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }

    async fn drop_index(&self, _table_name: &str, _index: &str) -> anyhow::Result<Option<String>> {
        self.backend_kind().not_support_feature()
    }

    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.backend_kind().not_support_feature()
    }

    async fn drop_materialized_view(&self, _name: &str) -> anyhow::Result<Option<String>> {
        self.backend_kind().not_support_feature()
    }

    async fn export_table_or_view(
        &self,
        _name: &str,
        _: Option<ShowTableParameter>,
        _kind: ExportKind,
        _sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature()
    }

    async fn show_view(
        &self,
        _view_name: &str,
        _para: ShowTableParameter,
        _: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature()
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
        if !self.table_exists(table_name).await? {
            anyhow::bail!("table not exists")
        }
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        let h1 = {
            let x = SchemaTrait::clone(self);
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
        self.backend_kind().not_support_feature()
    }

    async fn show_view_sql(&self, _view_name: &str, _materialized: bool) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }

    async fn list_trigger(&self, _table_name: &str) -> anyhow::Result<Vec<NamedStaff>> {
        self.backend_kind().not_support_feature()
    }
    async fn show_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<TriggerDetail> {
        self.backend_kind().not_support_feature()
    }
    async fn drop_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }

    async fn disable_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }
    async fn enable_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.backend_kind().not_support_feature()
    }

    async fn show_udf(
        &self,
        _name: &str,
        _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<DefAndMetas> {
        self.backend_kind().not_support_feature()
    }

    async fn drop_udf(
        &self,
        _name: &str,
        _extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.backend_kind().not_support_feature()
    }
    async fn show_procedure(
        &self,
        _name: &str,
        _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<ProcedureDetail> {
        self.backend_kind().not_support_feature()
    }
    async fn drop_procedure(
        &self,
        _name: &str,
        _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
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
        self.backend_kind().not_support_feature()
    }

    async fn drop_created_type(&self, _name: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }

    async fn export_table_to_string(
        &self,
        table_name: &str,
        kind: ExportKind,
    ) -> anyhow::Result<String> {
        let (sender, receiver) = tokio::sync::mpsc::channel(10);
        let h1 = {
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
        self.backend_kind().not_support_feature()
    }
    async fn rename_sequence(&self, _old: &str, _new: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }

    async fn drop_sequence(&self, _name: &str, _cascade: bool) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }
    fn list_sql_current_values(&self) -> &'static [&'static str] {
        &[]
    }
    async fn list_event(&self) -> anyhow::Result<Vec<SqlEvent>> {
        anyhow::Ok(vec![])
    }
    async fn drop_event(&self, _name: &str) -> anyhow::Result<String> {
        self.backend_kind().not_support_feature()
    }
}

#[derive(Debug, typescript_type_def::TypeDef, serde::Serialize)]
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

    fn hit_current_value(&self, value: Option<&str>) -> bool {
        value
            .map(|x| {
                self.get_current_values()
                    .iter()
                    .any(|y| x.eq_ignore_ascii_case(y))
            })
            .unwrap_or(false)
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
    fn put_value_in_sql(&self, _typ: &str, _value: Option<&str>) -> anyhow::Result<String>;

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
                    self.put_value_in_sql(&f.data_type, v.as_ref().map(|x| x.as_str()))?
                )
                .as_str(),
            );
            first = false;
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
            format!("{}", self.quote(table_name))
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
        &self,
        table_name: &str,
        fs: &[Field],
        values: &HashMap<String, Option<String>>,
    ) -> anyhow::Result<String> {
        let schema = self.get_schema();
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
                sql.push_str(&self.put_value_in_sql(&f.data_type, value.as_deref())?);
                first = false;
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
                    SqlNullAble::None => "".to_owned(),
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
                Some(x) => match self.backend_kind() {
                    BackendKind::Cassandra => {
                        anyhow::bail!("cassandra not support default value")
                    }
                    BackendKind::Firebird => {
                        // silently differrent from other database
                        format!("DEFAULT {}", self.put_value_in_sql(data_type, Some(x))?)
                    }
                    _ => format!("DEFAULT({})", self.put_value_in_sql(data_type, Some(x))?),
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

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, typescript_type_def::TypeDef)]
pub struct CreatedTypeDef {
    pub name: String,
    pub sql: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, TypeDef)]
pub struct ColumnType {
    // type string like `varchar`
    pub(crate) typ: String,
    pub(crate) nullable: Option<SqlNullAble>,
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

impl BackendKind {
    pub fn not_support_feature<T>(&self) -> anyhow::Result<T> {
        anyhow::bail!(format!("'{}' don't support this feature", self))
    }
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
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, TypeDef)]
#[serde(tag = "extra", rename_all = "lowercase")]
pub enum CreateDatabaseExtra {
    Couchbase {
        // 在 Couchbase 中，ram_quota_mb 是一个重要的配置参数，用于指定一个桶（bucket）分配的内存大小。这个参数的单位是兆字节（MB），它决定了该桶可以使用的内存量。
        ram_quota_mb: u64,
        // 是否启用 flush 操作。启用 flush 操作可以清空桶中的所有数据。
        flush_enabled: bool,
        // 副本数量，即每个文档的副本数。副本数量必须小于或等于集群中的节点数。
        num_replicas: u32,
        // in seconds
        max_expiry: Option<u64>,
        bucket_type: CouchBaseBucketType,
    },
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, TypeDef)]
#[serde(rename_all = "lowercase")]
pub enum CouchBaseBucketType {
    Couchbase,
    Ephemeral,
}

/// named staff maybe used for view udf etc...
#[derive(Debug, serde::Serialize, TypeDef)]
pub struct NamedStaff {
    // some backend support reload function like postgres
    pub parameters: Option<String>,
    // cassandra support reload and parameter is a `Vec`
    pub cassandra_parameters: Option<Vec<String>>,
    pub name: String,
    pub metas: Vec<MetaValue>,

    pub rand: String,
}

impl From<String> for NamedStaff {
    fn from(value: String) -> Self {
        NamedStaff {
            name: value,
            metas: vec![],
            parameters: None,
            cassandra_parameters: None,
            rand: generate_random_string(10),
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

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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
        write!(f, "{}-{}-{}", self.row, self.column, self.content)
    }
}

pub(crate) async fn export_query_as_xlsx_send_data(
    row_num: &mut u32,
    data: Vec<Option<String>>,
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

pub(crate) fn export_query_as_xlsx_send_blocking_data(
    row_num: &mut u32,
    data: Vec<Option<String>>,
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

#[derive(Debug, Clone, TypeDef, serde::Deserialize, serde::Serialize)]

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

#[derive(Debug, Clone, TypeDef, serde::Deserialize, serde::Serialize, Default)]
pub struct CreateTableReqColumn {
    pub name: String,
    pub data_type: String,
    pub auto_increment: bool,
    pub primary_key: bool,
    pub default_value: Option<String>,
    pub nullable: SqlNullAble,
}

#[derive(Debug, Clone, TypeDef, serde::Deserialize, serde::Serialize, Default, Copy)]
#[serde(rename_all = "lowercase")]
pub enum SqlNullAble {
    #[default]
    None,
    Null,
    NotNull,
}

impl std::fmt::Display for SqlNullAble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                SqlNullAble::None => "",
                SqlNullAble::Null => "NULL",
                SqlNullAble::NotNull => "NOT NULL",
            }
        )
    }
}

#[derive(Debug, Clone, TypeDef, serde::Deserialize, serde::Serialize)]
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

    pub(crate) fn common_sql(
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

#[derive(Debug, Clone, TypeDef, serde::Deserialize, serde::Serialize)]

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
            let mut first = true;
            while let Some(r) = receiver.recv().await {
                let value = match r {
                    ExportFrame::JsonValue(x) => x,
                    _ => {
                        unreachable!()
                    }
                };
                if !first {
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
                first = false;
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
                }
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
    let mut time_elapsed = TimeElapsed::new();
    let mut total = 0;
    let r = match kind {
        ExportKind::Json { pretty } => {
            let mut file = tokio::fs::File::create(&filepath).await?;
            file.write_all("[".as_bytes()).await?;
            let mut first = true;
            while let Some(e) = receiver.recv().await {
                let value = match e {
                    ExportFrame::JsonValue(x) => x,
                    _ => {
                        anyhow::bail!("not json value but:{:?}", e);
                    }
                };
                if !first {
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
                first = false;
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
        ExportKind::Xlsx { .. } => {
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

                    total += edit.content.len();
                    if time_elapsed.elapsed() {
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
        self.backend_kind().not_support_feature()
    }

    async fn set_auto_commit(&self, _query_id: &str, _commit: bool) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature()
    }
    async fn commit(&self, _query_id: &str) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature()
    }
    async fn rollback(&self, _query_id: &str) -> anyhow::Result<()> {
        self.backend_kind().not_support_feature()
    }

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

#[derive(Debug, serde::Serialize, TypeDef, Clone)]
pub struct ListTable {
    pub name: String,
    pub size: Option<u64>,
    // for frontend tree item id
    pub rand: String,
}

impl From<String> for ListTable {
    fn from(value: String) -> Self {
        Self {
            name: value,
            size: None,
            rand: generate_random_string(10),
        }
    }
}

impl ListTable {
    pub(crate) fn new(name: impl Into<String>, size: Option<u64>) -> Self {
        Self {
            name: name.into(),
            size,
            rand: generate_random_string(10),
        }
    }
}

impl From<&str> for ListTable {
    fn from(value: &str) -> Self {
        Self {
            name: value.to_string(),
            size: None,
            rand: generate_random_string(10),
        }
    }
}

/// get server metas info
#[async_trait::async_trait]
pub trait ServerMetas: Sync + Send + 'static {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>>;
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

#[derive(Clone, serde::Serialize, Copy, Debug, TypeDef, serde::Deserialize)]
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
            total: 0,
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
        fields: Vec<Field>,
        total: usize,
        data: Vec<Vec<Option<String>>>,
        sql: impl Into<Option<String>>,
    ) -> anyhow::Result<Self> {
        let sql = sql.into();
        let ret = Self {
            fields,
            total,
            sql,
            data,
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
    MarkDown,
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

/// meta value can used in various place.
#[derive(Debug, serde::Serialize, TypeDef)]
pub struct MetaValue {
    pub name: String,
    pub value: Option<String>,
}

impl MetaValue {
    pub(crate) fn new(name: impl Into<String>, value: impl Into<Option<String>>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
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

impl Default for TimeElapsed {
    fn default() -> Self {
        Self {
            d: Duration::from_millis(100),
            ins: std::time::Instant::now(),
        }
    }
}

impl TimeElapsed {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn elapsed(&mut self) -> bool {
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

pub(crate) fn create_markdown_header(header: &[String]) -> String {
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

pub(crate) fn create_markdown_data(header: &[Option<String>]) -> String {
    let ret = format!(
        "|{}|\n",
        header
            .iter()
            .map(|x| match x {
                Some(x) => x.as_str(),
                None => "",
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

pub(crate) const POOL_MAX_CONNECTIONS: u32 = 200;

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
            },
            ExportKind::Xlsx {
                include_header: false,
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
            let str = self.export_table_to_string(table_name, kind).await.unwrap();

            println!("export:{:?}\n{}", kind, str)
        }
    }
}
