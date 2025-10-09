use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::model::NodeKind;
use platynui_xpath::{
    XdmNode, evaluate_expr,
    simple_node::{attr, doc, elem, ns},
    xdm::XdmItem as I,
};
use rstest::{fixture, rstest};
type N = platynui_xpath::model::simple::SimpleNode;

fn build_tree() -> N {
    // <root id="r" xmlns:p="urn:x">
    //   <a/>
    //   <b class="B"/>
    //   <c xmlns:q="urn:y"><d data="D"/></c>
    //   <e/>
    // </root>
    let d = doc()
        .child(
            elem("root")
                .attr(attr("id", "r"))
                .namespace(ns("p", "urn:x"))
                .child(elem("a"))
                .child(elem("b").attr(attr("class", "B")))
                .child(elem("c").namespace(ns("q", "urn:y")).child(elem("d").attr(attr("data", "D"))))
                .child(elem("e")),
        )
        .build();
    d.children().next().unwrap()
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
fn axis_following(ctx: DynamicContext<N>) {
    // From <b>, following::* should include c, d, e (in order)
    let b = evaluate_expr::<N>("child::b", &ctx).unwrap();
    let bnode = match &b[0] {
        I::Node(n) => n.clone(),
        _ => panic!("expected node"),
    };
    let ctx_b = ctx_with(bnode);
    let out = evaluate_expr::<N>("following::*", &ctx_b).unwrap();
    let names: Vec<String> = out
        .iter()
        .map(|i| match i {
            I::Node(n) => n.name().unwrap().local,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(names, vec!["c".to_string(), "d".to_string(), "e".to_string()]);
}

#[rstest]
fn axis_preceding(ctx: DynamicContext<N>) {
    // From <d>, preceding::* should include a, b (exclude ancestor c)
    let d_node = {
        let c = evaluate_expr::<N>("child::c", &ctx).unwrap();
        let cnode = match &c[0] {
            I::Node(n) => n.clone(),
            _ => panic!("expected node"),
        };
        let ctx_c = ctx_with(cnode);
        let d = evaluate_expr::<N>("child::d", &ctx_c).unwrap();
        match &d[0] {
            I::Node(n) => n.clone(),
            _ => panic!("expected node"),
        }
    };
    let ctx_d = ctx_with(d_node);
    let out = evaluate_expr::<N>("preceding::*", &ctx_d).unwrap();
    let names: Vec<String> = out
        .iter()
        .map(|i| match i {
            I::Node(n) => n.name().unwrap().local,
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
}

#[rstest]
fn axis_following_excludes_attr_ns(ctx: DynamicContext<N>) {
    // Context: <b>. following::* must exclude attributes and namespaces anywhere.
    let b = evaluate_expr::<N>("child::b", &ctx).unwrap();
    let bnode = match &b[0] {
        I::Node(n) => n.clone(),
        _ => panic!("expected node"),
    };
    let ctx_b = ctx_with(bnode);
    // Ensure that following::node() does not include attributes or namespaces
    let nodes = evaluate_expr::<N>("following::node()", &ctx_b).unwrap();
    for it in nodes {
        if let I::Node(n) = it {
            assert!(
                !matches!(n.kind(), NodeKind::Attribute | NodeKind::Namespace),
                "following::node() must exclude attributes/namespaces, found kind: {:?}",
                n.kind()
            );
        }
    }
}

#[rstest]
fn axis_preceding_excludes_attr_ns(ctx: DynamicContext<N>) {
    // Context: <d>. preceding::* must exclude attributes and namespaces anywhere.
    let d_node = {
        let c = evaluate_expr::<N>("child::c", &ctx).unwrap();
        let cnode = match &c[0] {
            I::Node(n) => n.clone(),
            _ => panic!("expected node"),
        };
        let ctx_c = ctx_with(cnode);
        let d = evaluate_expr::<N>("child::d", &ctx_c).unwrap();
        match &d[0] {
            I::Node(n) => n.clone(),
            _ => panic!("expected node"),
        }
    };
    let ctx_d = ctx_with(d_node);
    let nodes = evaluate_expr::<N>("preceding::node()", &ctx_d).unwrap();
    for it in nodes {
        if let I::Node(n) = it {
            assert!(
                !matches!(n.kind(), NodeKind::Attribute | NodeKind::Namespace),
                "preceding::node() must exclude attributes/namespaces, found kind: {:?}",
                n.kind()
            );
        }
    }
}
