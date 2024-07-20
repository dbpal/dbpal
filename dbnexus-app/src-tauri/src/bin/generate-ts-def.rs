// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::prelude::*;
use std::path::Path;

use dbnexus_db::completion::{CompletionItem, CompletionKind};
use dbnexus_db::datasource::{PGDBConfig, SqlServerDBConfig};
use dbnexus_db::db::{
    ExportKind, Field, Index, ListProcedure, ListTable, ListTrigger, ListUdf, ListView,
    ProcedureDetail, QueryResp, SqlExecuteRet, TableListResp, TriggerDetail, TriggerEnable,
    TriggerEvent, TriggerTiming, UdfDetail,
};
use dbnexus_db::redis::RedisListTable;
use dvnexus_app_lib::storage::{self, OpendKind};
use typescript_type_def::{write_definition_file_from_type_infos, DefinitionFileOptions, TypeDef};

fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let filepath = Path::new(&dir).to_path_buf().join("../src/ApiStruct.ts");

    let ts_module = {
        let api = vec![
            &storage::DBConnectionConfig::INFO,
            &storage::UserSettings::INFO,
            &PGDBConfig::INFO,
            &SqlServerDBConfig::INFO,
            &ListTable::INFO,
            &Field::INFO,
            &TableListResp::INFO,
            &Index::INFO,
            &SqlExecuteRet::INFO,
            &ListView::INFO,
            &ExportKind::INFO,
            &RedisListTable::INFO,
            &TriggerDetail::INFO,
            &TriggerEvent::INFO,
            &TriggerTiming::INFO,
            &TriggerEnable::INFO,
            &ListTrigger::INFO,
            &CompletionItem::INFO,
            &CompletionKind::INFO,
            &QueryResp::INFO,
            &UdfDetail::INFO,
            &ListUdf::INFO,
            &ListProcedure::INFO,
            &ProcedureDetail::INFO,
            &OpendKind::INFO,
        ];

        let mut buf = Vec::new();
        let mut options: DefinitionFileOptions = DefinitionFileOptions::default();
        options.root_namespace = Some("db_apis");
        write_definition_file_from_type_infos(&mut buf, options, &api).unwrap();
        buf
    };
    let mut ops = std::fs::OpenOptions::new();
    ops.write(true);
    ops.truncate(true);
    ops.create(true);

    let mut file = ops.open(filepath.as_path()).unwrap();
    file.write_all(&ts_module).unwrap();
}
