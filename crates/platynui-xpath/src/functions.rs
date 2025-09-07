//! Function families per XQuery and XPath Functions and Operators.
//! Minimal default registry with a few core functions to bootstrap evaluator tests.

use crate::runtime::{Error, FunctionRegistry};
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence, ExpandedName};

const FNS: &str = "http://www.w3.org/2005/xpath-functions";

fn ebv<N>(seq: &XdmSequence<N>) -> Result<bool, Error> {
    match seq.len() {
        0 => Ok(false),
        1 => match &seq[0] {
            XdmItem::Atomic(XdmAtomicValue::Boolean(b)) => Ok(*b),
            XdmItem::Atomic(XdmAtomicValue::String(s)) => Ok(!s.is_empty()),
            XdmItem::Atomic(XdmAtomicValue::Integer(i)) => Ok(*i != 0),
            XdmItem::Atomic(XdmAtomicValue::Double(d)) => Ok(*d != 0.0 && !d.is_nan()),
            XdmItem::Atomic(XdmAtomicValue::Float(f)) => Ok(*f != 0.0 && !f.is_nan()),
            XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => Ok(!s.is_empty()),
            XdmItem::Atomic(_) => Err(Error::dynamic_err("err:FORG0006", "EBV for this atomic type not supported yet")),
            XdmItem::Node(_) => Ok(true),
        },
        _ => Err(Error::dynamic_err("err:FORG0006", "EBV of sequence with more than one item")),
    }
}

type FnSig<N> = fn(&[XdmSequence<N>]) -> Result<XdmSequence<N>, Error>;

