use crate::datasource_config::MysqlDBConfig;
use crate::db::{
    BackendKind, CheckConn, ColumnType, CreateSchemaExtra, CreateTableAndIndexSql, DBTrait,
    DefAndMetas, ExportKind, ExportQueryResult, Field, GetBackendKind, Index, IndexKind, ListTable,
    NamedStaff, ProcedureDetail, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait, SequenceDetail,
    ServerMetas, ShowTableExtraArgs, SqlExecuteRet, TriggerDetail, UdfOrProcedureExtraParameter,
    XlsxEdit,
};

use crate::meta_value::MetaValue;
use crate::mysql::MysqlDB;
use sqlx::types::BigDecimal;
use sqlx::{AssertSqlSafe, Row};
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct MariaDB {
    pub(crate) db: Arc<MysqlDB>,
}

impl GetBackendKind for MariaDB {
    fn backend_name(&self) -> BackendKind {
        BackendKind::Mariadb
    }
}

impl MariaDB {
    pub async fn new(config: MysqlDBConfig) -> anyhow::Result<Self> {
        let db = Self {
            db: Arc::new(MysqlDB::new(config).await?),
        };
        anyhow::Ok(db)
    }
}

#[async_trait::async_trait]
impl CheckConn for MariaDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        self.db.check_conn().await
    }
}

impl MariaDB {
    pub(crate) async fn list_sequences_impl(
        &self, schema: &str,
    ) -> anyhow::Result<Vec<SequenceDetail>> {
        let sql = format!(
            "
            select SEQUENCE_NAME , DATA_TYPE,
            START_VALUE, MINIMUM_VALUE ,
            MAXIMUM_VALUE  , INCREMENT
            from information_schema.SEQUENCES where SEQUENCE_SCHEMA = '{}'
            ",
            schema
        );
        let mut conn = self.db.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());

        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            let data_type: String = row.try_get(1)?;
            let start_value: BigDecimal = row.try_get(2)?;
            let min_value: BigDecimal = row.try_get(3)?;
            let max_value: BigDecimal = row.try_get(4)?;
            let increment: i64 = row.try_get(5)?;
            let create_sql = {
                let sql = format!("SHOW CREATE SEQUENCE {name}");
                let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                    .fetch_one(conn.deref_mut())
                    .await?;
                let create_sql: String = row.try_get(1)?;
                create_sql
            };

            ret.push(SequenceDetail {
                name,
                data_type,
                start_value: start_value.to_string(),
                min_value: min_value.to_string(),
                max_value: max_value.to_string(),
                increment_by: increment.to_string(),
                last_value: None, // TODO get last value
                sql: create_sql,
            });
        }
        anyhow::Ok(ret)
    }
}
impl std::fmt::Debug for MariaDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "mariadb@{:?}", self.db.config)
    }
}
#[async_trait::async_trait]

impl ExportQueryResult for MariaDB {
    async fn export_as_xlsx(
        &self, sql: &str, include_header: bool, sender: tokio::sync::mpsc::Sender<XlsxEdit>,
    ) -> anyhow::Result<()> {
        ExportQueryResult::export_as_xlsx(self.db.as_ref(), sql, include_header, sender).await
    }

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
impl QuerySql for MariaDB {
    async fn get_auto_commit(&self, query_id: u32) -> anyhow::Result<bool> {
        self.db.get_auto_commit(query_id).await
    }
    async fn set_auto_commit(&self, query_id: u32, commit: bool) -> anyhow::Result<()> {
        self.db.set_auto_commit(query_id, commit).await
    }
    async fn commit(&self, query_id: u32) -> anyhow::Result<()> {
        self.db.commit(query_id).await
    }
    async fn rollback(&self, query_id: u32) -> anyhow::Result<()> {
        self.db.rollback(query_id).await
    }

    async fn execute_sql_no_open_conn(
        &self, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        self.db.execute_sql_no_open_conn(sql).await
    }
    async fn execute_sql(
        &self, query_id: u32, sql: &str,
    ) -> anyhow::Result<Result<QueryResp, String>> {
        self.db.execute_sql(query_id, sql).await
    }

    async fn query_sql(
        &self, query_id: u32, sql: &str,
    ) -> anyhow::Result<Vec<Result<QueryResp, String>>> {
        self.db.query_sql(query_id, sql).await
    }

    async fn open_connection(&self) -> anyhow::Result<u32> {
        self.db.open_connection().await
    }

    async fn re_connect(&self, id: u32) -> anyhow::Result<()> {
        self.db.re_connect(id).await
    }

