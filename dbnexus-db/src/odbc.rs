#![allow(unused)]

use std::num::NonZero;

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use crate::db::{
    create_html_table_data, create_html_table_header, create_markdown_data, create_markdown_header,
    ExportFrame, ExportKind, Field, QueryEventEmitter, QueryResp, XlsxEdit,
};

use crate::helper::create_csv;

use odbc_api::handles::StatementImpl;
use odbc_api::sys::{Date, Numeric, Time, Timestamp};
use odbc_api::{
    buffers::{AnySlice, BufferDesc, ColumnarAnyBuffer},
    Connection, ConnectionOptions, Cursor, DataType, Environment, Nullability, ResultSetMetadata,
};
use odbc_api::{Bit, CursorImpl};
/// Maximum number of rows fetched with one row set. Fetching batches of rows is usually much
/// faster than fetching individual rows.
use std::sync::LazyLock;
use tokio::task::spawn_blocking;

pub(crate) static ODBC_ENV: LazyLock<Environment> =
    LazyLock::new(|| Environment::new().expect("should be able to create an environment"));

pub(crate) fn make_conn(conn_str: &str) -> Result<Connection<'static>, odbc_api::Error> {
    ODBC_ENV.connect_with_connection_string(
        conn_str,
        ConnectionOptions {
            login_timeout_sec: Some(10),
            packet_size: None,
        },
    )
}

#[derive(Clone, Debug)]
pub struct OdbcPool {
    pub(crate) conn_str: String,
    pub(crate) select_1_sql: &'static str,
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
        conn.execute(self.select_1_sql, (), None)?;
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

    // So, neither commit nor rollback changes the autocommit mode, right?
    // Correct.
    // SQLEndTran(SQL_COMMIT) or SQLRollback() only ends the current transaction; neither changes the SQL_ATTR_AUTOCOMMIT setting.
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
    fn max_str_len(&self) -> usize;

    fn max_bin_len(&self) -> usize;

    fn format_timestamp(&self, x: &Timestamp) -> String;
    fn format_bit(&self, x: &Bit) -> String;
    fn format_binary(&self, x: &[u8]) -> String;
    fn format_numeric(&self, x: &Numeric) -> String;
    fn get_str(&self, _: &[u8]) -> anyhow::Result<String>;
}

