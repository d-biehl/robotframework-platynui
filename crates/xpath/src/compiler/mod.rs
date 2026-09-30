use crate::engine::runtime::ErrorCode;
use crate::engine::runtime::{Error, StaticContext};
use crate::parser::{ast, parse};
use crate::xdm::{ExpandedName, XdmAtomicValue};
use smallvec::SmallVec;

pub mod ir;
pub mod optimizer;

use std::sync::{Arc, OnceLock};
use string_cache::DefaultAtom;

static DEFAULT_STATIC_CONTEXT: OnceLock<StaticContext> = OnceLock::new();

fn default_static_ctx() -> &'static StaticContext {
    DEFAULT_STATIC_CONTEXT.get_or_init(StaticContext::default)
}

/// Compile using a lazily initialized default `StaticContext`
///
/// # Errors
///
/// Returns an `XPST0003` error if `expr` is not syntactically valid or uses a construct the static
/// context rules out (a schema-aware type test, or the context item while it is statically typed as
/// `empty-sequence()`), `XPST0008` for an undeclared variable, `XPST0017` for an unknown function or
/// a call with an unsupported arity, and `FOER0000` if the compile cache lock is poisoned.
pub fn compile(expr: &str) -> Result<ir::CompiledXPath, Error> {
    compile_inner(expr, default_static_ctx())
}

/// Compile with an explicitly provided `StaticContext`
///
/// # Errors
///
/// Returns an `XPST0003` error if `expr` is not syntactically valid or uses a construct the static
/// context rules out (a schema-aware type test, or the context item while it is statically typed as
/// `empty-sequence()`), `XPST0008` for an undeclared variable, `XPST0017` for an unknown function or
/// a call with an unsupported arity, and `FOER0000` if the compile cache lock is poisoned.
pub fn compile_with_context(expr: &str, static_ctx: &StaticContext) -> Result<ir::CompiledXPath, Error> {
    compile_inner(expr, static_ctx)
}

/// Backing implementation shared by all compile entrypoints
fn compile_inner(expr: &str, static_ctx: &StaticContext) -> Result<ir::CompiledXPath, Error> {
    if let Some(instrs) = static_ctx
        .compile_cache
        .lock()
        .map_err(|_| Error::from_code(ErrorCode::FOER0000, "compile cache lock poisoned"))?
        .get(expr)
        .cloned()
    {
        return Ok(ir::CompiledXPath {
            instrs: (*instrs).clone(),
            static_ctx: Arc::new(static_ctx.clone()),
            source: expr.to_string(),
        });
    }

    let ast = parse(expr)?;
    let mut c = Compiler::new(static_ctx, expr);
    c.lower_expr(&ast)?;
    let instrs = optimizer::optimize(ir::InstrSeq(c.code));
    let source = expr.to_string();
    let cache_entry = Arc::new(instrs);

    static_ctx
        .compile_cache
        .lock()
        .map_err(|_| Error::from_code(ErrorCode::FOER0000, "compile cache lock poisoned"))?
        .put(source.clone(), Arc::clone(&cache_entry));

    Ok(ir::CompiledXPath {
        instrs: Arc::unwrap_or_clone(cache_entry),
        static_ctx: Arc::new(static_ctx.clone()),
        source,
    })
}

struct Compiler<'a> {
    static_ctx: &'a StaticContext,
    source: &'a str,
    code: Vec<ir::OpCode>,
    lexical_scopes: SmallVec<[SmallVec<[ExpandedName; 4]>; 8]>,
    // (no streaming hint flags; keep compiler simple and correct)
}

type CResult<T> = Result<T, Error>;

impl<'a> Compiler<'a> {
    fn new(static_ctx: &'a StaticContext, source: &'a str) -> Self {
        let reserve = std::cmp::max(32, source.len() / 2);
        Self { static_ctx, source, code: Vec::with_capacity(reserve), lexical_scopes: SmallVec::new() }
    }

    fn fork(&self) -> Self {
        Self {
            static_ctx: self.static_ctx,
            source: self.source,
            code: Vec::with_capacity(self.code.capacity()),
            lexical_scopes: self.lexical_scopes.clone(),
        }
    }

    fn emit(&mut self, op: ir::OpCode) {
        self.code.push(op);
    }

    fn load_context_item(&mut self, usage: &str) -> CResult<()> {
        if let Some(t) = &self.static_ctx.context_item_type
            && matches!(t, ir::SeqTypeIR::EmptySequence)
        {
            return Err(Error::from_code(
                ErrorCode::XPST0003,
                format!("context item is statically typed as empty-sequence(); cannot use {usage}"),
            ));
        }
        self.emit(ir::OpCode::LoadContextItem);
        if let Some(t) = &self.static_ctx.context_item_type {
            self.emit(ir::OpCode::Treat(t.clone()));
        }
        Ok(())
    }

    fn push_scope(&mut self) {
        self.lexical_scopes.push(SmallVec::new());
    }

    fn pop_scope(&mut self) {
        self.lexical_scopes.pop();
    }

    fn declare_local(&mut self, name: ExpandedName) {
        if self.lexical_scopes.is_empty() {
            self.lexical_scopes.push(SmallVec::new());
        }
        if let Some(scope) = self.lexical_scopes.last_mut() {
            scope.push(name);
        }
    }

    fn var_in_scope(&self, name: &ExpandedName) -> bool {
        self.lexical_scopes.iter().rev().any(|scope| scope.iter().any(|n| n == name))
            || self.static_ctx.in_scope_variables.contains(name)
    }

