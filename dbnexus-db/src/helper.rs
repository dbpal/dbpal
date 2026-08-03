use sqlparser::ast::{ArrayElemTypeDef, DataType};

use crate::db::{BackendKind, Field};

/// create csv file list of string
/// each line of line separator
pub(crate) fn create_csv(
    ds: &[Option<String>],
    delimiter: char,
) -> anyhow::Result<tokio_util::bytes::Bytes> {
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter as u8)
        .from_writer(Vec::new());

    writer.write_record(ds.iter().map(|x| match x {
        Some(x) => x.as_bytes(),
        None => &[],
    }))?;
    writer.flush()?;
    let r = writer.into_inner()?;
    let r = tokio_util::bytes::Bytes::from(r);
    anyhow::Ok(r)
}

/// get current timestamp
pub fn current_timestamp() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();
    let timestamp: u64 = now
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();
    timestamp
}

pub struct OffsetLimitIter<Index = usize> {
    pub(crate) total: Index,
    pub(crate) offset: Index,
    pub(crate) size: Index,
}

impl<T: Default> OffsetLimitIter<T> {
    pub(crate) fn new(total: T, size: T) -> Self {
        Self {
            offset: Default::default(),
            total,
            size,
        }
    }
}

impl<T: std::ops::Add + std::ops::AddAssign + Copy + std::cmp::Ord> Iterator
    for OffsetLimitIter<T>
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset < self.total {
            let offset = self.offset;
            self.offset += self.size;
            Some(offset)
        } else {
            None
        }
    }
}

#[allow(unused)]
pub struct TimeSome {
    some: String,
    start: std::time::Instant,
}

impl Drop for TimeSome {
    fn drop(&mut self) {
        println!(
            "start:{:?} elapsed:{}ms end:{}",
            self.start,
            self.start.elapsed().as_millis(),
            self.some
        );
    }
}

impl TimeSome {
    pub fn new(some: String) -> Self {
        let start = std::time::Instant::now();
        println!("start:{} time:{:?}", some, start);
        Self { some, start }
    }
}

#[inline]
pub(crate) fn remove_first_char_string(s: String) -> String {
    s.chars().skip(1).collect()
}

// try remove a @ used for mysql etc database
pub(crate) fn try_remove_one_at(value: String) -> String {
    if value.starts_with('@') && !value.starts_with("@@") {
        remove_first_char_string(value)
    } else {
        value
    }
}

