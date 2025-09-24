mod pointer;
pub mod provider;
pub mod runtime;
mod xpath;

pub use pointer::PointerError;
pub use runtime::{FocusError, Runtime};
pub use xpath::{
    EvaluateError, EvaluateOptions, EvaluatedAttribute, EvaluationItem, NodeResolver, evaluate,
};
