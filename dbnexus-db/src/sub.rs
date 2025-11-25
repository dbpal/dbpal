use std::{borrow::Cow, collections::HashMap, sync::Arc, time::Duration};

use crate::{
    db::{
        BackendKind, CheckConn, ColumnType, CreateTableAndIndexSql, CreatedTypeDef, DefAndMetas,
        ExportKind, ExportQueryResult, ExportedSizeEmitter, Field, GetBackendKind, Index,
        IndexKind, ListTable, MetaValue, NamedStaff, ProcedureDetail, QueryResp, QuerySql,
        ReadOnlyTrait, SchemaTrait, SequenceDetail, ServerMetas, ShowTableExtraArgs, SqlEvent,
        SqlExecuteRet, TriggerDetail, UdfOrProcedureExtraParameter,
    },
    mysql::MysqlDB,
};

#[derive(Clone)]
pub enum SubBackend {
    TiDB(Arc<MysqlDB>),
}

impl SubBackend {
    fn as_schema(&self) -> &dyn SchemaTrait {
        match self {
            SubBackend::TiDB(db) => db.as_ref(),
        }
    }
    fn as_export_query_result(&self) -> &dyn ExportQueryResult {
        match self {
            SubBackend::TiDB(db) => db.as_ref(),
        }
    }

    fn as_query_sql(&self) -> &dyn QuerySql {
        match self {
            SubBackend::TiDB(db) => db.as_ref(),
        }
    }
}

impl GetBackendKind for SubBackend {
    fn backend_name(&self) -> BackendKind {
        match self {
            SubBackend::TiDB(mysql_db) => BackendKind::Tidb,
        }
    }
}

#[async_trait::async_trait]
impl ExportQueryResult for SubBackend {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }

    async fn export_query_result(
        &self, _sql: &str, _kind: ExportKind, _sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        self.as_export_query_result()
            .export_query_result(_sql, _kind, _sender)
            .await
    }
}

impl ReadOnlyTrait for SubBackend {
    fn read_only(&self) -> bool {
        match self {
            SubBackend::TiDB(mysql_db) => mysql_db.read_only(),
        }
    }
}
#[async_trait::async_trait]
impl QuerySql for SubBackend {
    async fn get_auto_commit(&self, query_id: &str) -> anyhow::Result<bool> {
        self.as_query_sql().get_auto_commit(query_id).await
    }
    async fn set_auto_commit(&self, query_id: &str, commit: bool) -> anyhow::Result<()> {
        self.as_query_sql().set_auto_commit(query_id, commit).await
    }
    async fn commit(&self, query_id: &str) -> anyhow::Result<()> {
        self.as_query_sql().commit(query_id).await
    }
    async fn rollback(&self, query_id: &str) -> anyhow::Result<()> {
        self.as_query_sql().rollback(query_id).await
    }

    async fn query_sql(
        &self, query_id: &str, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        self.as_query_sql().query_sql(query_id, sql).await
    }

    async fn execute_sql(
        &self, query_id: &str, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        self.as_query_sql().execute_sql(query_id, sql).await
    }
    async fn execute_sql_no_open_conn(
        &self, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        self.as_query_sql().execute_sql_no_open_conn(sql).await
    }

    async fn open_connection(&self, id: &str) -> anyhow::Result<()> {
        self.as_query_sql().open_connection(id).await
    }

    async fn close_connection(&self, id: &str) -> anyhow::Result<()> {
        self.as_query_sql().close_connection(id).await
    }
}

#[async_trait::async_trait]

impl ServerMetas for SubBackend {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        match self {
            SubBackend::TiDB(db) => db.show_metas().await,
        }
    }
}

#[async_trait::async_trait]

impl CheckConn for SubBackend {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        match self {
            SubBackend::TiDB(db) => db.check_conn().await,
        }
    }
}

impl std::fmt::Debug for SubBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        unimplemented!()
    }
}

