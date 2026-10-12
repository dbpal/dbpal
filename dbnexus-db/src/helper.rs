use std::{
    fmt::Display,
    io::{BufRead, BufReader},
    vec,
};

use crate::db::{BackendKind, Field};
use calamine::Reader;
use sqlparser::ast::{ArrayElemTypeDef, ColumnDef, DataType, StructField};
use std::process::Command;

/// create csv file list of string
/// each line of line separator
pub fn create_csv(
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

pub trait ResultExtraContext<T, E> {
    fn context_msg(self, _: &str) -> anyhow::Result<T>;
}

impl<T, E: Display> ResultExtraContext<T, E> for Result<T, E> {
    fn context_msg(self, m: &str) -> anyhow::Result<T> {
        match self {
            Ok(x) => anyhow::Ok(x),
            Err(err) => Err(anyhow::Error::msg(format!("{}: err:{}", m, err))),
        }
    }
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
    pub total: Index,
    pub offset: Index,
    pub size: Index,
}

impl<T: Default> OffsetLimitIter<T> {
    pub fn new(total: T, size: T) -> Self {
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
pub fn remove_first_char_string(s: String) -> String {
    s.chars().skip(1).collect()
}

// try remove a @ used for mysql etc database
pub fn try_remove_one_at(value: String) -> String {
    if value.starts_with('@') && !value.starts_with("@@") {
        remove_first_char_string(value)
    } else {
        value
    }
}

pub fn ty_is_datetime(ty: &DataType) -> bool {
    matches!(ty, DataType::Datetime(..) | DataType::Datetime64(..))
}
pub fn ty_is_time(ty: &DataType) -> bool {
    matches!(ty, DataType::Time(..))
}
pub fn ty_is_date(ty: &DataType) -> bool {
    matches!(ty, DataType::Date | DataType::Date32)
}

// check if ty is plain integer
pub fn ty_is_int(ty: &DataType) -> bool {
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

pub fn ty_is_float(ty: &DataType) -> bool {
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

pub fn ty_is_decimal(ty: &DataType) -> bool {
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

pub fn uuid_v4_string() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn extract_bytes_from_serde_values(values: &[serde_json::Value]) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(values.len());
    for v in values.iter() {
        match v {
            serde_json::Value::Number(number) => match number.as_u64() {
                Some(x) => bytes.push(x as u8),
                None => anyhow::bail!(""),
            },
            _ => {
                anyhow::bail!("inside a array,not a number,can't convert this array to bytes")
            }
        }
    }
    anyhow::Ok(bytes)
}

pub fn get_array_tuple_element_type(ty: &DataType, index: usize) -> anyhow::Result<&DataType> {
    let r = match ty {
        DataType::Array(ast) => match ast {
            ArrayElemTypeDef::None => {
                anyhow::bail!("ArrayElemTypeDef::None can't retrive a valid type")
            }
            ArrayElemTypeDef::AngleBracket(data_type)
            | ArrayElemTypeDef::SquareBracket(data_type, _)
            | ArrayElemTypeDef::Parenthesis(data_type) => data_type.as_ref(), //  ArrayElemTypeDef::Qualified(data_type, _) => data_type.as_ref(),
        },
        DataType::Tuple(x) => match x.get(index) {
            Some(x) => &x.field_type,
            None => anyhow::bail!(format!("not found type on tuple #{}", index)),
        },
        _ => {
            anyhow::bail!(format!("type not is not ok,should be array,but:{:?}", ty))
        }
    };
    anyhow::Ok(r)
}

pub fn ty_remove_nullable(x: &DataType) -> DataType {
    match x {
        DataType::Nullable(x) => ty_remove_nullable(x.as_ref()),
        DataType::Table(column_defs) => DataType::Table(column_defs.as_ref().map(|columns| {
            columns
                .iter()
                .map(|x| ColumnDef {
                    data_type: ty_remove_nullable(&x.data_type),
                    ..Clone::clone(x)
                })
                .collect()
        })),

        DataType::NamedTable { name, columns } => DataType::NamedTable {
            name: name.clone(),
            columns: columns
                .iter()
                .map(|x| ColumnDef {
                    data_type: ty_remove_nullable(&x.data_type),
                    ..Clone::clone(x)
                })
                .collect(),
        },
        DataType::Character(..) => x.clone(),
        DataType::Char(..) => x.clone(),
        DataType::CharacterVarying(..) => x.clone(),
        DataType::CharVarying(..) => x.clone(),
        DataType::Varchar(..) => x.clone(),
        DataType::Nvarchar(..) => x.clone(),
        DataType::Uuid => x.clone(),
        DataType::CharacterLargeObject(_) => x.clone(),
        DataType::CharLargeObject(_) => x.clone(),
        DataType::Clob(_) => x.clone(),
        DataType::Binary(_) => x.clone(),
        DataType::Varbinary(..) => x.clone(),
        DataType::Blob(_) => x.clone(),
        DataType::TinyBlob => x.clone(),
        DataType::MediumBlob => x.clone(),
        DataType::LongBlob => x.clone(),
        DataType::Bytes(_) => x.clone(),
        DataType::Numeric(..) => x.clone(),
        DataType::Decimal(..) => x.clone(),
        DataType::DecimalUnsigned(..) => x.clone(),
        DataType::BigNumeric(..) => x.clone(),
        DataType::BigDecimal(..) => x.clone(),
        DataType::Dec(..) => x.clone(),
        DataType::DecUnsigned(..) => x.clone(),
        DataType::Float(..) => x.clone(),
        DataType::FloatUnsigned(..) => x.clone(),
        DataType::TinyInt(_) => x.clone(),
        DataType::TinyIntUnsigned(_) => x.clone(),
        DataType::UTinyInt => x.clone(),
        DataType::Int2(_) => x.clone(),
        DataType::Int2Unsigned(_) => x.clone(),
        DataType::SmallInt(_) => x.clone(),
        DataType::SmallIntUnsigned(_) => x.clone(),
        DataType::USmallInt => x.clone(),
        DataType::MediumInt(_) => x.clone(),
        DataType::MediumIntUnsigned(_) => x.clone(),
        DataType::Int(_) => x.clone(),
        DataType::Int4(_) => x.clone(),
        DataType::Int8(_) => x.clone(),
        DataType::Int16 => x.clone(),
        DataType::Int32 => x.clone(),
        DataType::Int64 => x.clone(),
        DataType::Int128 => x.clone(),
        DataType::Int256 => x.clone(),
        DataType::Integer(_) => x.clone(),
        DataType::IntUnsigned(_) => x.clone(),
        DataType::Int4Unsigned(_) => x.clone(),
        DataType::IntegerUnsigned(_) => x.clone(),
        DataType::HugeInt => x.clone(),
        DataType::UHugeInt => x.clone(),
        DataType::UInt8 => x.clone(),
        DataType::UInt16 => x.clone(),
        DataType::UInt32 => x.clone(),
        DataType::UInt64 => x.clone(),
        DataType::UInt128 => x.clone(),
        DataType::UInt256 => x.clone(),
        DataType::BigInt(_) => x.clone(),
        DataType::BigIntUnsigned(_) => x.clone(),
        DataType::UBigInt => x.clone(),
        DataType::Int8Unsigned(_) => x.clone(),
        DataType::Signed => x.clone(),
        DataType::SignedInteger => x.clone(),
        DataType::Unsigned => x.clone(),
        DataType::UnsignedInteger => x.clone(),
        DataType::Float4 => x.clone(),
        DataType::Float32 => x.clone(),
        DataType::Float64 => x.clone(),
        DataType::Real => x.clone(),
        DataType::RealUnsigned => x.clone(),
        DataType::Float8 => x.clone(),
        DataType::Double(..) => x.clone(),
        DataType::DoubleUnsigned(..) => x.clone(),
        DataType::DoublePrecision => x.clone(),
        DataType::DoublePrecisionUnsigned => x.clone(),
        DataType::Bool => x.clone(),
        DataType::Boolean => x.clone(),
        DataType::Date => x.clone(),
        DataType::Date32 => x.clone(),
        DataType::Time(..) => x.clone(),
        DataType::Datetime(_) => x.clone(),
        DataType::Datetime64(_, _) => x.clone(),
        DataType::Timestamp(..) => x.clone(),
        DataType::TimestampNtz(_) => x.clone(),
        DataType::Interval { .. } => x.clone(),
        DataType::JSON => x.clone(),
        DataType::JSONB => x.clone(),
        DataType::Regclass => x.clone(),
        DataType::Text => x.clone(),
        DataType::TinyText => x.clone(),
        DataType::MediumText => x.clone(),
        DataType::LongText => x.clone(),
        DataType::String(_) => x.clone(),
        DataType::FixedString(_) => x.clone(),
        DataType::Bytea => x.clone(),
        DataType::Bit(_) => x.clone(),
        DataType::BitVarying(_) => x.clone(),
        DataType::VarBit(_) => x.clone(),
        DataType::Custom(..) => x.clone(),
        DataType::Array(array_elem_type_def) => match array_elem_type_def {
            ArrayElemTypeDef::None => x.clone(),
            ArrayElemTypeDef::AngleBracket(data_type) => DataType::Array(
                ArrayElemTypeDef::AngleBracket(Box::new(ty_remove_nullable(data_type.as_ref()))),
            ),
            ArrayElemTypeDef::SquareBracket(data_type, x) => {
                DataType::Array(ArrayElemTypeDef::SquareBracket(
                    Box::new(ty_remove_nullable(data_type.as_ref())),
                    *x,
                ))
            }
            ArrayElemTypeDef::Parenthesis(data_type) => DataType::Array(
                ArrayElemTypeDef::Parenthesis(Box::new(ty_remove_nullable(data_type.as_ref()))),
            ),
            // ArrayElemTypeDef::Qualified(data_type, b) => {
            //     ArrayElemTypeDef::Qualified(Box::new(ty_remove_nullable(data_type.as_ref())), *b)
            // }
        },
        DataType::Map(data_type, data_type1) => DataType::Map(
            Box::new(ty_remove_nullable(data_type.as_ref())),
            Box::new(ty_remove_nullable(data_type1.as_ref())),
        ),
        DataType::Tuple(struct_fields) => DataType::Tuple(
            struct_fields
                .iter()
                .map(|x| StructField {
                    field_type: ty_remove_nullable(&x.field_type),
                    ..Clone::clone(x)
                })
                .collect(),
        ),
        DataType::Nested(column_defs) => DataType::Nested(
            column_defs
                .iter()
                .map(|x| ColumnDef {
                    data_type: ty_remove_nullable(&x.data_type),
                    ..Clone::clone(x)
                })
                .collect(),
        ),
        DataType::Enum(..) => x.clone(),
        DataType::Set(..) => x.clone(),
        DataType::Struct(struct_fields, struct_bracket_kind) => DataType::Struct(
            struct_fields
                .iter()
                .map(|x| StructField {
                    field_type: ty_remove_nullable(&x.field_type),
                    ..Clone::clone(x)
                })
                .collect(),
            *struct_bracket_kind,
        ),
        DataType::Union(union_fields) => DataType::Union(
            union_fields
                .iter()
                .map(|x| sqlparser::ast::UnionField {
                    field_name: x.field_name.clone(),
                    field_type: ty_remove_nullable(&x.field_type),
                })
                .collect(),
        ),
        DataType::LowCardinality(data_type) => {
            DataType::LowCardinality(Box::new(ty_remove_nullable(data_type.as_ref())))
        }
        DataType::Unspecified => x.clone(),
        DataType::Trigger => x.clone(),
        DataType::AnyType => x.clone(),
        DataType::GeometricType(..) => x.clone(),
        DataType::TsVector => x.clone(),
        DataType::TsQuery => x.clone(),
    }
}

pub fn convert_csv_delimiter(
    input_content: &str,
    old_delimiter: Option<char>,
    new_delimiter: Option<char>, // 1. Added the new delimiter parameter
) -> anyhow::Result<String> {
    use csv::{ReaderBuilder, WriterBuilder};

    // 2. Safely convert old_delimiter to a single u8 byte (defaults to b',')
    let old_delim_byte = match old_delimiter {
        Some(c) if c.is_ascii() => c as u8,
        _ => b',',
    };

    // 3. Safely convert new_delimiter to a single u8 byte (defaults to b',')
    let new_delim_byte = match new_delimiter {
        Some(c) if c.is_ascii() => c as u8,
        _ => b',',
    };
    if old_delim_byte == new_delim_byte {
        return anyhow::Ok(input_content.into());
    }

    // 4. Configure ReaderBuilder with the custom old delimiter
    let mut rdr = ReaderBuilder::new()
        .delimiter(old_delim_byte)
        .from_reader(input_content.as_bytes());

    // 5. Configure WriterBuilder with the custom new delimiter
    let mut wtr = WriterBuilder::new()
        .delimiter(new_delim_byte) // Dynamically passed here
        .from_writer(Vec::new());

    wtr.write_record(rdr.headers()?)?;

    // 6. Iterate through records and write them out
    for result in rdr.records() {
        let record = result?;
        wtr.write_record(&record)?;
    }

    // 7. Extract the underlying Vec<u8> buffer
    let inner_bytes = wtr.into_inner()?;

    // 8. Convert the byte stream safely back into a standard String
    let output_string = String::from_utf8(inner_bytes)?;
    Ok(output_string)
}

pub fn is_csv_file(f: &str) -> bool {
    let f = f.to_lowercase();
    f.ends_with(".csv") || f.ends_with(".tsv")
}

impl BackendKind {
    pub fn infer_excel(&self, path: &str) -> anyhow::Result<Vec<Field>> {
        let mut wb: calamine::Xlsx<std::io::BufReader<std::fs::File>> =
            calamine::open_workbook(path)?;
        let sheet_name = &wb.sheet_names()[0];
        let range = wb.worksheet_range(sheet_name)?;
        // collect headers

        let mut headers = vec![];
        for (row_idx, row) in range.rows().enumerate() {
            if row_idx == 0 {
                headers = row.iter().map(|x| x.to_string()).collect();
                continue;
            }
            if row_idx == 1 {
                // infer data from data
                let mut tys = Vec::new();
                for r in row.iter() {
                    let ty = self.infer_excel_data(r)?;
                    tys.push(ty);
                }

                return anyhow::Ok(
                    headers
                        .into_iter()
                        .zip(tys)
                        .map(|(field, ty)| Field::from_name_type(field, ty))
                        .collect(),
                );
            }
        }
        anyhow::Ok(Default::default())
    }

    pub fn infer_excel_data(&self, data: &calamine::Data) -> anyhow::Result<String> {
        //csv impl somehow we delegate the csv implementation
        self.map_csv_dtype(&match data {
            calamine::Data::Int(_) => csv_processor::Dtype::Integer,
            calamine::Data::Float(_) => csv_processor::Dtype::Float,
            calamine::Data::String(_) => csv_processor::Dtype::Str,
            calamine::Data::Bool(_) => csv_processor::Dtype::Boolean,
            calamine::Data::DateTime(..)
            | calamine::Data::DateTimeIso(_)
            | calamine::Data::DurationIso(_) => csv_processor::Dtype::DateTime,
            calamine::Data::Error(..) => csv_processor::Dtype::Str,
            calamine::Data::Empty => csv_processor::Dtype::Str,
        })
    }

    pub async fn infer_fields(
        self,
        path: &str,
        csv_delimiter: Option<char>,
    ) -> anyhow::Result<Vec<Field>> {
        let path = path.to_owned();
        tokio::task::spawn_blocking(move || {
            let path_lower = path.to_lowercase();

            if is_csv_file(&path_lower) {
                self.infer_csv(&path, csv_delimiter)
            } else if path_lower.ends_with(".json") {
                self.infer_json(&path)
            } else if path_lower.ends_with(".jsonl") {
                self.infer_jsonline(&path)
            } else if is_excel_file(&path_lower) {
                self.infer_excel(&path)
            } else {
                anyhow::bail!(format!("unsupported file type:{}", path))
            }
        })
        .await?
    }

    pub fn infer_csv(self, path: &str, csv_delimiter: Option<char>) -> anyhow::Result<Vec<Field>> {
        // convert the , to
        let f = std::fs::File::open(path)?;
        let reader = BufReader::new(f);
        let mut input_content = String::new();
        for line in reader.lines().take(100) {
            let line = line?;
            input_content.push_str(&line);
            input_content.push('\n');
        }
        let csv_str = convert_csv_delimiter(&input_content, csv_delimiter, Some(','))
            .map_err(anyhow::Error::msg)?;

        let df = csv_processor::DataFrame::from_strings(&csv_str)?;
        let mut ret = vec![];

        // Get column information directly from df.
        for (index, col) in df.columns().iter().enumerate() {
            let name = df.headers().get(index).cloned().unwrap_or_default();
            // Map based on the column data type.
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
            if samples.len() >= 10 {
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
                        if number.as_i64().is_some() {
                            "BIGINT".to_string()
                        } else if number.as_u64().is_some() {
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
                serde_json::Value::Array(_) => "JSON".to_string(), // JSON is supported in MySQL 5.7+.
                serde_json::Value::Object(_) => "JSON".to_string(),
            },

            BackendKind::PostGres(_) => match value {
                serde_json::Value::Null => "VARCHAR(255)".to_string(),
                serde_json::Value::Bool(_) => "BOOLEAN".to_string(),
                serde_json::Value::Number(number) => {
                    if number.is_i64() || number.is_u64() {
                        if number.as_i64().is_some() || number.as_u64().is_some() {
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
                        "BIGINT".to_string()
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
                            if let Some(v) = x.first() {
                                format!("list<{}>", handle_value(v))
                            } else {
                                "list<text>".into()
                            }
                        }
                        serde_json::Value::Object(x) => {
                            if let Some((k, v)) = x.iter().next() {
                                format!(
                                    "map<{}, {}>",
                                    if k.parse::<u64>().is_ok() {
                                        "bigint"
                                    } else if k.parse::<f64>().is_ok() {
                                        "double"
                                    } else {
                                        "String"
                                    },
                                    handle_value(v)
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
                            if number.as_i64().is_some() {
                                "Int64".to_string()
                            } else if number.as_u64().is_some() {
                                "UInt64".to_string()
                            } else {
                                "Float64".to_string()
                            }
                        }
                        serde_json::Value::String(..) => "String".to_string(),
                        serde_json::Value::Array(x) => {
                            if let Some(x) = x.first() {
                                format!("Array({})", handle_value(x))
                            } else {
                                "Array(String)".into()
                            }
                        }
                        serde_json::Value::Object(x) => {
                            if let Some((k, v)) = x.iter().next() {
                                format!(
                                    "Map({},{})",
                                    if k.parse::<u64>().is_ok() {
                                        "UInt64"
                                    } else if k.parse::<i64>().is_ok() {
                                        "Int64"
                                    } else if k.parse::<f64>().is_ok() {
                                        "Float64"
                                    } else {
                                        "String"
                                    },
                                    handle_value(v)
                                )
                            } else {
                                "Map(Int32,Int32)".into()
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
                        if number.as_i64().is_some() || number.as_u64().is_some() {
                            "BIGINT".to_string()
                        } else {
                            "DOUBLE PRECISION".to_string()
                        }
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
                        "NUMBER".to_string()
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
                serde_json::Value::Number(..) => "number".to_string(),
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

/// Check whether the file extension indicates an Excel file.
/// Supported: .xlsx, .xls, .xlsb, .xlsm, .ods
pub fn is_excel_file(filename: &str) -> bool {
    let excel_extensions = ["xlsx", "xls", "xlsb", "xlsm", "ods"];

    filename
        .rsplit('.')
        .next()
        .map(|ext| excel_extensions.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

impl BackendKind {
    /// Core responsibility separation: return the most suitable column type name for the current database, based on the type inferred from the CSV.
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
                Dtype::Null => "VARCHAR(255)".to_string(), // Fall back to text by default.
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
                Dtype::Boolean => "INTEGER".to_string(), // SQLite has no native boolean type; INTEGER is commonly used (storing 0 or 1).
                Dtype::Date => "DATE".to_string(), // Fix: use DATE directly to align types across ORMs and drivers.
                Dtype::DateTime => "DATETIME".to_string(), // Fix: use DATETIME directly.
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
                Dtype::Boolean => "BIT".to_string(), // SQL Server uses BIT for booleans.
                Dtype::Date => "DATE".to_string(),
                Dtype::DateTime => "DATETIME2".to_string(),
                Dtype::Str | Dtype::Null => "NVARCHAR(MAX)".to_string(),
            },
            BackendKind::Oracle => match csv_dtype {
                Dtype::Integer => "NUMBER(19)".to_string(),
                Dtype::Float => "NUMBER".to_string(),
                Dtype::Boolean => "NUMBER(1)".to_string(), // Oracle traditionally has no dedicated boolean type.
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
            // NoSQL or key-value databases usually map everything to basic text or a specific BSON type.
            BackendKind::Mongo => return Err(err),
            BackendKind::CouchBase => return Err(err),
            BackendKind::Redis => return Err(err),
        };
        anyhow::Ok(r)
    }
}

pub struct DockerContainerDropper {
    pub id: String,
}

impl Drop for DockerContainerDropper {
    fn drop(&mut self) {
        // Use -f (force) to remove the container even if it is currently running
        let output = Command::new("docker").args(["rm", "-f", &self.id]).output();

        match output {
            Ok(out) => {
                if out.status.success() {
                    println!("Successfully removed Docker container: {}", self.id);
                } else {
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    eprintln!("Failed to remove Docker container {}: {}", self.id, stderr);
                }
            }
            Err(e) => {
                eprintln!("Failed to execute docker command for {}: {}", self.id, e);
            }
        }
    }
}

impl<T: Into<String>> From<T> for DockerContainerDropper {
    fn from(value: T) -> Self {
        Self { id: value.into() }
    }
}

#[cfg(test)]
pub fn cert_files_dir() -> String {
    let mut cert_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    cert_dir.push("test");
    cert_dir.push("cert");

    cert_dir.to_str().unwrap().to_owned()
}

/// Start an existing Docker container that is stopped.
#[cfg(test)]
pub fn start_docker_container(id: &str) -> anyhow::Result<()> {
    // Run the docker start <id> command.
    let output = Command::new("docker").args(["start", id]).output()?;

    // Check whether the command succeeded.
    if output.status.success() {
        println!("Successfully started Docker container: {}", id);
        Ok(())
    } else {
        // If it fails, capture stderr and return it wrapped as an anyhow error.
        let stderr = String::from_utf8(output.stderr.clone())?;
        Err(anyhow::Error::msg(stderr))
    }
}

#[cfg(test)]
mod tests {
    use calamine::{open_workbook, Reader};

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

    #[ignore = "the file may not exists"]
    #[test]

    fn test_infer_schema_csv() {
        let x = BackendKind::Sqlite
            .infer_csv(
                "/home/yuyang/Downloads/Persons_20260810T032730.csv",
                Some(';'),
            )
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

    #[ignore = "this file may not exist"]
    #[test]

    fn test_read_xlsx() {
        let mut wb: calamine::Xlsx<std::io::BufReader<std::fs::File>> =
            open_workbook("/home/yuyang/Downloads/test.xlsx").unwrap();

        let sheet_name = &wb.sheet_names()[0];

        let range = wb.worksheet_range(sheet_name).unwrap();

        for (row_idx, row) in range.rows().enumerate() {
            // Skip the header (assuming the first row is the header).
            if row_idx == 0 {
                // Print the header.
                let headers: Vec<String> = row.iter().map(|cell| cell.to_string()).collect();
                println!("表头: {:?}", headers);
                continue;
            }

            // Process the data row.
            let row_data: Vec<String> = row.iter().map(|cell| cell.to_string()).collect();
            println!("第 {} 行: {:?}", row_idx, row_data);

            // Or get a specific value by column index.
            if let Some(name) = row.first() {
                println!("姓名: {}", name);
            }
            if let Some(age) = row.get(1) {
                println!("年龄: {}", age);
            }
        }
    }

    #[ignore = "this is file maybe is missing"]
    #[test]
    fn test_read_tsv() {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_path("/home/yuyang/Downloads/Persons_20260810T002914.tsv")
            .unwrap();
        let headers: Vec<_> = reader
            .headers()
            .unwrap()
            .iter()
            .map(|x| x.to_string())
            .collect();
        println!("!!!!!!!!!!!!!:{:?}", headers);
        for csv_line in reader.records() {
            let csv_line = csv_line.unwrap();

            println!("!!!!!!!!{:?}", csv_line);
        }
    }
}
