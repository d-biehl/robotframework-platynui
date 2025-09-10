use crate::compiler::ir::{ComparisonOp, CompiledXPath, InstrSeq, OpCode, AxisIR, NodeTestIR, SeqTypeIR, SingleTypeIR, QuantifierKind};
use crate::model::XdmNode;
use crate::runtime::{CallCtx, DynamicContext, Error};
use crate::xdm::{ExpandedName, XdmAtomicValue, XdmItem, XdmSequence};

/// Evaluate a compiled XPath program against a dynamic context.
pub fn evaluate<N: 'static + Send + Sync + XdmNode + Clone>(
    compiled: &CompiledXPath,
    dyn_ctx: &DynamicContext<N>,
)
    -> Result<XdmSequence<N>, Error>
{
    let mut vm = Vm::new(compiled, dyn_ctx);
    vm.run(&compiled.instrs)
}

/// Convenience: compile+evaluate a string using default static context.
pub fn evaluate_expr<N: 'static + Send + Sync + XdmNode + Clone>(
    expr: &str,
    dyn_ctx: &DynamicContext<N>,
)
    -> Result<XdmSequence<N>, Error>
{
    let compiled = crate::compiler::compile_xpath(expr)?;
    evaluate(&compiled, dyn_ctx)
}

struct Vm<'a, N> {
    compiled: &'a CompiledXPath,
    dyn_ctx: &'a DynamicContext<N>,
    stack: Vec<XdmSequence<N>>,
    // Frame stack for position()/last() support inside predicates / loops
    frames: Vec<Frame>,
    iter_state: Vec<IterState<N>>, // simple for-expression iteration state
}

#[derive(Clone, Debug)]
struct Frame {
    last: usize,
    pos: usize,
}

