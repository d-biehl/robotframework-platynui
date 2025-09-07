use crate::compiler::{
    AxisIR, ComparisonOp, CompiledIR, ItemTypeIR, NodeTestIR, OccurrenceIR, OpCode, SeqTypeIR,
    SingleTypeIR, compile_xpath as compile_to_ir,
};
use crate::runtime::{
    Collation, DynamicContext, DynamicContextBuilder as Builder, Error, StaticContext,
};
use crate::xdm::ExpandedName;
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};

#[derive(Debug, Clone)]
pub struct XPathExecutable(CompiledIR);

impl XPathExecutable {
    pub fn evaluate<N: crate::model::XdmNode>(
        &self,
        _dyn_ctx: &DynamicContext<N>,
    ) -> Result<XdmSequence<N>, Error> {
        self.run_instrs::<N>(&self.0.instrs, _dyn_ctx, 1, 1)
    }

    fn run_instrs<N: crate::model::XdmNode>(
        &self,
        instrs: &crate::compiler::InstrSeq,
        dyn_ctx: &DynamicContext<N>,
        position: i64,
        last: i64,
    ) -> Result<XdmSequence<N>, Error> {
        let mut stack: Vec<XdmSequence<N>> = Vec::new();
        let code = &instrs.0;
        let mut ip: usize = 0;
        while ip < code.len() {
            match &code[ip] {
                OpCode::PushAtomic(a) => {
                    stack.push(vec![XdmItem::Atomic(a.clone())]);
                }
                OpCode::Add => bin_num2(|a, b| Ok(num_add(a, b)), &mut stack)?,
                OpCode::Sub => bin_num2(|a, b| Ok(num_sub(a, b)), &mut stack)?,
                OpCode::Mul => bin_num2(|a, b| Ok(num_mul(a, b)), &mut stack)?,
                OpCode::Div => bin_num2(num_div, &mut stack)?,
                OpCode::IDiv => bin_num2(num_idiv, &mut stack)?,
                OpCode::Mod => bin_num2(|a, b| Ok(num_mod(a, b)), &mut stack)?,
                OpCode::And => bin_bool(|l, r| l && r, &mut stack)?,
                OpCode::Or => bin_bool(|l, r| l || r, &mut stack)?,
                OpCode::ToEBV => {
                    let s = stack
                        .pop()
                        .ok_or_else(|| Error::dynamic_err("err:FOER0000", "stack underflow"))?;
                    let b = ebv(&s)?;
                    stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                }
                OpCode::Pop => {
                    stack
                        .pop()
                        .ok_or_else(|| Error::dynamic_err("err:FOER0000", "stack underflow"))?;
                }
                OpCode::JumpIfTrue(off) => {
                    let top = stack
                        .last()
                        .ok_or_else(|| Error::dynamic_err("err:FOER0000", "stack underflow"))?;
                    let b = ebv(top)?;
                    if b {
                        ip += off;
                        continue;
                    }
                }
                OpCode::JumpIfFalse(off) => {
                    let top = stack
                        .last()
                        .ok_or_else(|| Error::dynamic_err("err:FOER0000", "stack underflow"))?;
                    let b = ebv(top)?;
                    if !b {
                        ip += off;
                        continue;
                    }
                }
                OpCode::Position => {
                    stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Integer(position))]);
                }
                OpCode::Last => {
                    stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Integer(last))]);
                }
                OpCode::LoadContextItem => {
                    if let Some(ci) = &dyn_ctx.context_item {
                        stack.push(vec![ci.clone()]);
                    } else {
                        return Err(Error::dynamic_err(
                            "err:XPDY0002",
                            "no context item defined",
                        ));
                    }
                }
                OpCode::LoadVarByName(name) => {
                    if let Some(val) = dyn_ctx.variables.get(name) {
                        stack.push(val.clone());
                    } else {
                        return Err(Error::dynamic_err(
                            "err:XPST0008",
                            format!("unknown variable {}", name.local),
                        ));
                    }
                }
                OpCode::ToRoot => {
                    let seq = stack
                        .pop()
                        .ok_or_else(|| Error::dynamic_err("err:FOER0000", "stack underflow"))?;
                    let mut out = Vec::new();
                    let nodes = node_seq(seq)?;
                    for n in nodes {
                        out.push(XdmItem::Node(root_of(n)));
                    }
                    stack.push(out);
                }
                OpCode::AxisStep(axis, test, preds) => {
                    let seq = stack
                        .pop()
                        .ok_or_else(|| Error::dynamic_err("err:FOER0000", "stack underflow"))?;
                    let mut nodes = node_seq(seq)?;
                    let mut acc: Vec<N> = Vec::new();
                    for n in nodes.drain(..) {
                        let step_nodes = apply_axis(&n, axis);
                        for s in step_nodes {
                            if matches_test(&s, test) {
                                acc.push(s);
                            }
                        }
                    }
                    // dedup
                    let mut unique: Vec<N> = Vec::new();
                    'outer: for n in acc.into_iter() {
                        for u in &unique {
                            if *u == n {
                                continue 'outer;
                            }
                        }
                        unique.push(n);
                    }
                    // sort in document order
                    unique.sort_by(|a, b| a.compare_document_order(b));
                    // Apply predicates in order
                    let mut filtered: Vec<N> = unique;
                    for pred in preds {
                        let total = filtered.len() as i64;
                        let mut next: Vec<N> = Vec::new();
                        for (idx, node) in filtered.into_iter().enumerate() {
                            let pos = (idx as i64) + 1;
                            let ctx_item = XdmItem::Node(node.clone());
                            let local_ctx = DynamicContext {
                                context_item: Some(ctx_item),
                                variables: dyn_ctx.variables.clone(),
                                default_collation: dyn_ctx.default_collation.clone(),
                                functions: dyn_ctx.functions.clone(),
                                collations: dyn_ctx.collations.clone(),
                                resolver: dyn_ctx.resolver.clone(),
                                regex: dyn_ctx.regex.clone(),
                            };
                            let res = self.run_instrs::<N>(pred, &local_ctx, pos, total)?;
                            if predicate_truthy(&res, pos)? {
                                next.push(node);
                            }
                        }
                        filtered = next;
                    }
                    stack.push(filtered.into_iter().map(XdmItem::Node).collect());
                }
                OpCode::MakeSeq(n) => {
                    let mut out: XdmSequence<N> = Vec::new();
                    for _ in 0..*n {
                        let mut s = stack
                            .pop()
                            .ok_or_else(|| Error::dynamic_err("err:FOER0000", "stack underflow"))?;
                        out.splice(0..0, s.drain(..)); // prepend to preserve left-to-right
                    }
                    stack.push(out);
                }
                OpCode::CallByName(name, argc) => {
                    let mut args: Vec<XdmSequence<N>> = Vec::with_capacity(*argc);
                    for _ in 0..*argc {
                        args.push(stack.pop().ok_or_else(|| {
                            Error::dynamic_err("err:FOER0000", "stack underflow")
                        })?);
                    }
                    args.reverse();
                    if let Some(fun) = dyn_ctx.functions.get(name, *argc) {
                        let res = (fun)(&args)?;
                        stack.push(res);
                    } else {
                        return Err(Error::static_err(
                            "err:XPST0017",
                            format!("unknown function {} with arity {}", name.local, argc),
                        ));
                    }
                }
                OpCode::CompareValue(code) => {
                    let (l, r) = take2(&mut stack)?;
                    let coll = resolve_default_collation(self, dyn_ctx);
                    let b = compare_value_with_collation(&l, &r, *code, coll.as_deref())?;
                    stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                }
                OpCode::CompareGeneral(code) => {
                    let (l, r) = take2(&mut stack)?;
                    let coll = resolve_default_collation(self, dyn_ctx);
                    let b = compare_general(&l, &r, *code, coll.as_deref())?;
                    stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                }
                OpCode::Union => {
                    let (l, r) = take2(&mut stack)?;
                    let mut nodes = node_seq(l)?;
                    nodes.extend(node_seq(r)?);
                    dedup_and_sort(&mut nodes);
                    stack.push(nodes.into_iter().map(XdmItem::Node).collect());
                }
                OpCode::Intersect => {
                    let (l, r) = take2(&mut stack)?;
                    let mut ln = node_seq(l)?;
                    let rn = node_seq(r)?;
                    ln.retain(|n| rn.iter().any(|m| m == n));
                    dedup_and_sort(&mut ln);
                    stack.push(ln.into_iter().map(XdmItem::Node).collect());
                }
                OpCode::Except => {
                    let (l, r) = take2(&mut stack)?;
                    let mut ln = node_seq(l)?;
                    let rn = node_seq(r)?;
                    ln.retain(|n| !rn.iter().any(|m| m == n));
                    dedup_and_sort(&mut ln);
                    stack.push(ln.into_iter().map(XdmItem::Node).collect());
                }
                OpCode::RangeTo => {
                    let (l, r) = take2(&mut stack)?;
                    let la = atomize_sequence(&l)?;
                    let ra = atomize_sequence(&r)?;
                    if la.len() != 1 || ra.len() != 1 {
                        return Err(Error::dynamic_err(
                            "err:FORG0006",
                            "range expects exactly one atomic value per side",
                        ));
                    }
                    let start = as_integer(&la[0])?;
                    let end = as_integer(&ra[0])?;
                    let mut out: XdmSequence<N> = Vec::new();
                    if start <= end {
                        for i in start..=end {
                            out.push(XdmItem::Atomic(XdmAtomicValue::Integer(i)));
                        }
                    }
                    stack.push(out);
                }
                OpCode::NodeIs => {
                    let (l, r) = take2(&mut stack)?;
                    let ln = single_node(l)?;
                    let rn = single_node(r)?;
                    stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(ln == rn))]);
                }
                OpCode::NodeBefore => {
                    let (l, r) = take2(&mut stack)?;
                    let ln = single_node(l)?;
                    let rn = single_node(r)?;
                    let b = ln.compare_document_order(&rn) == core::cmp::Ordering::Less;
                    stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                }
                OpCode::NodeAfter => {
                    let (l, r) = take2(&mut stack)?;
                    let ln = single_node(l)?;
                    let rn = single_node(r)?;
                    let b = ln.compare_document_order(&rn) == core::cmp::Ordering::Greater;
                    stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                }
                OpCode::Castable(t) => {
                    let s = stack
                        .pop()
                        .ok_or_else(|| Error::dynamic_err("err:FOER0000", "stack underflow"))?;
                    let b = is_castable::<N>(&s, t)?;
                    stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                }
                OpCode::Cast(t) => {
                    let s = stack
                        .pop()
                        .ok_or_else(|| Error::dynamic_err("err:FOER0000", "stack underflow"))?;
                    let out = do_cast::<N>(&s, t)?;
                    stack.push(out);
                }
                OpCode::InstanceOf(t) => {
                    let s = stack
                        .pop()
                        .ok_or_else(|| Error::dynamic_err("err:FOER0000", "stack underflow"))?;
                    let b = instance_of::<N>(&s, t);
                    stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                }
                OpCode::Treat(t) => {
                    let s = stack
                        .pop()
                        .ok_or_else(|| Error::dynamic_err("err:FOER0000", "stack underflow"))?;
                    if instance_of::<N>(&s, t) {
                        stack.push(s);
                    } else {
                        return Err(Error::dynamic_err("err:XPTY0004", "treat as type mismatch"));
                    }
                }
                other => return Err(Error::not_implemented(&format!("opcode {:?}", other))),
            }
            ip += 1;
        }
        Ok(stack.pop().unwrap_or_default())
    }

    pub fn evaluate_on<N: crate::model::XdmNode + 'static>(
        &self,
        context_item: impl Into<Option<N>>,
    ) -> Result<XdmSequence<N>, Error> {
        let mut builder: Builder<N> = Builder::new();
        if let Some(ci) = context_item.into() {
            builder = builder.with_context_item(crate::xdm::XdmItem::Node(ci));
        }
        self.evaluate(&builder.build())
    }

    pub fn evaluate_with_vars<N: crate::model::XdmNode + 'static>(
        &self,
        context_item: impl Into<Option<N>>,
        vars: impl IntoIterator<Item = (crate::xdm::ExpandedName, XdmSequence<N>)>,
    ) -> Result<XdmSequence<N>, Error> {
        let mut builder: Builder<N> = Builder::new();
        if let Some(ci) = context_item.into() {
            builder = builder.with_context_item(crate::xdm::XdmItem::Node(ci));
        }
        let mut builder = builder;
        for (k, v) in vars.into_iter() {
            builder = builder.with_variable(k, v);
        }
        self.evaluate(&builder.build())
    }

    pub fn debug_dump_ir(&self) -> String {
        use core::fmt::Write as _;
        let mut s = String::new();
        let _ = write!(&mut s, "{}", self.0.instrs);
        s
    }
}

