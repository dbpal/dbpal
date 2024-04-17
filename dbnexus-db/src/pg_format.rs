use chrono::FixedOffset;
use sqlx::postgres::types::{PgInterval, PgMoney, PgRange, PgTimeTz};
use sqlx::postgres::{PgRow, PgValueRef};
use sqlx::types::BigDecimal;
use sqlx::{Row, ValueRef};

use crate::db::{format_data_time_fix_offset, Field};

/// right now sqlx not implement macaddr8
/// impl it myself.
pub struct MacAddr8 {
    addr: [u8; 8],
}

impl sqlx::Decode<'_, sqlx::Postgres> for MacAddr8 {
    fn decode(value: PgValueRef<'_>) -> Result<Self, sqlx::error::BoxDynError> {
        let bs = value.as_bytes()?;
        Ok(Self {
            addr: [bs[0], bs[1], bs[2], bs[3], bs[4], bs[5], bs[6], bs[7]],
        })
    }
}

impl sqlx::postgres::PgHasArrayType for MacAddr8 {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("MACADDR8")
    }
}

impl sqlx::Type<sqlx::Postgres> for MacAddr8 {
    fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("MACADDR8")
    }
}

impl std::fmt::Display for MacAddr8 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut addr = String::new();
        for index in 0..self.addr.len() {
            addr.push_str(hex::encode_upper(&self.addr[index..index + 1]).as_str());
            if index != self.addr.len() - 1 {
                addr.push(':');
            }
        }
        write!(f, "{}", addr)
    }
}

/// right now sqlx not implement macaddr8
/// impl it myself.
pub struct JsonPath {
    path: String,
}

impl sqlx::Decode<'_, sqlx::Postgres> for JsonPath {
    fn decode(value: PgValueRef<'_>) -> Result<Self, sqlx::error::BoxDynError> {
        let mut v = value.as_bytes().unwrap().to_vec();
        if v.len() > 0 && v[0] == 1 {
            // TODO 为啥会出现这个字符 我现在把他去掉了。
            //  SOH（Start of
            // Heading）字符是ASCII字符集中的控制字符，其对应的十进制值为1。
            // SOH字符在传统的通信协议中用作起始标记，
            // 用于表示通信的起始或者标识一条消息的开始。
            // 在早期的计算机通信中，SOH字符常被用于与外部设备进行数据通信，
            // 比如打印机或终端。当发送数据时，SOH字符可用于指示开始传输数据。类似地，
            // 接收方可以通过检测到SOH字符来识别一条消息的开始。
            // 然而，随着通信技术的发展，
            // SOH字符的使用已经在许多现代协议中被取代或者不再使用。在常见的通信协议（如TCP/
            // IP）中，SOH字符没有特殊的含义，它只是一个具有特定编码的字符而已。
            v = (&v[1..]).to_vec();
        }

        match String::from_utf8(v) {
            Ok(s) => Ok(Self { path: s }),
            Err(err) => Err(Box::new(err)),
        }
    }
}

impl sqlx::postgres::PgHasArrayType for JsonPath {
    fn array_type_info() -> sqlx::postgres::PgTypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("JSONPATH")
    }
}

impl sqlx::Type<sqlx::Postgres> for JsonPath {
    fn type_info() -> <sqlx::Postgres as sqlx::Database>::TypeInfo {
        sqlx::postgres::PgTypeInfo::with_name("JSONPATH")
    }
}
impl std::fmt::Display for JsonPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.path)
    }
}

