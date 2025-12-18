#![allow(unused)]

use std::num::NonZero;
use std::string;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};
use std::{ffi::CString, sync::atomic::AtomicBool};

use anyhow::Error;
use gcp_bigquery_client::model::row;
use mongodb::bson::de;
use odbc_api::Bit;
use odbc_api::buffers::TextRowSet;
use odbc_api::sys::{Date, Numeric, Time, Timestamp};
use odbc_api::{
    Connection, ConnectionOptions, Cursor, DataType, Environment, Nullability, Nullable,
    ResultSetMetadata,
    buffers::{AnySlice, BufferDesc, ColumnarAnyBuffer},
    sys::AttrConnectionPooling,
};
use tokio::task::spawn_blocking;

use crate::db::{Field, QueryEventEmitter, QueryResp};
use crate::db::{QuerySql, TimeElapsed};

/// Maximum number of rows fetched with one row set. Fetching batches of rows is usually much
/// faster than fetching individual rows.
use std::sync::LazyLock;

pub(crate) static ODBC_ENV: LazyLock<Environment> = LazyLock::new(|| {
    let env = Environment::new().unwrap();
    env
});

pub(crate) fn make_conn(conn_str: &str) -> Result<Connection<'static>, odbc_api::Error> {
    ODBC_ENV.connect_with_connection_string(
        &conn_str,
        ConnectionOptions {
            login_timeout_sec: Some(10),
            packet_size: None,
        },
    )
}

#[derive(Clone, Debug)]
pub struct OdbcPool {
    pub(crate) conn_str: String,
    pub(crate) select_1_sql: String,
}

impl r2d2::ManageConnection for OdbcPool {
    type Connection = Connection<'static>;

    type Error = odbc_api::Error;

    fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let conn = ODBC_ENV
            .connect_with_connection_string(&self.conn_str, ConnectionOptions::default())?;
        Ok(conn)
    }

    fn is_valid(&self, conn: &mut Self::Connection) -> Result<(), Self::Error> {
        conn.execute(&self.select_1_sql, (), None)?;
        Ok(())
    }

    fn has_broken(&self, conn: &mut Self::Connection) -> bool {
        false
    }
}

impl OdbcDbQuery {
    pub fn new(conn: Connection<'static>, helper: Arc<dyn OdbcHelper>) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
            commit_mode: Arc::new(AtomicBool::new(true)),
            helper,
        }
    }
}

#[derive(Clone)]
pub struct OdbcDbQuery {
    conn: Arc<Mutex<Connection<'static>>>,

    // 也就是说 不管是commit还是rollback  不会影响提交模式对吧
    // 对，完全正确。
    // SQLEndTran(SQL_COMMIT) 或 SQLRollback() 只负责“把当前事务收尾”，绝不碰 SQL_ATTR_AUTOCOMMIT 这个开关。
    // so we save this to memory
    commit_mode: Arc<AtomicBool>,
    pub helper: Arc<dyn OdbcHelper>,
}

pub trait OdbcHelper: Sync + Send + 'static {
    fn format_date(&self, x: &Date) -> String {
        format!("{}-{}-{}", x.year, x.month, x.day)
    }
    fn format_time(&self, x: &Time) -> String {
        format!("{}:{}:{}", x.hour, x.minute, x.second)
    }
    fn get_max_str_len(&self) -> usize;

    fn get_max_bin_len(&self) -> usize;

    fn format_timestamp(&self, x: &Timestamp) -> String;
    fn format_bit(&self, x: &Bit) -> String;
    fn format_binary(&self, x: &[u8]) -> String;
    fn format_numeric(&self, x: &Numeric) -> String;
}

