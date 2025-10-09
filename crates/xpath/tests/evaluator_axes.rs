use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::{
    XdmNode, evaluate_expr,
    simple_node::{attr, doc, elem, text},
    xdm::XdmItem as I,
};
use rstest::{fixture, rstest};
type N = platynui_xpath::model::simple::SimpleNode;

fn build_tree() -> N {
    // <root id="r"><a><b>one</b><b>two</b></a><c><d>three</d></c></root>
    let doc_node = doc()
        .child(
            elem("root")
                .attr(attr("id", "r"))
                .child(elem("a").child(elem("b").child(text("one"))).child(elem("b").child(text("two"))))
                .child(elem("c").child(elem("d").child(text("three")))),
        )
        .build();

    doc_node.children().next().unwrap()
}

fn ctx_with(item: N) -> DynamicContext<N> {
    let mut b = DynamicContextBuilder::default();
    b = b.with_context_item(I::Node(item));
    b.build()
}

// Fixtures for shared tree + context
#[fixture]
fn root() -> N {
    build_tree()
}

#[fixture]
fn ctx(root: N) -> DynamicContext<N> {
    ctx_with(root)
}

#[rstest]
fn axis_child_descendant(ctx: DynamicContext<N>) {
    let out = evaluate_expr::<N>("child::a/child::b", &ctx).unwrap();
    assert_eq!(out.len(), 2);
}

#[rstest]
fn axis_descendant_or_self(ctx: DynamicContext<N>) {
    let out = evaluate_expr::<N>("descendant-or-self::b", &ctx).unwrap();
    assert_eq!(out.len(), 2);
}

#[rstest]
fn axis_attribute(ctx: DynamicContext<N>) {
    let out = evaluate_expr::<N>("attribute::id", &ctx).unwrap();
    assert_eq!(out.len(), 1);
}

#[rstest]
fn axis_parent(ctx: DynamicContext<N>) {
    let b_node_seq = evaluate_expr::<N>("child::a/child::b[1]", &ctx).unwrap();
    let first_b = b_node_seq[0].clone();
    let ctx_b = ctx_with(match first_b {
        I::Node(n) => n,
        _ => panic!("expected node"),
    });
    let out = evaluate_expr::<N>("parent::a", &ctx_b).unwrap();
    assert_eq!(out.len(), 1);
}

#[rstest]
fn axis_self(ctx: DynamicContext<N>) {
    let out = evaluate_expr::<N>("self::root", &ctx).unwrap();
    assert_eq!(out.len(), 1);
}

#[rstest]
fn axis_ancestor(ctx: DynamicContext<N>) {
    // Get a deep node (<d>) then from its context check ancestor::root
    let d_seq = evaluate_expr::<N>("child::c/child::d", &ctx).unwrap();
    let d_node = match &d_seq[0] {
        I::Node(n) => n.clone(),
        _ => panic!("expected node"),
    };
    let ctx_d = ctx_with(d_node);
    let out = evaluate_expr::<N>("ancestor::root", &ctx_d).unwrap();
    assert_eq!(out.len(), 1);
}

#[rstest]
fn axis_ancestor_or_self(ctx: DynamicContext<N>) {
    let a_seq = evaluate_expr::<N>("child::a", &ctx).unwrap();
    let a_node = match &a_seq[0] {
        I::Node(n) => n.clone(),
        _ => panic!("expected node"),
    };
    let ctx_a = ctx_with(a_node);
    let out = evaluate_expr::<N>("ancestor-or-self::a", &ctx_a).unwrap();
    assert_eq!(out.len(), 1);
}

#[rstest]
fn axis_following_sibling(ctx: DynamicContext<N>) {
    // From <a>, following-sibling::c should yield one node
    let a_seq = evaluate_expr::<N>("child::a", &ctx).unwrap();
    let a_node = match &a_seq[0] {
        I::Node(n) => n.clone(),
        _ => panic!("expected node"),
    };
    let ctx_a = ctx_with(a_node);
    let out = evaluate_expr::<N>("following-sibling::c", &ctx_a).unwrap();
    assert_eq!(out.len(), 1);
}

#[rstest]
fn axis_preceding_sibling(ctx: DynamicContext<N>) {
    // From <c>, preceding-sibling::a should yield one node
    let c_seq = evaluate_expr::<N>("child::c", &ctx).unwrap();
    let c_node = match &c_seq[0] {
        I::Node(n) => n.clone(),
        _ => panic!("expected node"),
    };
    let ctx_c = ctx_with(c_node);
    let out = evaluate_expr::<N>("preceding-sibling::a", &ctx_c).unwrap();
    assert_eq!(out.len(), 1);
}
