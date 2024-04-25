use std::collections::HashMap;
use std::ops::DerefMut;
use std::sync::Arc;

use chrono::FixedOffset;
use futures_core::future::BoxFuture;
use futures_util::StreamExt;
use tiberius::{Client, ColumnData, ColumnType, FromSql};
use tokio::net::TcpStream;
use tokio_util::compat::Compat;

use crate::completion::{AutoCompletionTrait, CompletionItem, LexerCompletionBackend};
use crate::datasource::CommDBConfig;
use crate::db::{
    format_data_time_fix_offset, sql_to_sql_seq, CreateTableAndIndexSql, DBTrait, ExeSqlError,
    Field, Index, IndexKind, ListTable, ListView, QuerySql, SqlExecuteRet, SqlSeq, TableListResp,
};
use crate::pool::{NexusPool, PoolManager};

#[derive(Clone)]
pub struct SqlServer {
    config: CommDBConfig,
    trust_cert: bool,
    pm: PoolManager,
    read_only: bool,
}

impl SqlServer {
    fn new(config: CommDBConfig, trust_cert: bool, pm: PoolManager, read_only: bool) -> Self {
        Self {
            config,
            trust_cert,
            pm,
            read_only,
        }
    }
}

fn get_column_type_string(c: ColumnType) -> String {
    let x = format!("{:?}", c);
    x.to_lowercase()
}

struct ExtractCoulmnType {
    typ: &'static str,
}

impl<'a> FromSql<'a> for ExtractCoulmnType {
    fn from_sql(value: &'a ColumnData<'static>) -> tiberius::Result<Option<Self>> {
        let typ = match value {
            tiberius::ColumnData::U8(_) => Some("tinyint"),
            tiberius::ColumnData::I16(_) => Some("smallint"),
            tiberius::ColumnData::I32(_) => Some("int"),
            tiberius::ColumnData::I64(_) => Some("bigint"),
            tiberius::ColumnData::F32(_) => Some("float"),
            tiberius::ColumnData::F64(_) => Some("float"),
            tiberius::ColumnData::Bit(_) => Some("bit"),
            tiberius::ColumnData::String(_) => None,
            tiberius::ColumnData::Guid(_) => Some("guid"),
            tiberius::ColumnData::Binary(_) => None,
            tiberius::ColumnData::Numeric(_) => Some("numeric"),
            tiberius::ColumnData::Xml(_) => Some("xml"),
            tiberius::ColumnData::DateTime(_) => Some("datetime"),
            tiberius::ColumnData::SmallDateTime(_) => Some("smalldatetime"),
            tiberius::ColumnData::Time(_) => Some("time"),
            tiberius::ColumnData::Date(_) => Some("date"),
            tiberius::ColumnData::DateTime2(_) => Some("datetime2"),
            tiberius::ColumnData::DateTimeOffset(_) => Some("datetimeoffset"),
        };
        Ok(typ.map(|x| Self { typ: x }))
    }
}
fn get_fields_from_row(row: &tiberius::Row) -> Vec<Field> {
    let mut ret = Vec::with_capacity(row.columns().len());
    for (index, col) in row.columns().iter().enumerate() {
        let typ: Option<ExtractCoulmnType> = row.get(index);
        let typ = typ
            .map(|x| x.typ.to_string())
            .unwrap_or(get_column_type_string(col.column_type()));
        ret.push(Field::from_name_type(col.name().to_string(), typ));
    }
    ret
}
fn get_fields_from_meta(meta: &tiberius::ResultMetadata) -> Vec<Field> {
    let mut ret = Vec::with_capacity(meta.columns().len());
    for c in meta.columns().iter() {
        ret.push(Field::from_name_type(
            c.name().to_string(),
            get_column_type_string(c.column_type()),
        ));
    }
    ret
}

struct ConvertColumnDataToString {
    value: String,
}

