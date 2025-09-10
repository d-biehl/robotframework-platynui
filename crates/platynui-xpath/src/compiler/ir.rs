use crate::runtime::StaticContext;
use crate::xdm::{ExpandedName, XdmAtomicValue};
use std::sync::Arc;

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
    Name(ExpandedName),
    Any,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeTestIR {
    // name tests
    AnyKind,                    // node()
    Name(ExpandedName),         // QName
    WildcardAny,                // *
    NsWildcard(String),         // ns:*
    LocalWildcard(String),      // *:local

    // kind tests
    KindText,                                       // text()
    KindComment,                                    // comment()
    KindProcessingInstruction(Option<String>),      // processing-instruction('target'?)
    KindDocument(Option<Box<NodeTestIR>>),          // document-node(element(...)? | schema-element(...)? | comment() | processing-instruction() | text())
    KindElement {                                   // element(QName? , Type? , nillable?)
        name: Option<NameOrWildcard>,
        ty: Option<ExpandedName>,
        nillable: bool,
    },
    KindAttribute {                                 // attribute(QName? , Type?)
        name: Option<NameOrWildcard>,
        ty: Option<ExpandedName>,
    },
    KindSchemaElement(ExpandedName),                // schema-element(QName)
    KindSchemaAttribute(ExpandedName),              // schema-attribute(QName)
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
    Dup,    // duplicate TOS
    Swap,   // swap top two stack items

    // Steps / filters
    AxisStep(AxisIR, NodeTestIR, Vec<InstrSeq>),
    // Apply n predicates to TOS sequence; each predicate is a separate InstrSeq.
    ApplyPredicates(Vec<InstrSeq>),
    // Ensure document order and no duplicates for a node sequence
    DocOrderDistinct,

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

    // Quantifiers and iteration
    ForStartByName(ExpandedName),
    ForNext,
    ForEnd,
    // Quantified expressions over a sequence on TOS
    QuantStartByName(QuantifierKind, ExpandedName),
    QuantEnd,

    // Types
    Cast(SingleTypeIR),
    Castable(SingleTypeIR),
    Treat(SeqTypeIR),
    InstanceOf(SeqTypeIR),

    // Functions
    CallByName(ExpandedName, usize /* argc */),
    // Errors
    Raise(&'static str), // raise a dynamic error code (e.g., "err:XPTY0004")
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstrSeq(pub Vec<OpCode>);

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum QuantifierKind {
    Some,
    Every,
}
