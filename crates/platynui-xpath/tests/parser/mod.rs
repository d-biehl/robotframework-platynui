//! Test modules for XPath 2.0 parser
//! Tests are organized by functionality for better maintainability

use rstest::rstest;

// Re-export common test utilities
pub use self::test_utils::*;

mod test_utils;
pub mod basic_syntax;
pub mod operators;
pub mod web_automation;
pub mod path_expressions;
pub mod predicates;
pub mod functions;
pub mod namespaces;
pub mod xpath2_features;
pub mod invalid_expressions;
pub mod complex_expressions;
pub mod performance_and_edge_cases;
pub mod xpath2_compliance;
pub mod ast_structure;
pub mod additional_cases;
pub mod string_literals;