impl dyn OdbcHelper {
    fn encode_batch(&self, batch: &ColumnarAnyBuffer) -> anyhow::Result<Vec<Vec<Option<String>>>> {
        let mut result = vec![];
        for row_index in 0..batch.num_rows() {
            let mut values: Vec<Option<String>> = Vec::with_capacity(batch.num_cols());
            for col_index in 0..batch.num_cols() {
                let v = match batch.column(col_index) {
                    AnySlice::Text(text_column_view) => {
                        let value = text_column_view.get(row_index);
                        match value {
                            Some(bs) => Some(String::from_utf8_lossy(bs).to_string()),
                            None => None,
                        }
                    }
                    AnySlice::WText(text_column_view) => {
                        let value = text_column_view.get(row_index);
                        match value {
                            Some(x) => match String::from_utf16(x) {
                                Ok(x) => Some(x),
                                Err(err) => {
                                    anyhow::bail!("not a valid utf-16 string,err:{}", err)
                                }
                            },
                            None => None,
                        }
                    }
                    AnySlice::Binary(bin_column_view) => {
                        let value = bin_column_view.get(row_index);
                        value.map(|x| self.format_binary(x))
                    }
                    AnySlice::Date(dates) => {
                        let r = self.format_date(&dates[row_index]);
                        Some(r)
                    }
                    AnySlice::Time(times) => {
                        let r = self.format_time(&times[row_index]);
                        Some(r)
                    }
                    AnySlice::Timestamp(timestamps) => {
                        let r = self.format_timestamp(&timestamps[row_index]);
                        Some(r)
                    }
                    AnySlice::F64(items) => Some(items[row_index].to_string()),
                    AnySlice::F32(items) => Some(items[row_index].to_string()),
                    AnySlice::I8(items) => Some(items[row_index].to_string()),
                    AnySlice::I16(items) => Some(items[row_index].to_string()),
                    AnySlice::I32(items) => Some(items[row_index].to_string()),
                    AnySlice::I64(items) => Some(items[row_index].to_string()),
                    AnySlice::U8(items) => Some(items[row_index].to_string()),
                    AnySlice::Bit(bits) => Some(self.format_bit(&bits[row_index])),
                    AnySlice::Numeric(numerics) => Some(self.format_numeric(&numerics[row_index])),
                    AnySlice::NullableDate(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        v.map(|x| self.format_date(x))
                    }
                    AnySlice::NullableTime(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        v.map(|x| self.format_time(x))
                    }
                    AnySlice::NullableTimestamp(nullable_slice) => {
                        let r = nullable_slice.get(row_index);
                        r.map(|x| self.format_timestamp(x))
                    }
                    AnySlice::NullableF64(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        v.map(|x| x.to_string())
                    }
                    AnySlice::NullableF32(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        v.map(|x| x.to_string())
                    }
                    AnySlice::NullableI8(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        v.map(|x| x.to_string())
                    }
                    AnySlice::NullableI16(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        v.map(|x| x.to_string())
                    }
                    AnySlice::NullableI32(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        v.map(|x| x.to_string())
                    }
                    AnySlice::NullableI64(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        v.map(|x| x.to_string())
                    }
                    AnySlice::NullableU8(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        v.map(|x| x.to_string())
                    }
                    AnySlice::NullableBit(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        v.map(|x| self.format_bit(&x))
                    }
                    AnySlice::NullableNumeric(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        v.map(|x| self.format_numeric(&x))
                    }
                };
                values.push(v);
            }
            result.push(values);
        }
        anyhow::Ok(result)
    }
}

impl OdbcDbQuery {
    fn fix_length(&self, tys: &mut [DataType]) {
        for ty in tys.iter_mut() {
            match ty {
                DataType::Unknown => {}
                DataType::Char { length }
                | DataType::WChar { length }
                | DataType::Varchar { length }
                | DataType::WVarchar { length }
                | DataType::LongVarchar { length }
                | DataType::WLongVarchar { length } => match length {
                    Some(_) => {}
                    None => *length = NonZero::new(self.helper.get_max_str_len()),
                },
                DataType::Numeric { .. } => {}
                DataType::Decimal { .. } => {}
                DataType::Integer => {}
                DataType::SmallInt => {}
                DataType::Float { .. } => {}
                DataType::Real => {}
                DataType::Double => {}

                DataType::LongVarbinary { length }
                | DataType::Varbinary { length }
                | DataType::Binary { length } => match length {
                    Some(_) => {}
                    None => *length = NonZero::new(self.helper.get_max_bin_len()),
                },
                DataType::Date => {}
                DataType::Time { .. } => {}
                DataType::Timestamp { precision } => {}
                DataType::BigInt => {}
                DataType::TinyInt => {}
                DataType::Bit => {}
                DataType::Other { .. } => {}
            }
        }
    }
}

impl OdbcDbQuery {
    pub(crate) fn get_auto_commit(&self) -> bool {
        self.commit_mode.load(Ordering::Relaxed)
    }