// check if ty is plain integer
pub(crate) fn ty_is_int(ty: &DataType) -> bool {
    match ty {
        DataType::Table(_) => false,
        DataType::NamedTable { .. } => false,
        DataType::Character(_) => false,
        DataType::Char(_) => false,
        DataType::CharacterVarying(_) => false,
        DataType::CharVarying(_) => false,
        DataType::Varchar(_) => false,
        DataType::Nvarchar(_) => false,
        DataType::Uuid => false,
        DataType::CharacterLargeObject(_) => false,
        DataType::CharLargeObject(_) => false,
        DataType::Clob(_) => false,
        DataType::Binary(_) => false,
        DataType::Varbinary(_) => false,
        DataType::Blob(_) => false,
        DataType::TinyBlob => false,
        DataType::MediumBlob => false,
        DataType::LongBlob => false,
        DataType::Bytes(_) => false,
        DataType::Numeric(_) => false,
        DataType::Decimal(_) => false,
        DataType::DecimalUnsigned(_) => false,
        DataType::BigNumeric(_) => false,
        DataType::BigDecimal(_) => false,
        DataType::Dec(_) => false,
        DataType::DecUnsigned(_) => false,
        DataType::Float(_) => false,
        DataType::FloatUnsigned(_) => false,
        DataType::TinyInt(_) => true,
        DataType::TinyIntUnsigned(_) => true,
        DataType::UTinyInt => true,
        DataType::Int2(_) => true,
        DataType::Int2Unsigned(_) => true,
        DataType::SmallInt(_) => true,
        DataType::SmallIntUnsigned(_) => true,
        DataType::USmallInt => true,
        DataType::MediumInt(_) => true,
        DataType::MediumIntUnsigned(_) => true,
        DataType::Int(_) => true,
        DataType::Int4(_) => true,
        DataType::Int8(_) => true,
        DataType::Int16 => true,
        DataType::Int32 => true,
        DataType::Int64 => true,
        DataType::Int128 => true,
        DataType::Int256 => true,
        DataType::Integer(_) => true,
        DataType::IntUnsigned(_) => true,
        DataType::Int4Unsigned(_) => true,
        DataType::IntegerUnsigned(_) => true,
        DataType::HugeInt => true,
        DataType::UHugeInt => true,
        DataType::UInt8 => true,
        DataType::UInt16 => true,
        DataType::UInt32 => true,
        DataType::UInt64 => true,
        DataType::UInt128 => true,
        DataType::UInt256 => true,
        DataType::BigInt(_) => true,
        DataType::BigIntUnsigned(_) => true,
        DataType::UBigInt => true,
        DataType::Int8Unsigned(_) => true,
        DataType::Signed => true,
        DataType::SignedInteger => true,
        DataType::Unsigned => true,
        DataType::UnsignedInteger => true,
        DataType::Float4 => false,
        DataType::Float32 => false,
        DataType::Float64 => false,
        DataType::Real => false,
        DataType::RealUnsigned => false,
        DataType::Float8 => false,
        DataType::Double(_) => false,
        DataType::DoubleUnsigned(_) => false,
        DataType::DoublePrecision => false,
        DataType::DoublePrecisionUnsigned => false,
        DataType::Bool => false,
        DataType::Boolean => false,
        DataType::Date => false,
        DataType::Date32 => false,
        DataType::Time(_, _) => false,
        DataType::Datetime(_) => false,
        DataType::Datetime64(_, _) => false,
        DataType::Timestamp(_, _) => false,
        DataType::TimestampNtz(_) => false,
        DataType::Interval { .. } => false,
        DataType::JSON => false,
        DataType::JSONB => false,
        DataType::Regclass => false,
        DataType::Text => false,
        DataType::TinyText => false,
        DataType::MediumText => false,
        DataType::LongText => false,
        DataType::String(_) => false,
        DataType::FixedString(_) => false,
        DataType::Bytea => false,
        DataType::Bit(_) => false,
        DataType::BitVarying(_) => false,
        DataType::VarBit(_) => false,
        DataType::Custom(_, _) => false,
        DataType::Array(_) => false,
        DataType::Map(_, _) => false,
        DataType::Tuple(_) => false,
        DataType::Nested(_) => false,
        DataType::Enum(_, _) => false,
        DataType::Set(_) => false,
        DataType::Struct(_, _) => false,
        DataType::Union(_) => false,
        DataType::Nullable(_) => false,
        DataType::LowCardinality(_) => false,
        DataType::Unspecified => false,
        DataType::Trigger => false,
        DataType::AnyType => false,
        DataType::GeometricType(_) => false,
        DataType::TsVector => false,
        DataType::TsQuery => false,
    }
}

