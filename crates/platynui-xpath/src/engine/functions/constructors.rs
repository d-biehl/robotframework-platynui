use super::common::{
    collapse_whitespace, int_subtype_i64, is_valid_language, item_to_string,
    parse_day_time_duration_secs, parse_duration_lexical, parse_qname_lexical,
    parse_year_month_duration_months, replace_whitespace, str_name_like, uint_subtype_u128,
};
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};
use base64::Engine as _;
use crate::util::temporal::{
    parse_g_day, parse_g_month, parse_g_month_day, parse_g_year, parse_g_year_month,
};

pub(super) fn integer_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    let s = item_to_string(&args[0]);
    let i: i64 = s
        .parse()
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid integer"))?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(i))])
}

pub(super) fn xs_string_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(
        item_to_string(&args[0]),
    ))])
}

pub(super) fn xs_untyped_atomic_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s))])
}

pub(super) fn xs_boolean_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    let v = match s.as_str() {
        "true" | "1" => true,
        "false" | "0" => false,
        _ => return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:boolean")),
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(v))])
}

pub(super) fn xs_integer_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    let s_trim = s.trim();
    if s_trim.is_empty() {
        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:integer"));
    }
    if (s_trim.contains('.') || s_trim.contains('e') || s_trim.contains('E'))
        && let Ok(f) = s_trim.parse::<f64>()
        && (!f.is_finite() || f.fract() != 0.0)
    {
        return Err(Error::from_code(
            ErrorCode::FOCA0001,
            "fractional part in integer cast",
        ));
    }
    let i: i64 = s_trim
        .parse()
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:integer"))?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(i))])
}

pub(super) fn xs_decimal_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]).trim().to_string();
    if s.eq_ignore_ascii_case("nan")
        || s.eq_ignore_ascii_case("inf")
        || s.eq_ignore_ascii_case("-inf")
    {
        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"));
    }
    let v: f64 = s
        .parse()
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"))?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Decimal(v))])
}

pub(super) fn xs_double_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]).trim().to_string();
    let v = match s.as_str() {
        "NaN" => f64::NAN,
        "INF" => f64::INFINITY,
        "-INF" => f64::NEG_INFINITY,
        _ => s
            .parse()
            .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:double"))?,
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(v))])
}

pub(super) fn xs_float_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]).trim().to_string();
    let v = match s.as_str() {
        "NaN" => f32::NAN,
        "INF" => f32::INFINITY,
        "-INF" => f32::NEG_INFINITY,
        _ => s
            .parse()
            .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:float"))?,
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Float(v))])
}

pub(super) fn xs_any_uri_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = collapse_whitespace(&item_to_string(&args[0]));
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(s))])
}

pub(super) fn xs_qname_fn<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    let (prefix_opt, local) = parse_qname_lexical(&s)
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:QName"))?;
    let ns_uri = match prefix_opt.as_deref() {
        None => None,
        Some("xml") => Some(crate::consts::XML_URI.to_string()),
        Some(p) => ctx.static_ctx.namespaces.by_prefix.get(p).cloned(),
    };
    if prefix_opt.is_some() && ns_uri.is_none() {
        return Err(Error::from_code(
            ErrorCode::FORG0001,
            "unknown namespace prefix for QName",
        ));
    }
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::QName {
        ns_uri,
        prefix: prefix_opt,
        local,
    })])
}

pub(super) fn xs_base64_binary_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let raw = item_to_string(&args[0]);
    let norm: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    if base64::engine::general_purpose::STANDARD
        .decode(&norm)
        .is_err()
    {
        return Err(Error::from_code(
            ErrorCode::FORG0001,
            "invalid xs:base64Binary",
        ));
    }
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Base64Binary(norm))])
}

pub(super) fn xs_hex_binary_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let raw = item_to_string(&args[0]);
    let norm: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    if norm.len() % 2 != 0 || !norm.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::from_code(
            ErrorCode::FORG0001,
            "invalid xs:hexBinary",
        ));
    }
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::HexBinary(norm))])
}

pub(super) fn xs_datetime_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    match crate::util::temporal::parse_date_time_lex(&s) {
        Ok((d, t, tz)) => {
            let dt = crate::util::temporal::build_naive_datetime(d, t, tz);
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::DateTime(dt))])
        }
        Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:dateTime")),
    }
}

pub(super) fn xs_date_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    match crate::util::temporal::parse_date_lex(&s) {
        Ok((d, tz)) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Date { date: d, tz })]),
        Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:date")),
    }
}

pub(super) fn xs_time_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    match crate::util::temporal::parse_time_lex(&s) {
        Ok((t, tz)) => Ok(vec![XdmItem::Atomic(XdmAtomicValue::Time { time: t, tz })]),
        Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:time")),
    }
}

pub(super) fn xs_duration_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    let (months_opt, secs_opt) = parse_duration_lexical(&s)?;
    let value = match (months_opt, secs_opt) {
        (Some(m), None) => XdmAtomicValue::YearMonthDuration(m),
        (None, Some(sec)) => XdmAtomicValue::DayTimeDuration(sec),
        _ => {
            return Err(Error::from_code(
                ErrorCode::NYI0000,
                "mixed duration components are not supported",
            ))
        }
    };
    Ok(vec![XdmItem::Atomic(value)])
}

