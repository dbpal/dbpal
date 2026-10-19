use chrono::FixedOffset;
use sqlx::postgres::types::{PgInterval, PgMoney, PgRange, PgTimeTz};
use sqlx::postgres::{PgHasArrayType, PgRow, PgTypeInfo, PgValueRef};
use sqlx::types::BigDecimal;
use sqlx::{Column, Decode, Postgres, Row, Type, ValueRef};

use crate::db::{format_data_time_fix_offset, quote_sql_str, EncodeXlsxDatetime, XlsxEditContent};
use crate::pg::PostGresDB;

// 1. Create a new wrapper type.
#[derive(Debug, Clone)]
pub struct RegProcString(pub String);

// 2. Tell sqlx that this type corresponds to Postgres regproc.
impl Type<Postgres> for RegProcString {
    fn type_info() -> PgTypeInfo {
        // Key: bind the Postgres regproc type.
        PgTypeInfo::with_name("regproc")
    }
}

// 3. Implement decoding: when reading from the database, automatically convert the OID numeric identifier to a function-name string.
impl<'r> Decode<'r, Postgres> for RegProcString {
    fn decode(value: PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        // Try to extract it as text (the Postgres driver converts it automatically).
        let s = <String as Decode<Postgres>>::decode(value)?;
        Ok(RegProcString(s))
    }
}

// If you later need to support arrays of this type (such as regproc[]), you can also add this line:
impl PgHasArrayType for RegProcString {
    fn array_type_info() -> PgTypeInfo {
        PgTypeInfo::with_name("_regproc")
    }
}

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

