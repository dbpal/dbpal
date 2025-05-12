#[cfg(test)]
mod tests {
    use crate::TEST_HOST;
    use crate::datasource_config::MysqlDBConfig;
    use crate::mysql::test_fns::{self, make_test_object};

    pub(crate) fn make_config() -> MysqlDBConfig {
        MysqlDBConfig {
            host: TEST_HOST.to_string(),
            username: "root".to_string(),
            password: Some("root".to_string()),
            port: 3307,
            db: Some("test".to_string()),
            read_only: false,
            save_password: true,
            ssl: Default::default(),
        }
    }
    #[tokio::test]
    async fn test_duplicate() {
        test_fns::test_duplicate(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_server_metas() {
        test_fns::test_server_metas(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_procedure() {
        test_fns::test_procedure(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_index() {
        test_fns::test_index(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_check_conn() {
        test_fns::test_check_conn(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_export() {
        test_fns::test_export(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_rename() {
        test_fns::test_rename(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_udf() {
        test_fns::test_udf(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types1() {
        test_fns::test_types1(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types2() {
        test_fns::test_types2(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types3() {
        test_fns::test_types3(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_types4() {
        test_fns::test_types4(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_triggers() {
        test_fns::test_triggers(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_list_database() {
        test_fns::test_list_database(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_query() {
        test_fns::test_query(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_table_column() {
        test_fns::test_table_column(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_view() {
        test_fns::test_view(make_test_object(make_config()).await).await;
    }
}
