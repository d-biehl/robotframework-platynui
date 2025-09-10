pub mod compiler;
pub mod functions;
pub mod model;
pub mod parser;
pub mod runtime;
pub mod xdm;
pub mod evaluator;
pub mod simple_node;

pub use model::{NodeKind, QName, XdmNode};
pub use parser::XPathParseError;
pub use parser::XPathParser;
pub use parser::parse_xpath;
pub use compiler::compile_xpath;
pub use runtime::{StaticContext, DynamicContext, DynamicContextBuilder};
pub use xdm::{ExpandedName, XdmItem, XdmSequence};
pub use evaluator::{evaluate, evaluate_expr};
pub use simple_node::{SimpleNode, SimpleNodeBuilder, elem, text, attr, ns, doc as simple_doc};
