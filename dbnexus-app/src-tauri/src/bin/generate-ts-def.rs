// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::prelude::*;
use std::path::Path;

use dbnexus_app_lib::handlers::{MuiDataGridData, MuiDataTableListResp};
use dbnexus_db::completion::{CompletionItem, CompletionKind};
use dbnexus_db::datasource_config::{
    MongoConfig, MysqlDbConfig, PGDbConfig, RedisDbConfig, SqlServerDbConfig,
};
use dbnexus_db::db::{
    ExportKind, Field, FormatSqlOpts, FormatSqlOptsIndent, Index, ListTable, NamedStaff,
    ProcedureDetail, SqlExecuteRet, TriggerDetail, TriggerEnable, TriggerEvent, TriggerTiming,
    UdfDetail,
};

use dbnexus_db::app::{self, OpendKind};
use dbnexus_db::redis::RedisListTable;
use typescript_type_def::{write_definition_file_from_type_infos, DefinitionFileOptions, TypeDef};

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let filepath = Path::new(&dir)
        .to_path_buf()
        .join("../src/comp/ApiStruct.ts");

    let ts_module = {
        let api = vec![
            &app::DBConnectionConfig::INFO,
            &app::UserSettings::INFO,
            &PGDbConfig::INFO,
            &SqlServerDbConfig::INFO,
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
            &OpendKind::INFO,
            &FormatSqlOpts::INFO,
            &FormatSqlOptsIndent::INFO,
            &RedisDbConfig::INFO,
            &MysqlDbConfig::INFO,
            &MongoConfig::INFO,
            &MuiDataTableListResp::INFO,
            &MuiDataGridData::INFO,
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
