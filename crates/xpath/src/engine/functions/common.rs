use crate::engine::runtime::{CallCtx, Error, ErrorCode};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};
use chrono::{DateTime as ChronoDateTime, FixedOffset as ChronoFixedOffset, NaiveDate, NaiveTime, Offset};

pub(super) fn require_context_item<N: crate::model::XdmNode + Clone>(ctx: &CallCtx<N>) -> Result<XdmItem<N>, Error> {
    if let Some(item) = ctx.current_context_item.clone() {
        Ok(item)
    } else {
        ctx.dyn_ctx
            .context_item
            .clone()
            .ok_or_else(|| Error::from_code(ErrorCode::XPDY0002, "context item is undefined"))
    }
}

pub(super) fn ebv<N>(seq: &XdmSequence<N>) -> Result<bool, Error> {
    match seq.len() {
        0 => Ok(false),
        1 => match &seq[0] {
            XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => Ok(*b),
            XdmItem::Atomic(XdmAtomicValue::String(s)) => Ok(!s.is_empty()),
            XdmItem::Atomic(XdmAtomicValue::Integer(i)) => Ok(*i != 0),
            XdmItem::Atomic(XdmAtomicValue::Decimal(d)) => Ok(*d != 0.0),
            XdmItem::Atomic(XdmAtomicValue::Double(d)) => Ok(*d != 0.0 && !d.is_nan()),
            XdmItem::Atomic(XdmAtomicValue::Float(f)) => Ok(*f != 0.0 && !f.is_nan()),
            XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => Ok(!s.is_empty()),
            XdmItem::Atomic(_) => {
                Err(Error::from_code(ErrorCode::FORG0006, "EBV for this atomic type not supported yet"))
            }
            XdmItem::Node(_) => Ok(true),
        },
        _ => Err(Error::from_code(ErrorCode::FORG0006, "EBV of sequence with more than one item")),
    }
}

pub(super) fn item_to_string<N: crate::model::XdmNode>(seq: &XdmSequence<N>) -> String {
    if seq.is_empty() {
        return String::new();
    }
    match &seq[0] {
        XdmItem::Atomic(a) => as_string(a),
        XdmItem::Node(n) => n.string_value(),
    }
}

// Default implementation for normalize-space handling both 0- and 1-arity variants
pub(super) fn normalize_space_default<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    let s = match arg_opt {
        Some(seq) => item_to_string(seq),
        None => match require_context_item(ctx)? {
            XdmItem::Atomic(a) => as_string(&a),
            XdmItem::Node(n) => n.string_value(),
        },
    };
    let mut out = String::new();
    let mut in_space = true;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
}

// Default implementation for data() 0/1-arity
pub(super) fn data_default<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    if let Some(seq) = arg_opt {
        let mut out: XdmSequence<N> = Vec::with_capacity(seq.len());
        for it in seq {
            match it {
                XdmItem::Atomic(a) => out.push(XdmItem::Atomic(a.clone())),
                XdmItem::Node(n) => {
                    for atom in n.typed_value() {
                        out.push(XdmItem::Atomic(atom));
                    }
                }
            }
        }
        Ok(out)
    } else {
        match require_context_item(ctx)? {
            XdmItem::Atomic(a) => Ok(vec![XdmItem::Atomic(a)]),
            XdmItem::Node(n) => Ok(n.typed_value().into_iter().map(XdmItem::Atomic).collect()),
        }
    }
}

// Default implementation for number() 0/1-arity
pub(super) fn number_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    let seq: XdmSequence<N> = if let Some(s) = arg_opt { s.clone() } else { vec![require_context_item(ctx)?] };
    let n = to_number(&seq).unwrap_or(f64::NAN);
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(n))])
}

// Default implementation for string() 0/1-arity
pub(super) fn string_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    let s = match arg_opt {
        Some(seq) => item_to_string(seq),
        None => match require_context_item(ctx)? {
            XdmItem::Atomic(a) => as_string(&a),
            XdmItem::Node(n) => n.string_value(),
        },
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))])
}

// Default implementation for substring handling both 2- and 3-arity variants
pub(super) fn substring_default(s: &str, start_raw: f64, len_raw_opt: Option<f64>) -> String {
    // NaN handling
    if start_raw.is_nan() {
        return String::new();
    }
    if let Some(len_raw) = len_raw_opt {
        if len_raw.is_nan() {
            return String::new();
        }
        if len_raw.is_infinite() && len_raw.is_sign_negative() {
            return String::new();
        }
        if len_raw <= 0.0 {
            return String::new();
        }
    }

    // +/- infinity on start
    if start_raw.is_infinite() {
        if start_raw.is_sign_positive() {
            return String::new();
        } else {
            // -INF: treat as starting well before string
            let chars: Vec<char> = s.chars().collect();
            if let Some(len_raw) = len_raw_opt {
                let len_rounded = round_half_to_even_f64(len_raw);
                if len_rounded <= 0.0 {
                    return String::new();
                }
                let total = chars.len() as isize;
                let first_pos: isize = 1;
                let mut last_pos: isize = first_pos + len_rounded as isize - 1;
                if first_pos > total {
                    return String::new();
                }
                if last_pos > total {
                    last_pos = total;
                }
                let from_index = 0usize;
                let to_index = last_pos.max(0) as usize;
                return chars[from_index..to_index].iter().collect();
            } else {
                // 2-arity: whole string
                return s.to_string();
            }
        }
    }

    // Common path
    let start_rounded = round_half_to_even_f64(start_raw);
    if let Some(len_raw) = len_raw_opt {
        let len_rounded = round_half_to_even_f64(len_raw);
        if len_rounded <= 0.0 {
            return String::new();
        }
        let chars: Vec<char> = s.chars().collect();
        let total = chars.len() as isize;
        let first_pos: isize = if start_rounded < 1.0 { 1 } else { start_rounded as isize };
        let mut last_pos: isize = first_pos + len_rounded as isize - 1;
        if first_pos > total {
            return String::new();
        }
        if last_pos > total {
            last_pos = total;
        }
        let from_index = (first_pos - 1).max(0) as usize;
        let to_index = last_pos.max(0) as usize; // inclusive 1-based -> exclusive
        chars[from_index..to_index].iter().collect()
    } else {
        // 2-arity: from start to end
        if start_rounded <= 1.0 {
            s.to_string()
        } else {
            let from_index: usize = (start_rounded as isize - 1).max(0) as usize;
            s.chars().skip(from_index).collect()
        }
    }
}

// Default implementation for node-name(1)
pub(super) fn node_name_default<N: crate::model::XdmNode + Clone>(
    _ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    let Some(seq) = arg_opt else {
        return Ok(vec![]);
    };
    if seq.is_empty() {
        return Ok(vec![]);
    }
    match &seq[0] {
        XdmItem::Node(n) => {
            if let Some(q) = n.name() {
                Ok(vec![XdmItem::Atomic(XdmAtomicValue::QName {
                    ns_uri: q.ns_uri.clone(),
                    prefix: q.prefix.clone(),
                    local: q.local.clone(),
                })])
            } else {
                Ok(vec![])
            }
        }
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "node-name expects node()")),
    }
}

// Default implementation for round($value[, $precision])
pub(super) fn round_default<N: crate::model::XdmNode>(
    value_seq: &XdmSequence<N>,
    precision_seq_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    match precision_seq_opt {
        None => {
            // Preserve existing semantics of 1-arity implementation using to_number (empty -> NaN)
            let n = to_number(value_seq).unwrap_or(f64::NAN);
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(n.round()))])
        }
        Some(pseq) => {
            if value_seq.is_empty() {
                return Ok(vec![]);
            }
            let n = to_number(value_seq).unwrap_or(f64::NAN);
            let precision = match pseq.first() {
                None => 0,
                Some(XdmItem::Atomic(XdmAtomicValue::Integer(v))) => *v,
                _ => {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "precision must be xs:integer"));
                }
            };
            let r = round_with_precision(n, precision);
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(r))])
        }
    }
}

