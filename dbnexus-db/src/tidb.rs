#[cfg(test)]
use crate::datasource_config::MysqlDBConfig;
use crate::mysql::MysqlDB;

pub struct TiDB {
    pub(crate) db: MysqlDB,
}
#[cfg(test)]
// docker run -d --name some-tidb    -p 4000:4000   pingcap/tidb
pub(crate) fn make_config() -> MysqlDBConfig {
    MysqlDBConfig {
        conn_timeout: Some(5000.into()),
        host: crate::TEST_HOST.to_string(),
        username: "root".to_string(),
        password: Some("root".to_string()),
        port: 4000,
        db: None,
        read_only: false,
        ssl: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use crate::datasource_config::MysqlDBConfig;
    use crate::mysql::test_fns::{self, make_test_object};
    use crate::TEST_HOST;

    pub(crate) fn make_config() -> MysqlDBConfig {
        MysqlDBConfig {
            conn_timeout: Some(5000.into()),
            host: TEST_HOST.to_string(),
            username: "root".to_string(),
            password: None,
            port: 4000,
            db: Some("test".to_string()),
            read_only: false,
            ssl: Default::default(),
        }
    }

    #[tokio::test]
    async fn test_transaction() {
        test_fns::test_transaction(make_test_object(make_config()).await).await;
    }

    // #[ignore = "1105 (HY000): 'CREATE TABLE ... SELECT' is not implemented yet"]
    // #[tokio::test]
    // async fn test_duplicate() {
    //     test_fns::test_duplicate(make_test_object(make_config()).await).await;
    // }

    #[tokio::test]
    async fn test_server_metas() {
        test_fns::test_server_metas(make_test_object(make_config()).await).await;
    }
    // #[tokio::test]
    // async fn test_procedure() {
    //     test_fns::test_procedure(make_test_object(make_config()).await).await;
    // }
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
    // 在 TiDB 中，用户定义函数（UDF）不能直接通过 SQL 语句创建。与 MySQL
    // 等其他数据库不同，TiDB 需要通过 C/C++ 编写 UDF，并编译成动态链接库（.so
    // 文件），然后将其注册。这是因为 UDF 的实现涉及底层的 C/C++ 代码，而不是简单的
    // SQL 语句。
    // #[tokio::test]
    // async fn test_udf() {
    //     test_fns::test_udf(make_test_object(make_config()).await).await;
    // }

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
    async fn test_list_database() {
        test_fns::test_list_database(make_test_object(make_config()).await).await;
    }

    #[tokio::test]
    async fn test_view() {
        test_fns::test_view(make_test_object(make_config()).await).await;
    }
}
