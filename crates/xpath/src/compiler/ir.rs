use crate::engine::runtime::StaticContext;
use crate::xdm::{ExpandedName, XdmAtomicValue};
use core::fmt;
use std::sync::Arc;
use string_cache::DefaultAtom;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InternedQName {
    pub original: ExpandedName,
    pub local: DefaultAtom,
    pub ns_uri: Option<DefaultAtom>,
}

impl InternedQName {
    #[must_use]
    pub fn from_expanded(expanded: ExpandedName) -> Self {
        let local = DefaultAtom::from(expanded.local.as_str());
        let ns_uri = expanded.ns_uri.as_ref().map(|uri| DefaultAtom::from(uri.as_str()));
        Self { original: expanded, local, ns_uri }
    }
}

impl fmt::Display for InternedQName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.original)
    }
}

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub enum NameOrWildcard {
    Name(InternedQName),
    Any,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeTestIR {
    // name tests
    AnyKind,                    // node()
    Name(InternedQName),        // QName
    WildcardAny,                // *
    NsWildcard(DefaultAtom),    // ns:*
    LocalWildcard(DefaultAtom), // *:local

    // kind tests
    KindText,                                  // text()
    KindComment,                               // comment()
    KindProcessingInstruction(Option<String>), // processing-instruction('target'?)
    KindDocument(Option<Box<NodeTestIR>>), // document-node(element(...)? | schema-element(...)? | comment() | processing-instruction() | text())
    KindElement {
        // element(QName? , Type? , nillable?)
        name: Option<NameOrWildcard>,
        ty: Option<ExpandedName>,
        nillable: bool,
    },
    KindAttribute {
        // attribute(QName? , Type?)
        name: Option<NameOrWildcard>,
        ty: Option<ExpandedName>,
    },
    KindSchemaElement(ExpandedName),   // schema-element(QName)
    KindSchemaAttribute(ExpandedName), // schema-attribute(QName)
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpCode {
    // Data and variables
    PushAtomic(XdmAtomicValue),
    LoadVarByName(ExpandedName),
    LoadContextItem,
    Position,
    Last,
    ToRoot,

    // Stack helpers
    Dup,  // duplicate TOS
    Swap, // swap top two stack items

    // Steps / filters
    // An axis step over the TOS node stream. Its predicates run per context node when one may count
    // positions; a child step merges the children of nested context nodes in document order.
    AxisStep(AxisIR, NodeTestIR, Vec<PredicateIR>),
    PathExprStep(InstrSeq),
    // Apply n predicates to the whole TOS sequence, in order.
    ApplyPredicates(Vec<PredicateIR>),
    // Sort the TOS sequence into document order and remove duplicate nodes (materializes)
    Normalize,
    // Reverse the TOS sequence (materializes): a reverse axis from one context node, in document order
    Reverse,
    // Remove duplicate nodes while preserving order (should be streaming on doc-ordered input)
    EnsureDistinct,

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
    // Atomize items according to XPath 2.0 atomization rules
    Atomize,
    Pop,
    JumpIfTrue(usize),  // relative forward
    JumpIfFalse(usize), // relative forward
    Jump(usize),        // relative forward (unconditional)

    // Comparisons
    CompareValue(ComparisonOp),
    CompareGeneral(ComparisonOp),
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
    // Enter/leave a new local scope with given number of slots
    BeginScope(usize),
    EndScope,
    LetStartByName(ExpandedName),
    LetEnd,

    // Quantifiers and iteration
    // For-expression loop over sequence on TOS
    ForLoop { var: ExpandedName, body: InstrSeq },
    // Quantified expression loop over sequence on TOS
    QuantLoop { kind: QuantifierKind, var: ExpandedName, body: InstrSeq },

    // Types
    Cast(SingleTypeIR),
    Castable(SingleTypeIR),
    Treat(SeqTypeIR),
    InstanceOf(SeqTypeIR),

    // Functions
    CallByName(ExpandedName, usize),
    // Errors
    Raise(&'static str), // raise a dynamic error code (e.g., "err:XPTY0004")
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstrSeq(pub Vec<OpCode>);

/// Whether a predicate may count positions, decided when it is lowered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PredicateKind {
    /// Provably a boolean or nodes, reading neither `position()` nor `last()` in its own focus. It
    /// selects the same items per context node as over a whole sequence, so it may move into a
    /// step and lets the step merge its context nodes.
    NonPositional,
    /// Anything else. It counts per context node inside a step and over the whole sequence on a
    /// parenthesized expression, so it stays where it was written.
    PossiblyPositional,
}

/// One predicate of a step or of a filter: its code and whether it may count positions.
#[derive(Debug, Clone, PartialEq)]
pub struct PredicateIR {
    pub code: InstrSeq,
    pub kind: PredicateKind,
}

impl fmt::Display for PredicateKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PredicateKind::NonPositional => write!(f, "non-positional"),
            PredicateKind::PossiblyPositional => write!(f, "possibly positional"),
        }
    }
}

