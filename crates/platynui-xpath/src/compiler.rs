use crate::parser::{ast, XPathParser};
use crate::runtime::{Error, StaticContext};
use crate::xdm::{ExpandedName, XdmAtomicValue};
use core::fmt;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub enum AxisIR {
    Child,
    Attribute,
    SelfAxis,
    DescendantOrSelf,
    Descendant,
    Parent,
    Ancestor,
    AncestorOrSelf,
    PrecedingSibling,
    FollowingSibling,
    Preceding,
    Following,
    Namespace,
}

#[derive(Debug, Clone)]
pub enum NodeTestIR {
    AnyKind,
    Name(ExpandedName),
    WildcardAny,
    NsWildcard(String),
    LocalWildcard(String),
    KindText,
}

#[derive(Debug, Clone)]
pub enum OpCode {
    // Data and variables
    PushAtomic(XdmAtomicValue),
    LoadVar(usize),
    StoreVar(usize),
    LoadContextItem,
    Position,
    Last,
    ToRoot,

    // Steps / filters
    AxisStep(AxisIR, NodeTestIR, Vec<InstrSeq>),
    PredicateStart,
    PredicateEnd,

    // Arithmetic / logic
    Add,
    Sub,
    Mul,
    Div,
    IDiv,
    Mod,
    And,
    Or,
    Not,
    ToEBV,
    Pop,
    JumpIfTrue(usize),   // relative forward
    JumpIfFalse(usize),  // relative forward

    // Comparisons
    CompareValue(u8 /* op */),
    CompareGeneral(u8 /* op */),
    NodeIs,
    NodeBefore,
    NodeAfter,

    // Sequences and sets
    MakeSeq(usize),
    ConcatSeq,
    Union,
    Intersect,
    Except,
    RangeTo,

    // Control flow / bindings
    IfElse,
    Some,
    Every,
    ForStart,
    ForNext,
    ForEnd,
    LetBind,

    // Types
    Cast,
    Castable,
    Treat,
    InstanceOf,

    // Functions
    Call(usize /* fn id */, usize /* argc */),
    CallByName(ExpandedName, usize /* argc */),
}

#[derive(Debug, Clone, Default)]
pub struct InstrSeq(pub Vec<OpCode>);

#[derive(Debug, Clone)]
pub struct CompiledIR {
    pub instrs: InstrSeq,
    pub static_ctx: Arc<StaticContext>,
    pub source: String,
}

pub fn compile_xpath(expr: &str, static_ctx: &StaticContext) -> Result<CompiledIR, Error> {
    // Straightforward: build full AST then compile
    let ast = match XPathParser::parse_to_ast(expr) {
        Ok(a) => a,
        Err(e) => return Err(Error::static_err("err:XPST0003", format!("syntax error: {}", e))),
    };

    let mut instrs = InstrSeq::default();
    compile_expr(&ast, &mut instrs, static_ctx)?;
    Ok(CompiledIR { instrs, static_ctx: Arc::new(static_ctx.clone()), source: expr.to_string() })
}

// Removed naive_simple_path/find_rule_any; rely on Pest path builders

// Removed naive helpers; rely on Pest AST builders exclusively

