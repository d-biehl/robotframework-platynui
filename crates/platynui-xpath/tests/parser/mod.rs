//! Test modules for XPath 2.0 parser
//! Tests are organized by functionality for better maintainability

use rstest::rstest;

// Re-export common test utilities
pub use self::test_utils::*;

pub mod additional_cases;
pub mod ast_structure;
pub mod basic_syntax;
pub mod complex_expressions;
pub mod functions;
pub mod invalid_expressions;
pub mod namespaces;
pub mod operators;
pub mod path_expressions;
pub mod performance_and_edge_cases;
pub mod predicates;
pub mod string_literals;
mod test_utils;
pub mod web_automation;
pub mod xpath2_compliance;
pub mod xpath2_features;