// Default implementation for round-half-to-even($value[, $precision])
pub(super) fn round_half_to_even_default<N: crate::model::XdmNode>(
    value_seq: &XdmSequence<N>,
    precision_seq_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    match precision_seq_opt {
        None => {
            let n = to_number(value_seq).unwrap_or(f64::NAN);
            let r = round_half_to_even_with_precision(n, 0);
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(r))])
        }
        Some(pseq) => {
            if value_seq.is_empty() {
                return Ok(vec![]);
            }
            let n = to_number(value_seq).unwrap_or(f64::NAN);
            let precision = match pseq.first() {
                None => 0,
                Some(XdmItem::Atomic(XdmAtomicValue::Integer(v))) => *v,
                _ => {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "precision must be xs:integer"));
                }
            };
            let r = round_half_to_even_with_precision(n, precision);
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(r))])
        }
    }
}

// Default implementation for subsequence($seq,$start[,$len])
#[allow(dead_code)]
pub(super) fn subsequence_default<N: crate::model::XdmNode + Clone>(
    seq: &XdmSequence<N>,
    start_raw: f64,
    len_raw_opt: Option<f64>,
) -> Result<XdmSequence<N>, Error> {
    if start_raw.is_nan() {
        return Ok(vec![]);
    }
    if start_raw.is_infinite() && start_raw.is_sign_positive() {
        return Ok(vec![]);
    }
    let start_rounded = round_half_to_even_f64(start_raw);
    if let Some(len_raw) = len_raw_opt {
        if len_raw.is_nan() || len_raw <= 0.0 {
            return Ok(vec![]);
        }
        let len_rounded = round_half_to_even_f64(len_raw);
        if len_rounded <= 0.0 {
            return Ok(vec![]);
        }
        let total = seq.len() as isize;
        let first_pos: isize = if start_rounded < 1.0 { 1 } else { start_rounded as isize };
        let last_pos = first_pos + len_rounded as isize - 1;
        if first_pos > total {
            return Ok(vec![]);
        }
        let last_pos = last_pos.min(total);
        let from_index = (first_pos - 1).max(0) as usize;
        let to_index_exclusive = last_pos as usize;
        Ok(seq.iter().skip(from_index).take(to_index_exclusive - from_index).cloned().collect())
    } else {
        if start_rounded <= 1.0 {
            return Ok(seq.clone());
        }
        let from_index = (start_rounded as isize - 1).max(0) as usize;
        Ok(seq.iter().skip(from_index).cloned().collect())
    }
}

// Default for deep-equal 2|3-arity
pub(super) fn deep_equal_default<N: crate::model::XdmNode>(
    ctx: &CallCtx<N>,
    a: &XdmSequence<N>,
    b: &XdmSequence<N>,
    collation_uri: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let k = crate::engine::collation::resolve_collation(
        ctx.dyn_ctx,
        ctx.default_collation.as_ref(),
        collation_uri.and_then(|u| if u.is_empty() { None } else { Some(u) }),
    )?;
    let b = deep_equal_with_collation(a, b, Some(k.as_trait()))?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
}

// Defaults for regex family: matches/replace/tokenize
pub(super) fn matches_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    input: &XdmSequence<N>,
    pattern: &XdmSequence<N>,
    flags_opt: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let inp = item_to_string(input);
    let pat = item_to_string(pattern);
    let flags = flags_opt.unwrap_or("");
    let b = regex_matches(ctx, &inp, &pat, flags)?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))])
}

pub(super) fn replace_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    input: &XdmSequence<N>,
    pattern: &XdmSequence<N>,
    repl: &XdmSequence<N>,
    flags_opt: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let inp = item_to_string(input);
    let pat = item_to_string(pattern);
    let rep = item_to_string(repl);
    let flags = flags_opt.unwrap_or("");
    let s = regex_replace(ctx, &inp, &pat, &rep, flags)?;
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))])
}

pub(super) fn tokenize_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    input: &XdmSequence<N>,
    pattern: &XdmSequence<N>,
    flags_opt: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    let inp = item_to_string(input);
    let pat = item_to_string(pattern);
    let flags = flags_opt.unwrap_or("");
    let parts = regex_tokenize(ctx, &inp, &pat, flags)?;
    Ok(parts.into_iter().map(|s| XdmItem::Atomic(XdmAtomicValue::String(s))).collect())
}

// Default for sum($seq[, $zero])
pub(super) fn sum_default<N: crate::model::XdmNode>(
    seq: &XdmSequence<N>,
    zero_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    if seq.is_empty() {
        if let Some(z) = zero_opt {
            if z.is_empty() {
                return Ok(vec![]);
            }
            if z.len() != 1 {
                return Err(Error::from_code(ErrorCode::FORG0006, "sum($seq,$zero) expects a single item for $zero"));
            }
            return Ok(z.clone());
        }
        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(0))]);
    }
    enum SumState {
        None,
        Numeric { kind: NumericKind, use_int: bool, int_acc: i128, dec_acc: f64 },
        YearMonth { total: i64 },
        DayTime { total: i128 },
    }
    let mut state = SumState::None;
    for it in seq {
        let XdmItem::Atomic(a) = it else {
            return Err(Error::from_code(ErrorCode::XPTY0004, "sum requires atomic values"));
        };
        match a {
            XdmAtomicValue::YearMonthDuration(months) => {
                state = match state {
                    SumState::None => SumState::YearMonth { total: *months as i64 },
                    SumState::YearMonth { total } => SumState::YearMonth {
                        total: total
                            .checked_add(*months as i64)
                            .ok_or_else(|| Error::from_code(ErrorCode::FOAR0002, "yearMonthDuration overflow"))?,
                    },
                    _ => return Err(Error::from_code(ErrorCode::XPTY0004, "mixed types in sum")),
                };
            }
            XdmAtomicValue::DayTimeDuration(secs) => {
                state = match state {
                    SumState::None => SumState::DayTime { total: *secs as i128 },
                    SumState::DayTime { total } => SumState::DayTime {
                        total: total
                            .checked_add(*secs as i128)
                            .ok_or_else(|| Error::from_code(ErrorCode::FOAR0002, "dayTimeDuration overflow"))?,
                    },
                    _ => return Err(Error::from_code(ErrorCode::XPTY0004, "mixed types in sum")),
                };
            }
            _ => {
                if let Some((nk, num)) = classify_numeric(a)? {
                    if nk == NumericKind::Double && num.is_nan() {
                        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(f64::NAN))]);
                    }
                    state = match state {
                        SumState::None => SumState::Numeric {
                            kind: nk,
                            use_int: matches!(nk, NumericKind::Integer),
                            int_acc: if matches!(nk, NumericKind::Integer) { a_as_i128(a).unwrap_or(0) } else { 0 },
                            dec_acc: if matches!(nk, NumericKind::Integer) {
                                a_as_i128(a).unwrap_or(0) as f64
                            } else {
                                num
                            },
                        },
                        SumState::Numeric { mut kind, mut use_int, mut int_acc, mut dec_acc } => {
                            kind = kind.promote(nk);
                            if matches!(nk, NumericKind::Integer) && use_int {
                                if let Some(i) = a_as_i128(a) {
                                    if let Some(v) = int_acc.checked_add(i) {
                                        int_acc = v;
                                    } else {
                                        use_int = false;
                                        dec_acc = int_acc as f64 + i as f64;
                                    }
                                }
                            } else {
                                if use_int {
                                    dec_acc = int_acc as f64;
                                    use_int = false;
                                }
                                dec_acc += num;
                            }
                            SumState::Numeric { kind, use_int, int_acc, dec_acc }
                        }
                        _ => {
                            return Err(Error::from_code(ErrorCode::XPTY0004, "mixed types in sum"));
                        }
                    };
                } else {
                    return Err(Error::from_code(ErrorCode::XPTY0004, "sum requires numeric or duration values"));
                }
            }
        }
    }
    let result = match state {
        SumState::None => XdmAtomicValue::Integer(0),
        SumState::Numeric { kind, use_int, int_acc, dec_acc } => {
            if use_int && matches!(kind, NumericKind::Integer) {
                XdmAtomicValue::Integer(int_acc as i64)
            } else {
                match kind {
                    NumericKind::Integer => XdmAtomicValue::Integer(int_acc as i64),
                    NumericKind::Decimal => XdmAtomicValue::Decimal(dec_acc),
                    NumericKind::Float => XdmAtomicValue::Float(dec_acc as f32),
                    NumericKind::Double => XdmAtomicValue::Double(dec_acc),
                }
            }
        }
        SumState::YearMonth { total } => {
            let months: i32 =
                total.try_into().map_err(|_| Error::from_code(ErrorCode::FOAR0002, "yearMonthDuration overflow"))?;
            XdmAtomicValue::YearMonthDuration(months)
        }
        SumState::DayTime { total } => {
            let secs: i64 =
                total.try_into().map_err(|_| Error::from_code(ErrorCode::FOAR0002, "dayTimeDuration overflow"))?;
            XdmAtomicValue::DayTimeDuration(secs)
        }
    };
    Ok(vec![XdmItem::Atomic(result)])
}

