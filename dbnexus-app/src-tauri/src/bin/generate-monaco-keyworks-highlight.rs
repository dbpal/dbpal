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
fn main() {
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