    pub(crate) async fn set_auto_commit(&self, commit: bool) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.conn.lock().expect("lock failed");
            conn.set_autocommit(commit)?;
            anyhow::Ok(())
        })
        .await?
    }

    pub(crate) async fn commit(&self) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.conn.lock().expect("lock failed");
            conn.commit()?;
            anyhow::Ok(())
        })
        .await?
    }
    pub(crate) async fn rollback(&self) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.conn.lock().expect("lock failed");
            conn.rollback()?;
            anyhow::Ok(())
        })
        .await?
    }

    pub(crate) async fn query_sql(&self, sql: &str, emit: QueryEventEmitter) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let sql = sql.to_string();
        spawn_blocking(move || {
            let conn = this.conn.lock().expect("lock failed");
            let cursor = conn.execute(&sql, (), None)?;
            let mut cursor = match cursor {
                Some(x) => x,
                None => return anyhow::Ok(()),
            };
            let mut fs = vec![];
            let mut tys = vec![];
            let mut nulls = vec![];
            for index in 0..cursor.column_names()?.count() {
                let name = cursor.col_name((index + 1) as u16)?;
                let ty = cursor.col_data_type((index + 1) as u16)?;
                fs.push(Field::from_name_type(name, format!("{:?}", ty)));
                tys.push(ty);
                nulls.push(cursor.col_nullability((index + 1) as u16)?);
            }
            emit.send(
                QueryResp {
                    fields: fs,
                    ..Default::default()
                }
                .into(),
            )?;
            this.fix_length(&mut tys);
            let mut buffer_desc = vec![];
            for (ty, null_able) in tys.into_iter().zip(nulls.iter()) {
                let d = BufferDesc::from_data_type(
                    ty.clone(),
                    match null_able {
                        Nullability::Unknown => true,
                        Nullability::Nullable => true,
                        Nullability::NoNulls => false,
                    },
                );
                buffer_desc.push(match d {
                    Some(x) => x,
                    None => {
                        anyhow::bail!(format!("should be able to convert {:?} to buffer desc", ty))
                    }
                });
            }
            //
            let mut buffers = ColumnarAnyBuffer::from_descs(100, buffer_desc);
            let mut row_set_cursor = cursor.bind_buffer(&mut buffers)?;
            while let Some(batch) = row_set_cursor.fetch()? {
                let data = this.helper.encode_batch(*batch)?;
                emit.send(data.into())?;
            }
            anyhow::Ok(())
        })
        .await?
    }

    pub(crate) async fn execute_sql(&self, sql: &str) -> anyhow::Result<Result<QueryResp, String>> {
        let this = Clone::clone(self);
        let sql = sql.to_string();
        spawn_blocking(move || {
            let conn = this.conn.lock().expect("lock failed");
            conn.execute(&sql, (), None)?;
            anyhow::Ok(Ok(QueryResp::execute_ok(None)))
        })
        .await?
    }
}

// https://public.dhe.ibm.com/ibmdl/export/pub/software/data/db2/drivers/odbc_cli/
fn test_db2() -> anyhow::Result<()> {
    let conn_str = "Driver={Driver_IBM_DB2};Database=TSKDB;Hostname=127.0.0.1;Port=50000;Protocol=TCPIP;UID=db2inst1;PWD=db2inst1-pwd;";
    let conn = ODBC_ENV
        .connect_with_connection_string(conn_str, ConnectionOptions::default())
        .unwrap();
    let mut cursor = conn
        .execute("SELECT 1 FROM SYSIBM.SYSDUMMY1", (), None)
        .unwrap()
        .unwrap();
    let mut buffer = TextRowSet::for_cursor(100, &mut cursor, Some(4096)).unwrap();
    let mut row_mut_set = cursor.bind_buffer(&mut buffer).unwrap();

    while let Some(batch) = row_mut_set.fetch().unwrap() {
        for row_index in 0..batch.num_rows() {
            for col_index in 0..batch.num_cols() {
                println!("col_index:{} , row_index:{}", col_index, row_index);
                let r = batch.at(col_index, row_index).unwrap();
                dbg!(r);
            }
        }
    }
    anyhow::Ok(())
}

mod tests {
    use super::*;

    #[test]

    fn ddddd() {
        test_db2();
    }
}