    // The central lowering dispatch: one arm per expression kind.
    #[allow(clippy::too_many_lines)]
    fn lower_expr(&mut self, e: &ast::Expr) -> CResult<()> {
        use ast::Expr as E;
        match e {
            E::Literal(l) => {
                self.lower_literal(l);
                Ok(())
            }
            E::Parenthesized(inner) => self.lower_expr(inner),
            E::VarRef(q) => {
                let en = self.to_expanded(q);
                if !self.var_in_scope(&en) {
                    return Err(Error::from_code(
                        ErrorCode::XPST0008,
                        format!("Variable ${en} is not declared in the static context"),
                    ));
                }
                self.emit(ir::OpCode::LoadVarByName(en));
                Ok(())
            }
            E::FunctionCall { name, args } => {
                // Special-case position() and last() as opcodes (zero-arg, default fn namespace or none)
                if args.is_empty()
                    && (name.local == "position" || name.local == "last")
                    && (name.ns_uri.is_none() || name.ns_uri.as_deref() == Some(crate::consts::FNS))
                {
                    self.emit(match name.local.as_str() {
                        "position" => ir::OpCode::Position,
                        _ => ir::OpCode::Last,
                    });
                    return Ok(());
                }
                let en = self.to_expanded(name);
                self.ensure_function_available(name, args.len())?;
                let param_specs = self
                    .static_ctx
                    .function_signatures
                    .param_types_for_call(&en, args.len(), self.static_ctx.default_function_namespace.as_deref())
                    .map(<[crate::engine::runtime::ParamTypeSpec]>::to_vec);
                for (idx, a) in args.iter().enumerate() {
                    self.lower_expr(a)?;
                    if let Some(specs) = &param_specs
                        && specs.get(idx).is_some_and(super::engine::runtime::ParamTypeSpec::requires_atomization)
                    {
                        self.emit(ir::OpCode::Atomize);
                    }
                }
                self.emit(ir::OpCode::CallByName(en, args.len()));
                Ok(())
            }
            E::Filter { input, predicates } => {
                self.lower_expr(input)?;
                let pred_ir = self.lower_predicates(predicates)?;
                self.emit(ir::OpCode::ApplyPredicates(pred_ir));
                Ok(())
            }
            E::Sequence(items) => {
                for it in items {
                    self.lower_expr(it)?;
                }
                self.emit(ir::OpCode::MakeSeq(items.len()));
                Ok(())
            }
            E::Binary { left, op, right } => {
                use ast::BinaryOp::{Add, And, Div, IDiv, Mod, Mul, Or, Sub};

                // Special handling for And/Or to enable short-circuit evaluation
                match op {
                    And => {
                        // Pattern for And with short-circuit:
                        // 1. Evaluate LHS
                        // 2. JumpIfFalse to skip RHS → if false, we need to push false
                        // 3. Evaluate RHS (LHS was true, result = EBV(RHS))
                        //
                        // Code: LHS JumpIfFalse(skip) Pop RHS ToEBV Jump(end) skip: Pop PushFalse end:

                        self.lower_expr(left)?; // Stack: [LHS]

                        let jump_if_false_pos = self.code.len();
                        self.emit(ir::OpCode::JumpIfFalse(0)); // Pops LHS, jumps if false

                        // LHS was true, evaluate RHS
                        self.lower_expr(right)?; // Stack: [RHS]
                        self.emit(ir::OpCode::ToEBV); // Convert RHS to boolean

                        let jump_to_end_pos = self.code.len();
                        self.emit(ir::OpCode::Jump(0)); // Jump over the false-push

                        // If we jumped here, LHS was false
                        let false_label = self.code.len();
                        self.emit(ir::OpCode::PushAtomic(XdmAtomicValue::Boolean(false)));

                        let end_label = self.code.len();

                        // Patch jumps
                        let jump_if_false_offset = false_label - jump_if_false_pos - 1;
                        self.code[jump_if_false_pos] = ir::OpCode::JumpIfFalse(jump_if_false_offset);

                        let jump_to_end_offset = end_label - jump_to_end_pos - 1;
                        self.code[jump_to_end_pos] = ir::OpCode::Jump(jump_to_end_offset);
                    }
                    Or => {
                        // Pattern for Or with short-circuit:
                        // LHS JumpIfTrue(skip) Pop RHS ToEBV Jump(end) skip: Pop PushTrue end:

                        self.lower_expr(left)?; // Stack: [LHS]

                        let jump_if_true_pos = self.code.len();
                        self.emit(ir::OpCode::JumpIfTrue(0)); // Pops LHS, jumps if true

                        // LHS was false, evaluate RHS
                        self.lower_expr(right)?; // Stack: [RHS]
                        self.emit(ir::OpCode::ToEBV); // Convert RHS to boolean

                        let jump_to_end_pos = self.code.len();
                        self.emit(ir::OpCode::Jump(0)); // Jump over the true-push

                        // If we jumped here, LHS was true
                        let true_label = self.code.len();
                        self.emit(ir::OpCode::PushAtomic(XdmAtomicValue::Boolean(true)));

                        let end_label = self.code.len();

                        // Patch jumps
                        let jump_if_true_offset = true_label - jump_if_true_pos - 1;
                        self.code[jump_if_true_pos] = ir::OpCode::JumpIfTrue(jump_if_true_offset);

                        let jump_to_end_offset = end_label - jump_to_end_pos - 1;
                        self.code[jump_to_end_pos] = ir::OpCode::Jump(jump_to_end_offset);
                    }
                    _ => {
                        // All other binary ops: evaluate both sides first
                        self.lower_expr(left)?;
                        self.lower_expr(right)?;
                        self.emit(match op {
                            Add => ir::OpCode::Add,
                            Sub => ir::OpCode::Sub,
                            Mul => ir::OpCode::Mul,
                            Div => ir::OpCode::Div,
                            IDiv => ir::OpCode::IDiv,
                            Mod => ir::OpCode::Mod,
                            And | Or => unreachable!("Handled above"),
                        });
                    }
                }
                Ok(())
            }
            E::GeneralComparison { left, op, right } => {
                self.lower_expr(left)?;
                self.lower_expr(right)?;
                self.emit(ir::OpCode::CompareGeneral(op.into()));
                Ok(())
            }
            E::ValueComparison { left, op, right } => {
                self.lower_expr(left)?;
                self.lower_expr(right)?;
                self.emit(ir::OpCode::CompareValue(op.into()));
                Ok(())
            }
            E::NodeComparison { left, op, right } => {
                use ast::NodeComp::{Follows, Is, Precedes};
                self.lower_expr(left)?;
                self.lower_expr(right)?;
                self.emit(match op {
                    Is => ir::OpCode::NodeIs,
                    Precedes => ir::OpCode::NodeBefore,
                    Follows => ir::OpCode::NodeAfter,
                });
                Ok(())
            }
            E::Unary { sign, expr } => {
                // compile as 0 +/- expr to reuse binary ops
                match sign {
                    ast::UnarySign::Plus => self.lower_expr(expr)?,
                    ast::UnarySign::Minus => {
                        self.emit(ir::OpCode::PushAtomic(XdmAtomicValue::Integer(0)));
                        self.lower_expr(expr)?;
                        self.emit(ir::OpCode::Sub);
                    }
                }
                Ok(())
            }
            E::IfThenElse { cond, then_expr, else_expr } => {
                self.lower_expr(cond)?;
                self.emit(ir::OpCode::ToEBV);
                // JumpIfFalse to else (emit placeholder, patched below)
                let pos_jf = self.code.len();
                self.emit(ir::OpCode::JumpIfFalse(0));
                self.lower_expr(then_expr)?;
                let pos_j = self.code.len();
                self.emit(ir::OpCode::Jump(0));
                // Patch JumpIfFalse to here
                Self::patch_jump(&mut self.code, pos_jf);
                self.lower_expr(else_expr)?;
                // Patch Jump to here
                Self::patch_jump(&mut self.code, pos_j);
                Ok(())
            }
            E::Range { start, end } => {
                self.lower_expr(start)?;
                self.lower_expr(end)?;
                self.emit(ir::OpCode::RangeTo);
                Ok(())
            }
            E::InstanceOf { expr, ty } => {
                self.lower_expr(expr)?;
                self.emit(ir::OpCode::InstanceOf(self.lower_seq_type(ty)?));
                Ok(())
            }
            E::TreatAs { expr, ty } => {
                self.lower_expr(expr)?;
                self.emit(ir::OpCode::Treat(self.lower_seq_type(ty)?));
                Ok(())
            }
            E::CastableAs { expr, ty } => {
                self.lower_expr(expr)?;
                self.emit(ir::OpCode::Castable(self.lower_single_type(ty)));
                Ok(())
            }
            E::CastAs { expr, ty } => {
                self.lower_expr(expr)?;
                self.emit(ir::OpCode::Cast(self.lower_single_type(ty)));
                Ok(())
            }
            E::ContextItem => self.load_context_item("the context item expression"),
            E::Path(p) => self.lower_path_expr(p).map(|_| ()),
            E::PathFrom { base, steps } => {
                let single = self.lower_node_stream(base)?;
                self.lower_path_steps(steps, single).map(|_| ())
            }
            E::Quantified { kind, bindings, satisfies } => {
                // Support multiple bindings, nested left-to-right
                if bindings.is_empty() {
                    // Vacuous: some() -> false, every() -> true
                    self.emit(ir::OpCode::PushAtomic(XdmAtomicValue::Boolean(match kind {
                        ast::Quantifier::Some => false,
                        ast::Quantifier::Every => true,
                    })));
                    return Ok(());
                }
                let k = match kind {
                    ast::Quantifier::Some => ir::QuantifierKind::Some,
                    ast::Quantifier::Every => ir::QuantifierKind::Every,
                };
                self.push_scope();
                self.lower_quant_chain(k, bindings, satisfies)?;
                self.pop_scope();
                Ok(())
            }
            E::ForExpr { bindings, return_expr } => {
                if bindings.is_empty() {
                    return self.lower_expr(return_expr);
                }
                self.push_scope();
                self.emit(ir::OpCode::BeginScope(bindings.len()));
                self.lower_for_chain(bindings, return_expr)?;
                self.emit(ir::OpCode::EndScope);
                self.pop_scope();
                Ok(())
            }
            E::LetExpr { bindings, return_expr } => {
                self.push_scope();
                for b in bindings {
                    self.lower_expr(&b.value)?;
                    let en = self.to_expanded(&b.var);
                    self.emit(ir::OpCode::LetStartByName(en.clone()));
                    self.declare_local(en);
                }
                self.lower_expr(return_expr)?;
                for _ in bindings.iter().rev() {
                    self.emit(ir::OpCode::LetEnd);
                }
                self.pop_scope();
                Ok(())
            }
            E::SetOp { left, op, right } => {
                use ast::SetOp::{Except, Intersect, Union};
                self.lower_expr(left)?;
                // Lower right in an isolated compiler fork to avoid state bleed
                let mut right_comp = self.fork();
                right_comp.lower_expr(right)?;
                let right_code = right_comp.code;
                self.code.extend(right_code);
                match op {
                    Union => {
                        // Emit dedicated Union opcode (evaluator handles doc-order + distinct for nodes)
                        self.emit(ir::OpCode::Union);
                    }
                    Intersect => self.emit(ir::OpCode::Intersect),
                    Except => self.emit(ir::OpCode::Except),
                }
                Ok(())
            }
        }
    }