fn take2<N>(stack: &mut Vec<XdmSequence<N>>) -> Result<(XdmSequence<N>, XdmSequence<N>), Error> {
    let r = stack
        .pop()
        .ok_or_else(|| Error::dynamic_err("err:FOER0000", "empty stack"))?;
    let l = stack
        .pop()
        .ok_or_else(|| Error::dynamic_err("err:FOER0000", "empty stack"))?;
    Ok((l, r))
}

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
            XdmItem::Atomic(_) => Err(Error::dynamic_err(
                "err:FORG0006",
                "EBV for this atomic type not supported yet",
            )),
            XdmItem::Node(_) => Ok(true),
        },
        _ => Err(Error::dynamic_err(
            "err:FORG0006",
            "EBV of sequence with more than one item",
        )),
    }
}

fn bin_bool<N, F: FnOnce(bool, bool) -> bool>(
    op: F,
    stack: &mut Vec<XdmSequence<N>>,
) -> Result<(), Error> {
    let (l, r) = take2(stack)?;
    let lb = ebv(&l)?;
    let rb = ebv(&r)?;
    stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(op(lb, rb)))]);
    Ok(())
}

fn the_number<N>(seq: &XdmSequence<N>) -> Result<f64, Error> {
    if seq.len() != 1 {
        return Err(Error::dynamic_err(
            "err:FORG0006",
            "arithmetic expects one item",
        ));
    }
    match &seq[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(i)) => Ok(*i as f64),
        XdmItem::Atomic(XdmAtomicValue::Double(d)) => Ok(*d),
        XdmItem::Atomic(XdmAtomicValue::Float(f)) => Ok(*f as f64),
        XdmItem::Atomic(XdmAtomicValue::Decimal(d)) => Ok(*d),
        XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(s)) => s
            .parse::<f64>()
            .map_err(|_| Error::dynamic_err("err:FORG0001", "cannot cast untypedAtomic to number")),
        _ => Err(Error::dynamic_err(
            "err:XPTY0004",
            "non-numeric in arithmetic",
        )),
    }
}

