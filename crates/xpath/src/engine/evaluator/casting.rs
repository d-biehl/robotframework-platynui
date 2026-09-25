//! XSD type casting for the `XPath` evaluator.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

use crate::engine::functions::parse_qname_lexical;
use crate::engine::runtime::{Error, ErrorCode};
use crate::model::XdmNode;
use crate::util::temporal::{parse_g_day, parse_g_month, parse_g_month_day, parse_g_year, parse_g_year_month};
use crate::xdm::{ExpandedName, XdmAtomicValue};

use super::Vm;
use super::xml_helpers::{
    collapse_xml_whitespace, decode_hex, encode_hex_upper, is_valid_language, is_valid_name, is_valid_nmtoken,
    replace_xml_whitespace, string_like_into_owned, string_like_value,
};

impl<N: 'static + XdmNode + Clone> Vm<N> {
    // One dispatch arm per XSD cast target type.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn cast_atomic(a: XdmAtomicValue, target: &ExpandedName) -> Result<XdmAtomicValue, Error> {
        // Namespace check: only xs:* types supported
        if let Some(ns) = &target.ns_uri {
            let xs_ns = crate::consts::XS;
            if ns.as_str() != xs_ns {
                return Err(Error::from_code(ErrorCode::XPTY0004, "unsupported cast target namespace"));
            }
        }
        match target.local.as_str() {
            "anyAtomicType" => Ok(a),
            "string" => match string_like_into_owned(a) {
                Ok(s) => Ok(XdmAtomicValue::String(s)),
                Err(other) => Ok(XdmAtomicValue::String(Self::atomic_to_string(&other))),
            },
            "untypedAtomic" => match string_like_into_owned(a) {
                Ok(s) => Ok(XdmAtomicValue::UntypedAtomic(s)),
                Err(other) => Ok(XdmAtomicValue::UntypedAtomic(Self::atomic_to_string(&other))),
            },
            "boolean" => match a {
                XdmAtomicValue::Boolean(b) => Ok(XdmAtomicValue::Boolean(b)),
                XdmAtomicValue::Integer(i) => Ok(XdmAtomicValue::Boolean(i != 0)),
                XdmAtomicValue::Decimal(d) => Ok(XdmAtomicValue::Boolean(!d.is_zero())),
                XdmAtomicValue::Double(d) => Ok(XdmAtomicValue::Boolean(d != 0.0 && !d.is_nan())),
                XdmAtomicValue::Float(f) => Ok(XdmAtomicValue::Boolean(f != 0.0 && !f.is_nan())),
                other => {
                    let text = Self::require_string_like(&other, "xs:boolean")?;
                    let b = match text {
                        "true" | "1" => true,
                        "false" | "0" => false,
                        _ => {
                            return Err(Error::from_code(ErrorCode::FORG0001, "invalid boolean lexical form"));
                        }
                    };
                    Ok(XdmAtomicValue::Boolean(b))
                }
            },
            "integer" => match a {
                XdmAtomicValue::Integer(v) => Ok(XdmAtomicValue::Integer(v)),
                other => {
                    let value = Self::integer_from_atomic(&other, "xs:integer")?;
                    let bounded =
                        Self::ensure_range_i128(value, i128::from(i64::MIN), i128::from(i64::MAX), "xs:integer")?;
                    Ok(XdmAtomicValue::Integer(bounded))
                }
            },
            "decimal" => match a {
                XdmAtomicValue::Decimal(d) => Ok(XdmAtomicValue::Decimal(d)),
                XdmAtomicValue::Integer(i)
                | XdmAtomicValue::Long(i)
                | XdmAtomicValue::NonPositiveInteger(i)
                | XdmAtomicValue::NegativeInteger(i) => Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i))),
                XdmAtomicValue::Int(i) => Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i))),
                XdmAtomicValue::Short(i) => Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i64::from(i)))),
                XdmAtomicValue::Byte(i) => Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i64::from(i)))),
                XdmAtomicValue::NonNegativeInteger(i)
                | XdmAtomicValue::PositiveInteger(i)
                | XdmAtomicValue::UnsignedLong(i) => Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i))),
                XdmAtomicValue::UnsignedInt(i) => Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i))),
                XdmAtomicValue::UnsignedShort(i) => {
                    Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(u64::from(i))))
                }
                XdmAtomicValue::UnsignedByte(i) => {
                    Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(u64::from(i))))
                }
                XdmAtomicValue::Double(d) => {
                    if d.is_finite() {
                        use rust_decimal::prelude::FromPrimitive;
                        Ok(XdmAtomicValue::Decimal(
                            rust_decimal::Decimal::from_f64(d).unwrap_or(rust_decimal::Decimal::ZERO),
                        ))
                    } else {
                        Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"))
                    }
                }
                XdmAtomicValue::Float(f) => {
                    if f.is_finite() {
                        use rust_decimal::prelude::FromPrimitive;
                        Ok(XdmAtomicValue::Decimal(
                            rust_decimal::Decimal::from_f32(f).unwrap_or(rust_decimal::Decimal::ZERO),
                        ))
                    } else {
                        Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"))
                    }
                }
                other => {
                    use std::str::FromStr;
                    let text = Self::require_string_like(&other, "xs:decimal")?;
                    let trimmed = text.trim();
                    if trimmed.eq_ignore_ascii_case("nan")
                        || trimmed.eq_ignore_ascii_case("inf")
                        || trimmed.eq_ignore_ascii_case("-inf")
                    {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"));
                    }
                    let value = rust_decimal::Decimal::from_str(trimmed)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"))?;
                    Ok(XdmAtomicValue::Decimal(value))
                }
            },
            // Casting an integer to xs:double rounds to the nearest double, per the XPath casting rules.
            #[allow(clippy::cast_precision_loss)]
            "double" => match a {
                XdmAtomicValue::Double(d) => Ok(XdmAtomicValue::Double(d)),
                XdmAtomicValue::Float(f) => Ok(XdmAtomicValue::Double(f64::from(f))),
                XdmAtomicValue::Decimal(d) => {
                    use rust_decimal::prelude::ToPrimitive;
                    Ok(XdmAtomicValue::Double(d.to_f64().unwrap_or(f64::NAN)))
                }
                XdmAtomicValue::Integer(i)
                | XdmAtomicValue::Long(i)
                | XdmAtomicValue::NonPositiveInteger(i)
                | XdmAtomicValue::NegativeInteger(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::Int(i) => Ok(XdmAtomicValue::Double(f64::from(i))),
                XdmAtomicValue::Short(i) => Ok(XdmAtomicValue::Double(f64::from(i))),
                XdmAtomicValue::Byte(i) => Ok(XdmAtomicValue::Double(f64::from(i))),
                XdmAtomicValue::NonNegativeInteger(i)
                | XdmAtomicValue::PositiveInteger(i)
                | XdmAtomicValue::UnsignedLong(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::UnsignedInt(i) => Ok(XdmAtomicValue::Double(f64::from(i))),
                XdmAtomicValue::UnsignedShort(i) => Ok(XdmAtomicValue::Double(f64::from(i))),
                XdmAtomicValue::UnsignedByte(i) => Ok(XdmAtomicValue::Double(f64::from(i))),
                other => {
                    let text = Self::require_string_like(&other, "xs:double")?;
                    let trimmed = text.trim();
                    let value = match trimmed {
                        "NaN" | "nan" => f64::NAN,
                        "INF" | "inf" => f64::INFINITY,
                        "-INF" | "-inf" => f64::NEG_INFINITY,
                        _ => trimmed.parse().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:double"))?,
                    };
                    Ok(XdmAtomicValue::Double(value))
                }
            },
            // Casting to xs:float rounds to the nearest float, per the XPath casting rules; out-of-range
            // doubles become infinity, as `as` does.
            #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
            "float" => match a {
                XdmAtomicValue::Float(f) => Ok(XdmAtomicValue::Float(f)),
                XdmAtomicValue::Double(d) => Ok(XdmAtomicValue::Float(d as f32)),
                XdmAtomicValue::Decimal(d) => {
                    use rust_decimal::prelude::ToPrimitive;
                    Ok(XdmAtomicValue::Float(d.to_f32().unwrap_or(f32::NAN)))
                }
                XdmAtomicValue::Integer(i)
                | XdmAtomicValue::Long(i)
                | XdmAtomicValue::NonPositiveInteger(i)
                | XdmAtomicValue::NegativeInteger(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::Int(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::Short(i) => Ok(XdmAtomicValue::Float(f32::from(i))),
                XdmAtomicValue::Byte(i) => Ok(XdmAtomicValue::Float(f32::from(i))),
                XdmAtomicValue::NonNegativeInteger(i)
                | XdmAtomicValue::PositiveInteger(i)
                | XdmAtomicValue::UnsignedLong(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::UnsignedInt(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::UnsignedShort(i) => Ok(XdmAtomicValue::Float(f32::from(i))),
                XdmAtomicValue::UnsignedByte(i) => Ok(XdmAtomicValue::Float(f32::from(i))),
                other => {
                    let text = Self::require_string_like(&other, "xs:float")?;
                    let trimmed = text.trim();
                    let value = match trimmed {
                        "NaN" | "nan" => f32::NAN,
                        "INF" | "inf" => f32::INFINITY,
                        "-INF" | "-inf" => f32::NEG_INFINITY,
                        _ => trimmed.parse().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:float"))?,
                    };
                    Ok(XdmAtomicValue::Float(value))
                }
            },
            "long" => match a {
                XdmAtomicValue::Long(v) => Ok(XdmAtomicValue::Long(v)),
                other => {
                    let value = Self::integer_from_atomic(&other, "xs:long")?;
                    let bounded =
                        Self::ensure_range_i128(value, i128::from(i64::MIN), i128::from(i64::MAX), "xs:long")?;
                    Ok(XdmAtomicValue::Long(bounded))
                }
            },
            "int" => match a {
                XdmAtomicValue::Int(v) => Ok(XdmAtomicValue::Int(v)),
                other => {
                    let value = Self::integer_from_atomic(&other, "xs:int")?;
                    let bounded = Self::ensure_range_i128(value, i128::from(i32::MIN), i128::from(i32::MAX), "xs:int")?;
                    Ok(XdmAtomicValue::Int(bounded))
                }
            },
            "short" => match a {
                XdmAtomicValue::Short(v) => Ok(XdmAtomicValue::Short(v)),
                other => {
                    let value = Self::integer_from_atomic(&other, "xs:short")?;
                    let bounded =
                        Self::ensure_range_i128(value, i128::from(i16::MIN), i128::from(i16::MAX), "xs:short")?;
                    Ok(XdmAtomicValue::Short(bounded))
                }
            },
            "byte" => match a {
                XdmAtomicValue::Byte(v) => Ok(XdmAtomicValue::Byte(v)),
                other => {
                    let value = Self::integer_from_atomic(&other, "xs:byte")?;
                    let bounded = Self::ensure_range_i128(value, i128::from(i8::MIN), i128::from(i8::MAX), "xs:byte")?;
                    Ok(XdmAtomicValue::Byte(bounded))
                }
            },
            "unsignedLong" => match a {
                XdmAtomicValue::UnsignedLong(v) => Ok(XdmAtomicValue::UnsignedLong(v)),
                other => {
                    let value = Self::unsigned_from_atomic(&other, "xs:unsignedLong")?;
                    let bounded = Self::ensure_range_u128(value, 0, u128::from(u64::MAX), "xs:unsignedLong")?;
                    Ok(XdmAtomicValue::UnsignedLong(bounded))
                }
            },
            "unsignedInt" => match a {
                XdmAtomicValue::UnsignedInt(v) => Ok(XdmAtomicValue::UnsignedInt(v)),
                other => {
                    let value = Self::unsigned_from_atomic(&other, "xs:unsignedInt")?;
                    let bounded = Self::ensure_range_u128(value, 0, u128::from(u32::MAX), "xs:unsignedInt")?;
                    Ok(XdmAtomicValue::UnsignedInt(bounded))
                }
            },
            "unsignedShort" => match a {
                XdmAtomicValue::UnsignedShort(v) => Ok(XdmAtomicValue::UnsignedShort(v)),
                other => {
                    let value = Self::unsigned_from_atomic(&other, "xs:unsignedShort")?;
                    let bounded = Self::ensure_range_u128(value, 0, u128::from(u16::MAX), "xs:unsignedShort")?;
                    Ok(XdmAtomicValue::UnsignedShort(bounded))
                }
            },
            "unsignedByte" => match a {
                XdmAtomicValue::UnsignedByte(v) => Ok(XdmAtomicValue::UnsignedByte(v)),
                other => {
                    let value = Self::unsigned_from_atomic(&other, "xs:unsignedByte")?;
                    let bounded = Self::ensure_range_u128(value, 0, u128::from(u8::MAX), "xs:unsignedByte")?;
                    Ok(XdmAtomicValue::UnsignedByte(bounded))
                }
            },
            "nonPositiveInteger" => match a {
                XdmAtomicValue::NonPositiveInteger(v) => Ok(XdmAtomicValue::NonPositiveInteger(v)),
                other => {
                    let value = Self::integer_from_atomic(&other, "xs:nonPositiveInteger")?;
                    if value > 0 {
                        return Err(Error::from_code(
                            ErrorCode::FORG0001,
                            "value must be <= 0 for xs:nonPositiveInteger",
                        ));
                    }
                    let bounded = Self::ensure_range_i128(value, i128::from(i64::MIN), 0, "xs:nonPositiveInteger")?;
                    Ok(XdmAtomicValue::NonPositiveInteger(bounded))
                }
            },
            "negativeInteger" => match a {
                XdmAtomicValue::NegativeInteger(v) => Ok(XdmAtomicValue::NegativeInteger(v)),
                other => {
                    let value = Self::integer_from_atomic(&other, "xs:negativeInteger")?;
                    if value >= 0 {
                        return Err(Error::from_code(ErrorCode::FORG0001, "value must be < 0 for xs:negativeInteger"));
                    }
                    let bounded = Self::ensure_range_i128(value, i128::from(i64::MIN), -1, "xs:negativeInteger")?;
                    Ok(XdmAtomicValue::NegativeInteger(bounded))
                }
            },
            "nonNegativeInteger" => match a {
                XdmAtomicValue::NonNegativeInteger(v) => Ok(XdmAtomicValue::NonNegativeInteger(v)),
                other => {
                    let value = Self::unsigned_from_atomic(&other, "xs:nonNegativeInteger")?;
                    let bounded = Self::ensure_range_u128(value, 0, u128::from(u64::MAX), "xs:nonNegativeInteger")?;
                    Ok(XdmAtomicValue::NonNegativeInteger(bounded))
                }
            },
            "positiveInteger" => match a {
                XdmAtomicValue::PositiveInteger(v) => Ok(XdmAtomicValue::PositiveInteger(v)),
                other => {
                    let value = Self::unsigned_from_atomic(&other, "xs:positiveInteger")?;
                    if value == 0 {
                        return Err(Error::from_code(ErrorCode::FORG0001, "value must be > 0 for xs:positiveInteger"));
                    }
                    let bounded = Self::ensure_range_u128(value, 1, u128::from(u64::MAX), "xs:positiveInteger")?;
                    Ok(XdmAtomicValue::PositiveInteger(bounded))
                }
            },
            "anyURI" => match a {
                XdmAtomicValue::AnyUri(uri) => Ok(XdmAtomicValue::AnyUri(uri)),
                other => {
                    let text = Self::require_string_like(&other, "xs:anyURI")?;
                    Ok(XdmAtomicValue::AnyUri(text.trim().to_string()))
                }
            },
            "QName" => match a {
                XdmAtomicValue::QName { ns_uri, prefix, local } => Ok(XdmAtomicValue::QName { ns_uri, prefix, local }),
                other => {
                    let text = Self::require_string_like(&other, "xs:QName")?;
                    let (prefix, local) = parse_qname_lexical(text)
                        .map_err(|()| Error::from_code(ErrorCode::FORG0001, "invalid QName lexical"))?;
                    Ok(XdmAtomicValue::QName { ns_uri: None, prefix, local })
                }
            },
            "NOTATION" => match a {
                XdmAtomicValue::Notation(s) => Ok(XdmAtomicValue::Notation(s)),
                other => {
                    let text = Self::require_string_like(&other, "xs:NOTATION")?;
                    if parse_qname_lexical(text).is_err() {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:NOTATION"));
                    }
                    Ok(XdmAtomicValue::Notation(text.to_owned()))
                }
            },
            "base64Binary" => match a {
                XdmAtomicValue::Base64Binary(v) => Ok(XdmAtomicValue::Base64Binary(v)),
                XdmAtomicValue::HexBinary(hex) => {
                    let bytes = decode_hex(&hex)
                        .ok_or_else(|| Error::from_code(ErrorCode::FORG0001, "invalid xs:hexBinary"))?;
                    let encoded = BASE64_STANDARD.encode(bytes);
                    Ok(XdmAtomicValue::Base64Binary(encoded))
                }
                other => {
                    let text = Self::require_string_like(&other, "xs:base64Binary")?;
                    let normalized: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                    if BASE64_STANDARD.decode(normalized.as_bytes()).is_err() {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:base64Binary"));
                    }
                    Ok(XdmAtomicValue::Base64Binary(normalized))
                }
            },
            "hexBinary" => match a {
                XdmAtomicValue::HexBinary(v) => Ok(XdmAtomicValue::HexBinary(v)),
                XdmAtomicValue::Base64Binary(b64) => {
                    let bytes = BASE64_STANDARD
                        .decode(b64.as_bytes())
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:base64Binary"))?;
                    let encoded = encode_hex_upper(&bytes);
                    Ok(XdmAtomicValue::HexBinary(encoded))
                }
                other => {
                    let text = Self::require_string_like(&other, "xs:hexBinary")?;
                    let normalized: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                    if decode_hex(&normalized).is_none() {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:hexBinary"));
                    }
                    Ok(XdmAtomicValue::HexBinary(normalized.to_uppercase()))
                }
            },
            "normalizedString" => match a {
                XdmAtomicValue::NormalizedString(s) => Ok(XdmAtomicValue::NormalizedString(s)),
                other => {
                    let text = Self::require_string_like(&other, "xs:normalizedString")?;
                    let normalized = replace_xml_whitespace(text);
                    Ok(XdmAtomicValue::NormalizedString(normalized.into_owned()))
                }
            },
            "token" => match a {
                XdmAtomicValue::Token(s) => Ok(XdmAtomicValue::Token(s)),
                other => {
                    let text = Self::require_string_like(&other, "xs:token")?;
                    let collapsed = collapse_xml_whitespace(text);
                    Ok(XdmAtomicValue::Token(collapsed.into_owned()))
                }
            },
            "language" => match a {
                XdmAtomicValue::Language(s) => Ok(XdmAtomicValue::Language(s)),
                other => {
                    let text = Self::require_string_like(&other, "xs:language")?;
                    let collapsed = collapse_xml_whitespace(text);
                    if !is_valid_language(&collapsed) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:language"));
                    }
                    Ok(XdmAtomicValue::Language(collapsed.into_owned()))
                }
            },
            "Name" => match a {
                XdmAtomicValue::Name(s) => Ok(XdmAtomicValue::Name(s)),
                other => {
                    let text = Self::require_string_like(&other, "xs:Name")?;
                    let collapsed = collapse_xml_whitespace(text);
                    if !is_valid_name(&collapsed, true, true) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:Name"));
                    }
                    Ok(XdmAtomicValue::Name(collapsed.into_owned()))
                }
            },
            "NCName" => match a {
                XdmAtomicValue::NCName(s) => Ok(XdmAtomicValue::NCName(s)),
                other => {
                    let text = Self::require_string_like(&other, "xs:NCName")?;
                    let collapsed = collapse_xml_whitespace(text);
                    if !is_valid_name(&collapsed, true, false) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:NCName"));
                    }
                    Ok(XdmAtomicValue::NCName(collapsed.into_owned()))
                }
            },
            "NMTOKEN" => match a {
                XdmAtomicValue::NMTOKEN(s) => Ok(XdmAtomicValue::NMTOKEN(s)),
                other => {
                    let text = Self::require_string_like(&other, "xs:NMTOKEN")?;
                    let collapsed = collapse_xml_whitespace(text);
                    if !is_valid_nmtoken(&collapsed) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:NMTOKEN"));
                    }
                    Ok(XdmAtomicValue::NMTOKEN(collapsed.into_owned()))
                }
            },
            "ID" => match a {
                XdmAtomicValue::Id(s) => Ok(XdmAtomicValue::Id(s)),
                other => {
                    let text = Self::require_string_like(&other, "xs:ID")?;
                    let collapsed = collapse_xml_whitespace(text);
                    if !is_valid_name(&collapsed, true, false) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:ID"));
                    }
                    Ok(XdmAtomicValue::Id(collapsed.into_owned()))
                }
            },
            "IDREF" => match a {
                XdmAtomicValue::IdRef(s) => Ok(XdmAtomicValue::IdRef(s)),
                other => {
                    let text = Self::require_string_like(&other, "xs:IDREF")?;
                    let collapsed = collapse_xml_whitespace(text);
                    if !is_valid_name(&collapsed, true, false) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:IDREF"));
                    }
                    Ok(XdmAtomicValue::IdRef(collapsed.into_owned()))
                }
            },
            "ENTITY" => match a {
                XdmAtomicValue::Entity(s) => Ok(XdmAtomicValue::Entity(s)),
                other => {
                    let text = Self::require_string_like(&other, "xs:ENTITY")?;
                    let collapsed = collapse_xml_whitespace(text);
                    if !is_valid_name(&collapsed, true, false) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:ENTITY"));
                    }
                    Ok(XdmAtomicValue::Entity(collapsed.into_owned()))
                }
            },
            "date" => match a {
                XdmAtomicValue::Date { date, tz } => Ok(XdmAtomicValue::Date { date, tz }),
                other => {
                    let text = Self::require_string_like(&other, "xs:date")?;
                    match Self::parse_date(text) {
                        Ok(v) => Ok(v),
                        Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid date")),
                    }
                }
            },
            "dateTime" => match a {
                XdmAtomicValue::DateTime(dt) => Ok(XdmAtomicValue::DateTime(dt)),
                other => {
                    let text = Self::require_string_like(&other, "xs:dateTime")?;
                    match Self::parse_date_time(text) {
                        Ok(v) => Ok(v),
                        Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid dateTime")),
                    }
                }
            },
            "time" => match a {
                XdmAtomicValue::Time { time, tz } => Ok(XdmAtomicValue::Time { time, tz }),
                other => {
                    let text = Self::require_string_like(&other, "xs:time")?;
                    match Self::parse_time(text) {
                        Ok(v) => Ok(v),
                        Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid time")),
                    }
                }
            },
            "yearMonthDuration" => match a {
                XdmAtomicValue::YearMonthDuration(m) => Ok(XdmAtomicValue::YearMonthDuration(m)),
                other => {
                    let text = Self::require_string_like(&other, "xs:yearMonthDuration")?;
                    Self::parse_year_month_duration(text)
                        .map_err(|()| Error::from_code(ErrorCode::FORG0001, "invalid yearMonthDuration"))
                }
            },
            "dayTimeDuration" => match a {
                XdmAtomicValue::DayTimeDuration(m) => Ok(XdmAtomicValue::DayTimeDuration(m)),
                other => {
                    let text = Self::require_string_like(&other, "xs:dayTimeDuration")?;
                    Self::parse_day_time_duration(text)
                        .map_err(|()| Error::from_code(ErrorCode::FORG0001, "invalid dayTimeDuration"))
                }
            },
            "gYear" => match a {
                XdmAtomicValue::GYear { year, tz } => Ok(XdmAtomicValue::GYear { year, tz }),
                other => {
                    let text = Self::require_string_like(&other, "xs:gYear")?;
                    let (year, tz) =
                        parse_g_year(text).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gYear"))?;
                    Ok(XdmAtomicValue::GYear { year, tz })
                }
            },
            "gYearMonth" => match a {
                XdmAtomicValue::GYearMonth { year, month, tz } => Ok(XdmAtomicValue::GYearMonth { year, month, tz }),
                other => {
                    let text = Self::require_string_like(&other, "xs:gYearMonth")?;
                    let (year, month, tz) = parse_g_year_month(text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gYearMonth"))?;
                    Ok(XdmAtomicValue::GYearMonth { year, month, tz })
                }
            },
            "gMonth" => match a {
                XdmAtomicValue::GMonth { month, tz } => Ok(XdmAtomicValue::GMonth { month, tz }),
                other => {
                    let text = Self::require_string_like(&other, "xs:gMonth")?;
                    let (month, tz) =
                        parse_g_month(text).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gMonth"))?;
                    Ok(XdmAtomicValue::GMonth { month, tz })
                }
            },
            "gMonthDay" => match a {
                XdmAtomicValue::GMonthDay { month, day, tz } => Ok(XdmAtomicValue::GMonthDay { month, day, tz }),
                other => {
                    let text = Self::require_string_like(&other, "xs:gMonthDay")?;
                    let (month, day, tz) = parse_g_month_day(text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gMonthDay"))?;
                    Ok(XdmAtomicValue::GMonthDay { month, day, tz })
                }
            },
            "gDay" => match a {
                XdmAtomicValue::GDay { day, tz } => Ok(XdmAtomicValue::GDay { day, tz }),
                other => {
                    let text = Self::require_string_like(&other, "xs:gDay")?;
                    let (day, tz) =
                        parse_g_day(text).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gDay"))?;
                    Ok(XdmAtomicValue::GDay { day, tz })
                }
            },
            _ => Err(Error::not_implemented("cast target type")),
        }
    }

    fn parse_integer_string(text: &str, target: &str) -> Result<i128, Error> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(Error::from_code(ErrorCode::FORG0001, format!("cannot cast to {target}: empty string")));
        }
        trimmed
            .parse::<i128>()
            .map_err(|_| Error::from_code(ErrorCode::FORG0001, format!("invalid lexical for {target}")))
    }

    // The value is finite, integral and range-checked against the i128 bounds (as f64) first;
    // `as` saturates the one remaining edge (exactly 2^127) to i128::MAX.
    #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]
    fn float_to_integer(value: f64, target: &str) -> Result<i128, Error> {
        if !value.is_finite() {
            return Err(Error::from_code(ErrorCode::FOCA0001, format!("{target} overflow")));
        }
        if value.fract() != 0.0 {
            return Err(Error::from_code(ErrorCode::FOCA0001, format!("non-integer value for {target}")));
        }
        if value < i128::MIN as f64 || value > i128::MAX as f64 {
            return Err(Error::from_code(ErrorCode::FOCA0001, format!("{target} overflow")));
        }
        Ok(value as i128)
    }

    fn integer_from_atomic(atom: &XdmAtomicValue, target: &str) -> Result<i128, Error> {
        use XdmAtomicValue::{Decimal, Double, Float};

        // Handle all integer subtypes via centralized as_i128()
        if let Some(v) = atom.as_i128() {
            return Ok(v);
        }

        match atom {
            Decimal(d) => {
                use rust_decimal::prelude::ToPrimitive;
                Self::float_to_integer(d.to_f64().unwrap_or(f64::NAN), target)
            }
            Double(d) => Self::float_to_integer(*d, target),
            Float(f) => Self::float_to_integer(f64::from(*f), target),
            other => {
                if let Some(text) = string_like_value(other) {
                    Self::parse_integer_string(text, target)
                } else {
                    Err(Error::from_code(ErrorCode::FORG0001, format!("cannot cast to {target}")))
                }
            }
        }
    }

    fn unsigned_from_atomic(atom: &XdmAtomicValue, target: &str) -> Result<u128, Error> {
        match atom {
            XdmAtomicValue::UnsignedLong(v)
            | XdmAtomicValue::NonNegativeInteger(v)
            | XdmAtomicValue::PositiveInteger(v) => Ok(u128::from(*v)),
            XdmAtomicValue::UnsignedInt(v) => Ok(u128::from(*v)),
            XdmAtomicValue::UnsignedShort(v) => Ok(u128::from(*v)),
            XdmAtomicValue::UnsignedByte(v) => Ok(u128::from(*v)),
            other => {
                let signed = Self::integer_from_atomic(other, target)?;
                // Only negative values fail the conversion.
                u128::try_from(signed).map_err(|_| {
                    Error::from_code(ErrorCode::FORG0001, format!("negative value not allowed for {target}"))
                })
            }
        }
    }

    /// Check `value` against `min..=max` and narrow it to the target storage type `T`.
    fn ensure_range_i128<T: TryFrom<i128>>(value: i128, min: i128, max: i128, target: &str) -> Result<T, Error> {
        let out_of_range = || Error::from_code(ErrorCode::FORG0001, format!("value out of range for {target}"));
        if value < min || value > max {
            return Err(out_of_range());
        }
        T::try_from(value).map_err(|_| out_of_range())
    }

    /// Check `value` against `min..=max` and narrow it to the target storage type `T`.
    fn ensure_range_u128<T: TryFrom<u128>>(value: u128, min: u128, max: u128, target: &str) -> Result<T, Error> {
        let out_of_range = || Error::from_code(ErrorCode::FORG0001, format!("value out of range for {target}"));
        if value < min || value > max {
            return Err(out_of_range());
        }
        T::try_from(value).map_err(|_| out_of_range())
    }

    pub(crate) fn require_string_like<'a>(atom: &'a XdmAtomicValue, target: &str) -> Result<&'a str, Error> {
        string_like_value(atom).ok_or_else(|| Error::from_code(ErrorCode::FORG0001, format!("cannot cast to {target}")))
    }

    // Helper: best-effort canonical string form for debugging / fallback casts
    pub(crate) fn atomic_to_string(a: &XdmAtomicValue) -> String {
        format!("{a:?}")
    }

    pub(crate) fn parse_date(s: &str) -> Result<XdmAtomicValue, crate::util::temporal::TemporalErr> {
        let (d, tz) = crate::util::temporal::parse_date_lex(s)?;
        Ok(XdmAtomicValue::Date { date: d, tz })
    }

    pub(crate) fn parse_time(s: &str) -> Result<XdmAtomicValue, crate::util::temporal::TemporalErr> {
        let (t, tz) = crate::util::temporal::parse_time_lex(s)?;
        Ok(XdmAtomicValue::Time { time: t, tz })
    }

    pub(crate) fn parse_date_time(s: &str) -> Result<XdmAtomicValue, crate::util::temporal::TemporalErr> {
        let (d, t, tz) = crate::util::temporal::parse_date_time_lex(s)?;
        let dt = crate::util::temporal::build_naive_datetime(d, t, tz);
        Ok(XdmAtomicValue::DateTime(dt))
    }

    pub(crate) fn parse_year_month_duration(s: &str) -> Result<XdmAtomicValue, ()> {
        // PnYnM pattern subset
        if !s.starts_with('P') {
            return Err(());
        }
        let body = &s[1..];
        let mut years = 0;
        let mut months = 0;
        let mut cur = String::new();
        for ch in body.chars() {
            if ch.is_ascii_digit() {
                cur.push(ch);
                continue;
            }
            match ch {
                'Y' => {
                    years = cur.parse::<i32>().map_err(|_| ())?;
                    cur.clear();
                }
                'M' => {
                    months = cur.parse::<i32>().map_err(|_| ())?;
                    cur.clear();
                }
                _ => return Err(()),
            }
        }
        if !cur.is_empty() {
            return Err(());
        }
        Ok(XdmAtomicValue::YearMonthDuration(years * 12 + months))
    }

    pub(crate) fn parse_day_time_duration(s: &str) -> Result<XdmAtomicValue, ()> {
        // PnDTnHnMnS subset (strict: at least one component)
        if !s.starts_with('P') {
            return Err(());
        }
        let body = &s[1..];
        let mut days = 0i64;
        let mut hours = 0i64;
        let mut mins = 0i64;
        let mut secs = 0i64;
        let mut cur = String::new();
        let mut time_part = false;
        let mut saw_component = false;
        for ch in body.chars() {
            if ch == 'T' {
                time_part = true;
                continue;
            }
            if ch.is_ascii_digit() {
                cur.push(ch);
                continue;
            }
            match ch {
                'D' => {
                    days = cur.parse::<i64>().map_err(|_| ())?;
                    cur.clear();
                    saw_component = true;
                }
                'H' => {
                    hours = cur.parse::<i64>().map_err(|_| ())?;
                    cur.clear();
                    saw_component = true;
                }
                'M' if time_part => {
                    mins = cur.parse::<i64>().map_err(|_| ())?;
                    cur.clear();
                    saw_component = true;
                }
                'S' => {
                    secs = cur.parse::<i64>().map_err(|_| ())?;
                    cur.clear();
                    saw_component = true;
                }
                _ => return Err(()),
            }
        }
        if !cur.is_empty() {
            return Err(());
        }
        if !saw_component {
            return Err(());
        } // reject bare "PT" (no component)
        let total = days * 86400 + hours * 3600 + mins * 60 + secs;
        Ok(XdmAtomicValue::DayTimeDuration(total))
    }
}
