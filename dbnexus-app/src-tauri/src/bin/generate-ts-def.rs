// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::os;
use std::path::Path;

use dbnexus_db::db::ExeSqlError;
use dbnexus_db::db::ExportKind;
use dbnexus_db::db::Field;
use dbnexus_db::db::Index;
use dbnexus_db::db::ListTable;
use dbnexus_db::db::ListView;
use dbnexus_db::db::SqlExecuteRet;
use dbnexus_db::db::TableListResp;

use dvnexus_app_lib::storage;

use std::fs::File;
use std::io::prelude::*;
use typescript_type_def::write_definition_file_from_type_infos;
use typescript_type_def::DefinitionFileOptions;
use typescript_type_def::TypeDef;
fn main() {
    let dir = env!("CARGO_MANIFEST_DIR");
    let filepath = Path::new(&dir).to_path_buf().join("../src/ApiStruct.ts");

    let ts_module = {
        let api = vec![
            &storage::DBConnectionConfig::INFO,
            &storage::UserSettings::INFO,
            &ListTable::INFO,
            &Field::INFO,
            &TableListResp::INFO,
            &Index::INFO,
            &SqlExecuteRet::INFO,
            &ListView::INFO,
            &ExeSqlError::INFO,
            &ExportKind::INFO,
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
}
