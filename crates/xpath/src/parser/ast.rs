use compact_str::CompactString;
use rust_decimal::Decimal;

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Integer(i64),
    Decimal(Decimal),
    Double(f64),
    String(CompactString),
    Boolean(bool),
    AnyUri(CompactString),
    UntypedAtomic(CompactString),
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnarySign {
    Plus,
    Minus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    IDiv,
    Mod,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GeneralComp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueComp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeComp {
    Is,
    Precedes,
    Follows,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QName {
    pub prefix: Option<String>,
    pub local: String,
    pub ns_uri: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Parenthesized(Box<Expr>),
    VarRef(QName),
    FunctionCall { name: QName, args: Vec<Expr> },
    Filter { input: Box<Expr>, predicates: Vec<Expr> },
    Sequence(Vec<Expr>),
    Binary { left: Box<Expr>, op: BinaryOp, right: Box<Expr> },
    GeneralComparison { left: Box<Expr>, op: GeneralComp, right: Box<Expr> },
    ValueComparison { left: Box<Expr>, op: ValueComp, right: Box<Expr> },
    NodeComparison { left: Box<Expr>, op: NodeComp, right: Box<Expr> },
    Unary { sign: UnarySign, expr: Box<Expr> },
    IfThenElse { cond: Box<Expr>, then_expr: Box<Expr>, else_expr: Box<Expr> },
    Range { start: Box<Expr>, end: Box<Expr> },
    InstanceOf { expr: Box<Expr>, ty: SequenceType },
    TreatAs { expr: Box<Expr>, ty: SequenceType },
    CastableAs { expr: Box<Expr>, ty: SingleType },
    CastAs { expr: Box<Expr>, ty: SingleType },
    ContextItem, // .
    Path(PathExpr),
    PathFrom { base: Box<Expr>, steps: Vec<Step> },
    Quantified { kind: Quantifier, bindings: Vec<QuantifiedBinding>, satisfies: Box<Expr> },
    ForExpr { bindings: Vec<ForBinding>, return_expr: Box<Expr> },
    LetExpr { bindings: Vec<LetBinding>, return_expr: Box<Expr> },
    SetOp { left: Box<Expr>, op: SetOp, right: Box<Expr> },
}

impl Expr {
    /// Returns whether evaluating this expression reads its context — the context item, its
    /// position or its size — so that the result depends on which node it is evaluated against.
    ///
    /// The context is read by a relative path (`.//x`, `child::x`), by the context item (`.`), and
    /// by a function that falls back to the context when an argument is left out (`name()`,
    /// `string()`, `root()`, `lang('en')`, `id('a')`) or that reads the position or the size
    /// (`position()`, `last()`). The check looks into every operand, condition, binding and
    /// function argument, so `count(.//x)`, `1 + count(.//x)` and
    /// `if (exists(.//x)) then //a else //b` are dependent, and `count(//x)` is not.
    ///
    /// It does not look where the focus changes: a predicate and every step of a path after the
    /// first evaluate with their own focus, so `//x[.='y']` and `//x/string()` are independent.
    /// Absolute paths (`/x`, `//x`) are independent as well: they start at the root of the tree,
    /// whichever of its nodes is the context.
    ///
    /// This drives `Set Root`, where a dependent selector drills into the current root and an
    /// independent one starts fresh from the desktop, and it tells a keyword whether it has to look
    /// its root up at all.
    #[must_use]
    pub fn is_context_dependent(&self) -> bool {
        match self {
            Expr::ContextItem => true,
            // A relative path starts with an axis step: the parser turns one that starts with an
            // expression into `PathFrom`. The steps after the first evaluate with their own focus.
            Expr::Path(path) => matches!(path.start, PathStart::Relative),
            Expr::PathFrom { base, .. } | Expr::Filter { input: base, .. } => base.is_context_dependent(),
            Expr::FunctionCall { name, args } => {
                reads_context_by_default(name, args.len()) || args.iter().any(Expr::is_context_dependent)
            }
            Expr::Parenthesized(inner)
            | Expr::Unary { expr: inner, .. }
            | Expr::InstanceOf { expr: inner, .. }
            | Expr::TreatAs { expr: inner, .. }
            | Expr::CastableAs { expr: inner, .. }
            | Expr::CastAs { expr: inner, .. } => inner.is_context_dependent(),
            Expr::Binary { left, right, .. }
            | Expr::GeneralComparison { left, right, .. }
            | Expr::ValueComparison { left, right, .. }
            | Expr::NodeComparison { left, right, .. }
            | Expr::SetOp { left, right, .. }
            | Expr::Range { start: left, end: right } => left.is_context_dependent() || right.is_context_dependent(),
            Expr::IfThenElse { cond, then_expr, else_expr } => {
                cond.is_context_dependent() || then_expr.is_context_dependent() || else_expr.is_context_dependent()
            }
            Expr::ForExpr { bindings, return_expr } => {
                bindings.iter().any(|binding| binding.in_expr.is_context_dependent())
                    || return_expr.is_context_dependent()
            }
            Expr::LetExpr { bindings, return_expr } => {
                bindings.iter().any(|binding| binding.value.is_context_dependent())
                    || return_expr.is_context_dependent()
            }
            Expr::Quantified { bindings, satisfies, .. } => {
                bindings.iter().any(|binding| binding.in_expr.is_context_dependent())
                    || satisfies.is_context_dependent()
            }
            Expr::Sequence(items) => items.iter().any(Expr::is_context_dependent),
            Expr::Literal(_) | Expr::VarRef(_) => false,
        }
    }
}

/// Whether the standard function `name`, called with `arity` arguments, reads the context: the
/// forms whose left-out argument defaults to the context item, and `position()` and `last()`,
/// which read the context position and size.
fn reads_context_by_default(name: &QName, arity: usize) -> bool {
    if !matches!(name.prefix.as_deref(), None | Some("fn")) {
        return false;
    }
    let local = name.local.as_str();
    match arity {
        0 => matches!(
            local,
            "position"
                | "last"
                | "data"
                | "number"
                | "string"
                | "string-length"
                | "normalize-space"
                | "name"
                | "local-name"
                | "namespace-uri"
                | "root"
                | "base-uri"
                | "document-uri"
        ),
        1 => matches!(local, "lang" | "id" | "element-with-id" | "idref"),
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetOp {
    Union,
    Intersect,
    Except,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Quantifier {
    Some,
    Every,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuantifiedBinding {
    pub var: QName,
    pub in_expr: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForBinding {
    pub var: QName,
    pub in_expr: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LetBinding {
    pub var: QName,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PathStart {
    Root,
    RootDescendant,
    Relative,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PathExpr {
    pub start: PathStart,
    pub steps: Vec<Step>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Axis {
    Child,
    Descendant,
    Attribute,
    SelfAxis,
    DescendantOrSelf,
    FollowingSibling,
    Following,
    Namespace,
    Parent,
    Ancestor,
    PrecedingSibling,
    Preceding,
    AncestorOrSelf,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Axis { axis: Axis, test: NodeTest, predicates: Vec<Expr> },
    FilterExpr(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeTest {
    Name(NameTest),
    Kind(KindTest),
}

#[derive(Debug, Clone, PartialEq)]
pub enum NameTest {
    QName(QName),
    Wildcard(WildcardName),
}

#[derive(Debug, Clone, PartialEq)]
pub enum WildcardName {
    Any,
    NsWildcard(String),
    LocalWildcard(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum KindTest {
    AnyKind,
    Document(Option<Box<KindTest>>),
    Text,
    Comment,
    ProcessingInstruction(Option<String>),
    Element { name: Option<ElementNameOrWildcard>, ty: Option<TypeName>, nillable: bool },
    Attribute { name: Option<AttributeNameOrWildcard>, ty: Option<TypeName> },
    SchemaElement(QName),
    SchemaAttribute(QName),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ElementNameOrWildcard {
    Name(QName),
    Any,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttributeNameOrWildcard {
    Name(QName),
    Any,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TypeName(pub QName);

#[derive(Debug, Clone, PartialEq)]
pub struct SingleType {
    pub atomic: QName,
    pub optional: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Occurrence {
    One,
    ZeroOrOne,
    ZeroOrMore,
    OneOrMore,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ItemType {
    Kind(KindTest),
    Item,
    Atomic(QName),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SequenceType {
    EmptySequence,
    Typed { item: ItemType, occ: Occurrence },
}