pub(crate) fn ty_is_float(ty: &DataType) -> bool {
    match ty {
        DataType::Table(_) => false,
        DataType::NamedTable { .. } => false,
        DataType::Character(_) => false,
        DataType::Char(_) => false,
        DataType::CharacterVarying(_) => false,
        DataType::CharVarying(_) => false,
        DataType::Varchar(_) => false,
        DataType::Nvarchar(_) => false,
        DataType::Uuid => false,
        DataType::CharacterLargeObject(_) => false,
        DataType::CharLargeObject(_) => false,
        DataType::Clob(_) => false,
        DataType::Binary(_) => false,
        DataType::Varbinary(_) => false,
        DataType::Blob(_) => false,
        DataType::TinyBlob => false,
        DataType::MediumBlob => false,
        DataType::LongBlob => false,
        DataType::Bytes(_) => false,
        DataType::Numeric(_) => false,
        DataType::Decimal(_) => false,
        DataType::DecimalUnsigned(_) => false,
        DataType::BigNumeric(_) => false,
        DataType::BigDecimal(_) => false,
        DataType::Dec(_) => false,
        DataType::DecUnsigned(_) => false,
        DataType::Float(_) => true,
        DataType::FloatUnsigned(_) => true,
        DataType::TinyInt(_) => false,
        DataType::TinyIntUnsigned(_) => false,
        DataType::UTinyInt => false,
        DataType::Int2(_) => false,
        DataType::Int2Unsigned(_) => false,
        DataType::SmallInt(_) => false,
        DataType::SmallIntUnsigned(_) => false,
        DataType::USmallInt => false,
        DataType::MediumInt(_) => false,
        DataType::MediumIntUnsigned(_) => false,
        DataType::Int(_) => false,
        DataType::Int4(_) => false,
        DataType::Int8(_) => false,
        DataType::Int16 => false,
        DataType::Int32 => false,
        DataType::Int64 => false,
        DataType::Int128 => false,
        DataType::Int256 => false,
        DataType::Integer(_) => false,
        DataType::IntUnsigned(_) => false,
        DataType::Int4Unsigned(_) => false,
        DataType::IntegerUnsigned(_) => false,
        DataType::HugeInt => false,
        DataType::UHugeInt => false,
        DataType::UInt8 => false,
        DataType::UInt16 => false,
        DataType::UInt32 => false,
        DataType::UInt64 => false,
        DataType::UInt128 => false,
        DataType::UInt256 => false,
        DataType::BigInt(_) => false,
        DataType::BigIntUnsigned(_) => false,
        DataType::UBigInt => false,
        DataType::Int8Unsigned(_) => false,
        DataType::Signed => false,
        DataType::SignedInteger => false,
        DataType::Unsigned => false,
        DataType::UnsignedInteger => false,
        DataType::Float4 => true,
        DataType::Float32 => true,
        DataType::Float64 => true,
        DataType::Real => true,
        DataType::RealUnsigned => true,
        DataType::Float8 => true,
        DataType::Double(_) => true,
        DataType::DoubleUnsigned(_) => true,
        DataType::DoublePrecision => true,
        DataType::DoublePrecisionUnsigned => true,
        DataType::Bool => false,
        DataType::Boolean => false,
        DataType::Date => false,
        DataType::Date32 => false,
        DataType::Time(_, _) => false,
        DataType::Datetime(_) => false,
        DataType::Datetime64(_, _) => false,
        DataType::Timestamp(_, _) => false,
        DataType::TimestampNtz(_) => false,
        DataType::Interval { .. } => false,
        DataType::JSON => false,
        DataType::JSONB => false,
        DataType::Regclass => false,
        DataType::Text => false,
        DataType::TinyText => false,
        DataType::MediumText => false,
        DataType::LongText => false,
        DataType::String(_) => false,
        DataType::FixedString(_) => false,
        DataType::Bytea => false,
        DataType::Bit(_) => false,
        DataType::BitVarying(_) => false,
        DataType::VarBit(_) => false,
        DataType::Custom(_, _) => false,
        DataType::Array(_) => false,
        DataType::Map(_, _) => false,
        DataType::Tuple(_) => false,
        DataType::Nested(_) => false,
        DataType::Enum(_, _) => false,
        DataType::Set(_) => false,
        DataType::Struct(_, _) => false,
        DataType::Union(_) => false,
        DataType::Nullable(_) => false,
        DataType::LowCardinality(_) => false,
        DataType::Unspecified => false,
        DataType::Trigger => false,
        DataType::AnyType => false,
        DataType::GeometricType(_) => false,
        DataType::TsVector => false,
        DataType::TsQuery => false,
    }
}