    async fn close_connection(&self, id: u32) -> anyhow::Result<()> {
        self.db.close_connection(id).await
    }
}

#[async_trait::async_trait]
impl SchemaTrait for MariaDB {
    async fn export_table_or_view_as_xlsx(
        &self, name: &str, include_header: bool, sender: tokio::sync::mpsc::Sender<XlsxEdit>,
    ) -> anyhow::Result<()> {
        SchemaTrait::export_table_or_view_as_xlsx(self.db.as_ref(), name, include_header, sender)
            .await
    }
    fn list_sql_current_values(&self) -> &'static [&'static str] {
        self.db.list_sql_current_values()
    }
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
        page_size: usize, extra: Option<ShowTableExtraArgs>,
    ) -> anyhow::Result<QueryResp> {
        self.db
            .show_table(table_name, where_str, order_by, page, page_size, extra)
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
        self.db.drop_index(table_name, index).await
    }

    async fn create_index(
        &self, table: &str, name: &str, kind: IndexKind, fields: &[String],
    ) -> anyhow::Result<()> {
        self.db.create_index(table, name, kind, fields).await
    }

    async fn table_insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.db.table_insert(table_name, values).await
    }

    async fn table_update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.db.table_update(table_name, values, wheres).await
    }

    async fn drop_table_column(&self, table: &str, col: &str) -> anyhow::Result<()> {
        self.db.drop_table_column(table, col).await
    }

    fn list_data_types(&self) -> anyhow::Result<&'static [&'static str]> {
        self.db.list_data_types()
    }

    async fn modify_table_column(
        &self, table: &str, old_name: &str, new_name: &str, typ: Option<ColumnType>,
    ) -> anyhow::Result<()> {
        self.db
            .modify_table_column(table, old_name, new_name, typ)
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

    async fn table_delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.db.table_delete(table_name, wheres).await
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        self.db.list_view().await
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.db.drop_view(view_name).await
    }

    async fn drop_table(&self, table: &str, cascade: bool) -> anyhow::Result<()> {
        self.db.drop_table(table, cascade).await
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize, _extra: Option<ShowTableExtraArgs>,
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
        // mysql has no materialized view
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

    async fn list_sequence(&self) -> anyhow::Result<Vec<SequenceDetail>> {
        self.list_sequences_impl(self.db.config.schema_str()).await
    }

    async fn rename_sequence(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let sql = format!("RENAME TABLE {old}  TO {new}");
        let mut conn = self.db.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    async fn drop_sequence(&self, name: &str, _cascade: bool) -> anyhow::Result<()> {
        let sql = format!("DROP SEQUENCE IF EXISTS {name};");
        let mut conn = self.db.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }
}

impl ReadOnlyTrait for MariaDB {
    fn read_only(&self) -> bool {
        self.db.read_only()
    }
}

#[async_trait::async_trait]
impl DBTrait for MariaDB {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        self.db.list_schema().await
    }

    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        let db = self.db.open_schema(name).await?;
        let db = Clone::clone(db.as_any().downcast_ref::<MysqlDB>().unwrap());
        anyhow::Ok(Arc::new(Self { db: Arc::new(db) }))
    }

    async fn create_schema(
        &self, name: &str, _extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.db.create_schema(name, _extra).await
    }

    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.db.drop_schema(name).await
    }
}

#[async_trait::async_trait]

impl ServerMetas for MariaDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        self.db.show_metas().await
    }
}

#[cfg(test)]
pub(crate) fn make_config() -> MysqlDBConfig {
    MysqlDBConfig {
        conn_timeout: Some(5000),
        host: crate::TEST_HOST.to_string(),
        username: "root".to_string(),
        password: Some("root".to_string()),
        port: 3307,
        db: Some("test".to_string()),
        read_only: false,
        save_password: true,
        ssl: Default::default(),
    }
}

#[cfg(test)]
pub(crate) async fn make_test_object_mariadb(config: MysqlDBConfig) -> MariaDB {
    let db: MariaDB = MariaDB::new(config).await.unwrap();
    db
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{QuerySql, SchemaTrait};
    use crate::language_server::LangServerCommon;
    use crate::mariadb::make_config;
    use crate::mysql::test_fns::{self, make_test_object};

    #[tokio::test]
    async fn test_prepare() {
        let db = make_test_object_mariadb(make_config()).await;
        let query_id = db.open_connection().await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            PREPARE stmtxxx FROM 'SELECT 1';
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let ts = db.list_execute(None).await.unwrap();

        assert!(
            ts.iter().any(|x| x.name.eq("stmtxxx")),
            "!!!!!!!!!!!{:?}",
            ts
        );
    }

    #[tokio::test]
    async fn test_sequence() {
        let db = make_test_object_mariadb(make_config()).await;
        let name = "test_seq";
        let query_id = db.open_connection().await.unwrap();
        let _ = db.drop_sequence(name, false).await;

        db.execute_sql(
            query_id,
            r#"
            CREATE SEQUENCE test_seq;
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let sequences = SchemaTrait::list_sequence(&db).await.unwrap();
        for seq in sequences.iter() {
            if seq.name.eq(name) {
                println!("{:?}", seq)
            }
        }
        let name2 = format!("{name}2");
        db.rename_sequence(name, &name2).await.unwrap();
        db.rename_sequence(&name2, name).await.unwrap();
    }

    #[tokio::test]
    async fn test_duplicate() {
        test_fns::test_duplicate(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_transaction() {
        test_fns::test_transaction(make_test_object(make_config()).await).await;
    }

    #[tokio::test]
    async fn test_server_metas() {
        test_fns::test_server_metas(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_procedure() {
        test_fns::test_procedure(make_test_object(make_config()).await).await;
    }
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
    #[tokio::test]
    async fn test_udf() {
        test_fns::test_udf(make_test_object(make_config()).await).await;
    }
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
    #[tokio::test]
    async fn test_triggers() {
        test_fns::test_triggers(make_test_object(make_config()).await).await;
    }
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
    #[tokio::test]
    async fn test_event() {
        test_fns::test_event(make_test_object(make_config()).await).await;
    }
}