fn compile_expr(ast: &ast::Expr, out: &mut InstrSeq, sc: &StaticContext) -> Result<(), Error> {
    match ast {
        ast::Expr::Literal(l) => match l {
            ast::Literal::Integer(v) => out.0.push(OpCode::PushAtomic(XdmAtomicValue::Integer(*v))),
            ast::Literal::Double(v) => out.0.push(OpCode::PushAtomic(XdmAtomicValue::Double(*v))),
            ast::Literal::String(s) => out.0.push(OpCode::PushAtomic(XdmAtomicValue::String(s.clone()))),
            ast::Literal::EmptySequence => { /* no-op: empty */ }
            _ => return Err(Error::static_err("err:XPST0017", "literal type not supported in M1")),
        },
        ast::Expr::Binary { left, op, right } => {
            use ast::BinaryOp::*;
            match op {
                And => {
                    // Short-circuit AND
                    compile_expr(left, out, sc)?;
                    out.0.push(OpCode::ToEBV);
                    let jmp_idx = out.0.len();
                    out.0.push(OpCode::JumpIfFalse(0)); // to be patched
                    out.0.push(OpCode::Pop);
                    compile_expr(right, out, sc)?;
                    out.0.push(OpCode::ToEBV);
                    let end = out.0.len();
                    if let OpCode::JumpIfFalse(ref mut off) = out.0[jmp_idx] {
                        *off = end - jmp_idx;
                    }
                }
                Or => {
                    // Short-circuit OR
                    compile_expr(left, out, sc)?;
                    out.0.push(OpCode::ToEBV);
                    let jmp_idx = out.0.len();
                    out.0.push(OpCode::JumpIfTrue(0)); // to be patched
                    out.0.push(OpCode::Pop);
                    compile_expr(right, out, sc)?;
                    out.0.push(OpCode::ToEBV);
                    let end = out.0.len();
                    if let OpCode::JumpIfTrue(ref mut off) = out.0[jmp_idx] {
                        *off = end - jmp_idx;
                    }
                }
                _ => {
                    compile_expr(left, out, sc)?;
                    compile_expr(right, out, sc)?;
                    match op {
                        Add => out.0.push(OpCode::Add),
                        Sub => out.0.push(OpCode::Sub),
                        Mul => out.0.push(OpCode::Mul),
                        Div => out.0.push(OpCode::Div),
                        IDiv => out.0.push(OpCode::IDiv),
                        Mod => out.0.push(OpCode::Mod),
                        And | Or => unreachable!(),
                    }
                }
            }
        }
        ast::Expr::GeneralComparison { left, op, right } => {
            compile_expr(left, out, sc)?;
            compile_expr(right, out, sc)?;
            let code = match op {
                ast::GeneralComp::Eq => 0,
                ast::GeneralComp::Ne => 1,
                ast::GeneralComp::Lt => 2,
                ast::GeneralComp::Le => 3,
                ast::GeneralComp::Gt => 4,
                ast::GeneralComp::Ge => 5,
            };
            out.0.push(OpCode::CompareGeneral(code));
        }
        ast::Expr::NodeComparison { left, op, right } => {
            compile_expr(left, out, sc)?;
            compile_expr(right, out, sc)?;
            match op {
                ast::NodeComp::Is => out.0.push(OpCode::NodeIs),
                ast::NodeComp::Precedes => out.0.push(OpCode::NodeBefore),
                ast::NodeComp::Follows => out.0.push(OpCode::NodeAfter),
            }
        }
        ast::Expr::ValueComparison { left, op, right } => {
            compile_expr(left, out, sc)?;
            compile_expr(right, out, sc)?;
            let code = match op {
                ast::ValueComp::Eq => 0,
                ast::ValueComp::Ne => 1,
                ast::ValueComp::Lt => 2,
                ast::ValueComp::Le => 3,
                ast::ValueComp::Gt => 4,
                ast::ValueComp::Ge => 5,
            };
            out.0.push(OpCode::CompareValue(code));
        }
        ast::Expr::Sequence(items) => {
            for it in items {
                compile_expr(it, out, sc)?;
            }
            out.0.push(OpCode::MakeSeq(items.len()));
        }
        ast::Expr::FunctionCall { name, args } => {
            for a in args { compile_expr(a, out, sc)?; }
            let local = if let Some(p) = &name.prefix { format!("{}:{}", p, name.local) } else { name.local.clone() };
            let q = ExpandedName { ns_uri: name.ns_uri.clone(), local };
            out.0.push(OpCode::CallByName(q, args.len()));
        }
        ast::Expr::ContextItem => {
            out.0.push(OpCode::LoadContextItem);
        }
        ast::Expr::Path(px) => {
            use ast::PathStart as PS;
            match px.start {
                PS::Relative => {
                    out.0.push(OpCode::LoadContextItem);
                }
                PS::Root => {
                    out.0.push(OpCode::LoadContextItem);
                    out.0.push(OpCode::ToRoot);
                }
                PS::RootDescendant => {
                    out.0.push(OpCode::LoadContextItem);
                    out.0.push(OpCode::ToRoot);
                    out.0.push(OpCode::AxisStep(AxisIR::DescendantOrSelf, NodeTestIR::AnyKind, vec![]));
                }
            }
            for s in &px.steps {
                let axis = match s.axis {
                    ast::Axis::Child => AxisIR::Child,
                    ast::Axis::Attribute => AxisIR::Attribute,
                    ast::Axis::SelfAxis => AxisIR::SelfAxis,
                    ast::Axis::DescendantOrSelf => AxisIR::DescendantOrSelf,
                    ast::Axis::Descendant => AxisIR::Descendant,
                    ast::Axis::Parent => AxisIR::Parent,
                    ast::Axis::Ancestor => AxisIR::Ancestor,
                    ast::Axis::AncestorOrSelf => AxisIR::AncestorOrSelf,
                    ast::Axis::PrecedingSibling => AxisIR::PrecedingSibling,
                    ast::Axis::FollowingSibling => AxisIR::FollowingSibling,
                    ast::Axis::Preceding => AxisIR::Preceding,
                    ast::Axis::Following => AxisIR::Following,
                    ast::Axis::Namespace => AxisIR::Namespace,
                };
                let test = match &s.test {
                    ast::NodeTest::Kind(ast::KindTest::AnyKind) => NodeTestIR::AnyKind,
                    ast::NodeTest::Kind(ast::KindTest::Text) => NodeTestIR::KindText,
                    ast::NodeTest::Name(ast::NameTest::QName(qn)) => {
                        let ns_uri = match (&qn.prefix, axis.clone()) {
                            (Some(pref), _) => sc.namespaces.by_prefix.get(pref).cloned(),
                            (None, AxisIR::Attribute) => None,
                            (None, _) => sc.namespaces.by_prefix.get("").cloned(),
                        };
                        NodeTestIR::Name(ExpandedName { ns_uri, local: qn.local.clone() })
                    }
                    ast::NodeTest::Name(ast::NameTest::Wildcard(ast::WildcardName::Any)) => NodeTestIR::WildcardAny,
                    ast::NodeTest::Name(ast::NameTest::Wildcard(ast::WildcardName::NsWildcard(ns))) => NodeTestIR::NsWildcard(ns.clone()),
                    ast::NodeTest::Name(ast::NameTest::Wildcard(ast::WildcardName::LocalWildcard(local))) => NodeTestIR::LocalWildcard(local.clone()),
                    _ => return Err(Error::static_err("err:XPST0017", "node test not supported in M3")),
                };
                let mut preds_ir: Vec<InstrSeq> = Vec::new();
                for p in &s.predicates {
                    let mut seq = InstrSeq::default();
                    compile_expr(p, &mut seq, sc)?;
                    preds_ir.push(seq);
                }
                out.0.push(OpCode::AxisStep(axis, test, preds_ir));
            }
        }
        ast::Expr::SetOp { left, op, right } => {
            compile_expr(left, out, sc)?;
            compile_expr(right, out, sc)?;
            match op {
                ast::SetOp::Union => out.0.push(OpCode::Union),
                ast::SetOp::Intersect => out.0.push(OpCode::Intersect),
                ast::SetOp::Except => out.0.push(OpCode::Except),
            }
        }
        ast::Expr::Range { start, end } => {
            compile_expr(start, out, sc)?;
            compile_expr(end, out, sc)?;
            out.0.push(OpCode::RangeTo);
        }
        _ => return Err(Error::static_err("err:XPST0017", "expression not supported in M1")),
    }
    Ok(())
}

