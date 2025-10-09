use platynui_xpath::compiler::compile_with_context;
use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder, StaticContextBuilder};
use platynui_xpath::{
    XdmNode, evaluate, evaluate_expr,
    simple_node::{doc, elem, ns},
    xdm::XdmItem as I,
};
use rstest::{fixture, rstest};
type N = platynui_xpath::model::simple::SimpleNode;

fn ctx_with(item: N) -> DynamicContext<N> {
    let mut b = DynamicContextBuilder::default();
    b = b.with_context_item(I::Node(item));
    b.build()
}

fn build_ns_tree() -> N {
    // <root xmlns:p="urn:one" xmlns="urn:default" id="r">
    //   <mid xmlns:q="urn:two"><leaf>t</leaf></mid>
    // </root>
    let doc_node = doc()
        .child(
            elem("root")
                .namespace(ns("p", "urn:one"))
                .namespace(ns("", "urn:default"))
                .attr(platynui_xpath::model::simple::attr("id", "r"))
                .child(
                    elem("mid")
                        .namespace(ns("q", "urn:two"))
                        .child(elem("leaf").child(platynui_xpath::model::simple::text("t"))),
                ),
        )
        .build();
    doc_node.children().next().unwrap()
}

#[fixture]
fn root() -> N {
    return build_ns_tree();
}

#[fixture]
fn ctx(root: N) -> DynamicContext<N> {
    return ctx_with(root);
}

#[rstest]
fn namespace_axis_on_element(ctx: DynamicContext<N>) {
    // From <mid>, expect q (self) and p/default (inherited) → total 3 namespace nodes
    let mid_seq = evaluate_expr::<N>("child::*[local-name()='mid']", &ctx).unwrap();
    let mid = match &mid_seq[0] {
        I::Node(n) => n.clone(),
        _ => panic!("node"),
    };
    let ctx_mid = ctx_with(mid);
    let out = evaluate_expr::<N>("namespace::node()", &ctx_mid).unwrap();
    assert!(out.len() >= 2); // at least q and p; default counted if represented
    // Ensure these are namespace nodes
    for it in &out {
        if let I::Node(n) = it {
            assert!(matches!(n.kind(), platynui_xpath::model::NodeKind::Namespace));
        }
    }
}

#[rstest]
fn namespace_axis_filters_by_name(ctx: DynamicContext<N>) {
    // From <mid>, q should be present
    let mid_seq = evaluate_expr::<N>("child::*[local-name()='mid']/namespace::q", &ctx).unwrap();
    assert!(!mid_seq.is_empty());
}

#[rstest]
fn namespace_axis_empty_on_non_element(ctx: DynamicContext<N>) {
    // From attribute or text, namespace axis is empty
    let attr_ctx = {
        let a = evaluate_expr::<N>("attribute::id", &ctx).unwrap();
        let n = match &a[0] {
            I::Node(n) => n.clone(),
            _ => panic!("node"),
        };
        ctx_with(n)
    };
    let out_attr = evaluate_expr::<N>("namespace::node()", &attr_ctx).unwrap();
    assert_eq!(out_attr.len(), 0);

    let text_ctx = {
        let t = evaluate_expr::<N>(
            "child::*[local-name()='mid']/child::*[local-name()='leaf']/descendant-or-self::text()",
            &ctx,
        )
        .unwrap();
        let n = match &t[0] {
            I::Node(n) => n.clone(),
            _ => panic!("node"),
        };
        ctx_with(n)
    };
    let out_text = evaluate_expr::<N>("namespace::node()", &text_ctx).unwrap();
    assert_eq!(out_text.len(), 0);
}

#[rstest]
fn following_preceding_exclude_namespaces(ctx: DynamicContext<N>) {
    // ensure namespace nodes are not returned by following/preceding
    let out = evaluate_expr::<N>("child::*[local-name()='mid']/following::node()", &ctx).unwrap();
    for it in &out {
        if let I::Node(n) = it {
            assert!(!matches!(n.kind(), platynui_xpath::model::NodeKind::Namespace));
        }
    }
    let out2 = evaluate_expr::<N>("child::*[local-name()='mid']/preceding::node()", &ctx).unwrap();
    for it in &out2 {
        if let I::Node(n) = it {
            assert!(!matches!(n.kind(), platynui_xpath::model::NodeKind::Namespace));
        }
    }
}

#[rstest]
fn namespace_prefixed_step_matches(ctx: DynamicContext<N>) {
    let static_ctx = StaticContextBuilder::new().with_namespace("d", "urn:default").build();
    let compiled = compile_with_context("child::d:mid", &static_ctx).unwrap();
    let seq = evaluate(&compiled, &ctx).unwrap();
    assert_eq!(seq.len(), 1);
    let n = match &seq[0] {
        I::Node(n) => n,
        _ => panic!("node expected"),
    };
    assert_eq!(n.name().unwrap().local, "mid");
    assert_eq!(n.lookup_namespace_uri(""), Some("urn:default".to_string()));
}

#[rstest]
fn namespace_unprefixed_requires_default_static(ctx: DynamicContext<N>) {
    let static_ctx = StaticContextBuilder::new().build();
    let compiled = compile_with_context("child::mid", &static_ctx).unwrap();
    let seq = evaluate(&compiled, &ctx).unwrap();
    assert!(seq.is_empty());
}

#[rstest]
fn namespace_wildcard_local_name(ctx: DynamicContext<N>) {
    let static_ctx = StaticContextBuilder::new().build();
    let compiled = compile_with_context("child::*:mid", &static_ctx).unwrap();
    let seq = evaluate(&compiled, &ctx).unwrap();
    assert_eq!(seq.len(), 1);
    let n = match &seq[0] {
        I::Node(n) => n,
        _ => panic!("node expected"),
    };
    assert_eq!(n.name().unwrap().local, "mid");
}

#[rstest]
fn namespace_wildcard_prefix(ctx: DynamicContext<N>) {
    let static_ctx = StaticContextBuilder::new().with_namespace("d", "urn:default").build();
    let compiled = compile_with_context("child::d:*", &static_ctx).unwrap();
    let seq = evaluate(&compiled, &ctx).unwrap();
    assert_eq!(seq.len(), 1);
    let n = match &seq[0] {
        I::Node(n) => n,
        _ => panic!("node expected"),
    };
    assert_eq!(n.lookup_namespace_uri(""), Some("urn:default".to_string()));
}

#[rstest]
fn namespace_axis_wildcard_returns_all(ctx: DynamicContext<N>) {
    let mid_seq = evaluate_expr::<N>("child::*[local-name()='mid']", &ctx).unwrap();
    let mid = match &mid_seq[0] {
        I::Node(n) => n.clone(),
        _ => panic!("node expected"),
    };
    let ctx_mid = ctx_with(mid);
    let out = evaluate_expr::<N>("namespace::*", &ctx_mid).unwrap();
    assert!(out.len() >= 2);
}

#[rstest]
fn attribute_default_namespace_not_applied(ctx: DynamicContext<N>) {
    let static_ctx = StaticContextBuilder::new().with_namespace("xml", "http://www.w3.org/XML/1998/namespace").build();
    let compiled_pref = compile_with_context("attribute::xml:id", &static_ctx).unwrap();
    let seq_pref = evaluate(&compiled_pref, &ctx).unwrap();
    assert_eq!(seq_pref.len(), 0);

    let compiled_unpref = compile_with_context("attribute::id", &static_ctx).unwrap();
    let seq_unpref = evaluate(&compiled_unpref, &ctx).unwrap();
    assert_eq!(seq_unpref.len(), 1);
}
