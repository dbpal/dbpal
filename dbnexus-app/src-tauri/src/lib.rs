pub mod handlers;

#[cfg(test)]
mod tests {
    use dbnexus_db::datasource_config::CassandraDBConfig;

    #[test]
    fn generate_ts_def() {
        use std::io::prelude::*;
        use std::path::Path;

        use crate::handlers::{MuiDataGridData, MuiDataTableListResp};
        use dbnexus_db::app::{self};
        use dbnexus_db::datasource_config::{
            MongoDBConfig, MysqlDBConfig, PGDBConfig, RedisDBConfig, SqlServerDBConfig,
        };
        use dbnexus_db::db::{
            ExportKind, Field, FormatSqlOpts, FormatSqlOptsIndent, Index, ListTable, NamedStaff,
            ProcedureDetail, SqlExecuteRet, TriggerDetail, TriggerEnable, TriggerEvent,
            TriggerTiming, UdfDetail,
        };
        use dbnexus_db::language_server::{CompletionItem, CompletionKind, SemanticToken};
        use dbnexus_db::query_log::QueryLogEvent;
        use dbnexus_db::redis::RedisListTable;
        use typescript_type_def::{
            DefinitionFileOptions, TypeDef, write_definition_file_from_type_infos,
        };

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
                &UdfDetail::INFO,
                &ProcedureDetail::INFO,
                &FormatSqlOpts::INFO,
                &FormatSqlOptsIndent::INFO,
                &RedisDBConfig::INFO,
                &MysqlDBConfig::INFO,
                &MongoDBConfig::INFO,
                &MuiDataTableListResp::INFO,
                &MuiDataGridData::INFO,
                &QueryLogEvent::INFO,
                &SemanticToken::INFO,
                &CassandraDBConfig::INFO,
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

    #[test]
    fn generate_keywords() {
        use std::env;
        use std::io::Write;
        use std::path::Path;

        use dbnexus_db::db::monaco_editor_keyword_regexp;
        use dbnexus_db::language_server::LexerCompletionTrait;
        use dbnexus_db::mssql::MsSqlServer;
        use dbnexus_db::mysql::MysqlDB;
        use dbnexus_db::oracle::OracleDB;
        use dbnexus_db::pg::PostGresDB;
        use dbnexus_db::redis_lang_server::REDIS_COMMANDS;
        use dbnexus_db::sqlite::SqliteDB;
        use dbnexus_db::tidb::TiDB;

        fn compose(name: &str, content: &str) -> String {
            return format!("const {} = {};", name, content);
        }

        let exe = env::current_exe().unwrap();

        let dir = env!("CARGO_MANIFEST_DIR");
        let filepath = Path::new(&dir)
            .to_path_buf()
            .join("../src/comp/keywords.ts");

        let mut ops = std::fs::OpenOptions::new();
        ops.write(true);
        ops.truncate(true);
        ops.create(true);
        let mut file = ops.open(filepath).unwrap();

        file.write(format!("// generate by {:?}\n", exe).as_bytes())
            .unwrap();
        file.write(format!("// don't edit!!!\n").as_bytes())
            .unwrap();
        // redis
        {
            let content = monaco_editor_keyword_regexp(REDIS_COMMANDS);
            file.write(compose("redisKeywords", &content).as_bytes())
                .unwrap();
            file.write("\n".as_bytes()).unwrap();
        }
        // mysql
        {
            let content = MysqlDB::monaco_editor_keyword_regexp();
            file.write(compose("mysqlKeywords", &content).as_bytes())
                .unwrap();
            file.write("\n".as_bytes()).unwrap();
        }

        // mssql
        {
            let content = MsSqlServer::monaco_editor_keyword_regexp();
            file.write(compose("mssqlKeywords", &content).as_bytes())
                .unwrap();
            file.write("\n".as_bytes()).unwrap();
        }
        // postgres
        {
            let content = PostGresDB::monaco_editor_keyword_regexp();
            file.write(compose("postgresKeywords", &content).as_bytes())
                .unwrap();
            file.write("\n".as_bytes()).unwrap();
        }
        // sqlite

        {
            let content = SqliteDB::monaco_editor_keyword_regexp();
            file.write(compose("sqliteKeywords", &content).as_bytes())
                .unwrap();
            file.write("\n".as_bytes()).unwrap();
        }
        // oracle

        {
            let content = OracleDB::monaco_editor_keyword_regexp();
            file.write(compose("oracleKeywords", &content).as_bytes())
                .unwrap();
            file.write("\n".as_bytes()).unwrap();
        }
        // tidb
        {
            let content = TiDB::monaco_editor_keyword_regexp();
            file.write(compose("tidbKeywords", &content).as_bytes())
                .unwrap();
            file.write("\n".as_bytes()).unwrap();
        }

        // TODO add more
        file.write(
        r#"export { redisKeywords , mysqlKeywords  , mssqlKeywords , postgresKeywords , sqliteKeywords , oracleKeywords ,tidbKeywords }"#.as_bytes(),
    )
    .unwrap();
    }
}
