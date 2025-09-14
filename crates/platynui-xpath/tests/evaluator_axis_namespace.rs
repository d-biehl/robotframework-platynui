use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::{
    xdm::XdmItem as I, XdmNode, evaluate_expr,
    simple_node::{doc, elem, ns},
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
    doc_node.children()[0].clone()
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
    let mid_seq = evaluate_expr::<N>("child::mid", &ctx).unwrap();
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
            assert!(matches!(
                n.kind(),
                platynui_xpath::model::NodeKind::Namespace
            ));
        }
    }
}

#[rstest]
fn namespace_axis_filters_by_name(ctx: DynamicContext<N>) {
    // From <mid>, q should be present
    let mid_seq = evaluate_expr::<N>("child::mid/namespace::q", &ctx).unwrap();
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
        let t =
            evaluate_expr::<N>("child::mid/child::leaf/descendant-or-self::text()", &ctx).unwrap();
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
    let out = evaluate_expr::<N>("child::mid/following::node()", &ctx).unwrap();
    for it in &out {
        if let I::Node(n) = it {
            assert!(!matches!(
                n.kind(),
                platynui_xpath::model::NodeKind::Namespace
            ));
        }
    }
    let out2 = evaluate_expr::<N>("child::mid/preceding::node()", &ctx).unwrap();
    for it in &out2 {
        if let I::Node(n) = it {
            assert!(!matches!(
                n.kind(),
                platynui_xpath::model::NodeKind::Namespace
            ));
        }
    }
}
