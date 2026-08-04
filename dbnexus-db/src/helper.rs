use std::{io::BufRead, vec};

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

    pub fn infer_json(self, path: &str) -> anyhow::Result<Vec<Field>> {
        let file = std::fs::File::open(path)?;
        let vs: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_reader(file)?;

        let samples = if vs.len() >= 10 { &vs[0..10] } else { &vs };

        self.infer_from_json_maps(samples)
    }

    pub fn infer_jsonline(self, path: &str) -> anyhow::Result<Vec<Field>> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let mut samples: Vec<serde_json::Map<String, serde_json::Value>> = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            samples.push(serde_json::from_str::<
                serde_json::Map<String, serde_json::Value>,
            >(&line)?);
            if samples.len() >= 100 {
                break;
            }
        }
        self.infer_from_json_maps(&samples)
    }

    pub fn infer_from_json_maps(
        &self,
        values: &[serde_json::Map<String, serde_json::Value>],
    ) -> anyhow::Result<Vec<Field>> {
        let mut merged_value = serde_json::Map::new();
        for value in values.iter() {
            for (k, v) in value {
                match v {
                    serde_json::Value::Null => {
                        if !merged_value.contains_key(k) {
                            merged_value.insert(k.clone(), v.clone());
                        }
                    }
                    _ => {
                        merged_value.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        self.infer_from_json_map(&merged_value)
    }

    pub fn infer_from_json_map(
        &self,
        map: &serde_json::Map<String, serde_json::Value>,
    ) -> anyhow::Result<Vec<Field>> {
        let mut ret = vec![];

        for (k, v) in map.iter() {
            let ty = self.infer_type_from_json_value(v)?;
            ret.push(Field::from_name_type(k, ty));
        }
        anyhow::Ok(ret)
    }

    pub fn infer_type_from_json_value(&self, value: &serde_json::Value) -> anyhow::Result<String> {
        let r = match self {
            BackendKind::Mysql(..) => match value {
                serde_json::Value::Null => "VARCHAR(255)".to_string(),
                serde_json::Value::Bool(_) => "TINYINT(1)".to_string(),
                serde_json::Value::Number(number) => {
                    if number.is_i64() || number.is_u64() {
                        if let Some(..) = number.as_i64() {
                            "BIGINT".to_string()
                        } else if let Some(..) = number.as_u64() {
                            "BIGINT UNSIGNED".to_string()
                        } else {
                            "DECIMAL(65,30)".to_string()
                        }
                    } else if number.is_f64() {
                        "DOUBLE".to_string()
                    } else {
                        "DECIMAL(65,30)".to_string()
                    }
                }
                serde_json::Value::String(..) => "TEXT".to_string(),
                serde_json::Value::Array(_) => "JSON".to_string(), // MySQL 5.7+ 支持 JSON
                serde_json::Value::Object(_) => "JSON".to_string(),
            },

            BackendKind::PostGres(_) => match value {
                serde_json::Value::Null => "VARCHAR(255)".to_string(),
                serde_json::Value::Bool(_) => "BOOLEAN".to_string(),
                serde_json::Value::Number(number) => {
                    if number.is_i64() || number.is_u64() {
                        if let Some(..) = number.as_i64() {
                            "BIGINT".to_string()
                        } else if let Some(..) = number.as_u64() {
                            "BIGINT".to_string()
                        } else {
                            "NUMERIC".to_string()
                        }
                    } else if number.is_f64() {
                        "DOUBLE PRECISION".to_string()
                    } else {
                        "NUMERIC".to_string()
                    }
                }
                serde_json::Value::String(..) => "TEXT".to_string(),
                serde_json::Value::Array(_) => "JSONB".to_string(),
                serde_json::Value::Object(_) => "JSONB".to_string(),
            },

            BackendKind::SqlServer => match value {
                serde_json::Value::Null => "NVARCHAR(255)".to_string(),
                serde_json::Value::Bool(_) => "BIT".to_string(),
                serde_json::Value::Number(number) => {
                    if number.is_i64() || number.is_u64() {
                        if let Some(..) = number.as_i64() {
                            "BIGINT".to_string()
                        } else if let Some(..) = number.as_u64() {
                            "BIGINT".to_string()
                        } else {
                            "DECIMAL(38,10)".to_string()
                        }
                    } else if number.is_f64() {
                        "FLOAT".to_string()
                    } else {
                        "DECIMAL(38,10)".to_string()
                    }
                }
                serde_json::Value::String(..) => "NVARCHAR(?)".to_string(),
                serde_json::Value::Array(_) => "BINARY(?)".to_string(),
                serde_json::Value::Object(_) => "NVARCHAR(?)".to_string(),
            },

            BackendKind::Sqlite | BackendKind::LibSql => match value {
                serde_json::Value::Null => "TEXT".to_string(),
                serde_json::Value::Bool(_) => "INTEGER".to_string(),
                serde_json::Value::Number(number) => {
                    if number.is_i64() || number.is_u64() {
                        "INTEGER".to_string()
                    } else if number.is_f64() {
                        "REAL".to_string()
                    } else {
                        "REAL".to_string()
                    }
                }
                serde_json::Value::String(_) => "TEXT".to_string(),
                serde_json::Value::Array(_) => "BLOB".to_string(),
                serde_json::Value::Object(_) => "TEXT".to_string(),
            },

            BackendKind::Cassandra => {
                fn handle_value(v: &serde_json::Value) -> String {
                    match v {
                        serde_json::Value::Null => "text".to_string(),
                        serde_json::Value::Bool(_) => "boolean".to_string(),
                        serde_json::Value::Number(number) => {
                            if number.is_i64() || number.is_u64() {
                                "bigint".to_string()
                            } else if number.is_f64() {
                                "double".to_string()
                            } else {
                                "decimal".to_string()
                            }
                        }
                        serde_json::Value::String(_) => "text".to_string(),
                        serde_json::Value::Array(x) => {
                            if let Some(v) = x.get(0) {
                                return format!("list<{}>", handle_value(v));
                            } else {
                                return "list<text>".into();
                            }
                        }
                        serde_json::Value::Object(x) => {
                            if let Some((k, v)) = x.iter().next() {
                                format!(
                                    "map<{}, {}>",
                                    if let Ok(..) = k.parse::<u64>() {
                                        "bigint"
                                    } else if let Ok(..) = k.parse::<f64>() {
                                        "double"
                                    } else {
                                        "String"
                                    },
                                    handle_value(&v)
                                )
                            } else {
                                "map<text, text>".to_string()
                            }
                        }
                    }
                }

                handle_value(value)
            }
            BackendKind::ClickHouse => {
                fn handle_value(value: &serde_json::Value) -> String {
                    match value {
                        serde_json::Value::Null => "String".to_string(),
                        serde_json::Value::Bool(_) => "UInt8".to_string(),
                        serde_json::Value::Number(number) => {
                            if let Some(..) = number.as_i64() {
                                "Int64".to_string()
                            } else if let Some(..) = number.as_u64() {
                                "UInt64".to_string()
                            } else {
                                "Float64".to_string()
                            }
                        }
                        serde_json::Value::String(..) => "String".to_string(),
                        serde_json::Value::Array(x) => {
                            if let Some(x) = x.get(0) {
                                format!("Array({})", handle_value(x))
                            } else {
                                return "Array(String)".into();
                            }
                        }
                        serde_json::Value::Object(x) => {
                            if let Some((k, v)) = x.iter().next() {
                                format!(
                                    "Map({},{})",
                                    if let Ok(..) = k.parse::<u64>() {
                                        "UInt64"
                                    } else if let Ok(..) = k.parse::<i64>() {
                                        "UInt64"
                                    } else if let Ok(..) = k.parse::<f64>() {
                                        "Float64"
                                    } else {
                                        "String"
                                    },
                                    handle_value(v)
                                )
                            } else {
                                return "Map(Int32,Int32)".into();
                            }
                        }
                    }
                }

                handle_value(value)
            }

            BackendKind::Firebird => match value {
                serde_json::Value::Null => "VARCHAR(255)".to_string(),
                serde_json::Value::Bool(_) => "SMALLINT".to_string(),
                serde_json::Value::Number(number) => {
                    if number.is_i64() || number.is_u64() {
                        if let Some(..) = number.as_i64() {
                            "BIGINT".to_string()
                        } else if let Some(..) = number.as_u64() {
                            "BIGINT".to_string()
                        } else {
                            "DOUBLE PRECISION".to_string()
                        }
                    } else if number.is_f64() {
                        "DOUBLE PRECISION".to_string()
                    } else {
                        "DOUBLE PRECISION".to_string()
                    }
                }
                serde_json::Value::String(s) => {
                    let len = s.len();
                    if len <= 255 {
                        "VARCHAR(255)".to_string()
                    } else {
                        "BLOB SUB_TYPE TEXT".to_string()
                    }
                }
                serde_json::Value::Array(_) => "BLOB SUB_TYPE TEXT".to_string(),
                serde_json::Value::Object(_) => "BLOB SUB_TYPE TEXT".to_string(),
            },

            BackendKind::Oracle => match value {
                serde_json::Value::Null => "VARCHAR2(255)".to_string(),
                serde_json::Value::Bool(_) => "NUMBER(1)".to_string(),
                serde_json::Value::Number(number) => {
                    if number.is_i64() || number.is_u64() {
                        if let Some(..) = number.as_i64() {
                            "NUMBER".to_string()
                        } else {
                            "NUMBER".to_string()
                        }
                    } else if number.is_f64() {
                        "FLOAT".to_string()
                    } else {
                        "NUMBER".to_string()
                    }
                }
                serde_json::Value::String(..) => "VARCHAR2(4000)".to_string(),
                serde_json::Value::Array(_) => "CLOB".to_string(),
                serde_json::Value::Object(_) => "CLOB".to_string(),
            },

            BackendKind::DuckDB => match value {
                serde_json::Value::Null => "VARCHAR".to_string(),
                serde_json::Value::Bool(_) => "BOOLEAN".to_string(),
                serde_json::Value::Number(number) => {
                    if number.is_i64() || number.is_u64() {
                        "BIGINT".to_string()
                    } else if number.is_f64() {
                        "DOUBLE".to_string()
                    } else {
                        "DOUBLE".to_string()
                    }
                }
                serde_json::Value::String(_) => "VARCHAR".to_string(),
                serde_json::Value::Array(_) => "JSON".to_string(),
                serde_json::Value::Object(_) => "JSON".to_string(),
            },

            BackendKind::BigQuery => match value {
                serde_json::Value::Null => "STRING".to_string(),
                serde_json::Value::Bool(_) => "BOOL".to_string(),
                serde_json::Value::Number(number) => {
                    if number.is_i64() || number.is_u64() {
                        "INT64".to_string()
                    } else if number.is_f64() {
                        "FLOAT64".to_string()
                    } else {
                        "NUMERIC".to_string()
                    }
                }
                serde_json::Value::String(_) => "STRING".to_string(),
                serde_json::Value::Array(_) => "ARRAY".to_string(),
                serde_json::Value::Object(_) => "JSON".to_string(),
            },

            BackendKind::Db2 => match value {
                serde_json::Value::Null => "VARCHAR(255)".to_string(),
                serde_json::Value::Bool(_) => "SMALLINT".to_string(),
                serde_json::Value::Number(number) => {
                    if number.is_i64() || number.is_u64() {
                        "BIGINT".to_string()
                    } else if number.is_f64() {
                        "DOUBLE".to_string()
                    } else {
                        "DECIMAL".to_string()
                    }
                }
                serde_json::Value::String(..) => "VARCHAR(32672)".to_string(),
                serde_json::Value::Array(_) => "CLOB".to_string(),
                serde_json::Value::Object(_) => "CLOB".to_string(),
            },

            BackendKind::CouchBase => match value {
                serde_json::Value::Null => "null".to_string(),
                serde_json::Value::Bool(_) => "boolean".to_string(),
                serde_json::Value::Number(number) => {
                    if number.is_i64() || number.is_u64() {
                        "number".to_string()
                    } else if number.is_f64() {
                        "number".to_string()
                    } else {
                        "number".to_string()
                    }
                }
                serde_json::Value::String(_) => "string".to_string(),
                serde_json::Value::Array(_) => "array".to_string(),
                serde_json::Value::Object(_) => "object".to_string(),
            },

            BackendKind::Mongo => anyhow::bail!("mongo ne need impl this"),

            BackendKind::Redis => anyhow::bail!("mongo ne need impl this"),
        };

        Ok(r)
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
    use crate::db::{BackendKindMysql, BackendKindPostGres};

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

    fn test_infer_schema_csv() {
        let x = BackendKind::Sqlite
            .infer_csv("/home/yuyang/Downloads/Persons.csv")
            .unwrap();
        for y in x.iter() {
            println!("{:?}", y);
        }
    }

    #[test]

    fn test_infer_json_type() {
        let backends = vec![
            BackendKind::Sqlite,
            BackendKind::Mysql(BackendKindMysql::Mysql),
            BackendKind::PostGres(BackendKindPostGres::PostGres),
            BackendKind::Cassandra,
            BackendKind::ClickHouse,
            BackendKind::SqlServer,
            BackendKind::Firebird,
            BackendKind::Oracle,
            BackendKind::DuckDB,
        ];

        for kind in backends.iter() {
            for value in &[
                serde_json::Value::Bool(true),
                serde_json::Value::from(1),
                serde_json::Value::from(1f64),
                serde_json::Value::from("haha"),
                serde_json::Value::Array(
                    [1, 2, 3, 4]
                        .iter()
                        .map(|x| serde_json::Value::from(*x))
                        .collect(),
                ),
                serde_json::json!({"name":"John Doe"}),
                serde_json::json!({"1":"John Doe"}),
                serde_json::json!({"1.1":"John Doe"}),
                serde_json::json!({"-111":"John Doe"}),
            ] {
                let ty = kind.infer_type_from_json_value(value).unwrap();
                println!("kind:{} value:{} ty:{}", kind, value, ty);
            }
        }
    }

    #[test]
    fn test_current_timestamp() {
        let x = current_timestamp();
        println!("{}", x);
    }
}
