use crate::compiler::{
    AxisIR, ComparisonOp, CompiledIR, ItemTypeIR, NodeTestIR, OccurrenceIR, OpCode, SeqTypeIR,
    SingleTypeIR, compile_xpath as compile_to_ir,
};
use crate::runtime::{
    CallCtx, Collation, DynamicContext, DynamicContextBuilder as Builder, Error, StaticContext,
};
use crate::xdm::ExpandedName;
use crate::xdm::{XdmAtomicValue, XdmItem, XdmSequence};
use chrono::{
    DateTime as ChronoDateTime, Datelike, FixedOffset as ChronoFixedOffset, NaiveDate, NaiveTime,
    Timelike,
};

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
                OpCode::Add => bin_arith2(ArithOp::Add, &mut stack)?,
                OpCode::Sub => bin_arith2(ArithOp::Sub, &mut stack)?,
                OpCode::Mul => bin_arith2(ArithOp::Mul, &mut stack)?,
                OpCode::Div => bin_arith2(ArithOp::Div, &mut stack)?,
                OpCode::IDiv => bin_arith2(ArithOp::IDiv, &mut stack)?,
                OpCode::Mod => bin_arith2(ArithOp::Mod, &mut stack)?,
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
                        let step_nodes = apply_axis(&n, axis)?;
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
                    // sort in document order (reject multi-root under default fallback)
                    sort_doc_order(&mut unique)?;
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
                                now: dyn_ctx.now,
                                timezone_override: dyn_ctx.timezone_override,
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
                        // Build CallCtx (M6): pass dyn/static ctx, resolved default collation, resolver, regex
                        let ctx = CallCtx {
                            dyn_ctx,
                            static_ctx: &self.0.static_ctx,
                            default_collation: resolve_default_collation(self, dyn_ctx),
                            resolver: dyn_ctx.resolver.clone(),
                            regex: dyn_ctx.regex.clone(),
                        };
                        let res = (fun)(&ctx, &args)?;
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
                    dedup_and_sort(&mut nodes)?;
                    stack.push(nodes.into_iter().map(XdmItem::Node).collect());
                }
                OpCode::Intersect => {
                    let (l, r) = take2(&mut stack)?;
                    let mut ln = node_seq(l)?;
                    let rn = node_seq(r)?;
                    ln.retain(|n| rn.iter().any(|m| m == n));
                    dedup_and_sort(&mut ln)?;
                    stack.push(ln.into_iter().map(XdmItem::Node).collect());
                }
                OpCode::Except => {
                    let (l, r) = take2(&mut stack)?;
                    let mut ln = node_seq(l)?;
                    let rn = node_seq(r)?;
                    ln.retain(|n| !rn.iter().any(|m| m == n));
                    dedup_and_sort(&mut ln)?;
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
                    // Protect against multi-root comparisons under fallback
                    if root_of(ln.clone()) != root_of(rn.clone()) {
                        return Err(Error::dynamic_err(
                            "err:FOER0000",
                            "node comparison across different roots requires adapter-provided document order",
                        ));
                    }
                    let b = ln.compare_document_order(&rn)? == core::cmp::Ordering::Less;
                    stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                }
                OpCode::NodeAfter => {
                    let (l, r) = take2(&mut stack)?;
                    let ln = single_node(l)?;
                    let rn = single_node(r)?;
                    if root_of(ln.clone()) != root_of(rn.clone()) {
                        return Err(Error::dynamic_err(
                            "err:FOER0000",
                            "node comparison across different roots requires adapter-provided document order",
                        ));
                    }
                    let b = ln.compare_document_order(&rn)? == core::cmp::Ordering::Greater;
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

// removed dead numeric helpers (replaced by bin_arith2)

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

#[derive(Copy, Clone)]
enum ArithOp {
    Add,
    Sub,
    Mul,
    Div,
    IDiv,
    Mod,
}

fn bin_arith2<N: crate::model::XdmNode>(
    op: ArithOp,
    stack: &mut Vec<XdmSequence<N>>,
) -> Result<(), Error> {
    let (l, r) = take2(stack)?;
    // Atomize both sides
    let la = atomize_sequence(&l)?;
    let ra = atomize_sequence(&r)?;
    if la.len() != 1 || ra.len() != 1 {
        return Err(Error::dynamic_err(
            "err:FORG0006",
            "arithmetic expects exactly one atomic value per operand",
        ));
    }
    let a = &la[0];
    let b = &ra[0];
    let res = match op {
        ArithOp::Add => add_atomic(a, b)?,
        ArithOp::Sub => sub_atomic(a, b)?,
        ArithOp::Mul => mul_atomic(a, b)?,
        ArithOp::Div => div_atomic(a, b)?,
        ArithOp::IDiv => idiv_atomic(a, b)?,
        ArithOp::Mod => mod_atomic(a, b)?,
    };
    stack.push(vec![XdmItem::Atomic(res)]);
    Ok(())
}

fn add_atomic(a: &XdmAtomicValue, b: &XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    match (a, b) {
        (V::Double(na), V::Double(nb)) => Ok(V::Double(num_add(*na, *nb))),
        _ => add_sub_temporal(a, b, true),
    }
}

fn sub_atomic(a: &XdmAtomicValue, b: &XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    match (a, b) {
        (V::Double(na), V::Double(nb)) => Ok(V::Double(num_sub(*na, *nb))),
        _ => add_sub_temporal(a, b, false),
    }
}

fn mul_atomic(a: &XdmAtomicValue, b: &XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    // Duration * number or number * duration
    let num = |x: &XdmAtomicValue| as_number(x);
    let aa = coerce_temporal(a).unwrap_or_else(|| a.clone());
    let bb = coerce_temporal(b).unwrap_or_else(|| b.clone());
    match (&aa, &bb) {
        (V::DayTimeDuration(s), other) | (other, V::DayTimeDuration(s)) => {
            let n = num(other)?;
            Ok(V::DayTimeDuration((*s as f64 * n).round() as i64))
        }
        (V::YearMonthDuration(m), other) | (other, V::YearMonthDuration(m)) => {
            let n = num(other)?;
            Ok(V::YearMonthDuration((*m as f64 * n).round() as i32))
        }
        _ => Ok(V::Double(num_mul(as_number(a)?, as_number(b)?))),
    }
}

fn div_atomic(a: &XdmAtomicValue, b: &XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    // Hard early match for YearMonthDuration ÷ YearMonthDuration
    if let (V::YearMonthDuration(m1), V::YearMonthDuration(m2)) = (a, b) {
        if *m2 == 0 {
            return Err(Error::dynamic_err("err:FOAR0001", "divide by zero"));
        }
        return Ok(V::Double(*m1 as f64 / *m2 as f64));
    }
    let aa = coerce_temporal(a).unwrap_or_else(|| a.clone());
    let bb = coerce_temporal(b).unwrap_or_else(|| b.clone());
    match (&aa, &bb) {
        (V::DayTimeDuration(s1), V::DayTimeDuration(s2)) => {
            if *s2 == 0 {
                return Err(Error::dynamic_err("err:FOAR0001", "divide by zero"));
            }
            Ok(V::Double(*s1 as f64 / *s2 as f64))
        }
        (V::YearMonthDuration(m1), V::YearMonthDuration(m2)) => {
            if *m2 == 0 {
                return Err(Error::dynamic_err("err:FOAR0001", "divide by zero"));
            }
            Ok(V::Double(*m1 as f64 / *m2 as f64))
        }
        (V::DayTimeDuration(s), other) => {
            let n = as_number(other)?;
            if n == 0.0 {
                return Err(Error::dynamic_err("err:FOAR0001", "divide by zero"));
            }
            Ok(V::DayTimeDuration((*s as f64 / n).round() as i64))
        }
        (V::YearMonthDuration(m), other) => {
            let n = as_number(other)?;
            if n == 0.0 {
                return Err(Error::dynamic_err("err:FOAR0001", "divide by zero"));
            }
            Ok(V::YearMonthDuration((*m as f64 / n).round() as i32))
        }
        _ => Ok(V::Double(num_div(as_number(a)?, as_number(b)?)?)),
    }
}

fn idiv_atomic(a: &XdmAtomicValue, b: &XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    Ok(XdmAtomicValue::Double(num_idiv(
        as_number(a)?,
        as_number(b)?,
    )?))
}

fn mod_atomic(a: &XdmAtomicValue, b: &XdmAtomicValue) -> Result<XdmAtomicValue, Error> {
    Ok(XdmAtomicValue::Double(num_mod(
        as_number(a)?,
        as_number(b)?,
    )))
}

fn add_sub_temporal(
    a: &XdmAtomicValue,
    b: &XdmAtomicValue,
    is_add: bool,
) -> Result<XdmAtomicValue, Error> {
    use XdmAtomicValue as V;
    let sgn = if is_add { 1i32 } else { -1i32 };
    let sgn64 = sgn as i64;
    let sgn32 = sgn;
    // Early matches on original operands to avoid any coercion ambiguity
    if let (V::Date { date, tz }, V::YearMonthDuration(m)) = (a, b) {
        let nd = add_months_date(*date, sgn32 * *m);
        return Ok(V::Date { date: nd, tz: *tz });
    }
    if let (V::YearMonthDuration(m), V::Date { date, tz }) = (a, b) {
        let nd = add_months_date(*date, sgn32 * *m);
        return Ok(V::Date { date: nd, tz: *tz });
    }
    if let (V::DateTime(dt), V::YearMonthDuration(m)) = (a, b) {
        return Ok(V::DateTime(add_months_datetime(*dt, sgn32 * *m)));
    }
    if let (V::YearMonthDuration(m), V::DateTime(dt)) = (a, b) {
        return Ok(V::DateTime(add_months_datetime(*dt, sgn32 * *m)));
    }
    // Coerce string/untypedAtomic to temporal/duration if possible
    let aa = coerce_temporal(a).unwrap_or_else(|| a.clone());
    let bb = coerce_temporal(b).unwrap_or_else(|| b.clone());
    match (&aa, &bb) {
        // Prefer yearMonthDuration handling first (no ambiguity with zero-seconds dayTimeDuration)
        (V::DateTime(dt), V::YearMonthDuration(m)) | (V::YearMonthDuration(m), V::DateTime(dt)) => {
            Ok(V::DateTime(add_months_datetime(*dt, sgn32 * *m)))
        }
        (V::Date { date, tz }, V::YearMonthDuration(m))
        | (V::YearMonthDuration(m), V::Date { date, tz }) => {
            let nd = add_months_date(*date, sgn32 * *m);
            Ok(V::Date { date: nd, tz: *tz })
        }
        // dayTimeDuration handling
        (V::DateTime(dt), V::DayTimeDuration(secs))
        | (V::DayTimeDuration(secs), V::DateTime(dt)) => {
            Ok(V::DateTime(*dt + chrono::TimeDelta::seconds(sgn64 * *secs)))
        }
        (V::Date { date, tz }, V::DayTimeDuration(secs))
        | (V::DayTimeDuration(secs), V::Date { date, tz }) => {
            let days = (sgn64 * *secs).div_euclid(86_400);
            let nd = *date + chrono::Days::new(days as u64);
            Ok(V::Date { date: nd, tz: *tz })
        }
        // time ± dayTimeDuration (wrap 24h)
        (V::Time { time, tz }, V::DayTimeDuration(secs))
        | (V::DayTimeDuration(secs), V::Time { time, tz }) => Ok(V::Time {
            time: add_seconds_time(*time, sgn64 * *secs),
            tz: *tz,
        }),
        // duration ± duration
        (V::DayTimeDuration(s1), V::DayTimeDuration(s2)) => {
            Ok(V::DayTimeDuration(*s1 + sgn64 * *s2))
        }
        (V::YearMonthDuration(m1), V::YearMonthDuration(m2)) => {
            Ok(V::YearMonthDuration(*m1 + sgn32 * *m2))
        }
        // dateTime - dateTime => dayTimeDuration
        (V::DateTime(a), V::DateTime(b)) if !is_add => {
            let secs = (*a - *b).num_seconds();
            Ok(V::DayTimeDuration(secs))
        }
        // date - date => dayTimeDuration (days)
        (V::Date { date: d1, .. }, V::Date { date: d2, .. }) if !is_add => {
            let days = (*d1 - *d2).num_days();
            Ok(V::DayTimeDuration(days * 86_400))
        }
        // time - time => dayTimeDuration (seconds)
        (V::Time { time: t1, tz: tz1 }, V::Time { time: t2, tz: tz2 }) if !is_add => {
            let off1 = tz1.map(|o| o.local_minus_utc()).unwrap_or(0);
            let off2 = tz2.map(|o| o.local_minus_utc()).unwrap_or(0);
            let s1 = t1.num_seconds_from_midnight() as i64 - off1 as i64;
            let s2 = t2.num_seconds_from_midnight() as i64 - off2 as i64;
            Ok(V::DayTimeDuration(s1 - s2))
        }
        _ => Ok(V::Double(match is_add {
            true => num_add(as_number(a)?, as_number(b)?),
            false => num_sub(as_number(a)?, as_number(b)?),
        })),
    }
}

fn add_months_datetime(
    dt: ChronoDateTime<ChronoFixedOffset>,
    months: i32,
) -> ChronoDateTime<ChronoFixedOffset> {
    let y = dt.year();
    let m = dt.month() as i32;
    let total = y * 12 + (m - 1) + months;
    let ny = total.div_euclid(12);
    let nm = total.rem_euclid(12) + 1; // 1..=12
    let day = dt.day().min(days_in_month(ny, nm as u32));
    let date = NaiveDate::from_ymd_opt(ny, nm as u32, day).unwrap();
    let time = dt.time();
    let ndt = date.and_time(time);
    ndt.and_local_timezone(*dt.offset()).unwrap()
}

fn add_months_date(date: NaiveDate, months: i32) -> NaiveDate {
    let y = date.year();
    let m = date.month() as i32;
    let total = y * 12 + (m - 1) + months;
    let ny = total.div_euclid(12);
    let nm = total.rem_euclid(12) + 1;
    let day = date.day().min(days_in_month(ny, nm as u32));
    NaiveDate::from_ymd_opt(ny, nm as u32, day).unwrap()
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let is_leap = |y: i32| (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn add_seconds_time(time: NaiveTime, secs: i64) -> NaiveTime {
    let base = time.num_seconds_from_midnight() as i64;
    let mut total = base + secs;
    let day = 86_400i64;
    total = ((total % day) + day) % day;
    NaiveTime::from_num_seconds_from_midnight_opt(total as u32, 0).unwrap()
}

fn coerce_temporal(v: &XdmAtomicValue) -> Option<XdmAtomicValue> {
    match v {
        XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => {
            if let Ok(dt) = ChronoDateTime::parse_from_rfc3339(s) {
                return Some(XdmAtomicValue::DateTime(dt));
            }
            if let Ok((d, tz)) = parse_xs_date(s) {
                return Some(XdmAtomicValue::Date { date: d, tz });
            }
            if let Ok((t, tz)) = parse_xs_time(s) {
                return Some(XdmAtomicValue::Time { time: t, tz });
            }
            // Prefer yearMonthDuration if no 'T' present (P..M months vs PT..M minutes)
            if s.contains('T') {
                if let Ok(sec) = parse_day_time_duration(s) {
                    return Some(XdmAtomicValue::DayTimeDuration(sec));
                }
                if let Ok(m) = parse_year_month_duration(s) {
                    return Some(XdmAtomicValue::YearMonthDuration(m));
                }
            } else {
                if let Ok(m) = parse_year_month_duration(s) {
                    return Some(XdmAtomicValue::YearMonthDuration(m));
                }
                if let Ok(sec) = parse_day_time_duration(s) {
                    return Some(XdmAtomicValue::DayTimeDuration(sec));
                }
            }
            None
        }
        _ => None,
    }
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
        XdmAtomicValue::DateTime(dt) => dt.format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
        XdmAtomicValue::Date { date, tz } => {
            if let Some(off) = tz {
                format!("{}{}", date.format("%Y-%m-%d"), fmt_offset(off))
            } else {
                date.format("%Y-%m-%d").to_string()
            }
        }
        XdmAtomicValue::Time { time, tz } => {
            if let Some(off) = tz {
                format!("{}{}", time.format("%H:%M:%S"), fmt_offset(off))
            } else {
                time.format("%H:%M:%S").to_string()
            }
        }
        XdmAtomicValue::YearMonthDuration(months) => format_year_month_duration(*months),
        XdmAtomicValue::DayTimeDuration(secs) => format_day_time_duration(*secs),
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

// ===== M8b: Minimal parse/format helpers for date/time/duration =====
fn parse_offset(tz: &str) -> Option<ChronoFixedOffset> {
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

fn parse_xs_date(s: &str) -> Result<(NaiveDate, Option<ChronoFixedOffset>), ()> {
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

fn parse_xs_time(s: &str) -> Result<(NaiveTime, Option<ChronoFixedOffset>), ()> {
    if let Some(pos) = s.rfind(['+', '-'])
        && pos >= 5
    {
        let (t, tzs) = s.split_at(pos);
        let time = NaiveTime::parse_from_str(t, "%H:%M:%S")
            .or_else(|_| NaiveTime::parse_from_str(t, "%H:%M:%S%.f"))
            .map_err(|_| ())?;
        let off = parse_offset(tzs).ok_or(())?;
        return Ok((time, Some(off)));
    }
    let time = NaiveTime::parse_from_str(s, "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(s, "%H:%M:%S%.f"))
        .map_err(|_| ())?;
    Ok((time, None))
}

fn parse_year_month_duration(s: &str) -> Result<i32, ()> {
    let neg = s.starts_with('-');
    let body = if neg { &s[1..] } else { s };
    if !body.starts_with('P') {
        return Err(());
    }
    let mut rest = &body[1..];
    let mut months: i32 = 0;
    while !rest.is_empty() {
        if let Some(ypos) = rest.find('Y') {
            let v: i32 = rest[..ypos].parse().map_err(|_| ())?;
            months += v * 12;
            rest = &rest[ypos + 1..];
            continue;
        }
        if let Some(mpos) = rest.find('M') {
            let v: i32 = rest[..mpos].parse().map_err(|_| ())?;
            months += v;
            rest = &rest[mpos + 1..];
            continue;
        }
        break;
    }
    if neg {
        months = -months;
    }
    Ok(months)
}

fn parse_day_time_duration(s: &str) -> Result<i64, ()> {
    let neg = s.starts_with('-');
    let body = if neg { &s[1..] } else { s };
    if !body.starts_with('P') {
        return Err(());
    }
    let mut secs: i64 = 0;
    let rest = &body[1..];
    let mut any = false;
    if let Some(tpos) = rest.find('T') {
        let (datep, timep) = rest.split_at(tpos);
        let s1 = parse_day_part_to_secs(datep)?;
        if s1 != 0 {
            any = true;
        }
        let s2 = parse_time_part_to_secs(&timep[1..])?;
        if s2 != 0 {
            any = true;
        }
        secs += s1 + s2;
    } else {
        let s1 = parse_day_part_to_secs(rest)?;
        if s1 != 0 {
            any = true;
        }
        secs += s1;
    }
    if !any {
        return Err(());
    }
    if neg {
        secs = -secs;
    }
    Ok(secs)
}

fn parse_day_part_to_secs(s: &str) -> Result<i64, ()> {
    let mut secs = 0i64;
    if let Some(dpos) = s.find('D') {
        let v: i64 = s[..dpos].parse().map_err(|_| ())?;
        secs += v * 24 * 3600;
    }
    Ok(secs)
}

fn parse_time_part_to_secs(mut s: &str) -> Result<i64, ()> {
    let mut secs = 0i64;
    if let Some(hpos) = s.find('H') {
        let v: i64 = s[..hpos].parse().map_err(|_| ())?;
        secs += v * 3600;
        s = &s[hpos + 1..];
    }
    if let Some(mpos) = s.find('M') {
        let v: i64 = s[..mpos].parse().map_err(|_| ())?;
        secs += v * 60;
        s = &s[mpos + 1..];
    }
    if let Some(spos) = s.find('S') {
        let v: i64 = s[..spos].parse().map_err(|_| ())?;
        secs += v;
    }
    Ok(secs)
}

fn format_year_month_duration(months: i32) -> String {
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

fn format_day_time_duration(total_secs: i64) -> String {
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

fn fmt_offset(off: &ChronoFixedOffset) -> String {
    let secs = off.local_minus_utc();
    let sign = if secs < 0 { '-' } else { '+' };
    let mut s = secs.abs();
    let hours = s / 3600;
    s %= 3600;
    let mins = s / 60;
    format!("{}{:02}:{:02}", sign, hours, mins)
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
        "dateTime" => match v {
            XdmAtomicValue::DateTime(_) => true,
            XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => {
                ChronoDateTime::parse_from_rfc3339(s).is_ok()
            }
            _ => false,
        },
        "date" => match v {
            XdmAtomicValue::Date { .. } => true,
            XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => {
                parse_xs_date(s).is_ok()
            }
            _ => false,
        },
        "time" => match v {
            XdmAtomicValue::Time { .. } => true,
            XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => {
                parse_xs_time(s).is_ok()
            }
            _ => false,
        },
        "dayTimeDuration" => match v {
            XdmAtomicValue::DayTimeDuration(_) => true,
            XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => {
                parse_day_time_duration(s).is_ok()
            }
            _ => false,
        },
        "yearMonthDuration" => match v {
            XdmAtomicValue::YearMonthDuration(_) => true,
            XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => {
                parse_year_month_duration(s).is_ok()
            }
            _ => false,
        },
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
        "dateTime" => match v {
            XdmAtomicValue::DateTime(dt) => XdmAtomicValue::DateTime(*dt),
            XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => {
                ChronoDateTime::parse_from_rfc3339(s)
                    .map(XdmAtomicValue::DateTime)
                    .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid xs:dateTime"))?
            }
            _ => {
                return Err(Error::dynamic_err(
                    "err:XPTY0004",
                    "cannot cast to xs:dateTime",
                ));
            }
        },
        "date" => match v {
            XdmAtomicValue::Date { date, tz } => XdmAtomicValue::Date {
                date: *date,
                tz: *tz,
            },
            XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => parse_xs_date(s)
                .map(|(d, tz)| XdmAtomicValue::Date { date: d, tz })
                .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid xs:date"))?,
            _ => return Err(Error::dynamic_err("err:XPTY0004", "cannot cast to xs:date")),
        },
        "time" => match v {
            XdmAtomicValue::Time { time, tz } => XdmAtomicValue::Time {
                time: *time,
                tz: *tz,
            },
            XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => parse_xs_time(s)
                .map(|(t, tz)| XdmAtomicValue::Time { time: t, tz })
                .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid xs:time"))?,
            _ => return Err(Error::dynamic_err("err:XPTY0004", "cannot cast to xs:time")),
        },
        "dayTimeDuration" => match v {
            XdmAtomicValue::DayTimeDuration(secs) => XdmAtomicValue::DayTimeDuration(*secs),
            XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => {
                parse_day_time_duration(s)
                    .map(XdmAtomicValue::DayTimeDuration)
                    .map_err(|_| Error::dynamic_err("err:FORG0001", "invalid xs:dayTimeDuration"))?
            }
            _ => {
                return Err(Error::dynamic_err(
                    "err:XPTY0004",
                    "cannot cast to xs:dayTimeDuration",
                ));
            }
        },
        "yearMonthDuration" => match v {
            XdmAtomicValue::YearMonthDuration(m) => XdmAtomicValue::YearMonthDuration(*m),
            XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => {
                parse_year_month_duration(s)
                    .map(XdmAtomicValue::YearMonthDuration)
                    .map_err(|_| {
                        Error::dynamic_err("err:FORG0001", "invalid xs:yearMonthDuration")
                    })?
            }
            _ => {
                return Err(Error::dynamic_err(
                    "err:XPTY0004",
                    "cannot cast to xs:yearMonthDuration",
                ));
            }
        },
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
                                    ("dateTime", XdmAtomicValue::DateTime(_)) => true,
                                    ("date", XdmAtomicValue::Date { .. }) => true,
                                    ("time", XdmAtomicValue::Time { .. }) => true,
                                    ("dayTimeDuration", XdmAtomicValue::DayTimeDuration(_)) => true,
                                    ("yearMonthDuration", XdmAtomicValue::YearMonthDuration(_)) => {
                                        true
                                    }
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
            // Duration/Temporal equality
            if let (XdmAtomicValue::DateTime(da), XdmAtomicValue::DateTime(db)) = (a, b) {
                let eq = (da.timestamp(), da.timestamp_subsec_nanos())
                    == (db.timestamp(), db.timestamp_subsec_nanos());
                return if matches!(op, Eq) { Ok(eq) } else { Ok(!eq) };
            }
            if let (
                XdmAtomicValue::Date { date: da, tz: ta },
                XdmAtomicValue::Date { date: db, tz: tb },
            ) = (a, b)
            {
                // Treat tz None as UTC
                let offa = ta.unwrap_or(chrono::FixedOffset::east_opt(0).unwrap());
                let offb = tb.unwrap_or(chrono::FixedOffset::east_opt(0).unwrap());
                let dta = da
                    .and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    .and_local_timezone(offa)
                    .unwrap();
                let dtb = db
                    .and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    .and_local_timezone(offb)
                    .unwrap();
                let eq = dta == dtb;
                return if matches!(op, Eq) { Ok(eq) } else { Ok(!eq) };
            }
            if let (
                XdmAtomicValue::Time { time: ta, tz: o1 },
                XdmAtomicValue::Time { time: tb, tz: o2 },
            ) = (a, b)
            {
                let off1 = o1.map(|o| o.local_minus_utc()).unwrap_or(0);
                let off2 = o2.map(|o| o.local_minus_utc()).unwrap_or(0);
                let s1 = ta.num_seconds_from_midnight() as i64 - off1 as i64;
                let s2 = tb.num_seconds_from_midnight() as i64 - off2 as i64;
                let eq = s1 == s2;
                return if matches!(op, Eq) { Ok(eq) } else { Ok(!eq) };
            }
            if let (XdmAtomicValue::DayTimeDuration(s1), XdmAtomicValue::DayTimeDuration(s2)) =
                (a, b)
            {
                let eq = s1 == s2;
                return if matches!(op, Eq) { Ok(eq) } else { Ok(!eq) };
            }
            if let (XdmAtomicValue::YearMonthDuration(m1), XdmAtomicValue::YearMonthDuration(m2)) =
                (a, b)
            {
                let eq = m1 == m2;
                return if matches!(op, Eq) { Ok(eq) } else { Ok(!eq) };
            }
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
            // Temporal ordering
            if let (XdmAtomicValue::DateTime(da), XdmAtomicValue::DateTime(db)) = (a, b) {
                use core::cmp::Ordering;
                let key_a = (da.timestamp(), da.timestamp_subsec_nanos());
                let key_b = (db.timestamp(), db.timestamp_subsec_nanos());
                let ord = key_a.cmp(&key_b);
                return Ok(match op {
                    Lt => ord == Ordering::Less,
                    Le => ord != Ordering::Greater,
                    Gt => ord == Ordering::Greater,
                    Ge => ord != Ordering::Less,
                    _ => false,
                });
            }
            if let (
                XdmAtomicValue::Date { date: da, tz: ta },
                XdmAtomicValue::Date { date: db, tz: tb },
            ) = (a, b)
            {
                let offa = ta.unwrap_or(chrono::FixedOffset::east_opt(0).unwrap());
                let offb = tb.unwrap_or(chrono::FixedOffset::east_opt(0).unwrap());
                let dta = da
                    .and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    .and_local_timezone(offa)
                    .unwrap();
                let dtb = db
                    .and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
                    .and_local_timezone(offb)
                    .unwrap();
                let ord = dta.cmp(&dtb);
                return Ok(match op {
                    Lt => ord.is_lt(),
                    Le => ord.is_le(),
                    Gt => ord.is_gt(),
                    Ge => ord.is_ge(),
                    _ => false,
                });
            }
            if let (
                XdmAtomicValue::Time { time: ta, tz: o1 },
                XdmAtomicValue::Time { time: tb, tz: o2 },
            ) = (a, b)
            {
                let off1 = o1.map(|o| o.local_minus_utc()).unwrap_or(0);
                let off2 = o2.map(|o| o.local_minus_utc()).unwrap_or(0);
                let s1 = ta.num_seconds_from_midnight() as i64 - off1 as i64;
                let s2 = tb.num_seconds_from_midnight() as i64 - off2 as i64;
                return Ok(match op {
                    Lt => s1 < s2,
                    Le => s1 <= s2,
                    Gt => s1 > s2,
                    Ge => s1 >= s2,
                    _ => false,
                });
            }
            if let (XdmAtomicValue::DayTimeDuration(s1), XdmAtomicValue::DayTimeDuration(s2)) =
                (a, b)
            {
                return Ok(match op {
                    Lt => s1 < s2,
                    Le => s1 <= s2,
                    Gt => s1 > s2,
                    Ge => s1 >= s2,
                    _ => false,
                });
            }
            if let (XdmAtomicValue::YearMonthDuration(m1), XdmAtomicValue::YearMonthDuration(m2)) =
                (a, b)
            {
                return Ok(match op {
                    Lt => m1 < m2,
                    Le => m1 <= m2,
                    Gt => m1 > m2,
                    Ge => m1 >= m2,
                    _ => false,
                });
            }
            // Prefer numeric if both are numeric-like; otherwise string ordering
            if (is_numeric(a) || may_be_numeric(a)) && (is_numeric(b) || may_be_numeric(b)) {
                let na = as_number(a)?;
                let nb = as_number(b)?;
                let res = match op {
                    Lt => na < nb,
                    Le => na <= nb,
                    Gt => na > nb,
                    Ge => na >= nb,
                    _ => {
                        return Err(Error::dynamic_err(
                            "err:FOER0000",
                            "unexpected comparison operator (numeric)",
                        ));
                    }
                };
                return Ok(res);
            }
            let sa = as_string(a);
            let sb = as_string(b);
            let ord = if let Some(c) = coll {
                c.compare(&sa, &sb)
            } else {
                sa.cmp(&sb)
            };
            let res = match op {
                ComparisonOp::Lt => ord == core::cmp::Ordering::Less,
                ComparisonOp::Le => {
                    ord == core::cmp::Ordering::Less || ord == core::cmp::Ordering::Equal
                }
                ComparisonOp::Gt => ord == core::cmp::Ordering::Greater,
                ComparisonOp::Ge => {
                    ord == core::cmp::Ordering::Greater || ord == core::cmp::Ordering::Equal
                }
                _ => {
                    return Err(Error::dynamic_err(
                        "err:FOER0000",
                        "unexpected comparison operator (string)",
                    ));
                }
            };
            Ok(res)
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

fn dedup_and_sort<N: crate::model::XdmNode>(nodes: &mut Vec<N>) -> Result<(), Error> {
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
    sort_doc_order(nodes)?;
    Ok(())
}

fn single_node<N>(seq: XdmSequence<N>) -> Result<N, Error> {
    let v = node_seq(seq)?;
    if v.len() != 1 {
        return Err(Error::dynamic_err(
            "err:XPTY0004",
            "node comparison expects single node on each side",
        ));
    }
    // Safe: length checked above; still avoid unwrap to prevent panic
    let first = v.into_iter().next().ok_or_else(|| {
        Error::dynamic_err(
            "err:XPTY0004",
            "node comparison expects single node on each side",
        )
    })?;
    Ok(first)
}

fn root_of<N: crate::model::XdmNode>(mut n: N) -> N {
    while let Some(p) = n.parent() {
        n = p;
    }
    n
}

fn sort_doc_order<N: crate::model::XdmNode>(nodes: &mut Vec<N>) -> Result<(), Error> {
    if nodes.len() <= 1 {
        return Ok(());
    }
    // Ensure all nodes share the same root; otherwise doc order under fallback is undefined.
    let base_root = root_of(nodes[0].clone());
    for n in nodes.iter() {
        if root_of(n.clone()) != base_root {
            return Err(Error::dynamic_err(
                "err:FOER0000",
                "document order requires adapter: nodes from different roots",
            ));
        }
    }
    // Stable insertion sort with error propagation
    let len = nodes.len();
    for i in 1..len {
        let mut j = i;
        while j > 0 {
            let ord = nodes[j - 1].compare_document_order(&nodes[j])?;
            if ord == core::cmp::Ordering::Greater {
                nodes.swap(j - 1, j);
                j -= 1;
            } else {
                break;
            }
        }
    }
    Ok(())
}

fn apply_axis<N: crate::model::XdmNode>(n: &N, axis: &AxisIR) -> Result<Vec<N>, Error> {
    match axis {
        AxisIR::SelfAxis => Ok(vec![n.clone()]),
        AxisIR::Child => Ok(n.children()),
        AxisIR::Attribute => Ok(n.attributes()),
        AxisIR::Descendant => {
            fn dfs<N: crate::model::XdmNode>(n: &N, out: &mut Vec<N>) {
                for c in n.children() {
                    out.push(c.clone());
                    dfs(&c, out);
                }
            }
            let mut acc = Vec::new();
            dfs(n, &mut acc);
            Ok(acc)
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
            Ok(acc)
        }
        AxisIR::Parent => Ok(n.parent().into_iter().collect()),
        AxisIR::Ancestor => {
            let mut v = Vec::new();
            let mut cur = n.parent();
            while let Some(p) = cur {
                v.push(p.clone());
                cur = p.parent();
            }
            Ok(v)
        }
        AxisIR::AncestorOrSelf => {
            let mut v = vec![n.clone()];
            let mut cur = n.parent();
            while let Some(p) = cur {
                v.push(p.clone());
                cur = p.parent();
            }
            Ok(v)
        }
        AxisIR::PrecedingSibling => {
            if let Some(parent) = n.parent() {
                let mut res = Vec::new();
                for c in parent.children() {
                    if c.compare_document_order(n)? == core::cmp::Ordering::Less {
                        res.push(c);
                    }
                }
                return Ok(res);
            }
            Ok(Vec::new())
        }
        AxisIR::FollowingSibling => {
            if let Some(parent) = n.parent() {
                let mut res = Vec::new();
                for c in parent.children() {
                    if c.compare_document_order(n)? == core::cmp::Ordering::Greater {
                        res.push(c);
                    }
                }
                return Ok(res);
            }
            Ok(Vec::new())
        }
        AxisIR::Preceding => {
            let root = root_of(n.clone());
            let all = apply_axis(&root, &AxisIR::DescendantOrSelf)?;
            let mut res = Vec::new();
            for m in all {
                if m.compare_document_order(n)? == core::cmp::Ordering::Less
                    && !is_ancestor_of(&m, n)
                    && m.kind() != crate::model::NodeKind::Attribute
                    && m.kind() != crate::model::NodeKind::Namespace
                {
                    res.push(m);
                }
            }
            Ok(res)
        }
        AxisIR::Following => {
            let root = root_of(n.clone());
            let all = apply_axis(&root, &AxisIR::DescendantOrSelf)?;
            let mut res = Vec::new();
            for m in all {
                if m.compare_document_order(n)? == core::cmp::Ordering::Greater
                    && !is_descendant_of(&m, n)
                    && m.kind() != crate::model::NodeKind::Attribute
                    && m.kind() != crate::model::NodeKind::Namespace
                {
                    res.push(m);
                }
            }
            Ok(res)
        }
        AxisIR::Namespace => {
            // Only element nodes have namespaces; otherwise empty
            if n.kind() == crate::model::NodeKind::Element {
                Ok(n.namespaces())
            } else {
                Ok(Vec::new())
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
