use platynui_xpath::model::simple::{attr, elem, ns, text};
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use platynui_xpath::{engine::evaluator::evaluate_expr, model::XdmNode, runtime::DynamicContextBuilder};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

fn ctx_with_tree() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    // <root xmlns:p="urn:one" a="1"><p:child id="c1">Hi</p:child><child/></root>
    let root = elem("root")
        .namespace(ns("p", "urn:one"))
        .attr(attr("a", "1"))
        .child(elem("child").attr(attr("id", "c1")).child(text("Hi")))
        .child(elem("child"))
        .build();
    DynamicContextBuilder::new().with_context_item(root).build()
}

// --- Small helpers to reduce repetition ---
fn eval_one(ctx: &platynui_xpath::engine::runtime::DynamicContext<N>, expr: &str) -> XdmItem<N> {
    let out = evaluate_expr::<N>(expr, ctx).unwrap();
    assert_eq!(out.len(), 1, "expected single item for expr {expr}");
    out.into_iter().next().unwrap()
}

fn eval_string(ctx: &platynui_xpath::engine::runtime::DynamicContext<N>, expr: &str) -> String {
    match eval_one(ctx, expr) {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => s,
        other => panic!("expected String, got {other:?}"),
    }
}

fn eval_qname(
    ctx: &platynui_xpath::engine::runtime::DynamicContext<N>,
    expr: &str,
) -> (Option<String>, Option<String>, String) {
    match eval_one(ctx, expr) {
        XdmItem::Atomic(XdmAtomicValue::QName { ns_uri, prefix, local }) => (ns_uri, prefix, local),
        other => panic!("expected QName, got {other:?}"),
    }
}

fn assert_empty(ctx: &platynui_xpath::engine::runtime::DynamicContext<N>, expr: &str) {
    let out = evaluate_expr::<N>(expr, ctx).unwrap();
    assert!(out.is_empty(), "expected empty sequence for expr {expr}, got {out:?}");
}

#[rstest]
fn node_name_and_local_namespace() {
    let ctx = ctx_with_tree();
    // node-name on element
    let (ns_uri, prefix, local) = eval_qname(&ctx, "node-name(.)");
    assert_eq!(prefix, None);
    assert_eq!(local, "root");
    assert_eq!(ns_uri, None);

    // name/local-name/namespace-uri on attribute node
    assert_eq!(eval_string(&ctx, "name(@a)"), "a");
    assert_eq!(eval_string(&ctx, "local-name(@a)"), "a");
    // attributes in no namespace unless explicitly bound -> empty sequence per spec
    assert_empty(&ctx, "namespace-uri(@a)");
}

#[rstest]
fn empty_and_unnamed_nodes() {
    // Empty sequence
    let ctx = ctx_with_tree();
    assert_empty(&ctx, "node-name(())");

    // Text node has no name
    let root = elem("r").child(text("t")).build();
    let ctx = DynamicContextBuilder::new().with_context_item(root.children().next().unwrap()).build();
    assert!(eval_string(&ctx, "name(.)").is_empty());
    assert!(eval_string(&ctx, "local-name(.)").is_empty());
    assert_empty(&ctx, "namespace-uri(.)");
}

#[rstest]
fn prefixed_and_namespace_nodes() {
    // Create element with namespace node and prefixed child
    let doc =
        platynui_xpath::simple_doc().child(elem("root").namespace(ns("p", "urn:one")).child(elem("child"))).build();
    let root = doc.children().next().unwrap();
    let ctx = DynamicContextBuilder::new().with_context_item(root.clone()).build();

    // namespace-uri for element with no ns is empty
    assert_empty(&ctx, "namespace-uri(.)");

    // Namespace axis returns namespace nodes with prefix in name(), but namespace-uri(name()) is empty (name of namespace node is prefix)
    assert_eq!(eval_string(&ctx, "name(namespace::p)"), "p");
    // Namespace nodes have no QName -> empty sequence per fn:namespace-uri definition
    assert_empty(&ctx, "namespace-uri(namespace::p)");
}
