use sqlx::Column;
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;

use futures_util::{StreamExt, TryStreamExt};
use sqlx::mysql::MySqlRow;
use sqlx::pool::PoolConnection;
use sqlx::{Row, TypeInfo};

use crate::datasource::{CommDBConfig, DBConfig};
use crate::db::{
    create_csv, is_statement_absolute_query, sql_to_sql_seq, CreateTableAndIndexSql, DBTrait,
    ExeSqlError, ExportKind, Field, Index, IndexKind, ListTable, ListView, MultiDBTrait, QuerySql,
    SqlExecuteRet, SqlSeq, TableListResp,
};
use crate::pool::{NexusPool, PoolManager};

#[derive(Clone)]
pub struct MysqlDB {
    pool: PoolManager,
    config: CommDBConfig,
    read_only: bool,
}

fn get_rows_types(rows: &Vec<MySqlRow>) -> Vec<Field> {
    if rows.len() == 0 {
        return vec![];
    }
    let mut ret = Vec::with_capacity(rows[0].columns().len());
    for c in rows[0].columns().iter() {
        let column = c as &dyn sqlx::Column<Database = sqlx::MySql>;
        ret.push(Field {
            name: column.name().to_string(),
            typ: column.type_info().to_string(),
            nullable: true,
            default_value: None,
            is_primary: false,
            is_auto_increment: false,
        });
    }
    ret
}

fn gen_select_sql(
    table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
    page_size: usize,
) -> String {
    let mut sql = format!("SELECT * FROM {table_name}");
    if let Some(where_str) = where_str {
        sql.push_str(format!(" WHERE {where_str} ").as_str());
    }
    if let Some(order_by) = order_by {
        sql.push_str(format!(" ORDER BY {order_by} ").as_str());
    }
    let offset = (page - 1) * page_size;
    sql.push_str(format!(" LIMIT {} OFFSET {} ", page_size, offset).as_str());
    sql
}