impl fmt::Display for OpCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use OpCode::*;
        match self {
            PushAtomic(a) => write!(f, "PUSH {:?}", a),
            LoadVar(i) => write!(f, "LOAD_VAR {}", i),
            StoreVar(i) => write!(f, "STORE_VAR {}", i),
            LoadContextItem => write!(f, "LOAD_CONTEXT_ITEM"),
            Position => write!(f, "POSITION"),
            Last => write!(f, "LAST"),
            ToRoot => write!(f, "TO_ROOT"),
            AxisStep(axis, nt, preds) => write!(f, "AXIS_STEP {:?} {:?} preds:{}", axis, nt, preds.len()),
            PredicateStart => write!(f, "PREDICATE_START"),
            PredicateEnd => write!(f, "PREDICATE_END"),
            Add => write!(f, "ADD"),
            Sub => write!(f, "SUB"),
            Mul => write!(f, "MUL"),
            Div => write!(f, "DIV"),
            IDiv => write!(f, "IDIV"),
            Mod => write!(f, "MOD"),
            And => write!(f, "AND"),
            Or => write!(f, "OR"),
            Not => write!(f, "NOT"),
            ToEBV => write!(f, "TO_EBV"),
            Pop => write!(f, "POP"),
            JumpIfTrue(o) => write!(f, "JMP_IF_TRUE +{}", o),
            JumpIfFalse(o) => write!(f, "JMP_IF_FALSE +{}", o),
            CompareValue(op) => write!(f, "CMP_VALUE {}", op),
            CompareGeneral(op) => write!(f, "CMP_GENERAL {}", op),
            NodeIs => write!(f, "NODE_IS"),
            NodeBefore => write!(f, "NODE_BEFORE"),
            NodeAfter => write!(f, "NODE_AFTER"),
            MakeSeq(n) => write!(f, "MAKE_SEQ {}", n),
            ConcatSeq => write!(f, "CONCAT_SEQ"),
            Union => write!(f, "UNION"),
            Intersect => write!(f, "INTERSECT"),
            Except => write!(f, "EXCEPT"),
            RangeTo => write!(f, "RANGE_TO"),
            IfElse => write!(f, "IF_ELSE"),
            Some => write!(f, "SOME"),
            Every => write!(f, "EVERY"),
            ForStart => write!(f, "FOR_START"),
            ForNext => write!(f, "FOR_NEXT"),
            ForEnd => write!(f, "FOR_END"),
            LetBind => write!(f, "LET_BIND"),
            Cast => write!(f, "CAST"),
            Castable => write!(f, "CASTABLE"),
            Treat => write!(f, "TREAT"),
            InstanceOf => write!(f, "INSTANCE_OF"),
            Call(id, argc) => write!(f, "CALL {} {}", id, argc),
            CallByName(name, argc) => write!(f, "CALL_BY_NAME {:?} {}", name, argc),
        }
    }
}

impl fmt::Display for InstrSeq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, op) in self.0.iter().enumerate() {
            writeln!(f, "{:04}: {}", i, op)?;
        }
        Ok(())
    }
}
