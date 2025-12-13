#![allow(unused)]

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, OnceLock};
use std::{ffi::CString, sync::atomic::AtomicBool};

use anyhow::Error;
use gcp_bigquery_client::model::row;
use odbc_api::buffers::TextRowSet;
use odbc_api::{
    Connection, ConnectionOptions, Cursor, DataType, Environment, Nullability, Nullable,
    ResultSetMetadata,
    buffers::{AnySlice, BufferDesc, ColumnarAnyBuffer},
    sys::AttrConnectionPooling,
};
use tokio::task::spawn_blocking;

use crate::db::QuerySql;
use crate::db::{Field, QueryEventEmitter, QueryResp};
/// Maximum number of rows fetched with one row set. Fetching batches of rows is usually much
/// faster than fetching individual rows.
use std::sync::LazyLock;

static ODBC_ENV: LazyLock<Environment> = LazyLock::new(|| {
    // may don't need this?
    unsafe { Environment::set_connection_pooling(AttrConnectionPooling::DriverAware).unwrap() };
    let env = Environment::new().unwrap();
    env
});

#[cfg(test)]
fn odbc_next_row() -> Result<(), Error> {
    let now = std::time::Instant::now();

    // If you do not do anything fancy it is recommended to have only one Environment in the
    // entire process.
    let environment = Environment::new()?;

    // Connect using a DSN. Alternatively we could have used a connection string

    let connection_string = test_connect_str();

    let connection = ODBC_ENV
        .connect_with_connection_string(&connection_string, ConnectionOptions::default())?;
    println!("make connection{:?}", now.elapsed());
    // Execute a one of query without any parameters.
    match connection.execute("SELECT * FROM Persons", (), None)? {
        Some(mut cursor) => {
            for name in cursor.column_names().unwrap() {
                println!("!!!!!!!!!!!!!!:{:?}", name);
            }
            let tys: Vec<_> = (0..cursor.column_names().unwrap().count())
                .map(|index| cursor.col_data_type((index + 1) as u16).unwrap())
                .collect();
            println!("typs:{:?}", tys);

            while let Some(mut row) = cursor.next_row().unwrap() {
                for (col_index, ty) in tys.iter().enumerate() {
                    match ty {
                        DataType::Integer => {
                            let mut field = Nullable::<i32>::null();
                            row.get_data((col_index + 1) as u16, &mut field).unwrap();

                            println!("int32 value:{:?}", field.as_opt());
                        }
                        DataType::Unknown => todo!(),
                        DataType::Char { length } => {
                            let mut buf = vec![];

                            row.get_text((col_index + 1) as u16, &mut buf).unwrap();
                            println!("chat value:{:?}", String::from_utf8(buf));
                        }
                        DataType::WChar { length } => todo!(),
                        DataType::Numeric { precision, scale } => todo!(),
                        DataType::Decimal { precision, scale } => todo!(),
                        DataType::SmallInt => todo!(),
                        DataType::Float { precision } => todo!(),
                        DataType::Real => todo!(),
                        DataType::Double => {
                            let mut field = Nullable::<f64>::null();
                            row.get_data((col_index + 1) as u16, &mut field).unwrap();

                            println!("int32 value:{:?}", field.as_opt());
                        }
                        DataType::Varchar { length } => todo!(),
                        DataType::WVarchar { length } => todo!(),
                        DataType::LongVarchar { length } => todo!(),
                        DataType::WLongVarchar { length } => todo!(),
                        DataType::LongVarbinary { length } => todo!(),
                        DataType::Date => todo!(),
                        DataType::Time { precision } => todo!(),
                        DataType::Timestamp { precision } => todo!(),
                        DataType::BigInt => todo!(),
                        DataType::TinyInt => todo!(),
                        DataType::Bit => todo!(),
                        DataType::Varbinary { length } => todo!(),
                        DataType::Binary { length } => todo!(),
                        DataType::Other {
                            data_type,
                            column_size,
                            decimal_digits,
                        } => todo!(),
                    }
                }
            }
        }
        None => {
            eprintln!("Query came back empty. No output has been created.");
        }
    }
    println!("whole {:?}", now.elapsed());
    Ok(())
}
#[cfg(test)]
fn to_desc(types: &[(DataType, Nullability)]) -> Vec<BufferDesc> {
    types
        .iter()
        .map(|(ty, n)| match ty {
            DataType::Unknown => panic!("haha"),
            DataType::Char { length } => BufferDesc::Text { max_str_len: 4096 },
            DataType::WChar { length } => todo!(),
            DataType::Numeric { precision, scale } => todo!(),
            DataType::Decimal { precision, scale } => todo!(),
            DataType::Integer => BufferDesc::I32 {
                nullable: *n == Nullability::Nullable,
            },
            DataType::SmallInt => todo!(),
            DataType::Float { precision } => todo!(),
            DataType::Real => todo!(),
            DataType::Double => todo!(),
            DataType::Varchar { length } => todo!(),
            DataType::WVarchar { length } => todo!(),
            DataType::LongVarchar { length } => todo!(),
            DataType::WLongVarchar { length } => todo!(),
            DataType::LongVarbinary { length } => todo!(),
            DataType::Date => todo!(),
            DataType::Time { precision } => todo!(),
            DataType::Timestamp { precision } => todo!(),
            DataType::BigInt => todo!(),
            DataType::TinyInt => todo!(),
            DataType::Bit => todo!(),
            DataType::Varbinary { length } => todo!(),
            DataType::Binary { length } => todo!(),
            DataType::Other {
                data_type,
                column_size,
                decimal_digits,
            } => todo!(),
        })
        .collect()
}

