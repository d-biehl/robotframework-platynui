//! Tests for streaming sequence predicates: `count()`, `exists()`, `empty()`.
//!
//! These three functions share a stream-based implementation, so each group is
//! exercised with the same table of scenarios (empty sequence, singleton,
//! multiple items, large range requiring early termination).

use platynui_xpath::{DynamicContextBuilder, SimpleNode, XdmItem, elem, simple_doc};
use rstest::{fixture, rstest};

mod common;
use common::{eval_single_boolean, eval_single_integer};

type Ctx = platynui_xpath::DynamicContext<SimpleNode>;

// ===== Fixtures =====

#[fixture]
fn empty_ctx() -> Ctx {
    DynamicContextBuilder::default().with_context_item(XdmItem::Node(simple_doc().build())).build()
}

#[fixture]
fn single_root_ctx() -> Ctx {
    let doc = simple_doc().child(elem("root")).build();
    DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build()
}

#[fixture]
fn three_items_ctx() -> Ctx {
    let doc = simple_doc().child(elem("root").child(elem("item")).child(elem("item")).child(elem("item"))).build();
    DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build()
}

#[fixture]
fn no_item_ctx() -> Ctx {
    DynamicContextBuilder::default().build()
}

// ===== count() =====

#[rstest]
#[case::empty_literal("count(())", 0)]
#[case::empty_tree_path("count(/item)", 0)]
fn count_on_empty_tree(empty_ctx: Ctx, #[case] xpath: &str, #[case] expected: i64) {
    assert_eq!(eval_single_integer(xpath, &empty_ctx), expected);
}

#[rstest]
#[case::root("count(/root)", 1)]
fn count_on_single_root(single_root_ctx: Ctx, #[case] xpath: &str, #[case] expected: i64) {
    assert_eq!(eval_single_integer(xpath, &single_root_ctx), expected);
}

#[rstest]
#[case::multiple_children("count(//item)", 3)]
#[case::filtered_empty("count(//missing)", 0)]
fn count_on_three_items(three_items_ctx: Ctx, #[case] xpath: &str, #[case] expected: i64) {
    assert_eq!(eval_single_integer(xpath, &three_items_ctx), expected);
}

#[rstest]
#[case::range("count(1 to 10000)", 10_000)]
#[case::range_single("count(1 to 1)", 1)]
#[case::range_empty("count(5 to 1)", 0)]
fn count_on_large_range_streams(no_item_ctx: Ctx, #[case] xpath: &str, #[case] expected: i64) {
    assert_eq!(eval_single_integer(xpath, &no_item_ctx), expected);
}

// ===== exists() =====

#[rstest]
#[case::empty_literal("exists(())", false)]
#[case::empty_tree_path("exists(/item)", false)]
fn exists_on_empty_tree(empty_ctx: Ctx, #[case] xpath: &str, #[case] expected: bool) {
    assert_eq!(eval_single_boolean(xpath, &empty_ctx), expected);
}

#[rstest]
#[case::root("exists(/root)", true)]
fn exists_on_single_root(single_root_ctx: Ctx, #[case] xpath: &str, #[case] expected: bool) {
    assert_eq!(eval_single_boolean(xpath, &single_root_ctx), expected);
}

#[rstest]
// Early termination: must not materialize 1M items to decide existence.
#[case::huge_range("exists(1 to 1000000)", true)]
#[case::empty_range("exists(5 to 1)", false)]
fn exists_terminates_early(no_item_ctx: Ctx, #[case] xpath: &str, #[case] expected: bool) {
    assert_eq!(eval_single_boolean(xpath, &no_item_ctx), expected);
}

// ===== empty() =====

#[rstest]
#[case::empty_literal("empty(())", true)]
#[case::empty_tree_path("empty(/item)", true)]
fn empty_on_empty_tree(empty_ctx: Ctx, #[case] xpath: &str, #[case] expected: bool) {
    assert_eq!(eval_single_boolean(xpath, &empty_ctx), expected);
}

#[rstest]
#[case::root("empty(/root)", false)]
fn empty_on_single_root(single_root_ctx: Ctx, #[case] xpath: &str, #[case] expected: bool) {
    assert_eq!(eval_single_boolean(xpath, &single_root_ctx), expected);
}

#[rstest]
// Early termination: mirror of `exists_terminates_early`.
#[case::huge_range("empty(1 to 1000000)", false)]
#[case::empty_range("empty(5 to 1)", true)]
fn empty_terminates_early(no_item_ctx: Ctx, #[case] xpath: &str, #[case] expected: bool) {
    assert_eq!(eval_single_boolean(xpath, &no_item_ctx), expected);
}

// ===== Combined =====

#[test]
fn count_exists_empty_compose_in_if_expression() {
    let doc = simple_doc().child(elem("root").child(elem("item")).child(elem("item"))).build();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();

    assert_eq!(eval_single_integer("if (exists(//item)) then count(//item) else 0", &ctx), 2);
    assert_eq!(eval_single_integer("if (empty(//missing)) then count(//item) else -1", &ctx), 2);
}