fn push_number<N>(stack: &mut Vec<XdmSequence<N>>, v: f64) {
    // For simplicity, always push as Double in minimal evaluator
    stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Double(v))]);
}

fn bin_num2<N, F: FnOnce(f64, f64) -> Result<f64, Error>>(
    f: F,
    stack: &mut Vec<XdmSequence<N>>,
) -> Result<(), Error> {
    let (l, r) = take2(stack)?;
    let a = the_number(&l)?;
    let b = the_number(&r)?;
    let v = f(a, b)?;
    push_number(stack, v);
    Ok(())
}

fn num_add(a: f64, b: f64) -> f64 {
    a + b
}
fn num_sub(a: f64, b: f64) -> f64 {
    a - b
}
fn num_mul(a: f64, b: f64) -> f64 {
    a * b
}
fn num_div(a: f64, b: f64) -> Result<f64, Error> {
    if b == 0.0 {
        Err(Error::dynamic_err("err:FOAR0001", "divide by zero"))
    } else {
        Ok(a / b)
    }
}
fn num_idiv(a: f64, b: f64) -> Result<f64, Error> {
    if b == 0.0 {
        Err(Error::dynamic_err("err:FOAR0001", "idiv by zero"))
    } else {
        Ok((a / b).floor())
    }
}
fn num_mod(a: f64, b: f64) -> f64 {
    a % b
}