// Default implementation for name() 0/1-arity
pub(super) fn name_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    use crate::model::NodeKind;
    let target_opt = if let Some(seq) = arg_opt {
        if seq.is_empty() { None } else { Some(seq[0].clone()) }
    } else {
        Some(require_context_item(ctx)?)
    };
    let s = match target_opt {
        None => String::new(),
        Some(XdmItem::Node(n)) => n
            .name()
            .map(|q| {
                if matches!(n.kind(), NodeKind::Namespace) {
                    q.local
                } else if let Some(p) = q.prefix {
                    format!("{}:{}", p, q.local)
                } else {
                    q.local
                }
            })
            .unwrap_or_default(),
        Some(_) => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "name() expects node()"));
        }
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))])
}

// Default implementation for local-name() 0/1-arity
pub(super) fn local_name_default<N: crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    arg_opt: Option<&XdmSequence<N>>,
) -> Result<XdmSequence<N>, Error> {
    let target_opt = if let Some(seq) = arg_opt {
        if seq.is_empty() { None } else { Some(seq[0].clone()) }
    } else {
        Some(require_context_item(ctx)?)
    };
    let s = match target_opt {
        None => String::new(),
        Some(XdmItem::Node(n)) => n.name().map(|q| q.local).unwrap_or_default(),
        Some(_) => {
            return Err(Error::from_code(ErrorCode::XPTY0004, "local-name() expects node()"));
        }
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))])
}

// (namespace-uri default helper removed; single spec-compliant implementation is registered above)

// Default implementation for compare($A,$B[,$collation])
pub(super) fn compare_default<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    a: &XdmSequence<N>,
    b: &XdmSequence<N>,
    collation_uri: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    if a.is_empty() || b.is_empty() {
        return Ok(vec![]);
    }
    let sa = item_to_string(a);
    let sb = item_to_string(b);
    let uri_opt = collation_uri.and_then(|u| if u.is_empty() { None } else { Some(u) });
    let k = crate::engine::collation::resolve_collation(ctx.dyn_ctx, ctx.default_collation.as_ref(), uri_opt)?;
    let c = k.as_trait();
    let ord = c.compare(&sa, &sb);
    let v = match ord {
        core::cmp::Ordering::Less => -1,
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Greater => 1,
    };
    Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(v))])
}

// Default implementation for index-of($seq,$search[,$collation])
pub(super) fn index_of_default<N: 'static + crate::model::XdmNode + Clone>(
    ctx: &CallCtx<N>,
    seq: &XdmSequence<N>,
    search: &XdmSequence<N>,
    collation_uri: Option<&str>,
) -> Result<XdmSequence<N>, Error> {
    use crate::engine::eq::{EqKey, build_eq_key};
    let mut out: XdmSequence<N> = Vec::new();
    let uri_opt = collation_uri.and_then(|u| if u.is_empty() { None } else { Some(u) });
    let coll_kind = crate::engine::collation::resolve_collation(ctx.dyn_ctx, ctx.default_collation.as_ref(), uri_opt)?;
    let coll: Option<&dyn crate::engine::collation::Collation> = Some(coll_kind.as_trait());
    let needle_opt = search.first();
    // Precompute key for atomic needle; if NaN early return empty.
    let needle_key = if let Some(XdmItem::Atomic(a)) = needle_opt {
        match build_eq_key::<crate::model::simple::SimpleNode>(&XdmItem::Atomic(a.clone()), coll) {
            Ok(k) => {
                if matches!(k, EqKey::NaN) {
                    return Ok(out);
                }
                Some(k)
            }
            Err(_) => None,
        }
    } else {
        None
    };
    for (i, it) in seq.iter().enumerate() {
        let eq = match (it, needle_opt) {
            (XdmItem::Atomic(a), Some(XdmItem::Atomic(_))) => {
                if let Some(ref nk) = needle_key {
                    match build_eq_key::<crate::model::simple::SimpleNode>(&XdmItem::Atomic(a.clone()), coll) {
                        Ok(k) => k == *nk,
                        Err(_) => false,
                    }
                } else {
                    false
                }
            }
            // Node vs node: fallback to string-value equality (legacy simplification)
            (XdmItem::Node(n), Some(XdmItem::Node(m))) => n.string_value() == m.string_value(),
            // Mixed node/atomic: compare string values; honor collation if provided
            (XdmItem::Node(n), Some(XdmItem::Atomic(b))) => {
                if let Some(c) = coll {
                    c.key(&n.string_value()) == c.key(&as_string(b))
                } else {
                    n.string_value() == as_string(b)
                }
            }
            (XdmItem::Atomic(a), Some(XdmItem::Node(n))) => {
                if let Some(c) = coll {
                    c.key(&as_string(a)) == c.key(&n.string_value())
                } else {
                    as_string(a) == n.string_value()
                }
            }
            _ => false,
        };
        if eq {
            out.push(XdmItem::Atomic(XdmAtomicValue::Integer(i as i64 + 1)));
        }
    }
    Ok(out)
}

// Unified handler for fn:error() 0-3 arities
pub(super) fn error_default<N: crate::model::XdmNode>(args: &[XdmSequence<N>]) -> Result<XdmSequence<N>, Error> {
    match args.len() {
        0 => Err(Error::from_code(ErrorCode::FOER0000, "fn:error()")),
        1 => {
            let code = item_to_string(&args[0]);
            if code.is_empty() {
                Err(Error::from_code(ErrorCode::FOER0000, "fn:error"))
            } else {
                Err(Error::new_qname(Error::parse_code(&code), "fn:error"))
            }
        }
        2 => {
            let code = item_to_string(&args[0]);
            let desc = item_to_string(&args[1]);
            let msg = if desc.is_empty() { "fn:error".to_string() } else { desc };
            if code.is_empty() {
                Err(Error::from_code(ErrorCode::FOER0000, msg))
            } else {
                Err(Error::new_qname(Error::parse_code(&code), msg))
            }
        }
        _ => {
            // 3 or more: third arg (data) ignored for now
            let code = item_to_string(&args[0]);
            let desc = item_to_string(&args[1]);
            let msg = if desc.is_empty() { "fn:error".to_string() } else { desc };
            if code.is_empty() {
                Err(Error::from_code(ErrorCode::FOER0000, msg))
            } else {
                Err(Error::new_qname(Error::parse_code(&code), msg))
            }
        }
    }
}