pub(crate) fn encode_row_as_json(
    r: &PgRow,
) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    let mut values = serde_json::Map::new();

    for c in 0..r.columns().len() {
        let col_info = r.column(c);

        let err = Err(anyhow::Error::msg(format!(
            "type '{}' not supported",
            col_info.type_info(),
        ))) as anyhow::Result<serde_json::Map<String, serde_json::Value>>;

        let ty = col_info.type_info().to_string().to_uppercase();
        let v = match ty.as_str() {
            "REGPROC" => {
                let value: Option<RegProcString> = r.try_get(c)?;
                value.map(|x| serde_json::Value::from(x.0))
            }
            "REGPROC[]" => {
                let value: Option<Vec<RegProcString>> = r.try_get(c)?;
                value.map(|x| {
                    serde_json::Value::Array(
                        x.into_iter()
                            .map(|x| serde_json::Value::from(x.0))
                            .collect(),
                    )
                })
            }

            "BOOL" => {
                let value: Option<bool> = r.try_get(c)?;
                value.map(serde_json::Value::from)
            }
            "BYTEA" => {
                let value: Option<Vec<u8>> = r.try_get(c)?;
                value
                    .map(|x| output_bytes(&x[..]))
                    .map(serde_json::Value::from)
            }

            "NAME" => {
                let value: Option<String> = r.try_get(c)?;
                value.map(serde_json::Value::from)
            }
            "INT8" => {
                let value: Option<i64> = r.try_get(c)?;
                value.map(serde_json::Value::from)
            }
            "INT2" => {
                let value: Option<i16> = r.try_get(c)?;
                value.map(serde_json::Value::from)
            }
            "INT4" => {
                let value: Option<i32> = r.try_get(c)?;
                value.map(serde_json::Value::from)
            }
            "TEXT" => {
                let value: Option<String> = r.try_get(c)?;
                value.map(serde_json::Value::from)
            }
            "OID" => {
                let value: Option<sqlx::postgres::types::Oid> = r.try_get(c)?;
                value.map(|x| x.0).map(serde_json::Value::from)
            }

            "FLOAT4" => {
                let value: Option<f32> = r.try_get(c)?;
                value.map(serde_json::Value::from)
            }
            "FLOAT8" => {
                let value: Option<f64> = r.try_get(c)?;
                value.map(serde_json::Value::from)
            }
            "MACADDR8" => {
                let value: Option<MacAddr8> = r.get_unchecked(c);
                value.map(|x| format!("{}", x)).map(serde_json::Value::from)
            }
            "MACADDR8[]" => {
                let value: Option<Vec<MacAddr8>> = r.get_unchecked(c);
                let value = value.map(|x| x.into_iter().map(|x| x.to_string()).collect::<Vec<_>>());

                value
                    .map(|x| {
                        x.into_iter()
                            .map(serde_json::Value::from)
                            .collect::<Vec<_>>()
                    })
                    .map(serde_json::Value::Array)
            }

            "MACADDR" => {
                let value: Option<mac_address::MacAddress> = r.try_get(c)?;
                value.map(|x| x.to_string()).map(serde_json::Value::from)
            }

            "INET" => {
                let value: Option<ipnetwork::IpNetwork> = r.try_get(c)?;
                value.map(|x| format!("{}", x)).map(serde_json::Value::from)
            }
            "BOOL[]" => {
                let value: Option<Vec<bool>> = r.try_get(c)?;

                value
                    .map(|x| {
                        x.into_iter()
                            .map(serde_json::Value::from)
                            .collect::<Vec<_>>()
                    })
                    .map(serde_json::Value::Array)
            }
            "BYTEA[]" => {
                let value: Option<Vec<Vec<u8>>> = r.try_get(c)?;
                let value = value.map(|data| output_bytes_array(&data));
                value.map(serde_json::Value::from)
            }
            "INT2[]" => {
                let value: Option<Vec<i16>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(serde_json::Value::from)
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }
            "INT4[]" => {
                let value: Option<Vec<i32>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(serde_json::Value::from)
                        .collect::<Vec<_>>()
                });
                value.map(serde_json::Value::Array)
            }
            _ if is_text_array_ty(&ty) => {
                let value: Option<Vec<String>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(serde_json::Value::from)
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }

            "INT8[]" => {
                let value: Option<Vec<i64>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(serde_json::Value::from)
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }
            "POINT[]" => {
                return err;
            }
            "LSEG[]" => {
                return err;
            }
            "PATH" => {
                return err;
            }
            "PATH[]" => {
                return err;
            }
            "BOX[]" => {
                return err;
            }
            "FLOAT4[]" => {
                let value: Option<Vec<f32>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(serde_json::Value::from)
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }
            "FLOAT8[]" => {
                let value: Option<Vec<f64>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(serde_json::Value::from)
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }
            "POLYGON[]" => {
                return err;
            }
            "OID[]" => {
                let value: Option<Vec<sqlx::postgres::types::Oid>> = r.try_get(c)?;

                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| serde_json::Value::from(x.0))
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }
            "MACADDR[]" => {
                let value: Option<Vec<mac_address::MacAddress>> = r.try_get(c)?;

                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| serde_json::Value::from(x.to_string()))
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }
            "INET[]" => {
                let value: Option<Vec<ipnetwork::IpNetwork>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| serde_json::Value::from(x.to_string()))
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }

            "CHAR" => {
                let value: Option<String> = r.try_get(c)?;
                value.map(serde_json::Value::from)
            }
            "\"CHAR\"" => {
                let value: Option<i8> = r.try_get(c)?;
                value
                    .map(|x| x as u8 as char)
                    .map(|x| x.to_string())
                    .map(serde_json::Value::from)
            }

            "VARCHAR" => {
                let value: Option<String> = r.try_get(c)?;
                value.map(serde_json::Value::from)
            }
            "DATE" => {
                let value: Option<chrono::NaiveDate> = r.try_get(c)?;
                value.map(|x| format!("{}", x)).map(serde_json::Value::from)
            }
            "TIME" | "TIME WITHOUT TIME ZONE" => {
                let value: Option<chrono::NaiveTime> = r.try_get(c)?;
                value.map(|x| format!("{}", x)).map(serde_json::Value::from)
            }
            "TIMESTAMP" | "TIMESTAMP WITHOUT TIME ZONE" => {
                let value: Option<chrono::NaiveDateTime> = r.try_get(c)?;
                value.map(|x| format!("{}", x)).map(serde_json::Value::from)
            }
            "TIMESTAMP[]" => {
                let value: Option<Vec<chrono::NaiveDateTime>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| serde_json::Value::from(x.to_string()))
                        .collect::<Vec<_>>()
                });
                value.map(serde_json::Value::Array)
            }
            "DATE[]" => {
                let value: Option<Vec<chrono::NaiveDate>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| serde_json::Value::from(x.to_string()))
                        .collect::<Vec<_>>()
                });
                value.map(serde_json::Value::Array)
            }
            "TIME[]" => {
                let value: Option<Vec<chrono::NaiveTime>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| serde_json::Value::from(x.to_string()))
                        .collect::<Vec<_>>()
                });
                value.map(serde_json::Value::Array)
            }
            "TIMESTAMPTZ" | "TIMESTAMP WITH TIME ZONE" => {
                let value: Option<chrono::DateTime<FixedOffset>> = r.try_get(c)?;
                value
                    .map(|x| format_data_time_fix_offset(&x))
                    .map(serde_json::Value::from)
            }

            "TIMESTAMPTZ[]" => {
                let value: Option<Vec<chrono::DateTime<FixedOffset>>> = r.try_get(c)?;

                // value.map(|x| output_date_time_fixoffset_array(&x))
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| format_data_time_fix_offset(&x))
                        .map(serde_json::Value::from)
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }
            "INTERVAL" => {
                let value: Option<sqlx::postgres::types::PgInterval> = r.try_get(c)?;
                value
                    .map(|x| output_pg_interval(&x))
                    .map(serde_json::Value::from)
            }

            "INTERVAL[]" => {
                let value: Option<Vec<PgInterval>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| serde_json::Value::from(output_pg_interval(&x)))
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }

            "NUMERIC" => {
                let value: Option<BigDecimal> = r.try_get(c)?;
                value.map(|x| format!("{}", x)).map(serde_json::Value::from)
            }
            "NUMERIC[]" => {
                let value: Option<Vec<BigDecimal>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| serde_json::Value::from(x.to_string()))
                        .collect::<Vec<_>>()
                });
                value.map(serde_json::Value::Array)
            }

            "TIMETZ" | "TIME WITH TIME ZONE" => {
                let value: Option<PgTimeTz> = r.try_get(c)?;
                value
                    .map(|x| output_pg_timez(&x))
                    .map(serde_json::Value::from)
            }
            "TIMETZ[]" => {
                let value: Option<Vec<PgTimeTz>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| serde_json::Value::from(output_pg_timez(&x)))
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }
            "BIT" => {
                let value: Option<sqlx::types::BitVec> = r.try_get(c)?;
                value
                    .map(|x| output_bit_vec(&x))
                    .map(serde_json::Value::from)
            }

            "BIT[]" => {
                let value: Option<Vec<sqlx::types::BitVec>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| serde_json::Value::from(output_bit_vec(&x)))
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }

            "VARBIT" | "BIT VARYING" => {
                let value: Option<sqlx::types::BitVec> = r.try_get(c)?;
                value
                    .map(|x| output_bit_vec(&x))
                    .map(serde_json::Value::from)
            }
            "VARBIT[]" => {
                let value: Option<Vec<sqlx::types::BitVec>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| serde_json::Value::from(output_bit_vec(&x)))
                        .collect::<Vec<_>>()
                });
                value.map(serde_json::Value::Array)
            }
            "RECORD" => {
                return err;
            }
            "RECORD[]" => {
                return err;
            }
            "UUID" => {
                let value: Option<uuid::Uuid> = r.try_get(c)?;
                value.map(|x| format!("{}", x)).map(serde_json::Value::from)
            }

            "UUID[]" => {
                let value: Option<Vec<uuid::Uuid>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| x.to_string())
                        .map(serde_json::Value::from)
                        .collect::<Vec<_>>()
                });
                value.map(serde_json::Value::Array)
            }

            "JSON" => {
                let value: Option<serde_json::Value> = r.try_get(c)?;
                value
            }
            "JSONB" => {
                let value: Option<serde_json::Value> = r.try_get(c)?;
                value
            }
            "JSONB[]" => {
                let value: Option<Vec<serde_json::Value>> = r.try_get(c)?;
                value.map(serde_json::Value::Array)
            }
            "INT4RANGE" => {
                let value: Option<PgRange<i32>> = r.try_get(c)?;
                value.map(|x| output_range(&x)).map(serde_json::Value::from)
            }
            "INT4RANGE[]" => {
                let value: Option<Vec<PgRange<i32>>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| output_range(&x))
                        .map(serde_json::Value::from)
                        .collect::<Vec<_>>()
                });
                value.map(serde_json::Value::Array)
            }
            "NUMRANGE" => {
                let value: Option<PgRange<BigDecimal>> = r.try_get(c)?;
                value.map(|x| output_range(&x)).map(serde_json::Value::from)
            }
            "NUMRANGE[]" => {
                let value: Option<Vec<PgRange<BigDecimal>>> = r.try_get(c)?;
                value
                    .map(|x| output_range_array(&x))
                    .map(serde_json::Value::from)
            }
            "TSRANGE" => {
                let value: Option<PgRange<chrono::NaiveDateTime>> = r.try_get(c)?;
                value
                    .map(|x| output_tsrange(&x))
                    .map(serde_json::Value::from)
            }
            "TSRANGE[]" => {
                let value: Option<Vec<PgRange<chrono::NaiveDateTime>>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| output_range(&x))
                        .map(serde_json::Value::from)
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }

            "TSTZRANGE" => {
                let value: Option<PgRange<chrono::DateTime<FixedOffset>>> = r.try_get(c)?;
                value
                    .map(|x| output_range_data_time_fixoffset(&x))
                    .map(serde_json::Value::from)
            }
            "TSTZRANGE[]" => {
                anyhow::bail!("TSTZRANGE[] not support");
            }

            "DATERANGE" => {
                let value: Option<PgRange<chrono::NaiveDate>> = r.try_get(c)?;
                value.map(|x| output_range(&x)).map(serde_json::Value::from)
            }
            "DATERANGE[]" => {
                let value: Option<Vec<PgRange<chrono::NaiveDate>>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| output_range(&x))
                        .map(serde_json::Value::from)
                        .collect::<Vec<_>>()
                });
                value.map(serde_json::value::Value::Array)
            }
            "INT8RANGE" => {
                let value: Option<PgRange<i64>> = r.try_get(c)?;
                value.map(|x| output_range(&x)).map(serde_json::Value::from)
            }
            "INT8RANGE[]" => {
                let value: Option<Vec<PgRange<i64>>> = r.try_get(c)?;
                let value =
                    value.map(|x| x.into_iter().map(|x| output_range(&x)).collect::<Vec<_>>());
                let value = value.map(|x| {
                    x.into_iter()
                        .map(serde_json::Value::from)
                        .collect::<Vec<_>>()
                });

                value.map(serde_json::Value::Array)
            }

            "JSONPATH" => {
                anyhow::bail!("JSONPATH not support");
            }
            "JSONPATH[]" => {
                anyhow::bail!("JSONPATH[] not support");
            }

            "MONEY" => {
                let value: Option<PgMoney> = r.try_get(c)?;
                value
                    .map(|x| format!("{}", PgMoneyFmt::from(x)))
                    .map(serde_json::Value::from)
            }

            "MONEY[]" => {
                let value: Option<Vec<PgMoney>> = r.try_get(c)?;
                let value = value.map(|x| {
                    x.into_iter()
                        .map(|x| serde_json::Value::from(PgMoneyFmt::from(x).to_string()))
                        .collect::<Vec<_>>()
                });
                value.map(serde_json::Value::Array)
            }
            "VOID" => {
                anyhow::bail!("void not support.")
            }
            _ => {
                return err;
            }
        };
        values.insert(
            col_info.name().to_string(),
            v.unwrap_or(serde_json::Value::Null),
        );
    }

    anyhow::Ok(values)
}