fn predicate_kinds(predicates: &[PredicateIR]) -> String {
    predicates.iter().map(|p| p.kind.to_string()).collect::<Vec<_>>().join(", ")
}

#[derive(Debug, Clone)]
pub struct CompiledXPath {
    pub instrs: InstrSeq,
    pub static_ctx: Arc<StaticContext>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SingleTypeIR {
    pub atomic: ExpandedName,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemTypeIR {
    AnyItem,
    Atomic(ExpandedName),
    Kind(NodeTestIR),
    // Convenience for any node() kind
    AnyNode,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OccurrenceIR {
    One,
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SeqTypeIR {
    EmptySequence,
    Typed { item: ItemTypeIR, occ: OccurrenceIR },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ComparisonOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl From<&crate::parser::ast::GeneralComp> for ComparisonOp {
    fn from(op: &crate::parser::ast::GeneralComp) -> Self {
        use crate::parser::ast::GeneralComp;
        match op {
            GeneralComp::Eq => Self::Eq,
            GeneralComp::Ne => Self::Ne,
            GeneralComp::Lt => Self::Lt,
            GeneralComp::Le => Self::Le,
            GeneralComp::Gt => Self::Gt,
            GeneralComp::Ge => Self::Ge,
        }
    }
}

impl From<&crate::parser::ast::ValueComp> for ComparisonOp {
    fn from(op: &crate::parser::ast::ValueComp) -> Self {
        use crate::parser::ast::ValueComp;
        match op {
            ValueComp::Eq => Self::Eq,
            ValueComp::Ne => Self::Ne,
            ValueComp::Lt => Self::Lt,
            ValueComp::Le => Self::Le,
            ValueComp::Gt => Self::Gt,
            ValueComp::Ge => Self::Ge,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuantifierKind {
    Some,
    Every,
}

// -----------------------------
// Pretty Display implementations
// -----------------------------

impl fmt::Display for AxisIR {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            AxisIR::Child => "child",
            AxisIR::Attribute => "attribute",
            AxisIR::SelfAxis => "self",
            AxisIR::DescendantOrSelf => "descendant-or-self",
            AxisIR::Descendant => "descendant",
            AxisIR::Parent => "parent",
            AxisIR::Ancestor => "ancestor",
            AxisIR::AncestorOrSelf => "ancestor-or-self",
            AxisIR::PrecedingSibling => "preceding-sibling",
            AxisIR::FollowingSibling => "following-sibling",
            AxisIR::Preceding => "preceding",
            AxisIR::Following => "following",
            AxisIR::Namespace => "namespace",
        };
        write!(f, "{s}")
    }
}

impl fmt::Display for NameOrWildcard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NameOrWildcard::Name(n) => write!(f, "{n}"),
            NameOrWildcard::Any => write!(f, "*"),
        }
    }
}

impl fmt::Display for NodeTestIR {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeTestIR::AnyKind => write!(f, "node()"),
            NodeTestIR::Name(n) => write!(f, "{n}"),
            NodeTestIR::WildcardAny => write!(f, "*"),
            NodeTestIR::NsWildcard(ns) => write!(f, "{ns}:*"),
            NodeTestIR::LocalWildcard(local) => write!(f, "*:{local}"),

            NodeTestIR::KindText => write!(f, "text()"),
            NodeTestIR::KindComment => write!(f, "comment()"),
            NodeTestIR::KindProcessingInstruction(None) => write!(f, "processing-instruction()"),
            NodeTestIR::KindProcessingInstruction(Some(t)) => {
                write!(f, "processing-instruction('{t}')")
            }
            NodeTestIR::KindDocument(None) => write!(f, "document-node()"),
            NodeTestIR::KindDocument(Some(inner)) => write!(f, "document-node({inner})"),
            NodeTestIR::KindElement { name, ty, nillable } => {
                write!(f, "element(")?;
                if let Some(n) = name {
                    write!(f, "{n}")?;
                }
                if let Some(t) = ty {
                    if name.is_some() {
                        write!(f, ", ")?;
                    }
                    write!(f, "{t}")?;
                }
                if *nillable {
                    if name.is_some() || ty.is_some() {
                        write!(f, ", ")?;
                    }
                    write!(f, "nillable")?;
                }
                write!(f, ")")
            }
            NodeTestIR::KindAttribute { name, ty } => {
                write!(f, "attribute(")?;
                if let Some(n) = name {
                    write!(f, "{n}")?;
                }
                if let Some(t) = ty {
                    if name.is_some() {
                        write!(f, ", ")?;
                    }
                    write!(f, "{t}")?;
                }
                write!(f, ")")
            }
            NodeTestIR::KindSchemaElement(n) => write!(f, "schema-element({n})"),
            NodeTestIR::KindSchemaAttribute(n) => write!(f, "schema-attribute({n})"),
        }
    }
}

impl fmt::Display for ComparisonOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ComparisonOp::Eq => "=",
            ComparisonOp::Ne => "!=",
            ComparisonOp::Lt => "<",
            ComparisonOp::Le => "<=",
            ComparisonOp::Gt => ">",
            ComparisonOp::Ge => ">=",
        };
        write!(f, "{s}")
    }
}

