pub mod compiler;
pub mod parser;
pub mod engine;
pub mod model;
pub mod xdm;
pub mod util;

// Back-compat public surface for existing tests and examples
pub use compiler::{compile_xpath, compile_xpath_with_context};
pub use engine::evaluator::{evaluate, evaluate_expr};
pub use engine::runtime::{
    DynamicContext, DynamicContextBuilder, StaticContext, StaticContextBuilder,
};
pub use model::{NodeKind, QName, XdmNode};
pub use xdm::{ExpandedName, XdmItem, XdmSequence, XdmAtomicValue};
pub use model::simple::{
    SimpleNode, SimpleNodeBuilder, attr, elem, text, ns, doc as simple_doc,
};

// Lightweight forwarding modules to ease transition
pub mod runtime { pub use crate::engine::runtime::*; }
pub mod evaluator { pub use crate::engine::evaluator::*; }
pub mod functions { pub use crate::engine::functions::*; }
pub mod collation { pub use crate::engine::collation::*; }
pub mod simple_node { pub use crate::model::simple::*; }