fn compare_value_with_collation<N: crate::model::XdmNode>(
    l: &XdmSequence<N>,
    r: &XdmSequence<N>,
    op: ComparisonOp,
    coll: Option<&dyn Collation>,
) -> Result<bool, Error> {
    let la = atomize_sequence(l)?;
    let ra = atomize_sequence(r)?;
    if la.len() != 1 || ra.len() != 1 {
        return Err(Error::dynamic_err(
            "err:FORG0006",
            "value comparison expects exactly one atomic value per operand",
        ));
    }
    compare_atomic(&la[0], &ra[0], op, coll)
}

fn compare_general<N: crate::model::XdmNode>(
    l: &XdmSequence<N>,
    r: &XdmSequence<N>,
    op: ComparisonOp,
    coll: Option<&dyn Collation>,
) -> Result<bool, Error> {
    let la = atomize_sequence(l)?;
    let ra = atomize_sequence(r)?;
    for a in &la {
        for b in &ra {
            match compare_atomic(a, b, op, coll) {
                Ok(true) => return Ok(true),
                Ok(false) => continue,
                Err(_) => continue, // ignore type-conversion errors for general comparisons
            }
        }
    }
    Ok(false)
}

fn atomize_sequence<N: crate::model::XdmNode>(
    s: &XdmSequence<N>,
) -> Result<Vec<XdmAtomicValue>, Error> {
    let mut out = Vec::new();
    for it in s {
        match it {
            XdmItem::Atomic(a) => out.push(a.clone()),
            XdmItem::Node(n) => out.push(XdmAtomicValue::UntypedAtomic(n.string_value())),
        }
    }
    Ok(out)
}