pub fn default_function_registry<N: 'static + Send + Sync + crate::model::XdmNode + Clone>() -> FunctionRegistry<N> {
    let mut reg = FunctionRegistry::new();
    // helper to register under default namespace
    let mut add = |local: &str, arity: usize, f: FnSig<N>| {
        let fun = std::sync::Arc::new(move |args: &[XdmSequence<N>]| f(args));
        reg.register(ExpandedName { ns_uri: Some(FNS.to_string()), local: local.to_string() }, arity, fun.clone());
        reg.register(ExpandedName { ns_uri: None, local: local.to_string() }, arity, fun);
    };

    // ===== Core booleans =====
    add("true", 0, |_args| Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(true))]));
    add("false", 0, |_args| Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(false))]));
    add("not", 1, |args| {
        let b = ebv(&args[0])?;
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(!b))])
    });

    // ===== String family =====
    add("string", 1, |args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(item_to_string(&args[0])) )])
    });
    add("string-length", 1, |args| {
        let s = item_to_string(&args[0]);
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(s.chars().count() as i64))])
    });
    for ar in 2..=5 {
        add("concat", ar, |args| {
            let mut out = String::new();
            for a in args { out.push_str(&item_to_string(a)); }
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
        });
    }
    add("contains", 2, |args| {
        let s = item_to_string(&args[0]);
        let sub = item_to_string(&args[1]);
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(s.contains(&sub)))])
    });
    add("starts-with", 2, |args| {
        let s = item_to_string(&args[0]);
        let sub = item_to_string(&args[1]);
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(s.starts_with(&sub)))])
    });
    add("ends-with", 2, |args| {
        let s = item_to_string(&args[0]);
        let sub = item_to_string(&args[1]);
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(s.ends_with(&sub)))])
    });
    add("substring", 2, |args| {
        let s = item_to_string(&args[0]);
        let start = to_number(&args[1])?; // 1-based
        let from = (start.floor() as isize - 1).max(0) as usize;
        let out: String = s.chars().skip(from).collect();
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
    });
    add("substring", 3, |args| {
        let s = item_to_string(&args[0]);
        let start = to_number(&args[1])?;
        let len = to_number(&args[2])?;
        let from = (start.floor() as isize - 1).max(0) as usize;
        let take = len.floor().max(0.0) as usize;
        let out: String = s.chars().skip(from).take(take).collect();
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
    });

    // substring-before/after
    add("substring-before", 2, |args| {
        let s = item_to_string(&args[0]);
        let sub = item_to_string(&args[1]);
        if sub.is_empty() || s.is_empty() { return Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))]); }
        if let Some(idx) = s.find(&sub) {
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s[..idx].to_string()))])
        } else { Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))]) }
    });
    add("substring-after", 2, |args| {
        let s = item_to_string(&args[0]);
        let sub = item_to_string(&args[1]);
        if sub.is_empty() { return Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(s))]); }
        if let Some(idx) = s.find(&sub) {
            let after = &s[idx + sub.len()..];
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(after.to_string()))])
        } else { Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(String::new()))]) }
    });

    // normalize-space (1-arg variant)
    add("normalize-space", 1, |args| {
        let s = item_to_string(&args[0]);
        let mut out = String::new();
        let mut in_space = true; // leading spaces skipped
        for ch in s.chars() {
            if ch.is_whitespace() {
                if !in_space { out.push(' '); in_space = true; }
            } else {
                out.push(ch);
                in_space = false;
            }
        }
        if out.ends_with(' ') { out.pop(); }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
    });

    // translate($s, $map, $trans)
    add("translate", 3, |args| {
        let s = item_to_string(&args[0]);
        let map = item_to_string(&args[1]);
        let trans = item_to_string(&args[2]);
        use std::collections::HashMap;
        let mut table: HashMap<char, Option<char>> = HashMap::new();
        let mut trans_iter = trans.chars();
        for m in map.chars() {
            use std::collections::hash_map::Entry;
            match table.entry(m) {
                Entry::Vacant(e) => { let repl = trans_iter.next(); e.insert(repl); }
                Entry::Occupied(_) => { let _ = trans_iter.next(); }
            }
        }
        let mut out = String::new();
        for ch in s.chars() {
            if let Some(opt) = table.get(&ch) {
                if let Some(rep) = opt { out.push(*rep); } // else drop char
            } else { out.push(ch); }
        }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(out))])
    });

    // lower-case / upper-case
    add("lower-case", 1, |args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(item_to_string(&args[0]).to_lowercase()))])
    });
    add("upper-case", 1, |args| {
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(item_to_string(&args[0]).to_uppercase()))])
    });

    // string-join($seq, $sep)
    add("string-join", 2, |args| {
        let sep = item_to_string(&args[1]);
        let mut parts: Vec<String> = Vec::new();
        for it in &args[0] { match it { XdmItem::Atomic(a) => parts.push(as_string(a)), XdmItem::Node(n) => parts.push(n.string_value()) } }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(parts.join(&sep)))])
    });

    // ===== Numeric family =====
    add("abs", 1, |args| Ok(num_unary(args, |n| n.abs())));
    add("floor", 1, |args| Ok(num_unary(args, |n| n.floor())));
    add("ceiling", 1, |args| Ok(num_unary(args, |n| n.ceil())));
    add("round", 1, |args| Ok(num_unary(args, |n| n.round())));
    add("sum", 1, |args| {
        let mut total = 0.0;
        for it in &args[0] { if let XdmItem::Atomic(a) = it { total += to_number_atomic(a)?; } }
        Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(total))])
    });
    add("avg", 1, |args| {
        let mut total = 0.0; let mut c = 0.0;
        for it in &args[0] { if let XdmItem::Atomic(a) = it { total += to_number_atomic(a)?; c += 1.0; } }
        if c == 0.0 { Ok(vec![]) } else { Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(total / c))]) }
    });

    // ===== Sequence family =====
    add("empty", 1, |args| Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(args[0].is_empty()))]));
    add("exists", 1, |args| Ok(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(!args[0].is_empty()))]));
    add("count", 1, |args| Ok(vec![XdmItem::Atomic(XdmAtomicValue::Integer(args[0].len() as i64))]));
    add("reverse", 1, |args| { let s: XdmSequence<N> = args[0].iter().cloned().rev().collect(); Ok(s) });
    add("subsequence", 2, |args| {
        let s = &args[0]; let start = to_number(&args[1])?; let from = (start.floor() as isize - 1).max(0) as usize; Ok(s.iter().skip(from).cloned().collect())
    });
    add("subsequence", 3, |args| {
        let s = &args[0]; let start = to_number(&args[1])?; let len = to_number(&args[2])?; let from = (start.floor() as isize - 1).max(0) as usize; let take = len.floor().max(0.0) as usize; Ok(s.iter().skip(from).cloned().take(take).collect())
    });

    // distinct-values($seq as xs:anyAtomicType*)
    add("distinct-values", 1, |args| {
        use std::collections::HashSet;
        let mut seen: HashSet<String> = HashSet::new();
        let mut out: XdmSequence<N> = Vec::new();
        for it in &args[0] {
            let (key, push_item) = match it {
                XdmItem::Atomic(a) => { let s = as_string(a); (s.clone(), XdmItem::Atomic(XdmAtomicValue::String(s))) }
                XdmItem::Node(n) => { let s = n.string_value(); (s.clone(), XdmItem::Atomic(XdmAtomicValue::String(s))) }
            };
            if seen.insert(key) { out.push(push_item); }
        }
        Ok(out)
    });

    // index-of($seq, $search)
    add("index-of", 2, |args| {
        let mut out: XdmSequence<N> = Vec::new();
        // Use string compare if either side is non-numeric; else numeric equality
        for (i, it) in args[0].iter().enumerate() {
            let eq = match (it, args[1].first()) {
                (XdmItem::Atomic(a), Some(XdmItem::Atomic(b))) => {
                    // numeric if possible, else string
                    match (to_number_atomic(a), to_number_atomic(b)) { (Ok(na), Ok(nb)) => na == nb, _ => as_string(a) == as_string(b) }
                }
                (XdmItem::Node(n), Some(XdmItem::Node(m))) => n.string_value() == m.string_value(),
                (XdmItem::Node(n), Some(XdmItem::Atomic(b))) => n.string_value() == as_string(b),
                (XdmItem::Atomic(a), Some(XdmItem::Node(n))) => as_string(a) == n.string_value(),
                _ => false,
            };
            if eq { out.push(XdmItem::Atomic(XdmAtomicValue::Integer(i as i64 + 1))); }
        }
        Ok(out)
    });

    // insert-before($seq, $pos, $item)
    add("insert-before", 3, |args| {
        let mut out: XdmSequence<N> = Vec::new();
        let pos = to_number(&args[1])?.floor() as isize; // 1-based
        let insert_at = pos.max(1) as usize;
        let mut i = 1usize;
        for it in &args[0] {
            if i == insert_at { out.extend(args[2].iter().cloned()); }
            out.push(it.clone());
            i += 1;
        }
        if insert_at > args[0].len() { out.extend(args[2].iter().cloned()); }
        Ok(out)
    });

    // remove($seq, $pos)
    add("remove", 2, |args| {
        let mut out: XdmSequence<N> = Vec::new();
        let pos = to_number(&args[1])?.floor() as isize; // 1-based
        let remove_at = pos.max(1) as usize;
        for (i, it) in args[0].iter().enumerate() {
            if i + 1 != remove_at { out.push(it.clone()); }
        }
        Ok(out)
    });

    // min/max
    add("min", 1, |args| {
        if args[0].is_empty() { return Ok(vec![]); }
        // numeric if all numeric, else string
        let mut all_num = true;
        for it in &args[0] { if let XdmItem::Atomic(a) = it { if to_number_atomic(a).is_err() { all_num = false; break; } } else { all_num = false; break; } }
        if all_num {
            let mut m = f64::INFINITY;
            for it in &args[0] { if let XdmItem::Atomic(a) = it { m = m.min(to_number_atomic(a).unwrap()); } }
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(m))])
        } else {
            let mut best: Option<String> = None;
            for it in &args[0] { let s = match it { XdmItem::Atomic(a) => as_string(a), XdmItem::Node(n) => n.string_value() }; best = Some(match best { None => s, Some(b) => if s < b { s } else { b } }) }
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(best.unwrap()))])
        }
    });
    add("max", 1, |args| {
        if args[0].is_empty() { return Ok(vec![]); }
        let mut all_num = true;
        for it in &args[0] { if let XdmItem::Atomic(a) = it { if to_number_atomic(a).is_err() { all_num = false; break; } } else { all_num = false; break; } }
        if all_num {
            let mut m = f64::NEG_INFINITY;
            for it in &args[0] { if let XdmItem::Atomic(a) = it { m = m.max(to_number_atomic(a).unwrap()); } }
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::Double(m))])
        } else {
            let mut best: Option<String> = None;
            for it in &args[0] { let s = match it { XdmItem::Atomic(a) => as_string(a), XdmItem::Node(n) => n.string_value() }; best = Some(match best { None => s, Some(b) => if s > b { s } else { b } }) }
            Ok(vec![XdmItem::Atomic(XdmAtomicValue::String(best.unwrap()))])
        }
    });

    reg
}