pub(crate) fn is_text_array_ty(ty: &str) -> bool {
    matches!(
        ty,
        "TEXT[]" | "CHAR[]" | "BPCHAR[]" | "VARCHAR[]" | "NAME[]"
    )
}

impl EncodeXlsxDatetime for PostGresDB {
    fn is_datatime(&self, ty: &str) -> anyhow::Result<bool> {
        anyhow::Ok(
            [
                "TIMESTAMP",
                "TIMESTAMP WITHOUT TIME ZONE",
                "TIMESTAMPTZ",
                "TIMESTAMP WITH TIME ZONE",
            ]
            .iter()
            .any(|t| t.eq_ignore_ascii_case(ty)),
        )
    }

    fn is_date(&self, ty: &str) -> anyhow::Result<bool> {
        anyhow::Ok(ty.eq_ignore_ascii_case("DATE"))
    }

    fn is_time(&self, ty: &str) -> anyhow::Result<bool> {
        anyhow::Ok(
            ["TIME", "TIME WITHOUT TIME ZONE"]
                .iter()
                .any(|t| t.eq_ignore_ascii_case(ty)),
        )
    }
}

/// encode rows to `Vec<Vec<String>>`
pub(crate) fn encode_row_as_xlsx(r: &PgRow) -> anyhow::Result<Vec<Option<XlsxEditContent>>> {
    let mut values = Vec::with_capacity(r.columns().len());
    for c in 0..r.len() {
        let col = r.column(c);
        let ty = col.type_info().to_string().to_uppercase();
        let value = r.try_get_raw(c)?;
        let default_str: Option<XlsxEditContent> = if value.is_null() {
            None
        } else {
            Some(
                format!(
                    "pg value[ty:{} data:{}]",
                    ty,
                    output_bytes(
                        value
                            .as_bytes()
                            .map_err(|e| anyhow::Error::msg(e.to_string()))?
                    )
                )
                .into(),
            )
        };

        let v: Option<XlsxEditContent> = match ty.as_str() {
            "REGPROC" => {
                let value: Option<RegProcString> = r.try_get(c)?;
                value.map(|x| x.0.into())
            }
            "REGPROC[]" => {
                let value: Option<Vec<RegProcString>> = r.try_get(c)?;
                if let Some(value) = value {
                    let x = value
                        .into_iter()
                        .map(|x| format!("\"{}\"", x.0))
                        .collect::<Vec<String>>();

                    Some(format!("{{{}}}", x.join(",")).into())
                } else {
                    None
                }
            }
            "BOOL" => {
                let value: Option<bool> = r.try_get(c)?;
                value.map(|x| x.into())
            }
            "BYTEA" => {
                let value: Option<Vec<u8>> = r.try_get(c)?;
                value.map(|x| output_bytes(&x[..]).into())
            }

            "NAME" => {
                let value: Option<String> = r.try_get(c)?;
                value.map(|x| x.into())
            }
            "INT8" => {
                let value: Option<i64> = r.try_get(c)?;
                value.map(|x| x.into())
            }
            "INT2" => {
                let value: Option<i16> = r.try_get(c)?;
                value.map(XlsxEditContent::number)
            }
            "INT4" => {
                let value: Option<i32> = r.try_get(c)?;
                value.map(XlsxEditContent::number)
            }
            "TEXT" => {
                let value: Option<String> = r.try_get(c)?;
                value.map(|x| x.into())
            }
            "OID" => {
                let value: Option<sqlx::postgres::types::Oid> = r.try_get(c)?;
                value.map(|x| XlsxEditContent::number(x.0))
            }

            "FLOAT4" => {
                let value: Option<f32> = r.try_get(c)?;
                value.map(XlsxEditContent::number)
            }
            "FLOAT8" => {
                let value: Option<f64> = r.try_get(c)?;
                value.map(XlsxEditContent::Number)
            }
            "MACADDR8" => {
                let value: Option<MacAddr8> = r.get_unchecked(c);
                value.map(|x| format!("{}", x).into())
            }
            "MACADDR8[]" => {
                let value: Option<Vec<MacAddr8>> = r.get_unchecked(c);
                value.map(|x| output_macaddr8_array(&x).into())
            }

            "MACADDR" => {
                let value: Option<mac_address::MacAddress> = r.try_get(c)?;
                value.map(|x| x.to_string().into())
            }

            "INET" => {
                let value: Option<ipnetwork::IpNetwork> = r.try_get(c)?;
                value.map(|x| format!("{}", x).into())
            }
            "BOOL[]" => {
                let value: Option<Vec<bool>> = r.try_get(c)?;
                value.map(|x| output_bool_array(&x).into())
            }
            "BYTEA[]" => {
                let value: Option<Vec<Vec<u8>>> = r.try_get(c)?;
                value.map(|x| output_bytes_array(&x).into())
            }

            "INT2[]" => {
                let value: Option<Vec<i16>> = r.try_get(c)?;
                value.map(|x| output_array(&x).into())
            }
            "INT4[]" => {
                let value: Option<Vec<i32>> = r.try_get(c)?;
                value.map(|x| output_array(&x).into())
            }
            _ if is_text_array_ty(&ty) => {
                let value: Option<Vec<String>> = r.try_get(c)?;
                value.map(|x| output_text_array(&x).into())
            }
            "INT8[]" => {
                let value: Option<Vec<i64>> = r.try_get(c)?;
                value.map(|x| output_array(&x).into())
            }
            "POINT[]" => default_str,
            "LSEG[]" => default_str,
            "PATH" => default_str,
            "PATH[]" => default_str,
            "BOX[]" => default_str,
            "FLOAT4[]" => {
                let value: Option<Vec<f32>> = r.try_get(c)?;
                value.map(|x| output_array(&x).into())
            }
            "FLOAT8[]" => {
                let value: Option<Vec<f64>> = r.try_get(c)?;
                value.map(|x| output_array(&x).into())
            }
            "POLYGON[]" => default_str,
            "OID[]" => {
                let value: Option<Vec<sqlx::postgres::types::Oid>> = r.try_get(c)?;
                value
                    .map(|x| x.into_iter().map(|x| x.0).collect::<Vec<_>>())
                    .map(|x| output_array(&x).into())
            }
            "MACADDR[]" => {
                let value: Option<Vec<mac_address::MacAddress>> = r.try_get(c)?;
                value.map(|x| output_array(&x).into())
            }
            "INET[]" => {
                let value: Option<Vec<ipnetwork::IpNetwork>> = r.try_get(c)?;
                value.map(|x| output_array(&x).into())
            }
            "CHAR" => {
                let value: Option<String> = r.try_get(c)?;
                value.map(|x| x.into())
            }
            "\"CHAR\"" => {
                let value: Option<i8> = r.try_get(c)?;
                value.map(|x| x as u8 as char).map(|x| x.to_string().into())
            }
            "VARCHAR" => {
                let value: Option<String> = r.try_get(c)?;
                value.map(|x| x.into())
            }
            "DATE" => {
                let value: Option<chrono::NaiveDate> = r.try_get(c)?;
                value.map(|x| format!("{}", x).into())
            }
            "TIME" | "TIME WITHOUT TIME ZONE" => {
                let value: Option<chrono::NaiveTime> = r.try_get(c)?;
                value.map(|x| x.into())
            }
            "TIMESTAMP" | "TIMESTAMP WITHOUT TIME ZONE" => {
                let value: Option<chrono::NaiveDateTime> = r.try_get(c)?;
                value.map(|x| x.into())
            }
            "TIMESTAMP[]" => {
                let value: Option<Vec<chrono::NaiveDateTime>> = r.try_get(c)?;
                value.map(|x| output_naive_datetime_array(&x).into())
            }
            "DATE[]" => {
                let value: Option<Vec<chrono::NaiveDate>> = r.try_get(c)?;
                value.map(|x| output_naive_date_array(&x).into())
            }
            "TIME[]" => {
                let value: Option<Vec<chrono::NaiveTime>> = r.try_get(c)?;
                value.map(|x| output_naive_time_array(&x).into())
            }
            "TIMESTAMPTZ" | "TIMESTAMP WITH TIME ZONE" => {
                let value: Option<chrono::DateTime<FixedOffset>> = r.try_get(c)?;
                value.map(|x| format_data_time_fix_offset(&x).into())
            }

            "TIMESTAMPTZ[]" => {
                let value: Option<Vec<chrono::DateTime<FixedOffset>>> = r.try_get(c)?;
                value.map(|x| output_date_time_fixoffset_array(&x).into())
            }
            "INTERVAL" => {
                let value: Option<sqlx::postgres::types::PgInterval> = r.try_get(c)?;
                value.map(|x| output_pg_interval(&x).into())
            }

            "INTERVAL[]" => {
                let value: Option<Vec<PgInterval>> = r.try_get(c)?;
                value.map(|x| output_pg_interval_array(&x).into())
            }
            "NUMERIC" => {
                let value: Option<BigDecimal> = r.try_get(c)?;
                value.map(|x| format!("{}", x).into())
            }
            "NUMERIC[]" => {
                let value: Option<Vec<BigDecimal>> = r.try_get(c)?;
                value.map(|x| output_array(&x).into())
            }

            "TIMETZ" | "TIME WITH TIME ZONE" => {
                let value: Option<PgTimeTz> = r.try_get(c)?;
                value.map(|x| output_pg_timez(&x).into())
            }
            "TIMETZ[]" => {
                let value: Option<Vec<PgTimeTz>> = r.try_get(c)?;
                value.map(|x| output_pg_timez_array(&x).into())
            }
            "BIT" => {
                let value: Option<sqlx::types::BitVec> = r.try_get(c)?;
                value.map(|x| output_bit_vec(&x).into())
            }
            "BIT[]" => {
                let value: Option<Vec<sqlx::types::BitVec>> = r.try_get(c)?;
                value.map(|x| output_bit_vec_array(&x).into())
            }

            "VARBIT" | "BIT VARYING" => {
                let value: Option<sqlx::types::BitVec> = r.try_get(c)?;
                value.map(|x| output_bit_vec(&x).into())
            }
            "VARBIT[]" => {
                let value: Option<Vec<sqlx::types::BitVec>> = r.try_get(c)?;
                value.map(|x| output_bit_vec_array(&x).into())
            }
            "RECORD" => default_str,
            "RECORD[]" => default_str,
            "UUID" => {
                let value: Option<uuid::Uuid> = r.try_get(c)?;
                value.map(|x| format!("{}", x).into())
            }

            "UUID[]" => {
                let value: Option<Vec<uuid::Uuid>> = r.try_get(c)?;
                value.map(|x| output_array(&x).into())
            }

            "JSON" => {
                let value: Option<serde_json::Value> = r.try_get(c)?;
                value.map(|x| x.to_string().into())
            }
            "JSONB" => {
                let value: Option<serde_json::Value> = r.try_get(c)?;
                value.map(|x| x.to_string().into())
            }
            "INT4RANGE" => {
                let value: Option<PgRange<i32>> = r.try_get(c)?;
                value.map(|x| output_range(&x).into())
            }
            "INT4RANGE[]" => {
                let value: Option<Vec<PgRange<i32>>> = r.try_get(c)?;
                value.map(|x| output_range_array(&x).into())
            }
            "NUMRANGE" => {
                let value: Option<PgRange<BigDecimal>> = r.try_get(c)?;
                value.map(|x| output_range(&x).into())
            }
            "NUMRANGE[]" => {
                let value: Option<Vec<PgRange<BigDecimal>>> = r.try_get(c)?;
                value.map(|x| output_range_array(&x).into())
            }
            "TSRANGE" => {
                let value: Option<PgRange<chrono::NaiveDateTime>> = r.try_get(c)?;
                value.map(|x| output_tsrange(&x).into())
            }
            "TSRANGE[]" => {
                let value: Option<Vec<PgRange<chrono::NaiveDateTime>>> = r.try_get(c)?;
                value.map(|x| output_range_array(&x).into())
            }

            "TSTZRANGE" => {
                let value: Option<PgRange<chrono::DateTime<FixedOffset>>> = r.try_get(c)?;
                value.map(|x| output_range_data_time_fixoffset(&x).into())
            }
            "TSTZRANGE[]" => {
                anyhow::bail!("TSTZRANGE[] not support");
            }
            "DATERANGE" => {
                let value: Option<PgRange<chrono::NaiveDate>> = r.try_get(c)?;
                value.map(|x| output_range(&x).into())
            }
            "DATERANGE[]" => {
                let value: Option<Vec<PgRange<chrono::NaiveDate>>> = r.try_get(c)?;
                value.map(|x| output_range_array(&x).into())
            }
            "INT8RANGE" => {
                let value: Option<PgRange<i64>> = r.try_get(c)?;
                value.map(|x| output_range(&x).into())
            }
            "INT8RANGE[]" => {
                let value: Option<Vec<PgRange<i64>>> = r.try_get(c)?;
                value.map(|x| output_range_array(&x).into())
            }

            "JSONPATH" => {
                anyhow::bail!("JSONPATH not support");
            }
            "JSONPATH[]" => {
                anyhow::bail!("JSONPATH[] not support");
            }

            "MONEY" => {
                let value: Option<PgMoney> = r.try_get(c)?;
                value.map(|x| format!("{}", PgMoneyFmt::from(x)).into())
            }

            "MONEY[]" => {
                let value: Option<Vec<PgMoney>> = r.try_get(c)?;
                value.map(|x| output_pg_money_array(&x).into())
            }
            "VOID" => {
                anyhow::bail!("void not support")
            }
            _ => default_str,
        };
        values.push(v);
    }
    anyhow::Ok(values)
}
/// encode rows to `Vec<Vec<String>>`
pub(crate) fn encode_row(r: &PgRow) -> anyhow::Result<Vec<Option<String>>> {
    let mut values = Vec::with_capacity(r.columns().len());
    for c in 0..r.len() {
        let col = r.column(c);
        let ty = col.type_info().to_string().to_uppercase();
        let value = r.try_get_raw(c)?;
        let default_str = if value.is_null() {
            None
        } else {
            Some(format!(
                "pg value[ty:{} data:{}]",
                ty,
                output_bytes(
                    value
                        .as_bytes()
                        .map_err(|e| anyhow::Error::msg(e.to_string()))?
                )
            ))
        };

        let v = match ty.as_str() {
            "REGPROC" => {
                let value: Option<RegProcString> = r.try_get(c)?;
                value.map(|x| x.0)
            }
            "REGPROC[]" => {
                let value: Option<Vec<RegProcString>> = r.try_get(c)?;
                if let Some(value) = value {
                    let x = value
                        .into_iter()
                        .map(|x| format!("\"{}\"", x.0))
                        .collect::<Vec<String>>();

                    Some(format!("{{{}}}", x.join(",")))
                } else {
                    None
                }
            }
            "BOOL" => {
                let value: Option<bool> = r.try_get(c)?;
                value.map(|x| x.to_string())
            }
            "BYTEA" => {
                let value: Option<Vec<u8>> = r.try_get(c)?;
                value.map(|x| output_bytes(&x[..]))
            }

            "NAME" => {
                let value: Option<String> = r.try_get(c)?;
                value
            }
            "INT8" => {
                let value: Option<i64> = r.try_get(c)?;
                value.map(|x| format!("{}", x))
            }
            "INT2" => {
                let value: Option<i16> = r.try_get(c)?;
                value.map(|x| format!("{}", x))
            }
            "INT4" => {
                let value: Option<i32> = r.try_get(c)?;
                value.map(|x| format!("{}", x))
            }
            "TEXT" => {
                let value: Option<String> = r.try_get(c)?;
                value
            }
            "OID" => {
                let value: Option<sqlx::postgres::types::Oid> = r.try_get(c)?;
                value.map(|x| format!("{}", x.0))
            }

            "FLOAT4" => {
                let value: Option<f32> = r.try_get(c)?;
                value.map(|x| format!("{}", x))
            }
            "FLOAT8" => {
                let value: Option<f64> = r.try_get(c)?;
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
                let value: Option<mac_address::MacAddress> = r.try_get(c)?;
                value.map(|x| x.to_string())
            }

            "INET" => {
                let value: Option<ipnetwork::IpNetwork> = r.try_get(c)?;
                value.map(|x| format!("{}", x))
            }
            "BOOL[]" => {
                let value: Option<Vec<bool>> = r.try_get(c)?;
                value.map(|x| output_bool_array(&x))
            }
            "BYTEA[]" => {
                let value: Option<Vec<Vec<u8>>> = r.try_get(c)?;
                value.map(|x| output_bytes_array(&x))
            }

            "INT2[]" => {
                let value: Option<Vec<i16>> = r.try_get(c)?;
                value.map(|x| output_array(&x))
            }
            "INT4[]" => {
                let value: Option<Vec<i32>> = r.try_get(c)?;
                value.map(|x| output_array(&x))
            }
            _ if is_text_array_ty(&ty) => {
                let value: Option<Vec<String>> = r.try_get(c)?;
                value.map(|x| output_text_array(&x))
            }
            "INT8[]" => {
                let value: Option<Vec<i64>> = r.try_get(c)?;
                value.map(|x| output_array(&x))
            }
            "POINT[]" => default_str,
            "LSEG[]" => default_str,
            "PATH" => default_str,
            "PATH[]" => default_str,
            "BOX[]" => default_str,
            "FLOAT4[]" => {
                let value: Option<Vec<f32>> = r.try_get(c)?;
                value.map(|x| output_array(&x))
            }
            "FLOAT8[]" => {
                let value: Option<Vec<f64>> = r.try_get(c)?;
                value.map(|x| output_array(&x))
            }
            "POLYGON[]" => default_str,
            "OID[]" => {
                let value: Option<Vec<sqlx::postgres::types::Oid>> = r.try_get(c)?;
                value
                    .map(|x| x.into_iter().map(|x| x.0).collect::<Vec<_>>())
                    .map(|x| output_array(&x))
            }
            "MACADDR[]" => {
                let value: Option<Vec<mac_address::MacAddress>> = r.try_get(c)?;
                value.map(|x| output_array(&x))
            }
            "INET[]" => {
                let value: Option<Vec<ipnetwork::IpNetwork>> = r.try_get(c)?;
                value.map(|x| output_array(&x))
            }
            "CHAR" => {
                let value: Option<String> = r.try_get(c)?;
                value
            }
            "\"CHAR\"" => {
                let value: Option<i8> = r.try_get(c)?;
                value.map(|x| x as u8 as char).map(|x| x.to_string())
            }
            "VARCHAR" => {
                let value: Option<String> = r.try_get(c)?;
                value
            }
            "DATE" => {
                let value: Option<chrono::NaiveDate> = r.try_get(c)?;
                value.map(|x| format!("{}", x))
            }
            "TIME" | "TIME WITHOUT TIME ZONE" => {
                let value: Option<chrono::NaiveTime> = r.try_get(c)?;
                value.map(|x| format!("{}", x))
            }
            "TIMESTAMP" | "TIMESTAMP WITHOUT TIME ZONE" => {
                let value: Option<chrono::NaiveDateTime> = r.try_get(c)?;
                value.map(|x| format!("{}", x))
            }
            "TIMESTAMP[]" => {
                let value: Option<Vec<chrono::NaiveDateTime>> = r.try_get(c)?;
                value.map(|x| output_naive_datetime_array(&x))
            }
            "DATE[]" => {
                let value: Option<Vec<chrono::NaiveDate>> = r.try_get(c)?;
                value.map(|x| output_naive_date_array(&x))
            }
            "TIME[]" => {
                let value: Option<Vec<chrono::NaiveTime>> = r.try_get(c)?;
                value.map(|x| output_naive_time_array(&x))
            }
            "TIMESTAMPTZ" | "TIMESTAMP WITH TIME ZONE" => {
                let value: Option<chrono::DateTime<FixedOffset>> = r.try_get(c)?;
                value.map(|x| format_data_time_fix_offset(&x))
            }

            "TIMESTAMPTZ[]" => {
                let value: Option<Vec<chrono::DateTime<FixedOffset>>> = r.try_get(c)?;
                value.map(|x| output_date_time_fixoffset_array(&x))
            }
            "INTERVAL" => {
                let value: Option<sqlx::postgres::types::PgInterval> = r.try_get(c)?;
                value.map(|x| output_pg_interval(&x))
            }

            "INTERVAL[]" => {
                let value: Option<Vec<PgInterval>> = r.try_get(c)?;
                value.map(|x| output_pg_interval_array(&x))
            }
            "NUMERIC" => {
                let value: Option<BigDecimal> = r.try_get(c)?;
                value.map(|x| format!("{}", x))
            }
            "NUMERIC[]" => {
                let value: Option<Vec<BigDecimal>> = r.try_get(c)?;
                value.map(|x| output_array(&x))
            }

            "TIMETZ" | "TIME WITH TIME ZONE" => {
                let value: Option<PgTimeTz> = r.try_get(c)?;
                value.map(|x| output_pg_timez(&x))
            }
            "TIMETZ[]" => {
                let value: Option<Vec<PgTimeTz>> = r.try_get(c)?;
                value.map(|x| output_pg_timez_array(&x))
            }
            "BIT" => {
                let value: Option<sqlx::types::BitVec> = r.try_get(c)?;
                value.map(|x| output_bit_vec(&x))
            }
            "BIT[]" => {
                let value: Option<Vec<sqlx::types::BitVec>> = r.try_get(c)?;
                value.map(|x| output_bit_vec_array(&x))
            }

            "VARBIT" | "BIT VARYING" => {
                let value: Option<sqlx::types::BitVec> = r.try_get(c)?;
                value.map(|x| output_bit_vec(&x))
            }
            "VARBIT[]" => {
                let value: Option<Vec<sqlx::types::BitVec>> = r.try_get(c)?;
                value.map(|x| output_bit_vec_array(&x))
            }
            "RECORD" => default_str,
            "RECORD[]" => default_str,
            "UUID" => {
                let value: Option<uuid::Uuid> = r.try_get(c)?;
                value.map(|x| format!("{}", x))
            }

            "UUID[]" => {
                let value: Option<Vec<uuid::Uuid>> = r.try_get(c)?;
                value.map(|x| output_array(&x))
            }

            "JSON" => {
                let value: Option<serde_json::Value> = r.try_get(c)?;
                value.map(|x| x.to_string())
            }
            "JSONB" => {
                let value: Option<serde_json::Value> = r.try_get(c)?;
                value.map(|x| x.to_string())
            }
            "INT4RANGE" => {
                let value: Option<PgRange<i32>> = r.try_get(c)?;
                value.map(|x| output_range(&x))
            }
            "INT4RANGE[]" => {
                let value: Option<Vec<PgRange<i32>>> = r.try_get(c)?;
                value.map(|x| output_range_array(&x))
            }
            "NUMRANGE" => {
                let value: Option<PgRange<BigDecimal>> = r.try_get(c)?;
                value.map(|x| output_range(&x))
            }
            "NUMRANGE[]" => {
                let value: Option<Vec<PgRange<BigDecimal>>> = r.try_get(c)?;
                value.map(|x| output_range_array(&x))
            }
            "TSRANGE" => {
                let value: Option<PgRange<chrono::NaiveDateTime>> = r.try_get(c)?;
                value.map(|x| output_tsrange(&x))
            }
            "TSRANGE[]" => {
                let value: Option<Vec<PgRange<chrono::NaiveDateTime>>> = r.try_get(c)?;
                value.map(|x| output_range_array(&x))
            }

            "TSTZRANGE" => {
                let value: Option<PgRange<chrono::DateTime<FixedOffset>>> = r.try_get(c)?;
                value.map(|x| output_range_data_time_fixoffset(&x))
            }
            "TSTZRANGE[]" => {
                anyhow::bail!("TSTZRANGE[] not support");
            }
            "DATERANGE" => {
                let value: Option<PgRange<chrono::NaiveDate>> = r.try_get(c)?;
                value.map(|x| output_range(&x))
            }
            "DATERANGE[]" => {
                let value: Option<Vec<PgRange<chrono::NaiveDate>>> = r.try_get(c)?;
                value.map(|x| output_range_array(&x))
            }
            "INT8RANGE" => {
                let value: Option<PgRange<i64>> = r.try_get(c)?;
                value.map(|x| output_range(&x))
            }
            "INT8RANGE[]" => {
                let value: Option<Vec<PgRange<i64>>> = r.try_get(c)?;
                value.map(|x| output_range_array(&x))
            }

            "JSONPATH" => {
                anyhow::bail!("JSONPATH not support");
            }
            "JSONPATH[]" => {
                anyhow::bail!("JSONPATH[] not support");
            }

            "MONEY" => {
                let value: Option<PgMoney> = r.try_get(c)?;
                value.map(|x| format!("{}", PgMoneyFmt::from(x)))
            }

            "MONEY[]" => {
                let value: Option<Vec<PgMoney>> = r.try_get(c)?;
                value.map(|x| output_pg_money_array(&x))
            }
            "VOID" => {
                anyhow::bail!("void not support")
            }
            _ => default_str,
        };
        values.push(v);
    }
    anyhow::Ok(values)
}