impl fmt::Display for QuantifierKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuantifierKind::Some => write!(f, "some"),
            QuantifierKind::Every => write!(f, "every"),
        }
    }
}

impl fmt::Display for OccurrenceIR {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            OccurrenceIR::One => "",
            OccurrenceIR::ZeroOrOne => "?",
            OccurrenceIR::ZeroOrMore => "*",
            OccurrenceIR::OneOrMore => "+",
        };
        write!(f, "{s}")
    }
}

impl fmt::Display for SingleTypeIR {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.optional { write!(f, "{}?", self.atomic) } else { write!(f, "{}", self.atomic) }
    }
}

impl fmt::Display for ItemTypeIR {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ItemTypeIR::AnyItem => write!(f, "item()"),
            ItemTypeIR::Atomic(n) => write!(f, "{n}"),
            ItemTypeIR::Kind(k) => write!(f, "{k}"),
            ItemTypeIR::AnyNode => write!(f, "node()"),
        }
    }
}

impl fmt::Display for SeqTypeIR {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SeqTypeIR::EmptySequence => write!(f, "empty-sequence()"),
            SeqTypeIR::Typed { item, occ } => {
                write!(f, "{item}{occ}")
            }
        }
    }
}

impl fmt::Display for OpCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Data and variables
            OpCode::PushAtomic(v) => write!(f, "push {v}"),
            OpCode::LoadVarByName(n) => write!(f, "load-var {n}"),
            OpCode::LoadContextItem => write!(f, "load ."),
            OpCode::Position => write!(f, "position"),
            OpCode::Last => write!(f, "last"),
            OpCode::ToRoot => write!(f, "to-root"),

            // Stack helpers
            OpCode::Dup => write!(f, "dup"),
            OpCode::Swap => write!(f, "swap"),

            // Steps / filters
            OpCode::AxisStep(axis, test, preds) => {
                write!(f, "{axis}::{test}")?;
                if !preds.is_empty() {
                    write!(f, " [predicates: {}]", predicate_kinds(preds))?;
                }
                Ok(())
            }
            OpCode::PathExprStep(_) => write!(f, "path-expr-step"),
            OpCode::ApplyPredicates(preds) => write!(f, "apply-predicates([{}])", predicate_kinds(preds)),
            OpCode::Normalize => write!(f, "normalize"),
            OpCode::Reverse => write!(f, "reverse"),
            OpCode::EnsureDistinct => write!(f, "ensure-distinct"),

            // Arithmetic / logic
            OpCode::Add => write!(f, "+"),
            OpCode::Sub => write!(f, "-"),
            OpCode::Mul => write!(f, "*"),
            OpCode::Div => write!(f, "/"),
            OpCode::IDiv => write!(f, "idiv"),
            OpCode::Mod => write!(f, "mod"),
            OpCode::And => write!(f, "and"),
            OpCode::Or => write!(f, "or"),
            OpCode::Not => write!(f, "not"),
            OpCode::ToEBV => write!(f, "to-ebv"),
            OpCode::Atomize => write!(f, "atomize"),
            OpCode::Pop => write!(f, "pop"),
            OpCode::JumpIfTrue(ofs) => write!(f, "jtrue +{ofs}"),
            OpCode::JumpIfFalse(ofs) => write!(f, "jfalse +{ofs}"),
            OpCode::Jump(ofs) => write!(f, "jump +{ofs}"),

            // Comparisons
            OpCode::CompareValue(op) => write!(f, "compare-value {op}"),
            OpCode::CompareGeneral(op) => write!(f, "compare-general {op}"),
            OpCode::NodeIs => write!(f, "is"),
            OpCode::NodeBefore => write!(f, "node-before"),
            OpCode::NodeAfter => write!(f, "node-after"),

            // Sequences and sets
            OpCode::MakeSeq(n) => write!(f, "make-seq {n}"),
            OpCode::ConcatSeq => write!(f, "concat-seq"),
            OpCode::Union => write!(f, "union"),
            OpCode::Intersect => write!(f, "intersect"),
            OpCode::Except => write!(f, "except"),
            OpCode::RangeTo => write!(f, "range-to"),

            // Control flow / bindings
            OpCode::BeginScope(n) => write!(f, "begin-scope slots={n}"),
            OpCode::EndScope => write!(f, "end-scope"),
            OpCode::LetStartByName(n) => write!(f, "let-start {n}"),
            OpCode::LetEnd => write!(f, "let-end"),

            // Quantifiers and iteration
            OpCode::ForLoop { var, .. } => write!(f, "for ${var} in …"),
            OpCode::QuantLoop { kind, var, .. } => write!(f, "{kind} ${var} in …"),

            // Types
            OpCode::Cast(t) => write!(f, "cast as {t}"),
            OpCode::Castable(t) => write!(f, "castable as {t}"),
            OpCode::Treat(t) => write!(f, "treat as {t}"),
            OpCode::InstanceOf(t) => write!(f, "instance of {t}"),

            // Functions
            OpCode::CallByName(name, arity) => write!(f, "call {name}({arity} args)"),
            // Errors
            OpCode::Raise(code) => write!(f, "raise {code}"),
        }
    }
}