    fn lower_literal(&mut self, l: &ast::Literal) {
        use ast::Literal::{AnyUri, Boolean, Decimal, Double, Integer, String, UntypedAtomic};
        let v = match l {
            Integer(i) => XdmAtomicValue::Integer(*i),
            Decimal(d) => XdmAtomicValue::Decimal(*d),
            Double(d) => XdmAtomicValue::Double(*d),
            String(s) => XdmAtomicValue::String(s.to_string()),
            Boolean(b) => XdmAtomicValue::Boolean(*b),
            AnyUri(s) => XdmAtomicValue::AnyUri(s.to_string()),
            UntypedAtomic(s) => XdmAtomicValue::UntypedAtomic(s.to_string()),
        };
        self.emit(ir::OpCode::PushAtomic(v));
    }

    fn lower_predicates(&mut self, preds: &[ast::Expr]) -> CResult<Vec<ir::PredicateIR>> {
        let mut v = Vec::with_capacity(preds.len());
        for p in preds {
            let start_len = self.code.len();
            self.lower_expr(p)?;
            let code = ir::InstrSeq(self.code.split_off(start_len));
            let kind = if p.is_non_positional_predicate() {
                ir::PredicateKind::NonPositional
            } else {
                ir::PredicateKind::PossiblyPositional
            };
            v.push(ir::PredicateIR { code, kind });
        }
        Ok(v)
    }