fn item_to_string<N: crate::model::XdmNode>(seq: &XdmSequence<N>) -> String {
    if seq.is_empty() { return String::new(); }
    match &seq[0] {
        XdmItem::Atomic(a) => as_string(a),
        XdmItem::Node(n) => n.string_value(),
    }
}

fn as_string(a: &XdmAtomicValue) -> String {
    match a {
        XdmAtomicValue::String(s) => s.clone(),
        XdmAtomicValue::UntypedAtomic(s) => s.clone(),
        XdmAtomicValue::AnyUri(u) => u.clone(),
        XdmAtomicValue::Boolean(b) => if *b { "true".into() } else { "false".into() },
        XdmAtomicValue::Integer(i) => i.to_string(),
        XdmAtomicValue::Double(d) => d.to_string(),
        XdmAtomicValue::Float(f) => f.to_string(),
        XdmAtomicValue::Decimal(d) => d.to_string(),
        XdmAtomicValue::QName{prefix, local, ..} => { if let Some(p) = prefix { format!("{}:{}", p, local) } else { local.clone() } }
    }
}

fn to_number<N: crate::model::XdmNode>(seq: &XdmSequence<N>) -> Result<f64, Error> {
    if seq.is_empty() { return Ok(f64::NAN); }
    if seq.len() != 1 { return Err(Error::dynamic_err("err:FORG0006", "expects single item")); }
    match &seq[0] { XdmItem::Atomic(a) => to_number_atomic(a), XdmItem::Node(n) => n.string_value().parse::<f64>().map_err(|_| Error::dynamic_err("err:FORG0001", "invalid number")) }
}

fn to_number_atomic(a: &XdmAtomicValue) -> Result<f64, Error> {
    match a {
        XdmAtomicValue::Integer(i) => Ok(*i as f64),
        XdmAtomicValue::Double(d) => Ok(*d),
        XdmAtomicValue::Float(f) => Ok(*f as f64),
        XdmAtomicValue::Decimal(d) => Ok(*d),
        XdmAtomicValue::UntypedAtomic(s) | XdmAtomicValue::String(s) | XdmAtomicValue::AnyUri(s) => s.parse::<f64>().map_err(|_| Error::dynamic_err("err:FORG0001", "invalid number")),
        XdmAtomicValue::Boolean(b) => Ok(if *b { 1.0 } else { 0.0 }),
        XdmAtomicValue::QName{..} => Err(Error::dynamic_err("err:XPTY0004", "cannot cast QName to number")),
    }
}

fn num_unary<N: crate::model::XdmNode>(args: &[XdmSequence<N>], f: impl Fn(f64) -> f64) -> XdmSequence<N> {
    let n = to_number(&args[0]).unwrap_or(f64::NAN);
    vec![XdmItem::Atomic(XdmAtomicValue::Double(f(n)))]
}
