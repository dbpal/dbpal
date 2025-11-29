use sqlx::types::BigDecimal;
use sqlx::{AssertSqlSafe, Row};
use std::ops::DerefMut;
use std::sync::Arc;

#[cfg(test)]
use crate::datasource_config::MysqlDBConfig;
use crate::db::SequenceDetail;
use crate::mysql::MysqlDB;
#[cfg(test)]
use crate::sub::SubDB;

impl MariaDB {
    pub(crate) async fn list_sequences_impl(
        &self, schema: &str,
    ) -> anyhow::Result<Vec<SequenceDetail>> {
        let sql = format!(
            "
            select SEQUENCE_NAME , DATA_TYPE,
            START_VALUE, MINIMUM_VALUE ,
            MAXIMUM_VALUE  , INCREMENT
            from information_schema.SEQUENCES where SEQUENCE_SCHEMA = '{}'
            ",
            schema
        );
        let mut conn = self.db.get_conn().await?;
        let rows = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for row in rows.iter() {
            let name: String = row.try_get(0)?;
            let data_type: String = row.try_get(1)?;
            let start_value: BigDecimal = row.try_get(2)?;
            let min_value: BigDecimal = row.try_get(3)?;
            let max_value: BigDecimal = row.try_get(4)?;
            let increment: i64 = row.try_get(5)?;
            let create_sql = {
                let sql = format!("SHOW CREATE SEQUENCE {name}");
                let row = sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
                    .fetch_one(conn.deref_mut())
                    .await?;
                let create_sql: String = row.try_get(1)?;
                create_sql
            };

            ret.push(SequenceDetail {
                name,
                data_type,
                start_value: start_value.to_string(),
                min_value: min_value.to_string(),
                max_value: max_value.to_string(),
                increment_by: increment.to_string(),
                last_value: None, // TODO get last value
                sql: create_sql,
            });
        }
        anyhow::Ok(ret)
    }
}

pub struct MariaDB {
    pub(crate) db: Arc<MysqlDB>,
}

impl MariaDB {
    pub(crate) async fn list_sequence(&self) -> anyhow::Result<Vec<SequenceDetail>> {
        self.list_sequences_impl(self.db.config.schema_str()?).await
    }

    pub(crate) async fn rename_sequence(&self, old: &str, new: &str) -> anyhow::Result<()> {
        let sql = format!("RENAME TABLE {old}  TO {new}");
        let mut conn = self.db.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }

    pub(crate) async fn drop_sequence(&self, name: &str, _cascade: bool) -> anyhow::Result<()> {
        let sql = format!("DROP SEQUENCE IF EXISTS {name};");
        let mut conn = self.db.get_conn().await?;
        sqlx::raw_sql(AssertSqlSafe(sql.as_str()))
            .execute(conn.deref_mut())
            .await?;
        anyhow::Ok(())
    }
}

#[cfg(test)]
pub(crate) fn make_config() -> MysqlDBConfig {
    MysqlDBConfig {
        conn_timeout: Some(5000.into()),
        host: crate::TEST_HOST.to_string(),
        username: "root".to_string(),
        password: Some("root".to_string()),
        port: 3307,
        db: Some("test".to_string()),
        read_only: false,
        save_password: true,
        ssl: Default::default(),
    }
}

#[cfg(test)]
pub(crate) async fn make_test_object_mariadb(config: MysqlDBConfig) -> SubDB {
    use crate::{mysql::MysqlDB, sub::SubDB};
    let db = SubDB::MariaDb(MysqlDB::new(config).await.unwrap().into());
    db
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{QuerySql, SchemaTrait};
    use crate::language_server::LangServerCommon;
    use crate::mariadb::make_config;
    use crate::mysql::test_fns::{self, make_test_object};

    #[tokio::test]
    async fn test_prepare() {
        let db = make_test_object_mariadb(make_config()).await;
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        db.execute_sql(
            query_id,
            r#"
            PREPARE stmtxxx FROM 'SELECT 1';
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let ts = db.list_execute(None).await.unwrap();

        assert!(
            ts.iter().any(|x| x.name.eq("stmtxxx")),
            "!!!!!!!!!!!{:?}",
            ts
        );
    }

    #[tokio::test]
    async fn test_sequence() {
        let db = make_test_object_mariadb(make_config()).await;
        let name = "test_seq";
        let query_id = "";
        db.open_connection(query_id).await.unwrap();
        let _ = db.drop_sequence(name, false).await;

        db.execute_sql(
            query_id,
            r#"
            CREATE SEQUENCE test_seq;
        "#,
        )
        .await
        .unwrap()
        .unwrap();
        let sequences = SchemaTrait::list_sequence(&db).await.unwrap();
        for seq in sequences.iter() {
            if seq.name.eq(name) {
                println!("{:?}", seq)
            }
        }
        let name2 = format!("{name}2");
        db.rename_sequence(name, &name2).await.unwrap();
        db.rename_sequence(&name2, name).await.unwrap();
    }

    #[tokio::test]
    async fn test_duplicate() {
        test_fns::test_duplicate(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_transaction() {
        test_fns::test_transaction(make_test_object(make_config()).await).await;
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
    async fn test_table_column() {
        test_fns::test_table_column(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_view() {
        test_fns::test_view(make_test_object(make_config()).await).await;
    }
    #[tokio::test]
    async fn test_event() {
        test_fns::test_event(make_test_object(make_config()).await).await;
    }
}