pub(crate) fn sql_encode_value(typ: &str, value: &str) -> anyhow::Result<String> {
    let default_str = value.to_string();

    let single_quote_str = quote_sql_str(value);

    let err =
        Err(anyhow::Error::msg(format!("type '{}' not supported", typ))) as anyhow::Result<String>;
    let typ = typ.to_uppercase();

    let encode = match typ.as_str() {
        "BOOL" => default_str,
        "BYTEA" => format!("{}::bytea", single_quote_str),

        "NAME" => single_quote_str,
        "\"CHAR\"" => single_quote_str,

        "INT8" | "BIGINT" => default_str,
        "INT2" | "SMALLINT" => default_str,
        "INT4" | "INTEGER" => default_str,
        "TEXT" => single_quote_str,
        "CHAR" | "CHARACTER" => single_quote_str,
        _ if typ.starts_with("CHAR") || typ.starts_with("CHARACTER") => single_quote_str,
        "OID" => default_str,
        "JSON" => format!("{}::json", single_quote_str),
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
        "FLOAT4" => default_str,
        "FLOAT8" => default_str,
        "UNKNOWN" => return err,
        "CIRCLE" => return err,
        "CIRCLE[]" => return err,
        "MACADDR8" => single_quote_str,
        "MACADDR8[]" => single_quote_str,
        "MACADDR" => single_quote_str,
        "INET" => single_quote_str,
        "BOOL[]" => single_quote_str,
        "BYTEA[]" => format!("{}::bytea[]", single_quote_str),

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
        "BIT" => format!("B'{value}'"),

        "BIT[]" => single_quote_str,
        "VARBIT" | "BIT VARYING" => format!("B'{value}'"),
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
        "TSTZRANGE[]" => return err,
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
    log::trace!("type: '{}' encode '{:?}' as #{}#", typ, value, encode);
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

pub(crate) fn output_pg_money_array(array: &[PgMoney]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("{}", PgMoneyFmt::from(*v)).as_str());
        if index != last {
            ret.push(',')
        }
    }
    ret.push('}');
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
        Unbounded => ret.push(')'),
    }
    ret
}

