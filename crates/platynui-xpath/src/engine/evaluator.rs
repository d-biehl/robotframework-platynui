use crate::compiler::ir::{
    AxisIR, ComparisonOp, CompiledXPath, InstrSeq, NameOrWildcard, NodeTestIR, OpCode,
    QuantifierKind, SeqTypeIR, SingleTypeIR,
};
use crate::engine::runtime::{CallCtx, DynamicContext, Error, ErrorCode, FunctionImplementations};
use crate::model::{NodeKind, XdmNode};
use crate::xdm::{ExpandedName, XdmAtomicValue, XdmItem, XdmSequence};
use chrono::Duration as ChronoDuration;
use chrono::{FixedOffset as ChronoFixedOffset, NaiveTime as ChronoNaiveTime, TimeZone};
use core::cmp::Ordering;
use smallvec::SmallVec;
use std::sync::Arc;

/// Evaluate a compiled XPath program against a dynamic context.
pub fn evaluate<N: 'static + Send + Sync + XdmNode + Clone>(
    compiled: &CompiledXPath,
    dyn_ctx: &DynamicContext<N>,
) -> Result<XdmSequence<N>, Error> {
    let mut vm = Vm::new(compiled, dyn_ctx);
    vm.run(&compiled.instrs)
}

/// Convenience: compile+evaluate a string using default static context.
pub fn evaluate_expr<N: 'static + Send + Sync + XdmNode + Clone>(
    expr: &str,
    dyn_ctx: &DynamicContext<N>,
) -> Result<XdmSequence<N>, Error> {
    let compiled = crate::compiler::compile_xpath(expr)?;
    evaluate(&compiled, dyn_ctx)
}

struct Vm<'a, N> {
    compiled: &'a CompiledXPath,
    dyn_ctx: &'a DynamicContext<N>,
    stack: SmallVec<[XdmSequence<N>; 8]>,
    local_vars: SmallVec<[(ExpandedName, XdmSequence<N>); 8]>,
    // Frame stack for position()/last() support inside predicates / loops
    frames: SmallVec<[Frame; 8]>,
    // Cached default collation for this VM (dynamic overrides static)
    default_collation: Option<std::sync::Arc<dyn crate::engine::collation::Collation>>,
    functions: Arc<FunctionImplementations<N>>,
}

#[derive(Clone, Debug)]
struct Frame {
    last: usize,
    pos: usize,
}