pub(crate) fn ty_is_decimal(ty: &DataType) -> bool {
    match ty {
        // Decimal-like types return true
        DataType::Numeric(_)
        | DataType::Decimal(_)
        | DataType::DecimalUnsigned(_)
        | DataType::BigNumeric(_)
        | DataType::BigDecimal(_)
        | DataType::Dec(_)
        | DataType::DecUnsigned(_) => true,

        // All other types return false
        DataType::Table(_)
        | DataType::NamedTable { .. }
        | DataType::Character(_)
        | DataType::Char(_)
        | DataType::CharacterVarying(_)
        | DataType::CharVarying(_)
        | DataType::Varchar(_)
        | DataType::Nvarchar(_)
        | DataType::Uuid
        | DataType::CharacterLargeObject(_)
        | DataType::CharLargeObject(_)
        | DataType::Clob(_)
        | DataType::Binary(_)
        | DataType::Varbinary(_)
        | DataType::Blob(_)
        | DataType::TinyBlob
        | DataType::MediumBlob
        | DataType::LongBlob
        | DataType::Bytes(_)
        | DataType::Float(_)
        | DataType::FloatUnsigned(_)
        | DataType::TinyInt(_)
        | DataType::TinyIntUnsigned(_)
        | DataType::UTinyInt
        | DataType::Int2(_)
        | DataType::Int2Unsigned(_)
        | DataType::SmallInt(_)
        | DataType::SmallIntUnsigned(_)
        | DataType::USmallInt
        | DataType::MediumInt(_)
        | DataType::MediumIntUnsigned(_)
        | DataType::Int(_)
        | DataType::Int4(_)
        | DataType::Int8(_)
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::Int128
        | DataType::Int256
        | DataType::Integer(_)
        | DataType::IntUnsigned(_)
        | DataType::Int4Unsigned(_)
        | DataType::IntegerUnsigned(_)
        | DataType::HugeInt
        | DataType::UHugeInt
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64
        | DataType::UInt128
        | DataType::UInt256
        | DataType::BigInt(_)
        | DataType::BigIntUnsigned(_)
        | DataType::UBigInt
        | DataType::Int8Unsigned(_)
        | DataType::Signed
        | DataType::SignedInteger
        | DataType::Unsigned
        | DataType::UnsignedInteger
        | DataType::Float4
        | DataType::Float32
        | DataType::Float64
        | DataType::Real
        | DataType::RealUnsigned
        | DataType::Float8
        | DataType::Double(_)
        | DataType::DoubleUnsigned(_)
        | DataType::DoublePrecision
        | DataType::DoublePrecisionUnsigned
        | DataType::Bool
        | DataType::Boolean
        | DataType::Date
        | DataType::Date32
        | DataType::Time(_, _)
        | DataType::Datetime(_)
        | DataType::Datetime64(_, _)
        | DataType::Timestamp(_, _)
        | DataType::TimestampNtz(_)
        | DataType::Interval { .. }
        | DataType::JSON
        | DataType::JSONB
        | DataType::Regclass
        | DataType::Text
        | DataType::TinyText
        | DataType::MediumText
        | DataType::LongText
        | DataType::String(_)
        | DataType::FixedString(_)
        | DataType::Bytea
        | DataType::Bit(_)
        | DataType::BitVarying(_)
        | DataType::VarBit(_)
        | DataType::Custom(_, _)
        | DataType::Array(_)
        | DataType::Map(_, _)
        | DataType::Tuple(_)
        | DataType::Nested(_)
        | DataType::Enum(_, _)
        | DataType::Set(_)
        | DataType::Struct(_, _)
        | DataType::Union(_)
        | DataType::Nullable(_)
        | DataType::LowCardinality(_)
        | DataType::Unspecified
        | DataType::Trigger
        | DataType::AnyType
        | DataType::GeometricType(_)
        | DataType::TsVector
        | DataType::TsQuery => false,
    }
}

