//! XSD type casting for the XPath evaluator.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

use crate::engine::functions::parse_qname_lexical;
use crate::engine::runtime::{Error, ErrorCode};
use crate::model::XdmNode;
use crate::util::temporal::{parse_g_day, parse_g_month, parse_g_month_day, parse_g_year, parse_g_year_month};
use crate::xdm::{ExpandedName, XdmAtomicValue};

use super::xml_helpers::{
    collapse_xml_whitespace, decode_hex, encode_hex_upper, is_valid_language, is_valid_name,
    is_valid_nmtoken, replace_xml_whitespace, string_like_value,
};
use super::Vm;

impl<N: 'static + XdmNode + Clone> Vm<N> {
    pub(crate) fn cast_atomic(
        &self,
        a: XdmAtomicValue,
        target: &ExpandedName,
    ) -> Result<XdmAtomicValue, Error> {
        // Namespace check: only xs:* types supported
        if let Some(ns) = &target.ns_uri {
            let xs_ns = crate::consts::XS;
            if ns.as_str() != xs_ns {
                return Err(Error::from_code(
                    ErrorCode::XPTY0004,
                    "unsupported cast target namespace",
                ));
            }
        }
        match target.local.as_str() {
            "anyAtomicType" => Ok(a),
            "string" => {
                let text = string_like_value(&a).unwrap_or_else(|| self.atomic_to_string(&a));
                Ok(XdmAtomicValue::String(text))
            }
            "untypedAtomic" => {
                let text = string_like_value(&a).unwrap_or_else(|| self.atomic_to_string(&a));
                Ok(XdmAtomicValue::UntypedAtomic(text))
            }
            "boolean" => match a {
                XdmAtomicValue::Boolean(b) => Ok(XdmAtomicValue::Boolean(b)),
                XdmAtomicValue::Integer(i) => Ok(XdmAtomicValue::Boolean(i != 0)),
                XdmAtomicValue::Decimal(d) => Ok(XdmAtomicValue::Boolean(!d.is_zero())),
                XdmAtomicValue::Double(d) => Ok(XdmAtomicValue::Boolean(d != 0.0 && !d.is_nan())),
                XdmAtomicValue::Float(f) => Ok(XdmAtomicValue::Boolean(f != 0.0 && !f.is_nan())),
                other => {
                    let text = self.require_string_like(&other, "xs:boolean")?;
                    let b = match text.as_str() {
                        "true" | "1" => true,
                        "false" | "0" => false,
                        _ => {
                            return Err(Error::from_code(
                                ErrorCode::FORG0001,
                                "invalid boolean lexical form",
                            ));
                        }
                    };
                    Ok(XdmAtomicValue::Boolean(b))
                }
            },
            "integer" => match a {
                XdmAtomicValue::Integer(v) => Ok(XdmAtomicValue::Integer(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:integer")?;
                    let bounded =
                        self.ensure_range_i128(value, i64::MIN as i128, i64::MAX as i128, "xs:integer")?;
                    Ok(XdmAtomicValue::Integer(bounded as i64))
                }
            },
            "decimal" => match a {
                XdmAtomicValue::Decimal(d) => Ok(XdmAtomicValue::Decimal(d)),
                XdmAtomicValue::Integer(i) => Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i))),
                XdmAtomicValue::Long(i) => Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i))),
                XdmAtomicValue::Int(i) => Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i))),
                XdmAtomicValue::Short(i) => {
                    Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i as i64)))
                }
                XdmAtomicValue::Byte(i) => {
                    Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i as i64)))
                }
                XdmAtomicValue::NonPositiveInteger(i) => {
                    Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i)))
                }
                XdmAtomicValue::NegativeInteger(i) => {
                    Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i)))
                }
                XdmAtomicValue::NonNegativeInteger(i) => {
                    Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i)))
                }
                XdmAtomicValue::PositiveInteger(i) => {
                    Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i)))
                }
                XdmAtomicValue::UnsignedLong(i) => {
                    Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i)))
                }
                XdmAtomicValue::UnsignedInt(i) => {
                    Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i)))
                }
                XdmAtomicValue::UnsignedShort(i) => {
                    Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i as u64)))
                }
                XdmAtomicValue::UnsignedByte(i) => {
                    Ok(XdmAtomicValue::Decimal(rust_decimal::Decimal::from(i as u64)))
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
                    let text = self.require_string_like(&other, "xs:decimal")?;
                    let trimmed = text.trim();
                    if trimmed.eq_ignore_ascii_case("nan")
                        || trimmed.eq_ignore_ascii_case("inf")
                        || trimmed.eq_ignore_ascii_case("-inf")
                    {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"));
                    }
                    use std::str::FromStr;
                    let value = rust_decimal::Decimal::from_str(trimmed)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"))?;
                    Ok(XdmAtomicValue::Decimal(value))
                }
            },
            "double" => match a {
                XdmAtomicValue::Double(d) => Ok(XdmAtomicValue::Double(d)),
                XdmAtomicValue::Float(f) => Ok(XdmAtomicValue::Double(f as f64)),
                XdmAtomicValue::Decimal(d) => {
                    use rust_decimal::prelude::ToPrimitive;
                    Ok(XdmAtomicValue::Double(d.to_f64().unwrap_or(f64::NAN)))
                }
                XdmAtomicValue::Integer(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::Long(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::Int(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::Short(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::Byte(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::NonPositiveInteger(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::NegativeInteger(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::NonNegativeInteger(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::PositiveInteger(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::UnsignedLong(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::UnsignedInt(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::UnsignedShort(i) => Ok(XdmAtomicValue::Double(i as f64)),
                XdmAtomicValue::UnsignedByte(i) => Ok(XdmAtomicValue::Double(i as f64)),
                other => {
                    let text = self.require_string_like(&other, "xs:double")?;
                    let trimmed = text.trim();
                    let value = match trimmed {
                        "NaN" | "nan" => f64::NAN,
                        "INF" | "inf" => f64::INFINITY,
                        "-INF" | "-inf" => f64::NEG_INFINITY,
                        _ => trimmed
                            .parse()
                            .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:double"))?,
                    };
                    Ok(XdmAtomicValue::Double(value))
                }
            },
            "float" => match a {
                XdmAtomicValue::Float(f) => Ok(XdmAtomicValue::Float(f)),
                XdmAtomicValue::Double(d) => Ok(XdmAtomicValue::Float(d as f32)),
                XdmAtomicValue::Decimal(d) => {
                    use rust_decimal::prelude::ToPrimitive;
                    Ok(XdmAtomicValue::Float(d.to_f32().unwrap_or(f32::NAN)))
                }
                XdmAtomicValue::Integer(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::Long(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::Int(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::Short(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::Byte(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::NonPositiveInteger(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::NegativeInteger(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::NonNegativeInteger(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::PositiveInteger(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::UnsignedLong(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::UnsignedInt(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::UnsignedShort(i) => Ok(XdmAtomicValue::Float(i as f32)),
                XdmAtomicValue::UnsignedByte(i) => Ok(XdmAtomicValue::Float(i as f32)),
                other => {
                    let text = self.require_string_like(&other, "xs:float")?;
                    let trimmed = text.trim();
                    let value = match trimmed {
                        "NaN" | "nan" => f32::NAN,
                        "INF" | "inf" => f32::INFINITY,
                        "-INF" | "-inf" => f32::NEG_INFINITY,
                        _ => trimmed
                            .parse()
                            .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:float"))?,
                    };
                    Ok(XdmAtomicValue::Float(value))
                }
            },
            "long" => match a {
                XdmAtomicValue::Long(v) => Ok(XdmAtomicValue::Long(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:long")?;
                    let bounded =
                        self.ensure_range_i128(value, i64::MIN as i128, i64::MAX as i128, "xs:long")?;
                    Ok(XdmAtomicValue::Long(bounded as i64))
                }
            },
            "int" => match a {
                XdmAtomicValue::Int(v) => Ok(XdmAtomicValue::Int(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:int")?;
                    let bounded =
                        self.ensure_range_i128(value, i32::MIN as i128, i32::MAX as i128, "xs:int")?;
                    Ok(XdmAtomicValue::Int(bounded as i32))
                }
            },
            "short" => match a {
                XdmAtomicValue::Short(v) => Ok(XdmAtomicValue::Short(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:short")?;
                    let bounded =
                        self.ensure_range_i128(value, i16::MIN as i128, i16::MAX as i128, "xs:short")?;
                    Ok(XdmAtomicValue::Short(bounded as i16))
                }
            },
            "byte" => match a {
                XdmAtomicValue::Byte(v) => Ok(XdmAtomicValue::Byte(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:byte")?;
                    let bounded =
                        self.ensure_range_i128(value, i8::MIN as i128, i8::MAX as i128, "xs:byte")?;
                    Ok(XdmAtomicValue::Byte(bounded as i8))
                }
            },
            "unsignedLong" => match a {
                XdmAtomicValue::UnsignedLong(v) => Ok(XdmAtomicValue::UnsignedLong(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:unsignedLong")?;
                    let bounded =
                        self.ensure_range_u128(value, 0, u64::MAX as u128, "xs:unsignedLong")?;
                    Ok(XdmAtomicValue::UnsignedLong(bounded as u64))
                }
            },
            "unsignedInt" => match a {
                XdmAtomicValue::UnsignedInt(v) => Ok(XdmAtomicValue::UnsignedInt(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:unsignedInt")?;
                    let bounded =
                        self.ensure_range_u128(value, 0, u32::MAX as u128, "xs:unsignedInt")?;
                    Ok(XdmAtomicValue::UnsignedInt(bounded as u32))
                }
            },
            "unsignedShort" => match a {
                XdmAtomicValue::UnsignedShort(v) => Ok(XdmAtomicValue::UnsignedShort(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:unsignedShort")?;
                    let bounded =
                        self.ensure_range_u128(value, 0, u16::MAX as u128, "xs:unsignedShort")?;
                    Ok(XdmAtomicValue::UnsignedShort(bounded as u16))
                }
            },
            "unsignedByte" => match a {
                XdmAtomicValue::UnsignedByte(v) => Ok(XdmAtomicValue::UnsignedByte(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:unsignedByte")?;
                    let bounded =
                        self.ensure_range_u128(value, 0, u8::MAX as u128, "xs:unsignedByte")?;
                    Ok(XdmAtomicValue::UnsignedByte(bounded as u8))
                }
            },
            "nonPositiveInteger" => match a {
                XdmAtomicValue::NonPositiveInteger(v) => Ok(XdmAtomicValue::NonPositiveInteger(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:nonPositiveInteger")?;
                    if value > 0 {
                        return Err(Error::from_code(
                            ErrorCode::FORG0001,
                            "value must be <= 0 for xs:nonPositiveInteger",
                        ));
                    }
                    let bounded =
                        self.ensure_range_i128(value, i64::MIN as i128, 0, "xs:nonPositiveInteger")?;
                    Ok(XdmAtomicValue::NonPositiveInteger(bounded as i64))
                }
            },
            "negativeInteger" => match a {
                XdmAtomicValue::NegativeInteger(v) => Ok(XdmAtomicValue::NegativeInteger(v)),
                other => {
                    let value = self.integer_from_atomic(&other, "xs:negativeInteger")?;
                    if value >= 0 {
                        return Err(Error::from_code(
                            ErrorCode::FORG0001,
                            "value must be < 0 for xs:negativeInteger",
                        ));
                    }
                    let bounded =
                        self.ensure_range_i128(value, i64::MIN as i128, -1, "xs:negativeInteger")?;
                    Ok(XdmAtomicValue::NegativeInteger(bounded as i64))
                }
            },
            "nonNegativeInteger" => match a {
                XdmAtomicValue::NonNegativeInteger(v) => Ok(XdmAtomicValue::NonNegativeInteger(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:nonNegativeInteger")?;
                    let bounded =
                        self.ensure_range_u128(value, 0, u64::MAX as u128, "xs:nonNegativeInteger")?;
                    Ok(XdmAtomicValue::NonNegativeInteger(bounded as u64))
                }
            },
            "positiveInteger" => match a {
                XdmAtomicValue::PositiveInteger(v) => Ok(XdmAtomicValue::PositiveInteger(v)),
                other => {
                    let value = self.unsigned_from_atomic(&other, "xs:positiveInteger")?;
                    if value == 0 {
                        return Err(Error::from_code(
                            ErrorCode::FORG0001,
                            "value must be > 0 for xs:positiveInteger",
                        ));
                    }
                    let bounded =
                        self.ensure_range_u128(value, 1, u64::MAX as u128, "xs:positiveInteger")?;
                    Ok(XdmAtomicValue::PositiveInteger(bounded as u64))
                }
            },
            "anyURI" => match a {
                XdmAtomicValue::AnyUri(uri) => Ok(XdmAtomicValue::AnyUri(uri)),
                other => {
                    let text = self.require_string_like(&other, "xs:anyURI")?;
                    Ok(XdmAtomicValue::AnyUri(text.trim().to_string()))
                }
            },
            "QName" => match a {
                XdmAtomicValue::QName {
                    ns_uri,
                    prefix,
                    local,
                } => Ok(XdmAtomicValue::QName {
                    ns_uri,
                    prefix,
                    local,
                }),
                other => {
                    let text = self.require_string_like(&other, "xs:QName")?;
                    let (prefix, local) = parse_qname_lexical(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid QName lexical"))?;
                    Ok(XdmAtomicValue::QName {
                        ns_uri: None,
                        prefix,
                        local,
                    })
                }
            },
            "NOTATION" => match a {
                XdmAtomicValue::Notation(s) => Ok(XdmAtomicValue::Notation(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:NOTATION")?;
                    if parse_qname_lexical(&text).is_err() {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:NOTATION"));
                    }
                    Ok(XdmAtomicValue::Notation(text))
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
                    let text = self.require_string_like(&other, "xs:base64Binary")?;
                    let normalized: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                    if BASE64_STANDARD.decode(normalized.as_bytes()).is_err() {
                        return Err(Error::from_code(
                            ErrorCode::FORG0001,
                            "invalid xs:base64Binary",
                        ));
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
                    let text = self.require_string_like(&other, "xs:hexBinary")?;
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
                    let text = self.require_string_like(&other, "xs:normalizedString")?;
                    let normalized = replace_xml_whitespace(&text);
                    Ok(XdmAtomicValue::NormalizedString(normalized))
                }
            },
            "token" => match a {
                XdmAtomicValue::Token(s) => Ok(XdmAtomicValue::Token(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:token")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    Ok(XdmAtomicValue::Token(collapsed))
                }
            },
            "language" => match a {
                XdmAtomicValue::Language(s) => Ok(XdmAtomicValue::Language(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:language")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_language(&collapsed) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:language"));
                    }
                    Ok(XdmAtomicValue::Language(collapsed))
                }
            },
            "Name" => match a {
                XdmAtomicValue::Name(s) => Ok(XdmAtomicValue::Name(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:Name")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_name(&collapsed, true, true) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:Name"));
                    }
                    Ok(XdmAtomicValue::Name(collapsed))
                }
            },
            "NCName" => match a {
                XdmAtomicValue::NCName(s) => Ok(XdmAtomicValue::NCName(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:NCName")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_name(&collapsed, true, false) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:NCName"));
                    }
                    Ok(XdmAtomicValue::NCName(collapsed))
                }
            },
            "NMTOKEN" => match a {
                XdmAtomicValue::NMTOKEN(s) => Ok(XdmAtomicValue::NMTOKEN(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:NMTOKEN")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_nmtoken(&collapsed) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:NMTOKEN"));
                    }
                    Ok(XdmAtomicValue::NMTOKEN(collapsed))
                }
            },
            "ID" => match a {
                XdmAtomicValue::Id(s) => Ok(XdmAtomicValue::Id(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:ID")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_name(&collapsed, true, false) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:ID"));
                    }
                    Ok(XdmAtomicValue::Id(collapsed))
                }
            },
            "IDREF" => match a {
                XdmAtomicValue::IdRef(s) => Ok(XdmAtomicValue::IdRef(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:IDREF")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_name(&collapsed, true, false) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:IDREF"));
                    }
                    Ok(XdmAtomicValue::IdRef(collapsed))
                }
            },
            "ENTITY" => match a {
                XdmAtomicValue::Entity(s) => Ok(XdmAtomicValue::Entity(s)),
                other => {
                    let text = self.require_string_like(&other, "xs:ENTITY")?;
                    let collapsed = collapse_xml_whitespace(&text);
                    if !is_valid_name(&collapsed, true, false) {
                        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:ENTITY"));
                    }
                    Ok(XdmAtomicValue::Entity(collapsed))
                }
            },
            "date" => match a {
                XdmAtomicValue::Date { date, tz } => Ok(XdmAtomicValue::Date { date, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:date")?;
                    match self.parse_date(&text) {
                        Ok(v) => Ok(v),
                        Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid date")),
                    }
                }
            },
            "dateTime" => match a {
                XdmAtomicValue::DateTime(dt) => Ok(XdmAtomicValue::DateTime(dt)),
                other => {
                    let text = self.require_string_like(&other, "xs:dateTime")?;
                    match self.parse_date_time(&text) {
                        Ok(v) => Ok(v),
                        Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid dateTime")),
                    }
                }
            },
            "time" => match a {
                XdmAtomicValue::Time { time, tz } => Ok(XdmAtomicValue::Time { time, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:time")?;
                    match self.parse_time(&text) {
                        Ok(v) => Ok(v),
                        Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid time")),
                    }
                }
            },
            "yearMonthDuration" => match a {
                XdmAtomicValue::YearMonthDuration(m) => Ok(XdmAtomicValue::YearMonthDuration(m)),
                other => {
                    let text = self.require_string_like(&other, "xs:yearMonthDuration")?;
                    self.parse_year_month_duration(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid yearMonthDuration"))
                }
            },
            "dayTimeDuration" => match a {
                XdmAtomicValue::DayTimeDuration(m) => Ok(XdmAtomicValue::DayTimeDuration(m)),
                other => {
                    let text = self.require_string_like(&other, "xs:dayTimeDuration")?;
                    self.parse_day_time_duration(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid dayTimeDuration"))
                }
            },
            "gYear" => match a {
                XdmAtomicValue::GYear { year, tz } => Ok(XdmAtomicValue::GYear { year, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:gYear")?;
                    let (year, tz) = parse_g_year(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gYear"))?;
                    Ok(XdmAtomicValue::GYear { year, tz })
                }
            },
            "gYearMonth" => match a {
                XdmAtomicValue::GYearMonth { year, month, tz } => {
                    Ok(XdmAtomicValue::GYearMonth { year, month, tz })
                }
                other => {
                    let text = self.require_string_like(&other, "xs:gYearMonth")?;
                    let (year, month, tz) = parse_g_year_month(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gYearMonth"))?;
                    Ok(XdmAtomicValue::GYearMonth { year, month, tz })
                }
            },
            "gMonth" => match a {
                XdmAtomicValue::GMonth { month, tz } => Ok(XdmAtomicValue::GMonth { month, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:gMonth")?;
                    let (month, tz) = parse_g_month(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gMonth"))?;
                    Ok(XdmAtomicValue::GMonth { month, tz })
                }
            },
            "gMonthDay" => match a {
                XdmAtomicValue::GMonthDay { month, day, tz } => {
                    Ok(XdmAtomicValue::GMonthDay { month, day, tz })
                }
                other => {
                    let text = self.require_string_like(&other, "xs:gMonthDay")?;
                    let (month, day, tz) = parse_g_month_day(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gMonthDay"))?;
                    Ok(XdmAtomicValue::GMonthDay { month, day, tz })
                }
            },
            "gDay" => match a {
                XdmAtomicValue::GDay { day, tz } => Ok(XdmAtomicValue::GDay { day, tz }),
                other => {
                    let text = self.require_string_like(&other, "xs:gDay")?;
                    let (day, tz) = parse_g_day(&text)
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gDay"))?;
                    Ok(XdmAtomicValue::GDay { day, tz })
                }
            },
            _ => Err(Error::not_implemented("cast target type")),
        }
    }

    fn parse_integer_string(&self, text: &str, target: &str) -> Result<i128, Error> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return Err(Error::from_code(
                ErrorCode::FORG0001,
                format!("cannot cast to {target}: empty string"),
            ));
        }
        trimmed
            .parse::<i128>()
            .map_err(|_| Error::from_code(ErrorCode::FORG0001, format!("invalid lexical for {target}")))
    }

    fn float_to_integer(&self, value: f64, target: &str) -> Result<i128, Error> {
        if !value.is_finite() {
            return Err(Error::from_code(
                ErrorCode::FOCA0001,
                format!("{target} overflow"),
            ));
        }
        if value.fract() != 0.0 {
            return Err(Error::from_code(
                ErrorCode::FOCA0001,
                format!("non-integer value for {target}"),
            ));
        }
        if value < i128::MIN as f64 || value > i128::MAX as f64 {
            return Err(Error::from_code(
                ErrorCode::FOCA0001,
                format!("{target} overflow"),
            ));
        }
        Ok(value as i128)
    }

    fn integer_from_atomic(&self, atom: &XdmAtomicValue, target: &str) -> Result<i128, Error> {
        use XdmAtomicValue::*;
        match atom {
            Integer(v) => Ok(*v as i128),
            Long(v) => Ok(*v as i128),
            Int(v) => Ok(*v as i128),
            Short(v) => Ok(*v as i128),
            Byte(v) => Ok(*v as i128),
            NonPositiveInteger(v) => Ok(*v as i128),
            NegativeInteger(v) => Ok(*v as i128),
            UnsignedLong(v) => Ok(*v as i128),
            UnsignedInt(v) => Ok(*v as i128),
            UnsignedShort(v) => Ok(*v as i128),
            UnsignedByte(v) => Ok(*v as i128),
            NonNegativeInteger(v) => Ok(*v as i128),
            PositiveInteger(v) => Ok(*v as i128),
            Decimal(d) => {
                use rust_decimal::prelude::ToPrimitive;
                self.float_to_integer(d.to_f64().unwrap_or(f64::NAN), target)
            }
            Double(d) => self.float_to_integer(*d, target),
            Float(f) => self.float_to_integer(*f as f64, target),
            other => {
                if let Some(text) = string_like_value(other) {
                    self.parse_integer_string(&text, target)
                } else {
                    Err(Error::from_code(
                        ErrorCode::FORG0001,
                        format!("cannot cast to {target}"),
                    ))
                }
            }
        }
    }

    fn unsigned_from_atomic(&self, atom: &XdmAtomicValue, target: &str) -> Result<u128, Error> {
        match atom {
            XdmAtomicValue::UnsignedLong(v) => Ok(*v as u128),
            XdmAtomicValue::UnsignedInt(v) => Ok(*v as u128),
            XdmAtomicValue::UnsignedShort(v) => Ok(*v as u128),
            XdmAtomicValue::UnsignedByte(v) => Ok(*v as u128),
            XdmAtomicValue::NonNegativeInteger(v) => Ok(*v as u128),
            XdmAtomicValue::PositiveInteger(v) => Ok(*v as u128),
            other => {
                let signed = self.integer_from_atomic(other, target)?;
                if signed < 0 {
                    Err(Error::from_code(
                        ErrorCode::FORG0001,
                        format!("negative value not allowed for {target}"),
                    ))
                } else {
                    Ok(signed as u128)
                }
            }
        }
    }

    fn ensure_range_i128(
        &self,
        value: i128,
        min: i128,
        max: i128,
        target: &str,
    ) -> Result<i128, Error> {
        if value < min || value > max {
            Err(Error::from_code(
                ErrorCode::FORG0001,
                format!("value out of range for {target}"),
            ))
        } else {
            Ok(value)
        }
    }

    fn ensure_range_u128(
        &self,
        value: u128,
        min: u128,
        max: u128,
        target: &str,
    ) -> Result<u128, Error> {
        if value < min || value > max {
            Err(Error::from_code(
                ErrorCode::FORG0001,
                format!("value out of range for {target}"),
            ))
        } else {
            Ok(value)
        }
    }

    pub(crate) fn require_string_like(
        &self,
        atom: &XdmAtomicValue,
        target: &str,
    ) -> Result<String, Error> {
        string_like_value(atom)
            .ok_or_else(|| Error::from_code(ErrorCode::FORG0001, format!("cannot cast to {target}")))
    }

    // Helper: best-effort canonical string form for debugging / fallback casts
    pub(crate) fn atomic_to_string(&self, a: &XdmAtomicValue) -> String {
        format!("{:?}", a)
    }

    pub(crate) fn parse_date(
        &self,
        s: &str,
    ) -> Result<XdmAtomicValue, crate::util::temporal::TemporalErr> {
        let (d, tz) = crate::util::temporal::parse_date_lex(s)?;
        Ok(XdmAtomicValue::Date { date: d, tz })
    }

    pub(crate) fn parse_time(
        &self,
        s: &str,
    ) -> Result<XdmAtomicValue, crate::util::temporal::TemporalErr> {
        let (t, tz) = crate::util::temporal::parse_time_lex(s)?;
        Ok(XdmAtomicValue::Time { time: t, tz })
    }

    pub(crate) fn parse_date_time(
        &self,
        s: &str,
    ) -> Result<XdmAtomicValue, crate::util::temporal::TemporalErr> {
        let (d, t, tz) = crate::util::temporal::parse_date_time_lex(s)?;
        let dt = crate::util::temporal::build_naive_datetime(d, t, tz);
        Ok(XdmAtomicValue::DateTime(dt))
    }

    pub(crate) fn parse_year_month_duration(&self, s: &str) -> Result<XdmAtomicValue, ()> {
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

    pub(crate) fn parse_day_time_duration(&self, s: &str) -> Result<XdmAtomicValue, ()> {
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
                'M' => {
                    if time_part {
                        mins = cur.parse::<i64>().map_err(|_| ())?;
                        cur.clear();
                        saw_component = true;
                    } else {
                        return Err(());
                    }
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
