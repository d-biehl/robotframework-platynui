use compact_str::CompactString;

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    Integer(i64),
    Decimal(f64),
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