#[cfg(test)]
fn any_buffer() -> Result<(), Error> {
    let now = std::time::Instant::now();
    // If you do not do anything fancy it is recommended to have only one Environment in the
    // entire process.

    let connection_string = test_connect_str();
    println!("conn str:{}", connection_string);

    let mut connection = ODBC_ENV
        .connect_with_connection_string(&connection_string, ConnectionOptions::default())?;
    println!("make connection{:?}", now.elapsed());
    // Execute a one of query without any parameters.
    match connection.execute("SELECT id,name FROM Persons", (), None)? {
        Some(mut cursor) => {
            println!("execute:{:?}", cursor);
            // Write the column names to stdout
            let mut headline: Vec<String> = cursor.column_names()?.collect::<Result<_, _>>()?;

            // Use schema in cursor to initialize a text buffer large enough to hold the largest
            // possible strings for each column up to an upper limit of 4KiB

            let mut buffers = ColumnarAnyBuffer::from_descs(
                100,
                to_desc(
                    &(0..cursor.column_names().unwrap().count())
                        .map(|index| {
                            (
                                cursor.col_data_type((index + 1) as u16).unwrap(),
                                cursor.col_nullability((index + 1) as u16).unwrap(),
                            )
                        })
                        .collect::<Vec<_>>()[..],
                ),
            );
            // Bind the buffer to the cursor. It is now being filled with every call to fetch.
            let mut row_set_cursor = cursor.bind_buffer(&mut buffers)?;

            // Iterate over batches
            while let Some(batch) = row_set_cursor.fetch()? {
                // Within a batch, iterate over every row
                for row_index in 0..batch.num_rows() {
                    for col_index in 0..batch.num_cols() {
                        match batch.column(col_index) {
                            AnySlice::Text(text_column_view) => {
                                let c = text_column_view.get(row_index).unwrap();
                                let s = unsafe { CString::from_vec_unchecked(c.to_vec()) };
                                println!("{:?}", s);
                            }
                            AnySlice::WText(text_column_view) => todo!(),
                            AnySlice::Binary(bin_column_view) => todo!(),
                            AnySlice::Date(dates) => todo!(),
                            AnySlice::Time(times) => todo!(),
                            AnySlice::Timestamp(timestamps) => todo!(),
                            AnySlice::F64(items) => todo!(),
                            AnySlice::F32(items) => todo!(),
                            AnySlice::I8(items) => todo!(),
                            AnySlice::I16(items) => todo!(),
                            AnySlice::I32(items) => todo!(),
                            AnySlice::I64(items) => todo!(),
                            AnySlice::U8(items) => todo!(),
                            AnySlice::Bit(bits) => todo!(),
                            AnySlice::Numeric(numerics) => todo!(),
                            AnySlice::NullableDate(nullable_slice) => todo!(),
                            AnySlice::NullableTime(nullable_slice) => todo!(),
                            AnySlice::NullableTimestamp(nullable_slice) => todo!(),
                            AnySlice::NullableF64(nullable_slice) => todo!(),
                            AnySlice::NullableF32(nullable_slice) => todo!(),
                            AnySlice::NullableI8(nullable_slice) => todo!(),
                            AnySlice::NullableI16(nullable_slice) => todo!(),
                            AnySlice::NullableI32(nullable_slice) => {
                                println!("!!!:{:?}", nullable_slice.raw_values());
                            }
                            AnySlice::NullableI64(nullable_slice) => todo!(),
                            AnySlice::NullableU8(nullable_slice) => todo!(),
                            AnySlice::NullableBit(nullable_slice) => todo!(),
                            AnySlice::NullableNumeric(nullable_slice) => todo!(),
                        }
                    }
                }
            }
        }
        None => {
            eprintln!("Query came back empty. No output has been created.");
        }
    }
    println!("whole {:?}", now.elapsed());
    Ok(())
}