#[async_trait::async_trait]
impl SchemaTrait for SubBackend {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        self.as_schema().rename_table(old, new).await
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        self.as_schema().duplicate_table(table, dup).await
    }
    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.as_schema().list_table().await
    }
    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        self.as_schema().show_table_columns(table_name).await
    }
    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, extra: Option<ShowTableExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        self.as_schema()
            .show_table(table_name, where_str, order_by, page, page_size, extra)
            .await
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.as_schema().table_insert(table_name, values).await
    }
    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.as_schema()
            .table_update(table_name, values, wheres)
            .await
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.as_schema().truncate_table(table_name).await
    }

    async fn table_delete(
        &self, _table_name: &str, _wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.as_schema().table_delete(_table_name, _wheres).await
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.as_schema().list_view().await
    }
    async fn drop_view(&self, _view_name: &str) -> anyhow::Result<()> {
        self.as_schema().drop_view(_view_name).await
    }
    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        self.as_schema().drop_table(table, cascade).await
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    fn list_data_types(&self) -> anyhow::Result<&'static [&'static str]> {
        self.as_schema().list_data_types()
    }

    async fn drop_table_column(&self, _table: &str, _col: &str) -> anyhow::Result<()> {
        self.as_schema().drop_table_column(_table, _col).await
    }
    async fn modify_table_column(
        &self, _table: &str, _old_name: &str, _new_name: &str, _typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        self.as_schema()
            .modify_table_column(_table, _old_name, _new_name, _typ)
            .await
    }
    async fn add_table_column(
        &self, _table: &str, _name: &str, _col: ColumnType,
    ) -> anyhow::Result<()> {
        self.as_schema().add_table_column(_table, _name, _col).await
    }

    async fn rename_index(
        &self, _table_name: &str, _oldname: &str, _newname: &str,
    ) -> anyhow::Result<()> {
        self.as_schema()
            .rename_index(_table_name, _oldname, _newname)
            .await
    }

    async fn show_indexes(&self, _table_name: &str) -> anyhow::Result<Vec<Index>> {
        self.as_schema().show_indexes(_table_name).await
    }

    async fn create_index(
        &self, _table: &str, _name: &str, _kind: IndexKind, _fields: &[String],
    ) -> anyhow::Result<()> {
        self.as_schema()
            .create_index(_table, _name, _kind, _fields)
            .await
    }

    async fn drop_index(&self, _table_name: &str, _index: &str) -> anyhow::Result<()> {
        self.as_schema().drop_index(_table_name, _index).await
    }

    async fn list_materialized_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.as_schema().list_materialized_view().await
    }

    async fn drop_materialized_view(&self, _name: &str) -> anyhow::Result<()> {
        self.as_schema().drop_materialized_view(_name).await
    }

    async fn export_table_or_view(
        &self, _name: &str, _kind: ExportKind,
        _sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        self.as_schema()
            .export_table_or_view(_name, _kind, _sender)
            .await
    }

    async fn show_view(
        &self, _view_name: &str, _where_str: Option<&str>, _order_by: Option<&str>, _page: usize,
        _page_size: usize, _extra: Option<ShowTableExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        self.as_schema()
            .show_view(_view_name, _where_str, _order_by, _page, _page_size, _extra)
            .await
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        self.as_schema().show_view_columns(view_name).await
    }

    async fn export_table_or_view_to_file(
        &self, table_name: &str, filepath: &str, kind: ExportKind, emit: ExportedSizeEmitter,
    ) -> anyhow::Result<()> {
        self.as_schema()
            .export_table_or_view_to_file(table_name, filepath, kind, emit)
            .await
    }

    async fn create_table_and_index_sql(
        &self, _table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        self.as_schema()
            .create_table_and_index_sql(_table_name)
            .await
    }

    async fn create_view_sql(
        &self, _view_name: &str, _materialized: bool,
    ) -> anyhow::Result<String> {
        self.as_schema()
            .create_view_sql(_view_name, _materialized)
            .await
    }

    async fn list_trigger(&self, _table_name: &str) -> anyhow::Result<Vec<NamedStaff>> {
        self.as_schema().list_trigger(_table_name).await
    }
    async fn show_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<TriggerDetail> {
        self.as_schema().show_trigger(_table, _name).await
    }
    async fn drop_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<()> {
        self.as_schema().drop_trigger(_table, _name).await
    }

    async fn disable_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<()> {
        self.as_schema().disable_trigger(_table, _name).await
    }
    async fn enable_trigger(&self, _table: &str, _name: &str) -> anyhow::Result<()> {
        self.as_schema().enable_trigger(_table, _name).await
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.as_schema().list_udf().await
    }

    async fn show_udf(
        &self, _name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<DefAndMetas> {
        self.as_schema().show_udf(_name, _arg).await
    }

    async fn drop_udf(
        &self, _name: &str, _extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        self.as_schema().drop_udf(_name, _extra).await
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.as_schema().list_procedure().await
    }
    async fn show_procedure(
        &self, _name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<ProcedureDetail> {
        self.as_schema().show_procedure(_name, _arg).await
    }
    async fn drop_procedure(
        &self, _name: &str, _arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        self.as_schema().drop_procedure(_name, _arg).await
    }

    async fn table_exists(&self, table: &str) -> anyhow::Result<bool> {
        self.as_schema().table_exists(table).await
    }
    async fn list_created_types(&self) -> anyhow::Result<Vec<CreatedTypeDef>> {
        self.as_schema().list_created_types().await
    }

    async fn drop_created_type(&self, _name: &str) -> anyhow::Result<()> {
        self.as_schema().drop_created_type(_name).await
    }

    async fn export_table_to_string(
        &self, table_name: &str, kind: ExportKind,
    ) -> anyhow::Result<String> {
        self.as_schema()
            .export_table_to_string(table_name, kind)
            .await
    }

    async fn list_sequence(&self) -> anyhow::Result<Vec<SequenceDetail>> {
        self.as_schema().list_sequence().await
    }
    async fn rename_sequence(&self, _old: &str, _new: &str) -> anyhow::Result<()> {
        self.as_schema().rename_sequence(_old, _new).await
    }

    async fn drop_sequence(&self, _name: &str, _cascade: bool) -> anyhow::Result<()> {
        self.as_schema().drop_sequence(_name, false).await
    }
    fn list_sql_current_values(&self) -> &'static [&'static str] {
        self.as_schema().list_sql_current_values()
    }
    async fn list_event(&self) -> anyhow::Result<Vec<SqlEvent>> {
        self.as_schema().list_event().await
    }
    async fn drop_event(&self, _name: &str) -> anyhow::Result<()> {
        self.as_schema().drop_event(_name).await
    }
}
