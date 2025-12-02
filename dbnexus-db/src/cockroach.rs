use crate::db::{
    GetBackendKind, ListTable, QueryEventEmitter, QueryResp, QuerySql, SchemaTrait,
    ShowTableParameter, TimeElapsed,
};
use crate::pg::{PostGresDB, gen_show_sql};
use crate::pg_format::encode_rows;
#[cfg(test)]
use crate::sub::SubDB;
use futures_util::StreamExt;
use sqlx::{AssertSqlSafe, Row};
use std::ops::DerefMut;

#[cfg(test)]
pub(crate) async fn make_test_object() -> SubDB {
    use std::sync::Arc;

    use crate::datasource_config::PGDBConfig;

    let r = PGDBConfig {
        host: crate::TEST_HOST.to_string(),
        port: 26257,
        username: "root".to_string(),
        password: None,
        db: Some("test".into()),
        schema: Some("xxx".to_string()),
        read_only: false,
        save_password: true,
        ssl: Default::default(),
        conn_timeout: Some(5000.into()),
    };

    let pg = SubDB::CocoRoach(Arc::new(PostGresDB::new(r).await.unwrap()));
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

impl PostGresDB {
    pub(crate) async fn cock_roach_show_table(
        &self, table_name: &str, para: ShowTableParameter, emit: QueryEventEmitter,
    ) -> anyhow::Result<()> {
        let ShowTableParameter {
            where_str,
            order_by,
            page,
            page_size,
            ..
        } = &para;

        emit("trying to query columns".into());
        let fields = self.show_table_columns(table_name).await?;
        let total = self.query_count(table_name, where_str.as_deref()).await?;
        emit("trying to query total".into());
        emit(
            QueryResp {
                fields: fields.clone(),
                total,
                ..Default::default()
            }
            .into(),
        );
        let sql = gen_show_sql(
            self.config.schema_str()?,
            table_name,
            Some(self.backend_name().format_fields(Some(&fields))),
            where_str.as_deref(),
            order_by.as_deref(),
            *page,
            *page_size,
        );
        log::info!("sql:{}", sql);
        emit("trying to query data".into());
        let mut conn = self.get_conn().await?;
        let mut stream = sqlx::raw_sql(AssertSqlSafe(sql.as_str())).fetch(conn.deref_mut());
        let mut rows = vec![];
        let mut time_elapsed = TimeElapsed::new();
        while let Some(x) = stream.next().await {
            let row = x?;
            rows.push(row);
            if time_elapsed.elapsed() || rows.len() >= self.query_buffer_size() {
                emit(encode_rows(&std::mem::take(&mut rows))?.into());
            }
        }
        if !rows.is_empty() {
            emit(encode_rows(&rows)?.into());
        }
        anyhow::Ok(())
    }

    pub(crate) async fn cock_roach_list_view(&self) -> anyhow::Result<Vec<ListTable>> {
        let sql = format!(
            r#"
            SELECT  table_name 
            FROM information_schema.tables
            WHERE table_type = 'VIEW'
                AND table_schema = '{}' and table_name not in (SELECT 
                    matviewname
            FROM 
                pg_matviews where schemaname = '{}' ) 
        "#,
            self.config.schema_str()?,
            self.config.schema_str()?,
        );
        let mut conn = self.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::new();
        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            ret.push(ListTable::from(name));
        }

        anyhow::Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        CatalogTrait, CheckConn, ColumnType, IndexKind, QuerySql, ServerMetas,
        UdfOrProcedureExtraParameter,
    };
    use crate::db::{ExportKind, ShowTableParameter};

    use crate::enable_test_log;
    use std::collections::HashMap;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_transaction() {
        let db = make_test_object().await;

        let table_name = "test_commit";
        let _ = db.drop_table(table_name, false).await;

        let query_id = "";
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
                == 0
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
                == 1
        );
    }

    #[tokio::test]
    async fn test_table_column() {
        let db = make_test_object().await;
        let table_name = "test_column";
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
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
                nullness: Some("null".to_string()),
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
                nullness: Some("null".to_string()),
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
        let query_id = "";
        db.open_connection(query_id).await.unwrap();

        db.execute_sql(query_id, format!(
            r#" create table {table_name} (a int, b int, c int, constraint pk_test primary key(a, b)); "#,
        ).as_str()).await .unwrap() .unwrap();

        let index_name2 = format!("{}2", index_name);
        db.create_index(
            table_name,
            index_name,
            IndexKind::Unique,
            &["a".to_string(), "b".to_string()],
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
        let _ = CatalogTrait::drop_database(&db, "abc").await;
        CatalogTrait::create_database(&db, "abc", None)
            .await
            .unwrap();
        let dbs = CatalogTrait::list_database(&db).await.unwrap();
        println!("{:?}", dbs);
    }
    #[tokio::test]
    async fn test_schemas() {
        use crate::db::DBTrait;
        let db = make_test_object().await;
        let _ = db.drop_schema("abc").await;
        let _ = db.create_schema("abc", None).await;
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

        let query_id = "";
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

        db.show_table(
            "test_rename2",
            ShowTableParameter::new_stupid_test("id asc"),
            Arc::new(|s| {
                println!("{:?}", s);
            }),
        )
        .await
        .unwrap();
    }
    #[tokio::test]
    async fn test_duplicate() {
        let db = make_test_object().await;
        let _ = db.drop_table("test_duplicate", false).await;
        let _ = db.drop_table("test_duplicate2", false).await;
        let query_id = "";
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
                ShowTableParameter::new_stupid_test("id asc"),
            )
            .await
            .unwrap();
        assert_eq!(resp.data.len(), 5);
    }

    #[tokio::test]
    async fn test_procedure() {
        let db = make_test_object().await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_procedure("test_procedure", None).await;
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
            let details = db
                .show_procedure(
                    &l.name,
                    Some(UdfOrProcedureExtraParameter::PostGres {
                        parameters: "".to_string(),
                    }),
                )
                .await
                .unwrap();
            println!("udf:{},details:{:?}", l.name, details);
        }
    }
    #[tokio::test]
    async fn test_view() {
        enable_test_log();
        let db = make_test_object().await;
        let _ = db.drop_view("test_view").await;
        let _ = db.drop_table("test_view_table", true).await;

        let query_id = "";
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
                .show_view(
                    &l.name,
                    ShowTableParameter::new_stupid_test("id asc"),
                    Arc::new(|s| {
                        println!("{:?}", s);
                    }),
                )
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

        let query_id = "";
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
                .show_view(
                    &l.name,
                    ShowTableParameter::new_stupid_test("id asc"),
                    Arc::new(|s| {
                        println!("{:?}", s);
                    }),
                )
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
        let query_id = "";
        pg.open_connection("").await.unwrap();
        let udf_name = "test_udf";
        let _ = pg
            .drop_udf(
                udf_name,
                Some(UdfOrProcedureExtraParameter::PostGres {
                    parameters: "a integer, b integer".to_string(),
                }),
            )
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
            let details = pg
                .show_udf(
                    &l.name,
                    Some(UdfOrProcedureExtraParameter::PostGres {
                        parameters: "int8, int8".to_string(),
                    }),
                )
                .await
                .unwrap();
            println!("name:{} , udf:{:?}", l.name, details);
        }
    }

    #[tokio::test]
    async fn test_types1() {
        enable_test_log();
        let table_name = "test_types1";
        let db = make_test_object().await;
        let _ = db.drop_table(table_name, false).await;
        let query_id = "";
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
        db.table_insert(table_name, values.clone()).await.unwrap();
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