    fn lower_for_chain(&mut self, bindings: &[ast::ForBinding], return_expr: &ast::Expr) -> CResult<()> {
        if bindings.is_empty() {
            return self.lower_expr(return_expr);
        }
        let (first, rest) = bindings
            .split_first()
            .ok_or_else(|| Error::from_code(ErrorCode::XPST0003, "for expression requires binding"))?;

        self.lower_expr(&first.in_expr)?;
        let var = self.to_expanded(&first.var);

        let mut body = self.fork();
        body.declare_local(var.clone());
        if rest.is_empty() {
            body.lower_expr(return_expr)?;
        } else {
            body.lower_for_chain(rest, return_expr)?;
        }
        let body_instr = ir::InstrSeq(body.code);
        self.emit(ir::OpCode::ForLoop { var, body: body_instr });
        Ok(())
    }

    fn lower_quant_chain(
        &mut self,
        kind: ir::QuantifierKind,
        bindings: &[ast::QuantifiedBinding],
        satisfies: &ast::Expr,
    ) -> CResult<()> {
        if bindings.is_empty() {
            return Ok(());
        }
        let (first, rest) = bindings
            .split_first()
            .ok_or_else(|| Error::from_code(ErrorCode::XPST0003, "quantified expression requires binding"))?;

        self.lower_expr(&first.in_expr)?;
        let var = self.to_expanded(&first.var);

        let mut body = self.fork();
        body.declare_local(var.clone());
        if rest.is_empty() {
            body.lower_expr(satisfies)?;
        } else {
            body.lower_quant_chain(kind, rest, satisfies)?;
        }
        let body_instr = ir::InstrSeq(body.code);
        self.emit(ir::OpCode::QuantLoop { kind, var, body: body_instr });
        Ok(())
    }

    /// Lowers a path. Its result is in document order without duplicates, and the return value
    /// tells whether it is at most one node.
    fn lower_path_expr(&mut self, p: &ast::PathExpr) -> CResult<bool> {
        match p.start {
            ast::PathStart::Root => {
                self.load_context_item("root path expression")?;
                self.emit(ir::OpCode::Pop);
                self.emit(ir::OpCode::ToRoot);
                self.lower_path_steps(&p.steps, true)
            }
            ast::PathStart::RootDescendant => {
                self.load_context_item("root descendant path expression")?;
                self.emit(ir::OpCode::Pop);
                self.emit(ir::OpCode::ToRoot);
                // `//` is `/descendant-or-self::node()/`; lowered as that step, the steps after it
                // are rewritten as they are after the step the parser writes for `//`.
                let mut steps = Vec::with_capacity(p.steps.len() + 1);
                steps.push(ast::Step::Axis {
                    axis: ast::Axis::DescendantOrSelf,
                    test: ast::NodeTest::Kind(ast::KindTest::AnyKind),
                    predicates: Vec::new(),
                });
                steps.extend(p.steps.iter().cloned());
                self.lower_path_steps(&steps, true)
            }
            ast::PathStart::Relative => {
                self.load_context_item("relative path")?;
                self.lower_path_steps(&p.steps, true)
            }
        }
    }

