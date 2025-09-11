pub mod app;
pub mod handlers;

#[cfg(test)]
mod tests {
    use dbnexus_db::datasource_config::{CassandraDBConfig, EnableSsl, LibSqlDBConfigSouce};
    use dbnexus_db::db::{
        ColumnType, CreateTableAndIndexSql, CreatedTypeDef, QueryResp, SequenceDetail,
        ShowExtraArgs,
    };

    #[test]
    fn generate_ts_def() {
        use std::io::prelude::*;
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
        use crate::handlers::MuiDataGridData;

        let dir = env!("CARGO_MANIFEST_DIR");
        let filepath = Path::new(&dir)
            .to_path_buf()
            .join("../src/comp/ApiStruct.ts");

        let ts_module = {
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
                &MuiDataGridData::INFO,
                &CassandraDBConfig::INFO,
                &LibSqlDBConfigSouce::INFO,
                &CreateTableAndIndexSql::INFO,
                &EnableSsl::INFO,
                &ColumnType::INFO,
                &QueryResp::INFO,
                &SequenceDetail::INFO,
                &CreatedTypeDef::INFO,
                &ShowExtraArgs::INFO,
            ];

            let mut buf = Vec::new();
            let options: DefinitionFileOptions = DefinitionFileOptions::default();
            write_definition_file_from_type_infos(&mut buf, options, &api).unwrap();
            buf
        };
        let mut ops = std::fs::OpenOptions::new();
        ops.write(true);
        ops.truncate(true);
        ops.create(true);

        let mut file = ops.open(filepath.as_path()).unwrap();
        file.write_all(&ts_module).unwrap();
        println!("generate OK.")
    }
}