impl<'a, N: 'static + Send + Sync + XdmNode + Clone> Vm<'a, N> {
    fn new(compiled: &'a CompiledXPath, dyn_ctx: &'a DynamicContext<N>) -> Self {
        // Resolve default collation once per VM (dynamic takes precedence over static)
        let default_collation = {
            let reg = &dyn_ctx.collations;
            if let Some(uri) = &dyn_ctx.default_collation {
                reg.get(uri)
            } else if let Some(uri) = &compiled.static_ctx.default_collation {
                reg.get(uri)
            } else {
                None
            }
        };
        let functions = dyn_ctx.provide_functions();
        Self {
            compiled,
            dyn_ctx,
            stack: SmallVec::new(),
            local_vars: SmallVec::new(),
            frames: SmallVec::new(),
            default_collation,
            functions,
        }
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
                    if let Some((_, v)) = self.local_vars.iter().rev().find(|(n, _)| n == name) {
                        self.stack.push(v.clone());
                    } else {
                        let v = self.dyn_ctx.variable(name).unwrap_or_else(Vec::new);
                        self.stack.push(v);
                    }
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
                    self.stack
                        .push(vec![XdmItem::Atomic(XdmAtomicValue::Integer(v))]);
                    ip += 1;
                }
                OpCode::Last => {
                    let v = self.frames.last().map(|f| f.last).unwrap_or(0) as i64;
                    self.stack
                        .push(vec![XdmItem::Atomic(XdmAtomicValue::Integer(v))]);
                    ip += 1;
                }
                OpCode::ToRoot => {
                    // Navigate from current context item to root via parent() chain
                    let root = match &self.dyn_ctx.context_item {
                        Some(XdmItem::Node(n)) => {
                            let mut cur = n.clone();
                            let mut parent_opt = cur.parent();
                            while let Some(p) = parent_opt {
                                cur = p.clone();
                                parent_opt = cur.parent();
                            }
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
                    if len >= 2 {
                        self.stack.swap(len - 1, len - 2);
                    }
                    ip += 1;
                }

                // Steps / filters
                OpCode::AxisStep(axis, test, pred_ir) => {
                    let input = self.pop_seq();
                    let mut out: XdmSequence<N> = Vec::with_capacity(input.len());
                    for it in input {
                        if let XdmItem::Node(node) = it {
                            let nodes = self.axis_iter(node.clone(), axis);
                            for n in nodes {
                                // Base node-test evaluation
                                let mut pass = self.node_test(&n, test);
                                // XPath semantics: On the child axis, a wildcard NameTest ('*')
                                // selects element nodes only (not text, comments, or PIs).
                                // Our IR represents wildcard NameTests as NodeTestIR::WildcardAny.
                                // Restrict this case to element nodes when axis is Child.
                                if pass {
                                    use crate::model::NodeKind;
                                    if let (AxisIR::Child, NodeTestIR::WildcardAny) = (axis, test) {
                                        pass = matches!(n.kind(), NodeKind::Element);
                                    }
                                }
                                if pass {
                                    out.push(XdmItem::Node(n));
                                }
                            }
                        }
                    }
                    // Apply embedded predicates sequentially rebasing positions
                    if !pred_ir.is_empty() {
                        let mut current = out;
                        for pred_code in pred_ir {
                            let len = current.len();
                            let mut next: XdmSequence<N> = Vec::with_capacity(len);
                            for (idx, item) in current.into_iter().enumerate() {
                                let child = self.dyn_ctx.with_context_item(Some(item.clone()));
                                let mut vm = Vm::new(self.compiled, &child);
                                vm.frames.push(Frame {
                                    last: len,
                                    pos: idx + 1,
                                });
                                let r = vm.run(pred_code)?;
                                if Self::predicate_truth_value(&r, idx + 1, len)? {
                                    next.push(item);
                                }
                            }
                            current = next;
                        }
                        self.stack.push(current);
                    } else {
                        self.stack.push(out);
                    }
                    ip += 1;
                }
                OpCode::PathExprStep(step_ir) => {
                    let input = self.pop_seq();
                    let len = input.len();
                    let mut out: XdmSequence<N> = Vec::with_capacity(len);
                    for (idx, item) in input.into_iter().enumerate() {
                        let child = self.dyn_ctx.with_context_item(Some(item.clone()));
                        let mut vm = Vm::new(self.compiled, &child);
                        vm.frames.push(Frame {
                            last: len,
                            pos: idx + 1,
                        });
                        let res = vm.run(step_ir)?;
                        out.extend(res);
                    }
                    self.stack.push(out);
                    ip += 1;
                }
                OpCode::ApplyPredicates(preds) => {
                    let input = self.pop_seq();
                    // Apply each predicate in order, boolean semantics only (compiler ensures ToEBV)
                    let mut current = input;
                    for p in preds {
                        let len = current.len();
                        let mut out: XdmSequence<N> = Vec::with_capacity(len);
                        for (idx, it) in current.into_iter().enumerate() {
                            let child = self.dyn_ctx.with_context_item(Some(it.clone()));
                            let mut vm = Vm::new(self.compiled, &child);
                            vm.frames.push(Frame {
                                last: len,
                                pos: idx + 1,
                            });
                            let res = vm.run(p)?; // predicate raw result
                            let keep = Self::predicate_truth_value(&res, idx + 1, len)?;
                            if keep {
                                out.push(it);
                            }
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
                OpCode::Add
                | OpCode::Sub
                | OpCode::Mul
                | OpCode::Div
                | OpCode::IDiv
                | OpCode::Mod => {
                    use XdmAtomicValue as V;
                    // Atomize and enforce singleton operands
                    let rhs_seq = Self::atomize(self.pop_seq());
                    let lhs_seq = Self::atomize(self.pop_seq());
                    if lhs_seq.len() != 1 || rhs_seq.len() != 1 {
                        return Err(Error::from_code(
                            ErrorCode::FORG0006,
                            "arithmetic operands must be singletons",
                        ));
                    }
                    let (mut a, mut b) = match (&lhs_seq[0], &rhs_seq[0]) {
                        (XdmItem::Atomic(a), XdmItem::Atomic(b)) => (a.clone(), b.clone()),
                        _ => {
                            return Err(Error::from_code(
                                ErrorCode::XPTY0004,
                                "arithmetic on non-atomic",
                            ));
                        }
                    };

                    // Handle temporal arithmetic and duration ops before numeric normalization
                    // Supported:
                    // - dateTime ± dayTimeDuration
                    // - date ± yearMonthDuration (with day saturation)
                    // - duration ± duration (same family)
                    // - duration * number | number * duration
                    // - duration div number
                    // - yearMonthDuration div yearMonthDuration -> double
                    // - dayTimeDuration div dayTimeDuration -> double
                    let op = &ops[ip];
                    // Helper: add months to NaiveDate saturating day to end of month
                    fn add_months_saturating(
                        date: chrono::NaiveDate,
                        delta_months: i32,
                    ) -> chrono::NaiveDate {
                        use chrono::{Datelike, NaiveDate};
                        let y = date.year();
                        let m = date.month() as i32; // 1-12
                        let total = y
                            .checked_mul(12)
                            .unwrap_or(0)
                            .checked_add(m - 1)
                            .unwrap_or(0)
                            .checked_add(delta_months)
                            .unwrap_or(0);
                        let ny = total.div_euclid(12);
                        let nm0 = total.rem_euclid(12);
                        let nm = (nm0 + 1) as u32; // 1..=12
                        // compute last day of target month
                        let last_day = match nm {
                            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                            4 | 6 | 9 | 11 => 30,
                            2 => {
                                let leap = (ny % 4 == 0 && ny % 100 != 0) || (ny % 400 == 0);
                                if leap { 29 } else { 28 }
                            }
                            _ => 30,
                        } as u32;
                        let day = date.day().min(last_day);
                        NaiveDate::from_ymd_opt(ny, nm, day).unwrap()
                    }

                    // Numeric value for a if numeric, else None
                    let classify_numeric = |v: &V| -> Option<f64> {
                        match v {
                            V::Integer(i) => Some(*i as f64),
                            V::Decimal(d) => Some(*d),
                            V::Double(d) => Some(*d),
                            V::Float(f) => Some(*f as f64),
                            _ => None,
                        }
                    };

                    // duration * number and friends
                    let handled_temporal = match op {
                        OpCode::Add => {
                            match (&a, &b) {
                                (V::DateTime(dt), V::DayTimeDuration(secs)) => {
                                    let ndt = *dt + ChronoDuration::seconds(*secs);
                                    self.stack.push(vec![XdmItem::Atomic(V::DateTime(ndt))]);
                                    ip += 1;
                                    true
                                }
                                (V::DayTimeDuration(secs), V::DateTime(dt)) => {
                                    let ndt = *dt + ChronoDuration::seconds(*secs);
                                    self.stack.push(vec![XdmItem::Atomic(V::DateTime(ndt))]);
                                    ip += 1;
                                    true
                                }
                                (V::Date { date, tz }, V::YearMonthDuration(months)) => {
                                    let nd = add_months_saturating(*date, *months);
                                    self.stack
                                        .push(vec![XdmItem::Atomic(V::Date { date: nd, tz: *tz })]);
                                    ip += 1;
                                    true
                                }
                                (V::YearMonthDuration(months), V::Date { date, tz }) => {
                                    let nd = add_months_saturating(*date, *months);
                                    self.stack
                                        .push(vec![XdmItem::Atomic(V::Date { date: nd, tz: *tz })]);
                                    ip += 1;
                                    true
                                }
                                (V::DateTime(dt), V::YearMonthDuration(months)) => {
                                    // Apply months with day saturation preserving time and timezone offset
                                    let date_part = dt.naive_utc().date();
                                    let nd = add_months_saturating(date_part, *months);
                                    let naive_with_time = nd.and_time(dt.time());
                                    let ndt = dt
                                        .offset()
                                        .from_local_datetime(&naive_with_time)
                                        .single()
                                        .unwrap_or_else(|| {
                                            chrono::DateTime::from_naive_utc_and_offset(
                                                naive_with_time,
                                                *dt.offset(),
                                            )
                                        });
                                    self.stack.push(vec![XdmItem::Atomic(V::DateTime(ndt))]);
                                    ip += 1;
                                    true
                                }
                                (V::YearMonthDuration(months), V::DateTime(dt)) => {
                                    let date_part = dt.naive_utc().date();
                                    let nd = add_months_saturating(date_part, *months);
                                    let naive_with_time = nd.and_time(dt.time());
                                    let ndt = dt
                                        .offset()
                                        .from_local_datetime(&naive_with_time)
                                        .single()
                                        .unwrap_or_else(|| {
                                            chrono::DateTime::from_naive_utc_and_offset(
                                                naive_with_time,
                                                *dt.offset(),
                                            )
                                        });
                                    self.stack.push(vec![XdmItem::Atomic(V::DateTime(ndt))]);
                                    ip += 1;
                                    true
                                }
                                (V::YearMonthDuration(a_m), V::YearMonthDuration(b_m)) => {
                                    self.stack.push(vec![XdmItem::Atomic(V::YearMonthDuration(
                                        *a_m + *b_m,
                                    ))]);
                                    ip += 1;
                                    true
                                }
                                (V::DayTimeDuration(a_s), V::DayTimeDuration(b_s)) => {
                                    self.stack.push(vec![XdmItem::Atomic(V::DayTimeDuration(
                                        *a_s + *b_s,
                                    ))]);
                                    ip += 1;
                                    true
                                }
                                _ => false,
                            }
                        }
                        OpCode::Sub => match (&a, &b) {
                            (V::DateTime(dt), V::DayTimeDuration(secs)) => {
                                let ndt = *dt - ChronoDuration::seconds(*secs);
                                self.stack.push(vec![XdmItem::Atomic(V::DateTime(ndt))]);
                                ip += 1;
                                true
                            }
                            (V::Date { date, tz }, V::YearMonthDuration(months)) => {
                                let nd = add_months_saturating(*date, -*months);
                                self.stack
                                    .push(vec![XdmItem::Atomic(V::Date { date: nd, tz: *tz })]);
                                ip += 1;
                                true
                            }
                            (V::DateTime(dt), V::YearMonthDuration(months)) => {
                                let date_part = dt.naive_utc().date();
                                let nd = add_months_saturating(date_part, -*months);
                                let naive_with_time = nd.and_time(dt.time());
                                let ndt = dt
                                    .offset()
                                    .from_local_datetime(&naive_with_time)
                                    .single()
                                    .unwrap_or_else(|| {
                                        chrono::DateTime::from_naive_utc_and_offset(
                                            naive_with_time,
                                            *dt.offset(),
                                        )
                                    });
                                self.stack.push(vec![XdmItem::Atomic(V::DateTime(ndt))]);
                                ip += 1;
                                true
                            }
                            (V::DateTime(da), V::DateTime(db)) => {
                                let diff = (*da - *db).num_seconds();
                                self.stack
                                    .push(vec![XdmItem::Atomic(V::DayTimeDuration(diff))]);
                                ip += 1;
                                true
                            }
                            (V::YearMonthDuration(a_m), V::YearMonthDuration(b_m)) => {
                                self.stack
                                    .push(vec![XdmItem::Atomic(V::YearMonthDuration(*a_m - *b_m))]);
                                ip += 1;
                                true
                            }
                            (V::DayTimeDuration(a_s), V::DayTimeDuration(b_s)) => {
                                self.stack
                                    .push(vec![XdmItem::Atomic(V::DayTimeDuration(*a_s - *b_s))]);
                                ip += 1;
                                true
                            }
                            _ => false,
                        },
                        OpCode::Mul => {
                            // duration * number or number * duration
                            match (&a, &b) {
                                (V::DayTimeDuration(secs), _) => {
                                    if let Some(n) = classify_numeric(&b) {
                                        let v = (*secs as f64 * n).trunc() as i64;
                                        self.stack
                                            .push(vec![XdmItem::Atomic(V::DayTimeDuration(v))]);
                                        ip += 1;
                                        true
                                    } else {
                                        false
                                    }
                                }
                                (V::YearMonthDuration(months), _) => {
                                    if let Some(n) = classify_numeric(&b) {
                                        let v = (*months as f64 * n).trunc() as i32;
                                        self.stack
                                            .push(vec![XdmItem::Atomic(V::YearMonthDuration(v))]);
                                        ip += 1;
                                        true
                                    } else {
                                        false
                                    }
                                }
                                (_, V::DayTimeDuration(secs)) => {
                                    if let Some(n) = classify_numeric(&a) {
                                        let v = (*secs as f64 * n).trunc() as i64;
                                        self.stack
                                            .push(vec![XdmItem::Atomic(V::DayTimeDuration(v))]);
                                        ip += 1;
                                        true
                                    } else {
                                        false
                                    }
                                }
                                (_, V::YearMonthDuration(months)) => {
                                    if let Some(n) = classify_numeric(&a) {
                                        let v = (*months as f64 * n).trunc() as i32;
                                        self.stack
                                            .push(vec![XdmItem::Atomic(V::YearMonthDuration(v))]);
                                        ip += 1;
                                        true
                                    } else {
                                        false
                                    }
                                }
                                _ => false,
                            }
                        }
                        OpCode::Div => match (&a, &b) {
                            (V::YearMonthDuration(a_m), V::YearMonthDuration(b_m)) => {
                                if *b_m == 0 {
                                    return Err(Error::from_code(
                                        ErrorCode::FOAR0001,
                                        "divide by zero",
                                    ));
                                }
                                let v = *a_m as f64 / *b_m as f64;
                                self.stack.push(vec![XdmItem::Atomic(V::Double(v))]);
                                ip += 1;
                                true
                            }
                            (V::DayTimeDuration(a_s), V::DayTimeDuration(b_s)) => {
                                if *b_s == 0 {
                                    return Err(Error::from_code(
                                        ErrorCode::FOAR0001,
                                        "divide by zero",
                                    ));
                                }
                                let v = *a_s as f64 / *b_s as f64;
                                self.stack.push(vec![XdmItem::Atomic(V::Double(v))]);
                                ip += 1;
                                true
                            }
                            (V::YearMonthDuration(months), _) => {
                                if let Some(n) = classify_numeric(&b) {
                                    if n == 0.0 {
                                        return Err(Error::from_code(
                                            ErrorCode::FOAR0001,
                                            "divide by zero",
                                        ));
                                    }
                                    let v = (*months as f64 / n).trunc() as i32;
                                    self.stack
                                        .push(vec![XdmItem::Atomic(V::YearMonthDuration(v))]);
                                    ip += 1;
                                    true
                                } else {
                                    false
                                }
                            }
                            (V::DayTimeDuration(secs), _) => {
                                if let Some(n) = classify_numeric(&b) {
                                    if n == 0.0 {
                                        return Err(Error::from_code(
                                            ErrorCode::FOAR0001,
                                            "divide by zero",
                                        ));
                                    }
                                    let v = (*secs as f64 / n).trunc() as i64;
                                    self.stack
                                        .push(vec![XdmItem::Atomic(V::DayTimeDuration(v))]);
                                    ip += 1;
                                    true
                                } else {
                                    false
                                }
                            }
                            _ => false,
                        },
                        OpCode::IDiv | OpCode::Mod => false, // not supported for durations
                        _ => false,
                    };
                    if handled_temporal {
                        continue;
                    }
                    // Normalize untypedAtomic: must be numeric for arithmetic. Invalid lexical -> FORG0001.
                    let norm_untyped = |v: &V| -> Result<V, Error> {
                        Ok(match v {
                            V::UntypedAtomic(s) => match s.parse::<f64>() {
                                Ok(num) => V::Double(num),
                                Err(_) => {
                                    return Err(Error::from_code(
                                        ErrorCode::FORG0001,
                                        "invalid numeric literal for arithmetic",
                                    ));
                                }
                            },
                            _ => v.clone(),
                        })
                    };
                    a = norm_untyped(&a)?;
                    b = norm_untyped(&b)?;

                    // Classification + minimal numeric promotion (duplicated small helper from compare_atomic)
                    #[derive(Clone, Copy)]
                    enum NumKind {
                        Int(i64),
                        Dec(f64),
                        Float(f32),
                        Double(f64),
                    }
                    impl NumKind {
                        fn to_f64(self) -> f64 {
                            match self {
                                NumKind::Int(i) => i as f64,
                                NumKind::Dec(d) => d,
                                NumKind::Float(f) => f as f64,
                                NumKind::Double(d) => d,
                            }
                        }
                    }
                    fn classify(v: &V) -> Option<NumKind> {
                        match v {
                            V::Integer(i) => Some(NumKind::Int(*i)),
                            V::Decimal(d) => Some(NumKind::Dec(*d)),
                            V::Float(f) => Some(NumKind::Float(*f)),
                            V::Double(d) => Some(NumKind::Double(*d)),
                            _ => None,
                        }
                    }
                    fn unify_numeric(a: NumKind, b: NumKind) -> (NumKind, NumKind) {
                        use NumKind::*;
                        match (a, b) {
                            (Double(x), y) => (Double(x), Double(y.to_f64())),
                            (y, Double(x)) => (Double(y.to_f64()), Double(x)),
                            (Float(x), Float(y)) => (Float(x), Float(y)),
                            (Float(x), Int(y)) => (Float(x), Float(y as f32)),
                            (Int(x), Float(y)) => (Float(x as f32), Float(y)),
                            (Float(x), Dec(y)) => (Float(x), Float(y as f32)),
                            (Dec(x), Float(y)) => (Float(x as f32), Float(y)),
                            (Dec(x), Dec(y)) => (Dec(x), Dec(y)),
                            (Dec(x), Int(y)) => (Dec(x), Dec(y as f64)),
                            (Int(x), Dec(y)) => (Dec(x as f64), Dec(y)),
                            (Int(x), Int(y)) => (Int(x), Int(y)),
                        }
                    }

                    let (ka, kb) = match (classify(&a), classify(&b)) {
                        (Some(x), Some(y)) => (x, y),
                        _ => {
                            return Err(Error::from_code(
                                ErrorCode::XPTY0004,
                                "non-numeric operand",
                            ));
                        }
                    };
                    let (ua, ub) = unify_numeric(ka, kb);

                    // Determine promoted result "kind" (excluding operation-specific adjustments)
                    use NumKind::*;
                    let promoted_kind = match (ua, ub) {
                        (Double(_), _) | (_, Double(_)) => Double(0.0),
                        (Float(_), _) | (_, Float(_)) => Float(0.0),
                        (Dec(_), _) | (_, Dec(_)) => Dec(0.0),
                        (Int(_), Int(_)) => Int(0),
                    };

                    // Integer-specialized path: when both operands are Int, prefer exact i128 arithmetic
                    // with lazy promotion to decimal on overflow. Only emit FOAR0002 where no representable
                    // promotion exists (e.g., idiv result beyond i64 range which must be xs:integer).
                    let mut pushed = false;
                    if matches!((ua, ub), (Int(_), Int(_))) {
                        let (ai, bi) = match (ua, ub) {
                            (Int(x), Int(y)) => (x as i128, y as i128),
                            _ => unreachable!(),
                        };
                        match &ops[ip] {
                            OpCode::Add => {
                                if let Some(sum) = ai.checked_add(bi) {
                                    if sum >= i64::MIN as i128 && sum <= i64::MAX as i128 {
                                        self.stack
                                            .push(vec![XdmItem::Atomic(V::Integer(sum as i64))]);
                                    } else {
                                        self.stack
                                            .push(vec![XdmItem::Atomic(V::Decimal(sum as f64))]);
                                    }
                                    ip += 1;
                                    pushed = true;
                                } else {
                                    // i128 overflow (extremely rare) → promote to decimal
                                    self.stack.push(vec![XdmItem::Atomic(V::Decimal(
                                        (ai as f64) + (bi as f64),
                                    ))]);
                                    ip += 1;
                                    pushed = true;
                                }
                            }
                            OpCode::Sub => {
                                if let Some(diff) = ai.checked_sub(bi) {
                                    if diff >= i64::MIN as i128 && diff <= i64::MAX as i128 {
                                        self.stack
                                            .push(vec![XdmItem::Atomic(V::Integer(diff as i64))]);
                                    } else {
                                        self.stack
                                            .push(vec![XdmItem::Atomic(V::Decimal(diff as f64))]);
                                    }
                                    ip += 1;
                                    pushed = true;
                                } else {
                                    self.stack.push(vec![XdmItem::Atomic(V::Decimal(
                                        (ai as f64) - (bi as f64),
                                    ))]);
                                    ip += 1;
                                    pushed = true;
                                }
                            }
                            OpCode::Mul => {
                                if let Some(prod) = ai.checked_mul(bi) {
                                    if prod >= i64::MIN as i128 && prod <= i64::MAX as i128 {
                                        self.stack
                                            .push(vec![XdmItem::Atomic(V::Integer(prod as i64))]);
                                    } else {
                                        self.stack
                                            .push(vec![XdmItem::Atomic(V::Decimal(prod as f64))]);
                                    }
                                    ip += 1;
                                    pushed = true;
                                } else {
                                    self.stack.push(vec![XdmItem::Atomic(V::Decimal(
                                        (ai as f64) * (bi as f64),
                                    ))]);
                                    ip += 1;
                                    pushed = true;
                                }
                            }
                            OpCode::IDiv => {
                                if bi == 0 {
                                    return Err(Error::from_code(
                                        ErrorCode::FOAR0001,
                                        "idiv by zero",
                                    ));
                                }
                                // floor division semantics
                                let q_trunc = ai / bi; // trunc toward 0
                                let r = ai % bi;
                                let needs_adjust = (r != 0) && ((ai ^ bi) < 0);
                                let q_floor = if needs_adjust { q_trunc - 1 } else { q_trunc };
                                if q_floor >= i64::MIN as i128 && q_floor <= i64::MAX as i128 {
                                    self.stack
                                        .push(vec![XdmItem::Atomic(V::Integer(q_floor as i64))]);
                                } else {
                                    // xs:integer result cannot be represented by our i64 storage → FOAR0002
                                    return Err(Error::from_code(
                                        ErrorCode::FOAR0002,
                                        "idiv result overflows xs:integer range",
                                    ));
                                }
                                ip += 1;
                                pushed = true;
                            }
                            OpCode::Mod => {
                                if bi == 0 {
                                    return Err(Error::from_code(
                                        ErrorCode::FOAR0001,
                                        "mod by zero",
                                    ));
                                }
                                // XPath mod defined as a - b*floor(a/b); for integers we can mirror via arithmetic
                                let q_trunc = ai / bi;
                                let r_trunc = ai % bi;
                                let needs_adjust = (r_trunc != 0) && ((ai ^ bi) < 0);
                                let q_floor = if needs_adjust { q_trunc - 1 } else { q_trunc };
                                let rem = ai - bi * q_floor;
                                // rem magnitude is < |bi|, thus guaranteed to fit into i64
                                self.stack
                                    .push(vec![XdmItem::Atomic(V::Integer(rem as i64))]);
                                ip += 1;
                                pushed = true;
                            }
                            OpCode::Div => {}
                            _ => {}
                        }
                    }
                    if pushed {
                        continue;
                    }

                    // Extract numeric primitives for calculation (generic floating/decimal path)
                    let (av_f64, bv_f64) = (ua.to_f64(), ub.to_f64());
                    // Operation semantics
                    let op = &ops[ip];
                    let result_value = match op {
                        OpCode::Add => av_f64 + bv_f64,
                        OpCode::Sub => av_f64 - bv_f64,
                        OpCode::Mul => av_f64 * bv_f64,
                        OpCode::Div => {
                            if bv_f64 == 0.0 {
                                match promoted_kind {
                                    // IEEE 754 semantics for float/double: produce ±INF or NaN
                                    NumKind::Double(_) | NumKind::Float(_) => av_f64 / bv_f64,
                                    // Decimal / Integer division by zero is an error per XPath 2.0
                                    _ => {
                                        return Err(Error::from_code(
                                            ErrorCode::FOAR0001,
                                            "divide by zero",
                                        ));
                                    }
                                }
                            } else {
                                av_f64 / bv_f64
                            }
                        }
                        OpCode::IDiv => {
                            if bv_f64 == 0.0 {
                                return Err(Error::from_code(ErrorCode::FOAR0001, "idiv by zero"));
                            }
                            // floor division per spec (handles negatives correctly)
                            (av_f64 / bv_f64).floor()
                        }
                        OpCode::Mod => {
                            if bv_f64 == 0.0 {
                                return Err(Error::from_code(ErrorCode::FOAR0001, "mod by zero"));
                            }
                            av_f64 % bv_f64
                        }
                        _ => unreachable!(),
                    };

                    // Determine result type (XPath 2.0 rules simplified):
                    // - idiv -> integer
                    // - div: if promoted integer -> decimal; if decimal -> decimal; float->float; double->double
                    // - add/sub/mul/mod -> promoted kind
                    let result_atomic = match op {
                        OpCode::IDiv => {
                            // Guard overflow: xs:integer result must fit our i64 storage
                            if !result_value.is_finite()
                                || result_value < i64::MIN as f64
                                || result_value > i64::MAX as f64
                            {
                                return Err(Error::from_code(
                                    ErrorCode::FOAR0002,
                                    "idiv result overflows xs:integer range",
                                ));
                            }
                            V::Integer(result_value as i64)
                        }
                        OpCode::Div => match promoted_kind {
                            Double(_) => V::Double(result_value),
                            Float(_) => V::Float(result_value as f32),
                            Dec(_) | Int(_) => V::Decimal(result_value), // integer division yields decimal
                        },
                        OpCode::Add | OpCode::Sub | OpCode::Mul => match promoted_kind {
                            Double(_) => V::Double(result_value),
                            Float(_) => V::Float(result_value as f32),
                            Dec(_) => V::Decimal(result_value),
                            Int(_) => {
                                // If exact integer keep integer else decimal (rare due to overflow/frac)
                                if (result_value.fract()).abs() < f64::EPSILON {
                                    V::Integer(result_value as i64)
                                } else {
                                    V::Decimal(result_value)
                                }
                            }
                        },
                        OpCode::Mod => match promoted_kind {
                            Double(_) => V::Double(result_value),
                            Float(_) => V::Float(result_value as f32),
                            Dec(_) => V::Decimal(result_value),
                            Int(_) => V::Integer(result_value as i64),
                        },
                        _ => unreachable!(),
                    };
                    self.stack.push(vec![XdmItem::Atomic(result_atomic)]);
                    ip += 1;
                }
                OpCode::And => {
                    let rhs = self.pop_seq();
                    let lhs = self.pop_seq();
                    let b = Self::ebv(&lhs)? && Self::ebv(&rhs)?;
                    self.stack
                        .push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::Or => {
                    let rhs = self.pop_seq();
                    let lhs = self.pop_seq();
                    let b = Self::ebv(&lhs)? || Self::ebv(&rhs)?;
                    self.stack
                        .push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::Not => {
                    let v = self.pop_seq();
                    let b = !Self::ebv(&v)?;
                    self.stack
                        .push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::ToEBV => {
                    let v = self.pop_seq();
                    let b = Self::ebv(&v)?;
                    self.stack
                        .push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
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
                    if b {
                        ip += 1 + *delta;
                    } else {
                        ip += 1;
                    }
                }
                OpCode::JumpIfFalse(delta) => {
                    let v = self.pop_seq();
                    let b = Self::ebv(&v)?;
                    if !b {
                        ip += 1 + *delta;
                    } else {
                        ip += 1;
                    }
                }
                OpCode::Jump(delta) => {
                    ip += 1 + *delta;
                }

                // Comparisons
                OpCode::CompareValue(op) => {
                    // Value comparison ( =, !=, lt, etc. with 'value' grammar) expects each side to be a singleton.
                    // We atomize inside compare_value() exactly once per operand and error with FORG0006
                    // if cardinality != 1. Avoided adding a separate Atomize opcode here to keep bytecode compact.
                    let rhs = self.pop_seq();
                    let lhs = self.pop_seq();
                    let b = self.compare_value(&lhs, &rhs, *op)?;
                    self.stack
                        .push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::CompareGeneral(op) => {
                    // General comparison (any-to-any). We atomize both sequences here once and then iterate pairs.
                    // Incomparable type pairs (FORG0006 / XPTY0004) are skipped per XPath 2.0 general comparison semantics.
                    let rhs = Self::atomize(self.pop_seq());
                    let lhs = Self::atomize(self.pop_seq());
                    let mut any_true = false;
                    'search: for a in &lhs {
                        for c in &rhs {
                            if let (XdmItem::Atomic(la), XdmItem::Atomic(rb)) = (a, c) {
                                match self.compare_atomic(la, rb, *op) {
                                    Ok(res) => {
                                        if res {
                                            any_true = true;
                                            break 'search;
                                        }
                                    }
                                    Err(e) => {
                                        // Treat FORG0006 / XPTY0004 as incomparable → ignore
                                        match e.code_enum() {
                                            ErrorCode::FORG0006 | ErrorCode::XPTY0004 => {}
                                            _ => return Err(e),
                                        }
                                    }
                                }
                            }
                        }
                    }
                    self.stack
                        .push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(any_true))]);
                    ip += 1;
                }
                OpCode::NodeIs => {
                    let rhs = self.pop_seq();
                    let lhs = self.pop_seq();
                    let b = match (lhs.first(), rhs.first()) {
                        (Some(XdmItem::Node(a)), Some(XdmItem::Node(b))) => a == b,
                        _ => false,
                    };
                    self.stack
                        .push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }
                OpCode::NodeBefore | OpCode::NodeAfter => {
                    let after = matches!(&ops[ip], OpCode::NodeAfter);
                    let rhs = self.pop_seq();
                    let lhs = self.pop_seq();
                    let b = match (lhs.first(), rhs.first()) {
                        (Some(XdmItem::Node(a)), Some(XdmItem::Node(b))) => {
                            match a.compare_document_order(b) {
                                Ok(ord) => {
                                    if after {
                                        ord.is_gt()
                                    } else {
                                        ord.is_lt()
                                    }
                                }
                                Err(e) => {
                                    return Err(e);
                                }
                            }
                        }
                        _ => false,
                    };
                    self.stack
                        .push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }

                // Sequences and sets
                OpCode::MakeSeq(n) => {
                    let n = *n;
                    let mut items: XdmSequence<N> = Vec::new();
                    // Pop N sequences, preserving left-to-right order
                    let len = self.stack.len();
                    let start = len.saturating_sub(n);
                    for _ in start..len {}
                    let mut parts: Vec<XdmSequence<N>> = Vec::with_capacity(n);
                    for _ in 0..n {
                        parts.push(self.stack.pop().unwrap_or_default());
                    }
                    parts.reverse();
                    for p in parts {
                        items.extend(p);
                    }
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
                    // Spec: set operators apply to node sequences only
                    let is_nodes_only =
                        |s: &XdmSequence<N>| s.iter().all(|it| matches!(it, XdmItem::Node(_)));
                    if !is_nodes_only(&lhs) || !is_nodes_only(&rhs) {
                        return Err(Error::from_code(
                            ErrorCode::XPTY0004,
                            "set operators require node sequences",
                        ));
                    }
                    let res = match &ops[ip] {
                        OpCode::Union => self.set_union(lhs, rhs),
                        OpCode::Intersect => self.set_intersect(lhs, rhs),
                        OpCode::Except => self.set_except(lhs, rhs),
                        _ => unreachable!(),
                    }?;
                    self.stack.push(res);
                    ip += 1;
                }
                OpCode::RangeTo => {
                    let end = Self::to_number(&self.pop_seq())?;
                    let start = Self::to_number(&self.pop_seq())?;
                    let mut out = Vec::new();
                    let a = start as i64;
                    let b = end as i64;
                    if a <= b {
                        for i in a..=b {
                            out.push(XdmItem::Atomic(XdmAtomicValue::Integer(i)));
                        }
                    }
                    self.stack.push(out);
                    ip += 1;
                }

                // Control flow / bindings (not fully supported)
                OpCode::BeginScope(_) | OpCode::EndScope => {
                    ip += 1;
                }
                OpCode::LetStartByName(var_name) => {
                    let value = self.pop_seq();
                    self.local_vars.push((var_name.clone(), value));
                    ip += 1;
                }
                OpCode::LetEnd => {
                    if self.local_vars.pop().is_none() {
                        return Err(Error::from_code(
                            ErrorCode::FOER0000,
                            "imbalanced let scope during evaluation",
                        ));
                    }
                    ip += 1;
                }
                OpCode::ForStartByName(var_name) => {
                    // Input sequence is on stack before ForStartByName.
                    // We execute the embedded body (between ForStartByName and ForNext at current depth)
                    // once per item using a child VM with a cloned dynamic context where the variable is bound.
                    let input_seq = self.pop_seq();
                    let item_count = input_seq.len();

                    // Locate the body [ip+1 .. for_next_index) and the matching ForEnd (to skip afterwards),
                    // handling nesting via depth counting for ForStartByName/ForEnd pairs.
                    let mut depth: i32 = 0;
                    let mut j = ip + 1; // scan after ForStartByName
                    let mut for_next_index: Option<usize> = None;
                    let mut for_end_index: Option<usize> = None;
                    while j < ops.len() {
                        match &ops[j] {
                            OpCode::ForStartByName(_) => {
                                depth += 1;
                            }
                            OpCode::ForEnd => {
                                if depth == 0 {
                                    for_end_index = Some(j);
                                    break;
                                } else {
                                    depth -= 1;
                                }
                            }
                            OpCode::ForNext => {
                                if depth == 0 && for_next_index.is_none() {
                                    for_next_index = Some(j);
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    let (for_next_ix, for_end_ix) = match (for_next_index, for_end_index) {
                        (Some(n), Some(e)) => (n, e),
                        _ => {
                            return Err(Error::not_implemented(
                                "unterminated or malformed for-expression body",
                            ));
                        }
                    };
                    let body_slice = &ops[ip + 1..for_next_ix];
                    let body_instr = InstrSeq(body_slice.to_vec());

                    // Prepare base dynamic context for child VMs (shared via cheap clones)
                    let shared_ctx = self.dyn_ctx.clone();
                    let mut acc: XdmSequence<N> = Vec::new();
                    for (idx, item) in input_seq.into_iter().enumerate() {
                        let iter_ctx = shared_ctx
                            .with_context_item(Some(item.clone()))
                            .with_variable(var_name.clone(), vec![item.clone()]);
                        let mut inner_vm = Vm::new(self.compiled, &iter_ctx);
                        inner_vm.frames.push(Frame {
                            last: item_count,
                            pos: idx + 1,
                        });
                        let body_res = inner_vm.run(&body_instr)?;
                        acc.extend(body_res);
                    }

                    // Push accumulated result and jump to instruction after ForEnd (skip body/ForNext/ForEnd entirely)
                    self.stack.push(acc);
                    ip = for_end_ix + 1;
                }
                // These are effectively skipped when ForStart handles the loop; keep as harmless no-ops for safety.
                OpCode::ForNext => {
                    ip += 1;
                }
                OpCode::ForEnd => {
                    ip += 1;
                }
                OpCode::QuantStartByName(kind, var_name) => {
                    // Sequence evaluated before quantifier is on stack
                    let input_seq = self.pop_seq();
                    let item_count = input_seq.len();

                    // Find matching QuantEnd in CURRENT ops slice (handles nesting by depth counting)
                    let mut depth: i32 = 0;
                    let mut j = ip + 1; // start just after QuantStartByName
                    while j < ops.len() {
                        match &ops[j] {
                            OpCode::QuantStartByName(_, _) => {
                                depth += 1;
                            }
                            OpCode::QuantEnd => {
                                if depth == 0 {
                                    break;
                                } else {
                                    depth -= 1;
                                }
                            }
                            _ => {}
                        }
                        j += 1;
                    }
                    if j >= ops.len() {
                        return Err(Error::not_implemented("unterminated quantifier body"));
                    }
                    let end_index = j; // ops[end_index] is QuantEnd
                    let body_slice = &ops[ip + 1..end_index];
                    // Cache body instructions once
                    let body_instr = InstrSeq(body_slice.to_vec());

                    // Clone context once; we'll derive per-iteration bindings from it
                    let shared_ctx = self.dyn_ctx.clone();
                    let mut quant_result = match kind {
                        QuantifierKind::Some => false,
                        QuantifierKind::Every => true,
                    };
                    for (idx, item) in input_seq.into_iter().enumerate() {
                        let iter_ctx = shared_ctx
                            .with_context_item(Some(item.clone()))
                            .with_variable(var_name.clone(), vec![item.clone()]);
                        let mut inner_vm = Vm::new(self.compiled, &iter_ctx);
                        inner_vm.frames.push(Frame {
                            last: item_count,
                            pos: idx + 1,
                        });
                        let body_res = inner_vm.run(&body_instr)?;
                        let truth = Self::ebv(&body_res)?;
                        match kind {
                            QuantifierKind::Some => {
                                if truth {
                                    quant_result = true;
                                    break;
                                }
                            }
                            QuantifierKind::Every => {
                                if !truth {
                                    quant_result = false;
                                    break;
                                }
                            }
                        }
                    }
                    // Advance ip to instruction after QuantEnd
                    ip = end_index + 1;
                    self.stack
                        .push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(quant_result))]);
                }
                OpCode::QuantEnd => {
                    ip += 1;
                }

                // Types
                OpCode::Cast(t) => {
                    let v = self.pop_seq();
                    self.stack.push(self.cast(v, t)?);
                    ip += 1;
                }
                OpCode::Castable(t) => {
                    // Lightweight castability check per XPath 2.0: does NOT raise dynamic errors, returns false instead.
                    let v = self.pop_seq();
                    let ok = self.is_castable(&v, t);
                    self.stack
                        .push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(ok))]);
                    ip += 1;
                }
                OpCode::Treat(t) => {
                    let v = self.pop_seq();
                    self.assert_treat(&v, t)?;
                    self.stack.push(v);
                    ip += 1;
                }
                OpCode::InstanceOf(t) => {
                    let v = self.pop_seq();
                    let b = self.instance_of(&v, t)?;
                    self.stack
                        .push(vec![XdmItem::Atomic(XdmAtomicValue::Boolean(b))]);
                    ip += 1;
                }

                // Functions
                OpCode::CallByName(name, argc) => {
                    let argc = *argc;
                    let mut args: Vec<XdmSequence<N>> = Vec::with_capacity(argc);
                    for _ in 0..argc {
                        args.push(self.pop_seq());
                    }
                    args.reverse();
                    let en = name; // lookup will resolve default namespace when needed
                    let def_ns = self
                        .compiled
                        .static_ctx
                        .default_function_namespace
                        .as_deref();
                    let f = match self.functions.resolve(en, argc, def_ns) {
                        Ok(f) => f,
                        Err(crate::engine::runtime::ResolveError::Unknown(resolved)) => {
                            return Err(Error::from_code(
                                ErrorCode::XPST0017,
                                format!("unknown function: {{{:?}}}#{argc}", resolved),
                            ));
                        }
                        Err(crate::engine::runtime::ResolveError::WrongArity {
                            name: resolved,
                            ..
                        }) => {
                            // Humanize the provided argument count for a clearer diagnostic
                            let arg_phrase = match argc {
                                0 => "no arguments".to_string(),
                                1 => "one argument".to_string(),
                                2 => "two arguments".to_string(),
                                3 => "three arguments".to_string(),
                                n => format!("{n} arguments"),
                            };
                            return Err(Error::from_code(
                                ErrorCode::XPST0017,
                                format!(
                                    "function {}() cannot be called with {}",
                                    resolved.local, arg_phrase
                                ),
                            ));
                        }
                    };
                    // Use cached default collation for this VM
                    let default_collation = self.default_collation.clone();
                    let call_ctx = CallCtx {
                        dyn_ctx: self.dyn_ctx,
                        static_ctx: &self.compiled.static_ctx,
                        default_collation,
                        regex: self.dyn_ctx.regex.clone(),
                    };
                    let result = (f)(&call_ctx, &args)?;
                    self.stack.push(result);
                    ip += 1;
                }

                // Errors
                OpCode::Raise(code) => {
                    // Interpret legacy raise codes; prefer enum when possible.
                    return Err(Error::new_qname(
                        Error::parse_code(code),
                        "raised by program",
                    ));
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
                _ => Err(Error::from_code(
                    ErrorCode::FORG0006,
                    "EBV for this atomic type not supported",
                )),
            },
            _ => Err(Error::from_code(
                ErrorCode::FORG0006,
                "effective boolean value of sequence of length > 1",
            )),
        }
    }

    // XPath 2.0 predicate semantics:
    // - If result is a number: keep node iff number == position()
    // - Else: use EBV of the result
    fn predicate_truth_value(
        result: &XdmSequence<N>,
        position: usize,
        _last: usize,
    ) -> Result<bool, Error> {
        if result.len() == 1
            && let XdmItem::Atomic(a) = &result[0]
            && let Some(num) = match a {
                XdmAtomicValue::Integer(i) => Some(*i as f64),
                XdmAtomicValue::Decimal(d) => Some(*d),
                XdmAtomicValue::Double(d) => Some(*d),
                XdmAtomicValue::Float(f) => Some(*f as f64),
                XdmAtomicValue::UntypedAtomic(s) => s.parse::<f64>().ok(),
                _ => None,
            }
        {
            // Numeric predicate: position match (NaN never matches)
            if num.is_nan() {
                return Ok(false);
            }
            return Ok((num - (position as f64)).abs() < f64::EPSILON);
        }
        // Fallback to EBV rules
        Self::ebv(result)
    }

    fn atomize(seq: XdmSequence<N>) -> XdmSequence<N> {
        let mut out = Vec::with_capacity(seq.len());
        for it in seq {
            match it {
                XdmItem::Atomic(a) => out.push(XdmItem::Atomic(a)),
                XdmItem::Node(n) => out.push(XdmItem::Atomic(XdmAtomicValue::UntypedAtomic(
                    n.string_value(),
                ))),
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
            XdmAtomicValue::Boolean(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            XdmAtomicValue::UntypedAtomic(s) | XdmAtomicValue::String(s) => {
                s.parse::<f64>().unwrap_or(f64::NAN)
            }
            _ => f64::NAN,
        })
    }

    fn compare_value(
        &self,
        lhs: &XdmSequence<N>,
        rhs: &XdmSequence<N>,
        op: ComparisonOp,
    ) -> Result<bool, Error> {
        let la = Self::atomize(lhs.clone());
        let ra = Self::atomize(rhs.clone());
        if la.len() != 1 || ra.len() != 1 {
            return Err(Error::from_code(
                ErrorCode::FORG0006,
                "value comparison requires singletons",
            ));
        }
        match (&la[0], &ra[0]) {
            (XdmItem::Atomic(a), XdmItem::Atomic(b)) => self.compare_atomic(a, b, op),
            _ => Ok(false),
        }
    }

    fn compare_atomic(
        &self,
        a: &XdmAtomicValue,
        b: &XdmAtomicValue,
        op: ComparisonOp,
    ) -> Result<bool, Error> {
        use ComparisonOp::*;
        // XPath 2.0 value comparison promotions (refined numeric path):
        // 1. untypedAtomic normalization (string or attempt numeric if other numeric)
        // 2. Numeric tower minimal promotion: integer + integer -> integer; integer + decimal -> decimal; decimal + float -> float;
        //    any + double -> double; float + float -> float; decimal + decimal -> decimal; integer + float -> float; etc.
        // 3. Boolean: only Eq/Ne allowed vs boolean; relational ops on booleans error
        // 4. String vs numeric relational is error (still simplified to FORG0006 here)
        use XdmAtomicValue as V;

        // Helper: determine unified numeric representation with minimal promotion.
        #[derive(Clone, Copy)]
        enum NumKind {
            Int(i64),
            Dec(f64),
            Float(f32),
            Double(f64),
        }
        impl NumKind {
            fn to_f64(self) -> f64 {
                match self {
                    NumKind::Int(i) => i as f64,
                    NumKind::Dec(d) => d,
                    NumKind::Float(f) => f as f64,
                    NumKind::Double(d) => d,
                }
            }
        }
        fn classify(v: &V) -> Option<NumKind> {
            match v {
                V::Integer(i) => Some(NumKind::Int(*i)),
                V::Decimal(d) => Some(NumKind::Dec(*d)),
                V::Float(f) => Some(NumKind::Float(*f)),
                V::Double(d) => Some(NumKind::Double(*d)),
                _ => None,
            }
        }
        fn unify_numeric(a: NumKind, b: NumKind) -> (NumKind, NumKind) {
            use NumKind::*;
            match (a, b) {
                (Double(x), y) => (Double(x), Double(y.to_f64())),
                (y, Double(x)) => (Double(y.to_f64()), Double(x)),
                (Float(x), Float(y)) => (Float(x), Float(y)),
                (Float(x), Int(y)) => (Float(x), Float(y as f32)),
                (Int(x), Float(y)) => (Float(x as f32), Float(y)),
                (Float(x), Dec(y)) => (Float(x), Float(y as f32)),
                (Dec(x), Float(y)) => (Float(x as f32), Float(y)),
                (Dec(x), Dec(y)) => (Dec(x), Dec(y)),
                (Dec(x), Int(y)) => (Dec(x), Dec(y as f64)),
                (Int(x), Dec(y)) => (Dec(x as f64), Dec(y)),
                (Int(x), Int(y)) => (Int(x), Int(y)),
            }
        }

        // Normalize untypedAtomic per context: if the counterpart is numeric attempt numeric cast (error on failure),
        // else treat both sides' untyped as string. untyped vs untyped -> both strings.
        let (a_norm, b_norm) = match (a, b) {
            (V::UntypedAtomic(sa), V::UntypedAtomic(sb)) => {
                (V::String(sa.clone()), V::String(sb.clone()))
            }
            (V::UntypedAtomic(s), other)
                if matches!(
                    other,
                    V::Integer(_) | V::Decimal(_) | V::Double(_) | V::Float(_)
                ) =>
            {
                let num = s.parse::<f64>().map_err(|_| {
                    Error::from_code(ErrorCode::FORG0001, "invalid numeric literal")
                })?;
                (V::Double(num), other.clone())
            }
            (other, V::UntypedAtomic(s))
                if matches!(
                    other,
                    V::Integer(_) | V::Decimal(_) | V::Double(_) | V::Float(_)
                ) =>
            {
                let num = s.parse::<f64>().map_err(|_| {
                    Error::from_code(ErrorCode::FORG0001, "invalid numeric literal")
                })?;
                (other.clone(), V::Double(num))
            }
            (V::UntypedAtomic(s), other) => (V::String(s.clone()), other.clone()),
            (other, V::UntypedAtomic(s)) => (other.clone(), V::String(s.clone())),
            _ => (a.clone(), b.clone()),
        };

        // Boolean handling
        if let (V::Boolean(x), V::Boolean(y)) = (&a_norm, &b_norm) {
            return Ok(match op {
                Eq => x == y,
                Ne => x != y,
                Lt | Le | Gt | Ge => {
                    return Err(Error::from_code(
                        ErrorCode::XPTY0004,
                        "relational op on boolean",
                    ));
                }
            });
        }

        // If both (after normalization) are strings and not numeric context
        if matches!((&a_norm, &b_norm), (V::String(_), V::String(_)))
            && matches!(op, Lt | Le | Gt | Ge | Eq | Ne)
        {
            let ls = if let V::String(s) = &a_norm {
                s
            } else {
                unreachable!()
            };
            let rs = if let V::String(s) = &b_norm {
                s
            } else {
                unreachable!()
            };
            // Collation-aware: use default collation (fallback to codepoint)
            let coll_arc;
            let coll: &dyn crate::engine::collation::Collation =
                if let Some(c) = &self.default_collation {
                    c.as_ref()
                } else {
                    coll_arc = self
                        .dyn_ctx
                        .collations
                        .get(crate::engine::collation::CODEPOINT_URI)
                        .expect("codepoint collation registered");
                    coll_arc.as_ref()
                };
            return Ok(match op {
                Eq => coll.key(ls) == coll.key(rs),
                Ne => coll.key(ls) != coll.key(rs),
                Lt => coll.compare(ls, rs).is_lt(),
                Le => {
                    let ord = coll.compare(ls, rs);
                    ord.is_lt() || ord.is_eq()
                }
                Gt => coll.compare(ls, rs).is_gt(),
                Ge => {
                    let ord = coll.compare(ls, rs);
                    ord.is_gt() || ord.is_eq()
                }
            });
        }

        // QName equality (only Eq/Ne permitted); compare namespace URI + local name; ignore prefix
        if let (
            XdmAtomicValue::QName {
                ns_uri: nsa,
                local: la,
                ..
            },
            XdmAtomicValue::QName {
                ns_uri: nsb,
                local: lb,
                ..
            },
        ) = (a, b)
        {
            return Ok(match op {
                Eq => nsa == nsb && la == lb,
                Ne => nsa != nsb || la != lb,
                Lt | Le | Gt | Ge => {
                    return Err(Error::from_code(
                        ErrorCode::XPTY0004,
                        "relational op on QName",
                    ));
                }
            });
        }

        // NOTATION equality (only Eq/Ne permitted); current engine treats NOTATION as lexical string
        if let (XdmAtomicValue::Notation(na), XdmAtomicValue::Notation(nb)) = (a, b) {
            return Ok(match op {
                Eq => na == nb,
                Ne => na != nb,
                Lt | Le | Gt | Ge => {
                    return Err(Error::from_code(
                        ErrorCode::XPTY0004,
                        "relational op on NOTATION",
                    ));
                }
            });
        }

        // Numeric path with minimal promotion
        if let (Some(ca), Some(cb)) = (classify(&a_norm), classify(&b_norm)) {
            let (ua, ub) = unify_numeric(ca, cb);
            let (ln, rn) = (ua.to_f64(), ub.to_f64());
            if ln.is_nan() || rn.is_nan() {
                return Ok(matches!(op, ComparisonOp::Ne));
            }
            return Ok(match op {
                Eq => ln == rn,
                Ne => ln != rn,
                Lt => ln < rn,
                Le => ln <= rn,
                Gt => ln > rn,
                Ge => ln >= rn,
            });
        }

        // dateTime relational comparisons by absolute instant
        if let (XdmAtomicValue::DateTime(da), XdmAtomicValue::DateTime(db)) = (a, b) {
            let (a_ts, b_ts) = (da.timestamp(), db.timestamp());
            let (a_ns, b_ns) = (da.timestamp_subsec_nanos(), db.timestamp_subsec_nanos());
            let ord = (a_ts, a_ns).cmp(&(b_ts, b_ns));
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }

        // duration comparisons (same family only)
        if let (XdmAtomicValue::YearMonthDuration(ma), XdmAtomicValue::YearMonthDuration(mb)) =
            (a, b)
        {
            let ord = ma.cmp(mb);
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }
        if let (XdmAtomicValue::DayTimeDuration(sa), XdmAtomicValue::DayTimeDuration(sb)) = (a, b) {
            let ord = sa.cmp(sb);
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }

        // date comparisons: normalize to midnight in effective timezone
        if let (
            XdmAtomicValue::Date { date: da, tz: ta },
            XdmAtomicValue::Date { date: db, tz: tb },
        ) = (a, b)
        {
            let eff_tz_a = (*ta).unwrap_or_else(|| self.implicit_timezone());
            let eff_tz_b = (*tb).unwrap_or_else(|| self.implicit_timezone());
            let na = da.and_time(ChronoNaiveTime::from_hms_opt(0, 0, 0).unwrap());
            let nb = db.and_time(ChronoNaiveTime::from_hms_opt(0, 0, 0).unwrap());
            let dta = eff_tz_a.from_local_datetime(&na).single().unwrap();
            let dtb = eff_tz_b.from_local_datetime(&nb).single().unwrap();
            let ord = (dta.timestamp(), dta.timestamp_subsec_nanos())
                .cmp(&(dtb.timestamp(), dtb.timestamp_subsec_nanos()));
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }

        // time comparisons: anchor to a fixed date and compare instants in effective timezone
        if let (
            XdmAtomicValue::Time { time: ta, tz: tza },
            XdmAtomicValue::Time { time: tb, tz: tzb },
        ) = (a, b)
        {
            let eff_tz_a = (*tza).unwrap_or_else(|| self.implicit_timezone());
            let eff_tz_b = (*tzb).unwrap_or_else(|| self.implicit_timezone());
            let base = chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap();
            let na = base.and_time(*ta);
            let nb = base.and_time(*tb);
            let dta = eff_tz_a.from_local_datetime(&na).single().unwrap();
            let dtb = eff_tz_b.from_local_datetime(&nb).single().unwrap();
            let ord = (dta.timestamp(), dta.timestamp_subsec_nanos())
                .cmp(&(dtb.timestamp(), dtb.timestamp_subsec_nanos()));
            return Ok(match op {
                Eq => ord == core::cmp::Ordering::Equal,
                Ne => ord != core::cmp::Ordering::Equal,
                Lt => ord == core::cmp::Ordering::Less,
                Le => ord != core::cmp::Ordering::Greater,
                Gt => ord == core::cmp::Ordering::Greater,
                Ge => ord != core::cmp::Ordering::Less,
            });
        }

        // Unsupported / incomparable type combination → type error (XPTY0004)
        Err(Error::from_code(
            ErrorCode::XPTY0004,
            "incomparable atomic types",
        ))
    }

    fn implicit_timezone(&self) -> ChronoFixedOffset {
        if let Some(tz) = self.dyn_ctx.timezone_override {
            return tz;
        }
        if let Some(n) = self.dyn_ctx.now {
            return *n.offset();
        }
        ChronoFixedOffset::east_opt(0).unwrap()
    }

    fn doc_order_distinct(&self, seq: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
        // For non-node items: return as-is; For nodes: sort+dedup by document order
        let mut nodes: Vec<N> = Vec::new();
        let mut others: Vec<XdmItem<N>> = Vec::new();
        for it in seq {
            match it {
                XdmItem::Node(n) => nodes.push(n),
                other => others.push(other),
            }
        }
        if nodes.is_empty() {
            return Ok(others);
        }
        if nodes.len() == 1 {
            others.push(XdmItem::Node(nodes.pop().unwrap()));
            return Ok(others);
        }
        let mut deduped: Vec<N> = Vec::with_capacity(nodes.len());
        let mut need_sort = false;
        let mut last = nodes[0].clone();
        deduped.push(last.clone());
        for node in nodes.iter().skip(1) {
            match self.node_compare(&last, node)? {
                Ordering::Less => {
                    deduped.push(node.clone());
                    last = node.clone();
                }
                Ordering::Equal => {
                    if last == *node {
                        // exact duplicate
                    } else {
                        need_sort = true;
                        break;
                    }
                }
                Ordering::Greater => {
                    need_sort = true;
                    break;
                }
            }
        }
        let mut out: XdmSequence<N> = others;
        if !need_sort {
            out.extend(deduped.into_iter().map(XdmItem::Node));
            return Ok(out);
        }
        nodes.sort_by(|a, b| self.node_compare(a, b).unwrap_or(Ordering::Equal));
        nodes.dedup();
        out.extend(nodes.into_iter().map(XdmItem::Node));
        Ok(out)
    }

    // (function name resolution for error messages is handled in FunctionRegistry::resolve)

    // (default_collation cached in Vm::new)

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
                while let Some(p) = cur_opt {
                    out.push(p.clone());
                    cur_opt = p.parent();
                }
                out
            }
            AxisIR::AncestorOrSelf => {
                let mut v = self.axis_iter(node.clone(), &AxisIR::Ancestor);
                v.insert(0, node);
                v
            }
            AxisIR::Descendant => self.collect_descendants(node, false),
            AxisIR::DescendantOrSelf => self.collect_descendants(node, true),
            AxisIR::FollowingSibling => self.siblings(node, false),
            AxisIR::PrecedingSibling => self.siblings(node, true),
            AxisIR::Following => {
                let mut out = Vec::new();
                let mut cursor = self.doc_successor(&node);
                while let Some(next) = cursor {
                    let next_cursor = self.doc_successor(&next);
                    if !Self::is_attr_or_namespace(&next) && !self.is_descendant_of(&next, &node) {
                        out.push(next.clone());
                    }
                    cursor = next_cursor;
                }
                out
            }
            AxisIR::Namespace => {
                // Namespace axis: namespaces in scope for the element node.
                // Walk self → ancestors collecting namespace nodes, keeping first binding per prefix.
                use crate::model::NodeKind;
                if !matches!(node.kind(), NodeKind::Element) {
                    return Vec::new();
                }
                let mut out: Vec<N> = Vec::new();
                let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
                let mut cur: Option<N> = Some(node);
                while let Some(n) = cur {
                    if matches!(n.kind(), NodeKind::Element) {
                        for ns in n.namespaces() {
                            if let Some(qn) = ns.name() {
                                let pfx = qn.prefix.unwrap_or_default();
                                if !seen.contains(&pfx) {
                                    seen.insert(pfx.clone());
                                    out.push(ns.clone());
                                }
                            }
                        }
                    }
                    cur = n.parent();
                }
                out
            }
            AxisIR::Preceding => {
                let mut out = Vec::new();
                let mut cursor = self.doc_predecessor(&node);
                while let Some(prev) = cursor {
                    let prev_cursor = self.doc_predecessor(&prev);
                    if !Self::is_attr_or_namespace(&prev) && !self.is_ancestor_of(&prev, &node) {
                        out.push(prev.clone());
                    }
                    cursor = prev_cursor;
                }
                out.reverse();
                out
            }
        }
    }
    fn collect_descendants(&self, node: N, include_self: bool) -> Vec<N> {
        let mut out = Vec::new();
        if include_self {
            out.push(node.clone());
        }
        let mut stack: Vec<N> = node
            .children()
            .into_iter()
            .filter(|child| !Self::is_attr_or_namespace(child))
            .collect();
        stack.reverse();
        while let Some(cur) = stack.pop() {
            out.push(cur.clone());
            let mut children: Vec<N> = cur
                .children()
                .into_iter()
                .filter(|child| !Self::is_attr_or_namespace(child))
                .collect();
            children.reverse();
            stack.extend(children);
        }
        out
    }
    fn doc_successor(&self, node: &N) -> Option<N> {
        if let Some(child) = Self::first_child_in_doc(node) {
            return Some(child);
        }
        let mut current = node.clone();
        while let Some(parent) = current.parent() {
            if let Some(sib) = Self::next_sibling_in_doc(&current) {
                return Some(sib);
            }
            current = parent;
        }
        None
    }
    fn doc_predecessor(&self, node: &N) -> Option<N> {
        if let Some(prev) = Self::prev_sibling_in_doc(node) {
            return Some(Self::last_descendant_in_doc(prev));
        }
        let mut current = node.clone();
        while let Some(parent) = current.parent() {
            if !Self::is_attr_or_namespace(&parent) {
                return Some(parent);
            }
            if let Some(prev) = Self::prev_sibling_in_doc(&parent) {
                return Some(Self::last_descendant_in_doc(prev));
            }
            current = parent;
        }
        None
    }
    fn first_child_in_doc(node: &N) -> Option<N> {
        for child in node.children() {
            if !Self::is_attr_or_namespace(&child) {
                return Some(child);
            }
        }
        None
    }
    fn next_sibling_in_doc(node: &N) -> Option<N> {
        let parent = node.parent()?;
        let siblings = parent.children();
        let mut found = false;
        for sib in siblings.iter() {
            if found && !Self::is_attr_or_namespace(sib) {
                return Some(sib.clone());
            }
            if sib == node {
                found = true;
            }
        }
        None
    }
    fn prev_sibling_in_doc(node: &N) -> Option<N> {
        let parent = node.parent()?;
        let siblings = parent.children();
        let mut prev: Option<N> = None;
        for sib in siblings {
            if sib == *node {
                break;
            }
            if !Self::is_attr_or_namespace(&sib) {
                prev = Some(sib);
            }
        }
        prev
    }
    fn last_descendant_in_doc(node: N) -> N {
        let mut current = node;
        loop {
            let children = current.children();
            let mut last_child: Option<N> = None;
            for child in children.into_iter().rev() {
                if !Self::is_attr_or_namespace(&child) {
                    last_child = Some(child);
                    break;
                }
            }
            if let Some(child) = last_child {
                current = child;
            } else {
                return current;
            }
        }
    }
    fn is_attr_or_namespace(node: &N) -> bool {
        matches!(node.kind(), NodeKind::Attribute | NodeKind::Namespace)
    }
    fn is_descendant_of(&self, node: &N, ancestor: &N) -> bool {
        let mut current = node.parent();
        while let Some(parent) = current {
            if parent == *ancestor {
                return true;
            }
            current = parent.parent();
        }
        false
    }
    fn is_ancestor_of(&self, node: &N, descendant: &N) -> bool {
        self.is_descendant_of(descendant, node)
    }
    fn siblings(&self, node: N, preceding: bool) -> Vec<N> {
        if let Some(parent) = node.parent() {
            let sibs = parent.children();
            // Find index of current node among element children
            let mut idx_opt: Option<usize> = None;
            for (i, s) in sibs.iter().enumerate() {
                if s == &node {
                    idx_opt = Some(i);
                    break;
                }
            }
            if let Some(idx) = idx_opt {
                if preceding {
                    // preceding-sibling: nodes before self, in document order (left to right)
                    sibs.into_iter().take(idx).collect()
                } else {
                    // following-sibling: nodes after self, in document order (left to right)
                    sibs.into_iter().skip(idx + 1).collect()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        }
    }
    #[allow(clippy::only_used_in_recursion)]
    fn node_test(&self, node: &N, test: &NodeTestIR) -> bool {
        use NodeTestIR::*;
        match test {
            AnyKind => true,
            Name(q) => {
                // For namespace nodes, the NameTest matches by prefix (local) only.
                if matches!(node.kind(), crate::model::NodeKind::Namespace) {
                    return node.name().map(|n| n.local == q.local).unwrap_or(false);
                }
                node.name()
                    .map(|n| n.local == q.local && q.ns_uri == n.ns_uri)
                    .unwrap_or(false)
            }
            WildcardAny => true,
            NsWildcard(ns) => node
                .name()
                .map(|n| {
                    // Effective namespace URI: prefer the QName's ns_uri if present; otherwise resolve
                    // using in-scope namespaces and the node's prefix (if any). The default namespace
                    // does not apply to attributes, but resolution here only happens when a prefix exists.
                    let eff = if let Some(uri) = n.ns_uri.clone() {
                        Some(uri)
                    } else if let Some(pref) = &n.prefix {
                        self.resolve_in_scope_prefix(node, pref)
                    } else {
                        None
                    };
                    eff.unwrap_or_default() == *ns
                })
                .unwrap_or(false),
            LocalWildcard(local) => node.name().map(|n| n.local == *local).unwrap_or(false),
            KindText => matches!(node.kind(), crate::model::NodeKind::Text),
            KindComment => matches!(node.kind(), crate::model::NodeKind::Comment),
            KindProcessingInstruction(target_opt) => {
                if !matches!(node.kind(), crate::model::NodeKind::ProcessingInstruction) {
                    return false;
                }
                if let Some(target) = target_opt {
                    if let Some(nm) = node.name() {
                        nm.local == *target
                    } else {
                        false
                    }
                } else {
                    true
                }
            }
            KindDocument(inner_opt) => {
                if !matches!(node.kind(), crate::model::NodeKind::Document) {
                    return false;
                }
                if let Some(inner) = inner_opt {
                    for c in node.children() {
                        if self.node_test(&c, inner) {
                            return true;
                        }
                    }
                    false
                } else {
                    true
                }
            }
            KindElement { name, .. } => {
                if !matches!(node.kind(), crate::model::NodeKind::Element) {
                    return false;
                }
                match name {
                    None => true,
                    Some(NameOrWildcard::Any) => true,
                    Some(NameOrWildcard::Name(exp)) => node
                        .name()
                        .map(|n| n.local == exp.local && n.ns_uri == exp.ns_uri)
                        .unwrap_or(false),
                }
            }
            KindAttribute { name, .. } => {
                if !matches!(node.kind(), crate::model::NodeKind::Attribute) {
                    return false;
                }
                match name {
                    None => true,
                    Some(NameOrWildcard::Any) => true,
                    Some(NameOrWildcard::Name(exp)) => node
                        .name()
                        .map(|n| n.local == exp.local && n.ns_uri == exp.ns_uri)
                        .unwrap_or(false),
                }
            }
            KindSchemaElement(_) | KindSchemaAttribute(_) => true, // simplified
        }
    }

    /// Resolve a namespace prefix to its in-scope namespace URI for the given node by walking
    /// up the ancestor chain and inspecting declared namespace nodes. Honors the implicit `xml`
    /// binding. Returns `None` when no binding is found.
    fn resolve_in_scope_prefix(&self, node: &N, prefix: &str) -> Option<String> {
        if prefix == "xml" {
            return Some(crate::consts::XML_URI.to_string());
        }
        use crate::model::NodeKind;
        let mut cur = Some(node.clone());
        while let Some(n) = cur {
            if matches!(n.kind(), NodeKind::Element) {
                for ns in n.namespaces() {
                    if let Some(q) = ns.name() {
                        let p = q.prefix.unwrap_or_default();
                        if p == prefix {
                            return Some(ns.string_value());
                        }
                    }
                }
            }
            cur = n.parent();
        }
        None
    }

    // ===== Set operations (nodes-only; results in document order with duplicates removed) =====
    fn set_union(&self, a: XdmSequence<N>, b: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
        let lhs = self.sorted_distinct_nodes(a)?;
        let rhs = self.sorted_distinct_nodes(b)?;
        let mut out: Vec<N> = Vec::with_capacity(lhs.len() + rhs.len());
        let mut i = 0usize;
        let mut j = 0usize;
        while i < lhs.len() && j < rhs.len() {
            match self.node_compare(&lhs[i], &rhs[j])? {
                Ordering::Less => {
                    out.push(lhs[i].clone());
                    i += 1;
                }
                Ordering::Greater => {
                    out.push(rhs[j].clone());
                    j += 1;
                }
                Ordering::Equal => {
                    out.push(lhs[i].clone());
                    i += 1;
                    j += 1;
                }
            }
        }
        if i < lhs.len() {
            out.extend(lhs[i..].iter().cloned());
        }
        if j < rhs.len() {
            out.extend(rhs[j..].iter().cloned());
        }
        Ok(out.into_iter().map(XdmItem::Node).collect())
    }
    fn set_intersect(&self, a: XdmSequence<N>, b: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
        let lhs = self.sorted_distinct_nodes(a)?;
        let rhs = self.sorted_distinct_nodes(b)?;
        let mut out: Vec<N> = Vec::new();
        let mut i = 0usize;
        let mut j = 0usize;
        while i < lhs.len() && j < rhs.len() {
            match self.node_compare(&lhs[i], &rhs[j])? {
                Ordering::Equal => {
                    out.push(lhs[i].clone());
                    i += 1;
                    j += 1;
                }
                Ordering::Less => i += 1,
                Ordering::Greater => j += 1,
            }
        }
        Ok(out.into_iter().map(XdmItem::Node).collect())
    }
    fn set_except(&self, a: XdmSequence<N>, b: XdmSequence<N>) -> Result<XdmSequence<N>, Error> {
        let lhs = self.sorted_distinct_nodes(a)?;
        let rhs = self.sorted_distinct_nodes(b)?;
        let mut out: Vec<N> = Vec::new();
        let mut i = 0usize;
        let mut j = 0usize;
        while i < lhs.len() {
            if j >= rhs.len() {
                out.extend(lhs[i..].iter().cloned());
                break;
            }
            match self.node_compare(&lhs[i], &rhs[j])? {
                Ordering::Equal => {
                    i += 1;
                    j += 1;
                }
                Ordering::Less => {
                    out.push(lhs[i].clone());
                    i += 1;
                }
                Ordering::Greater => j += 1,
            }
        }
        Ok(out.into_iter().map(XdmItem::Node).collect())
    }
    fn sorted_distinct_nodes(&self, seq: XdmSequence<N>) -> Result<Vec<N>, Error> {
        let ordered = self.doc_order_distinct(seq)?;
        ordered
            .into_iter()
            .map(|item| match item {
                XdmItem::Node(n) => Ok(n),
                _ => Err(Error::not_implemented(
                    "non-node item encountered in set operation",
                )),
            })
            .collect()
    }
    fn node_compare(&self, a: &N, b: &N) -> Result<Ordering, Error> {
        match (a.doc_order_key(), b.doc_order_key()) {
            (Some(ak), Some(bk)) => Ok(ak.cmp(&bk)),
            _ => a.compare_document_order(b),
        }
    }

    // ===== Type operations (very small subset) =====
    fn cast(&self, seq: XdmSequence<N>, t: &SingleTypeIR) -> Result<XdmSequence<N>, Error> {
        if seq.len() > 1 {
            return Err(Error::from_code(ErrorCode::XPTY0004, "cast of multi-item"));
        }
        if seq.is_empty() {
            if t.optional {
                return Ok(Vec::new());
            } else {
                return Err(Error::from_code(ErrorCode::XPST0003, "empty not allowed"));
            }
        }
        let item = seq[0].clone();
        let val = match item {
            XdmItem::Atomic(a) => a,
            XdmItem::Node(n) => XdmAtomicValue::UntypedAtomic(n.string_value()),
        };
        let casted = self.cast_atomic(val, &t.atomic)?;
        Ok(vec![XdmItem::Atomic(casted)])
    }
    fn cast_atomic(
        &self,
        a: XdmAtomicValue,
        target: &ExpandedName,
    ) -> Result<XdmAtomicValue, Error> {
        let local = &target.local;
        // NOTE: Namespace of target currently ignored (assumes xs:*); extend when QName type system expanded.
        match local.as_str() {
            // xs:string
            "string" => Ok(match a {
                XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => {
                    XdmAtomicValue::String(s)
                }
                other => XdmAtomicValue::String(self.atomic_to_string(&other)),
            }),
            // Boolean per XPath 2.0 casting rules (non-empty string except "0" and "false" => true)
            "boolean" => {
                let b = match a {
                    XdmAtomicValue::Boolean(b) => b,
                    XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => {
                        match s.as_str() {
                            "true" => true,
                            "1" => true,
                            "false" => false,
                            "0" => false,
                            _ => {
                                return Err(Error::from_code(
                                    ErrorCode::FORG0001,
                                    "invalid boolean lexical form",
                                ));
                            }
                        }
                    }
                    XdmAtomicValue::Integer(i) => i != 0,
                    XdmAtomicValue::Decimal(d) => d != 0.0,
                    XdmAtomicValue::Double(d) => d != 0.0 && !d.is_nan(),
                    XdmAtomicValue::Float(f) => f != 0.0 && !f.is_nan(),
                    other => {
                        return Err(Error::from_code(
                            ErrorCode::FORG0001,
                            format!("cannot cast {:?} to boolean", other),
                        ));
                    }
                };
                Ok(XdmAtomicValue::Boolean(b))
            }
            // Integer family collapse to xs:integer (subset of numeric tower for now)
            "integer" => {
                let s = match a {
                    XdmAtomicValue::Integer(i) => return Ok(XdmAtomicValue::Integer(i)),
                    XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => s,
                    XdmAtomicValue::Decimal(d) => {
                        if d.fract() == 0.0 {
                            return Ok(XdmAtomicValue::Integer(d as i64));
                        } else {
                            return Err(Error::from_code(
                                ErrorCode::FOCA0001,
                                "fractional part in integer cast",
                            ));
                        }
                    }
                    XdmAtomicValue::Double(d) => {
                        if d.fract() == 0.0 && d.is_finite() {
                            return Ok(XdmAtomicValue::Integer(d as i64));
                        } else {
                            return Err(Error::from_code(
                                ErrorCode::FOCA0001,
                                "non-integer double for integer cast",
                            ));
                        }
                    }
                    XdmAtomicValue::Float(f) => {
                        if f.fract() == 0.0 && f.is_finite() {
                            return Ok(XdmAtomicValue::Integer(f as i64));
                        } else {
                            return Err(Error::from_code(
                                ErrorCode::FOCA0001,
                                "non-integer float for integer cast",
                            ));
                        }
                    }
                    other => self.atomic_to_string(&other),
                };
                s.parse::<i64>().map(XdmAtomicValue::Integer).map_err(|_| {
                    Error::from_code(ErrorCode::FORG0001, "invalid integer lexical form")
                })
            }
            // decimal
            "decimal" => {
                let v = match a {
                    XdmAtomicValue::Decimal(d) => return Ok(XdmAtomicValue::Decimal(d)),
                    XdmAtomicValue::Integer(i) => i as f64,
                    XdmAtomicValue::Double(d) => {
                        if d.is_finite() {
                            d
                        } else {
                            return Err(Error::from_code(
                                ErrorCode::FOCA0001,
                                "INF/NaN to decimal",
                            ));
                        }
                    }
                    XdmAtomicValue::Float(f) => {
                        if f.is_finite() {
                            f as f64
                        } else {
                            return Err(Error::from_code(
                                ErrorCode::FOCA0001,
                                "INF/NaN to decimal",
                            ));
                        }
                    }
                    XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => s
                        .parse::<f64>()
                        .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid decimal"))?,
                    other => {
                        return Err(Error::from_code(
                            ErrorCode::FORG0001,
                            format!("cannot cast {:?} to decimal", other),
                        ));
                    }
                };
                Ok(XdmAtomicValue::Decimal(v))
            }
            // float / double
            "float" => {
                let f = self
                    .to_f64(&a)
                    .ok_or_else(|| Error::from_code(ErrorCode::FORG0001, "non-numeric to float"))?
                    as f32;
                Ok(XdmAtomicValue::Float(f))
            }
            "double" => {
                let f = self.to_f64(&a).ok_or_else(|| {
                    Error::from_code(ErrorCode::FORG0001, "non-numeric to double")
                })?;
                Ok(XdmAtomicValue::Double(f))
            }
            // anyURI: whitespace collapse only (simple form)
            "anyURI" => {
                let s = match a {
                    XdmAtomicValue::AnyUri(u) => u,
                    XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => {
                        s.trim().to_string()
                    }
                    other => self.atomic_to_string(&other),
                };
                // Project policy update: allow empty anyURI after whitespace collapse
                Ok(XdmAtomicValue::AnyUri(s))
            }
            // QName lexical form (prefix:local or local); namespace resolution not attempted here for xs:QName(string)
            "QName" => {
                let lex = match a {
                    XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => s,
                    other => self.atomic_to_string(&other),
                };
                if lex.is_empty() {
                    return Err(Error::from_code(ErrorCode::FORG0001, "empty QName"));
                }
                let mut parts = lex.split(':');
                let first = parts.next().unwrap();
                let second = parts.next();
                if let Some(local) = second {
                    if first.is_empty() || local.is_empty() {
                        return Err(Error::from_code(
                            ErrorCode::FORG0001,
                            "invalid QName lexical",
                        ));
                    }
                    // No static prefix resolution in pure cast (spec: QName constructor does resolve? kept simple)
                    Ok(XdmAtomicValue::QName {
                        ns_uri: None,
                        prefix: Some(first.to_string()),
                        local: local.to_string(),
                    })
                } else {
                    // unprefixed
                    Ok(XdmAtomicValue::QName {
                        ns_uri: None,
                        prefix: None,
                        local: first.to_string(),
                    })
                }
            }
            // date / time (naive parsing subset)
            "date" => {
                let s = match a {
                    XdmAtomicValue::Date { date, tz } => {
                        return Ok(XdmAtomicValue::Date { date, tz });
                    }
                    XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => s,
                    _ => self.atomic_to_string(&a),
                };
                match self.parse_date(&s) {
                    Ok(v) => Ok(v),
                    Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid date")),
                }
            }
            "dateTime" => {
                let s = match a {
                    XdmAtomicValue::DateTime(dt) => return Ok(XdmAtomicValue::DateTime(dt)),
                    XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => s,
                    _ => self.atomic_to_string(&a),
                };
                match self.parse_date_time(&s) {
                    Ok(v) => Ok(v),
                    Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid dateTime")),
                }
            }
            "time" => {
                let s = match a {
                    XdmAtomicValue::Time { time, tz } => {
                        return Ok(XdmAtomicValue::Time { time, tz });
                    }
                    XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => s,
                    _ => self.atomic_to_string(&a),
                };
                match self.parse_time(&s) {
                    Ok(v) => Ok(v),
                    Err(_) => Err(Error::from_code(ErrorCode::FORG0001, "invalid time")),
                }
            }
            "yearMonthDuration" => match a {
                XdmAtomicValue::YearMonthDuration(m) => Ok(XdmAtomicValue::YearMonthDuration(m)),
                XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => {
                    self.parse_year_month_duration(&s).map_err(|_| {
                        Error::from_code(ErrorCode::FORG0001, "invalid yearMonthDuration")
                    })
                }
                _ => Err(Error::from_code(
                    ErrorCode::FORG0001,
                    "cannot cast to yearMonthDuration",
                )),
            },
            "dayTimeDuration" => match a {
                XdmAtomicValue::DayTimeDuration(m) => Ok(XdmAtomicValue::DayTimeDuration(m)),
                XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => self
                    .parse_day_time_duration(&s)
                    .map_err(|_| Error::from_code(ErrorCode::FORG0001, "invalid dayTimeDuration")),
                _ => Err(Error::from_code(
                    ErrorCode::FORG0001,
                    "cannot cast to dayTimeDuration",
                )),
            },
            _ => Err(Error::not_implemented("cast target type")),
        }
    }
    // Spec-compliant lightweight castability check: returns true iff a cast would succeed.
    fn is_castable(&self, seq: &XdmSequence<N>, t: &SingleTypeIR) -> bool {
        // Cardinality: empty sequence is castable only if optional
        if seq.is_empty() {
            return t.optional;
        }
        if seq.len() > 1 {
            return false;
        }
        // Obtain atomic (atomization semantics simplified; node => untypedAtomic)
        let item = &seq[0];
        let atomic = match item {
            XdmItem::Atomic(a) => a.clone(),
            XdmItem::Node(n) => XdmAtomicValue::UntypedAtomic(n.string_value()),
        };
        // Fast-path for QName to ensure prefix resolution requirement similar to constructor semantics.
        if t.atomic.local == "QName"
            && let XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) = &atomic
        {
            if let Some(idx) = s.find(':') {
                let p = &s[..idx];
                if p.is_empty() {
                    return false;
                }
                if p == "xml" {
                } else {
                    // look up prefix in static context
                    if !self
                        .compiled
                        .static_ctx
                        .namespaces
                        .by_prefix
                        .contains_key(p)
                    {
                        return false;
                    }
                }
                // local part must exist
                if idx == s.len() - 1 {
                    return false;
                }
            } else if s.is_empty() {
                return false;
            }
        }
        self.cast_atomic(atomic, &t.atomic).is_ok()
    }
    // Helper: best-effort canonical string form for debugging / fallback casts
    fn atomic_to_string(&self, a: &XdmAtomicValue) -> String {
        format!("{:?}", a)
    }
    fn to_f64(&self, a: &XdmAtomicValue) -> Option<f64> {
        match a {
            XdmAtomicValue::Integer(i) => Some(*i as f64),
            XdmAtomicValue::Decimal(d) => Some(*d),
            XdmAtomicValue::Double(d) => Some(*d),
            XdmAtomicValue::Float(f) => Some(*f as f64),
            XdmAtomicValue::String(s) | XdmAtomicValue::UntypedAtomic(s) => s.parse::<f64>().ok(),
            _ => None,
        }
    }
    fn parse_date(&self, s: &str) -> Result<XdmAtomicValue, crate::util::temporal::TemporalErr> {
        let (d, tz) = crate::util::temporal::parse_date_lex(s)?;
        Ok(XdmAtomicValue::Date { date: d, tz })
    }
    fn parse_time(&self, s: &str) -> Result<XdmAtomicValue, crate::util::temporal::TemporalErr> {
        let (t, tz) = crate::util::temporal::parse_time_lex(s)?;
        Ok(XdmAtomicValue::Time { time: t, tz })
    }
    fn parse_date_time(
        &self,
        s: &str,
    ) -> Result<XdmAtomicValue, crate::util::temporal::TemporalErr> {
        let (d, t, tz) = crate::util::temporal::parse_date_time_lex(s)?;
        let dt = crate::util::temporal::build_naive_datetime(d, t, tz);
        Ok(XdmAtomicValue::DateTime(dt))
    }
    fn parse_year_month_duration(&self, s: &str) -> Result<XdmAtomicValue, ()> {
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
    fn parse_day_time_duration(&self, s: &str) -> Result<XdmAtomicValue, ()> {
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
    fn assert_treat(&self, seq: &XdmSequence<N>, t: &SeqTypeIR) -> Result<(), Error> {
        // Spec oriented: produce differentiated diagnostics while keeping XPTY0004 as error code.
        use crate::compiler::ir::{OccurrenceIR, SeqTypeIR};
        let (need_min, need_max, item_type) = match t {
            SeqTypeIR::EmptySequence => {
                if !seq.is_empty() {
                    return Err(Error::from_code(
                        ErrorCode::XPTY0004,
                        "treat as empty-sequence() failed: cardinality mismatch (expected 0 got >0)",
                    ));
                }
                return Ok(());
            }
            SeqTypeIR::Typed { item, occ } => {
                let (min, max) = match occ {
                    OccurrenceIR::One => (1, Some(1)),
                    OccurrenceIR::ZeroOrOne => (0, Some(1)),
                    OccurrenceIR::ZeroOrMore => (0, None),
                    OccurrenceIR::OneOrMore => (1, None),
                };
                (min, max, item)
            }
        };
        let actual = seq.len();
        if actual < need_min {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                format!(
                    "treat as failed: cardinality mismatch (expected min {} got {})",
                    need_min, actual
                ),
            ));
        }
        if let Some(max) = need_max
            && actual > max
        {
            return Err(Error::from_code(
                ErrorCode::XPTY0004,
                format!(
                    "treat as failed: cardinality mismatch (expected max {} got {})",
                    max, actual
                ),
            ));
        }
        for it in seq {
            if !self.item_matches_type(it, item_type)? {
                return Err(Error::from_code(
                    ErrorCode::XPTY0004,
                    "treat as failed: type mismatch",
                ));
            }
        }
        Ok(())
    }
    fn instance_of(&self, seq: &XdmSequence<N>, t: &SeqTypeIR) -> Result<bool, Error> {
        use crate::compiler::ir::{OccurrenceIR, SeqTypeIR};
        match t {
            SeqTypeIR::EmptySequence => Ok(seq.is_empty()),
            SeqTypeIR::Typed { item, occ } => {
                let ok_card = match occ {
                    OccurrenceIR::One => seq.len() == 1,
                    OccurrenceIR::ZeroOrOne => seq.len() <= 1,
                    OccurrenceIR::ZeroOrMore => true,
                    OccurrenceIR::OneOrMore => !seq.is_empty(),
                };
                if !ok_card {
                    return Ok(false);
                }
                for it in seq {
                    if !self.item_matches_type(it, item)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
        }
    }
    fn item_matches_type(
        &self,
        item: &XdmItem<N>,
        t: &crate::compiler::ir::ItemTypeIR,
    ) -> Result<bool, Error> {
        use crate::compiler::ir::ItemTypeIR;
        use XdmItem::*;
        match (item, t) {
            (_, ItemTypeIR::AnyItem) => Ok(true),
            (Node(_), ItemTypeIR::AnyNode) => Ok(true),
            (Atomic(_), ItemTypeIR::AnyNode) => Ok(false),
            (Node(n), ItemTypeIR::Kind(k)) => Ok(self.node_test(n, &k.clone())), // reuse existing node_test via IR NodeTestIR
            (Atomic(a), ItemTypeIR::Atomic(exp)) => Ok(self.atomic_matches_name(a, exp)),
            (Atomic(_), ItemTypeIR::Kind(_)) => Ok(false),
            (Node(_), ItemTypeIR::Atomic(_)) => Ok(false),
        }
    }
    fn atomic_matches_name(&self, a: &XdmAtomicValue, exp: &crate::xdm::ExpandedName) -> bool {
        use XdmAtomicValue::*;
        // Only recognize XML Schema built-ins (xs:*). Unknown namespaces do not match.
        let xs_ns = crate::consts::XS;
        if let Some(ns) = &exp.ns_uri
            && ns.as_str() != xs_ns
        {
            return false;
        }
        match exp.local.as_str() {
            // Supertype
            "anyAtomicType" => true,

            // Primitives
            "string" => matches!(
                a,
                String(_)
                    | NormalizedString(_)
                    | Token(_)
                    | Language(_)
                    | Name(_)
                    | NCName(_)
                    | NMTOKEN(_)
                    | Id(_)
                    | IdRef(_)
                    | Entity(_)
            ),
            "boolean" => matches!(a, Boolean(_)),
            "decimal" => matches!(
                a,
                Decimal(_)
                    | Integer(_)
                    | Long(_)
                    | Int(_)
                    | Short(_)
                    | Byte(_)
                    | UnsignedLong(_)
                    | UnsignedInt(_)
                    | UnsignedShort(_)
                    | UnsignedByte(_)
                    | NonPositiveInteger(_)
                    | NegativeInteger(_)
                    | NonNegativeInteger(_)
                    | PositiveInteger(_)
            ),
            "integer" => matches!(
                a,
                Integer(_)
                    | Long(_)
                    | Int(_)
                    | Short(_)
                    | Byte(_)
                    | UnsignedLong(_)
                    | UnsignedInt(_)
                    | UnsignedShort(_)
                    | UnsignedByte(_)
                    | NonPositiveInteger(_)
                    | NegativeInteger(_)
                    | NonNegativeInteger(_)
                    | PositiveInteger(_)
            ),
            "double" => matches!(a, Double(_)),
            "float" => matches!(a, Float(_)),
            "anyURI" => matches!(a, AnyUri(_)),
            "QName" => matches!(a, QName { .. }),
            "NOTATION" => matches!(a, Notation(_)),

            // Untyped
            "untypedAtomic" => matches!(a, UntypedAtomic(_)),

            // Binary
            "base64Binary" => matches!(a, Base64Binary(_)),
            "hexBinary" => matches!(a, HexBinary(_)),

            // Temporal and durations
            "dateTime" => matches!(a, DateTime(_)),
            "date" => matches!(a, Date { .. }),
            "time" => matches!(a, Time { .. }),
            "yearMonthDuration" => matches!(a, YearMonthDuration(_)),
            "dayTimeDuration" => matches!(a, DayTimeDuration(_)),

            // String-derived specifics (covered by string above but allow exact tests)
            "normalizedString" => matches!(a, NormalizedString(_)),
            "token" => matches!(a, Token(_)),
            "language" => matches!(a, Language(_)),
            "Name" => matches!(a, Name(_)),
            "NCName" => matches!(a, NCName(_)),
            "NMTOKEN" => matches!(a, NMTOKEN(_)),
            "ID" => matches!(a, Id(_)),
            "IDREF" => matches!(a, IdRef(_)),
            "ENTITY" => matches!(a, Entity(_)),

            // Unknown atomic type name -> no match
            _ => false,
        }
    }
}

// (removed IterState; for-expr now handled entirely within ForStartByName execution)