impl<'a, N: 'static + Send + Sync + XdmNode + Clone> Vm<'a, N> {
    fn new(compiled: &'a CompiledXPath, dyn_ctx: &'a DynamicContext<N>) -> Self {
    Self { compiled, dyn_ctx, stack: Vec::new(), frames: Vec::new(), iter_state: Vec::new() }
    }

    fn run(&mut self, code: &InstrSeq) -> Result<XdmSequence<N>, Error> {
        let mut ip: usize = 0;
        let ops = &code.0;
        while ip < ops.len() {
            match &ops[ip] {
                // Data and variables
                OpCode::PushAtomic(a) => {
                    self.stack.push(vec![XdmItem::Atomic(a.clone())]);
                    ip += 1;
                }
                OpCode::LoadVarByName(name) => {
                    let v = self
                        .dyn_ctx
                        .variables
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| Vec::new());
                    self.stack.push(v);
                    ip += 1;
                }
                OpCode::LoadContextItem => {
                    match &self.dyn_ctx.context_item {
                        Some(it) => self.stack.push(vec![it.clone()]),
                        None => self.stack.push(Vec::new()),
                    }
                    ip += 1;
                }
                OpCode::Position => {
                    let v = self.frames.last().map(|f| f.pos).unwrap_or(0) as i64;
                    self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Integer(v))]);
                    ip += 1;
                }
                OpCode::Last => {
                    let v = self.frames.last().map(|f| f.last).unwrap_or(0) as i64;
                    self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Integer(v))]);
                    ip += 1;
                }
                OpCode::ToRoot => {
                    // Navigate from current context item to root via parent() chain
                    let root = match &self.dyn_ctx.context_item {
                        Some(XdmItem::Node(n)) => {
                            let mut cur = n.clone();
                            let mut parent_opt = cur.parent();
                            while let Some(p) = parent_opt { cur = p.clone(); parent_opt = cur.parent(); }
                            vec![XdmItem::Node(cur)]
                        }
                        _ => Vec::new(),
                    };
                    self.stack.push(root);
                    ip += 1;
                }

                // Stack helpers
                OpCode::Dup => {
                    let top = self.stack.last().cloned().unwrap_or_default();
                    self.stack.push(top);
                    ip += 1;
                }
                OpCode::Swap => {
                    let len = self.stack.len();
                    if len >= 2 { self.stack.swap(len-1, len-2); }
                    ip += 1;
                }

                // Steps / filters
                OpCode::AxisStep(axis, test, pred_ir) => {
                    let input = self.pop_seq();
                    let mut out: XdmSequence<N> = Vec::new();
                    for it in input {
                        if let XdmItem::Node(node) = it {
                            let nodes = self.axis_iter(node.clone(), axis);
                            for n in nodes {
                                if self.node_test(&n, test) { out.push(XdmItem::Node(n)); }
                            }
                        }
                    }
                    // Apply embedded predicates (compiled with ToEBV already)
                    if !pred_ir.is_empty() {
                        let mut filtered = Vec::new();
                        let len = out.len();
                        for (idx, item) in out.into_iter().enumerate() {
                            let mut child = self.dyn_ctx.clone();
                            child.context_item = Some(item.clone());
                            let mut vm = Vm::new(self.compiled, &child);
                            vm.frames.push(Frame { last: len, pos: idx + 1 });
                            let mut keep = true;
                            for pred_code in pred_ir {
                                let r = vm.run(pred_code)?;
                                if !Self::ebv(&r)? { keep = false; break; }
                            }
                            if keep { filtered.push(item); }
                        }
                        self.stack.push(filtered);
                    } else {
                        self.stack.push(out);
                    }
                    ip += 1;
                }
                OpCode::ApplyPredicates(preds) => {
                    let input = self.pop_seq();
                    // Apply each predicate in order, boolean semantics only (compiler ensures ToEBV)
                    let mut current = input;
                    for p in preds {
                        let mut out: XdmSequence<N> = Vec::new();
                        let len = current.len();
                        for (idx, it) in current.into_iter().enumerate() {
                            let mut child = self.dyn_ctx.clone();
                            child.context_item = Some(it.clone());
                            let mut vm = Vm::new(self.compiled, &child);
                            vm.frames.push(Frame { last: len, pos: idx + 1 });
                            let res = vm.run(p)?; // predicate ends with ToEBV
                            let keep = Self::ebv(&res)?;
                            if keep { out.push(it); }
                        }
                        current = out;
                    }
                    self.stack.push(current);
                    ip += 1;
                }
                OpCode::DocOrderDistinct => {
                    let seq = self.pop_seq();
                    self.stack.push(self.doc_order_distinct(seq)?);
                    ip += 1;
                }

                // Arithmetic / logic
                OpCode::Add | OpCode::Sub | OpCode::Mul | OpCode::Div | OpCode::IDiv | OpCode::Mod => {
                    let rhs = self.pop_seq();
                    let lhs = self.pop_seq();
                    let a = Self::to_number(&lhs)?;
                    let b = Self::to_number(&rhs)?;
                    let res = match ops[ip] {
                        OpCode::Add => a + b,
                        OpCode::Sub => a - b,
                        OpCode::Mul => a * b,
                        OpCode::Div => {
                            if b == 0.0 { return Err(Error::dynamic_err("err:FOAR0001", "divide by zero")); }
                            a / b
                        }
                        OpCode::IDiv => {
                            if b == 0.0 { return Err(Error::dynamic_err("err:FOAR0001", "idiv by zero")); }
                            (a / b).trunc()
                        }
                        OpCode::Mod => {
                            if b == 0.0 { return Err(Error::dynamic_err("err:FOAR0001", "mod by zero")); }
                            a % b
                        }
                        _ => unreachable!(),
                    };
                    // Prefer integer if exact
                    if res.fract() == 0.0 {
                        self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Integer(res as i64))]);
                    } else {
                        self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Double(res))]);
                    }
                    ip += 1;
                }
                OpCode::And => {
                    let rhs = self.pop_seq();
                    let lhs = self.pop_seq();
                    let b = Self::ebv(&lhs)? && Self::ebv(&rhs)?;
                    self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::Or => {
                    let rhs = self.pop_seq();
                    let lhs = self.pop_seq();
                    let b = Self::ebv(&lhs)? || Self::ebv(&rhs)?;
                    self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::Not => {
                    let v = self.pop_seq();
                    let b = !Self::ebv(&v)?;
                    self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::ToEBV => {
                    let v = self.pop_seq();
                    let b = Self::ebv(&v)?;
                    self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::Atomize => {
                    let v = self.pop_seq();
                    self.stack.push(Self::atomize(v));
                    ip += 1;
                }
                OpCode::Pop => {
                    let _ = self.stack.pop();
                    ip += 1;
                }
                OpCode::JumpIfTrue(delta) => {
                    let v = self.pop_seq();
                    let b = Self::ebv(&v)?;
                    if b { ip += 1 + *delta; } else { ip += 1; }
                }
                OpCode::JumpIfFalse(delta) => {
                    let v = self.pop_seq();
                    let b = Self::ebv(&v)?;
                    if !b { ip += 1 + *delta; } else { ip += 1; }
                }
                OpCode::Jump(delta) => { ip += 1 + *delta; }

                // Comparisons
                OpCode::CompareValue(op) => {
                    let rhs = self.pop_seq();
                    let lhs = self.pop_seq();
                    let b = self.compare_value(&lhs, &rhs, *op)?;
                    self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::CompareGeneral(op) => {
                    let rhs = Self::atomize(self.pop_seq());
                    let lhs = Self::atomize(self.pop_seq());
                    let mut b = false;
                    'outer: for a in &lhs {
                        for c in &rhs {
                            if let (XdmItem::Atomic(la), XdmItem::Atomic(rb)) = (a, c) {
                                if self.compare_atomic(la, rb, *op)? { b = true; break 'outer; }
                            }
                        }
                    }
                    self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::NodeIs => {
                    let rhs = self.pop_seq();
                    let lhs = self.pop_seq();
                    let b = match (lhs.get(0), rhs.get(0)) {
                        (Some(XdmItem::Node(a)), Some(XdmItem::Node(b))) => a == b,
                        _ => false,
                    };
                    self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::NodeBefore | OpCode::NodeAfter => {
                    let after = matches!(&ops[ip], OpCode::NodeAfter);
                    let rhs = self.pop_seq();
                    let lhs = self.pop_seq();
                    let b = match (lhs.get(0), rhs.get(0)) {
                        (Some(XdmItem::Node(a)), Some(XdmItem::Node(b))) => {
                            match a.compare_document_order(b) {
                                Ok(ord) => if after { ord.is_gt() } else { ord.is_lt() },
                                Err(_) => false,
                            }
                        }
                        _ => false,
                    };
                    self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }

                // Sequences and sets
                OpCode::MakeSeq(n) => {
                    let n = *n;
                    let mut items: XdmSequence<N> = Vec::new();
                    // Pop N sequences, preserving left-to-right order
                    let len = self.stack.len();
                    let start = len.saturating_sub(n);
                    for _ in start..len { /* pop later */ }
                    let mut parts: Vec<XdmSequence<N>> = Vec::with_capacity(n);
                    for _ in 0..n { parts.push(self.stack.pop().unwrap_or_default()); }
                    parts.reverse();
                    for p in parts { items.extend(p); }
                    self.stack.push(items);
                    ip += 1;
                }
                OpCode::ConcatSeq => {
                    let rhs = self.pop_seq();
                    let mut lhs = self.pop_seq();
                    lhs.extend(rhs);
                    self.stack.push(lhs);
                    ip += 1;
                }
                OpCode::Union | OpCode::Intersect | OpCode::Except => {
                    let rhs = self.pop_seq();
                    let lhs = self.pop_seq();
                    let res = match &ops[ip] {
                        OpCode::Union => self.set_union(lhs, rhs),
                        OpCode::Intersect => self.set_intersect(lhs, rhs),
                        OpCode::Except => self.set_except(lhs, rhs),
                        _ => unreachable!(),
                    };
                    self.stack.push(res);
                    ip += 1;
                }
                OpCode::RangeTo => {
                    let end = Self::to_number(&self.pop_seq())?;
                    let start = Self::to_number(&self.pop_seq())?;
                    let mut out = Vec::new();
                    let a = start as i64; let b = end as i64;
                    if a <= b {
                        for i in a..=b { out.push(XdmItem::Atomic(XdmAtomicValue::Integer(i))); }
                    }
                    self.stack.push(out);
                    ip += 1;
                }

                // Control flow / bindings (not fully supported)
                OpCode::BeginScope(_) | OpCode::EndScope => { ip += 1; }
                OpCode::ForStartByName(var) => {
                    // Sequence to iterate is on stack before ForStartByName (compiler emits BeginScope then ForStartByName then body then ForNext then ForEnd EndScope)
                    // We replace it with first item; store iteration state in a synthetic frame using stack top sentinel (push remaining sequence reversed into stack?). Simpler: store iterator in frames via special frame extension.
                    let seq = self.pop_seq();
                    let len = seq.len();
                    let mut iter = seq.into_iter();
                    if let Some(first) = iter.next() { self.stack.push(vec![first]); } else { self.stack.push(Vec::new()); }
                    // Keep remaining items in an iterator boxed
                    self.iter_state.push(IterState { _var: var.clone(), rest: iter.collect(), index: 1, total: len });
                    ip += 1;
                }
                OpCode::ForNext => {
                    if let Some(state) = self.iter_state.last_mut() {
                        if state.index >= state.total { ip += 1; } else {
                            let next_item = state.rest.remove(0); // small sequences acceptable for now
                            state.index += 1;
                            self.stack.pop(); // remove previous body result (will be concatenated at end by ForEnd)
                            self.stack.push(vec![next_item]);
                            ip += 1;
                        }
                    } else { ip += 1; }
                }
                OpCode::ForEnd => {
                    let _ = self.iter_state.pop();
                    ip += 1;
                }
                OpCode::QuantStartByName(kind, _var) => {
                    // Input sequence on stack. Evaluate following predicate (already compiled) until QuantEnd encountered.
                    let seq = self.pop_seq();
                    let len = seq.len();
                    let mut result = match kind { QuantifierKind::Some => false, QuantifierKind::Every => true };
                    for (idx, it) in seq.into_iter().enumerate() {
                        let mut child = self.dyn_ctx.clone();
                        child.context_item = Some(it.clone());
                        let mut vm = Vm::new(self.compiled, &child);
                        vm.frames.push(Frame { last: len, pos: idx + 1 });
                        // Run until we hit QuantEnd; we simulate by reading subsequent ops copying them until QuantEnd
                        let body_ops = self.slice_until(ip + 1, |op| matches!(op, OpCode::QuantEnd));
                        let r = vm.run(&InstrSeq(body_ops))?;
                        let b = Self::ebv(&r)?;
                        match kind {
                            QuantifierKind::Some => { if b { result = true; break; } }
                            QuantifierKind::Every => { if !b { result = false; break; } }
                        }
                    }
                    // Skip body ops + QuantEnd
                    let skip_ops = self.skip_count(ip + 1, |op| matches!(op, OpCode::QuantEnd));
                    ip = ip + 1 + skip_ops + 1; // jump past QuantEnd
                    self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(result))]);
                }
                OpCode::QuantEnd => { ip += 1; }

                // Types
                OpCode::Cast(t) => { let v = self.pop_seq(); self.stack.push(self.cast(v, t)?); ip += 1; }
                OpCode::Castable(t) => { let v = self.pop_seq(); let ok = self.cast(v.clone(), t).is_ok(); self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(ok))]); ip += 1; }
                OpCode::Treat(t) => { let v = self.pop_seq(); self.assert_treat(&v, t)?; self.stack.push(v); ip += 1; }
                OpCode::InstanceOf(t) => { let v = self.pop_seq(); let b = self.instance_of(&v, t)?; self.stack.push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]); ip += 1; }

                // Functions
                OpCode::CallByName(name, argc) => {
                    let argc = *argc;
                    let mut args: Vec<XdmSequence<N>> = Vec::with_capacity(argc);
                    for _ in 0..argc { args.push(self.pop_seq()); }
                    args.reverse();
                    let en = self.resolve_function_name(name);
                    let Some(f) = self.dyn_ctx.functions.get(&en, argc) else {
                        return Err(Error::dynamic_err("err:XPST0017", format!("unknown function: {{{:?}}}#{argc}", en)));
                    };
                    // Resolve default collation for this call
                    let default_collation = self.resolve_default_collation();
                    let call_ctx = CallCtx {
                        dyn_ctx: self.dyn_ctx,
                        static_ctx: &self.compiled.static_ctx,
                        default_collation,
                        resolver: self.dyn_ctx.resolver.clone(),
                        regex: self.dyn_ctx.regex.clone(),
                    };
                    let result = (f)(&call_ctx, &args)?;
                    self.stack.push(result);
                    ip += 1;
                }

                // Errors
                OpCode::Raise(code) => {
                    return Err(Error::dynamic_err(code, "raised by program"));
                }
            }
        }

        // Result is TOS or empty
        Ok(self.stack.pop().unwrap_or_default())
    }

    fn pop_seq(&mut self) -> XdmSequence<N> {
        self.stack.pop().unwrap_or_default()
    }

    fn ebv(seq: &XdmSequence<N>) -> Result<bool, Error> {
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
                XdmItem::Node(_) => Ok(true),
                _ => Ok(true),
            },
            _ => Err(Error::dynamic_err(
                "err:FORG0006",
                "effective boolean value of sequence of length > 1",
            )),
        }
    }

    fn atomize(seq: XdmSequence<N>) -> XdmSequence<N> {
        let mut out = Vec::with_capacity(seq.len());
        for it in seq {
            match it {
                XdmItem::Atomic(a) => out.push(XdmItem::Atomic(a)),
                XdmItem::Node(n) => out.push(XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(n.string_value()))),
            }
        }
        out
    }

    fn to_number(seq: &XdmSequence<N>) -> Result<f64, Error> {
        let aseq = Self::atomize(seq.clone());
        if aseq.is_empty() {
            return Ok(f64::NAN);
        }
        match &aseq[0] {
            XdmItem::Atomic(a) => Self::atomic_to_number(a),
            XdmItem::Node(_) => Ok(f64::NAN),
        }
    }

    fn atomic_to_number(a: &XdmAtomicValue) -> Result<f64, Error> {
        Ok(match a {
            XdmAtomicValue::Integer(i) => *i as f64,
            XdmAtomicValue::Decimal(d) => *d,
            XdmAtomicValue::Double(d) => *d,
            XdmAtomicValue::Float(f) => *f as f64,
            XdmAtomicValue::Boolean(b) => if *b { 1.0 } else { 0.0 },
            XdmAtomicValue::UntypedAtomic(s) | XdmAtomicValue::String(s) => {
                s.parse::<f64>().unwrap_or(f64::NAN)
            }
            _ => f64::NAN,
        })
    }

    fn compare_value(&self, lhs: &XdmSequence<N>, rhs: &XdmSequence<N>, op: ComparisonOp) -> Result<bool, Error> {
        let la = Self::atomize(lhs.clone());
        let ra = Self::atomize(rhs.clone());
        if la.len() != 1 || ra.len() != 1 {
            return Err(Error::dynamic_err("err:FORG0006", "value comparison requires singletons"));
        }
        match (&la[0], &ra[0]) {
            (XdmItem::Atomic(a), XdmItem::Atomic(b)) => self.compare_atomic(a, b, op),
            _ => Ok(false),
        }
    }

    fn compare_atomic(&self, a: &XdmAtomicValue, b: &XdmAtomicValue, op: ComparisonOp) -> Result<bool, Error> {
        use ComparisonOp::*;
        // Very small coercion: if both numerics, compare numerically; if both booleans, compare booleans; else compare as strings
        let num = |x: &XdmAtomicValue| Self::atomic_to_number(x).unwrap_or(f64::NAN);
        let s = |x: &XdmAtomicValue| match x {
            XdmAtomicValue::String(v) | XdmAtomicValue::UntypedAtomic(v) => v.clone(),
            _ => format!("{:?}", x),
        };
        let res = match (a, b) {
            (XdmAtomicValue::Boolean(x), XdmAtomicValue::Boolean(y)) => match op {
                Eq => x == y,
                Ne => x != y,
                Lt => (!x) & *y,
                Le => (!x) | (*x == *y),
                Gt => *x & (!y),
                Ge => *x | (*x == *y),
            },
            (XdmAtomicValue::String(_), _) | (XdmAtomicValue::UntypedAtomic(_), _) | (_, XdmAtomicValue::String(_)) | (_, XdmAtomicValue::UntypedAtomic(_)) => {
                let ls = s(a); let rs = s(b);
                match op { Eq=> ls==rs, Ne=>ls!=rs, Lt=>ls<rs, Le=>ls<=rs, Gt=>ls>rs, Ge=>ls>=rs }
            }
            _ => {
                let ln = num(a); let rn = num(b);
                match op { Eq=> ln==rn, Ne=>ln!=rn, Lt=>ln<rn, Le=>ln<=rn, Gt=>ln>rn, Ge=>ln>=rn }
            }
        };
        Ok(res)
    }

    fn doc_order_distinct(&self, seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
        // For non-node items: return as-is; For nodes: sort+dedup by document order
        let mut nodes: Vec<N> = Vec::new();
        let mut others: Vec<XdmItem<N>> = Vec::new();
        for it in seq {
            match it { XdmItem::Node(n) => nodes.push(n), other => others.push(other) }
        }
        if nodes.is_empty() { return Ok(others); }
        // Dedup preserving first occurrence order by document order
        nodes.sort_by(|a, b| a.compare_document_order(b).unwrap_or(core::cmp::Ordering::Equal));
        nodes.dedup();
        let mut out: XdmSequence<N> = others;
        out.extend(nodes.into_iter().map(XdmItem::Node));
        Ok(out)
    }

    fn resolve_function_name(&self, n: &ExpandedName) -> ExpandedName {
        if n.ns_uri.is_some() { return n.clone(); }
        if let Some(ns) = &self.compiled.static_ctx.default_function_namespace {
            return ExpandedName { ns_uri: Some(ns.clone()), local: n.local.clone() };
        }
        n.clone()
    }

    fn resolve_default_collation(&self) -> Option<std::sync::Arc<dyn crate::runtime::Collation>> {
        // Dynamic default takes precedence, else static default
        let reg = &self.dyn_ctx.collations;
        if let Some(uri) = &self.dyn_ctx.default_collation { return reg.get(uri); }
        if let Some(uri) = &self.compiled.static_ctx.default_collation { return reg.get(uri); }
        None
    }

    // ===== Axis & NodeTest helpers =====
    fn axis_iter(&self, node: N, axis: &AxisIR) -> Vec<N> {
        match axis {
            AxisIR::SelfAxis => vec![node],
            AxisIR::Child => node.children(),
            AxisIR::Attribute => node.attributes(),
            AxisIR::Parent => node.parent().into_iter().collect(),
            AxisIR::Ancestor => {
                let mut out = Vec::new();
                let mut cur_opt = node.parent();
                while let Some(p) = cur_opt { out.push(p.clone()); cur_opt = p.parent(); }
                out
            }
            AxisIR::AncestorOrSelf => {
                let mut v = self.axis_iter(node.clone(), &AxisIR::Ancestor); v.insert(0, node); v
            }
            AxisIR::Descendant => self.collect_descendants(node, false),
            AxisIR::DescendantOrSelf => self.collect_descendants(node, true),
            AxisIR::FollowingSibling => self.siblings(node, false),
            AxisIR::PrecedingSibling => self.siblings(node, true),
            AxisIR::Following | AxisIR::Preceding | AxisIR::Namespace => Vec::new(), // simplified / NYI
        }
    }
    fn collect_descendants(&self, node: N, include_self: bool) -> Vec<N> {
        let mut out = Vec::new();
        if include_self { out.push(node.clone()); }
        fn dfs<N: XdmNode>(n: N, out: &mut Vec<N>) { for c in n.children() { out.push(c.clone()); dfs(c, out); } }
        dfs(node, &mut out);
        out
    }
    fn siblings(&self, node: N, preceding: bool) -> Vec<N> {
        if let Some(parent) = node.parent() {
            let mut sibs = parent.children();
            if preceding { sibs.retain(|s| s != &node); } else { sibs.retain(|s| s != &node); }
            sibs
        } else { Vec::new() }
    }
    fn node_test(&self, node: &N, test: &NodeTestIR) -> bool {
        use NodeTestIR::*;
        match test {
            AnyKind => true,
            Name(q) => node.name().map(|n| n.local == q.local && q.ns_uri == n.ns_uri).unwrap_or(false),
            WildcardAny => true,
            NsWildcard(ns) => node.name().map(|n| n.ns_uri.unwrap_or_default() == *ns).unwrap_or(false),
            LocalWildcard(local) => node.name().map(|n| n.local == *local).unwrap_or(false),
            KindText => matches!(node.kind(), crate::model::NodeKind::Text),
            KindComment => matches!(node.kind(), crate::model::NodeKind::Comment),
            KindProcessingInstruction(_) => matches!(node.kind(), crate::model::NodeKind::ProcessingInstruction),
            KindDocument(_) => matches!(node.kind(), crate::model::NodeKind::Document),
            KindElement { .. } => matches!(node.kind(), crate::model::NodeKind::Element),
            KindAttribute { .. } => matches!(node.kind(), crate::model::NodeKind::Attribute),
            KindSchemaElement(_) | KindSchemaAttribute(_) => true, // simplified
        }
    }

    // ===== Set operations (simplified, distinct by string value for atomic, pointer eq for nodes) =====
    fn set_union(&self, mut a: XdmSequence<N>, b: XdmSequence<N>) -> XdmSequence<N> { self.set_extend_distinct(&mut a, b); a }
    fn set_intersect(&self, a: XdmSequence<N>, b: XdmSequence<N>) -> XdmSequence<N> {
        a.into_iter().filter(|i| self.contains(&b, i)).collect()
    }
    fn set_except(&self, a: XdmSequence<N>, b: XdmSequence<N>) -> XdmSequence<N> {
        a.into_iter().filter(|i| !self.contains(&b, i)).collect()
    }
    fn set_extend_distinct(&self, a: &mut XdmSequence<N>, b: XdmSequence<N>) {
        for it in b { if !self.contains(a, &it) { a.push(it); } }
    }
    fn contains(&self, seq: &XdmSequence<N>, item: &XdmItem<N>) -> bool {
        seq.iter().any(|i| self.item_equal(i, item))
    }
    fn item_equal(&self, a: &XdmItem<N>, b: &XdmItem<N>) -> bool {
        match (a, b) { (XdmItem::Atomic(x), XdmItem::Atomic(y)) => format!("{:?}", x)==format!("{:?}", y), (XdmItem::Node(x), XdmItem::Node(y)) => x==y, _=>false }
    }

    // ===== Type operations (very small subset) =====
    fn cast(&self, seq: XdmSequence<N>, t: &SingleTypeIR) -> Result<XdmSequence<N>, Error> {
        if seq.len()>1 { return Err(Error::dynamic_err("err:XPTY0004", "cast of multi-item")); }
        if seq.is_empty() { if t.optional { return Ok(Vec::new()); } else { return Err(Error::dynamic_err("err:XPST0003","empty not allowed")); }}
        let item = seq[0].clone();
        let val = match item { XdmItem::Atomic(a)=>a, XdmItem::Node(n)=> XdmAtomicValue::UntypedAtomic(n.string_value()) };
        let casted = self.cast_atomic(val, &t.atomic)?;
        Ok(vec![XdmItem::Atomic(casted)])
    }
    fn cast_atomic(&self, a: XdmAtomicValue, target: &ExpandedName) -> Result<XdmAtomicValue, Error> {
        let local = &target.local;
        match local.as_str() {
            "string" => Ok(match a { XdmAtomicValue::String(s)|XdmAtomicValue::UntypedAtomic(s)=>XdmAtomicValue::String(s), other=>XdmAtomicValue::String(format!("{:?}", other)) }),
            "integer" => {
                let s = match a { XdmAtomicValue::Integer(i)=>return Ok(XdmAtomicValue::Integer(i)), XdmAtomicValue::String(s)|XdmAtomicValue::UntypedAtomic(s)=>s, _=>format!("{:?}", a) };
                s.parse::<i64>().map(|i| XdmAtomicValue::Integer(i)).map_err(|_| Error::dynamic_err("err:FORG0001","invalid integer"))
            }
            "boolean" => {
                let b = match a { XdmAtomicValue::Boolean(b)=>b, XdmAtomicValue::String(s)|XdmAtomicValue::UntypedAtomic(s)=> s=="true", _=>false }; Ok(XdmAtomicValue::Boolean(b))
            }
            _ => Err(Error::not_implemented("cast target type")),
        }
    }
    fn assert_treat(&self, seq: &XdmSequence<N>, t: &SeqTypeIR) -> Result<(), Error> {
        // Cardinality check only (for now); type promotion is NOT performed here.
    use crate::compiler::ir::{SeqTypeIR, OccurrenceIR};
        let (need_min, need_max, item_test) = match t {
            SeqTypeIR::EmptySequence => { if !seq.is_empty() { return Err(Error::dynamic_err("err:XPTY0004","treat as empty-sequence() but not empty")); } return Ok(()); }
            SeqTypeIR::Typed { item, occ } => {
                let (min,max) = match occ { OccurrenceIR::One => (1, Some(1)), OccurrenceIR::ZeroOrOne => (0, Some(1)), OccurrenceIR::ZeroOrMore => (0,None), OccurrenceIR::OneOrMore => (1,None) };
                (min,max,item)
            }
        };
        if seq.len() < need_min { return Err(Error::dynamic_err("err:XPTY0004","sequence too short for treat")); }
        if let Some(m) = need_max { if seq.len()>m { return Err(Error::dynamic_err("err:XPTY0004","sequence too long for treat")); }}
        for it in seq { if !self.item_matches_type(it, item_test)? { return Err(Error::dynamic_err("err:XPTY0004","item type mismatch in treat")); } }
        Ok(())
    }
    fn instance_of(&self, seq: &XdmSequence<N>, t: &SeqTypeIR) -> Result<bool, Error> {
    use crate::compiler::ir::{SeqTypeIR, OccurrenceIR};
        match t {
            SeqTypeIR::EmptySequence => Ok(seq.is_empty()),
            SeqTypeIR::Typed { item, occ } => {
                let ok_card = match occ { OccurrenceIR::One => seq.len()==1, OccurrenceIR::ZeroOrOne => seq.len()<=1, OccurrenceIR::ZeroOrMore => true, OccurrenceIR::OneOrMore => !seq.is_empty() };
                if !ok_card { return Ok(false); }
                for it in seq { if !self.item_matches_type(it, item)? { return Ok(false); } }
                Ok(true)
            }
        }
    }
    fn item_matches_type(&self, item: &XdmItem<N>, t: &crate::compiler::ir::ItemTypeIR) -> Result<bool, Error> {
        use crate::compiler::ir::ItemTypeIR; use XdmItem::*; match (item, t) {
            (_, ItemTypeIR::AnyItem) => Ok(true),
            (Node(_), ItemTypeIR::AnyNode) => Ok(true),
            (Atomic(_), ItemTypeIR::AnyNode) => Ok(false),
            (Node(n), ItemTypeIR::Kind(k)) => Ok(self.node_test(n, &match k { _ => k.clone() })), // reuse existing node_test via IR NodeTestIR
            (Atomic(a), ItemTypeIR::Atomic(exp)) => {
                // Very simplified: match only on local name; namespace ignored for now.
                let local = &exp.local;
                Ok(self.atomic_matches_name(a, local))
            }
            (Atomic(_), ItemTypeIR::Kind(_)) => Ok(false),
            (Node(_), ItemTypeIR::Atomic(_)) => Ok(false),
        }
    }
    fn atomic_matches_name(&self, a: &XdmAtomicValue, local: &str) -> bool {
        use XdmAtomicValue::*;
        match local {
            "anyAtomicType"|"atomic"|"string" => true, // placeholder broad acceptance
            "boolean" => matches!(a, Boolean(_)),
            "integer" => matches!(a, Integer(_)|Long(_)|Int(_)|Short(_)|Byte(_)|NonPositiveInteger(_)|NegativeInteger(_)|NonNegativeInteger(_)|PositiveInteger(_)),
            "decimal" => matches!(a, Decimal(_)|Integer(_)),
            "double" => matches!(a, Double(_) ),
            "float" => matches!(a, Float(_) ),
            _ => true, // TODO: refine; accept for now to avoid false negatives while expanding
        }
    }

    // ===== Quantifier helpers =====
    fn slice_until(&self, start: usize, end_pred: impl Fn(&OpCode)->bool) -> Vec<OpCode> { let mut v=Vec::new(); for op in self.compiled.instrs.0[start..].iter() { if end_pred(op){break;} v.push(op.clone()); } v }
    fn skip_count(&self, start: usize, end_pred: impl Fn(&OpCode)->bool) -> usize { let mut c=0; for op in self.compiled.instrs.0[start..].iter(){ if end_pred(op){break;} c+=1;} c }

}

#[derive(Clone)]
struct IterState<N> {
    _var: ExpandedName,
    rest: Vec<XdmItem<N>>, // remaining items
    index: usize,
    total: usize,
}