pub(super) fn as_string(a: &XdmAtomicValue) -> String {
    match a {
        XdmAtomicValue::String(s) => s.clone(),
        XdmAtomicValue::UntypedAtomic(s) => s.clone(),
        XdmAtomicValue::AnyUri(u) => u.clone(),
        XdmAtomicValue::Boolean(b) => {
            if *b {
                "true".into()
            } else {
                "false".into()
            }
        }
        XdmAtomicValue::Integer(i) => i.to_string(),
        // Numeric subtypes fallback to their numeric representation
        XdmAtomicValue::Long(i) => i.to_string(),
        XdmAtomicValue::Int(i) => i.to_string(),
        XdmAtomicValue::Short(i) => i.to_string(),
        XdmAtomicValue::Byte(i) => i.to_string(),
        XdmAtomicValue::UnsignedLong(i) => i.to_string(),
        XdmAtomicValue::UnsignedInt(i) => i.to_string(),
        XdmAtomicValue::UnsignedShort(i) => i.to_string(),
        XdmAtomicValue::UnsignedByte(i) => i.to_string(),
        XdmAtomicValue::NonPositiveInteger(i) => i.to_string(),
        XdmAtomicValue::NegativeInteger(i) => i.to_string(),
        XdmAtomicValue::NonNegativeInteger(i) => i.to_string(),
        XdmAtomicValue::PositiveInteger(i) => i.to_string(),
        XdmAtomicValue::Double(d) => d.to_string(),
        XdmAtomicValue::Float(f) => f.to_string(),
        XdmAtomicValue::Decimal(d) => d.to_string(),
        XdmAtomicValue::QName { prefix, local, .. } => {
            if let Some(p) = prefix {
                format!("{}:{}", p, local)
            } else {
                local.clone()
            }
        }
        XdmAtomicValue::DateTime(dt) => dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        XdmAtomicValue::Date { date, tz } => {
            if let Some(off) = tz {
                format!("{}{}", date.format("%Y-%m-%d"), fmt_offset_local(off))
            } else {
                date.format("%Y-%m-%d").to_string()
            }
        }
        XdmAtomicValue::Time { time, tz } => {
            if let Some(off) = tz {
                format!("{}{}", time.format("%H:%M:%S"), fmt_offset_local(off))
            } else {
                time.format("%H:%M:%S").to_string()
            }
        }
        XdmAtomicValue::YearMonthDuration(months) => format_year_month_duration_local(*months),
        XdmAtomicValue::DayTimeDuration(secs) => format_day_time_duration_local(*secs),
        // Binary & lexical string-derived types: return stored lexical form
        XdmAtomicValue::Base64Binary(s)
        | XdmAtomicValue::HexBinary(s)
        | XdmAtomicValue::NormalizedString(s)
        | XdmAtomicValue::Token(s)
        | XdmAtomicValue::Language(s)
        | XdmAtomicValue::Name(s)
        | XdmAtomicValue::NCName(s)
        | XdmAtomicValue::NMTOKEN(s)
        | XdmAtomicValue::Id(s)
        | XdmAtomicValue::IdRef(s)
        | XdmAtomicValue::Entity(s)
        | XdmAtomicValue::Notation(s) => s.clone(),
        // g* date fragments: simple ISO-ish formatting
        XdmAtomicValue::GYear { year, tz } => {
            format!("{:04}{}", year, tz.map(|o| fmt_offset_local(&o)).unwrap_or_default())
        }
        XdmAtomicValue::GYearMonth { year, month, tz } => {
            format!("{:04}-{:02}{}", year, month, tz.map(|o| fmt_offset_local(&o)).unwrap_or_default())
        }
        XdmAtomicValue::GMonth { month, tz } => {
            format!("--{:02}{}", month, tz.map(|o| fmt_offset_local(&o)).unwrap_or_default())
        }
        XdmAtomicValue::GMonthDay { month, day, tz } => {
            format!("--{:02}-{:02}{}", month, day, tz.map(|o| fmt_offset_local(&o)).unwrap_or_default())
        }
        XdmAtomicValue::GDay { day, tz } => {
            format!("---{:02}{}", day, tz.map(|o| fmt_offset_local(&o)).unwrap_or_default())
        }
    }
}

pub(super) fn to_number<N: crate::model::XdmNode>(seq: &XdmSequence<N>) -> Result<f64, Error> {
    if seq.is_empty() {
        return Ok(f64::NAN);
    }
    if seq.len() != 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "expects single item"));
    }
    match &seq[0] {
        XdmItem::Atomic(a) => to_number_atomic(a),
        XdmItem::Node(n) => {
            n.string_value().parse::<f64>().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid number"))
        }
    }
}

pub(super) fn to_number_atomic(a: &XdmAtomicValue) -> Result<f64, Error> {
    match a {
        XdmAtomicValue::Integer(i) => Ok(*i as f64),
        XdmAtomicValue::Long(i) => Ok(*i as f64),
        XdmAtomicValue::Int(i) => Ok(*i as f64),
        XdmAtomicValue::Short(i) => Ok(*i as f64),
        XdmAtomicValue::Byte(i) => Ok(*i as f64),
        XdmAtomicValue::UnsignedLong(i) => Ok(*i as f64),
        XdmAtomicValue::UnsignedInt(i) => Ok(*i as f64),
        XdmAtomicValue::UnsignedShort(i) => Ok(*i as f64),
        XdmAtomicValue::UnsignedByte(i) => Ok(*i as f64),
        XdmAtomicValue::NonPositiveInteger(i) => Ok(*i as f64),
        XdmAtomicValue::NegativeInteger(i) => Ok(*i as f64),
        XdmAtomicValue::NonNegativeInteger(i) => Ok(*i as f64),
        XdmAtomicValue::PositiveInteger(i) => Ok(*i as f64),
        XdmAtomicValue::Double(d) => Ok(*d),
        XdmAtomicValue::Float(f) => Ok(*f as f64),
        XdmAtomicValue::Decimal(d) => Ok(*d),
        XdmAtomicValue::UntypedAtomic(s) | XdmAtomicValue::String(s) | XdmAtomicValue::AnyUri(s) => {
            s.parse::<f64>().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid number"))
        }
        XdmAtomicValue::Boolean(b) => Ok(if *b { 1.0 } else { 0.0 }),
        XdmAtomicValue::QName { .. } => Err(Error::from_code(ErrorCode::XPTY0004, "cannot cast QName to number")),
        XdmAtomicValue::DateTime(_)
        | XdmAtomicValue::Date { .. }
        | XdmAtomicValue::Time { .. }
        | XdmAtomicValue::YearMonthDuration(_)
        | XdmAtomicValue::DayTimeDuration(_)
        | XdmAtomicValue::Base64Binary(_)
        | XdmAtomicValue::HexBinary(_)
        | XdmAtomicValue::GYear { .. }
        | XdmAtomicValue::GYearMonth { .. }
        | XdmAtomicValue::GMonth { .. }
        | XdmAtomicValue::GMonthDay { .. }
        | XdmAtomicValue::GDay { .. }
        | XdmAtomicValue::NormalizedString(_)
        | XdmAtomicValue::Token(_)
        | XdmAtomicValue::Language(_)
        | XdmAtomicValue::Name(_)
        | XdmAtomicValue::NCName(_)
        | XdmAtomicValue::NMTOKEN(_)
        | XdmAtomicValue::Id(_)
        | XdmAtomicValue::IdRef(_)
        | XdmAtomicValue::Entity(_)
        | XdmAtomicValue::Notation(_) => Err(Error::from_code(ErrorCode::XPTY0004, "cannot cast value to number")),
    }
}

pub(super) fn num_unary<N: crate::model::XdmNode>(args: &[XdmSequence<N>], f: impl Fn(f64) -> f64) -> XdmSequence<N> {
    let n = to_number(&args[0]).unwrap_or(f64::NAN);
    vec![XdmItem::Atomic(XdmAtomicValue::Double(f(n)))]
}

// Precision rounding helpers for multi-arg numeric functions
// Implements XPath 2.0 semantics (simplified: treat value as xs:double, precision as integer; NaN/INF propagate)
pub(super) fn round_with_precision(value: f64, precision: i64) -> f64 {
    if value.is_nan() || value.is_infinite() {
        return value;
    }
    // Optimization: if precision outside reasonable range, short-circuit
    if precision >= 0 {
        if precision > 15 {
            // beyond double mantissa; value unchanged
            return value;
        }
        let factor = 10_f64.powi(precision as i32);
        (value * factor).round() / factor
    } else {
        let negp = (-precision) as i32;
        if negp > 15 {
            // rounds to 0 (or +/-) at large magnitude beyond precision; emulate by returning 0 with sign
            return 0.0 * value.signum();
        }
        let factor = 10_f64.powi(negp);
        (value / factor).round() * factor
    }
}