pub(crate) fn output_tsrange(r: &PgRange<chrono::NaiveDateTime>) -> String {
    use std::ops::Bound::*;
    let mut ret = "".to_string();
    match &r.start {
        Included(x) => ret.push_str(format!("[\"{}\",", x).as_str()),
        Excluded(x) => ret.push_str(format!("(\"{}\",", x).as_str()),
        Unbounded => ret.push_str("(,"),
    };
    match &r.end {
        Included(x) => ret.push_str(format!("\"{}\"]", x).as_str()),
        Excluded(x) => ret.push_str(format!("\"{}\")", x).as_str()),
        Unbounded => ret.push(')'),
    }
    ret
}

pub(crate) fn output_range_data_time_fixoffset(
    r: &PgRange<chrono::DateTime<FixedOffset>>,
) -> String {
    use std::ops::Bound::*;
    let mut ret = "".to_string();
    match &r.start {
        // Removed the double quotes around the time value.
        Included(x) => ret.push_str(format!("[\"{}\",", format_data_time_fix_offset(x)).as_str()),
        Excluded(x) => ret.push_str(format!("(\"{}\",", format_data_time_fix_offset(x)).as_str()),
        Unbounded => ret.push_str("(,"),
    };
    match &r.end {
        // Removed the double quotes around the time value.
        Included(x) => ret.push_str(format!("{}]", format_data_time_fix_offset(x)).as_str()),
        Excluded(x) => ret.push_str(format!("{})", format_data_time_fix_offset(x)).as_str()),
        Unbounded => ret.push(')'),
    }
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

pub(crate) fn output_range_array<T: std::fmt::Display>(array: &[PgRange<T>]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("\"{}\"", output_range(v)).as_str());
        if index != last {
            ret.push(',')
        }
    }
    ret.push('}');
    ret
}