/// encode rows to `Vec<Vec<String>>`
pub(crate) fn encode_rows(rows: &Vec<PgRow>) -> anyhow::Result<Vec<Vec<Option<String>>>> {
    let mut ret = Vec::with_capacity(rows.len());
    for r in rows.iter() {
        let mut values = Vec::with_capacity(r.columns().len());
        for c in 0..r.len() {
            let col = r.column(c);
            let ty = (col as &dyn sqlx::Column<Database = sqlx::postgres::Postgres>)
                .type_info()
                .to_string();
            let value = r.try_get_raw(c)?;
            let default_str = if value.is_null() {
                None
            } else {
                Some(format!(
                    "pg object[ty:{} data:{}]",
                    ty,
                    output_bytes(value.as_bytes().unwrap())
                ))
            };

            let v = match ty.as_str() {
                "BOOL" => {
                    let value: Option<bool> = r.get(c);
                    value.map(|x| x.to_string())
                }
                "BYTEA" => {
                    let value: Option<Vec<u8>> = r.get(c);
                    value.map(|x| output_bytes(&x[..]))
                }

                "NAME" => {
                    let value: Option<String> = r.get(c);
                    value
                }
                "INT8" => {
                    let value: Option<i64> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "INT2" => {
                    let value: Option<i16> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "INT4" => {
                    let value: Option<i32> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "TEXT" => {
                    let value: Option<String> = r.get(c);
                    value
                }
                "OID" => {
                    let value: Option<sqlx::postgres::types::Oid> = r.get(c);
                    value.map(|x| format!("{}", x.0))
                }

                "FLOAT4" => {
                    let value: Option<f32> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "FLOAT8" => {
                    let value: Option<f64> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "MACADDR8" => {
                    let value: Option<MacAddr8> = r.get_unchecked(c);
                    value.map(|x| format!("{}", x))
                }
                "MACADDR8[]" => {
                    let value: Option<Vec<MacAddr8>> = r.get_unchecked(c);
                    value.map(|x| output_macaddr8_array(&x))
                }

                "MACADDR" => {
                    let value: Option<mac_address::MacAddress> = r.get(c);
                    value.map(|x| x.to_string())
                }

                "INET" => {
                    let value: Option<ipnetwork::IpNetwork> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "BOOL[]" => {
                    let value: Option<Vec<bool>> = r.get(c);
                    value.map(|x| output_array(&x))
                }
                "BYTEA[]" => {
                    let value: Option<Vec<Vec<u8>>> = r.get(c);
                    value.map(|x| output_bytes_array(&x))
                }
                "NAME[]" => {
                    let value: Option<Vec<String>> = r.get(c);
                    value.map(|x| output_array(&x))
                }
                "INT2[]" => {
                    let value: Option<Vec<i16>> = r.get(c);
                    value.map(|x| output_array(&x))
                }
                "INT4[]" => {
                    let value: Option<Vec<i32>> = r.get(c);
                    value.map(|x| output_array(&x))
                }
                "TEXT[]" => {
                    let value: Option<Vec<String>> = r.get(c);
                    value.map(|x| output_array(&x))
                }
                "CHAR[]" | "BPCHAR[]" => {
                    let value: Option<Vec<String>> = r.get(c);
                    value.map(|x| output_array(&x))
                }
                "VARCHAR[]" => {
                    let value: Option<Vec<String>> = r.get(c);
                    value.map(|x| output_array(&x))
                }
                "INT8[]" => {
                    let value: Option<Vec<i64>> = r.get(c);
                    value.map(|x| output_array(&x))
                }
                "POINT[]" => default_str,
                "LSEG[]" => default_str,
                "PATH" => default_str,
                "PATH[]" => default_str,
                "BOX[]" => default_str,
                "FLOAT4[]" => {
                    let value: Option<Vec<f32>> = r.get(c);
                    value.map(|x| output_array(&x))
                }
                "FLOAT8[]" => {
                    let value: Option<Vec<f64>> = r.get(c);
                    value.map(|x| output_array(&x))
                }
                "POLYGON[]" => default_str,
                "OID[]" => {
                    let value: Option<Vec<sqlx::postgres::types::Oid>> = r.get(c);
                    value
                        .map(|x| x.into_iter().map(|x| x.0).collect::<Vec<_>>())
                        .map(|x| output_array(&x))
                }
                "MACADDR[]" => {
                    let value: Option<Vec<mac_address::MacAddress>> = r.get(c);
                    value.map(|x| output_array(&x))
                }
                "INET[]" => {
                    let value: Option<Vec<ipnetwork::IpNetwork>> = r.get(c);
                    value.map(|x| output_array(&x))
                }
                "CHAR" => {
                    let value: Option<String> = r.get(c);
                    value
                }
                "VARCHAR" => {
                    let value: Option<String> = r.get(c);
                    value
                }
                "DATE" => {
                    let value: Option<chrono::NaiveDate> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "TIME" | "TIME WITHOUT TIME ZONE" => {
                    let value: Option<chrono::NaiveTime> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "TIMESTAMP" | "TIMESTAMP WITHOUT TIME ZONE" => {
                    let value: Option<chrono::NaiveDateTime> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "TIMESTAMP[]" => {
                    let value: Option<Vec<chrono::NaiveDateTime>> = r.get(c);
                    value.map(|x| output_naive_datetime_array(&x))
                }
                "DATE[]" => {
                    let value: Option<Vec<chrono::NaiveDate>> = r.get(c);
                    value.map(|x| output_naive_date_array(&x))
                }
                "TIME[]" => {
                    let value: Option<Vec<chrono::NaiveTime>> = r.get(c);
                    value.map(|x| output_naive_time_array(&x))
                }
                "TIMESTAMPTZ" | "TIMESTAMP WITH TIME ZONE" => {
                    let value: Option<chrono::DateTime<FixedOffset>> = r.get(c);
                    value.map(|x| format_data_time_fix_offset(&x))
                }

                "TIMESTAMPTZ[]" => {
                    let value: Option<Vec<chrono::DateTime<FixedOffset>>> = r.get(c);
                    value.map(|x| output_date_time_fixoffset_array(&x))
                }
                "INTERVAL" => {
                    let value: Option<sqlx::postgres::types::PgInterval> = r.get(c);
                    value.map(|x| output_pg_interval(&x))
                }

                "INTERVAL[]" => {
                    let value: Option<Vec<PgInterval>> = r.get(c);
                    value.map(|x| output_pg_interval_array(&x))
                }
                "NUMERIC" => {
                    let value: Option<BigDecimal> = r.get(c);
                    value.map(|x| format!("{}", x))
                }
                "NUMERIC[]" => {
                    let value: Option<Vec<BigDecimal>> = r.get(c);
                    value.map(|x| output_array(&x))
                }

                "TIMETZ" | "TIME WITH TIME ZONE" => {
                    let value: Option<PgTimeTz> = r.get(c);
                    value.map(|x| output_pg_timez(&x))
                }
                "TIMETZ[]" => {
                    let value: Option<Vec<PgTimeTz>> = r.get(c);
                    value.map(|x| output_pg_timez_array(&x))
                }
                "BIT" => {
                    let value: Option<bit_vec::BitVec> = r.get(c);
                    value.map(|x| output_bit_vec(&x))
                }
                "BIT[]" => {
                    let value: Option<Vec<bit_vec::BitVec>> = r.get(c);
                    value.map(|x| output_bit_vec_array(&x))
                }

                "VARBIT" | "BIT VARYING" => {
                    let value: Option<bit_vec::BitVec> = r.get(c);
                    value.map(|x| output_bit_vec(&x))
                }
                "VARBIT[]" => {
                    let value: Option<Vec<bit_vec::BitVec>> = r.get(c);
                    value.map(|x| output_bit_vec_array(&x))
                }
                "RECORD" => default_str,
                "RECORD[]" => default_str,
                "UUID" => {
                    let value: Option<uuid::Uuid> = r.get(c);
                    value.map(|x| format!("{}", x))
                }

                "UUID[]" => {
                    let value: Option<Vec<uuid::Uuid>> = r.get(c);
                    value.map(|x| output_array(&x))
                }

                "JSON" => {
                    let value: Option<serde_json::Value> = r.get(c);
                    value.map(|x| x.to_string())
                }
                "JSONB" => {
                    let value: Option<serde_json::Value> = r.get(c);
                    value.map(|x| x.to_string())
                }
                "JSONB[]" => {
                    let value: Option<Vec<serde_json::Value>> = r.get(c);
                    value.map(|x| output_array(&x))
                }
                "INT4RANGE" => {
                    let value: Option<PgRange<i32>> = r.get(c);
                    value.map(|x| output_range(&x))
                }
                "INT4RANGE[]" => {
                    let value: Option<Vec<PgRange<i32>>> = r.get(c);
                    value.map(|x| output_range_array(&x))
                }
                "NUMRANGE" => {
                    let value: Option<PgRange<BigDecimal>> = r.get(c);
                    value.map(|x| output_range(&x))
                }
                "NUMRANGE[]" => {
                    let value: Option<Vec<PgRange<BigDecimal>>> = r.get(c);
                    value.map(|x| output_range_array(&x))
                }
                "TSRANGE" => {
                    let value: Option<PgRange<chrono::NaiveDateTime>> = r.get(c);
                    value.map(|x| output_range(&x))
                }
                "TSRANGE[]" => {
                    let value: Option<Vec<PgRange<chrono::NaiveDateTime>>> = r.get(c);
                    value.map(|x| output_range_array(&x))
                }

                "TSTZRANGE" => {
                    let value: Option<PgRange<chrono::DateTime<FixedOffset>>> = r.get(c);
                    value.map(|x| output_range_data_time_fixoffset(&x))
                }
                "TSTZRANGE[]" => {
                    let value: Option<Vec<PgRange<chrono::DateTime<FixedOffset>>>> = r.get(c);
                    value.map(|x| output_range_data_time_fix_offset_array(&x))
                }
                "DATERANGE" => {
                    let value: Option<PgRange<chrono::NaiveDate>> = r.get(c);
                    value.map(|x| output_range(&x))
                }
                "DATERANGE[]" => {
                    let value: Option<Vec<PgRange<chrono::NaiveDate>>> = r.get(c);
                    value.map(|x| output_range_array(&x))
                }
                "INT8RANGE" => {
                    let value: Option<PgRange<i64>> = r.get(c);
                    value.map(|x| output_range(&x))
                }
                "INT8RANGE[]" => {
                    let value: Option<Vec<PgRange<i64>>> = r.get(c);
                    value.map(|x| output_range_array(&x))
                }

                "JSONPATH" => {
                    let value: Option<JsonPath> = r.get_unchecked(c);
                    value.map(|x| format!("{}", x))
                }
                "JSONPATH[]" => {
                    let value: Option<Vec<JsonPath>> = r.get_unchecked(c);
                    value.map(|x| output_array(&x))
                }

                "MONEY" => {
                    let value: Option<PgMoney> = r.get(c);
                    value.map(|x| format!("{}", PgMoneyFmt::from(x)))
                }

                "MONEY[]" => {
                    let value: Option<Vec<PgMoney>> = r.get(c);
                    value.map(|x| output_pg_money_array(&x))
                }
                "VOID" => {
                    // TODO it this right.
                    None
                }
                _ => default_str,
            };
            values.push(v);
        }
        ret.push(values);
    }
    anyhow::Ok(ret)
}

pub(crate) fn put_value_in_sql(f: &Field, value: Option<&str>) -> anyhow::Result<String> {
    let default_str = value.map(|x| x.to_string()).unwrap_or("null".to_string());
    let single_quote_str = value
        .map(|x| format!("'{}'", x))
        .unwrap_or("null".to_string());
    let err = Err(anyhow::Error::msg(format!(
        "type '{}' not supported",
        f.typ
    ))) as anyhow::Result<String>;

    let encode = match f.typ.to_uppercase().as_str() {
        "BOOL" => default_str,
        "BYTEA" => default_str,

        "NAME" => single_quote_str,

        "INT8" | "BIGINT" => default_str,
        "INT2" | "SMALLINT" => default_str,
        "INT4" | "INTEGER" => default_str,
        "TEXT" => single_quote_str,

        "CHAR" | "CHARACTER" => single_quote_str,
        "OID" => default_str,
        "JSON" => single_quote_str,
        "JSON[]" => return err,
        "POINT" => return err,
        "LSEG" => return err,
        "PATH" => single_quote_str,
        "BOX" => return err,
        "POLYGON" => return err,
        "LINE" => return err,
        "LINE[]" => return err,
        "CIDR" => return err,
        "CIDR[]" => return err,
        "FLOAT4" => return err,
        "FLOAT8" => return err,
        "UNKNOWN" => return err,
        "CIRCLE" => return err,
        "CIRCLE[]" => return err,
        "MACADDR8" => single_quote_str,
        "MACADDR8[]" => single_quote_str,
        "MACADDR" => single_quote_str,
        "INET" => single_quote_str,
        "BOOL[]" => single_quote_str,
        "BYTEA[]" => single_quote_str,

        "NAME[]" => single_quote_str,
        "INT2[]" => single_quote_str,
        "INT4[]" => single_quote_str,
        "TEXT[]" => single_quote_str,
        "CHAR[]" | "BPCHAR[]" => single_quote_str,
        "VARCHAR[]" => single_quote_str,
        "INT8[]" => single_quote_str,
        "POINT[]" => single_quote_str,
        "LSEG[]" => single_quote_str,
        "PATH[]" => single_quote_str,
        "BOX[]" => single_quote_str,
        "FLOAT4[]" => single_quote_str,
        "FLOAT8[]" => single_quote_str,
        "POLYGON[]" => single_quote_str,
        "OID[]" => single_quote_str,
        "MACADDR[]" => single_quote_str,
        "INET[]" => single_quote_str,

        "VARCHAR" => single_quote_str,
        "DATE" => single_quote_str,
        "TIME" | "TIME WITHOUT TIME ZONE" => single_quote_str,
        "TIMESTAMP" | "TIMESTAMP WITHOUT TIME ZONE" => single_quote_str,
        "TIMESTAMP[]" => single_quote_str,
        "DATE[]" => single_quote_str,
        "TIME[]" => single_quote_str,
        "TIMESTAMPTZ" | "TIMESTAMP WITH TIME ZONE" => single_quote_str,
        "TIMESTAMPTZ[]" => single_quote_str,
        "INTERVAL" => single_quote_str,
        "INTERVAL[]" => single_quote_str,
        "NUMERIC[]" => single_quote_str,
        "TIMETZ" | "TIME WITH TIME ZONE" => single_quote_str,
        "TIMETZ[]" => single_quote_str,
        "BIT" => {
            value
                .map(|x| format!("B'{x}'"))
                .unwrap_or("null".to_string())
        }

        "BIT[]" => single_quote_str,
        "VARBIT" | "BIT VARYING" => {
            value
                .map(|x| format!("B'{x}'"))
                .unwrap_or("null".to_string())
        }
        "VARBIT[]" => single_quote_str,
        "NUMERIC" => default_str,
        "RECORD" => default_str,
        "RECORD[]" => single_quote_str,
        "UUID" => single_quote_str,
        "UUID[]" => single_quote_str,
        "JSONB" => single_quote_str,
        "JSONB[]" => single_quote_str,
        "INT4RANGE" => single_quote_str,
        "INT4RANGE[]" => single_quote_str,
        "NUMRANGE" => single_quote_str,
        "NUMRANGE[]" => single_quote_str,
        "TSRANGE" => single_quote_str,
        "TSRANGE[]" => single_quote_str,
        "TSTZRANGE" => single_quote_str,
        "TSTZRANGE[]" => single_quote_str,
        "DATERANGE" => single_quote_str,
        "DATERANGE[]" => single_quote_str,
        "INT8RANGE" => single_quote_str,
        "INT8RANGE[]" => single_quote_str,
        "JSONPATH" => single_quote_str,
        "JSONPATH[]" => single_quote_str,
        "MONEY" => single_quote_str,
        "MONEY[]" => single_quote_str,
        _ => default_str,
    };
    log::trace!("type: '{}' encode '{:?}' as #{}#", f.typ, value, encode);
    anyhow::Ok(encode)
}

pub(crate) struct PgMoneyFmt {
    money: PgMoney,
}

impl std::fmt::Display for PgMoneyFmt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let m = self.money.0;
        write!(f, "${}.{}", m / 100, m % 100)
    }
}

impl From<PgMoney> for PgMoneyFmt {
    fn from(value: PgMoney) -> Self {
        PgMoneyFmt { money: value }
    }
}

pub(crate) fn output_pg_money_array(array: &Vec<PgMoney>) -> String {
    if array.len() == 0 {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("{}", PgMoneyFmt::from(v.clone())).as_str());
        if index != last {
            ret.push_str(",")
        }
    }
    ret.push_str("}");
    ret
}

pub(crate) fn output_range<T: std::fmt::Display>(r: &PgRange<T>) -> String {
    use std::ops::Bound::*;
    let mut ret = "".to_string();
    match &r.start {
        Included(x) => ret.push_str(format!("[{},", x).as_str()),
        Excluded(x) => ret.push_str(format!("({},", x).as_str()),
        Unbounded => ret.push_str("(,"),
    };
    match &r.end {
        Included(x) => ret.push_str(format!("{}]", x).as_str()),
        Excluded(x) => ret.push_str(format!("{})", x).as_str()),
        Unbounded => ret.push_str(")"),
    }
    ret
}

pub(crate) fn output_range_data_time_fixoffset(
    r: &PgRange<chrono::DateTime<FixedOffset>>,
) -> String {
    use std::ops::Bound::*;
    let mut ret = "".to_string();
    match &r.start {
        Included(x) => ret.push_str(format!("[{},", format_data_time_fix_offset(x)).as_str()),
        Excluded(x) => ret.push_str(format!("({},", format_data_time_fix_offset(x)).as_str()),
        Unbounded => ret.push_str("(,"),
    };
    match &r.end {
        Included(x) => ret.push_str(format!("{}]", format_data_time_fix_offset(x)).as_str()),
        Excluded(x) => ret.push_str(format!("{})", format_data_time_fix_offset(x)).as_str()),
        Unbounded => ret.push_str(")"),
    }
    ret
}

pub(crate) fn output_range_data_time_fix_offset_array(
    array: &Vec<PgRange<chrono::DateTime<FixedOffset>>>,
) -> String {
    if array.len() == 0 {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("\"{}\"", output_range_data_time_fixoffset(v)).as_str());
        if index != last {
            ret.push_str(",")
        }
    }
    ret.push_str("}");
    ret
}

#[cfg(test)]
#[test]
pub(crate) fn test_output_int_range() {
    use std::ops::Bound::*;
    {
        let r = PgRange::from(1..2);
        let s = output_range(&r);
        assert_eq!("[1,2)", s);
    }
    {
        let r = PgRange::from([Unbounded, Included(10)]);
        let s = output_range(&r);
        assert_eq!("(,10]", s);
    }
    {
        let r = PgRange::from([Unbounded, Excluded(10)]);
        let s = output_range(&r);
        assert_eq!("(,10)", s);
    }
    {
        let r = PgRange::from([Included(2), Included(10)]);
        let s = output_range(&r);
        assert_eq!("[2,10]", s);
    }
    {
        let r = PgRange::from([Excluded(2), Excluded(10)]);
        let s = output_range(&r);
        assert_eq!("(2,10)", s);
    }
    {
        let r = PgRange::from([Excluded(2), Unbounded]);
        let s = output_range(&r);
        assert_eq!("(2,)", s);
    }
}

pub(crate) fn output_range_array<T: std::fmt::Display>(array: &Vec<PgRange<T>>) -> String {
    if array.len() == 0 {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("\"{}\"", output_range(v)).as_str());
        if index != last {
            ret.push_str(",")
        }
    }
    ret.push_str("}");
    ret
}

pub(crate) fn output_bit_vec(v: &bit_vec::BitVec) -> String {
    let mut ret = String::new();
    for b in v.clone().into_iter() {
        if b {
            ret.push('1');
        } else {
            ret.push('0');
        }
    }
    ret
}

pub(crate) fn output_bit_vec_array(array: &Vec<bit_vec::BitVec>) -> String {
    if array.len() == 0 {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("'{}'", output_bit_vec(v)).as_str());
        if index != last {
            ret.push_str(",")
        }
    }

    ret.push_str("}");
    ret
}
pub(crate) fn output_pg_timez(v: &PgTimeTz) -> String {
    format!("{}{}", v.time, v.offset)
}

pub(crate) fn output_pg_timez_array(array: &Vec<PgTimeTz>) -> String {
    if array.len() == 0 {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("'{}'", output_pg_timez(v)).as_str());
        if index != last {
            ret.push_str(",")
        }
    }
    ret.push_str("}");
    ret
}

pub(crate) fn output_pg_interval(v: &PgInterval) -> String {
    // 暂时还没实现 只是能过测试
    format!("{:?}", v)
}

pub(crate) fn output_pg_interval_array(array: &Vec<PgInterval>) -> String {
    if array.len() == 0 {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("'{}'", output_pg_interval(v)).as_str());
        if index != last {
            ret.push_str(",")
        }
    }
    ret.push_str("}");
    ret
}

pub(crate) fn output_date_time_fixoffset_array(
    array: &Vec<chrono::DateTime<FixedOffset>>,
) -> String {
    if array.len() == 0 {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("\"{}\"", format_data_time_fix_offset(v)).as_str());
        if index != last {
            ret.push_str(",")
        }
    }
    ret.push_str("}");
    ret
}

