use crate::engine::runtime::ErrorCode;
use crate::engine::runtime::{Error, StaticContext};
use crate::parser::{ast, parse_xpath};
use crate::xdm::{ExpandedName, XdmAtomicValue};

pub mod ir;

use std::sync::OnceLock;

static DEFAULT_STATIC_CONTEXT: OnceLock<StaticContext> = OnceLock::new();

fn default_static_ctx() -> &'static StaticContext {
    DEFAULT_STATIC_CONTEXT.get_or_init(StaticContext::default)
}

/// Compile using a lazily initialized default StaticContext
pub fn compile_xpath(expr: &str) -> Result<ir::CompiledXPath, Error> {
    compile_inner(expr, default_static_ctx())
}

/// Compile with an explicitly provided StaticContext
pub fn compile_xpath_with_context(
    expr: &str,
    static_ctx: &StaticContext,
) -> Result<ir::CompiledXPath, Error> {
    compile_inner(expr, static_ctx)
}

/// Backing implementation shared by all compile entrypoints
fn compile_inner(expr: &str, static_ctx: &StaticContext) -> Result<ir::CompiledXPath, Error> {
    let ast = parse_xpath(expr)?;
    let mut c = Compiler::new(static_ctx, expr);
    c.lower_expr(&ast)?;
    Ok(ir::CompiledXPath {
        instrs: ir::InstrSeq(c.code),
        static_ctx: std::sync::Arc::new(static_ctx.clone()),
        source: expr.to_string(),
    })
}

struct Compiler<'a> {
    static_ctx: &'a StaticContext,
    source: &'a str,
    code: Vec<ir::OpCode>,
    lexical_scopes: Vec<Vec<ExpandedName>>,
}

type CResult<T> = Result<T, Error>;

impl<'a> Compiler<'a> {
    fn new(static_ctx: &'a StaticContext, source: &'a str) -> Self {
        Self {
            static_ctx,
            source,
            code: Vec::new(),
            lexical_scopes: Vec::new(),
        }
    }

    fn fork(&self) -> Self {
        Self {
            static_ctx: self.static_ctx,
            source: self.source,
            code: Vec::new(),
            lexical_scopes: self.lexical_scopes.clone(),
        }
    }

    fn emit(&mut self, op: ir::OpCode) {
        self.code.push(op);
    }

    fn push_scope(&mut self) {
        self.lexical_scopes.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        self.lexical_scopes.pop();
    }

    fn declare_local(&mut self, name: ExpandedName) {
        if self.lexical_scopes.is_empty() {
            self.lexical_scopes.push(Vec::new());
        }
        if let Some(scope) = self.lexical_scopes.last_mut() {
            scope.push(name);
        }
    }