impl InstrSeq {
    fn fmt_with_indent(&self, f: &mut fmt::Formatter<'_>, indent: usize) -> fmt::Result {
        let pad = " ".repeat(indent);
        for (i, op) in self.0.iter().enumerate() {
            // For AxisStep and ApplyPredicates we print additional structure after the line
            match op {
                OpCode::AxisStep(_, _, preds) | OpCode::ApplyPredicates(preds) if !preds.is_empty() => {
                    writeln!(f, "{pad}{i:02}: {op}")?;
                    for (pi, p) in preds.iter().enumerate() {
                        writeln!(f, "{pad}    [pred {pi}: {}]", p.kind)?;
                        p.code.fmt_with_indent(f, indent + 8)?;
                    }
                }
                OpCode::ForLoop { var, body } => {
                    writeln!(f, "{pad}{i:02}: for ${var} in")?;
                    body.fmt_with_indent(f, indent + 4)?;
                }
                OpCode::QuantLoop { kind, var, body } => {
                    writeln!(f, "{pad}{i:02}: {kind} ${var} in")?;
                    body.fmt_with_indent(f, indent + 4)?;
                }
                _ => {
                    writeln!(f, "{pad}{i:02}: {op}")?;
                }
            }
        }
        Ok(())
    }
}

impl fmt::Display for InstrSeq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_with_indent(f, 0)
    }
}

impl fmt::Display for CompiledXPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "XPath: {}", self.source)?;
        writeln!(
            f,
            "Static Context: [base_uri={:?}, default_fn_ns={:?}, default_collation={:?}, prefixes={{{}}}]",
            self.static_ctx.base_uri,
            self.static_ctx.default_function_namespace,
            self.static_ctx.default_collation,
            {
                use smallvec::SmallVec;
                // Namespace prefix sets are usually tiny (handful); keep inline.
                let mut keys: SmallVec<[&String; 8]> = self.static_ctx.namespaces.by_prefix.keys().collect();
                keys.sort();
                let mut parts: SmallVec<[&str; 8]> = SmallVec::with_capacity(keys.len());
                for k in &keys {
                    parts.push(k.as_str());
                }
                parts.join(", ")
            }
        )?;
        writeln!(f, "Instructions:")?;
        self.instrs.fmt_with_indent(f, 2)
    }
}
