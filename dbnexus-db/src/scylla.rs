// docker run --name my-scylla -p 9043:9042 -d scylladb/scylla:latest

#[cfg(test)]
use crate::{
    cassandra::CassandraDB,
    datasource_config::{CassandraDBConfig, CassandraDialect},
};

/*
Datacenter datacenter1 doesn't have enough token-owning nodes for replication_factor=3
测试节点  replication_factor=1 这样设置呢


drop udf error:Sql:DROP FUNCTION "test_udf"(list<int>)  Err:Database returned an error: The query is syntactically correct but invalid, Error message: User defined functions are disabled. Set enable_user_defined_functions and experimental_features:udf to enable them
需要enable这个设置这个呢


*/

#[cfg(test)]
pub(crate) async fn make_test_object(db: Option<String>) -> CassandraDB {
    use crate::TEST_HOST;
    let ss = CassandraDB::new(CassandraDBConfig {
        host: TEST_HOST.to_string(),
        port: 9043,
        username: None,
        password: None,
        read_only: false,
        dialect: CassandraDialect::Scylla,
        db,
        conn_timeout: None,
        ssl_ca_file: None,
        ssl_enable: false,
    })
    .await
    .unwrap();
    ss
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{
        cassandra::{test_impls, test_lock},
        db::{NamedStaff, QuerySql, SchemaTrait, ShowTableParameter, TestQueryEventEmit},
    };

    #[tokio::test]
    async fn test_list_schema() {
        let db = make_test_object(None).await;
        test_impls::test_list_schema_inner(db).await;
    }

    #[tokio::test]
    pub async fn test_aggregate() {
        let _guard = test_lock().lock().await;
        let db = make_test_object(Some("test".to_string())).await;
        {
            let _ = db
                .drop_aggregate("test_aggregate", vec!["int".to_string()])
                .await;
            let _ = db
                .drop_udf(&NamedStaff::from("sum_state_function".to_string()))
                .await;

            let _ = db
                .drop_udf(&NamedStaff {
                    name: "sum_final_function".to_string(),
                    cassandra_parameters: Some(vec!["int".to_string()]),
                    ..Default::default()
                })
                .await;
        }
        let query_id = "aaaa";
        db.open_connection(query_id).await.unwrap();
        let _resp = db
            .execute_sql(
                query_id,
                r#"
        CREATE OR REPLACE FUNCTION sum_state_function(state int, val int)
            RETURNS NULL ON NULL INPUT
            RETURNS int
            LANGUAGE lua
            AS $$
                return state + val
            $$;
        "#,
            )
            .await
            .unwrap()
            .unwrap();

        let _resp = db
            .execute_sql(
                query_id,
                r#"
                CREATE OR REPLACE FUNCTION sum_final_function(state int)
                RETURNS NULL ON NULL INPUT
                RETURNS int
                LANGUAGE lua
                AS $$
                    return state
                $$;
    "#,
            )
            .await
            .unwrap()
            .unwrap();

        let _resp = db
            .execute_sql(
                query_id,
                r#"
            CREATE AGGREGATE test_aggregate(int)
            SFUNC sum_state_function
            STYPE int
            FINALFUNC sum_final_function
            INITCOND 0;
"#,
            )
            .await
            .unwrap();

        let list = db.list_aggregate().await.unwrap();
        list.iter().for_each(|x| println!("aggregate:{:?}", x));

        let details = db
            .show_aggregate("test_aggregate", vec!["int".to_string()])
            .await
            .unwrap();

        println!("details:{:?}", details);
    }

    #[tokio::test]
    async fn test_check_conn() {
        let db = make_test_object(None).await;
        test_impls::test_check_conn_inner(db).await;
    }

    #[ignore]
    #[tokio::test]
    async fn test_a_lot_data() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_a_lot_data_inner(db).await;
    }

    #[tokio::test]
    async fn test_sort() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_sort_inner(db).await;
    }

    #[tokio::test]
    async fn test_show_table_column() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_show_table_column_inner(db).await;
    }

    #[tokio::test]
    async fn test_duplicate() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_duplicate_inner(db).await;
    }

    #[tokio::test]
    async fn test_list_created_types() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_list_created_types_inner(db).await;
    }

    #[tokio::test]
    async fn test_types1() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types1_inner(db).await;
    }

    #[tokio::test]
    async fn test_types1_list() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types1_list_inner(db).await;
    }

    #[tokio::test]
    async fn test_types2() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types2_inner(db).await;
    }

    #[tokio::test]
    async fn test_types2_list() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types2_list_inner(db).await;
    }

    #[tokio::test]
    async fn test_types3() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types3_inner(db).await;
    }

    #[tokio::test]
    async fn test_types_list3() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types_list3_inner(db).await;
    }

    #[tokio::test]
    async fn test_types4() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_types4_inner(db).await;
    }

    #[tokio::test]
    async fn test_table_column() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_table_column_inner(db).await;
    }

    #[tokio::test]
    async fn test_counter() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_counter_inner(db).await;
    }

    #[tokio::test]
    pub async fn test_udf() {
        let db = make_test_object(Some("test".to_string())).await;
        let _guard = test_lock().lock().await;
        let name = "test_udf";
        {
            let err = db
                .drop_udf(&NamedStaff {
                    name: name.to_string(),
                    cassandra_parameters: Some(vec!["text".to_string()]),
                    ..Default::default()
                })
                .await;
            if let Err(err) = err {
                println!("drop udf error:{}", err);
            }
        }

        db.session
            .query_unpaged(
                r#"
            CREATE FUNCTION IF NOT EXISTS test_udf(input text)
            RETURNS NULL ON NULL INPUT
            RETURNS text
            LANGUAGE lua
            AS 'return input';
            "#,
                &[],
            )
            .await
            .unwrap();

        let list = db.list_udf().await.unwrap();
        list.iter().for_each(|x| println!("udf:{:?}", x));

        let detail = db
            .show_udf(&NamedStaff {
                name: "test_udf".to_string(),
                cassandra_parameters: Some(vec!["text".to_string()]),
                ..Default::default()
            })
            .await
            .unwrap();
        println!("detail:{:?}", detail);
    }

    #[tokio::test]
    async fn test_create_type() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_create_type_inner(db).await;
    }

    #[tokio::test]
    async fn test_rename() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_rename_inner(db).await;
    }

    #[tokio::test]
    async fn test_index() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_index_inner(db).await;
    }

    #[tokio::test]
    async fn test_view() {
        let db = make_test_object(Some("test".to_string())).await;
        test_impls::test_view_inner(db).await;
    }

    #[tokio::test]

    async fn test_show_index_info() {
        let db = make_test_object(Some("system".to_string())).await;
        db.show_table(
            "IndexInfo",
            ShowTableParameter {
                where_str: None,
                order_by: None,
                page: 1,
                page_size: 100,
                limit: None,
                min: None,
                max: None,
                allow_filtering: false,
                tab_size: 4,
            },
            Arc::new(TestQueryEventEmit::default()),
        )
        .await
        .unwrap();
    }
}
