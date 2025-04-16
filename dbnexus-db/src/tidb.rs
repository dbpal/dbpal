use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use crate::datasource_config::MysqlDBConfig;
use crate::db::{
    BackendName, CheckConn, ColumnType, CreateSchemaExtra, CreateTableAndIndexSql, DBTrait,
    DefAndMetas, ExportKind, ExportQueryResult, Field, Index, IndexKind, ListTable, NamedStaff,
    ProcedureDetail, QueryResp, QuerySql, QuoteStyle, ReadOnlyTrait, SchemaTrait, ServerMetas,
    ShowExtraArgs, SqlBuilder, SqlExecuteRet, TriggerDetail, UdfOrProcedureExtraParameter,
};
use crate::meta_value::MetaValue;
use crate::mysql::MysqlDB;
use crate::query_log::QueryLogEvent;

#[derive(Clone)]
pub struct TiDB {
    pub(crate) db: Arc<MysqlDB>,
}

impl BackendName for TiDB {
    fn backend_name(&self) -> &str {
        "tidb"
    }
}

impl TiDB {
    pub async fn new(config: MysqlDBConfig) -> anyhow::Result<Self> {
        let db = Self {
            db: Arc::new(MysqlDB::new(config).await?),
        };
        anyhow::Ok(db)
    }
}

#[async_trait::async_trait]
impl CheckConn for TiDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        self.db.check_conn().await
    }
}

impl std::fmt::Debug for TiDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.db)
    }
}
#[async_trait::async_trait]

impl ExportQueryResult for TiDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }
    async fn export_query_result(
        &self, sql: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        self.db.export_query_result(sql, kind, sender).await
    }
}

#[async_trait::async_trait]
impl QuerySql for TiDB {
    fn get_query_log(&self, id: uuid::Uuid) -> anyhow::Result<Vec<QueryLogEvent>> {
        self.db.get_query_log(id)
    }
    async fn execute_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        self.db.execute_sql(query_id, sql).await
    }
    async fn query_sql(
        &self, query_id: uuid::Uuid, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        self.db.query_sql(query_id, sql).await
    }

    async fn open_connection(&self) -> anyhow::Result<uuid::Uuid> {
        self.db.open_connection().await
    }

    async fn re_connect(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.db.re_connect(id).await
    }

    async fn close_connection(&self, id: uuid::Uuid) -> anyhow::Result<()> {
        self.db.close_connection(id).await
    }

    fn is_dangerous_sql(&self, sql: &str) -> anyhow::Result<bool> {
        self.db.is_dangerous_sql(sql)
    }
}

impl QuoteStyle for TiDB {
    fn quote_style(&self) -> &'static str {
        self.db.quote_style()
    }
}

