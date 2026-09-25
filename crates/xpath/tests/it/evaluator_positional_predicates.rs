use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::{
    XdmNode, evaluate_expr,
    simple_node::{doc, elem, text},
    xdm::XdmItem as I,
};
use rstest::{fixture, rstest};
type N = platynui_xpath::model::simple::SimpleNode;

fn build_tree() -> N {
    // <root><a>one</a><a>two</a><a>three</a><a>four</a></root>
    let doc_node = doc()
        .child(
            elem("root")
                .child(elem("a").child(text("one")))
                .child(elem("a").child(text("two")))
                .child(elem("a").child(text("three")))
                .child(elem("a").child(text("four"))),
        )
        .build();
    doc_node.children().next().unwrap()
}

fn ctx_with(item: N) -> DynamicContext<N> {
    let mut b = DynamicContextBuilder::default();
    b = b.with_context_item(I::Node(item));
    b.build()
}

#[fixture]
fn root() -> N {
    return build_tree();
}
#[fixture]
fn ctx(root: N) -> DynamicContext<N> {
    return ctx_with(root);
}

#[rstest]
fn predicate_numeric_first(ctx: DynamicContext<N>) {
    let out = evaluate_expr::<N>("child::a[1]", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    // Should be the first <a>
    assert!(matches!(&out[0], I::Node(n) if n.string_value()=="one"));
}

#[rstest]
fn predicate_numeric_middle(ctx: DynamicContext<N>) {
    let out = evaluate_expr::<N>("child::a[3]", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    assert!(matches!(&out[0], I::Node(n) if n.string_value()=="three"));
}

#[rstest]
fn predicate_numeric_out_of_range(ctx: DynamicContext<N>) {
    let out = evaluate_expr::<N>("child::a[10]", &ctx).unwrap();
    assert_eq!(out.len(), 0);
}

#[rstest]
fn predicate_boolean(ctx: DynamicContext<N>) {
    // Keep only nodes whose string() length > 3 (three,four) using boolean expression
    let out = evaluate_expr::<N>("child::a[string-length(string(.)) > 3]", &ctx).unwrap();
    assert_eq!(out.len(), 2);
}

#[rstest]
fn predicate_position_function(ctx: DynamicContext<N>) {
    let out = evaluate_expr::<N>("child::a[position() < 3]", &ctx).unwrap();
    assert_eq!(out.len(), 2);
}

#[rstest]
fn predicate_last_function(ctx: DynamicContext<N>) {
    let out = evaluate_expr::<N>("child::a[last()]", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    assert!(matches!(&out[0], I::Node(n) if n.string_value()=="four"));
}

#[rstest]
fn predicate_position_eq_last(ctx: DynamicContext<N>) {
    let out = evaluate_expr::<N>("child::a[position() = last()]", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    assert!(matches!(&out[0], I::Node(n) if n.string_value()=="four"));
}

#[rstest]
fn predicate_chained_numeric_boolean(ctx: DynamicContext<N>) {
    // First keep position() <= 3 (one,two,three), then boolean length test > 3 -> only "three"
    let out = evaluate_expr::<N>("child::a[position() <= 3][string-length(string(.)) > 3]", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    assert!(matches!(&out[0], I::Node(n) if n.string_value()=="three"));
}

#[rstest]
fn predicate_chained_boolean_numeric(ctx: DynamicContext<N>) {
    // First boolean filter string-length > 3 (three,four) then [1] should pick "three"
    let out = evaluate_expr::<N>("child::a[string-length(string(.)) > 3][1]", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    assert!(matches!(&out[0], I::Node(n) if n.string_value()=="three"));
}