pub(super) fn round_half_to_even_with_precision(value: f64, precision: i64) -> f64 {
    if value.is_nan() || value.is_infinite() {
        return value;
    }
    if precision == 0 {
        // replicate single-arg banker rounding
        let t = value.trunc();
        let frac = value - t;
        if frac.abs() != 0.5 {
            return value.round();
        }
        let ti = t as i64;
        if ti % 2 == 0 { t } else { t + value.signum() }
    } else if precision > 0 {
        if precision > 15 {
            return value;
        }
        let factor = 10_f64.powi(precision as i32);
        let scaled = value * factor;
        banker_round(scaled) / factor
    } else {
        // precision < 0
        let negp = (-precision) as i32;
        if negp > 15 {
            return 0.0 * value.signum();
        }
        let factor = 10_f64.powi(negp);
        banker_round(value / factor) * factor
    }
}

pub(super) fn banker_round(x: f64) -> f64 {
    let t = x.trunc();
    let frac = x - t;
    // Floating point scaling can yield values like 234.49999999997 for an
    // expected half (.5). Treat near-half within epsilon as exact .5 so the
    // even rule applies (ensures 2.345 -> 2.34 for precision=2 case).
    let frac_abs = frac.abs();
    const HALF: f64 = 0.5;
    const EPS: f64 = 1e-9;
    if (frac_abs - HALF).abs() > EPS {
        return x.round();
    }
    let ti = t as i64;
    if ti % 2 == 0 { t } else { t + x.signum() }
}

// (removed contains_with_collation/starts_with_with_collation/ends_with_with_collation after refactor)

