pub mod app;
pub mod handlers;

#[cfg(test)]
mod tests {
    use dbnexus_db::datasource_config::{
        CassandraDBConfig, LibSqlDBConfigSource, MySqlSslMode, MysqlSsl, PgSsl, PgSslMode, SSLFiles,
    };
    use dbnexus_db::db::{
        ColumnType, CreateTableAndIndexSql, CreatedTypeDef, SequenceDetail, ShowTableExtraArgs,
        SqlEvent, UdfOrProcedureExtraParameter,
    };
    use dbnexus_db::language_server::CompletionList;

    #[test]
    fn generate_ts_def() {
        use std::path::Path;

        use dbnexus_db::datasource_config::{
            MongoDBConfig, MysqlDBConfig, PGDBConfig, RedisDBConfig, SqlServerDBConfig,
        };
        use dbnexus_db::db::{
            DefAndMetas, ExportKind, Field, Index, ListTable, NamedStaff, ProcedureDetail,
            SqlExecuteRet, TriggerDetail, TriggerEnable, TriggerEvent, TriggerTiming,
        };
        use dbnexus_db::language_server::{CompletionItem, CompletionKind};
        use dbnexus_db::redis::RedisListTable;
        use typescript_type_def::{
            DefinitionFileOptions, TypeDef, write_definition_file_from_type_infos,
        };

        use crate::app::{self};
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
            &SqlExecuteRet::INFO,
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
            &ShowTableExtraArgs::INFO,
            &SqlEvent::INFO,
            &CompletionList::INFO,
            &PgSslMode::INFO,
            &MySqlSslMode::INFO,
            &SSLFiles::INFO,
            &UdfOrProcedureExtraParameter::INFO,
        ];

        let options: DefinitionFileOptions = DefinitionFileOptions::default();
        write_definition_file_from_type_infos(&mut file, options, &api).unwrap();
        println!("generate OK.")
    }
}