fn encode_row_as_json(r: &MySqlRow) -> anyhow::Result<serde_json::Value> {
    let mut values = serde_json::Map::new();
    let null_value = serde_json::Value::Null;

    for c in 0..r.columns().len() {
        let col = r.column(c);
        let ty = (col as &dyn sqlx::Column<Database = sqlx::mysql::MySql>)
            .type_info()
            .name();

        let v = match ty.to_uppercase().as_str() {
            "BOOLEAN" | "BOOL" => {
                let value: Option<bool> = r.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "TINYINT" => {
                let value: Option<i8> = r.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "SMALLINT" => {
                let value: Option<i16> = r.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "INT" => {
                let value: Option<i32> = r.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "BIGINT" => {
                let value: Option<i64> = r.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "TINYINT UNSIGNED" => {
                let value: Option<u8> = r.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "SMALLINT UNSIGNED" => {
                let value: Option<u16> = r.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "INT UNSIGNED" => {
                let value: Option<u32> = r.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "BIGINT UNSIGNED" => {
                let value: Option<u64> = r.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "FLOAT" => {
                let value: Option<f32> = r.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "DOUBLE" => {
                let value: Option<f64> = r.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "VARCHAR" | "CHAR" | "TEXT" => {
                let value: Option<String> = r.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            _ if ty.starts_with("VARCHAR") || ty.starts_with("CHAR") || ty.starts_with("TEXT") => {
                let value: Option<String> = r.get(c);
                value
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }
            "VARBINARY" | "BINARY" | "BLOB" => {
                let value: Option<Vec<u8>> = r.get(c);
                value
                    .map(|x| format!("0x{}", hex::encode_upper(x.as_slice())))
                    .map(serde_json::Value::from)
                    .unwrap_or(null_value.clone())
            }

            _ => anyhow::bail!(format!("type '{ty}' not supported")),
        };
        values.insert(r.column(c).name().to_string(), v);
    }

    anyhow::Ok(serde_json::Value::Object(values))
}

impl MysqlDB {
    async fn export_table_as_sql(
        &self, table_name: &str, include_table_schema: bool,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        if include_table_schema {
            let ss = DBTrait::create_table_and_index_sql(self, table_name).await?;
            sender.send(Cow::Owned(ss.create_table.clone())).await?;
            sender.send(Cow::Borrowed("\n")).await?;

            for index in ss.indexes.into_iter() {
                sender.send(Cow::Owned(index)).await?;
                sender.send(Cow::Borrowed("\n")).await?;
            }
        }
        let mut conn = self.get_conn().await?;
        let cols = DBTrait::show_columns(self, table_name).await?;
        let sql = format!("select * from {}", table_name);
        let mut stream = sqlx::query(sql.as_str())
            .fetch(conn.deref_mut())
            .try_chunks(100);
        while let Some(rows) = stream.next().await {
            let rows = rows?;

            let data = encode_rows(&rows)?;
            for d in data.into_iter() {
                let mut m = HashMap::new();
                for (d, f) in d.into_iter().zip(cols.iter()) {
                    m.insert(f.name.clone(), d);
                }

                let insert_sql = (self as &dyn DBTrait).insert_sql(table_name, &cols, &m)?;
                sender.send(Cow::Owned(insert_sql)).await?;
                sender.send(Cow::Borrowed("\n")).await?;
            }
        }
        anyhow::Ok(())
    }

    async fn export_table_as_json(
        &self, table_name: &str, pretty: bool, sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;

        let sql = format!("select * from {}", table_name);
        let mut stream = sqlx::query(sql.as_str()).fetch(conn.deref_mut());

        sender.send(Cow::Borrowed("[")).await?;
        let mut first = true;
        while let Some(row) = stream.next().await {
            let row = row?;
            let value = encode_row_as_json(&row)?;
            if !first {
                sender.send(Cow::Borrowed(",")).await?;
            }
            if pretty {
                sender.send(Cow::Borrowed("\n")).await?;
            }
            sender
                .send(Cow::Owned(if pretty {
                    serde_json::to_string_pretty(&value)?
                } else {
                    serde_json::to_string(&value)?
                }))
                .await?;
            first = false;
        }
        if pretty {
            sender.send(Cow::Borrowed("\n")).await?;
        }
        sender.send(Cow::Borrowed("]")).await?;
        anyhow::Ok(())
    }

    async fn export_table_as_csv(
        &self, table_name: &str, include_header: bool, delimiter: char,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        let mut conn = self.get_conn().await?;
        if include_header {
            let fs = self.show_columns(table_name).await?;
            let fs = fs.iter().map(|x| Some(x.name.clone())).collect::<Vec<_>>();
            sender
                .send(Cow::Owned(create_csv(&vec![fs], delimiter)))
                .await?;
        }

        let sql = format!("select * from {}", table_name);
        let mut stream = sqlx::query(sql.as_str())
            .fetch(conn.deref_mut())
            .try_chunks(100);

        while let Some(rows) = stream.next().await {
            let rows = rows?;
            let data = encode_rows(&rows)?;
            sender
                .send(Cow::Owned(create_csv(&data, delimiter)))
                .await?;
        }
        anyhow::Ok(())
    }
}
#[async_trait::async_trait]

impl DBTrait for MysqlDB {
    async fn export_table(
        &self, table_name: &str, kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<Cow<'static, str>>,
    ) -> anyhow::Result<()> {
        match kind {
            ExportKind::CSV {
                include_header,
                delimiter,
            } => {
                self.export_table_as_csv(table_name, include_header, delimiter, sender)
                    .await
            }
            ExportKind::Json { pretty } => {
                self.export_table_as_json(table_name, pretty, sender).await
            }
            ExportKind::Sql {
                include_table_schema,
            } => {
                self.export_table_as_sql(table_name, include_table_schema, sender)
                    .await
            }
        }
    }

    fn read_only(&self) -> bool {
        self.read_only
    }

    async fn create_table_and_index_sql(
        &self, _table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        unimplemented!()
    }

    async fn create_view_sql(&self, _view_name: &str) -> anyhow::Result<String> {
        unimplemented!()
    }

    fn backend_name(&self) -> &str {
        "mysql"
    }

    fn clone(&self) -> Box<dyn DBTrait> {
        Box::new(Clone::clone(self))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn show_view(
        &self, view_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let sql = gen_select_sql(view_name, where_str, order_by, page, page_size);
        let mut conn = self.get_conn().await?;
        let total = self.query_count(view_name, where_str).await?;

        let rows = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await?;

        let data = encode_rows(&rows)?;
        let fields = get_rows_types(&rows);
        anyhow::Ok(TableListResp::new(fields, total, data, sql))
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.can_write()?;

        let mut conn = self.get_conn().await?;
        let sql = format!("DROP VIEW {view_name}");
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let mut conn = self.get_conn().await?;
        let sql = format!("DROP TABLE {table}");
        sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListView>> {
        let sql = format!(
            r#"
        SELECT table_name FROM information_schema.tables WHERE table_type = 'VIEW' AND table_schema = '{}';
        "#,
            self.config.db_name.as_ref().unwrap()
        );
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            ret.push(ListView::from(name));
        }
        anyhow::Ok(ret)
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fields = self.show_columns(table_name).await?;
        let sql = (self as &dyn crate::db::DBTrait).delete_sql(table_name, &fields, &wheres)?;
        let mut conn = self.get_conn().await?;
        let r = sqlx::query(sql.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: r.rows_affected(),
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let s = format!(
            r#"
            TRUNCATE TABLE {table_name};
        "#
        );
        let mut conn = self.get_conn().await?;
        sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(())
    }

    fn put_value_in_sql(&self, f: &Field, value: Option<&str>) -> anyhow::Result<String> {
        let default_str = value.map(|x| x.to_string()).unwrap_or("null".to_string());
        let single_quote_str = value
            .map(|x| format!("'{}'", x))
            .unwrap_or("null".to_string());
        let ty = f.typ.to_uppercase();

        let v = match ty.as_str() {
            "TINYINT" => default_str,
            "BOOLEAN" | "BOOL" => default_str,
            "SMALLINT" => default_str,
            "INT" => default_str,
            "BIGINT" => default_str,
            "TINYINT UNSIGNED" => default_str,
            "SMALLINT UNSIGNED" => default_str,
            "INT UNSIGNED" => default_str,
            "BIGINT UNSIGNED" => default_str,
            "FLOAT" => default_str,
            "DOUBLE" => default_str,
            "VARCHAR" | "CHAR" | "TEXT" => single_quote_str,
            _ if ty.starts_with("VARCHAR") || ty.starts_with("CHAR") || ty.starts_with("TEXT") => {
                single_quote_str
            }
            "VARBINARY" | "BINARY" | "BLOB" => default_str,
            _ => {
                log::error!("mysql type '{}' not handled", ty);
                default_str
            }
        };

        log::trace!("type '{ty}' encode '{:?}' as #{v}#", value);
        anyhow::Ok(v)
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_columns(table_name).await?;
        let s = (self as &dyn crate::db::DBTrait).insert_sql(table_name, &fs, &values)?;

        let mut conn = self.get_conn().await?;

        let r = sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql: s,
            changes: r.rows_affected(),
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    async fn update(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
        wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;
        let fs = self.show_columns(table_name).await?;
        let s = (self as &dyn crate::db::DBTrait).update_sql(table_name, &fs, &values, &wheres)?;
        let mut conn = self.get_conn().await?;

        let r = sqlx::query(s.as_str()).execute(conn.deref_mut()).await?;
        anyhow::Ok(SqlExecuteRet {
            sql: s,
            changes: r.rows_affected(),
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let mut conn = self.get_conn().await?;
        let s = format!(
            r#"SELECT table_name
        FROM information_schema.tables
        WHERE table_schema = '{}';"#,
            self.config.db_name.as_ref().map(|x| x.as_str()).unwrap()
        );
        let rows = sqlx::query(s.as_str()).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let table_name: String = r.get(0);
            ret.push(table_name);
        }
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    async fn show_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        // 执行 DESCRIBE users; 或 DESC users;
        // 命令将返回描述表结构的结果集。结果集包含以下列信息：
        //
        // Field：列的名称。
        // Type：列的数据类型。
        // Null：指示列是否允许为空值（YES 或 NO）。
        // Key：指示列是否是索引列，以及索引的类型。
        // Default：列的默认值。
        // Extra：显示附加信息，如是否自增、是否使用了自定义规则等。

        // +----------+--------------+------+-----+-------------------+-----------------------------+
        // | Field    | Type         | Null | Key | Default           | Extra                       |
        // +----------+--------------+------+-----+-------------------+-----------------------------+
        // | id       | int(11)      | NO   | PRI | NULL              | auto_increment              |
        // | username | varchar(50)  | NO   |     | NULL              |                             |
        // | password | varchar(100) | NO   |     | NULL              |                             |
        // +----------+--------------+------+-----+-------------------+-----------------------------+

        let s = format!("describe {}", table_name);
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(s.as_str()).fetch_all(conn.deref_mut()).await?;
        let mut ret = Vec::with_capacity(rows.len());
        for r in rows.iter() {
            let name: String = r.get(0);
            let ty: String = r.get(1);
            let null: String = r.get(2);
            let key: Option<String> = r.get(3);
            let default_value: Option<String> = r.try_get(4)?;
            let extra: String = r.get(5);

            ret.push(Field::new(
                name,
                ty,
                null != "NO",
                default_value,
                key.map_or(false, |x| x == "PRI"),
                extra == "auto_increment",
            ));
        }
        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let total = self.query_count(table_name, where_str.clone()).await?;
        let sql = gen_select_sql(table_name, where_str, order_by, page, page_size);

        let mut conn = self.get_conn().await?;
        let row = sqlx::query(sql.as_str())
            .fetch_all(conn.deref_mut())
            .await?;
        let data = encode_rows(&row)?;
        let fields = self.show_columns(table_name).await?;
        anyhow::Ok(TableListResp::new(fields, total, data, sql))
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let sql = format!("SHOW INDEX FROM {table_name}");
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query(&sql).fetch_all(conn.deref_mut()).await?;
        let mut map: HashMap<String, Index> = HashMap::new();
        for r in rows.iter() {
            let unique: Option<i32> = r.get(1);
            let name: String = r.get(2);
            let column_name = r.get(4);
            if let Some(x) = map.get_mut(&name) {
                x.on.push(column_name);
            } else {
                map.insert(
                    name.clone(),
                    Index::new(
                        name.clone(),
                        if name == "PRIMARY" {
                            IndexKind::PriMary
                        } else if unique.map(|x| x != 0).unwrap_or_default() {
                            IndexKind::Unique
                        } else {
                            IndexKind::Normal
                        },
                        vec![column_name],
                    ),
                );
            }
        }
        anyhow::Ok(map.into_iter().map(|x| x.1).collect())
    }
}

fn encode_rows(rows: &Vec<MySqlRow>) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let mut ret = Vec::with_capacity(rows.len());
    for r in rows.iter() {
        let mut values = Vec::with_capacity(r.len());
        for c in 0..r.len() {
            let col = r.column(c);
            let ty = (col as &dyn sqlx::Column<Database = sqlx::mysql::MySql>)
                .type_info()
                .name();

            let v = match ty.to_uppercase().as_str() {
                "BOOLEAN" | "BOOL" => {
                    let value: Option<bool> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "TINYINT" => {
                    let value: Option<i8> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "SMALLINT" => {
                    let value: Option<i16> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "INT" => {
                    let value: Option<i32> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "BIGINT" => {
                    let value: Option<i64> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "TINYINT UNSIGNED" => {
                    let value: Option<u8> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "SMALLINT UNSIGNED" => {
                    let value: Option<u16> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "INT UNSIGNED" => {
                    let value: Option<u32> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "BIGINT UNSIGNED" => {
                    let value: Option<u64> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "FLOAT" => {
                    let value: Option<f32> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "DOUBLE" => {
                    let value: Option<f64> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "VARCHAR" | "CHAR" | "TEXT" => {
                    let value: Option<String> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                _ if ty.starts_with("VARCHAR")
                    || ty.starts_with("CHAR")
                    || ty.starts_with("TEXT") =>
                {
                    let value: Option<String> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "VARBINARY" | "BINARY" | "BLOB" => {
                    let value: Option<Vec<u8>> = r.get(c);
                    value.map(|x| format!("0x{}", hex::encode_upper(x.as_slice())))
                }

                _ => anyhow::bail!(format!("type '{ty}' not supported")),
            };
            values.push(v);
        }
        ret.push(values);
    }
    anyhow::Ok(ret)
}

impl MysqlDB {
    fn new(pool: PoolManager, conn: CommDBConfig, read_only: bool) -> Self {
        Self {
            pool,
            config: conn,
            read_only,
        }
    }

    async fn query_count(&self, name: &str, where_str: Option<&str>) -> anyhow::Result<usize> {
        let mut conn = self.get_conn().await?;
        let mut s = format!("SELECT count(1) FROM {name} ");
        if let Some(where_str) = where_str {
            s.push_str(format!(" where {where_str}").as_str());
        }
        let row = sqlx::query(s.as_str()).fetch_one(conn.deref_mut()).await?;
        let count: i64 = row.get(0);
        anyhow::Ok(count as usize)
    }

    pub async fn get_conn(&self) -> anyhow::Result<PoolConnection<sqlx::MySql>> {
        let pool = match self
            .pool
            .get(&&DBConfig::Mysql {
                conn: self.config.clone(),
            })
            .await?
        {
            NexusPool::Mysql(p) => p.clone(),
            _ => unreachable!(),
        };

        let conn = pool.acquire().await?;
        return anyhow::Ok(conn);
    }
}

fn get_sql_seq(sql: &str) -> anyhow::Result<Vec<SqlSeq>> {
    let dialect = sqlparser::dialect::MySqlDialect {};

    sql_to_sql_seq(
        sql,
        &dialect,
        |s| {
            if is_statement_absolute_query(s) {
                crate::db::SqlKind::Query
            } else {
                crate::db::SqlKind::Execution
            }
        },
        |x| {
            let t = x.to_uppercase();

            if t.starts_with("SELECT") || t.starts_with("SHOW") || t.starts_with("EXPLAIN") {
                crate::db::SqlKind::Query
            } else {
                crate::db::SqlKind::Execution
            }
        },
    )
}

#[async_trait::async_trait]
impl MultiDBTrait for MysqlDB {
    async fn list_db(&self) -> anyhow::Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        let rows = sqlx::query("show databases")
            .fetch_all(conn.deref_mut())
            .await?;

        let mut ret = Vec::new();
        for r in rows.iter() {
            let name: String = r.get(0);
            ret.push(name);
        }
        anyhow::Ok(ret)
    }

    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        debug_assert!(self.config.db_name.is_none());
        let mut x = Clone::clone(self);
        x.config.db_name = Some(name.to_owned());
        Ok(Arc::new(x))
    }
}

#[async_trait::async_trait]
impl QuerySql for MysqlDB {
    async fn query_sql(
        &self, sql: &str,
    ) -> anyhow::Result<Vec<Result<TableListResp, ExeSqlError>>> {
        let seq = get_sql_seq(sql);
        let x = match seq {
            anyhow::Result::Ok(e) => e,
            anyhow::Result::Err(_err) => {
                log::error!("parse sql fail use as query");
                vec![SqlSeq {
                    sql: sql.to_string(),
                    kind: crate::db::SqlKind::Query,
                }]
            }
        };

        let mut ret = Vec::with_capacity(x.len());
        let mut conn = self.get_conn().await?;
        use crate::db::SqlKind;

        for c in x.iter() {
            let result = match c.kind {
                SqlKind::Query => {
                    let result = sqlx::query(&c.sql).fetch_all(conn.deref_mut()).await;
                    let result: Result<TableListResp, ExeSqlError> = match result {
                        Ok(rows) => {
                            let data = encode_rows(&rows)?;
                            let fields = get_rows_types(&rows);
                            Ok(TableListResp::new(fields, data.len(), data, &c.sql))
                        }
                        Err(err) => Err(ExeSqlError::new(
                            &c.sql,
                            anyhow::Error::msg(err.to_string()),
                        )),
                    };
                    result
                }

                SqlKind::Execution => {
                    let result = sqlx::query(&c.sql).execute(conn.deref_mut()).await;
                    let result: Result<TableListResp, ExeSqlError> = match result {
                        Ok(rows) => Ok(TableListResp::row_affected(&c.sql, rows.rows_affected())),
                        Err(err) => Err(ExeSqlError::new(
                            &c.sql,
                            anyhow::Error::msg(err.to_string()),
                        )),
                    };
                    result
                }
            };
            ret.push(result);
        }
        anyhow::Ok(ret)
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{init_log, TEST_HOST};
    // this test no need clean up, we use memory sqlite db.
    async fn make_test_db() -> MysqlDB {
        let pool_manager = PoolManager::new();
        let db = MysqlDB::new(
            pool_manager,
            CommDBConfig {
                host: TEST_HOST.to_string(),
                username: "root".to_string(),
                password: "root".to_string(),
                port: 3306,
                db_name: Some("test".to_string()),
            },
            false,
        );
        db
    }

    #[tokio::test]
    async fn test_export() {
        // init_log();
        let table_name = "test_export";
        let db = make_test_db().await;
        let mut conn = db.get_conn().await.unwrap();
        let rs =
            vec![r#"drop table if exists test_export;"#,
            r#"CREATE TABLE test_export (
                id INT AUTO_INCREMENT PRIMARY KEY,
                name VARCHAR(50),
                age INT,
                email VARCHAR(100)
            );"# , 
        "INSERT INTO test_export (name, age, email) VALUES ('John', 25, 'john@example.com');" ,
        "INSERT INTO test_export (name, age, email) VALUES ('Jane', 30, 'jane@example.com');"  ,
        "INSERT INTO test_export (name, age, email) VALUES ('Bob', 40, 'bob@example.com');",

        ];
        for s in rs.iter() {
            sqlx::query(*s).execute(conn.deref_mut()).await.unwrap();
        }
        let db = &db as &dyn DBTrait;

        {
            let s = db
                .export_table_to_string(table_name, ExportKind::Json { pretty: true })
                .await
                .unwrap();
            println!("json:{}", s);
            let _: serde_json::Value = serde_json::from_str(s.as_str()).unwrap();
        }
        {
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::CSV {
                        include_header: true,
                        delimiter: ',',
                    },
                )
                .await
                .unwrap();
            println!("csv:{}", s);
        }
        {
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: false,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", s);
        }
        {
            let s = db
                .export_table_to_string(
                    table_name,
                    ExportKind::Sql {
                        include_table_schema: true,
                    },
                )
                .await
                .unwrap();
            println!("sql:{}", s);
        }
    }

    #[tokio::test]
    async fn test_index() {
        init_log();
        let table_name = "test_index";
        let db = make_test_db().await;
        let mut conn = db.get_conn().await.unwrap();
        let _ = sqlx::query(&format!("drop table {table_name}"))
            .execute(conn.deref_mut())
            .await;

        sqlx::query(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    id INT PRIMARY KEY,
                    name VARCHAR(255),
                    age INT,
                    email VARCHAR(255),
                    INDEX idx_name (name),
                    UNIQUE INDEX idx_email (email),
                    FULLTEXT INDEX idx_fulltext (name, email)
                );
            "#
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();

        let indexes = db.show_indexes(table_name).await.unwrap();
        println!("{:?}", indexes);
    }
    #[tokio::test]
    async fn test1() {
        init_log();

        let table_name = "test_types1";

        let db = make_test_db().await;
        let mut conn = db.get_conn().await.unwrap();
        let _ = sqlx::query(&format!("drop table {table_name}"))
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
            CREATE TABLE {table_name} (
                tinyint_col TINYINT(1),
                boolean_col BOOLEAN,
                tinyint_col2 TINYINT,
                smallint_col SMALLINT,
                int_col INT,
                bigint_col BIGINT
            );
            "#
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();
        let db = &db as &dyn crate::db::DBTrait;
        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert("tinyint_col".to_string(), Some("1".to_string()));
        values.insert("boolean_col".to_string(), Some("false".to_string()));
        values.insert("tinyint_col2".to_string(), Some("2".to_string()));
        values.insert("smallint_col".to_string(), Some("3".to_string()));
        values.insert("int_col".to_string(), Some("4".to_string()));
        values.insert("bigint_col".to_string(), Some("5".to_string()));

        db.insert(table_name, values).await.unwrap();

        let cols = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", cols);
    }

    #[tokio::test]
    async fn test2() {
        init_log();

        let table_name = "test_types2";

        let db = make_test_db().await;
        let mut conn = db.get_conn().await.unwrap();
        let _ = sqlx::query(&format!("drop table {table_name}"))
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    u8_col TINYINT UNSIGNED,
                    u16_col SMALLINT UNSIGNED,
                    u32_col INT UNSIGNED,
                    u64_col BIGINT UNSIGNED,
                    f32_col FLOAT,
                    f64_col DOUBLE
                );
            "#
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();
        let db = &db as &dyn crate::db::DBTrait;
        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert("u8_col".to_string(), Some("1".to_string()));
        values.insert("u16_col".to_string(), Some("2".to_string()));
        values.insert("u32_col".to_string(), Some("2".to_string()));
        values.insert("u64_col".to_string(), Some("3".to_string()));
        values.insert("f32_col".to_string(), Some("4".to_string()));
        values.insert("f64_col".to_string(), Some("5".to_string()));

        db.insert(table_name, values).await.unwrap();

        let cols = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", cols);
    }

    #[tokio::test]
    async fn test3() {
        init_log();

        let table_name = "test_types3";

        let db = make_test_db().await;
        let mut conn = db.get_conn().await.unwrap();
        let _ = sqlx::query(&format!("drop table {table_name}"))
            .execute(conn.deref_mut())
            .await;
        sqlx::query(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    varchar_col VARCHAR(255),
                    char_col CHAR(10),
                    text_col TEXT,
                    varbinary_col VARBINARY(255),
                    binary_col BINARY(10),
                    blob_col BLOB
                );

            "#
            )
            .as_str(),
        )
        .execute(conn.deref_mut())
        .await
        .unwrap();
        let db = &db as &dyn crate::db::DBTrait;
        let mut values: HashMap<String, Option<String>> = HashMap::new();
        values.insert("varchar_col".to_string(), Some("x".to_string()));
        values.insert("char_col".to_string(), Some("y".to_string()));
        values.insert("text_col".to_string(), Some("z".to_string()));
        values.insert("varbinary_col".to_string(), Some("0x01020304".to_string()));
        values.insert("binary_col".to_string(), Some("0x0102030405".to_string()));
        values.insert("blob_col".to_string(), Some("0x01020304".to_string()));

        db.insert(table_name, values).await.unwrap();

        let cols = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("{:?}", cols);
    }
}