#[cfg(test)]
fn test_connect_str() -> String {
    format!(
        r#"Driver={{SQLite3}};Database={}"#,
        crate::sqlite::sqlite_test_file_path()
    )
}

#[derive(Clone)]
pub struct OdbcDb {
    conn: Arc<Mutex<Connection<'static>>>,

    // 也就是说 不管是commit还是rollback  不会影响提交模式对吧
    // 对，完全正确。
    // SQLEndTran(SQL_COMMIT) 或 SQLRollback() 只负责“把当前事务收尾”，绝不碰 SQL_ATTR_AUTOCOMMIT 这个开关。
    // so we save this to memory
    commit_mode: Arc<AtomicBool>,
}

impl OdbcDb {
    async fn get_auto_commit(&self) -> anyhow::Result<bool> {
        anyhow::Ok(self.commit_mode.load(Ordering::Relaxed))
    }
    async fn set_auto_commit(&self, _commit: bool) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.conn.lock().expect("lock failed");
            conn.set_autocommit(_commit)?;
            anyhow::Ok(())
        })
        .await?
    }
    async fn commit(&self, _query_id: &str) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.conn.lock().expect("lock failed");
            conn.commit()?;
            anyhow::Ok(())
        })
        .await?
    }
    async fn rollback(&self, _query_id: &str) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        spawn_blocking(move || {
            let conn = this.conn.lock().expect("lock failed");
            conn.rollback()?;
            anyhow::Ok(())
        })
        .await?
    }

    async fn query_sql(&self, _sql: &str, emit: QueryEventEmitter) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let _sql = _sql.to_string();
        spawn_blocking(move || {
            let conn = this.conn.lock().expect("lock failed");
            let cursor = conn.execute(&_sql, (), None)?;
            let mut cursor = match cursor {
                Some(x) => x,
                None => return anyhow::Ok(()),
            };
            let mut fs = vec![];
            for index in 0..cursor.column_names()?.count() {
                let name = cursor.col_name((index + 1) as u16)?;
                let ty = cursor.col_data_type((index + 1) as u16)?;
                fs.push(Field::from_name_type(name, format!("{:?}", ty)));
            }
            emit.send(
                QueryResp {
                    fields: fs,
                    ..Default::default()
                }
                .into(),
            )?;

            // TODO config max str len
            let mut buffers = TextRowSet::for_cursor(100, &mut cursor, Some(4096))?;
            let mut row_set_cursor = cursor.bind_buffer(&mut buffers)?;
            while let Some(batch) = row_set_cursor.fetch()? {
                for row_index in 0..batch.num_rows() {
                    let mut data = vec![];
                    for col_index in 0..batch.num_cols() {
                        let r = batch.at_as_str(col_index, row_index)?;
                        data.push(r.map(|x| x.to_string()));
                    }
                    emit.send(data.into())?;
                }
            }
            anyhow::Ok(())
        })
        .await?
    }

    async fn execute_sql(&self, _sql: &str) -> anyhow::Result<Result<QueryResp, String>> {
        let this = Clone::clone(self);
        let _sql = _sql.to_string();
        spawn_blocking(move || {
            let conn = this.conn.lock().expect("lock failed");
            conn.execute(&_sql, (), None)?;
            anyhow::Ok(Ok(QueryResp::execute_ok(None)))
        })
        .await?
    }
}

mod tests {
    use super::*;

    #[test]

    fn xxx() {
        println!("odbc");
        odbc_next_row();
        any_buffer();
    }
}
