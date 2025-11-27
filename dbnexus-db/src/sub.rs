use crate::{
    datasource_config::{MysqlDBConfig, PGDBConfig},
    db::{
        BackendKind, CatalogTrait, CheckConn, ColumnType, CreateDatabaseExtra, CreateSchemaExtra,
        CreateTableAndIndexSql, CreatedTypeDef, DBTrait, DefAndMetas, ExportKind,
        ExportQueryResult, ExportedSizeEmitter, Field, GetBackendKind, Index, IndexKind, ListTable,
        MetaValue, NamedStaff, ProcedureDetail, QueryResp, QuerySql, ReadOnlyTrait, SchemaTrait,
        SequenceDetail, ServerMetas, ShowTableExtraArgs, SqlEvent, SqlExecuteRet, TriggerDetail,
        UdfOrProcedureExtraParameter,
    },
    mariadb::MariaDB,
    mysql::MysqlDB,
    pg::PostGresDB,
};
use std::{borrow::Cow, collections::HashMap, sync::Arc, time::Duration};

#[derive(Clone)]
pub enum SubDB {
    TiDB(Arc<MysqlDB>),
    MariaDb(Arc<MysqlDB>),
    CocoRoach(Arc<PostGresDB>),
}

impl SubDB {
    pub async fn new_tidb(config: MysqlDBConfig) -> anyhow::Result<Self> {
        let r = MysqlDB::new(config).await?;
        anyhow::Ok(SubDB::TiDB(Arc::new(r)))
    }
    pub async fn new_mariadb(config: MysqlDBConfig) -> anyhow::Result<Self> {
        let r = MysqlDB::new(config).await?;
        anyhow::Ok(SubDB::MariaDb(Arc::new(r)))
    }

    pub async fn new_cockroach(config: PGDBConfig) -> anyhow::Result<Self> {
        let r = PostGresDB::new(config).await?;
        anyhow::Ok(SubDB::CocoRoach(Arc::new(r)))
    }

    fn as_db_trait(&self) -> &dyn DBTrait {
        match self {
            SubDB::TiDB(db) => db.as_ref(),
            SubDB::MariaDb(db) => db.as_ref(),
            SubDB::CocoRoach(db) => db.as_ref(),
        }
    }
}

#[async_trait::async_trait]

impl DBTrait for SubDB {
    async fn list_schema(&self) -> anyhow::Result<Vec<String>> {
        self.as_db_trait().list_schema().await
    }
    async fn open_schema(&self, name: &str) -> anyhow::Result<Arc<dyn SchemaTrait>> {
        match self {
            SubDB::TiDB(db) => {
                let r = db.open_schema(name).await?;
                let r = Clone::clone(r.as_any().downcast_ref::<MysqlDB>().unwrap());
                anyhow::Ok(Arc::new(SubDB::TiDB(Arc::new(r))))
            }
            SubDB::MariaDb(db) => {
                let r = db.open_schema(name).await?;
                let r = Clone::clone(r.as_any().downcast_ref::<MysqlDB>().unwrap());
                anyhow::Ok(Arc::new(SubDB::MariaDb(Arc::new(r))))
            }
            SubDB::CocoRoach(db) => {
                let r = db.open_schema(name).await?;
                let r = Clone::clone(r.as_any().downcast_ref::<PostGresDB>().unwrap());
                anyhow::Ok(Arc::new(SubDB::CocoRoach(Arc::new(r))))
            }
        }
    }

    async fn create_schema(
        &self, name: &str, extra: Option<CreateSchemaExtra>,
    ) -> anyhow::Result<()> {
        self.as_db_trait().create_schema(name, extra).await
    }
    async fn drop_schema(&self, name: &str) -> anyhow::Result<()> {
        self.as_db_trait().drop_schema(name).await
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[async_trait::async_trait]
impl CatalogTrait for SubDB {
    async fn list_database(&self) -> anyhow::Result<Vec<String>> {
        match self {
            SubDB::CocoRoach(db) => db.list_database().await,
            _ => anyhow::bail!("not support"),
        }
    }
    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        match self {
            SubDB::CocoRoach(db) => {
                let r = db.open_db(name).await?;
                let r = Clone::clone(r.as_any().downcast_ref::<PostGresDB>().unwrap());
                anyhow::Ok(Arc::new(SubDB::CocoRoach(Arc::new(r))))
            }
            _ => anyhow::bail!("not support"),
        }
    }

    async fn create_database(
        &self, name: &str, extra: Option<CreateDatabaseExtra>,
    ) -> anyhow::Result<()> {
        match self {
            SubDB::CocoRoach(db) => db.create_database(name, extra).await,
            _ => anyhow::bail!("not support"),
        }
    }
    async fn drop_database(&self, name: &str) -> anyhow::Result<()> {
        match self {
            SubDB::CocoRoach(db) => db.drop_created_type(name).await,
            _ => anyhow::bail!("not support"),
        }
    }
}

impl SubDB {
    fn as_schema(&self) -> &dyn SchemaTrait {
        match self {
            SubDB::TiDB(db) => db.as_ref(),
            SubDB::MariaDb(mysql_db) => mysql_db.as_ref(),
            SubDB::CocoRoach(post_gres_db) => post_gres_db.as_ref(),
        }
    }
    fn as_export_query_result(&self) -> &dyn ExportQueryResult {
        match self {
            SubDB::TiDB(db) => db.as_ref(),
            SubDB::MariaDb(mysql_db) => mysql_db.as_ref(),
            SubDB::CocoRoach(post_gres_db) => post_gres_db.as_ref(),
        }
    }