pub(super) fn xs_day_time_duration_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    let secs = parse_day_time_duration_secs(&s)
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:dayTimeDuration"))?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs))])
}

pub(super) fn xs_g_year_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    let (year, tz) = parse_g_year(&s)
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gYear"))?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::GYear { year, tz })])
}

pub(super) fn xs_g_year_month_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    let (year, month, tz) = parse_g_year_month(&s)
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gYearMonth"))?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::GYearMonth { year, month, tz })])
}

pub(super) fn xs_g_month_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    let (month, tz) = parse_g_month(&s)
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gMonth"))?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::GMonth { month, tz })])
}

pub(super) fn xs_g_month_day_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    let (month, day, tz) = parse_g_month_day(&s)
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gMonthDay"))?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::GMonthDay { month, day, tz })])
}

pub(super) fn xs_g_day_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    let (day, tz) = parse_g_day(&s)
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gDay"))?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::GDay { day, tz })])
}

pub(super) fn xs_year_month_duration_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    let months = parse_year_month_duration_months(&s)
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:yearMonthDuration"))?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(
        months,
    ))])
}

pub(super) fn xs_long_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    int_subtype_i64(args, i64::MIN, i64::MAX, XdmAtomicValue::Long)
}

pub(super) fn xs_int_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    int_subtype_i64(args, i32::MIN as i64, i32::MAX as i64, |v| {
        XdmAtomicValue::Int(v as i32)
    })
}

pub(super) fn xs_short_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    int_subtype_i64(args, i16::MIN as i64, i16::MAX as i64, |v| {
        XdmAtomicValue::Short(v as i16)
    })
}

pub(super) fn xs_byte_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    int_subtype_i64(args, i8::MIN as i64, i8::MAX as i64, |v| {
        XdmAtomicValue::Byte(v as i8)
    })
}

pub(super) fn xs_unsigned_long_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    uint_subtype_u128(args, 0, u64::MAX as u128, |v| {
        XdmAtomicValue::UnsignedLong(v as u64)
    })
}

pub(super) fn xs_unsigned_int_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    uint_subtype_u128(args, 0, u32::MAX as u128, |v| {
        XdmAtomicValue::UnsignedInt(v as u32)
    })
}

pub(super) fn xs_unsigned_short_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    uint_subtype_u128(args, 0, u16::MAX as u128, |v| {
        XdmAtomicValue::UnsignedShort(v as u16)
    })
}

pub(super) fn xs_unsigned_byte_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    uint_subtype_u128(args, 0, u8::MAX as u128, |v| {
        XdmAtomicValue::UnsignedByte(v as u8)
    })
}

pub(super) fn xs_non_positive_integer_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    int_subtype_i64(args, i64::MIN, 0, XdmAtomicValue::NonPositiveInteger)
}

pub(super) fn xs_negative_integer_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    int_subtype_i64(args, i64::MIN, -1, XdmAtomicValue::NegativeInteger)
}

pub(super) fn xs_non_negative_integer_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    uint_subtype_u128(args, 0, u64::MAX as u128, |v| {
        XdmAtomicValue::NonNegativeInteger(v as u64)
    })
}

pub(super) fn xs_positive_integer_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    uint_subtype_u128(args, 1, u64::MAX as u128, |v| {
        XdmAtomicValue::PositiveInteger(v as u64)
    })
}

pub(super) fn xs_normalized_string_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = replace_whitespace(&item_to_string(&args[0]));
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::NormalizedString(s))])
}

pub(super) fn xs_token_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = collapse_whitespace(&item_to_string(&args[0]));
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Token(s))])
}

pub(super) fn xs_language_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = collapse_whitespace(&item_to_string(&args[0]));
    if !is_valid_language(&s) {
        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:language"));
    }
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Language(s))])
}

pub(super) fn xs_name_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    str_name_like(args, true, true, XdmAtomicValue::Name)
}

pub(super) fn xs_ncname_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    str_name_like(args, true, false, XdmAtomicValue::NCName)
}

pub(super) fn xs_nmtoken_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    str_name_like(args, false, false, XdmAtomicValue::NMTOKEN)
}

pub(super) fn xs_id_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    str_name_like(args, true, false, XdmAtomicValue::Id)
}

pub(super) fn xs_idref_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    str_name_like(args, true, false, XdmAtomicValue::IdRef)
}

pub(super) fn xs_entity_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    str_name_like(args, true, false, XdmAtomicValue::Entity)
}

pub(super) fn xs_notation_fn<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequence<N>],
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(
            ErrorCode::FORG0006,
            "constructor expects at most one item",
        ));
    }
    let s = item_to_string(&args[0]);
    if parse_qname_lexical(&s).is_err() {
        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:NOTATION"));
    }
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Notation(s))])
}