pub(crate) fn output_bit_vec(v: &sqlx::types::BitVec) -> String {
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

pub(crate) fn output_bit_vec_array(array: &[sqlx::types::BitVec]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(output_bit_vec(v).as_str());
        if index != last {
            ret.push(',')
        }
    }

    ret.push('}');
    ret
}
pub(crate) fn output_pg_timez(v: &PgTimeTz) -> String {
    format!("{}{}", v.time, v.offset)
}

pub(crate) fn output_pg_timez_array(array: &[PgTimeTz]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(output_pg_timez(v).as_str());
        if index != last {
            ret.push(',')
        }
    }
    ret.push('}');
    ret
}

pub(crate) fn output_pg_interval(v: &PgInterval) -> String {
    let x = pg_interval::Interval::new(v.months, v.days, v.microseconds);
    x.to_postgres()
}

pub(crate) fn output_pg_interval_array(array: &[PgInterval]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("\"{}\"", output_pg_interval(v)).as_str());
        if index != last {
            ret.push(',')
        }
    }
    ret.push('}');
    ret
}

pub(crate) fn output_date_time_fixoffset_array(array: &[chrono::DateTime<FixedOffset>]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("\"{}\"", format_data_time_fix_offset(v)).as_str());
        if index != last {
            ret.push(',')
        }
    }
    ret.push('}');
    ret
}

