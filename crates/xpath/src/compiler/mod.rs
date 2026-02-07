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

/// Compile using a lazily initialized default StaticContext
pub fn compile(expr: &str) -> Result<ir::CompiledXPath, Error> {
    compile_inner(expr, default_static_ctx())
}

/// Compile with an explicitly provided StaticContext
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
    let cache_entry = Arc::new(instrs.clone());

    static_ctx
        .compile_cache
        .lock()
        .map_err(|_| Error::from_code(ErrorCode::FOER0000, "compile cache lock poisoned"))?
        .put(source.clone(), cache_entry);

    Ok(ir::CompiledXPath { instrs, static_ctx: Arc::new(static_ctx.clone()), source })
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

    fn lower_expr(&mut self, e: &ast::Expr) -> CResult<()> {
        use ast::Expr as E;
        match e {
            E::Literal(l) => self.lower_literal(l),
            E::Parenthesized(inner) => self.lower_expr(inner),
            E::VarRef(q) => {
                let en = self.to_expanded(q);
                if !self.var_in_scope(&en) {
                    return Err(Error::from_code(
                        ErrorCode::XPST0008,
                        format!("Variable ${} is not declared in the static context", en),
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
                    .map(|kinds| kinds.to_vec());
                for (idx, a) in args.iter().enumerate() {
                    self.lower_expr(a)?;
                    if let Some(specs) = &param_specs
                        && specs.get(idx).is_some_and(|spec| spec.requires_atomization())
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
                use ast::BinaryOp::*;

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
                self.emit(ir::OpCode::CompareGeneral(self.map_cmp(op)));
                Ok(())
            }
            E::ValueComparison { left, op, right } => {
                self.lower_expr(left)?;
                self.lower_expr(right)?;
                self.emit(ir::OpCode::CompareValue(self.map_cmp(op)));
                Ok(())
            }
            E::NodeComparison { left, op, right } => {
                self.lower_expr(left)?;
                self.lower_expr(right)?;
                use ast::NodeComp::*;
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
                self.emit(ir::OpCode::Castable(self.lower_single_type(ty)?));
                Ok(())
            }
            E::CastAs { expr, ty } => {
                self.lower_expr(expr)?;
                self.emit(ir::OpCode::Cast(self.lower_single_type(ty)?));
                Ok(())
            }
            E::ContextItem => self.load_context_item("the context item expression"),
            E::Path(p) => self.lower_path_expr(p, None),
            E::PathFrom { base, steps } => {
                self.lower_expr(base)?;
                self.lower_path_steps(steps)
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
                self.lower_expr(left)?;
                // Lower right in an isolated compiler fork to avoid state bleed
                let mut right_comp = self.fork();
                right_comp.lower_expr(right)?;
                let right_code = right_comp.code;
                self.code.extend(right_code);
                use ast::SetOp::*;
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

    fn lower_literal(&mut self, l: &ast::Literal) -> CResult<()> {
        use ast::Literal::*;
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
        Ok(())
    }

    fn lower_predicates(&mut self, preds: &[ast::Expr]) -> CResult<Vec<ir::InstrSeq>> {
        let mut v = Vec::with_capacity(preds.len());
        for p in preds {
            let start_len = self.code.len();
            self.lower_expr(p)?;
            let sub_code = self.code.split_off(start_len);
            v.push(ir::InstrSeq(sub_code));
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

    fn lower_path_expr(&mut self, p: &ast::PathExpr, base: Option<&ast::Expr>) -> CResult<()> {
        match p.start {
            ast::PathStart::Root => {
                self.load_context_item("root path expression")?;
                self.emit(ir::OpCode::Pop);
                self.emit(ir::OpCode::ToRoot);
            }
            ast::PathStart::RootDescendant => {
                self.load_context_item("root descendant path expression")?;
                self.emit(ir::OpCode::Pop);
                self.emit(ir::OpCode::ToRoot);
                self.emit(ir::OpCode::AxisStep(ir::AxisIR::DescendantOrSelf, ir::NodeTestIR::AnyKind, vec![]));
            }
            ast::PathStart::Relative => {
                if let Some(b) = base {
                    self.lower_expr(b)?;
                } else {
                    self.load_context_item("relative path")?;
                }
            }
        }
        self.lower_path_steps(&p.steps)
    }

    fn lower_path_steps(&mut self, steps: &[ast::Step]) -> CResult<()> {
        for s in steps {
            match s {
                ast::Step::Axis { axis, test, predicates } => {
                    let axis_ir = self.map_axis(axis);
                    let test_ir = self.map_node_test_checked(test, &axis_ir)?;
                    let preds = self.lower_predicates(predicates)?;
                    self.emit(ir::OpCode::AxisStep(axis_ir.clone(), test_ir.clone(), preds));
                    // Emit doc-order/distinct only where required by axis semantics.
                    // For child/attribute/self the concatenation over a doc-ordered, distinct
                    // input remains doc-ordered and duplicate-free.
                    match axis_ir {
                        // Forward axes that do not introduce duplicates and preserve order
                        ir::AxisIR::Child | ir::AxisIR::SelfAxis => {}
                        // Forward axes that may introduce duplicates but keep order
                        ir::AxisIR::Descendant
                        | ir::AxisIR::DescendantOrSelf
                        | ir::AxisIR::Following
                        | ir::AxisIR::FollowingSibling => self.emit(ir::OpCode::EnsureDistinct),
                        ir::AxisIR::Attribute | ir::AxisIR::Namespace => { /* no normalization needed */ }
                        // Reverse axes need both: order and distinct
                        ir::AxisIR::Parent
                        | ir::AxisIR::Ancestor
                        | ir::AxisIR::AncestorOrSelf
                        | ir::AxisIR::Preceding
                        | ir::AxisIR::PrecedingSibling => {
                            self.emit(ir::OpCode::EnsureDistinct);
                            self.emit(ir::OpCode::EnsureOrder);
                        }
                    }
                }
                ast::Step::FilterExpr(expr) => {
                    let mut sub = self.fork();
                    sub.lower_expr(expr)?;
                    self.emit(ir::OpCode::PathExprStep(ir::InstrSeq(sub.code)));
                    // General expression → normalize fully for next axis
                    self.emit(ir::OpCode::EnsureDistinct);
                    self.emit(ir::OpCode::EnsureOrder);
                }
            }
        }
        Ok(())
    }

    // no special optimistic streaming hints; evaluator handles streaming per axis

    fn map_axis(&self, a: &ast::Axis) -> ir::AxisIR {
        use ast::Axis::*;
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
                self.validate_kind_test(k)?;
                self.map_kind_test(k)
            }
        })
    }

    fn validate_kind_test(&self, k: &ast::KindTest) -> CResult<()> {
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
                        expanded.ns_uri = self.static_ctx.default_element_namespace.clone();
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

    fn map_cmp<T>(&self, op: &T) -> ir::ComparisonOp
    where
        T: std::fmt::Debug,
    {
        // op is either GeneralComp or ValueComp with same set
        // map via string, safe due to same names
        match format!("{:?}", op).as_str() {
            "Eq" => ir::ComparisonOp::Eq,
            "Ne" => ir::ComparisonOp::Ne,
            "Lt" => ir::ComparisonOp::Lt,
            "Le" => ir::ComparisonOp::Le,
            "Gt" => ir::ComparisonOp::Gt,
            "Ge" => ir::ComparisonOp::Ge,
            _ => ir::ComparisonOp::Eq,
        }
    }

    fn lower_single_type(&self, t: &ast::SingleType) -> CResult<ir::SingleTypeIR> {
        Ok(ir::SingleTypeIR { atomic: self.to_expanded(&t.atomic), optional: t.optional })
    }
    fn lower_seq_type(&self, t: &ast::SequenceType) -> CResult<ir::SeqTypeIR> {
        use ast::SequenceType::*;
        Ok(match t {
            EmptySequence => ir::SeqTypeIR::EmptySequence,
            Typed { item, occ } => ir::SeqTypeIR::Typed { item: self.lower_item_type(item)?, occ: self.lower_occ(occ) },
        })
    }
    fn lower_item_type(&self, t: &ast::ItemType) -> CResult<ir::ItemTypeIR> {
        use ast::ItemType::*;
        Ok(match t {
            Item => ir::ItemTypeIR::AnyItem,
            Atomic(q) => ir::ItemTypeIR::Atomic(self.to_expanded(q)),
            Kind(k) => {
                self.validate_kind_test(k)?;
                ir::ItemTypeIR::Kind(self.map_kind_test(k))
            }
        })
    }
    fn lower_occ(&self, o: &ast::Occurrence) -> ir::OccurrenceIR {
        use ast::Occurrence::*;
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
                    format!("function {}() cannot be called with {}", friendly, arg_phrase),
                ));
            }
        }

        let display = candidates.last().cloned().unwrap_or(expanded);
        Err(Error::from_code(ErrorCode::XPST0017, format!("unknown function: {}#{}", display, arity)))
    }

    fn patch_jump(code: &mut [ir::OpCode], pos: usize) {
        let delta = code.len() - pos - 1;
        if let Some(op) = code.get_mut(pos) {
            match op {
                ir::OpCode::JumpIfFalse(d) => *d = delta,
                ir::OpCode::JumpIfTrue(d) => *d = delta,
                ir::OpCode::Jump(d) => *d = delta,
                _ => {}
            }
        }
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