    /// Lowers the base of a path, `E` in `E/…`, as a stream in document order without duplicates,
    /// and returns whether it is at most one node. Only a base whose order is not proven, such as
    /// a variable, a sequence or a function call, is normalized; a predicate on it still counts in
    /// the base's own order, before the normalization.
    fn lower_node_stream(&mut self, e: &ast::Expr) -> CResult<bool> {
        match e {
            ast::Expr::ContextItem => {
                self.lower_expr(e)?;
                Ok(true)
            }
            ast::Expr::Path(p) => self.lower_path_expr(p),
            ast::Expr::PathFrom { base, steps } => {
                let single = self.lower_node_stream(base)?;
                self.lower_path_steps(steps, single)
            }
            ast::Expr::Parenthesized(inner) => self.lower_node_stream(inner),
            ast::Expr::SetOp { .. } => {
                self.lower_expr(e)?;
                Ok(false)
            }
            ast::Expr::Filter { input, predicates } if Self::is_ordered_node_stream(input) => {
                let single = self.lower_node_stream(input)?;
                let predicates_ir = self.lower_predicates(predicates)?;
                self.emit(ir::OpCode::ApplyPredicates(predicates_ir));
                Ok(single || predicates.iter().any(|p| matches!(p, ast::Expr::Literal(ast::Literal::Integer(1)))))
            }
            _ => {
                self.lower_expr(e)?;
                self.emit(ir::OpCode::Normalize);
                Ok(false)
            }
        }
    }

    /// Whether `e` is lowered as a node stream in document order without duplicates.
    fn is_ordered_node_stream(e: &ast::Expr) -> bool {
        match e {
            ast::Expr::ContextItem | ast::Expr::Path(_) | ast::Expr::PathFrom { .. } | ast::Expr::SetOp { .. } => true,
            ast::Expr::Parenthesized(inner) | ast::Expr::Filter { input: inner, .. } => {
                Self::is_ordered_node_stream(inner)
            }
            _ => false,
        }
    }

    /// Lowers the steps of a path over an input in document order without duplicates, which is at
    /// most one node when `single` is set. After every step the stream is in document order
    /// without duplicates again: a step either keeps that order by construction, or is followed
    /// by a normalization. Returns whether the result is at most one node.
    fn lower_path_steps(&mut self, steps: &[ast::Step], mut single: bool) -> CResult<bool> {
        let mut rest = steps;
        while let Some((s, after)) = rest.split_first() {
            if let Some(lowered) = self.lower_descendant_search(rest)? {
                rest = &rest[lowered..];
                single = false;
                continue;
            }
            rest = after;
            match s {
                ast::Step::Axis { axis, test, predicates } => {
                    let axis_ir = Self::map_axis(axis);
                    let test_ir = self.map_node_test_checked(test, &axis_ir)?;
                    let preds = self.lower_predicates(predicates)?;
                    let positional = preds.iter().any(|p| p.kind == ir::PredicateKind::PossiblyPositional);
                    self.emit(ir::OpCode::AxisStep(axis_ir.clone(), test_ir, preds));
                    single = self.restore_document_order(&axis_ir, positional, single);
                }
                ast::Step::FilterExpr(expr) => {
                    let mut sub = self.fork();
                    sub.lower_expr(expr)?;
                    self.emit(ir::OpCode::PathExprStep(ir::InstrSeq(sub.code)));
                    self.emit(ir::OpCode::Normalize);
                    single = false;
                }
            }
        }
        Ok(single)
    }

    /// Emits what brings the output of an axis step back into document order without duplicates,
    /// and returns whether that output is at most one node. `positional` tells whether a predicate
    /// of the step may count positions, which makes it count per context node.
    ///
    /// - `self`, `attribute` and `child` keep the order of their input; a child step merges the
    ///   children of nested context nodes in document order as it runs.
    /// - `descendant`, `descendant-or-self` and `following` keep the order when their context nodes
    ///   may be merged first, which needs non-positional predicates, or when there is only one;
    ///   a streaming distinct pass stays after them, for models that give two positions one
    ///   identity.
    /// - From one context node, a reverse axis runs nearest first, so reversing its output is
    ///   enough; `preceding` gets there from any number of them, because the last context node's
    ///   preceding nodes include every other's.
    /// - Everything else is normalized.
    fn restore_document_order(&mut self, axis: &ir::AxisIR, positional: bool, single: bool) -> bool {
        use ir::AxisIR::{
            Ancestor, AncestorOrSelf, Attribute, Child, Descendant, DescendantOrSelf, Following, FollowingSibling,
            Namespace, Parent, Preceding, PrecedingSibling, SelfAxis,
        };
        let op = match axis {
            SelfAxis => return single,
            Attribute | Child => return false,
            Descendant | DescendantOrSelf | Following if !positional || single => ir::OpCode::EnsureDistinct,
            FollowingSibling if single => ir::OpCode::EnsureDistinct,
            Parent if single => return true,
            Ancestor | AncestorOrSelf | PrecedingSibling if single => ir::OpCode::Reverse,
            Preceding if !positional || single => ir::OpCode::Reverse,
            Namespace if single => return false,
            Descendant | DescendantOrSelf | Following | FollowingSibling | Parent | Ancestor | AncestorOrSelf
            | Preceding | PrecedingSibling | Namespace => ir::OpCode::Normalize,
        };
        self.emit(op);
        false
    }

