pub mod app;
pub mod handlers;

#[cfg(test)]
mod tests {
    use dbnexus_db::datasource_config::{
        CassandraDBConfig, CouchBaseDBConfig, Db2Config, LibSqlDBConfigSource, MySqlSslMode,
        MysqlSsl, OdbcCommon, PgSsl, PgSslMode, SSLFiles,
    };
    use dbnexus_db::db::{
        ColumnType, CouchBaseBucketType, CreateDatabaseExtra, CreateIndexColumn, CreateIndexReq,
        CreateTableAndIndexSql, CreateTableReq, CreateTableReqColumn, CreatedTypeDef, IndexKind,
        SequenceDetail, ShowTableParameter, SqlEvent, SqlNullAble, UdfOrProcedureExtraParameter,
    };
    use dbnexus_db::language_server::CompletionList;

    use crate::app::{OpenAIReq, SetZoom};
    #[test]
    fn generate_ts_def() {
        use crate::app::{self};
        use dbnexus_db::datasource_config::{
            MongoDBConfig, MysqlDBConfig, PGDBConfig, RedisDBConfig, SqlServerDBConfig,
        };
        use dbnexus_db::db::{
            DefAndMetas, ExportKind, Field, Index, ListTable, NamedStaff, ProcedureDetail,
            TriggerDetail, TriggerEnable, TriggerEvent, TriggerTiming,
        };
        use dbnexus_db::language_server::{CompletionItem, CompletionKind};
        use dbnexus_db::redis::RedisListTable;
        use std::path::Path;
        use typescript_type_def::{
            write_definition_file_from_type_infos, DefinitionFileOptions, TypeDef,
        };
        let dir = env!("CARGO_MANIFEST_DIR");
        let filepath = Path::new(&dir)
            .to_path_buf()
            .join("../src/comp/ApiStruct.ts");

        let mut ops = std::fs::OpenOptions::new();
        ops.write(true);
        ops.truncate(true);
        ops.create(true);

        let mut file = ops.open(filepath.as_path()).unwrap();

        let api = vec![
            &app::DBConnectionConfig::INFO,
            &PGDBConfig::INFO,
            &SqlServerDBConfig::INFO,
            &ListTable::INFO,
            &Field::INFO,
            &Index::INFO,
            &NamedStaff::INFO,
            &ExportKind::INFO,
            &RedisListTable::INFO,
            &TriggerDetail::INFO,
            &TriggerEvent::INFO,
            &TriggerTiming::INFO,
            &TriggerEnable::INFO,
            &CompletionItem::INFO,
            &CompletionKind::INFO,
            &DefAndMetas::INFO,
            &ProcedureDetail::INFO,
            &RedisDBConfig::INFO,
            &MysqlDBConfig::INFO,
            &MongoDBConfig::INFO,
            &CassandraDBConfig::INFO,
            &LibSqlDBConfigSource::INFO,
            &CreateTableAndIndexSql::INFO,
            &PgSsl::INFO,
            &MysqlSsl::INFO,
            &ColumnType::INFO,
            &SequenceDetail::INFO,
            &CreatedTypeDef::INFO,
            &SqlEvent::INFO,
            &CompletionList::INFO,
            &PgSslMode::INFO,
            &MySqlSslMode::INFO,
            &SSLFiles::INFO,
            &UdfOrProcedureExtraParameter::INFO,
            &ShowTableParameter::INFO,
            &Db2Config::INFO,
            &CreateTableReq::INFO,
            &CreateTableReqColumn::INFO,
            &CreateIndexColumn::INFO,
            &OdbcCommon::INFO,
            &CreateIndexReq::INFO,
            &IndexKind::INFO,
            &SqlNullAble::INFO,
            &CouchBaseDBConfig::INFO,
            &CreateDatabaseExtra::INFO,
            &CouchBaseBucketType::INFO,
            &OpenAIReq::INFO,
            &SetZoom::INFO,
        ];
        let options: DefinitionFileOptions = DefinitionFileOptions::default();
        write_definition_file_from_type_infos(&mut file, options, &api).unwrap();

        println!("generate OK.")
    }
}