#[doc(hidden)]
pub fn deep_equal_with_collation<N: crate::model::XdmNode>(
    a: &XdmSequence<N>,
    b: &XdmSequence<N>,
    coll: Option<&dyn crate::engine::collation::Collation>,
) -> Result<bool, Error> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (ia, ib) in a.iter().zip(b.iter()) {
        let eq = match (ia, ib) {
            (XdmItem::Atomic(aa), XdmItem::Atomic(bb)) => atomic_equal_with_collation(aa, bb, coll)?,
            (XdmItem::Node(na), XdmItem::Node(nb)) => node_deep_equal(na, nb, coll)?,
            _ => false,
        };
        if !eq {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn node_deep_equal<N: crate::model::XdmNode>(
    a: &N,
    b: &N,
    coll: Option<&dyn crate::engine::collation::Collation>,
) -> Result<bool, Error> {
    use crate::model::NodeKind;
    // Kind must match
    if a.kind() != b.kind() {
        return Ok(false);
    }
    // Name (if present) must match (namespace + local)
    if a.name() != b.name() {
        return Ok(false);
    }
    match a.kind() {
        NodeKind::Text
        | NodeKind::Comment
        | NodeKind::ProcessingInstruction
        | NodeKind::Attribute
        | NodeKind::Namespace => {
            let mut sa = a.string_value();
            let mut sb = b.string_value();
            if let Some(c) = coll {
                sa = c.key(&sa);
                sb = c.key(&sb);
            }
            Ok(sa == sb)
        }
        NodeKind::Element | NodeKind::Document => {
            // Attributes unordered
            let mut attrs_a: Vec<(Option<String>, String, String)> = a
                .attributes()
                .map(|at| {
                    let name = at.name();
                    let ns = name.as_ref().and_then(|q| q.ns_uri.clone());
                    let local = name.as_ref().map(|q| q.local.clone()).unwrap_or_default();
                    let mut val = at.string_value();
                    if let Some(c) = coll {
                        val = c.key(&val);
                    }
                    (ns, local, val)
                })
                .collect();
            let mut attrs_b: Vec<(Option<String>, String, String)> = b
                .attributes()
                .map(|at| {
                    let name = at.name();
                    let ns = name.as_ref().and_then(|q| q.ns_uri.clone());
                    let local = name.as_ref().map(|q| q.local.clone()).unwrap_or_default();
                    let mut val = at.string_value();
                    if let Some(c) = coll {
                        val = c.key(&val);
                    }
                    (ns, local, val)
                })
                .collect();
            attrs_a.sort();
            attrs_b.sort();
            if attrs_a != attrs_b {
                return Ok(false);
            }
            // Namespace nodes unordered (exclude reserved xml prefix if present). Treat as (prefix, uri) pairs.
            let mut ns_a: Vec<(String, String)> = a
                .namespaces()
                .filter_map(|ns| {
                    let name = ns.name()?; // prefix stored both in prefix/local
                    if name.prefix.as_deref() == Some("xml") {
                        return None;
                    }
                    Some((name.prefix.unwrap_or_default(), ns.string_value()))
                })
                .collect();
            let mut ns_b: Vec<(String, String)> = b
                .namespaces()
                .filter_map(|ns| {
                    let name = ns.name()?;
                    if name.prefix.as_deref() == Some("xml") {
                        return None;
                    }
                    Some((name.prefix.unwrap_or_default(), ns.string_value()))
                })
                .collect();
            ns_a.sort();
            ns_b.sort();
            if ns_a != ns_b {
                return Ok(false);
            }
            // Children ordered
            let ca: Vec<N> = a.children().collect();
            let cb: Vec<N> = b.children().collect();
            if ca.len() != cb.len() {
                return Ok(false);
            }
            for (child_a, child_b) in ca.iter().zip(cb.iter()) {
                if !node_deep_equal(child_a, child_b, coll)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

// distinct-values core implementation shared by 1- and 2-arg variants.
pub(super) fn distinct_values_impl<N: crate::model::XdmNode>(
    _ctx: &CallCtx<N>,
    seq: &XdmSequence<N>,
    coll: Option<&dyn crate::engine::collation::Collation>,
) -> Result<XdmSequence<N>, Error> {
    use crate::engine::eq::{EqKey, build_eq_key};
    use std::collections::HashSet;
    let mut seen: HashSet<EqKey> = HashSet::new();
    let mut out: XdmSequence<N> = Vec::new();
    for it in seq {
        match it {
            XdmItem::Node(_) => {
                return Err(Error::from_code(ErrorCode::XPTY0004, "distinct-values on non-atomic item"));
            }
            XdmItem::Atomic(a) => {
                let tmp: XdmItem<N> = XdmItem::Atomic(a.clone());
                let key = build_eq_key(&tmp, coll)?;
                if seen.insert(key) {
                    out.push(tmp);
                }
            }
        }
    }
    Ok(out)
}

pub(super) fn atomic_equal_with_collation(
    a: &XdmAtomicValue,
    b: &XdmAtomicValue,
    coll: Option<&dyn crate::engine::collation::Collation>,
) -> Result<bool, Error> {
    use crate::engine::eq::build_eq_key;
    // Helper generic to appease type inference for XdmNode parameter.
    fn key_for<N: crate::model::XdmNode>(
        v: &XdmAtomicValue,
        coll: Option<&dyn crate::engine::collation::Collation>,
    ) -> Result<crate::engine::eq::EqKey, Error> {
        let item: XdmItem<N> = XdmItem::Atomic(v.clone());
        build_eq_key(&item, coll)
    }
    // We don't know the node type here (deep-equal may compare atomics only) – use SimpleNode phantom by leveraging that EqKey building ignores node internals for atomic case.
    // SAFETY: build_eq_key only pattern matches &XdmItem and for Atomic branch does not access node-specific APIs.
    type AnyNode = crate::model::simple::SimpleNode;
    let ka = key_for::<AnyNode>(a, coll)?;
    let kb = key_for::<AnyNode>(b, coll)?;
    Ok(ka == kb)
}

// ===== Helpers (Regex) =====
pub(super) fn get_regex_provider<N>(ctx: &CallCtx<N>) -> std::rc::Rc<dyn crate::engine::runtime::RegexProvider> {
    if let Some(p) = &ctx.regex { p.clone() } else { std::rc::Rc::new(crate::engine::runtime::FancyRegexProvider) }
}

pub(super) fn regex_matches<N>(ctx: &CallCtx<N>, input: &str, pattern: &str, flags: &str) -> Result<bool, Error> {
    let provider = get_regex_provider(ctx);
    let normalized = validate_regex_flags(flags)?;
    reject_backref_in_char_class(pattern)?;
    provider.matches(pattern, &normalized, input)
}

pub(super) fn regex_replace<N>(
    ctx: &CallCtx<N>,
    input: &str,
    pattern: &str,
    repl: &str,
    flags: &str,
) -> Result<String, Error> {
    let provider = get_regex_provider(ctx);
    let normalized = validate_regex_flags(flags)?;
    reject_backref_in_char_class(pattern)?;
    provider.replace(pattern, &normalized, input, repl)
}

pub(super) fn regex_tokenize<N>(
    ctx: &CallCtx<N>,
    input: &str,
    pattern: &str,
    flags: &str,
) -> Result<Vec<String>, Error> {
    let provider = get_regex_provider(ctx);
    let normalized = validate_regex_flags(flags)?;
    reject_backref_in_char_class(pattern)?;
    provider.tokenize(pattern, &normalized, input)
}

// Validate XPath 2.0 regex flags: i (case-insensitive), m (multiline), s (dotall), x (free-spacing)
// Return normalized string with duplicates removed preserving input order of first occurrences.
pub(super) fn validate_regex_flags(flags: &str) -> Result<String, Error> {
    if flags.is_empty() {
        return Ok(String::new());
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut out = String::new();
    for ch in flags.chars() {
        match ch {
            'i' | 'm' | 's' | 'x' => {
                if seen.insert(ch) {
                    out.push(ch);
                }
            }
            _ => {
                return Err(Error::from_code(
                    crate::engine::runtime::ErrorCode::FORX0001,
                    format!("unsupported regex flag: {ch}"),
                ));
            }
        }
    }
    Ok(out)
}

// Quick scan to reject illegal backreferences inside character classes like [$1] which XPath 2.0 forbids.
// We don't implement a full parser here; a conservative heuristic is sufficient: if we see a '[' ... ']' region
// and a '$' followed by a digit within it, we raise FORX0002.
pub(super) fn reject_backref_in_char_class(pattern: &str) -> Result<(), Error> {
    let bytes = pattern.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'[' {
            // enter class; handle nested \] and escaped \[
            i += 1;
            while i < bytes.len() && bytes[i] != b']' {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == b'$' && i + 1 < bytes.len() && (bytes[i + 1] as char).is_ascii_digit() {
                    return Err(Error::from_code(
                        crate::engine::runtime::ErrorCode::FORX0002,
                        "backreference not allowed in character class",
                    ));
                }
                i += 1;
            }
        }
        i += 1;
    }
    Ok(())
}

// Banker (half-to-even) rounding for f64 values matching fn:round-half-to-even semantics used by substring/subsequence.
pub(super) fn round_half_to_even_f64(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let ax = x.abs();
    let floor = ax.floor();
    let frac = ax - floor;
    const EPS: f64 = 1e-12; // tolerant epsilon for .5 detection
    let rounded_abs = if (frac - 0.5).abs() < EPS {
        // tie -> even
        if ((floor as i64) & 1) == 0 { floor } else { floor + 1.0 }
    } else if frac < 0.5 {
        floor
    } else {
        floor + 1.0
    };
    if x.is_sign_negative() { -rounded_abs } else { rounded_abs }
}

pub(super) fn minmax_impl<N: crate::model::XdmNode>(
    ctx: &CallCtx<N>,
    seq: &XdmSequence<N>,
    coll: Option<&dyn crate::engine::collation::Collation>,
    is_min: bool,
) -> Result<XdmSequence<N>, Error> {
    if seq.is_empty() {
        return Ok(vec![]);
    }
    // numeric if all numeric, else string using collation (default or provided)
    let mut all_num = true;
    let mut acc_num = if is_min { f64::INFINITY } else { f64::NEG_INFINITY };
    for it in seq {
        match it {
            XdmItem::Atomic(a) => match to_number_atomic(a) {
                Ok(n) => {
                    if n.is_nan() {
                        return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(f64::NAN))]);
                    }
                    if is_min { acc_num = acc_num.min(n) } else { acc_num = acc_num.max(n) }
                }
                Err(_) => {
                    all_num = false;
                    break;
                }
            },
            _ => {
                all_num = false;
                break;
            }
        }
    }
    if all_num {
        // Re-run with detailed kind inference to decide result type & value (acc_num already min/max as f64)
        let mut kind = NumericKind::Integer;
        for it in seq {
            if let XdmItem::Atomic(a) = it
                && let Some((nk, num)) = classify_numeric(a)?
            {
                if nk == NumericKind::Double && num.is_nan() {
                    return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(f64::NAN))]);
                }
                if nk == NumericKind::Float && num.is_nan() {
                    return Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(f64::NAN))]);
                }
                kind = kind.promote(nk);
            }
        }
        let out = match kind {
            NumericKind::Integer => XdmAtomicValue::Integer(acc_num as i64),
            NumericKind::Decimal => XdmAtomicValue::Decimal(acc_num),
            NumericKind::Float => XdmAtomicValue::Float(acc_num as f32),
            NumericKind::Double => XdmAtomicValue::Double(acc_num),
        };
        return Ok(vec![XdmItem::Atomic(out)]);
    }
    // String branch
    // Ensure owned Arc lives while function executes (store optionally)
    let mut owned_coll: Option<std::rc::Rc<dyn crate::engine::collation::Collation>> = None;
    let effective_coll: Option<&dyn crate::engine::collation::Collation> = if let Some(c) = coll {
        Some(c)
    } else {
        let k = crate::engine::collation::resolve_collation(ctx.dyn_ctx, ctx.default_collation.as_ref(), None)?;
        match k {
            crate::engine::collation::CollationKind::Codepoint(a)
            | crate::engine::collation::CollationKind::Other(a) => {
                owned_coll = Some(a);
            }
        }
        owned_coll.as_deref()
    };
    let mut iter = seq.iter();
    let first = match iter.next() {
        Some(XdmItem::Atomic(a)) => as_string(a),
        Some(XdmItem::Node(n)) => n.string_value(),
        None => String::new(), // unreachable due to non-empty
    };
    if let Some(c) = effective_coll {
        let mut best_orig = first.clone();
        let mut best_key = c.key(&first);
        for it in iter {
            let s = match it {
                XdmItem::Atomic(a) => as_string(a),
                XdmItem::Node(n) => n.string_value(),
            };
            let k = c.key(&s);
            let ord = k.cmp(&best_key);
            if (is_min && ord == core::cmp::Ordering::Less) || (!is_min && ord == core::cmp::Ordering::Greater) {
                best_key = k;
                best_orig = s;
            }
        }
        let _arc_hold = &owned_coll; // keep alive
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(best_orig))])
    } else {
        let best = iter.fold(first, |acc, it| {
            let s = match it {
                XdmItem::Atomic(a) => as_string(a),
                XdmItem::Node(n) => n.string_value(),
            };
            let ord = s.cmp(&acc);
            if is_min {
                if ord == core::cmp::Ordering::Less { s } else { acc }
            } else if ord == core::cmp::Ordering::Greater {
                s
            } else {
                acc
            }
        });
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(best))])
    }
}

// ===== Aggregate numeric typing helpers (WP-A) =====
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(super) enum NumericKind {
    Integer,
    Decimal,
    Float,
    Double,
}

impl NumericKind {
    pub(super) fn promote(self, other: NumericKind) -> NumericKind {
        use NumericKind::*;
        match (self, other) {
            (Double, _) | (_, Double) => Double,
            (Float, _) | (_, Float) => {
                if matches!(self, Double) || matches!(other, Double) {
                    Double
                } else {
                    Float
                }
            }
            (Decimal, _) | (_, Decimal) => match (self, other) {
                (Integer, Decimal) | (Decimal, Integer) | (Decimal, Decimal) => Decimal,
                (Decimal, Float) | (Float, Decimal) => Float,
                (Decimal, Double) | (Double, Decimal) => Double,
                _ => Decimal,
            },
            (Integer, Integer) => Integer,
        }
    }
}