    /// Lowers a `descendant-or-self::node()` step without predicates and the step after it as one
    /// `descendant` step, where the two are equivalent: `//T[p]` and `.//T[p]` become
    /// `descendant::T[p]`, and `//(A|B)[p]` becomes `descendant::*[self::A or self::B][p]`, when no
    /// predicate may count positions. A positional predicate counts per parent after `//` and along
    /// the whole descendant axis after `descendant::`, so `//T[1]` keeps both steps. Returns how
    /// many steps it lowered, or `None` for any other shape.
    fn lower_descendant_search(&mut self, steps: &[ast::Step]) -> CResult<Option<usize>> {
        let [
            ast::Step::Axis {
                axis: ast::Axis::DescendantOrSelf,
                test: ast::NodeTest::Kind(ast::KindTest::AnyKind),
                predicates: step_predicates,
            },
            next,
            ..,
        ] = steps
        else {
            return Ok(None);
        };
        if !step_predicates.is_empty() {
            return Ok(None);
        }
        match next {
            ast::Step::Axis { axis: ast::Axis::Child, test, predicates }
                if predicates.iter().all(ast::Expr::is_non_positional_predicate) =>
            {
                let axis = ir::AxisIR::Descendant;
                let test = self.map_node_test_checked(test, &axis)?;
                let predicates = self.lower_predicates(predicates)?;
                self.emit(ir::OpCode::AxisStep(axis, test, predicates));
                self.emit(ir::OpCode::EnsureDistinct);
                Ok(Some(2))
            }
            ast::Step::FilterExpr(expr) => {
                let (union, predicates) = match expr.as_ref() {
                    ast::Expr::Filter { input, predicates } => (input.as_ref(), predicates.as_slice()),
                    other => (other, &[][..]),
                };
                let mut tests = Vec::new();
                if !collect_child_names(union, &mut tests)
                    || tests.len() < 2
                    || !predicates.iter().all(ast::Expr::is_non_positional_predicate)
                {
                    return Ok(None);
                }
                // `self::A or self::B …` keeps what the union selected, as the first predicate.
                let names = tests
                    .into_iter()
                    .map(|test| {
                        ast::Expr::Path(ast::PathExpr {
                            start: ast::PathStart::Relative,
                            steps: vec![ast::Step::Axis {
                                axis: ast::Axis::SelfAxis,
                                test: test.clone(),
                                predicates: Vec::new(),
                            }],
                        })
                    })
                    .reduce(|left, right| ast::Expr::Binary {
                        left: Box::new(left),
                        op: ast::BinaryOp::Or,
                        right: Box::new(right),
                    })
                    .ok_or_else(|| Error::from_code(ErrorCode::XPST0003, "a union has operands"))?;
                let mut all = Vec::with_capacity(predicates.len() + 1);
                all.push(names);
                all.extend(predicates.iter().cloned());
                let predicates = self.lower_predicates(&all)?;
                self.emit(ir::OpCode::AxisStep(ir::AxisIR::Descendant, ir::NodeTestIR::WildcardAny, predicates));
                self.emit(ir::OpCode::EnsureDistinct);
                Ok(Some(2))
            }
            ast::Step::Axis { .. } => Ok(None),
        }
    }

    // no special optimistic streaming hints; evaluator handles streaming per axis

    fn map_axis(a: &ast::Axis) -> ir::AxisIR {
        use ast::Axis::{
            Ancestor, AncestorOrSelf, Attribute, Child, Descendant, DescendantOrSelf, Following, FollowingSibling,
            Namespace, Parent, Preceding, PrecedingSibling, SelfAxis,
        };
        match a {
            Child => ir::AxisIR::Child,
            Descendant => ir::AxisIR::Descendant,
            Attribute => ir::AxisIR::Attribute,
            SelfAxis => ir::AxisIR::SelfAxis,
            DescendantOrSelf => ir::AxisIR::DescendantOrSelf,
            FollowingSibling => ir::AxisIR::FollowingSibling,
            Following => ir::AxisIR::Following,
            Namespace => ir::AxisIR::Namespace,
            Parent => ir::AxisIR::Parent,
            Ancestor => ir::AxisIR::Ancestor,
            PrecedingSibling => ir::AxisIR::PrecedingSibling,
            Preceding => ir::AxisIR::Preceding,
            AncestorOrSelf => ir::AxisIR::AncestorOrSelf,
        }
    }

    fn should_apply_default_element_namespace(&self, axis: &ir::AxisIR) -> bool {
        !matches!(axis, ir::AxisIR::Attribute | ir::AxisIR::Namespace)
            && self.static_ctx.default_element_namespace.is_some()
    }

    fn map_node_test_checked(&self, t: &ast::NodeTest, axis: &ir::AxisIR) -> CResult<ir::NodeTestIR> {
        Ok(match t {
            ast::NodeTest::Name(nt) => match nt {
                ast::NameTest::QName(q) => {
                    let mut expanded = self.to_expanded(q);
                    if expanded.ns_uri.is_none()
                        && q.prefix.is_none()
                        && self.should_apply_default_element_namespace(axis)
                        && let Some(ns) = &self.static_ctx.default_element_namespace
                    {
                        expanded.ns_uri = Some(ns.clone());
                    }
                    ir::NodeTestIR::Name(ir::InternedQName::from_expanded(expanded))
                }
                ast::NameTest::Wildcard(w) => match w {
                    ast::WildcardName::Any => ir::NodeTestIR::WildcardAny,
                    ast::WildcardName::NsWildcard(prefix) => {
                        let uri =
                            self.static_ctx.namespaces.by_prefix.get(prefix).cloned().unwrap_or_else(|| prefix.clone());
                        ir::NodeTestIR::NsWildcard(DefaultAtom::from(uri.as_str()))
                    }
                    ast::WildcardName::LocalWildcard(loc) => {
                        ir::NodeTestIR::LocalWildcard(DefaultAtom::from(loc.as_str()))
                    }
                },
            },
            ast::NodeTest::Kind(k) => {
                Self::validate_kind_test(k)?;
                self.map_kind_test(k)
            }
        })
    }

