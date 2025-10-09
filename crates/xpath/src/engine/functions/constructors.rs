use super::common::{
    collapse_whitespace, int_subtype_i64, is_valid_language, item_to_string, parse_day_time_duration_secs,
    parse_duration_lexical, parse_qname_lexical, parse_year_month_duration_months, replace_whitespace, str_name_like,
    uint_subtype_u128,
};
use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::util::temporal::{parse_g_day, parse_g_month, parse_g_month_day, parse_g_year, parse_g_year_month};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence, XdmSequenceStream};
use base64::Engine as _;

pub(super) fn integer_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    let s = item_to_string(&seq);
    let i: i64 = s.parse().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid integer"))?;
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Integer(i))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_string_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let result = vec![XdmItem::Atomic(XdmAtomicValue::String(item_to_string(&seq)))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_untyped_atomic_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let result = vec![XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_boolean_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let v = match s.as_str() {
        "true" | "1" => true,
        "false" | "0" => false,
        _ => return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:boolean")),
    };
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Boolean(v))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_integer_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let s_trim = s.trim();
    if s_trim.is_empty() {
        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:integer"));
    }
    if (s_trim.contains('.') || s_trim.contains('e') || s_trim.contains('E'))
        && let Ok(f) = s_trim.parse::<f64>()
        && (!f.is_finite() || f.fract() != 0.0)
    {
        return Err(Error::from_code(ErrorCode::FOCA0001, "fractional part in integer cast"));
    }
    let i: i64 = s_trim.parse().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:integer"))?;
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Integer(i))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_decimal_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq).trim().to_string();
    if s.eq_ignore_ascii_case("nan") || s.eq_ignore_ascii_case("inf") || s.eq_ignore_ascii_case("-inf") {
        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"));
    }
    let v: f64 = s.parse().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:decimal"))?;
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Decimal(v))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_double_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq).trim().to_string();
    let v = match s.as_str() {
        "NaN" => f64::NAN,
        "INF" => f64::INFINITY,
        "-INF" => f64::NEG_INFINITY,
        _ => s.parse().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:double"))?,
    };
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Double(v))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_float_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq).trim().to_string();
    let v = match s.as_str() {
        "NaN" => f32::NAN,
        "INF" => f32::INFINITY,
        "-INF" => f32::NEG_INFINITY,
        _ => s.parse().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:float"))?,
    };
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Float(v))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_any_uri_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = collapse_whitespace(&item_to_string(&seq));
    let result = vec![XdmItem::Atomic(XdmAtomicValue::AnyUri(s))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_qname_stream<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let (prefix_opt, local) =
        parse_qname_lexical(&s).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:QName"))?;
    let ns_uri = match prefix_opt.as_deref() {
        None => None,
        Some("xml") => Some(crate::consts::XML_URI.to_string()),
        Some(p) => ctx.static_ctx.namespaces.by_prefix.get(p).cloned(),
    };
    if prefix_opt.is_some() && ns_uri.is_none() {
        return Err(Error::from_code(ErrorCode::FORG0001, "unknown namespace prefix for QName"));
    }
    let result = vec![XdmItem::Atomic(XdmAtomicValue::QName { ns_uri, prefix: prefix_opt, local })];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_base64_binary_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let raw = item_to_string(&seq);
    let norm: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    if base64::engine::general_purpose::STANDARD.decode(&norm).is_err() {
        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:base64Binary"));
    }
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Base64Binary(norm))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_hex_binary_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let raw = item_to_string(&seq);
    let norm: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    if !norm.len().is_multiple_of(2) || !norm.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:hexBinary"));
    }
    let result = vec![XdmItem::Atomic(XdmAtomicValue::HexBinary(norm))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_datetime_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let result = match crate::util::temporal::parse_date_time_lex(&s) {
        Ok((d, t, tz)) => {
            let dt = crate::util::temporal::build_naive_datetime(d, t, tz);
            vec![XdmItem::Atomic(XdmAtomicValue::DateTime(dt))]
        }
        Err(_) => return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:dateTime")),
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_date_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let result = match crate::util::temporal::parse_date_lex(&s) {
        Ok((d, tz)) => vec![XdmItem::Atomic(XdmAtomicValue::Date { date: d, tz })],
        Err(_) => return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:date")),
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_time_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let result = match crate::util::temporal::parse_time_lex(&s) {
        Ok((t, tz)) => vec![XdmItem::Atomic(XdmAtomicValue::Time { time: t, tz })],
        Err(_) => return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:time")),
    };
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_duration_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let (months_opt, secs_opt) = parse_duration_lexical(&s)?;
    let value = match (months_opt, secs_opt) {
        (Some(m), None) => XdmAtomicValue::YearMonthDuration(m),
        (None, Some(sec)) => XdmAtomicValue::DayTimeDuration(sec),
        _ => {
            return Err(Error::from_code(ErrorCode::NYI0000, "mixed duration components are not supported"));
        }
    };
    let result = vec![XdmItem::Atomic(value)];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_day_time_duration_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let secs = parse_day_time_duration_secs(&s)
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:dayTimeDuration"))?;
    let result = vec![XdmItem::Atomic(XdmAtomicValue::DayTimeDuration(secs))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_g_year_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let (year, tz) = parse_g_year(&s).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gYear"))?;
    let result = vec![XdmItem::Atomic(XdmAtomicValue::GYear { year, tz })];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_g_year_month_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let (year, month, tz) =
        parse_g_year_month(&s).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gYearMonth"))?;
    let result = vec![XdmItem::Atomic(XdmAtomicValue::GYearMonth { year, month, tz })];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_g_month_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let (month, tz) = parse_g_month(&s).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gMonth"))?;
    let result = vec![XdmItem::Atomic(XdmAtomicValue::GMonth { month, tz })];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_g_month_day_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let (month, day, tz) =
        parse_g_month_day(&s).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gMonthDay"))?;
    let result = vec![XdmItem::Atomic(XdmAtomicValue::GMonthDay { month, day, tz })];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_g_day_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let (day, tz) = parse_g_day(&s).map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:gDay"))?;
    let result = vec![XdmItem::Atomic(XdmAtomicValue::GDay { day, tz })];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_year_month_duration_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    let months = parse_year_month_duration_months(&s)
        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:yearMonthDuration"))?;
    let result = vec![XdmItem::Atomic(XdmAtomicValue::YearMonthDuration(months))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_long_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = int_subtype_i64(&[seq], i64::MIN, i64::MAX, XdmAtomicValue::Long)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_int_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = int_subtype_i64(&[seq], i32::MIN as i64, i32::MAX as i64, |v| XdmAtomicValue::Int(v as i32))?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_short_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = int_subtype_i64(&[seq], i16::MIN as i64, i16::MAX as i64, |v| XdmAtomicValue::Short(v as i16))?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_byte_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = int_subtype_i64(&[seq], i8::MIN as i64, i8::MAX as i64, |v| XdmAtomicValue::Byte(v as i8))?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_unsigned_long_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = uint_subtype_u128(&[seq], 0, u64::MAX as u128, |v| XdmAtomicValue::UnsignedLong(v as u64))?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_unsigned_int_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = uint_subtype_u128(&[seq], 0, u32::MAX as u128, |v| XdmAtomicValue::UnsignedInt(v as u32))?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_unsigned_short_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = uint_subtype_u128(&[seq], 0, u16::MAX as u128, |v| XdmAtomicValue::UnsignedShort(v as u16))?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_unsigned_byte_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = uint_subtype_u128(&[seq], 0, u8::MAX as u128, |v| XdmAtomicValue::UnsignedByte(v as u8))?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_non_positive_integer_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = int_subtype_i64(&[seq], i64::MIN, 0, XdmAtomicValue::NonPositiveInteger)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_negative_integer_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = int_subtype_i64(&[seq], i64::MIN, -1, XdmAtomicValue::NegativeInteger)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_non_negative_integer_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = uint_subtype_u128(&[seq], 0, u64::MAX as u128, |v| XdmAtomicValue::NonNegativeInteger(v as u64))?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_positive_integer_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = uint_subtype_u128(&[seq], 1, u64::MAX as u128, |v| XdmAtomicValue::PositiveInteger(v as u64))?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_normalized_string_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = replace_whitespace(&item_to_string(&seq));
    let result = vec![XdmItem::Atomic(XdmAtomicValue::NormalizedString(s))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_token_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = collapse_whitespace(&item_to_string(&seq));
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Token(s))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_language_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = collapse_whitespace(&item_to_string(&seq));
    if !is_valid_language(&s) {
        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:language"));
    }
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Language(s))];
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_name_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = str_name_like(&[seq], true, true, XdmAtomicValue::Name)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_ncname_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = str_name_like(&[seq], true, false, XdmAtomicValue::NCName)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_nmtoken_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = str_name_like(&[seq], false, false, XdmAtomicValue::NMTOKEN)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_id_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = str_name_like(&[seq], true, false, XdmAtomicValue::Id)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_idref_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = str_name_like(&[seq], true, false, XdmAtomicValue::IdRef)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_entity_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    let result = str_name_like(&[seq], true, false, XdmAtomicValue::Entity)?;
    Ok(XdmSequenceStream::from_vec(result))
}

pub(super) fn xs_notation_stream<N: 'static + crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    args: &[XdmSequenceStream<N>],
) -> Result<XdmSequenceStream<N>, Error> {
    let seq: XdmSequence<N> = args[0].materialize()?;
    if seq.is_empty() {
        return Ok(XdmSequenceStream::from_vec(vec![]));
    }
    if seq.len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&seq);
    if parse_qname_lexical(&s).is_err() {
        return Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:NOTATION"));
    }
    let result = vec![XdmItem::Atomic(XdmAtomicValue::Notation(s))];
    Ok(XdmSequenceStream::from_vec(result))
}