impl<'a> tiberius::FromSql<'a> for ConvertColumnDataToString {
    fn from_sql(value: &'a tiberius::ColumnData<'static>) -> tiberius::Result<Option<Self>> {
        let value = match value {
            tiberius::ColumnData::U8(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::I16(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::I32(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::I64(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::F32(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::F64(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::Bit(x) => x.as_ref().map(|x| if *x { 1 } else { 0 }.to_string()),
            tiberius::ColumnData::String(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::Guid(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::Binary(x) => {
                x.as_ref().map(|x| format!("0X{}", hex::encode_upper(x)))
            }
            tiberius::ColumnData::Numeric(x) => x.as_ref().map(|x| x.to_string()),

            tiberius::ColumnData::Xml(x) => x.as_ref().map(|x| x.to_string()),
            tiberius::ColumnData::DateTime(_)
            | tiberius::ColumnData::DateTime2(_)
            | tiberius::ColumnData::SmallDateTime(_) => {
                chrono::NaiveDateTime::from_sql(value)
                    .ok()
                    .flatten()
                    .map(|x| x.to_string())
            }

            tiberius::ColumnData::Date(_) => {
                chrono::NaiveTime::from_sql(value)
                    .ok()
                    .flatten()
                    .map(|x| x.to_string())
            }

            tiberius::ColumnData::Time(_x) => {
                chrono::NaiveTime::from_sql(value)
                    .ok()
                    .flatten()
                    .map(|x| x.to_string())
            }

            tiberius::ColumnData::DateTimeOffset(_) => {
                chrono::DateTime::<FixedOffset>::from_sql(value)
                    .ok()
                    .flatten()
                    .map(|x| format_data_time_fix_offset(&x))
            }
        };
        Ok(value.map(|x| Self { value: x }))
    }
}

fn make_table_list_from_meta_and_row(
    meta: &tiberius::ResultMetadata, rows: &Vec<tiberius::Row>, total: usize,
    sql: impl Into<String>,
) -> anyhow::Result<TableListResp> {
    let fiels = if rows.len() > 0 {
        get_fields_from_row(rows.get(0).unwrap())
    } else {
        get_fields_from_meta(meta)
    };
    let mut data = vec![];
    for r in rows.iter() {
        let mut values: Vec<Option<String>> = Vec::new();
        for index in 0..meta.columns().len() {
            let v: Option<ConvertColumnDataToString> = r.try_get(index)?;
            values.push(v.map(|x| x.value));
        }
        data.push(values);
    }

    anyhow::Ok(TableListResp::new(fiels, total, data, sql))
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
    } else {
        // sql server 的offet子句只能和order by一起使用
        sql.push_str(" ORDER  BY (SELECT NULL) ")
    }

    let offset = (page - 1) * page_size;
    sql.push_str(format!(" OFFSET {} ROWS FETCH NEXT {} ROWS ONLY", offset, page_size).as_str());

    sql
}

#[async_trait::async_trait]
impl DBTrait for SqlServer {
    fn read_only(&self) -> bool {
        self.read_only
    }

    async fn create_table_and_index_sql(
        &self, _table_name: &str,
    ) -> anyhow::Result<CreateTableAndIndexSql> {
        unimplemented!()
    }

    fn backend_name(&self) -> &str {
        "sqlserver"
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
        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let conn = conn.deref_mut();
        let total = SqlServer::query_count(conn, view_name, where_str.clone()).await?;
        let sql = gen_select_sql(view_name, where_str, order_by, page, page_size);

        let mut stream = conn
            .query(&sql, &[])
            .await
            .expect(format!("sql :{}", sql).as_str());
        let mut meta = None;
        let mut rows = Vec::with_capacity(page_size);
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    rows.push(row);
                }
                tiberius::QueryItem::Metadata(m) => meta = Some(m),
            }
        }
        let ret = match meta {
            Some(x) => make_table_list_from_meta_and_row(&x, &rows, total as usize, sql)?,
            None => anyhow::bail!("SELECT should yield at lease one meta"),
        };
        anyhow::Ok(ret)
    }

    async fn drop_view(&self, view_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let s = format!("DROP VIEW {view_name}");
        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let conn = conn.deref_mut();

        conn.execute(s.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn drop_table(&self, table: &str) -> anyhow::Result<()> {
        self.can_write()?;

        let s = format!("DROP TABLE {table}");
        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let conn = conn.deref_mut();
        conn.execute(s.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn list_view(&self) -> anyhow::Result<Vec<ListView>> {
        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let conn = conn.deref_mut();
        let mut stream = conn
            .query(
                r#"
                SELECT name
                FROM sys.views;
        "#,
                &[],
            )
            .await?;
        let mut ret = Vec::new();

        while let Some(item) = stream.next().await {
            let item = item?;

            match item {
                tiberius::QueryItem::Row(r) => {
                    let table_name: Option<&str> = r.try_get(0)?;
                    ret.push(table_name.unwrap().to_string());
                }
                tiberius::QueryItem::Metadata(_) => {
                    // I know the meta pretty well
                }
            }
        }
        anyhow::Ok(ret.into_iter().map(ListView::from).collect())
    }

    async fn list_table(&self) -> anyhow::Result<Vec<ListTable>> {
        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let conn = conn.deref_mut();
        let mut stream = conn
            .query(
                r#"
            SELECT TABLE_NAME
            FROM INFORMATION_SCHEMA.TABLES
            WHERE TABLE_TYPE = 'BASE TABLE';
        "#,
                &[],
            )
            .await?;
        let mut ret = Vec::new();

        while let Some(item) = stream.next().await {
            let item = item?;

            match item {
                tiberius::QueryItem::Row(r) => {
                    let table_name: Option<&str> = r.try_get(0)?;
                    ret.push(table_name.unwrap().to_string());
                }

                tiberius::QueryItem::Metadata(_) => {
                    // I know the meta pretty well
                }
            }
        }
        anyhow::Ok(ret.into_iter().map(ListTable::from).collect())
    }

    async fn show_columns(&self, table_name: &str) -> anyhow::Result<Vec<Field>> {
        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let mut conn = conn.deref_mut();
        let mut stream = conn
            .query(
                format!(
                    r#"
                SELECT COLUMN_NAME,DATA_TYPE,COLUMN_DEFAULT,IS_NULLABLE
                FROM INFORMATION_SCHEMA.COLUMNS
                WHERE TABLE_NAME = '{table_name}' order by  ORDINAL_POSITION
                "#
                ),
                &[],
            )
            .await?;
        let mut ret = Vec::new();

        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    let name: Option<&str> = row.get(0);
                    let name = name.unwrap().to_string();
                    let data_type: Option<&str> = row.get(1);
                    let data_type = data_type.unwrap().to_string();
                    let default_value: Option<&str> = row.get(2);
                    let nullable: Option<&str> = row.get(3);
                    ret.push(Field::new(
                        name,
                        data_type,
                        nullable
                            .map(|x| x.to_uppercase() == "YES")
                            .unwrap_or_default(),
                        default_value.map(|x| x.to_string()),
                        false,
                        false,
                    ));
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        drop(stream);
        for f in ret.iter_mut() {
            let is_primary = SqlServer::is_primary(&mut conn, table_name, &f.name).await?;
            let is_auto_increment =
                SqlServer::is_auto_increment(&mut conn, table_name, &f.name).await?;
            f.is_primary = is_primary;
            f.is_auto_increment = is_auto_increment;
        }

        anyhow::Ok(ret)
    }

    async fn show_table(
        &self, table_name: &str, where_str: Option<&str>, order_by: Option<&str>, page: usize,
        page_size: usize,
    ) -> anyhow::Result<TableListResp> {
        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let conn = conn.deref_mut();
        let total = SqlServer::query_count(conn, table_name, where_str.clone()).await?;
        let sql = gen_select_sql(table_name, where_str, order_by, page, page_size);

        let mut stream = conn
            .query(&sql, &[])
            .await
            .expect(format!("sql :{}", sql).as_str());
        let mut meta = None;
        let mut rows = Vec::with_capacity(page_size);
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    rows.push(row);
                }
                tiberius::QueryItem::Metadata(m) => meta = Some(m),
            }
        }
        let cols = self.show_columns(table_name).await?;
        let mut ret = match meta {
            Some(x) => make_table_list_from_meta_and_row(&x, &rows, total as usize, sql)?,
            None => anyhow::bail!("SELECT should yield at lease one meta"),
        };
        // infered schema is accurate as show_columns
        ret.fields = cols;
        anyhow::Ok(ret)
    }

    async fn show_indexes(&self, table_name: &str) -> anyhow::Result<Vec<Index>> {
        let sql = format!(
            r#"
        SELECT
    i.name AS IndexName,
    i.is_unique AS IsUnique,
    c.name AS ColumnName
FROM sys.indexes AS i
INNER JOIN sys.index_columns AS ic ON i.object_id = ic.object_id AND i.index_id = ic.index_id
INNER JOIN sys.columns AS c ON ic.object_id = c.object_id AND ic.column_id = c.column_id
WHERE i.object_id = OBJECT_ID('{table_name}')
        "#
        );

        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let conn = conn.deref_mut();
        let mut stream = conn.query(sql.as_str(), &[]).await?;
        let mut ret: HashMap<String, Index> = HashMap::new();
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(r) => {
                    let name: Option<&str> = r.try_get(0)?;
                    let name = name.unwrap().to_string();
                    let is_unique: Option<bool> = r.try_get(1)?;
                    let col: Option<&str> = r.try_get(2)?;
                    let col = col.unwrap().to_string();
                    if let Some(x) = ret.get_mut(&name) {
                        x.on.push(col);
                    } else {
                        ret.insert(
                            name.clone(),
                            Index {
                                name,
                                kind: if is_unique.unwrap_or_default() {
                                    IndexKind::Unique
                                } else {
                                    IndexKind::Normal
                                },
                                on: vec![col],
                            },
                        );
                    }
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        anyhow::Ok(ret.into_iter().map(|(_, v)| v).collect())
    }

    async fn insert(
        &self, table_name: &str, values: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fs = self.show_columns(table_name).await?;
        let s = (self as &dyn crate::db::DBTrait).insert_sql(table_name, &fs, &values)?;

        log::trace!("insert sql:{}", s);

        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let conn = conn.deref_mut();
        let _r = conn.execute(s.as_str(), &[]).await?;
        anyhow::Ok(SqlExecuteRet {
            sql: s,
            changes: 1, // I know this
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
        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let conn = conn.deref_mut();
        let r = conn.execute(s.as_str(), &[]).await?;
        anyhow::Ok(SqlExecuteRet::from((s, r.rows_affected().iter().sum())))
    }

    /// format a value make it suitable for insert into sql.
    /// this method current use in `insert_sql` etc.
    fn put_value_in_sql(&self, f: &Field, value: Option<&str>) -> anyhow::Result<String> {
        let default_value = value.map(|x| x.to_string()).unwrap_or("null".to_string());
        let single_quote = value
            .map(|x| format!("'{}'", x))
            .unwrap_or("null".to_string());
        let v = match f.typ.as_str().to_uppercase().as_str() {
            "TINYINT" | "SMALLINT" | "INT" | "BIGINT" | "FLOAT" => default_value,
            "BIT" => default_value,
            "NVARCHAR" | "VARCHAR" | "NCHAR" | "CHAR" | "NTEXT" | "TEXT" => single_quote,
            "BINARY" | " VARBINARY" | "IMAGE" | "UNIQUEIDENTIFIER" | "NUMERIC" | "DECIMAL" => {
                default_value
            }
            "XML" | "DATETIME" | "DATETIME2" | "SMALLDATETIME" | "DATE" | "TIME"
            | "DATETIMEOFFSET" => single_quote,
            _ => anyhow::bail!("type '{}' not support", f.typ.as_str()),
        };
        log::trace!("type:{:?} encode '{:?}' as #{}#", f.typ, value, v);
        anyhow::Ok(v)
    }

    async fn truncate_table(&self, table_name: &str) -> anyhow::Result<()> {
        self.can_write()?;
        let s = format!(
            r#"
            TRUNCATE TABLE {table_name};
        "#
        );
        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let conn = conn.deref_mut();
        conn.execute(s.as_str(), &[]).await?;
        anyhow::Ok(())
    }

    async fn delete(
        &self, table_name: &str, wheres: HashMap<String, Option<String>>,
    ) -> anyhow::Result<SqlExecuteRet> {
        self.can_write()?;

        let fields = self.show_columns(table_name).await?;
        let sql = (self as &dyn crate::db::DBTrait).delete_sql(table_name, &fields, &wheres)?;
        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let conn = conn.deref_mut();

        let r = conn.execute(sql.as_str(), &[]).await?;
        anyhow::Ok(SqlExecuteRet {
            sql,
            changes: r.rows_affected().iter().sum(),
            last_insert_row_id: None,
            last_insert_id_string: None,
        })
    }
}

impl SqlServer {
    async fn query_count(
        conn: &mut Connection, name: &str, where_str: Option<&str>,
    ) -> anyhow::Result<i32> {
        let mut stream = conn
            .query(
                format!(
                    "SELECT count(*) from {name} {}",
                    match where_str {
                        Some(x) => format!("  where {} ", x),
                        None => "".to_string(),
                    }
                ),
                &[],
            )
            .await?;

        let mut count: Option<i32> = None;
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(row) => {
                    count = row.get(0);
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        anyhow::Ok(count.unwrap_or_default())
    }
}
impl SqlServer {
    async fn is_primary(
        conn: &mut Connection, table_name: &str, col_name: &str,
    ) -> anyhow::Result<bool> {
        let s = format!(
            r#"
        SELECT count(*)
        FROM INFORMATION_SCHEMA.KEY_COLUMN_USAGE
        WHERE TABLE_NAME = '{table_name}' AND COLUMN_NAME = '{col_name}' AND CONSTRAINT_NAME LIKE 'PK_%'
        "#
        );
        let mut count: Option<i32> = None;
        let mut stream = conn.query(s.as_str(), &[]).await?;
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(x) => {
                    count = x.get(0);
                    break;
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }

        let d = count.map(|x| x > 0).unwrap_or_default();
        anyhow::Ok(d)
    }

    async fn is_auto_increment(
        conn: &mut Connection, table_name: &str, col_name: &str,
    ) -> anyhow::Result<bool> {
        let s = format!(
            r#"
            SELECT COLUMN_NAME
            FROM INFORMATION_SCHEMA.COLUMNS
            WHERE TABLE_NAME = '{table_name}' AND COLUMNPROPERTY(object_id(TABLE_SCHEMA + '.{table_name}' ), COLUMN_NAME, 'IsIdentity') = 1
        "#
        );
        let mut name: Option<String> = None;
        let mut stream = conn.query(s.as_str(), &[]).await?;
        while let Some(item) = stream.next().await {
            let item = item?;
            match item {
                tiberius::QueryItem::Row(x) => {
                    name = x.get::<'_, &str, _>(0).map(|x| x.to_string());

                    break;
                }
                tiberius::QueryItem::Metadata(_) => {}
            }
        }
        let d = name.map(|x| x.as_str() == col_name).unwrap_or_default();
        anyhow::Ok(d)
    }
}

type Connection = Client<Compat<TcpStream>>;
#[async_trait::async_trait]
impl QuerySql for SqlServer {
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
        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let conn = conn.deref_mut();
        use crate::db::SqlKind;

        for c in x.iter() {
            match c.kind {
                SqlKind::Query => {
                    let result = conn
                        .query(&c.sql, &[])
                        .await
                        .map_err(|e| anyhow::Error::msg(e.to_string()));
                    let mut result = match result {
                        Ok(x) => x,
                        Err(err) => {
                            ret.push(Err(ExeSqlError::new(c.sql.as_str(), err)));
                            continue;
                        }
                    };

                    let mut meta_prev = None;
                    let mut rows = Vec::new();
                    while let Some(item) = result.next().await {
                        let item = item?;
                        match item {
                            tiberius::QueryItem::Row(row) => rows.push(row),
                            tiberius::QueryItem::Metadata(meta) => {
                                if meta_prev.is_some() {
                                    let tablelist = make_table_list_from_meta_and_row(
                                        meta_prev.as_ref().unwrap(),
                                        &rows,
                                        rows.len(),
                                        &c.sql,
                                    )?;
                                    ret.push(Ok(tablelist));
                                    rows = vec![];
                                }
                                meta_prev = Some(meta);
                            }
                        }
                        // last one
                        if meta_prev.is_some() {
                            let tablelist = make_table_list_from_meta_and_row(
                                meta_prev.as_ref().unwrap(),
                                &rows,
                                rows.len(),
                                &c.sql,
                            )?;
                            ret.push(Ok(tablelist));
                        }
                    }
                }

                SqlKind::Execution => {
                    let result = conn.execute(c.sql.as_str(), &[]).await;
                    let result: Result<TableListResp, ExeSqlError> = match result {
                        Ok(rows) => {
                            Ok(TableListResp::row_affected(
                                &c.sql,
                                rows.rows_affected().iter().sum(),
                            ))
                        }
                        Err(err) => {
                            Err(ExeSqlError::new(
                                &c.sql,
                                anyhow::Error::msg(err.to_string()),
                            ))
                        }
                    };
                    ret.push(result);
                }
            };
        }
        anyhow::Ok(ret)
    }
}
impl SqlServer {
    async fn get_conn(&self) -> anyhow::Result<bb8::Pool<bb8_tiberius::ConnectionManager>> {
        let p = self
            .pm
            .get(&crate::datasource::DBConfig::SqlServer {
                config: self.config.clone(),
                trust_cert: self.trust_cert,
            })
            .await?;
        let p = match p {
            NexusPool::SqlServer(s) => s,
            _ => unreachable!(),
        };
        anyhow::Ok(p)
    }

    async fn with_conn<R>(
        &self,
        call_back: impl FnOnce(&mut Client<Compat<TcpStream>>) -> BoxFuture<anyhow::Result<R>>,
    ) -> anyhow::Result<R> {
        let conn = self.get_conn().await?;
        let mut conn = conn.get().await?;
        let conn = conn.deref_mut();
        call_back(conn).await
    }
}

fn get_sql_seq(sql: &str) -> anyhow::Result<Vec<SqlSeq>> {
    let dialect = sqlparser::dialect::MsSqlDialect {};
    use sqlparser::ast::Statement;
    sql_to_sql_seq(
        sql,
        &dialect,
        |s| {
            match s {
                Statement::Query(_) => crate::db::SqlKind::Query,
                _ => crate::db::SqlKind::Execution,
            }
        },
        |x| {
            let t = x.to_uppercase();
            if t.starts_with("SELECT") || t.starts_with("EXPLAIN") {
                crate::db::SqlKind::Query
            } else {
                crate::db::SqlKind::Execution
            }
        },
    )
}

#[async_trait::async_trait]
impl crate::db::MultiDBTrait for SqlServer {
    async fn list_db(&self) -> anyhow::Result<Vec<String>> {
        unimplemented!()
    }

    async fn open_db(&self, name: &str) -> anyhow::Result<Arc<dyn DBTrait>> {
        debug_assert!(self.config.db_name.is_none());
        let mut x = Clone::clone(self);
        x.config.db_name = Some(name.to_owned());
        Ok(Arc::new(x))
    }
}
#[async_trait::async_trait]
impl LexerCompletionBackend for SqlServer {
    type Dialect = sqlparser::dialect::MsSqlDialect;

    const BUILD_IN_FUNCTIONS: &'static [&'static str] = BUILD_IN_FUNCTIONS;
    const KEY_WORDS: &'static [&'static str] = KEYWORDS;
    const TYPES: &'static [&'static str] = TYPES;

    async fn list_table(&self) -> anyhow::Result<Vec<String>> {
        crate::db::DBTrait::list_table(self)
            .await
            .map(|x| x.into_iter().map(|x| x.table).collect())
    }

    async fn list_db(&self) -> anyhow::Result<Vec<String>> {
        // TODO
        anyhow::Ok(vec![])
    }

    async fn list_columns(&self, table_name: &str) -> anyhow::Result<Vec<String>> {
        crate::db::DBTrait::show_columns(self, table_name)
            .await
            .map(|x| x.into_iter().map(|x| x.name).collect())
    }
}

#[async_trait::async_trait]
impl AutoCompletionTrait for SqlServer {
    async fn auto_completion(
        &self, sql: &str, pos: sqlparser::tokenizer::Location,
    ) -> anyhow::Result<Vec<CompletionItem>> {
        LexerCompletionBackend::do_auto_completion(
            self,
            sql,
            &sqlparser::dialect::MsSqlDialect {},
            pos,
        )
        .await
    }
}

#[cfg(test)]
mod tests {

    use futures_util::FutureExt;

    use super::*;
    use crate::{init_log, TEST_HOST};

    fn make_test_object() -> SqlServer {
        let pg = SqlServer::new(
            CommDBConfig {
                host: TEST_HOST.to_string(),
                port: 1433,
                username: "sa".to_string(),
                password: "fdsOPNHB76".to_string(),
                db_name: Some("test".to_string()),
            },
            true,
            Default::default(),
            false,
        );

        pg
    }

    #[tokio::test]
    async fn test_view() {
        let db = make_test_object();
        let db = &db as &dyn crate::db::DBTrait;

        let resp = db.show_view("test_view", None, None, 1, 100).await.unwrap();
        println!("resp:{:?}", resp);
    }

    #[tokio::test]
    async fn test_index() {
        let db = make_test_object();

        db.with_conn(|conn| {
            async {
                conn.execute(
                    r#"
                    drop table if exists test_index_table;
                    -- 创建表
                    CREATE TABLE test_index_table (
                        Column1 int,
                        Column2 int,
                    );
                    -- 添加唯一索引
                    CREATE UNIQUE INDEX IX_YourTableName_Column1_Column2
                        ON test_index_table (Column1, Column2);
        "#,
                    &[],
                )
                .await
                .map(|_| ())
                .map_err(|e| anyhow::Error::msg(e.to_string()))
            }
            .boxed()
        })
        .await
        .unwrap();
        let db = &db as &dyn crate::db::DBTrait;
        let indexes = db.show_indexes("test_index_table").await.unwrap();
        println!("indexes:{:?}", indexes);
        db.drop_table("test_index_table").await.unwrap();
    }

    #[tokio::test]
    async fn test() {
        let db = make_test_object();
        let db = &db as &dyn crate::db::DBTrait;
        let tables = db.list_table().await.unwrap();
        for t in tables.iter() {
            println!("t :{:?}", t);
            let cols = db.show_columns(t.table.as_str()).await.unwrap();
            println!("cols:{:?}", cols);
            let list = db
                .show_table(t.table.as_str(), None, None, 1, 100)
                .await
                .unwrap();
            println!("list:{:?}", list);
            println!()
        }
    }

    #[tokio::test]
    async fn test_type1() {
        init_log();
        let db = make_test_object();
        let conn = db.get_conn().await.unwrap();
        let mut conn = conn.get().await.unwrap();
        let conn = conn.deref_mut();
        let table_name = "test_type1";
        let _ = conn.execute(format!("drop table {table_name}"), &[]).await;

        conn.execute(
            format!(
                r#"
            CREATE TABLE {table_name} (
                id INT IDENTITY(1,1) PRIMARY KEY,
                col_tinyint TINYINT,
                col_smallint SMALLINT,
                col_int INT,
                col_bigint BIGINT,
                col_float FLOAT(24),
                col_double FLOAT(53)
            );
            "#
            ),
            &[],
        )
        .await
        .unwrap();
        let db = &db as &dyn crate::db::DBTrait;
        let mut values = HashMap::new();
        values.insert("col_tinyint".to_string(), Some("1".to_string()));
        values.insert("col_smallint".to_string(), Some("100".to_string()));
        values.insert("col_int".to_string(), Some("1000".to_string()));
        values.insert("col_bigint".to_string(), Some("10000".to_string()));
        values.insert("col_float".to_string(), Some("3.14".to_string()));
        values.insert("col_double".to_string(), Some("3.14159".to_string()));
        db.insert(table_name, values.clone()).await.unwrap();
        db.update(table_name, values, {
            let mut wheres = HashMap::new();
            wheres.insert("id".to_string(), Some("1".to_string()));
            wheres
        })
        .await
        .unwrap();
        let list = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("ret:{:?}", list);
    }

    #[tokio::test]
    async fn test_type2() {
        init_log();
        let db = make_test_object();
        let conn = db.get_conn().await.unwrap();
        let mut conn = conn.get().await.unwrap();
        let conn = conn.deref_mut();
        let table_name = "test_type2";
        let _ = conn.execute(format!("drop table {table_name}"), &[]).await;

        conn.execute(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    id INT IDENTITY(1,1) PRIMARY KEY,
                    bit_column bit,
                    nvarchar_column nvarchar(50),
                    varchar_column varchar(50),
                    nchar_column nchar(50),
                    char_column char(50),
                    ntext_column ntext,
                    text_column text
                  );
            "#
            ),
            &[],
        )
        .await
        .unwrap();
        let db = &db as &dyn crate::db::DBTrait;
        let mut values = HashMap::new();
        values.insert("bit_column".to_string(), Some("1".to_string()));
        values.insert("nvarchar_column".to_string(), Some("b".to_string()));
        values.insert("varchar_column".to_string(), Some("c".to_string()));

        values.insert("nchar_column".to_string(), Some("d".to_string()));
        values.insert("char_column".to_string(), Some("e".to_string()));
        values.insert("ntext_column".to_string(), Some("f".to_string()));
        values.insert("text_column".to_string(), Some("g".to_string()));

        db.insert(table_name, values.clone()).await.unwrap();
        db.update(table_name, values, {
            let mut wheres = HashMap::new();
            wheres.insert("id".to_string(), Some("1".to_string()));
            wheres
        })
        .await
        .unwrap();
        let list = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("ret:{:?}", list);
    }

    #[tokio::test]
    async fn test_type3() {
        init_log();
        let db = make_test_object();
        let conn = db.get_conn().await.unwrap();
        let mut conn = conn.get().await.unwrap();

        let conn = conn.deref_mut();
        let table_name = "test_type3";
        let _ = conn.execute(format!("drop table {table_name}"), &[]).await;

        conn.execute(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    id INT IDENTITY(1,1) PRIMARY KEY,
                    BinaryColumn binary(10),
                    VarBinaryColumn varbinary(50),
                    ImageColumn image,
                    UniqueIdentifierColumn uniqueidentifier,
                    NumericColumn numeric(10, 2),
                    DecimalColumn decimal(12, 4)
                  );
            "#
            ),
            &[],
        )
        .await
        .unwrap();
        let db = &db as &dyn crate::db::DBTrait;
        let mut values = HashMap::new();
        values.insert("BinaryColumn".to_string(), Some("0x0123456789".to_string()));
        values.insert("VarBinaryColumn".to_string(), Some("0xABCDEF".to_string()));
        values.insert(
            "ImageColumn".to_string(),
            Some("0x0123456789ABCDEF".to_string()),
        );

        values.insert(
            "UniqueIdentifierColumn".to_string(),
            Some("NEWID()".to_string()),
        );
        values.insert("NumericColumn".to_string(), Some("123.45".to_string()));
        values.insert("DecimalColumn".to_string(), Some("12.3456".to_string()));
        db.insert(table_name, values.clone()).await.unwrap();
        db.update(table_name, values, {
            let mut wheres = HashMap::new();
            wheres.insert("id".to_string(), Some("1".to_string()));
            wheres
        })
        .await
        .unwrap();
        let list = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("ret:{:?}", list);
    }

    #[tokio::test]
    async fn test_type4() {
        init_log();
        let db = make_test_object();
        let conn = db.get_conn().await.unwrap();
        let mut conn = conn.get().await.unwrap();

        let conn = conn.deref_mut();
        let table_name = "test_type4";
        let _ = conn.execute(format!("drop table {table_name}"), &[]).await;

        conn.execute(
            format!(
                r#"
                CREATE TABLE {table_name} (
                    id INT IDENTITY(1,1) PRIMARY KEY,
                    XmlData XML,
                    DateTimeData DATETIME,
                    DateTime2Data DATETIME2,
                    SmallDateTimeData SMALLDATETIME,
                    DateData DATE,
                    TimeData TIME,
                    DateTimeOffsetData DATETIMEOFFSET
                );
            "#
            ),
            &[],
        )
        .await
        .unwrap();
        let db = &db as &dyn crate::db::DBTrait;
        let mut values = HashMap::new();
        values.insert(
            "XmlData".to_string(),
            Some("<root><data>XML Data</data></root>".to_string()),
        );
        values.insert(
            "DateTimeData".to_string(),
            Some("2022-01-01 12:34:56".to_string()),
        );
        values.insert(
            "DateTime2Data".to_string(),
            Some("2022-01-01 12:34:56.1234567".to_string()),
        );
        values.insert(
            "SmallDateTimeData".to_string(),
            Some("2022-01-01 12:34:00".to_string()),
        );

        values.insert("DateData".to_string(), Some("2022-01-01".to_string()));
        values.insert("TimeData".to_string(), Some("12:34:56".to_string()));
        values.insert(
            "DateTimeOffsetData".to_string(),
            Some("2022-01-01 12:34:56.1234567 +01:00".to_string()),
        );
        db.insert(table_name, values.clone()).await.unwrap();
        db.update(table_name, values, {
            let mut wheres = HashMap::new();
            wheres.insert("id".to_string(), Some("1".to_string()));
            wheres
        })
        .await
        .unwrap();
        let list = db.show_table(table_name, None, None, 1, 100).await.unwrap();
        println!("ret:{:?}", list);
    }
}

const KEYWORDS: &'static [&'static str] = &[
    "ABSOLUTE",
    "EXEC",
    "OVERLAPS",
    "ACTION",
    "EXECUTE",
    "PAD",
    "ADA",
    "EXISTS",
    "PARTIAL",
    "ADD",
    "EXTERNAL",
    "PASCAL",
    "ALL",
    "EXTRACT",
    "POSITION",
    "ALLOCATE",
    "FALSE",
    "PRECISION",
    "ALTER",
    "FETCH",
    "PREPARE",
    "AND",
    "FIRST",
    "PRESERVE",
    "ANY",
    "FLOAT",
    "PRIMARY",
    "ARE",
    "FOR",
    "PRIOR",
    "AS",
    "FOREIGN",
    "PRIVILEGES",
    "ASC",
    "FORTRAN",
    "PROCEDURE",
    "ASSERTION",
    "FOUND",
    "PUBLIC",
    "AT",
    "FROM",
    "READ",
    "AUTHORIZATION",
    "FULL",
    "REAL",
    "AVG",
    "GET",
    "REFERENCES",
    "BEGIN",
    "GLOBAL",
    "RELATIVE",
    "BETWEEN",
    "GO",
    "RESTRICT",
    "BIT",
    "GOTO",
    "REVOKE",
    "BIT_LENGTH",
    "GRANT",
    "RIGHT",
    "BOTH",
    "GROUP",
    "ROLLBACK",
    "BY",
    "HAVING",
    "ROWS",
    "CASCADE",
    "HOUR",
    "SCHEMA",
    "CASCADED",
    "IDENTITY",
    "SCROLL",
    "CASE",
    "IMMEDIATE",
    "SECOND",
    "CAST",
    "IN",
    "SECTION",
    "CATALOG",
    "INCLUDE",
    "SELECT",
    "CHAR",
    "INDEX",
    "SESSION",
    "CHAR_LENGTH",
    "INDICATOR",
    "SESSION_USER",
    "CHARACTER",
    "INITIALLY",
    "SET",
    "CHARACTER_LENGTH",
    "INNER",
    "SIZE",
    "CHECK",
    "INPUT",
    "SMALLINT",
    "CLOSE",
    "INSENSITIVE",
    "SOME",
    "COALESCE",
    "INSERT",
    "SPACE",
    "COLLATE",
    "INT",
    "SQL",
    "COLLATION",
    "INTEGER",
    "SQLCA",
    "COLUMN",
    "INTERSECT",
    "SQLCODE",
    "COMMIT",
    "INTERVAL",
    "SQLERROR",
    "CONNECT",
    "INTO",
    "SQLSTATE",
    "CONNECTION",
    "IS",
    "SQLWARNING",
    "CONSTRAINT",
    "ISOLATION",
    "SUBSTRING",
    "CONSTRAINTS",
    "JOIN",
    "SUM",
    "CONTINUE",
    "KEY",
    "SYSTEM_USER",
    "CONVERT",
    "LANGUAGE",
    "TABLE",
    "CORRESPONDING",
    "LAST",
    "TEMPORARY",
    "COUNT",
    "LEADING",
    "THEN",
    "CREATE",
    "LEFT",
    "TIME",
    "CROSS",
    "LEVEL",
    "TIMESTAMP",
    "CURRENT",
    "LIKE",
    "TIMEZONE_HOUR",
    "CURRENT_DATE",
    "LOCAL",
    "TIMEZONE_MINUTE",
    "CURRENT_TIME",
    "LOWER",
    "TO",
    "CURRENT_TIMESTAMP",
    "MATCH",
    "TRAILING",
    "CURRENT_USER",
    "MAX",
    "TRANSACTION",
    "CURSOR",
    "MIN",
    "TRANSLATE",
    "DATE",
    "MINUTE",
    "TRANSLATION",
    "DAY",
    "MODULE",
    "TRIM",
    "DEALLOCATE",
    "MONTH",
    "TRUE",
    "DEC",
    "NAMES",
    "UNION",
    "DECIMAL",
    "NATIONAL",
    "UNIQUE",
    "DECLARE",
    "NATURAL",
    "UNKNOWN",
    "DEFAULT",
    "NCHAR",
    "UPDATE",
    "DEFERRABLE",
    "NEXT",
    "UPPER",
    "DEFERRED",
    "NO",
    "USAGE",
    "DELETE",
    "NONE",
    "USER",
    "DESC",
    "NOT",
    "USING",
    "DESCRIBE",
    "NULL",
    "VALUE",
    "DESCRIPTOR",
    "NULLIF",
    "VALUES",
    "DIAGNOSTICS",
    "NUMERIC",
    "VARCHAR",
    "DISCONNECT",
    "OCTET_LENGTH",
    "VARYING",
    "DISTINCT",
    "OF",
    "VIEW",
    "DOMAIN",
    "ON",
    "WHEN",
    "DOUBLE",
    "ONLY",
    "WHENEVER",
    "DROP",
    "OPEN",
    "WHERE",
    "ELSE",
    "OPTION",
    "WITH",
    "END",
    "OR",
    "WORK",
    "END-EXEC",
    "ORDER",
    "WRITE",
    "ESCAPE",
    "OUTER",
    "YEAR",
    "EXCEPT",
    "OUTPUT",
    "ZONE",
    "EXCEPTION",
];

const TYPES: &'static [&'static str] = &[
    "tinyint",
    "smallint",
    "int",
    "bigint",
    "float",
    "bit",
    "nvarchar",
    "varchar",
    "nchar",
    "char",
    "ntext",
    "text",
    "binary",
    "varbinary",
    "image",
    "uniqueidentifier",
    "numeric",
    "decimal",
    "xml",
    "datetime",
    "datetime2",
    "smalldatetime",
    "date",
    "time",
    "datetimeoffset",
];

/// todo I don't find the full list on internet
const BUILD_IN_FUNCTIONS: &'static [&'static str] = &[];
