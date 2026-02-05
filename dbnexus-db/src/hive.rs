#[cfg(test)]

mod tests {
    use crate::odbc::make_conn;

    #[tokio::test]

    async fn test_hive() {
        let _conn =
            make_conn(r#"DRIVER={Cloudera_Hive_ODBC};Host=127.0.0.1;Port=10000;Database=default;"#)
                .unwrap();
    }
}
