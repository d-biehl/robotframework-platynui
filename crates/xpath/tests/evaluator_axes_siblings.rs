use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::{
    XdmNode, evaluate_expr,
    simple_node::{doc, elem},
    xdm::XdmItem as I,
};
use rstest::{fixture, rstest};
type N = platynui_xpath::model::simple::SimpleNode;

fn build_tree() -> N {
    // <root><s1/><s2/><s3/><s4/></root>
    let doc_node =
        doc().child(elem("root").child(elem("s1")).child(elem("s2")).child(elem("s3")).child(elem("s4"))).build();
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
fn following_sibling_direction(ctx: DynamicContext<N>) {
    // From s2, following-sibling::* => s3, s4
    let s2 = evaluate_expr::<N>("child::s2", &ctx).unwrap();
    let s2n = match &s2[0] {
        I::Node(n) => n.clone(),
        _ => panic!("expected node"),
    };
    let ctx_s2 = ctx_with(s2n);
    let out = evaluate_expr::<N>("following-sibling::*", &ctx_s2).unwrap();
    assert_eq!(out.len(), 2);
    let names: Vec<String> = out
        .iter()
        .map(|i| match i {
            I::Node(n) => n.name().unwrap().local,
            _ => panic!("node expected"),
        })
        .collect();
    assert_eq!(names, vec!["s3".to_string(), "s4".to_string()]);
}

#[rstest]
fn preceding_sibling_direction(ctx: DynamicContext<N>) {
    // From s3, preceding-sibling::* => s1, s2
    let s3 = evaluate_expr::<N>("child::s3", &ctx).unwrap();
    let s3n = match &s3[0] {
        I::Node(n) => n.clone(),
        _ => panic!("expected node"),
    };
    let ctx_s3 = ctx_with(s3n);
    let out = evaluate_expr::<N>("preceding-sibling::*", &ctx_s3).unwrap();
    assert_eq!(out.len(), 2);
    let names: Vec<String> = out
        .iter()
        .map(|i| match i {
            I::Node(n) => n.name().unwrap().local,
            _ => panic!("node expected"),
        })
        .collect();
    assert_eq!(names, vec!["s1".to_string(), "s2".to_string()]);
}
