pub mod parser;

// Public API surface for the upcoming Parser → Compiler → Evaluator pipeline.
// The parser is already available via `parser`.
// Below we expose stubs for Static/Dynamic contexts, XDM types, and the
// compilation/evaluation entry points as described in docs/xpath20-evaluator-plan.md.

pub mod model;
pub mod xdm;
pub mod runtime;
pub mod compiler;
pub mod evaluator;
pub mod functions;

pub use evaluator::compile_xpath;
pub use evaluator::XPathExecutable;
pub use runtime::{DynamicContext, DynamicContextBuilder};
pub use model::{NodeKind, QName, XdmNode};
pub use xdm::{ExpandedName, XdmItem, XdmSequence};
