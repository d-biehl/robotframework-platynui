//! Function families per XQuery and XPath Functions and Operators.
//! For now, this module only exposes a hook to create a default registry.

use crate::runtime::{FunctionRegistry};

pub fn default_function_registry<N>() -> FunctionRegistry<N> {
    // Will be populated with standard functions (string, numeric, sequence, node, regex, date/time, aggregate)
    FunctionRegistry::default()
}