    fn var_in_scope(&self, name: &ExpandedName) -> bool {
        self.lexical_scopes
            .iter()
            .rev()
            .any(|scope| scope.iter().any(|n| n == name))
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
                for a in args {
                    self.lower_expr(a)?;
                }
                let en = self.to_expanded(name);
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
                self.lower_expr(left)?;
                self.lower_expr(right)?;
                use ast::BinaryOp::*;
                self.emit(match op {
                    Add => ir::OpCode::Add,
                    Sub => ir::OpCode::Sub,
                    Mul => ir::OpCode::Mul,
                    Div => ir::OpCode::Div,
                    IDiv => ir::OpCode::IDiv,
                    Mod => ir::OpCode::Mod,
                    And => ir::OpCode::And,
                    Or => ir::OpCode::Or,
                });
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
            E::IfThenElse {
                cond,
                then_expr,
                else_expr,
            } => {
                self.lower_expr(cond)?;
                self.emit(ir::OpCode::ToEBV);
                // JumpIfFalse to else
                // Placeholder offset 0; patch later
                let pos_jf = self.code.len();
                self.emit(ir::OpCode::JumpIfFalse(0));
                self.lower_expr(then_expr)?;
                let pos_j = self.code.len();
                self.emit(ir::OpCode::Jump(0));
                // patch JumpIfFalse to here
                Self::patch_jump(&mut self.code, pos_jf);
                self.lower_expr(else_expr)?;
                // patch Jump to here
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
            E::ContextItem => {
                self.emit(ir::OpCode::LoadContextItem);
                Ok(())
            }
            E::Path(p) => self.lower_path_expr(p, None),
            E::PathFrom { base, steps } => {
                self.lower_expr(base)?;
                self.lower_path_steps(steps)
            }
            E::Quantified {
                kind,
                bindings,
                satisfies,
            } => {
                // Support multiple bindings by nesting quantifiers left-to-right
                if bindings.is_empty() {
                    // Vacuous: some() -> false, every() -> true
                    self.emit(ir::OpCode::PushAtomic(XdmAtomicValue::Boolean(
                        match kind {
                            ast::Quantifier::Some => false,
                            ast::Quantifier::Every => true,
                        },
                    )));
                    return Ok(());
                }
                let k = match kind {
                    ast::Quantifier::Some => ir::QuantifierKind::Some,
                    ast::Quantifier::Every => ir::QuantifierKind::Every,
                };
                self.push_scope();
                // Emit nested QuantStartByName for each binding
                for b in bindings {
                    self.lower_expr(&b.in_expr)?;
                    let en = self.to_expanded(&b.var);
                    self.emit(ir::OpCode::QuantStartByName(k, en.clone()));
                    self.declare_local(en);
                }
                // Body: satisfies expression (evaluated in innermost scope)
                self.lower_expr(satisfies)?;
                self.emit(ir::OpCode::ToEBV);
                // Close quantifiers in reverse order
                for _ in bindings {
                    self.emit(ir::OpCode::QuantEnd);
                }
                self.pop_scope();
                Ok(())
            }
            E::ForExpr {
                bindings,
                return_expr,
            } => {
                if bindings.is_empty() {
                    return self.lower_expr(return_expr);
                }
                // Nest for-loops left-to-right
                self.push_scope();
                self.emit(ir::OpCode::BeginScope(bindings.len()));
                for b in bindings {
                    self.lower_expr(&b.in_expr)?;
                    let en = self.to_expanded(&b.var);
                    self.emit(ir::OpCode::ForStartByName(en.clone()));
                    self.declare_local(en);
                }
                self.lower_expr(return_expr)?;
                // Close loops: ForNext for each, then ForEnd for each
                for _ in 0..bindings.len() {
                    self.emit(ir::OpCode::ForNext);
                    self.emit(ir::OpCode::ForEnd);
                }
                self.emit(ir::OpCode::EndScope);
                self.pop_scope();
                Ok(())
            }
            E::LetExpr { .. } => {
                // Note: 'let' is part of XPath 2.0, and is currently intentionally not supported in this project.
                // We reject it at the parser/compiler level to maintain the scope of "pure XPath 2.0 without let".
                Err(Error::from_code(
                    ErrorCode::XPST0003,
                    "'let' is not supported in this XPath implementation",
                ))
            }
            E::SetOp { left, op, right } => {
                self.lower_expr(left)?;
                self.lower_expr(right)?;
                use ast::SetOp::*;
                self.emit(match op {
                    Union => ir::OpCode::Union,
                    Intersect => ir::OpCode::Intersect,
                    Except => ir::OpCode::Except,
                });
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
            String(s) => XdmAtomicValue::String(s.clone()),
            Boolean(b) => XdmAtomicValue::Boolean(*b),
            AnyUri(s) => XdmAtomicValue::AnyUri(s.clone()),
            UntypedAtomic(s) => XdmAtomicValue::UntypedAtomic(s.clone()),
        };
        self.emit(ir::OpCode::PushAtomic(v));
        Ok(())
    }

    fn lower_predicates(&mut self, preds: &[ast::Expr]) -> CResult<Vec<ir::InstrSeq>> {
        let mut v = Vec::with_capacity(preds.len());
        for p in preds {
            let mut sub = self.fork();
            sub.lower_expr(p)?;
            v.push(ir::InstrSeq(sub.code));
        }
        Ok(v)
    }

    fn lower_path_expr(&mut self, p: &ast::PathExpr, base: Option<&ast::Expr>) -> CResult<()> {
        match p.start {
            ast::PathStart::Root => self.emit(ir::OpCode::ToRoot),
            ast::PathStart::RootDescendant => {
                self.emit(ir::OpCode::ToRoot);
                self.emit(ir::OpCode::AxisStep(
                    ir::AxisIR::DescendantOrSelf,
                    ir::NodeTestIR::AnyKind,
                    vec![],
                ));
            }
            ast::PathStart::Relative => {
                if let Some(b) = base {
                    self.lower_expr(b)?;
                } else {
                    self.emit(ir::OpCode::LoadContextItem);
                }
            }
        }
        self.lower_path_steps(&p.steps)
    }

    fn lower_path_steps(&mut self, steps: &[ast::Step]) -> CResult<()> {
        for s in steps {
            let axis = self.map_axis(&s.axis);
            let test = self.map_node_test_checked(&s.test)?;
            let preds = self.lower_predicates(&s.predicates)?;
            self.emit(ir::OpCode::AxisStep(axis, test, preds));
            self.emit(ir::OpCode::DocOrderDistinct);
        }
        Ok(())
    }

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

    fn map_node_test_checked(&self, t: &ast::NodeTest) -> CResult<ir::NodeTestIR> {
        Ok(match t {
            ast::NodeTest::Name(nt) => match nt {
                ast::NameTest::QName(q) => ir::NodeTestIR::Name(self.to_expanded(q)),
                ast::NameTest::Wildcard(w) => match w {
                    ast::WildcardName::Any => ir::NodeTestIR::WildcardAny,
                    ast::WildcardName::NsWildcard(prefix) => {
                        let uri = self
                            .static_ctx
                            .namespaces
                            .by_prefix
                            .get(prefix)
                            .cloned()
                            .unwrap_or_else(|| prefix.clone());
                        ir::NodeTestIR::NsWildcard(uri)
                    }
                    ast::WildcardName::LocalWildcard(loc) => {
                        ir::NodeTestIR::LocalWildcard(loc.clone())
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
            K::Document(inner) => ir::NodeTestIR::KindDocument(
                inner.as_ref().map(|b| Box::new(self.map_kind_test(b))),
            ),
            K::Text => ir::NodeTestIR::KindText,
            K::Comment => ir::NodeTestIR::KindComment,
            K::ProcessingInstruction(opt) => ir::NodeTestIR::KindProcessingInstruction(opt.clone()),
            K::Element { name, ty, nillable } => ir::NodeTestIR::KindElement {
                name: name.as_ref().map(|n| match n {
                    ast::ElementNameOrWildcard::Any => ir::NameOrWildcard::Any,
                    ast::ElementNameOrWildcard::Name(q) => {
                        ir::NameOrWildcard::Name(self.to_expanded(q))
                    }
                }),
                ty: ty.as_ref().map(|t| self.to_expanded(&t.0)),
                nillable: *nillable,
            },
            K::Attribute { name, ty } => ir::NodeTestIR::KindAttribute {
                name: name.as_ref().map(|n| match n {
                    ast::AttributeNameOrWildcard::Any => ir::NameOrWildcard::Any,
                    ast::AttributeNameOrWildcard::Name(q) => {
                        ir::NameOrWildcard::Name(self.to_expanded(q))
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
        Ok(ir::SingleTypeIR {
            atomic: self.to_expanded(&t.atomic),
            optional: t.optional,
        })
    }
    fn lower_seq_type(&self, t: &ast::SequenceType) -> CResult<ir::SeqTypeIR> {
        use ast::SequenceType::*;
        Ok(match t {
            EmptySequence => ir::SeqTypeIR::EmptySequence,
            Typed { item, occ } => ir::SeqTypeIR::Typed {
                item: self.lower_item_type(item)?,
                occ: self.lower_occ(occ),
            },
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
        ExpandedName {
            ns_uri: ns,
            local: q.local.clone(),
        }
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