impl dyn OdbcHelper {
    fn encode_batch_as_json(
        &self,
        batch: &ColumnarAnyBuffer,
        col_names: &[String],
    ) -> anyhow::Result<Vec<serde_json::Value>> {
        let mut ret = vec![];
        for row_index in 0..batch.num_rows() {
            let mut m = serde_json::Map::new();
            for (col_index, name) in col_names.iter().enumerate() {
                let v: serde_json::Value = match batch.column(col_index) {
                    AnySlice::Text(text_column_view) => match text_column_view.get(row_index) {
                        Some(x) => serde_json::Value::from(self.get_str(x)?),
                        None => serde_json::Value::Null,
                    },
                    AnySlice::WText(text_column_view) => match text_column_view.get(row_index) {
                        Some(x) => serde_json::Value::from(String::from_utf16_lossy(x)),
                        None => serde_json::Value::Null,
                    },
                    AnySlice::Binary(bin_column_view) => {
                        let v = bin_column_view.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::Array(
                                x.iter().map(|x| serde_json::Value::from(*x)).collect(),
                            ),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::Date(dates) => {
                        let v = dates.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(self.format_date(x)),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::Time(times) => {
                        let v = times.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(self.format_time(x)),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::Timestamp(timestamps) => {
                        let v = timestamps.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(self.format_timestamp(x)),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::F64(items) => {
                        let v = items.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(*x),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::F32(items) => {
                        let v = items.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(*x),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::I8(items) => {
                        let v = items.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(*x),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::I16(items) => {
                        let v = items.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(*x),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::I32(items) => {
                        let v = items.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(*x),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::I64(items) => {
                        let v = items.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(*x),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::U8(items) => {
                        let v = items.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(*x),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::Bit(bits) => {
                        let v = bits.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(self.format_bit(x)),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::Numeric(numerics) => {
                        let v = numerics.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(self.format_numeric(x)),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::NullableDate(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(self.format_date(x)),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::NullableTime(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(self.format_time(x)),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::NullableTimestamp(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(self.format_timestamp(x)),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::NullableF64(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        match v {
                            Some(v) => serde_json::Value::from(*v),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::NullableF32(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        match v {
                            Some(v) => serde_json::Value::from(*v),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::NullableI8(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        match v {
                            Some(v) => serde_json::Value::from(*v),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::NullableI16(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        match v {
                            Some(v) => serde_json::Value::from(*v),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::NullableI32(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        match v {
                            Some(v) => serde_json::Value::from(*v),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::NullableI64(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        match v {
                            Some(v) => serde_json::Value::from(*v),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::NullableU8(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        match v {
                            Some(v) => serde_json::Value::from(*v),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::NullableBit(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(self.format_bit(x)),
                            None => serde_json::Value::Null,
                        }
                    }
                    AnySlice::NullableNumeric(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        match v {
                            Some(x) => serde_json::Value::from(self.format_numeric(x)),
                            None => serde_json::Value::Null,
                        }
                    }
                };
                m.insert(name.clone(), v);
            }
            ret.push(serde_json::Value::Object(m));
        }
        anyhow::Ok(ret)
    }
    fn encode_batch(&self, batch: &ColumnarAnyBuffer) -> anyhow::Result<Vec<Vec<Option<String>>>> {
        let mut result = vec![];
        for row_index in 0..batch.num_rows() {
            let mut values: Vec<Option<String>> = Vec::with_capacity(batch.num_cols());
            for col_index in 0..batch.num_cols() {
                let v = match batch.column(col_index) {
                    AnySlice::Text(text_column_view) => {
                        let value = text_column_view.get(row_index);
                        match value {
                            Some(bs) => Some(self.get_str(bs)?),
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
                        v.map(|x| self.format_bit(x))
                    }
                    AnySlice::NullableNumeric(nullable_slice) => {
                        let v = nullable_slice.get(row_index);
                        v.map(|x| self.format_numeric(x))
                    }
                };
                values.push(v);
            }
            result.push(values);
        }
        anyhow::Ok(result)
    }
}

impl dyn OdbcHelper {
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
                    None => *length = NonZero::new(self.max_str_len()),
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
                    None => *length = NonZero::new(self.max_bin_len()),
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

            let OdbcColumnMeta {
                col_names,
                col_types,
                col_nulls: _,
                buffer_desc,
            } = this.helper.collect_meta(&mut cursor)?;
            emit.send(
                QueryResp {
                    fields: col_names
                        .iter()
                        .zip(col_types.iter())
                        .map(|(name, ty)| Field::from_name_type(name, format!("{:?}", ty)))
                        .collect(),
                    ..Default::default()
                }
                .into(),
            )?;
            let mut buffers = ColumnarAnyBuffer::from_descs(100, buffer_desc);
            let mut row_set_cursor = cursor.bind_buffer(&mut buffers)?;
            while let Some(batch) = row_set_cursor.fetch()? {
                let data = this.helper.encode_batch(batch)?;
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

pub struct OdbcColumnMeta {
    col_names: Vec<String>,
    col_types: Vec<DataType>,
    col_nulls: Vec<Nullability>,
    buffer_desc: Vec<BufferDesc>,
}

impl dyn OdbcHelper {
    fn collect_meta(
        &self,
        cursor: &mut CursorImpl<StatementImpl<'_>>,
    ) -> anyhow::Result<OdbcColumnMeta> {
        let mut name = vec![];
        let mut ty = vec![];
        let mut null = vec![];
        let mut desc = vec![];
        for index in 0..cursor.column_names()?.count() {
            let index = (index + 1) as u16;
            name.push(cursor.col_name(index)?);
            ty.push(cursor.col_data_type(index)?);
            null.push(cursor.col_nullability(index)?);
        }
        self.fix_length(&mut ty);
        for (t, null) in ty.iter().zip(null.iter()) {
            let d = BufferDesc::from_data_type(
                *t,
                match null {
                    Nullability::Unknown => true,
                    Nullability::Nullable => true,
                    Nullability::NoNulls => false,
                },
            );
            let d = match d {
                Some(d) => d,
                None => anyhow::bail!(format!("desc is null,from:{:?}", t)),
            };
            desc.push(d);
        }
        anyhow::Ok(OdbcColumnMeta {
            col_names: name,
            col_types: ty,
            col_nulls: null,
            buffer_desc: desc,
        })
    }
}

impl OdbcDbQuery {
    async fn export_query_result(
        &self,
        sql: &str,
        kind: ExportKind,
        sender: tokio::sync::mpsc::Sender<ExportFrame>,
    ) -> anyhow::Result<()> {
        let this = Clone::clone(self);
        let sql = sql.to_string();
        spawn_blocking(move || {
            let conn = this.conn.lock().expect("lock failed");
            let cursor = conn.execute(&sql, (), None)?;
            let mut cursor: CursorImpl<StatementImpl<'_>> = match cursor {
                Some(x) => x,
                None => return anyhow::Ok(()),
            };
            let OdbcColumnMeta {
                col_names,
                buffer_desc,
                ..
            } = this.helper.collect_meta(&mut cursor)?;
            let mut buffers = ColumnarAnyBuffer::from_descs(100, buffer_desc);
            let mut row_set_cursor = cursor.bind_buffer(&mut buffers)?;
            let mut first = FirstTrue::default();

            while let Some(batch) = row_set_cursor.fetch()? {
                match kind {
                    ExportKind::Csv {
                        include_header,
                        delimiter,
                    } => {
                        let r = this.helper.encode_batch(batch)?;
                        for x in r {
                            let e = create_csv(&x, delimiter)?;
                            sender.blocking_send(e.into())?;
                        }
                    }
                    ExportKind::Json { .. } | ExportKind::JsonLine => {
                        let rs = this.helper.encode_batch_as_json(batch, &col_names)?;
                        for v in rs {
                            sender.blocking_send(v.into())?;
                        }
                    }
                    ExportKind::Sql { .. } => {
                        return ExportKind::cannot_query_result_export_as_sql();
                    }
                    ExportKind::Xlsx { include_header } => {
                        let mut num_row = 0;
                        if first && include_header {
                            for (index, name) in col_names.iter().enumerate() {
                                sender.blocking_send(
                                    XlsxEdit {
                                        row: num_row,
                                        column: index,
                                        content: name.clone(),
                                    }
                                    .into(),
                                )?;
                            }
                            num_row += 1;
                        }
                        let r = this.helper.encode_batch(batch)?;
                        for x in r {
                            for (index, col) in x.into_iter().enumerate() {
                                if let Some(col) = col {
                                    sender.blocking_send(
                                        XlsxEdit {
                                            row: num_row,
                                            column: index,
                                            content: col,
                                        }
                                        .into(),
                                    )?;
                                }
                            }
                            num_row += 1;
                        }
                    }
                    ExportKind::Html { include_header } => {
                        if first && include_header {
                            let r = create_html_table_header(&col_names);
                            sender.blocking_send(r.into())?;
                        }
                        let r = this.helper.encode_batch(batch)?;
                        sender.blocking_send(create_html_table_data(&r).into())?;
                    }
                    ExportKind::MarkDown => {
                        if first {
                            let r = create_markdown_header(&col_names);
                            sender.blocking_send(r.into())?;
                        }
                        let r = this.helper.encode_batch(batch)?;
                        for x in r {
                            let r = create_markdown_data(&x);
                            sender.blocking_send(r.into())?;
                        }
                    }
                }
            }
            anyhow::Ok(())
        })
        .await?
    }
}

pub fn odbc_query<T: FromSql>(
    conn: &Connection<'static>,
    sql: &str,
    helper: Arc<dyn OdbcHelper>,
) -> anyhow::Result<Vec<T>> {
    let cursor = conn.execute(sql, (), None)?;
    let mut cursor = match cursor {
        Some(x) => x,
        None => return anyhow::Ok(vec![]),
    };
    let meta = helper.collect_meta(&mut cursor)?;
    let mut buffers = ColumnarAnyBuffer::from_descs(100, meta.buffer_desc);
    let mut row_set_cursor = cursor.bind_buffer(&mut buffers)?;
    let mut ret = vec![];
    while let Some(batch) = row_set_cursor.fetch()? {
        // println!("now row:{} {:?}", batch.num_rows(), *batch.indicator_at());

        for row_index in 0..batch.num_rows() {
            let mut t = T::default();
            t.use_sql(batch, row_index, helper.clone())?;
            ret.push(t);
        }
    }
    anyhow::Ok(ret)
}

pub trait FromSql: Default {
    fn use_sql(
        &mut self,
        buffer: &ColumnarAnyBuffer,
        row_index: usize,
        helper: Arc<dyn OdbcHelper>,
    ) -> anyhow::Result<()>;
}

impl<T: FromSqlElement> FromSql for (T) {
    fn use_sql(
        &mut self,
        buffer: &ColumnarAnyBuffer,
        row_index: usize,
        helper: Arc<dyn OdbcHelper>,
    ) -> anyhow::Result<()> {
        self.use_sql_col(buffer.column(0), row_index, helper)?;
        anyhow::Ok(())
    }
}

impl<T1: FromSqlElement, T2: FromSqlElement> FromSql for (T1, T2) {
    fn use_sql(
        &mut self,
        buffer: &ColumnarAnyBuffer,
        row_index: usize,
        helper: Arc<dyn OdbcHelper>,
    ) -> anyhow::Result<()> {
        self.0
            .use_sql_col(buffer.column(0), row_index, helper.clone())?;
        self.1
            .use_sql_col(buffer.column(1), row_index, helper.clone())?;
        anyhow::Ok(())
    }
}

pub trait FromSqlElement: Default {
    fn use_sql_col(
        &mut self,
        buffer: AnySlice<'_>,
        row_index: usize,
        helper: Arc<dyn OdbcHelper>,
    ) -> anyhow::Result<()>;
}

impl FromSqlElement for String {
    fn use_sql_col(
        &mut self,
        buffer: AnySlice<'_>,
        row_index: usize,
        helper: Arc<dyn OdbcHelper>,
    ) -> anyhow::Result<()> {
        match buffer {
            AnySlice::Text(text_column_view) => {
                let r = match text_column_view.get(row_index) {
                    Some(x) => x,
                    None => anyhow::bail!("text is null using Option<String>"),
                };
                let s = helper.get_str(r)?;
                *self = s;
            }
            _ => anyhow::bail!("this is not text buffer."),
        }
        anyhow::Ok(())
    }
}

impl FromSqlElement for Option<String> {
    fn use_sql_col(
        &mut self,
        buffer: AnySlice<'_>,
        row_index: usize,
        helper: Arc<dyn OdbcHelper>,
    ) -> anyhow::Result<()> {
        match buffer {
            AnySlice::Text(text_column_view) => {
                let r = match text_column_view.get(row_index) {
                    Some(x) => x,
                    None => {
                        println!("row index is none:{}", row_index);
                        // nothing here option data default to None
                        return anyhow::Ok(());
                    }
                };
                let s = helper.get_str(r)?;
                *self = Some(s);
            }
            _ => {
                anyhow::bail!("this is not a text buffer.")
            }
        }
        anyhow::Ok(())
    }
}

#[cfg(test)]
mod tests {}