    fn validate_kind_test(k: &ast::KindTest) -> CResult<()> {
        use ast::KindTest as K;
        match k {
            K::Element { ty, nillable, .. } => {
                if ty.is_some() || *nillable {
                    return Err(Error::from_code(
                        ErrorCode::XPST0003,
                        "element() with type/nillable not supported without schema awareness",
                    ));
                }
                Ok(())
            }
            K::Attribute { ty, .. } => {
                if ty.is_some() {
                    return Err(Error::from_code(
                        ErrorCode::XPST0003,
                        "attribute() with type not supported without schema awareness",
                    ));
                }
                Ok(())
            }
            K::SchemaElement(_) | K::SchemaAttribute(_) => Err(Error::from_code(
                ErrorCode::XPST0003,
                "schema-* kind tests are not supported without schema awareness",
            )),
            _ => Ok(()),
        }
    }

    fn map_kind_test(&self, k: &ast::KindTest) -> ir::NodeTestIR {
        use ast::KindTest as K;
        match k {
            K::AnyKind => ir::NodeTestIR::AnyKind,
            K::Document(inner) => ir::NodeTestIR::KindDocument(inner.as_ref().map(|b| Box::new(self.map_kind_test(b)))),
            K::Text => ir::NodeTestIR::KindText,
            K::Comment => ir::NodeTestIR::KindComment,
            K::ProcessingInstruction(opt) => ir::NodeTestIR::KindProcessingInstruction(opt.clone()),
            K::Element { name, ty, nillable } => ir::NodeTestIR::KindElement {
                name: name.as_ref().map(|n| match n {
                    ast::ElementNameOrWildcard::Any => ir::NameOrWildcard::Any,
                    ast::ElementNameOrWildcard::Name(q) => {
                        let mut expanded = self.to_expanded(q);
                        if expanded.ns_uri.is_none()
                            && q.prefix.is_none()
                            && let Some(ns) = &self.static_ctx.default_element_namespace
                        {
                            expanded.ns_uri = Some(ns.clone());
                        }
                        ir::NameOrWildcard::Name(ir::InternedQName::from_expanded(expanded))
                    }
                }),
                ty: ty.as_ref().map(|t| {
                    let mut expanded = self.to_expanded(&t.0);
                    if expanded.ns_uri.is_none() && self.static_ctx.default_element_namespace.is_some() {
                        expanded.ns_uri.clone_from(&self.static_ctx.default_element_namespace);
                    }
                    expanded
                }),
                nillable: *nillable,
            },
            K::Attribute { name, ty } => ir::NodeTestIR::KindAttribute {
                name: name.as_ref().map(|n| match n {
                    ast::AttributeNameOrWildcard::Any => ir::NameOrWildcard::Any,
                    ast::AttributeNameOrWildcard::Name(q) => {
                        ir::NameOrWildcard::Name(ir::InternedQName::from_expanded(self.to_expanded(q)))
                    }
                }),
                ty: ty.as_ref().map(|t| self.to_expanded(&t.0)),
            },
            K::SchemaElement(q) => ir::NodeTestIR::KindSchemaElement(self.to_expanded(q)),
            K::SchemaAttribute(q) => ir::NodeTestIR::KindSchemaAttribute(self.to_expanded(q)),
        }
    }

    fn lower_single_type(&self, t: &ast::SingleType) -> ir::SingleTypeIR {
        ir::SingleTypeIR { atomic: self.to_expanded(&t.atomic), optional: t.optional }
    }
    fn lower_seq_type(&self, t: &ast::SequenceType) -> CResult<ir::SeqTypeIR> {
        use ast::SequenceType::{EmptySequence, Typed};
        Ok(match t {
            EmptySequence => ir::SeqTypeIR::EmptySequence,
            Typed { item, occ } => {
                ir::SeqTypeIR::Typed { item: self.lower_item_type(item)?, occ: Self::lower_occ(occ) }
            }
        })
    }
    fn lower_item_type(&self, t: &ast::ItemType) -> CResult<ir::ItemTypeIR> {
        use ast::ItemType::{Atomic, Item, Kind};
        Ok(match t {
            Item => ir::ItemTypeIR::AnyItem,
            Atomic(q) => ir::ItemTypeIR::Atomic(self.to_expanded(q)),
            Kind(k) => {
                Self::validate_kind_test(k)?;
                ir::ItemTypeIR::Kind(self.map_kind_test(k))
            }
        })
    }
    fn lower_occ(o: &ast::Occurrence) -> ir::OccurrenceIR {
        use ast::Occurrence::{One, OneOrMore, ZeroOrMore, ZeroOrOne};
        match o {
            One => ir::OccurrenceIR::One,
            ZeroOrOne => ir::OccurrenceIR::ZeroOrOne,
            ZeroOrMore => ir::OccurrenceIR::ZeroOrMore,
            OneOrMore => ir::OccurrenceIR::OneOrMore,
        }
    }

    fn to_expanded(&self, q: &ast::QName) -> ExpandedName {
        // Resolve namespace using static context; retain built-in defaults for fn/xs.
        let mut ns = match q.prefix.as_deref() {
            Some("fn") => Some(crate::consts::FNS.to_string()),
            Some("xs") => Some(crate::consts::XS.to_string()),
            _ => q.ns_uri.clone(),
        };
        if ns.is_none()
            && let Some(pref) = &q.prefix
            && let Some(uri) = self.static_ctx.namespaces.by_prefix.get(pref)
        {
            ns = Some(uri.clone());
        }
        ExpandedName { ns_uri: ns, local: q.local.clone() }
    }