    fn as_query_sql(&self) -> &dyn QuerySql {
        match self {
            SubDB::TiDB(db) => db.as_ref(),
            SubDB::MariaDb(mysql_db) => mysql_db.as_ref(),
            SubDB::CocoRoach(post_gres_db) => post_gres_db.as_ref(),
        }
    }
}

impl GetBackendKind for SubDB {
    fn backend_name(&self) -> BackendKind {
        match self {
            SubDB::TiDB(..) => BackendKind::Tidb,
            SubDB::MariaDb(..) => BackendKind::Mariadb,
            SubDB::CocoRoach(..) => BackendKind::CockRoach,
        }
    }
}

#[async_trait::async_trait]
impl ExportQueryResult for SubDB {
    fn clone(&self) -> Box<dyn ExportQueryResult> {
        Box::new(Clone::clone(self))
    }

    async fn export_query_result(
        &self, query_id: &str, _sql: &str, _kind: ExportKind,
        _sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        self.as_export_query_result()
            .export_query_result(query_id, _sql, _kind, _sender)
            .await
    }
}

impl ReadOnlyTrait for SubDB {
    fn read_only(&self) -> bool {
        match self {
            SubDB::TiDB(db) => db.read_only(),
            SubDB::MariaDb(db) => db.read_only(),
            SubDB::CocoRoach(db) => db.read_only(),
        }
    }
}

#[async_trait::async_trait]
impl QuerySql for SubDB {
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

impl ServerMetas for SubDB {
    async fn show_metas(&self) -> anyhow::Result<Vec<MetaValue>> {
        match self {
            SubDB::TiDB(db) => db.show_metas().await,
            SubDB::MariaDb(db) => db.show_metas().await,
            SubDB::CocoRoach(db) => db.show_metas().await,
        }
    }
}

#[async_trait::async_trait]

impl CheckConn for SubDB {
    async fn check_conn(&self) -> anyhow::Result<Duration> {
        match self {
            SubDB::TiDB(db) => db.check_conn().await,
            SubDB::MariaDb(db) => db.check_conn().await,
            SubDB::CocoRoach(db) => db.check_conn().await,
        }
    }
}

impl std::fmt::Debug for SubDB {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {:?}", self.backend_name(), self.as_schema())
    }
}

#[async_trait::async_trait]
impl SchemaTrait for SubDB {
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
        match self {
            SubDB::CocoRoach(x) => {
                x.cock_roach_show_table(table_name, where_str, order_by, page, page_size, extra)
                    .await
            }
            _ => {
                self.as_schema()
                    .show_table(table_name, where_str, order_by, page, page_size, extra)
                    .await
            }
        }
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
        match self {
            SubDB::CocoRoach(x) => x.cock_roach_list_view().await,
            _ => self.as_schema().list_view().await,
        }
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

    async fn list_data_types(&self) -> anyhow::Result<Vec<String>> {
        self.as_schema().list_data_types().await
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
        match self {
            SubDB::MariaDb(db) => {
                let t = MariaDB { db: db.clone() };
                t.list_sequence().await
            }
            _ => self.as_schema().list_sequence().await,
        }
    }
    async fn rename_sequence(&self, _old: &str, _new: &str) -> anyhow::Result<()> {
        match self {
            SubDB::MariaDb(db) => {
                let t = MariaDB { db: db.clone() };
                t.rename_sequence(_old, _new).await
            }

            _ => self.as_schema().rename_sequence(_old, _new).await,
        }
    }

    async fn drop_sequence(&self, _name: &str, _cascade: bool) -> anyhow::Result<()> {
        match self {
            SubDB::MariaDb(db) => {
                let t = MariaDB { db: db.clone() };
                t.drop_sequence(_name, false).await
            }
            _ => self.as_schema().drop_sequence(_name, false).await,
        }
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