fn as_number(a: &XdmAtomicValue) -> Result<f64, Error> {
    match a {
        XdmAtomicValue::Integer(i) => Ok(*i as f64),
        XdmAtomicValue::Double(d) => Ok(*d),
        XdmAtomicValue::Float(f) => Ok(*f as f64),
        XdmAtomicValue::Decimal(d) => Ok(*d),
        XdmAtomicValue::UntypedAtomic(s) => s
            .parse::<f64>()
            .map_err(|_| Error::dynamic_err("err:FORG0001", "cannot cast untypedAtomic to number")),
        XdmAtomicValue::String(s) => s
            .parse::<f64>()
            .map_err(|_| Error::dynamic_err("err:FORG0001", "cannot cast string to number")),
        _ => Err(Error::dynamic_err("err:XPTY0004", "not a numeric value")),
    }
}

fn is_numeric(a: &XdmAtomicValue) -> bool {
    matches!(
        a,
        XdmAtomicValue::Integer(_)
            | XdmAtomicValue::Double(_)
            | XdmAtomicValue::Float(_)
            | XdmAtomicValue::Decimal(_)
    )
}

fn may_be_numeric(a: &XdmAtomicValue) -> bool {
    matches!(a, XdmAtomicValue::UntypedAtomic(_))
}

fn as_integer(a: &XdmAtomicValue) -> Result<i64, Error> {
    match a {
        XdmAtomicValue::Integer(i) => Ok(*i),
        XdmAtomicValue::Double(d) => Ok(d.floor() as i64),
        XdmAtomicValue::Float(f) => Ok(f.floor() as i64),
        XdmAtomicValue::Decimal(d) => Ok(d.floor() as i64),
        XdmAtomicValue::UntypedAtomic(s) => s
            .parse::<f64>()
            .map_err(|_| Error::dynamic_err("err:FORG0001", "cannot cast untypedAtomic to number"))
            .map(|v| v.floor() as i64),
        _ => Err(Error::dynamic_err("err:XPTY0004", "not a numeric value")),
    }
}

fn as_string(a: &XdmAtomicValue) -> String {
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
        XdmAtomicValue::Double(d) => d.to_string(),
        XdmAtomicValue::Float(f) => f.to_string(),
        XdmAtomicValue::Decimal(d) => d.to_string(),
        XdmAtomicValue::QName {
            ns_uri: _,
            prefix,
            local,
        } => {
            if let Some(p) = prefix {
                format!("{}:{}", p, local)
            } else {
                local.clone()
            }
        }
    }
}

