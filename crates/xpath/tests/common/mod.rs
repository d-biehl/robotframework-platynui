//! Shared helpers for xpath integration tests.
//!
//! Each top-level `tests/*.rs` file compiles as its own binary, so shared code
//! lives in this directory-module form — Cargo does not treat it as a test
//! binary on its own. Include it with `mod common;` in any test file that needs
//! the helpers.

#![allow(dead_code)] // not every integration test binary uses every helper

use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use platynui_xpath::{DynamicContext, SimpleNode, evaluate_expr};

/// Evaluates `xpath` against `ctx` and returns the single integer it produces.
///
/// Panics with the offending xpath string if compilation fails, evaluation
/// fails, or the sequence is not exactly one integer atomic.
pub fn eval_single_integer(xpath: &str, ctx: &DynamicContext<SimpleNode>) -> i64 {
    let seq = evaluate_expr::<SimpleNode>(xpath, ctx).unwrap_or_else(|e| panic!("evaluate `{xpath}`: {e:?}"));
    match &seq[..] {
        [XdmItem::Atomic(XdmAtomicValue::Integer(n))] => *n,
        other => panic!("expected single integer for `{xpath}`, got: {other:?}"),
    }
}

/// Evaluates `xpath` against `ctx` and returns the single boolean it produces.
pub fn eval_single_boolean(xpath: &str, ctx: &DynamicContext<SimpleNode>) -> bool {
    let seq = evaluate_expr::<SimpleNode>(xpath, ctx).unwrap_or_else(|e| panic!("evaluate `{xpath}`: {e:?}"));
    match &seq[..] {
        [XdmItem::Atomic(XdmAtomicValue::Boolean(b))] => *b,
        other => panic!("expected single boolean for `{xpath}`, got: {other:?}"),
    }
}