pub(super) fn classify_numeric(a: &XdmAtomicValue) -> Result<Option<(NumericKind, f64)>, Error> {
    use XdmAtomicValue::*;
    Ok(match a {
        Integer(i) => Some((NumericKind::Integer, *i as f64)),
        Long(i) => Some((NumericKind::Integer, *i as f64)),
        Int(i) => Some((NumericKind::Integer, *i as f64)),
        Short(i) => Some((NumericKind::Integer, *i as f64)),
        Byte(i) => Some((NumericKind::Integer, *i as f64)),
        UnsignedLong(i) => Some((NumericKind::Integer, *i as f64)),
        UnsignedInt(i) => Some((NumericKind::Integer, *i as f64)),
        UnsignedShort(i) => Some((NumericKind::Integer, *i as f64)),
        UnsignedByte(i) => Some((NumericKind::Integer, *i as f64)),
        NonPositiveInteger(i) => Some((NumericKind::Integer, *i as f64)),
        NegativeInteger(i) => Some((NumericKind::Integer, *i as f64)),
        NonNegativeInteger(i) => Some((NumericKind::Integer, *i as f64)),
        PositiveInteger(i) => Some((NumericKind::Integer, *i as f64)),
        Decimal(d) => Some((NumericKind::Decimal, *d)),
        Float(f) => Some((NumericKind::Float, *f as f64)),
        Double(d) => Some((NumericKind::Double, *d)),
        UntypedAtomic(s) => {
            // Attempt numeric cast; if fails treat as non-numeric (caller will error)
            if let Ok(parsed) = s.parse::<f64>() { Some((NumericKind::Double, parsed)) } else { None }
        }
        String(_) | AnyUri(_) => None,
        Boolean(b) => Some((NumericKind::Integer, if *b { 1.0 } else { 0.0 })),
        _ => None,
    })
}

pub(super) fn a_as_i128(a: &XdmAtomicValue) -> Option<i128> {
    use XdmAtomicValue::*;
    Some(match a {
        Integer(i) => *i as i128,
        Long(i) => *i as i128,
        Int(i) => *i as i128,
        Short(i) => *i as i128,
        Byte(i) => *i as i128,
        UnsignedLong(i) => *i as i128,
        UnsignedInt(i) => *i as i128,
        UnsignedShort(i) => *i as i128,
        UnsignedByte(i) => *i as i128,
        NonPositiveInteger(i) => *i as i128,
        NegativeInteger(i) => *i as i128,
        NonNegativeInteger(i) => *i as i128,
        PositiveInteger(i) => *i as i128,
        Boolean(b) => {
            if *b {
                1
            } else {
                0
            }
        }
        _ => return None,
    })
}

pub(super) fn now_in_effective_tz<N>(ctx: &CallCtx<N>) -> chrono::DateTime<chrono::FixedOffset> {
    // Base instant: context-provided 'now' or system time in UTC
    let base = if let Some(n) = ctx.dyn_ctx.now {
        n
    } else {
        // Use local offset if available; fallback to UTC+00:00
        let utc = chrono::Utc::now();
        let fixed = chrono::Utc.fix();
        utc.with_timezone(&fixed)
    };
    if let Some(tz) = ctx.dyn_ctx.timezone_override { base.with_timezone(&tz) } else { base }
}

// ===== Helpers for component functions =====
pub(super) fn parse_offset(tz: &str) -> Option<ChronoFixedOffset> {
    if tz.len() != 6 {
        return None;
    }
    let sign = &tz[0..1];
    let hours: i32 = tz[1..3].parse().ok()?;
    let mins: i32 = tz[4..6].parse().ok()?;
    let total = hours * 3600 + mins * 60;
    let secs = if sign == "-" { -total } else { total };
    chrono::FixedOffset::east_opt(secs)
}

pub(super) fn get_datetime<N: crate::model::XdmNode>(
    seq: &XdmSequence<N>,
) -> Result<Option<ChronoDateTime<ChronoFixedOffset>>, Error> {
    if seq.is_empty() {
        return Ok(None);
    }
    match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::DateTime(dt)) => Ok(Some(*dt)),
        XdmItem::Atomic(XdmAtomicValue::String(s)) | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            ChronoDateTime::parse_from_rfc3339(s)
                .map(Some)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:dateTime"))
        }
        XdmItem::Node(n) => ChronoDateTime::parse_from_rfc3339(&n.string_value())
            .map(Some)
            .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:dateTime")),
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "not a dateTime")),
    }
}

pub(super) fn get_time<N: crate::model::XdmNode>(
    seq: &XdmSequence<N>,
) -> Result<Option<(NaiveTime, Option<ChronoFixedOffset>)>, Error> {
    if seq.is_empty() {
        return Ok(None);
    }
    match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::Time { time, tz }) => Ok(Some((*time, *tz))),
        XdmItem::Atomic(XdmAtomicValue::String(s)) | XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => {
            crate::util::temporal::parse_time_lex(s)
                .map(Some)
                .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:time"))
        }
        XdmItem::Node(n) => crate::util::temporal::parse_time_lex(&n.string_value())
            .map(Some)
            .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid xs:time")),
        _ => Err(Error::from_code(ErrorCode::XPTY0004, "not a time")),
    }
}

pub(super) fn format_year_month_duration_local(months: i32) -> String {
    if months == 0 {
        return "P0M".to_string();
    }
    let neg = months < 0;
    let mut m = months.abs();
    let y = m / 12;
    m %= 12;
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push('P');
    if y != 0 {
        out.push_str(&format!("{}Y", y));
    }
    if m != 0 {
        out.push_str(&format!("{}M", m));
    }
    if y == 0 && m == 0 {
        out.push('0');
        out.push('M');
    }
    out
}

pub(super) fn format_day_time_duration_local(total_secs: i64) -> String {
    if total_secs == 0 {
        return "PT0S".to_string();
    }
    let neg = total_secs < 0;
    let mut s = total_secs.abs();
    let days = s / (24 * 3600);
    s %= 24 * 3600;
    let hours = s / 3600;
    s %= 3600;
    let mins = s / 60;
    s %= 60;
    let secs = s;
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push('P');
    if days != 0 {
        out.push_str(&format!("{}D", days));
    }
    if hours != 0 || mins != 0 || secs != 0 {
        out.push('T');
    }
    if hours != 0 {
        out.push_str(&format!("{}H", hours));
    }
    if mins != 0 {
        out.push_str(&format!("{}M", mins));
    }
    if secs != 0 {
        out.push_str(&format!("{}S", secs));
    }
    out
}

pub(super) fn fmt_offset_local(off: &ChronoFixedOffset) -> String {
    let secs = off.local_minus_utc();
    let sign = if secs < 0 { '-' } else { '+' };
    let mut s = secs.abs();
    let hours = s / 3600;
    s %= 3600;
    let mins = s / 60;
    format!("{}{:02}:{:02}", sign, hours, mins)
}

pub(super) fn parse_xs_date_local(s: &str) -> Result<(NaiveDate, Option<ChronoFixedOffset>), ()> {
    if s.ends_with('Z') || s.ends_with('z') {
        let d = &s[..s.len() - 1];
        let date = NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| ())?;
        return Ok((date, Some(ChronoFixedOffset::east_opt(0).ok_or(())?)));
    }
    if let Some(pos) = s.rfind(['+', '-'])
        && pos >= 10
    {
        let (d, tzs) = s.split_at(pos);
        let date = NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| ())?;
        let off = parse_offset(tzs).ok_or(())?;
        return Ok((date, Some(off)));
    }
    let date = NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| ())?;
    Ok((date, None))
}

// ===== Additional Helpers =====

pub(super) fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_space = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !in_space {
                out.push(' ');
                in_space = true;
            }
        } else {
            out.push(ch);
            in_space = false;
        }
    }
    if out.starts_with(' ') {
        out.remove(0);
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

pub(super) fn replace_whitespace(s: &str) -> String {
    s.chars().map(|c| if matches!(c, '\u{0009}' | '\u{000A}' | '\u{000D}') { ' ' } else { c }).collect()
}

pub(crate) fn parse_qname_lexical(s: &str) -> Result<(Option<String>, String), ()> {
    if s.is_empty() {
        return Err(());
    }
    if let Some(pos) = s.find(':') {
        let (p, l) = s.split_at(pos);
        let local = &l[1..];
        if !is_valid_ncname(p) || !is_valid_ncname(local) {
            return Err(());
        }
        Ok((Some(p.to_string()), local.to_string()))
    } else {
        if !is_valid_ncname(s) {
            return Err(());
        }
        Ok((None, s.to_string()))
    }
}

pub(super) fn is_valid_ncname(s: &str) -> bool {
    // ASCII-only approximation: [_A-Za-z] [_A-Za-z0-9.-]* (no colon)
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if first == ':' || !(first == '_' || first.is_ascii_alphabetic()) {
        return false;
    }
    for ch in chars {
        if ch == ':' || !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.') {
            return false;
        }
    }
    true
}

