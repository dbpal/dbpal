use tauri_plugin_sql::{Migration, MigrationKind};

pub fn migrations() -> Vec<Migration> {
    // v1 version of dbpal
    vec![
        Migration {
            version: 1,
            description: "create_query_log",
            sql: r#"
CREATE TABLE IF NOT EXISTS query_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT NOT NULL,          -- uuid like mysql/xxx
    name TEXT NOT NULL,          -- name like mysql
    sql TEXT NOT NULL,
    language TEXT NULL, 
    create_at DATETIME NOT NULL DEFAULT current_timestamp,
    success INTEGER NOT NULL
);"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "index_query_log_uuid",
            sql: "CREATE INDEX IF NOT EXISTS index_query_log_uuid ON query_log (uuid);",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 3,
            description: "create_element_size",
            sql: r#"
CREATE TABLE IF NOT EXISTS element_size (
    name TEXT PRIMARY KEY NOT NULL,
    width INTEGER NULL,
    height INTEGER NULL
);"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 4,
            description: "create_column_width",
            sql: r#"
CREATE TABLE IF NOT EXISTS column_width (
    name TEXT PRIMARY KEY NOT NULL,
    width INTEGER NOT NULL
);"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 5,
            description: "create_favorite_sql",
            sql: r#"
CREATE TABLE IF NOT EXISTS favorite_sql (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    uuid TEXT NOT NULL,
    kind TEXT NOT NULL,
    sql TEXT NOT NULL,
    name TEXT NOT NULL,
    connection_name TEXT NOT NULL,
    last_modified DATETIME NOT NULL DEFAULT current_timestamp
);"#,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 6,
            description: "index_favorite_sql_uuid",
            sql: "CREATE INDEX IF NOT EXISTS index_favorite_sql_uuid ON favorite_sql (uuid);",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 7,
            description: "unique_index_favorite_sql_uuid_name",
            sql: "CREATE UNIQUE INDEX IF NOT EXISTS index_favorite_sql_uuid_name ON favorite_sql (uuid, name);",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 8,
            description: "create table connection",
            sql: r#"
                CREATE TABLE connection (
                    id   INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT UNIQUE,
                    config TEXT
                );
                
            "#,
            kind: MigrationKind::Up,
        },
    ]
}
