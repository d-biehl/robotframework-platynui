pub mod compiler;
pub(crate) mod consts;
pub mod engine;
pub mod model;
pub mod parser;
pub mod util;
pub mod xdm;

// Back-compat public surface for existing tests and examples
pub use compiler::{compile, compile_with_context};
pub use engine::evaluator::{
    evaluate, evaluate_expr, evaluate_first, evaluate_first_expr, evaluate_stream, evaluate_stream_expr,
};
pub use engine::runtime::{DynamicContext, DynamicContextBuilder, StaticContext, StaticContextBuilder};
pub use model::simple::{SimpleNode, SimpleNodeBuilder, attr, doc as simple_doc, elem, ns, text};
pub use model::{NodeKind, QName, XdmNode};
pub use xdm::{ExpandedName, XdmAtomicValue, XdmItem, XdmSequence};

// Lightweight forwarding modules to ease transition
pub mod runtime {
    pub use crate::engine::runtime::*;
}
pub mod evaluator {
    pub use crate::engine::evaluator::*;
}
pub mod functions {
    pub use crate::engine::functions::*;
}
pub mod collation {
    pub use crate::engine::collation::*;
}
pub mod simple_node {
    pub use crate::model::simple::*;
}