#[allow(dead_code)]
pub fn generate_random_string(len: usize) -> String {
    use rand::prelude::*;
    // 定义只包含字母和数字的字符集
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                              abcdefghijklmnopqrstuvwxyz\
                              0123456789";

    let mut rng = rand::rng();
    let rand_string: String = (0..len)
        .map(|_| {
            let idx = rng.random_range(0usize..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();
    rand_string
}

pub(crate) fn extract_bytes_from_serde_values(
    values: &[serde_json::Value],
) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(values.len());
    for v in values.iter() {
        match v {
            serde_json::Value::Number(number) => match number.as_u64() {
                Some(x) => bytes.push(x as u8),
                None => anyhow::bail!(""),
            },
            _ => {
                anyhow::bail!(format!(
                    "inside a array,not a number,can't convert this array to bytes"
                ))
            }
        }
    }
    anyhow::Ok(bytes)
}

pub(crate) fn get_array_tuple_element_type(
    ty: &DataType,
    index: usize,
) -> anyhow::Result<&DataType> {
    let r = match extract_null_able(ty) {
        DataType::Array(x) => match x {
            ArrayElemTypeDef::None => {
                anyhow::bail!("ArrayElemTypeDef::None can't retrive a valid type")
            }
            ArrayElemTypeDef::AngleBracket(data_type) => data_type.as_ref(),
            ArrayElemTypeDef::SquareBracket(data_type, _) => data_type.as_ref(),
            ArrayElemTypeDef::Parenthesis(data_type) => data_type.as_ref(),
        },
        DataType::Tuple(x) => &mut match x.get(index) {
            Some(x) => &x.field_type,
            None => anyhow::bail!(format!("not found type on tupe#{}", index)),
        },
        _ => {
            anyhow::bail!(format!("type not is not ok,should be array,but:{:?}", ty))
        }
    };
    anyhow::Ok(r)
}

pub(crate) fn extract_null_able(x: &DataType) -> &DataType {
    match x {
        DataType::Nullable(x) => x.as_ref(),
        _ => x,
    }
}

impl BackendKind {
    pub async fn infer_fields(self, path: &str) -> anyhow::Result<Vec<Field>> {
        let path = path.to_owned();

        tokio::task::spawn_blocking(move || {
            let path_lower = path.to_lowercase();

            if path_lower.ends_with(".csv") {
                self.infer_csv(&path)
            } else if path_lower.ends_with(".json") {
                self.infer_json(&path)
            } else if path_lower.ends_with(".jsonl") {
                self.infer_jsonline(&path)
            } else {
                anyhow::bail!(format!("unsupported file type:{}", path))
            }
        })
        .await?
    }

    pub fn infer_csv(self, path: &str) -> anyhow::Result<Vec<Field>> {
        let df = csv_processor::DataFrame::from_csv(path)?;
        let mut ret = vec![];

        // 直接从 df 获取列信息
        for (index, col) in df.columns().iter().enumerate() {
            let name = df
                .headers()
                .get(index)
                .map(|x| x.clone())
                .unwrap_or_default();
            // 根据列的数据类型映射
            let data_type = self.map_csv_dtype(&col.dtype())?;

            ret.push(Field {
                field: name,
                data_type,
                ..Default::default()
            });
        }

        anyhow::Ok(ret)
    }

    pub fn infer_json(self, _path: &str) -> anyhow::Result<Vec<Field>> {
        todo!()
    }
    pub fn infer_jsonline(self, _path: &str) -> anyhow::Result<Vec<Field>> {
        // let file = std::fs::File::open(path)?;
        // let reader = std::io::BufReader::new(file);
        // let samples: Vec<String> = Vec::new();

        // for line in reader.lines() {
        //     let line = line?;
        //     if line.trim().is_empty() {
        //         continue;
        //     }
        //     samples.push(line);
        //     if samples.len() >= 100 {
        //         break;
        //     }
        // }

        // let schema = genson_core::infer_json_schema(&samples, None).unwrap();

        // self.infer_json_from_samples(&schema)

        unimplemented!()
    }
}

impl BackendKind {
    /// 核心職責分離：根據當前資料庫與 CSV 推導出的型態，返回該資料庫最適合的欄位型態名稱
    pub fn map_csv_dtype(&self, csv_dtype: &csv_processor::Dtype) -> anyhow::Result<String> {
        use csv_processor::Dtype;
        let err = anyhow::Error::msg(format!("'{} not support infer type from csv", self));

        let r = match self {
            BackendKind::Mysql(_) => match csv_dtype {
                Dtype::Integer => "INT".to_string(),
                Dtype::Float => "DOUBLE".to_string(),
                Dtype::Boolean => "TINYINT(1)".to_string(),
                Dtype::Date => "DATE".to_string(),
                Dtype::DateTime => "DATETIME".to_string(),
                Dtype::Str => "VARCHAR(255)".to_string(),
                Dtype::Null => "VARCHAR(255)".to_string(), // 預設降級為文字
            },
            BackendKind::PostGres(_) => match csv_dtype {
                Dtype::Integer => "BIGINT".to_string(),
                Dtype::Float => "DOUBLE PRECISION".to_string(),
                Dtype::Boolean => "BOOLEAN".to_string(),
                Dtype::Date => "DATE".to_string(),
                Dtype::DateTime => "TIMESTAMP".to_string(),
                Dtype::Str => "TEXT".to_string(),
                Dtype::Null => "TEXT".to_string(),
            },
            BackendKind::Sqlite | BackendKind::LibSql => match csv_dtype {
                Dtype::Integer => "INTEGER".to_string(),
                Dtype::Float => "REAL".to_string(),
                Dtype::Boolean => "INTEGER".to_string(), // SQLite 底層無布林，習慣宣告為 INTEGER (存 0 或 1)
                Dtype::Date => "DATE".to_string(), // 修正：直接使用 DATE 宣告，方便 ORM/驅動對齊型態
                Dtype::DateTime => "DATETIME".to_string(), // 修正：直接使用 DATETIME 宣告
                Dtype::Str | Dtype::Null => "TEXT".to_string(),
            },
            BackendKind::ClickHouse => match csv_dtype {
                Dtype::Integer => "Int64".to_string(),
                Dtype::Float => "Float64".to_string(),
                Dtype::Boolean => "Bool".to_string(),
                Dtype::Date => "Date32".to_string(),
                Dtype::DateTime => "DateTime64(3)".to_string(),
                Dtype::Str | Dtype::Null => "String".to_string(),
            },
            BackendKind::DuckDB => match csv_dtype {
                Dtype::Integer => "BIGINT".to_string(),
                Dtype::Float => "DOUBLE".to_string(),
                Dtype::Boolean => "BOOLEAN".to_string(),
                Dtype::Date => "DATE".to_string(),
                Dtype::DateTime => "TIMESTAMP".to_string(),
                Dtype::Str | Dtype::Null => "VARCHAR".to_string(),
            },
            BackendKind::BigQuery => match csv_dtype {
                Dtype::Integer => "INT64".to_string(),
                Dtype::Float => "FLOAT64".to_string(),
                Dtype::Boolean => "BOOL".to_string(),
                Dtype::Date => "DATE".to_string(),
                Dtype::DateTime => "DATETIME".to_string(),
                Dtype::Str | Dtype::Null => "STRING".to_string(),
            },
            BackendKind::SqlServer => match csv_dtype {
                Dtype::Integer => "INT".to_string(),
                Dtype::Float => "FLOAT".to_string(),
                Dtype::Boolean => "BIT".to_string(), // SQL Server 的布林是 BIT
                Dtype::Date => "DATE".to_string(),
                Dtype::DateTime => "DATETIME2".to_string(),
                Dtype::Str | Dtype::Null => "NVARCHAR(MAX)".to_string(),
            },
            BackendKind::Oracle => match csv_dtype {
                Dtype::Integer => "NUMBER(19)".to_string(),
                Dtype::Float => "NUMBER".to_string(),
                Dtype::Boolean => "NUMBER(1)".to_string(), // Oracle 傳統上沒有專門的布林型態
                Dtype::Date => "DATE".to_string(),
                Dtype::DateTime => "TIMESTAMP".to_string(),
                Dtype::Str | Dtype::Null => "VARCHAR2(4000)".to_string(),
            },
            BackendKind::Db2 => match csv_dtype {
                Dtype::Integer => "BIGINT".to_string(),
                Dtype::Float => "DOUBLE".to_string(),
                Dtype::Boolean => "BOOLEAN".to_string(),
                Dtype::Date => "DATE".to_string(),
                Dtype::DateTime => "TIMESTAMP".to_string(),
                Dtype::Str | Dtype::Null => "VARCHAR(32700)".to_string(),
            },
            BackendKind::Firebird => match csv_dtype {
                Dtype::Integer => "BIGINT".to_string(),
                Dtype::Float => "DOUBLE PRECISION".to_string(),
                Dtype::Boolean => "BOOLEAN".to_string(),
                Dtype::Date => "DATE".to_string(),
                Dtype::DateTime => "TIMESTAMP".to_string(),
                Dtype::Str | Dtype::Null => "VARCHAR(8191)".to_string(),
            },
            BackendKind::Cassandra => match csv_dtype {
                Dtype::Integer => "bigint".to_string(),
                Dtype::Float => "double".to_string(),
                Dtype::Boolean => "boolean".to_string(),
                Dtype::Date => "date".to_string(),
                Dtype::DateTime => "timestamp".to_string(),
                Dtype::Str | Dtype::Null => "text".to_string(),
            },
            // NoSQL 或 鍵值資料庫 通常全部對齊到基礎文字或其特定 BSON 型態
            BackendKind::Mongo => return Err(err),
            BackendKind::CouchBase => return Err(err),
            BackendKind::Redis => return Err(err),
        };
        anyhow::Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_off_iter() {
        assert_eq!(
            OffsetLimitIter::new(9, 3).collect::<Vec<_>>(),
            vec![0, 3, 6]
        );
        assert_eq!(
            OffsetLimitIter::new(9, 4).collect::<Vec<_>>(),
            vec![0, 4, 8]
        );
    }

    #[test]

    fn test_infer_csv() {
        let x = BackendKind::Sqlite
            .infer_csv("/home/yuyang/Downloads/Persons.csv")
            .unwrap();
        for y in x.iter() {
            println!("{:?}", y);
        }
    }

    #[test]
    fn test_current_timestamp() {
        let x = current_timestamp();
        println!("{}", x);
    }
}