    fn ensure_function_available(&self, name: &ast::QName, arity: usize) -> CResult<()> {
        let expanded = self.to_expanded(name);
        let signatures = &self.static_ctx.function_signatures;

        let mut candidates: Vec<ExpandedName> = Vec::new();
        candidates.push(expanded.clone());
        if expanded.ns_uri.is_none()
            && name.prefix.is_none()
            && let Some(default_ns) = &self.static_ctx.default_function_namespace
        {
            let mut with_default = expanded.clone();
            with_default.ns_uri = Some(default_ns.clone());
            candidates.push(with_default);
        }

        for cand in &candidates {
            if signatures.supports(cand, arity) {
                return Ok(());
            }
        }

        let default_fn_ns = self.static_ctx.default_function_namespace.as_ref();

        for cand in &candidates {
            if let Some(_ranges) = signatures.arities(cand) {
                let arg_phrase = match arity {
                    0 => "no arguments".to_string(),
                    1 => "one argument".to_string(),
                    2 => "two arguments".to_string(),
                    3 => "three arguments".to_string(),
                    n => format!("{n} arguments"),
                };
                let friendly =
                    if cand.ns_uri.as_ref() == default_fn_ns { cand.local.clone() } else { cand.to_string() };
                return Err(Error::from_code(
                    ErrorCode::XPST0017,
                    format!("function {friendly}() cannot be called with {arg_phrase}"),
                ));
            }
        }

        let display = candidates.last().cloned().unwrap_or(expanded);
        Err(Error::from_code(ErrorCode::XPST0017, format!("unknown function: {display}#{arity}")))
    }

    fn patch_jump(code: &mut [ir::OpCode], pos: usize) {
        let delta = code.len() - pos - 1;
        if let Some(ir::OpCode::JumpIfFalse(d) | ir::OpCode::JumpIfTrue(d) | ir::OpCode::Jump(d)) = code.get_mut(pos) {
            *d = delta;
        }
    }
}

/// Collects the name tests of a union of single child steps without predicates, such as
/// `(A|B|C)`. Returns `false` for any other operand.
fn collect_child_names<'a>(expr: &'a ast::Expr, tests: &mut Vec<&'a ast::NodeTest>) -> bool {
    match expr {
        ast::Expr::SetOp { left, op: ast::SetOp::Union, right } => {
            collect_child_names(left, tests) && collect_child_names(right, tests)
        }
        ast::Expr::Parenthesized(inner) => collect_child_names(inner, tests),
        ast::Expr::Path(ast::PathExpr { start: ast::PathStart::Relative, steps }) => match steps.as_slice() {
            [ast::Step::Axis { axis: ast::Axis::Child, test: test @ ast::NodeTest::Name(_), predicates }]
                if predicates.is_empty() =>
            {
                tests.push(test);
                true
            }
            _ => false,
        },
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::compile_with_context;
    use crate::engine::runtime::{STATIC_CONTEXT_COMPILE_CACHE_CAPACITY, StaticContext, StaticContextBuilder};
    use std::sync::Arc;

    #[test]
    fn cache_serves_repeat_compiles_without_reparsing() {
        let expr = "1 + 2";
        let ctx = StaticContext::default();

        assert_eq!(ctx.compile_cache.lock().expect("cache lock").len(), 0);

        let first = compile_with_context(expr, &ctx).expect("first compile succeeds");

        let first_ptr = {
            let cache = ctx.compile_cache.lock().expect("cache lock");
            let entry = cache.peek(expr).expect("entry present after first compile");
            Arc::as_ptr(entry)
        };

        let second = compile_with_context(expr, &ctx).expect("second compile succeeds");

        let second_ptr = {
            let cache = ctx.compile_cache.lock().expect("cache lock");
            let entry = cache.peek(expr).expect("entry present after second compile");
            Arc::as_ptr(entry)
        };

        assert_eq!(first.instrs, second.instrs);
        assert_eq!(first_ptr, second_ptr);
    }

    #[test]
    fn cache_separates_entries_by_static_context() {
        let expr = "string(1)";
        let default_ctx = StaticContext::default();
        let custom_ctx = StaticContextBuilder::new().with_namespace("p", "http://example.com/custom").build();

        compile_with_context(expr, &default_ctx).expect("default context compile succeeds");
        compile_with_context(expr, &custom_ctx).expect("custom context compile succeeds");

        let default_len = {
            let cache = default_ctx.compile_cache.lock().expect("default cache lock poisoned");
            cache.len()
        };
        let custom_len = {
            let cache = custom_ctx.compile_cache.lock().expect("custom cache lock poisoned");
            cache.len()
        };

        assert_eq!(default_len, 1);
        assert_eq!(custom_len, 1);
    }

    #[test]
    fn cache_respects_capacity_limit() {
        let ctx = StaticContext::default();
        for i in 0..(STATIC_CONTEXT_COMPILE_CACHE_CAPACITY + 5) {
            let expr = format!("{} + {}", i, i + 1);
            compile_with_context(&expr, &ctx).expect("compilation succeeds");
        }

        let len = ctx.compile_cache.lock().expect("test cache lock poisoned").len();
        assert_eq!(len, STATIC_CONTEXT_COMPILE_CACHE_CAPACITY);
    }
}