pub(crate) fn output_naive_time_array(array: &Vec<chrono::NaiveTime>) -> String {
    if array.len() == 0 {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("{}", v).as_str());
        if index != last {
            ret.push_str(",")
        }
    }
    ret.push_str("}");
    ret
}
pub(crate) fn output_naive_date_array(array: &Vec<chrono::NaiveDate>) -> String {
    if array.len() == 0 {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("{}", v).as_str());
        if index != last {
            ret.push_str(",")
        }
    }
    ret.push_str("}");
    ret
}

pub(crate) fn output_naive_datetime_array(array: &Vec<chrono::NaiveDateTime>) -> String {
    if array.len() == 0 {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("\"{}\"", v).as_str());
        if index != last {
            ret.push_str(",")
        }
    }
    ret.push_str("}");
    ret
}

pub(crate) fn output_array<T: std::fmt::Display>(array: &Vec<T>) -> String {
    if array.len() == 0 {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("{}", v).as_str());
        if index != last {
            ret.push_str(",")
        }
    }
    ret.push_str("}");
    ret
}
pub(crate) fn output_bytes_array(array: &Vec<Vec<u8>>) -> String {
    if array.len() == 0 {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(&output_bytes(v));
        if index != last {
            ret.push_str(",")
        }
    }
    ret.push_str("}");
    ret
}

pub(crate) fn output_bytes(bs: &[u8]) -> String {
    format!("0x{}", hex::encode_upper(bs))
}

pub(crate) fn output_macaddr8_array(array: &Vec<MacAddr8>) -> String {
    if array.len() == 0 {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("{}", v).as_str());
        if index != last {
            ret.push_str(",")
        }
    }
    ret.push_str("}");
    ret
}
