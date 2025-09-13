pub mod collation;
pub mod compiler;
pub mod eq;
pub mod evaluator;
pub mod functions;
pub mod model;
pub mod parser;
pub mod runtime;
pub mod simple_node;
pub mod temporal;
pub mod xdm; // EqKey equality kernel

pub use compiler::{compile_xpath, compile_xpath_with_context};
pub use evaluator::{evaluate, evaluate_expr};
pub use model::{NodeKind, QName, XdmNode};
pub use parser::XPathParseError;
pub use parser::XPathParser;
pub use parser::parse_xpath;
pub use runtime::{DynamicContext, DynamicContextBuilder, StaticContext, StaticContextBuilder};
pub use simple_node::{SimpleNode, SimpleNodeBuilder, attr, doc as simple_doc, elem, ns, text};
pub use xdm::{ExpandedName, XdmItem, XdmSequence};
