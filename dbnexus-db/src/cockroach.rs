#[cfg(test)]
use crate::{
    datasource_config::{PGDBConfig, SslFilesWithStrModel},
    db::BackendKindPostGres,
    pg::PostGresDB,
};

#[cfg(test)]
pub(crate) async fn make_test_object() -> PostGresDB {
    let r = PGDBConfig {
        host: crate::TEST_HOST.to_string(),
        port: 26257,
        username: "root".to_string(),
        password: None,
        db: Some("test".into()),
        schema: Some("xxx".to_string()),
        read_only: false,
        ssl: SslFilesWithStrModel::pg_default(),
        conn_timeout: Some(5000.into()),
        options: Default::default(),
    };
    let pg = PostGresDB::new(r, BackendKindPostGres::CockRoach)
        .await
        .unwrap();
    pg
}

pub(crate) const COCKROACH_TYPES: &[&str] = &[
    "BIT",
    "BIT[]",
    "BOOL",
    "BOOL[]",
    "BYTES",
    "BYTES[]",
    "COLLATE",
    "COLLATE[]",
    "DATE",
    "DATE[]",
    "DECIMAL",
    "DECIMAL[]",
    "FLOAT",
    "FLOAT[]",
    "INET",
    "INET[]",
    "INT",
    "INT[]",
    "INTERVAL",
    "INTERVAL[]",
    "NULL",
    "NULL[]",
    "JSONB",
    "JSONB[]",
    "OID",
    "OID[]",
    "SERIAL",
    "STRING",
    "STRING[]",
    "TIME",
    "TIME[]",
    "TIMETZ",
    "TIMETZ[]",
    "TIMESTAMP",
    "TIMESTAMP[]",
    "TIMESTAMPTZ",
    "TIMESTAMPTZ[]",
    "TSQUERY",
    "TSVECTOR",
    "UUID",
    "VECTOR",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        CatalogTrait, CheckConn, ColumnType, CreateIndexColumn, CreateIndexReq, IndexKind,
        NameWithParameter, QuerySql, SchemaTrait, ServerMetas, SqlNullAble,
    };
    use crate::db::{ExportKind, ShowTableParameter};
    use crate::enable_test_log;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_sequence() {
        let db = make_test_object().await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let name = "test_seq";
        let _ = db.drop_sequence(name, true).await;
        db.execute_sql(
            query_id,
            r#"
            CREATE SEQUENCE test_seq AS integer START 1;
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let name2 = format!("{}2", name);

        let seq = db.list_sequence().await.unwrap();
        for seq in seq.iter() {
            if seq.name.eq(name) {
                println!("seq:{:?}", seq);
            }
        }
        db.rename_sequence(name, &name2).await.unwrap();
        db.rename_sequence(&name2, name).await.unwrap();
    }
    #[tokio::test]
    async fn test_list_created_types() {
        let db = make_test_object().await;
        let _ = db.drop_created_type("test_type").await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();

        db.execute_sql(
            query_id,
            r#"
            CREATE TYPE test_type AS ENUM ('value1', 'value2', 'value3');"#,
        )
        .await
        .unwrap()
        .unwrap();
        let list = db.list_created_types().await.unwrap();
        for l in list.iter() {
            println!("create type:{:?}", l);
        }
    }

    #[tokio::test]
    async fn test_transaction() {
        let db = make_test_object().await;

        let table_name = "test_commit";
        let _ = db.drop_table(table_name, false).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_commit (
                ID INTEGER PRIMARY KEY,
                NAME VARCHAR(50)
            );
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.set_auto_commit(query_id, false).await.unwrap();

        db.execute_sql(
            query_id,
            r#"
            INSERT INTO test_commit (ID, NAME) VALUES (1, 'Test');
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.rollback(query_id).await.unwrap();

        assert!(
            db.show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"),)
                .await
                .unwrap()
                .total
                == 0f64
        );
        db.set_auto_commit(query_id, false).await.unwrap();

        db.execute_sql(
            query_id,
            r#"
            INSERT INTO test_commit (ID, NAME) VALUES (1, 'Test');
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.commit(query_id).await.unwrap();
        assert!(
            db.show_table_collect(table_name, ShowTableParameter::new_stupid_test("id asc"),)
                .await
                .unwrap()
                .total
                == 1f64
        );
    }

    #[tokio::test]
    async fn test_table_column() {
        let db = make_test_object().await;
        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_column
                (
                    id INT PRIMARY KEY                     -- 主键，无符号32位整数
                )
                "#,
        )
        .await
        .unwrap()
        .unwrap();

        db.add_table_column(
            table_name,
            "name",
            ColumnType {
                typ: "INT2".to_string(),
                nullable: SqlNullAble::Null.into(),
                after: None,
                default_value: Some("1".to_string()),
            },
        )
        .await
        .unwrap();

        db.modify_table_column(
            table_name,
            "name",
            "name2",
            Some(ColumnType {
                typ: "INT4".to_string(),
                nullable: SqlNullAble::Null.into(),
                after: None,
                default_value: Some("2".to_string()),
            }),
        )
        .await
        .unwrap();

        db.drop_table_column(table_name, "name2").await.unwrap();
    }

    #[tokio::test]
    async fn test_index() {
        let table_name = "index_test";
        let db = make_test_object().await;
        let index_name = "index_test_index_xxx";

        let _ = db.drop_index(table_name, index_name).await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();

        db.execute_sql(query_id, format!(
            r#" create table {table_name} (a int, b int, c int, constraint pk_test primary key(a, b)); "#,
        ).as_str()).await .unwrap() .unwrap();

        let index_name2 = format!("{}2", index_name);
        db.create_index(
            table_name,
            &CreateIndexReq {
                name: index_name.to_string(),
                kind: IndexKind::Unique,
                columns: vec![
                    CreateIndexColumn {
                        col: "a".to_string(),
                        asc: Some(true),
                    },
                    CreateIndexColumn {
                        col: "b".to_string(),
                        asc: Some(true),
                    },
                ],
            },
        )
        .await
        .unwrap();
        db.rename_index(table_name, index_name, &index_name2)
            .await
            .unwrap();
        db.rename_index(table_name, &index_name2, index_name)
            .await
            .unwrap();

        let indexes = db.show_indexes(table_name).await.unwrap();
        for index in indexes.iter() {
            println!("index:{:?}", index)
        }
    }

    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object().await;
        let dur = CheckConn::check_conn(&db).await.unwrap();
        println!("duration:{:?}", dur)
    }
    #[tokio::test]
    async fn test_databases() {
        let db = make_test_object().await;
        let dbs = CatalogTrait::list_database(&db).await.unwrap();
        println!("{:?}", dbs);
    }
    #[tokio::test]
    async fn test_schemas() {
        use crate::db::DBTrait;
        let db = make_test_object().await;
        let resp = db.list_schema().await.unwrap();
        println!("{:?}", resp);
    }
    #[tokio::test]
    async fn test_list_table_columns() {
        let db = make_test_object().await;

        let x = db.show_table_columns("test_types1").await.unwrap();
        for f in x.iter() {
            println!("{:?}", f);
        }
    }

    #[tokio::test]
    async fn test_rename() {
        let db = make_test_object().await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_table("test_rename", false).await;
        let _ = db.drop_table("test_rename2", false).await;
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_rename (
                    
                    name VARCHAR(255) NOT NULL,
                    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT CURRENT_TIMESTAMP
                );"#,
        )
        .await
        .unwrap()
        .unwrap();
        db.rename_table("test_rename", "test_rename2")
            .await
            .unwrap();

        db.show_table_collect(
            "test_rename2",
            ShowTableParameter::new_stupid_test("name asc"),
        )
        .await
        .unwrap();
    }
    #[tokio::test]
    async fn test_duplicate() {
        let db = make_test_object().await;
        let _ = db.drop_table("test_duplicate", false).await;
        let _ = db.drop_table("test_duplicate2", false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
                CREATE TABLE test_duplicate (
                    
                    data TEXT,
                    created_at TIMESTAMP WITHOUT TIME ZONE DEFAULT CURRENT_TIMESTAMP
                );
                INSERT INTO test_duplicate (data) VALUES
                ('测试数据1'),
                ('测试数据2'),
                ('测试数据3'),
                ('测试数据4'),
                ('测试数据5');
                "#,
        )
        .await
        .unwrap()
        .unwrap();
        db.duplicate_table("test_duplicate", "test_duplicate2")
            .await
            .unwrap();

        let resp = db
            .show_table_collect(
                "test_duplicate2",
                ShowTableParameter::new_stupid_test("data asc"),
            )
            .await
            .unwrap();
        assert_eq!(resp.data.len(), 5);
    }

    #[tokio::test]
    async fn test_procedure() {
        let db = make_test_object().await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _ = db
            .drop_procedure(&NameWithParameter {
                name: "test_procedure".to_string(),
                parameters: Some("".to_string()),
                ..Default::default()
            })
            .await;
        db.execute_sql(
            query_id,
            r#"
            CREATE PROCEDURE test_procedure()
            LANGUAGE plpgsql AS
            $$
            BEGIN
                -- procedure logic goes here
            END;
            $$;
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let list = db.list_procedure().await.unwrap();
        for l in list.iter() {
            let details = db.show_procedure(l).await.unwrap();
            println!("udf:{},details:{:?}", l.name, details);
        }
    }
    #[tokio::test]
    async fn test_view() {
        enable_test_log();
        let db = make_test_object().await;
        let _ = db.drop_view("test_view").await;
        let _ = db.drop_table("test_view_table", true).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_view_table (
                
                name TEXT,
                value INT
            );
            INSERT INTO test_view_table (name, value) VALUES
                ('Item A', 10),
                ('Item B', 20),
                ('Item C', 30);

            CREATE VIEW test_view AS
                SELECT * FROM test_view_table;
        "#,
        )
        .await
        .unwrap()
        .unwrap();

        let list = db.list_view().await.unwrap();
        for l in list.iter() {
            let table_resp = db
                .show_view_collect(&l.name, ShowTableParameter::new_stupid_test("name asc"))
                .await
                .unwrap();
            println!("resp:{:?}", table_resp);
        }
    }

    #[tokio::test]
    async fn test_materialized_view() {
        enable_test_log();
        let db = make_test_object().await;
        let _ = db.drop_materialized_view("test_material_view").await;
        let _ = db.drop_table("test_material_view_table", true).await;

        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            CREATE TABLE test_material_view_table (
                
                name TEXT,
                value INT
            );
            INSERT INTO test_material_view_table (name, value) VALUES
                ('Item A', 10),
                ('Item B', 20),
                ('Item C', 30);

            CREATE MATERIALIZED VIEW test_material_view AS
                SELECT * FROM test_material_view_table;
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let list = db.list_materialized_view().await.unwrap();
        for l in list.iter() {
            let table_resp = db
                .show_view_collect(&l.name, ShowTableParameter::new_stupid_test("name asc"))
                .await
                .unwrap();
            println!("resp:{:?}", table_resp);
        }
    }

    #[tokio::test]
    async fn test_server_metas() {
        let db = make_test_object().await;
        let metas = ServerMetas::show_metas(&db).await.unwrap();
        println!("{:?}", metas);
    }
    #[tokio::test]
    async fn test_list_table() {
        let db = make_test_object().await;
        let t = db.list_table().await.unwrap();
        for t in t.iter() {
            println!("{:?}", t);
        }
    }

    #[tokio::test]
    async fn test_udf() {
        let pg = make_test_object().await;
        let query_id = "aaaa";
        pg.open_connection(query_id).await.unwrap();
        let udf_name = "test_udf";
        let _ = pg
            .drop_udf(&NameWithParameter {
                name: udf_name.to_string(),
                parameters: Some("a integer, b integer".to_string()),
                ..Default::default()
            })
            .await;

        pg.execute_sql(
            query_id,
            r#"
            CREATE OR REPLACE FUNCTION test_udf(a INT, b INT)
            RETURNS INT AS $$
            BEGIN
                RETURN a + b;
            END;
            $$ LANGUAGE plpgsql;
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let list = pg.list_udf().await.unwrap();
        for l in list.iter() {
            // I created some other functions to test.
            // so we only test on `test_udf`.
            if l.name != udf_name {
                continue;
            }
            println!("udf:{:?}", l);
            let details = pg.show_udf(l).await.unwrap();
            println!("name:{} , udf:{:?}", l.name, details);
        }
    }

    #[tokio::test]
    async fn test_types1() {
        enable_test_log();
        let table_name = "test_types1";
        let db = make_test_object().await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            format!(
                r#"
        CREATE TABLE {table_name} (
            bool_column BOOL,
            bytea_column BYTEA,
            char_column CHAR,
            name_column NAME,
            int8_column INT8,
            int2_column INT2,
            int4_column INT4
        );"#
            )
            .as_str(),
        )
        .await
        .unwrap()
        .unwrap();
        let mut values = HashMap::new();
        values.insert("bool_column".to_string(), Some("true".to_string()));
        values.insert(
            "bytea_column".to_string(),
            Some("\\\\x0123456789ABCDEF".to_string()),
        );
        values.insert("char_column".to_string(), Some("A".to_string()));
        values.insert("name_column".to_string(), Some("John".to_string()));
        values.insert("int8_column".to_string(), Some("1".to_string()));
        values.insert("int2_column".to_string(), Some("2".to_string()));
        values.insert("int4_column".to_string(), Some("100".to_string()));
        values.insert("rowid".to_string(), Some("unique_rowid()".to_string()));
        db.table_insert(table_name, &values).await.unwrap();
        let mut wheres = values.clone();
        wheres.insert("id".to_string(), Some("1".to_string()));
        let json = db
            .export_table_to_string("test_types1", ExportKind::Json { pretty: true })
            .await
            .unwrap();
        let _: serde_json::Value = serde_json::from_str(json.as_str()).unwrap();

        println!("export json:{}", json);
    }
}