#[async_trait::async_trait]
impl SchemaTrait for TiDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        self.db.list_table().await
    }

    async fn show_table_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        self.db.show_table_columns(table_name).await
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        self.db
            .show_table(&table_name, where_str, order_by, page, page_size, extra)
            .await
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        self.db.show_indexes(table_name).await
    }

    async fn rename_index(
        &self, table_name: &str, oldname: &str, newname: &str,
    ) -> anyhow::Result<()> {
        self.db.rename_index(table_name, oldname, newname).await
    }

    async fn drop_index(&self, table_name: &str, index: &str) -> anyhow::Result<()> {
        self.db.drop_index(&table_name, index).await
    }

    async fn create_index(
        &self, table: &str, name: &str, kind: IndexKind, fields: &[String],
    ) -> anyhow::Result<()> {
        self.db.create_index(&table, name, kind, &fields).await
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.db.insert(&table_name, values).await
    }

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.db.update(table_name, values, wheres).await
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        self.db.drop_table_column(table, col).await
    }

    fn list_data_types(&self) -> anyhow::Result<&'static [&'static str]> {
        // TODO TIDB have different types???
        self.db.list_data_types()
    }

    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        self.db
            .modify_table_column(table, &old_name, &new_name, typ)
            .await
    }

    async fn add_table_column(
        &self, table: &str, name: &str, col: ColumnType,
    ) -> anyhow::Result<()> {
        self.db.add_table_column(table, name, col).await
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.db.truncate_table(table_name).await
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.db.delete(table_name, wheres).await
    }

    async fn list_view(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.db.list_view().await
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.db.drop_view(&view_name).await
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        self.db.drop_table(table, cascade).await
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        self.db
            .show_view(view_name, where_str, order_by, page, page_size, None)
            .await
    }

    async fn show_view_columns(&self, view_name: &str) -> anyhow::Result<Vec<Field>> {
        self.db.show_view_columns(view_name).await
    }

    async fn export_table_or_view(
        &self, name: &str, kind: ExportKind, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        self.db.export_table_or_view(name, kind, sender).await
    }

    fn clone(&self) -> Box<dyn SchemaTrait> {
        Box::new(Clone::clone(self))
    }

    async fn create_table_and_index_sql(
        &self, table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        self.db.create_table_and_index_sql(table_name).await
    }

    async fn create_view_sql(&self, view_name: &str, _m: bool) -> anyhow::Result<String> {
        // mysql has no materlized view
        self.db.create_view_sql(view_name, false).await
    }

    async fn list_trigger(&self, table_name: &str) -> anyhow::Result<Vec<NamedStaff>> {
        self.db.list_trigger(table_name).await
    }

    async fn show_trigger(&self, table: &str, name: &str) -> anyhow::Result<TriggerDetail> {
        self.db.show_trigger(table, name).await
    }

    async fn drop_trigger(&self, table: &str, name: &str) -> anyhow::Result<()> {
        self.db.drop_trigger(table, name).await
    }

    async fn disable_trigger(&self, table: &str, name: &str) -> anyhow::Result<()> {
        self.db.disable_trigger(table, name).await
    }

    async fn enable_trigger(&self, table: &str, name: &str) -> anyhow::Result<()> {
        self.db.enable_trigger(table, name).await
    }

    async fn rename_table(&self, old: &str, new: &str) -> anyhow::Result<()> {
        self.db.rename_table(old, new).await
    }

    async fn duplicate_table(&self, table: &str, dup: &str) -> anyhow::Result<()> {
        self.db.duplicate_table(table, dup).await
    }

    async fn list_udf(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.db.list_udf().await
    }

    async fn show_udf(
        &self, name: &str, arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<DefAndMetas> {
        self.db.show_udf(name, arg).await
    }

    async fn drop_udf(
        &self, name: &str, extra: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        self.db.drop_udf(name, extra).await
    }

    async fn list_procedure(&self) -> anyhow::Result<Vec<NamedStaff>> {
        self.db.list_procedure().await
    }

    async fn show_procedure(
        &self, name: &str, arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<ProcedureDetail> {
        self.db.show_procedure(name, arg).await
    }

    async fn drop_procedure(
        &self, name: &str, arg: Option<UdfOrProcedureExtraParameter>,
    ) -> anyhow::Result<()> {
        self.db.drop_procedure(name, arg).await
    }
}

impl SqlBuilder for TiDB {
    fn put_value_in_sql(
        &self, typ: &str, _: Option<i64>, value: Option<&str>,
    ) -> anyhow::Result<String> {
        self.db.put_value_in_sql(typ, None, value)
    }
    fn type_support_compare(&self, typ: &str) -> bool {
        self.db.type_support_compare(typ)
    }
}

impl ReadOnlyTrait for TiDB {
    fn read_only(&self) -> bool {
        self.db.read_only()
    }
}
#[async_trait::async_trait]
impl DBTrait for TiDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        self.db.list_schema().await
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let db = self.db.open_schema(&name).await?;
        let db = Clone::clone(db.as_any().downcast_ref::<MysqlDB>().unwrap());
        anyhow::Ok(Arc::new(db))
    }

    async fn create_schema(
        &self, name: &str, _extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.db.create_schema(name, _extra).await
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.db.drop_schema(&name).await
    }
}

#[async_trait::async_trait]

impl ServerMetas for TiDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        self.db.show_metas().await
    }
}

#[cfg(test)]
mod tests {
    use crate::TEST_HOST;
    use crate::datasource_config::MysqlDBConfig;
    use crate::mysql::test_fns::{self, make_test_object};

    pub(crate) fn make_config() -> MysqlDBConfig {
        MysqlDBConfig {
            host: TEST_HOST.to_string(),
            username: "root".to_string(),
            password: None,
            port: 4000,
            db: Some("test".to_string()),
            read_only: false,
            save_password: true,
            ssl: Default::default(),
        }
    }

    #[ignore = "1105 (HY000): 'CREATE TABLE ... SELECT' is not implemented yet"]
    #[tokio::test]
    async fn test_duplicate() {
        test_fns::test_duplicate(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_server_metas() {
        test_fns::test_server_metas(make_test_object(make_config()).await).await;
    }
    // #[tokio::test]
    // async fn test_procedure() {
    //     test_fns::test_procedure(make_test_object(make_config()).await).await;
    // }
    #[tokio::test]
    async fn test_index() {
        test_fns::test_index(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_check_conn() {
        test_fns::test_check_conn(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_export() {
        test_fns::test_export(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_rename() {
        test_fns::test_rename(make_test_object(make_config()).await).await;
    }
    // 在 TiDB 中，用户定义函数（UDF）不能直接通过 SQL 语句创建。与 MySQL
    // 等其他数据库不同，TiDB 需要通过 C/C++ 编写 UDF，并编译成动态链接库（.so
    // 文件），然后将其注册。这是因为 UDF 的实现涉及底层的 C/C++ 代码，而不是简单的
    // SQL 语句。
    // #[tokio::test]
    // async fn test_udf() {
    //     test_fns::test_udf(make_test_object(make_config()).await).await;
    // }

    #[tokio::test]
    async fn test_types1() {
        test_fns::test_types1(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types2() {
        test_fns::test_types2(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types3() {
        test_fns::test_types3(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types4() {
        test_fns::test_types4(make_test_object(make_config()).await).await;
    }
    // #[tokio::test]
    // async fn test_triggers() {
    //     test_fns::test_triggers(make_test_object(make_config()).await).await;
    // }
    #[tokio::test]
    async fn test_list_database() {
        test_fns::test_list_database(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_query() {
        test_fns::test_query(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_table_column() {
        test_fns::test_table_column(make_test_object(make_config()).await).await;
    }

    #[tokio::test]
    async fn test_view() {
        test_fns::test_view(make_test_object(make_config()).await).await;
    }
}