fn parse_boolean(s: &str) -> Option<bool> {
    match s {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

fn target_type_local(t: &ExpandedName) -> &str {
    t.local.split(':').next_back().unwrap_or(&t.local)
}

fn is_castable<N: crate::model::XdmNode>(
    s: &XdmSequence<N>,
    t: &SingleTypeIR,
) -> Result<bool, Error> {
    if s.is_empty() {
        return Ok(t.optional);
    }
    if s.len() != 1 {
        return Ok(false);
    }
    let a = atomize_sequence(s)?;
    if a.len() != 1 {
        return Ok(false);
    }
    let v = &a[0];
    let tgt = target_type_local(&t.atomic);
    let ok = match tgt {
        "string" => true,
        "boolean" => match v {
            XdmAtomicValue::Boolean(_) => true,
            XdmAtomicValue::String(s) => parse_boolean(s).is_some(),
            _ => true,
        },
        "integer" => match v {
            XdmAtomicValue::Integer(_) => true,
            XdmAtomicValue::String(s) => s.parse::<f64>().is_ok(),
            _ => is_numeric(v) || may_be_numeric(v),
        },
        "double" | "float" | "decimal" => match v {
            XdmAtomicValue::String(s) => s.parse::<f64>().is_ok(),
            _ => is_numeric(v) || may_be_numeric(v),
        },
        "anyURI" => true,
        _ => false,
    };
    Ok(ok)
}

fn do_cast<N: crate::model::XdmNode>(
    s: &XdmSequence<N>,
    t: &SingleTypeIR,
) -> Result<XdmSequence<N>, Error> {
    if s.is_empty() {
        return if t.optional {
            Ok(vec![])
        } else {
            Err(Error::dynamic_err("err:FORG0006", "cast requires one item"))
        };
    }
    if s.len() != 1 {
        return Err(Error::dynamic_err("err:FORG0006", "cast requires one item"));
    }
    let a = atomize_sequence(s)?;
    if a.len() != 1 {
        return Err(Error::dynamic_err(
            "err:FORG0006",
            "cast requires atomic value",
        ));
    }
    let v = &a[0];
    let tgt = target_type_local(&t.atomic);
    let res = match tgt {
        "string" => XdmAtomicValue::String(as_string(v)),
        "boolean" => match v {
            XdmAtomicValue::Boolean(b) => XdmAtomicValue::Boolean(*b),
            XdmAtomicValue::String(s) => {
                if let Some(b) = parse_boolean(s) {
                    XdmAtomicValue::Boolean(b)
                } else {
                    return Err(Error::dynamic_err(
                        "err:FORG0001",
                        "invalid boolean literal",
                    ));
                }
            }
            _ => XdmAtomicValue::Boolean(match as_number(v) {
                Ok(n) => n != 0.0 && !n.is_nan(),
                Err(_) => return Err(Error::dynamic_err("err:FORG0001", "cannot cast to boolean")),
            }),
        },
        "integer" => {
            let n = match v {
                XdmAtomicValue::String(s) => s
                    .parse::<f64>()
                    .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid integer literal"))?,
                _ => as_number(v)?,
            };
            XdmAtomicValue::Integer(n.trunc() as i64)
        }
        "double" => XdmAtomicValue::Double(match v {
            XdmAtomicValue::String(s) => s
                .parse::<f64>()
                .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid double literal"))?,
            _ => as_number(v)?,
        }),
        "float" => XdmAtomicValue::Float(match v {
            XdmAtomicValue::String(s) => s
                .parse::<f64>()
                .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid float literal"))?,
            _ => as_number(v)?,
        } as f32),
        "decimal" => XdmAtomicValue::Decimal(match v {
            XdmAtomicValue::String(s) => s
                .parse::<f64>()
                .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid decimal literal"))?,
            _ => as_number(v)?,
        }),
        "anyURI" => XdmAtomicValue::AnyUri(as_string(v)),
        _ => {
            return Err(Error::dynamic_err(
                "err:XPST0017",
                "unsupported cast target",
            ));
        }
    };
    Ok(vec![XdmItem::Atomic(res)])
}

fn occurrence_ok(len: usize, occ: &OccurrenceIR) -> bool {
    match occ {
        OccurrenceIR::One => len == 1,
        OccurrenceIR::ZeroOrOne => len <= 1,
        OccurrenceIR::ZeroOrMore => true,
        OccurrenceIR::OneOrMore => len >= 1,
    }
}

fn instance_of<N: crate::model::XdmNode>(s: &XdmSequence<N>, t: &SeqTypeIR) -> bool {
    match t {
        SeqTypeIR::EmptySequence => s.is_empty(),
        SeqTypeIR::Typed { item, occ } => {
            if !occurrence_ok(s.len(), occ) {
                return false;
            }
            match item {
                ItemTypeIR::AnyItem => true,
                ItemTypeIR::Atomic(exp) => {
                    for it in s {
                        match it {
                            XdmItem::Atomic(a) => {
                                let tgt = target_type_local(exp);
                                let ok = match (tgt, a) {
                                    ("string", XdmAtomicValue::String(_)) => true,
                                    ("boolean", XdmAtomicValue::Boolean(_)) => true,
                                    ("integer", XdmAtomicValue::Integer(_)) => true,
                                    ("double", XdmAtomicValue::Double(_)) => true,
                                    ("float", XdmAtomicValue::Float(_)) => true,
                                    ("decimal", XdmAtomicValue::Decimal(_)) => true,
                                    ("anyURI", XdmAtomicValue::AnyUri(_)) => true,
                                    _ => false,
                                };
                                if !ok {
                                    return false;
                                }
                            }
                            _ => return false,
                        }
                    }
                    true
                }
                ItemTypeIR::Kind(nt) => {
                    for it in s {
                        match it {
                            XdmItem::Node(n) => {
                                if !matches_test(n, nt) {
                                    return false;
                                }
                            }
                            _ => return false,
                        }
                    }
                    true
                }
            }
        }
    }
}

fn compare_atomic(
    a: &XdmAtomicValue,
    b: &XdmAtomicValue,
    op: ComparisonOp,
    coll: Option<&dyn Collation>,
) -> Result<bool, Error> {
    use ComparisonOp::*;
    match op {
        Eq | Ne => {
            // Numeric if either is numeric or both can be numeric (untypedAtomic)
            if is_numeric(a) || is_numeric(b) || (may_be_numeric(a) && may_be_numeric(b)) {
                let na = as_number(a)?;
                let nb = as_number(b)?;
                return if matches!(op, Eq) {
                    Ok(na == nb)
                } else {
                    Ok(na != nb)
                };
            }
            // Otherwise string compare (collation-aware)
            let sa = as_string(a);
            let sb = as_string(b);
            if let Some(c) = coll {
                let eq = c.compare(&sa, &sb) == core::cmp::Ordering::Equal;
                if matches!(op, Eq) { Ok(eq) } else { Ok(!eq) }
            } else if matches!(op, Eq) {
                Ok(sa == sb)
            } else {
                Ok(sa != sb)
            }
        }
        Lt | Le | Gt | Ge => {
            // Prefer numeric if both are numeric-like; otherwise string ordering
            if (is_numeric(a) || may_be_numeric(a)) && (is_numeric(b) || may_be_numeric(b)) {
                let na = as_number(a)?;
                let nb = as_number(b)?;
                return Ok(match op {
                    Lt => na < nb,
                    Le => na <= nb,
                    Gt => na > nb,
                    Ge => na >= nb,
                    _ => unreachable!(),
                });
            }
            let sa = as_string(a);
            let sb = as_string(b);
            let ord = if let Some(c) = coll {
                c.compare(&sa, &sb)
            } else {
                sa.cmp(&sb)
            };
            Ok(match op {
                ComparisonOp::Lt => ord == core::cmp::Ordering::Less,
                ComparisonOp::Le => {
                    ord == core::cmp::Ordering::Less || ord == core::cmp::Ordering::Equal
                }
                ComparisonOp::Gt => ord == core::cmp::Ordering::Greater,
                ComparisonOp::Ge => {
                    ord == core::cmp::Ordering::Greater || ord == core::cmp::Ordering::Equal
                }
                _ => unreachable!(),
            })
        }
    }
}

fn node_seq<N>(seq: XdmSequence<N>) -> Result<Vec<N>, Error> {
    let mut out = Vec::new();
    for item in seq {
        match item {
            XdmItem::Node(n) => out.push(n),
            _ => return Err(Error::dynamic_err("err:XPTY0020", "expected node sequence")),
        }
    }
    Ok(out)
}

fn dedup_and_sort<N: crate::model::XdmNode>(nodes: &mut Vec<N>) {
    // dedup preserving first occurrence
    let mut i = 0;
    while i < nodes.len() {
        let mut j = i + 1;
        while j < nodes.len() {
            if nodes[j] == nodes[i] {
                nodes.remove(j);
            } else {
                j += 1;
            }
        }
        i += 1;
    }
    nodes.sort_by(|a, b| a.compare_document_order(b));
}

fn single_node<N>(seq: XdmSequence<N>) -> Result<N, Error> {
    let v = node_seq(seq)?;
    if v.len() != 1 {
        return Err(Error::dynamic_err(
            "err:XPTY0004",
            "node comparison expects single node on each side",
        ));
    }
    Ok(v.into_iter().next().unwrap())
}

fn root_of<N: crate::model::XdmNode>(mut n: N) -> N {
    while let Some(p) = n.parent() {
        n = p;
    }
    n
}

fn apply_axis<N: crate::model::XdmNode>(n: &N, axis: &AxisIR) -> Vec<N> {
    match axis {
        AxisIR::SelfAxis => vec![n.clone()],
        AxisIR::Child => n.children(),
        AxisIR::Attribute => n.attributes(),
        AxisIR::Descendant => {
            fn dfs<N: crate::model::XdmNode>(n: &N, out: &mut Vec<N>) {
                for c in n.children() {
                    out.push(c.clone());
                    dfs(&c, out);
                }
            }
            let mut acc = Vec::new();
            dfs(n, &mut acc);
            acc
        }
        AxisIR::DescendantOrSelf => {
            let mut acc = vec![n.clone()];
            fn dfs<N: crate::model::XdmNode>(n: &N, out: &mut Vec<N>) {
                for c in n.children() {
                    out.push(c.clone());
                    dfs(&c, out);
                }
            }
            let mut more = Vec::new();
            dfs(n, &mut more);
            acc.extend(more);
            acc
        }
        AxisIR::Parent => n.parent().into_iter().collect(),
        AxisIR::Ancestor => {
            let mut v = Vec::new();
            let mut cur = n.parent();
            while let Some(p) = cur {
                v.push(p.clone());
                cur = p.parent();
            }
            v
        }
        AxisIR::AncestorOrSelf => {
            let mut v = vec![n.clone()];
            let mut cur = n.parent();
            while let Some(p) = cur {
                v.push(p.clone());
                cur = p.parent();
            }
            v
        }
        AxisIR::PrecedingSibling => {
            if let Some(parent) = n.parent() {
                let mut res = Vec::new();
                for c in parent.children() {
                    if c.compare_document_order(n) == core::cmp::Ordering::Less {
                        res.push(c);
                    }
                }
                return res;
            }
            Vec::new()
        }
        AxisIR::FollowingSibling => {
            if let Some(parent) = n.parent() {
                let mut res = Vec::new();
                for c in parent.children() {
                    if c.compare_document_order(n) == core::cmp::Ordering::Greater {
                        res.push(c);
                    }
                }
                return res;
            }
            Vec::new()
        }
        AxisIR::Preceding => {
            let root = root_of(n.clone());
            let all = apply_axis(&root, &AxisIR::DescendantOrSelf);
            let mut res = Vec::new();
            for m in all {
                if m.compare_document_order(n) == core::cmp::Ordering::Less
                    && !is_ancestor_of(&m, n)
                    && m.kind() != crate::model::NodeKind::Attribute
                    && m.kind() != crate::model::NodeKind::Namespace
                {
                    res.push(m);
                }
            }
            res
        }
        AxisIR::Following => {
            let root = root_of(n.clone());
            let all = apply_axis(&root, &AxisIR::DescendantOrSelf);
            let mut res = Vec::new();
            for m in all {
                if m.compare_document_order(n) == core::cmp::Ordering::Greater
                    && !is_descendant_of(&m, n)
                    && m.kind() != crate::model::NodeKind::Attribute
                    && m.kind() != crate::model::NodeKind::Namespace
                {
                    res.push(m);
                }
            }
            res
        }
        AxisIR::Namespace => {
            // Only element nodes have namespaces; otherwise empty
            if n.kind() == crate::model::NodeKind::Element {
                n.namespaces()
            } else {
                Vec::new()
            }
        }
    }
}

fn is_ancestor_of<N: crate::model::XdmNode>(a: &N, n: &N) -> bool {
    let mut cur = n.parent();
    while let Some(p) = cur {
        if &p == a {
            return true;
        }
        cur = p.parent();
    }
    false
}

fn is_descendant_of<N: crate::model::XdmNode>(m: &N, n: &N) -> bool {
    is_ancestor_of(n, m)
}

fn matches_test<N: crate::model::XdmNode>(n: &N, test: &NodeTestIR) -> bool {
    use crate::model::NodeKind as NK;
    match test {
        NodeTestIR::AnyKind => true,
        NodeTestIR::KindText => n.kind() == NK::Text,
        NodeTestIR::KindComment => n.kind() == NK::Comment,
        NodeTestIR::KindProcessingInstruction(target) => {
            if n.kind() != NK::ProcessingInstruction {
                return false;
            }
            // If a target is specified, match by string value of target name
            if let Some(t) = target {
                n.name().map(|q| q.local == *t).unwrap_or(false)
            } else {
                true
            }
        }
        NodeTestIR::KindDocument => n.kind() == NK::Document,
        NodeTestIR::KindElement => n.kind() == NK::Element,
        NodeTestIR::KindAttribute => n.kind() == NK::Attribute,
        NodeTestIR::WildcardAny => n.name().is_some(),
        NodeTestIR::NsWildcard(ns_uri) => {
            if let Some(q) = n.name() {
                q.ns_uri.as_deref() == Some(ns_uri.as_str())
            } else {
                false
            }
        }
        NodeTestIR::LocalWildcard(local) => {
            if let Some(q) = n.name() {
                q.local == *local
            } else {
                false
            }
        }
        NodeTestIR::Name(exp) => {
            if let Some(q) = n.name() {
                // If ns_uri is None (unresolved), match on local only for now.
                if let Some(ns) = &exp.ns_uri {
                    q.ns_uri.as_deref() == Some(ns.as_str()) && q.local == exp.local
                } else {
                    q.local == exp.local
                }
            } else {
                false
            }
        }
    }
}

fn predicate_truthy<N>(seq: &XdmSequence<N>, pos: i64) -> Result<bool, Error> {
    if seq.len() == 1
        && let XdmItem::Atomic(a) = &seq[0]
        && is_numeric(a)
    {
        let v = as_number(a)?;
        return Ok((v as i64) == pos);
    }
    ebv(seq)
}

pub fn compile_xpath(expr: &str, static_ctx: &StaticContext) -> Result<XPathExecutable, Error> {
    let ir = compile_to_ir(expr, static_ctx)?;
    Ok(XPathExecutable(ir))
}

// Re-exports for API are done from lib.rs

fn resolve_default_collation<N>(
    exec: &XPathExecutable,
    dyn_ctx: &DynamicContext<N>,
) -> Option<std::sync::Arc<dyn Collation>> {
    if let Some(uri) = &dyn_ctx.default_collation
        && let Some(c) = dyn_ctx.collations.get(uri)
    {
        return Some(c);
    }
    if let Some(uri) = &exec.0.static_ctx.default_collation
        && let Some(c) = dyn_ctx.collations.get(uri)
    {
        return Some(c);
    }
    dyn_ctx
        .collations
        .get("http://www.w3.org/2005/xpath-functions/collation/codepoint")
}