pub(crate) fn output_naive_time_array(array: &[chrono::NaiveTime]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("{}", v).as_str());
        if index != last {
            ret.push(',')
        }
    }
    ret.push('}');
    ret
}
pub(crate) fn output_naive_date_array(array: &[chrono::NaiveDate]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("{}", v).as_str());
        if index != last {
            ret.push(',')
        }
    }
    ret.push('}');
    ret
}

pub(crate) fn output_naive_datetime_array(array: &[chrono::NaiveDateTime]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("\"{}\"", v).as_str());
        if index != last {
            ret.push(',')
        }
    }
    ret.push('}');
    ret
}

pub(crate) fn output_bool_array(array: &[bool]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str((if *v { "t" } else { "f" }).to_string().as_str());
        if index != last {
            ret.push(',')
        }
    }
    ret.push('}');
    ret
}

pub(crate) fn escape_double_quote(v: &str) -> String {
    format!("\"{}\"", v.replace('"', "\\\""))
}

pub(crate) fn output_text_array(array: &[String]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(escape_double_quote(v).as_str());
        if index != last {
            ret.push(',')
        }
    }
    ret.push('}');
    ret
}

pub(crate) fn output_array<T: std::fmt::Display>(array: &[T]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("{}", v).as_str());
        if index != last {
            ret.push(',')
        }
    }
    ret.push('}');
    ret
}
pub(crate) fn output_bytes_array(array: &[Vec<u8>]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }

    pub(crate) fn output_bytes(bs: &[u8]) -> String {
        format!(r#"\\x{}"#, hex::encode_upper(bs))
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!(r#""{}""#, output_bytes(v)).as_str());
        if index != last {
            ret.push(',')
        }
    }
    ret.push('}');
    ret
}

pub(crate) fn output_bytes(bs: &[u8]) -> String {
    format!(r#"\x{}"#, hex::encode_upper(bs))
}

pub(crate) fn output_macaddr8_array(array: &[MacAddr8]) -> String {
    if array.is_empty() {
        return "{}".to_string();
    }
    let last = array.len() - 1;
    let mut ret = "{".to_string();
    for (index, v) in array.iter().enumerate() {
        ret.push_str(format!("{}", v).as_str());
        if index != last {
            ret.push(',')
        }
    }
    ret.push('}');
    ret
}