pub(super) fn is_valid_language(s: &str) -> bool {
    // Simple BCP47-ish: 1-8 alpha, then (- 1-8 alnum) repeated
    let mut parts = s.split('-');
    if let Some(first) = parts.next() {
        if !(1..=8).contains(&first.len()) || !first.chars().all(|c| c.is_ascii_alphabetic()) {
            return false;
        }
    } else {
        return false;
    }
    for p in parts {
        if !(1..=8).contains(&p.len()) || !p.chars().all(|c| c.is_ascii_alphanumeric()) {
            return false;
        }
    }
    true
}

pub(crate) fn parse_duration_lexical(s: &str) -> Result<(Option<i32>, Option<i64>), Error> {
    if let Ok(m) = parse_year_month_duration_months(s) {
        return Ok((Some(m), None));
    }
    if let Ok(sec) = parse_day_time_duration_secs(s) {
        return Ok((None, Some(sec)));
    }
    Err(Error::from_code(ErrorCode::FORG0001, "invalid xs:duration"))
}

pub(crate) fn parse_year_month_duration_months(s: &str) -> Result<i32, ()> {
    // Pattern: -?P(\d+Y)?(\d+M)? with at least one present
    let s = s.trim();
    if s.is_empty() {
        return Err(());
    }
    let neg = s.starts_with('-');
    let body = if neg { &s[1..] } else { s };
    if !body.starts_with('P') {
        return Err(());
    }
    let mut years: i32 = 0;
    let mut months: i32 = 0;
    let mut cur = &body[1..];
    let mut consumed_any = false;
    while !cur.is_empty() {
        // find next number
        let mut i = 0;
        while i < cur.len() && cur.as_bytes()[i].is_ascii_digit() {
            i += 1;
        }
        if i == 0 {
            break;
        }
        let n: i32 = cur[..i].parse().map_err(|_| ())?;
        cur = &cur[i..];
        if cur.starts_with('Y') {
            years = n;
            cur = &cur[1..];
            consumed_any = true;
        } else if cur.starts_with('M') {
            months = n;
            cur = &cur[1..];
            consumed_any = true;
        } else {
            return Err(());
        }
    }
    if !consumed_any || !cur.is_empty() {
        return Err(());
    }
    let total = years.checked_mul(12).ok_or(())?.checked_add(months).ok_or(())?;
    Ok(if neg { -total } else { total })
}

/// Parse an `xs:dayTimeDuration` literal into whole seconds.
///
/// Fractional seconds are truncated towards zero to match the current internal
/// representation (`XdmAtomicValue::DayTimeDuration` stores integral seconds).
pub(crate) fn parse_day_time_duration_secs(s: &str) -> Result<i64, ()> {
    // Pattern: -?P(\d+D)?(T(\d+H)?(\d+M)?(\d+(\.\d+)?S)?)?
    let s = s.trim();
    if s.is_empty() {
        return Err(());
    }
    let neg = s.starts_with('-');
    let body = if neg { &s[1..] } else { s };
    if !body.starts_with('P') {
        return Err(());
    }
    let mut cur = &body[1..];
    let mut days: i64 = 0;
    let mut hours: i64 = 0;
    let mut mins: i64 = 0;
    let mut secs: f64 = 0.0;
    // days
    if !cur.is_empty() {
        let mut i = 0;
        while i < cur.len() && cur.as_bytes()[i].is_ascii_digit() {
            i += 1;
        }
        if i > 0 && cur[i..].starts_with('D') {
            days = cur[..i].parse().map_err(|_| ())?;
            cur = &cur[i + 1..];
        }
    }
    if cur.starts_with('T') {
        cur = &cur[1..];
        // hours
        if !cur.is_empty() {
            let mut i = 0;
            while i < cur.len() && cur.as_bytes()[i].is_ascii_digit() {
                i += 1;
            }
            if i > 0 && cur[i..].starts_with('H') {
                hours = cur[..i].parse().map_err(|_| ())?;
                cur = &cur[i + 1..];
            }
        }
        // minutes
        if !cur.is_empty() {
            let mut i = 0;
            while i < cur.len() && cur.as_bytes()[i].is_ascii_digit() {
                i += 1;
            }
            if i > 0 && cur[i..].starts_with('M') {
                mins = cur[..i].parse().map_err(|_| ())?;
                cur = &cur[i + 1..];
            }
        }
        // seconds (allow fractional)
        if !cur.is_empty() {
            let mut i = 0;
            while i < cur.len() && (cur.as_bytes()[i].is_ascii_digit() || cur.as_bytes()[i] == b'.') {
                i += 1;
            }
            if i > 0 && cur[i..].starts_with('S') {
                secs = cur[..i].parse().map_err(|_| ())?;
                cur = &cur[i + 1..];
            }
        }
    }
    if !cur.is_empty() {
        return Err(());
    }
    let mut total = days
        .checked_mul(24 * 3600)
        .ok_or(())?
        .checked_add(hours.checked_mul(3600).ok_or(())?)
        .ok_or(())?
        .checked_add(mins.checked_mul(60).ok_or(())?)
        .ok_or(())? as f64
        + secs;
    if neg {
        total = -total;
    }
    Ok(total.trunc() as i64)
}

pub(super) fn int_subtype_i64<N: crate::model::XdmNode>(
    args: &[XdmSequence<N>],
    min: i64,
    max: i64,
    mk: impl Fn(i64) -> XdmAtomicValue,
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&args[0]).trim().to_string();
    let v: i64 = s.parse().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid integer"))?;
    if v < min || v > max {
        return Err(Error::from_code(ErrorCode::FORG0001, "out of range"));
    }
    Ok(vec![XdmItem::Atomic(mk(v))])
}

pub(super) fn uint_subtype_u128<N: crate::model::XdmNode>(
    args: &[XdmSequence<N>],
    min: u128,
    max: u128,
    mk: impl Fn(u128) -> XdmAtomicValue,
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = item_to_string(&args[0]).trim().to_string();
    if s.starts_with('-') {
        return Err(Error::from_code(ErrorCode::FORG0001, "negative not allowed"));
    }
    let v: u128 = s.parse().map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid unsigned integer"))?;
    if v < min || v > max {
        return Err(Error::from_code(ErrorCode::FORG0001, "out of range"));
    }
    Ok(vec![XdmItem::Atomic(mk(v))])
}

pub(super) fn str_name_like<N: crate::model::XdmNode>(
    args: &[XdmSequence<N>],
    require_start: bool,
    allow_colon: bool,
    mk: impl Fn(String) -> XdmAtomicValue,
) -> Result<XdmSequence<N>, Error> {
    if args[0].is_empty() {
        return Ok(vec![]);
    }
    if args[0].len() > 1 {
        return Err(Error::from_code(ErrorCode::FORG0006, "constructor expects at most one item"));
    }
    let s = collapse_whitespace(&item_to_string(&args[0]));
    // Simplified validation
    if require_start {
        let mut chars = s.chars();
        if let Some(first) = chars.next() {
            if !(first == '_' || first.is_ascii_alphabetic() || (allow_colon && first == ':')) {
                return Err(Error::from_code(ErrorCode::FORG0001, "invalid Name"));
            }
            for ch in chars {
                if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || ch == '.' || (allow_colon && ch == ':')) {
                    return Err(Error::from_code(ErrorCode::FORG0001, "invalid Name"));
                }
            }
        } else {
            return Err(Error::from_code(ErrorCode::FORG0001, "invalid Name"));
        }
    }
    Ok(vec![XdmItem::Atomic(mk(s))])
}
