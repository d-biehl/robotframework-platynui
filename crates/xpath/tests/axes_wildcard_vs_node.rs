use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::model::simple::{doc, elem, text};
use platynui_xpath::{XdmNode, evaluate_expr, xdm::XdmItem as I};

type N = platynui_xpath::model::simple::SimpleNode;

fn ctx_with(root: N) -> DynamicContext<N> {
    DynamicContextBuilder::default().with_context_item(I::Node(root)).build()
}

#[test]
fn wildcard_child_vs_node_axis_differences() {
    // <root>pre<a>one<b/>two</a>post</root>
    let d = doc()
        .child(
            elem("root")
                .child(text("pre"))
                .child(elem("a").child(text("one")).child(elem("b")).child(text("two")))
                .child(text("post")),
        )
        .build();
    let root = d.children().next().unwrap();
    let ctx = ctx_with(root.clone());

    // //*: all element descendants (root,a,b) = 3 (root matched by desc-or-self from ToRoot step)
    let elements = evaluate_expr::<N>("//*", &ctx).unwrap();
    // Should include <root>, <a>, <b>
    let names: Vec<String> = elements
        .iter()
        .filter_map(|i| match i {
            I::Node(n) => n.name().map(|q| q.local),
            _ => None,
        })
        .collect();
    assert!(names.contains(&"root".to_string()));
    assert!(names.contains(&"a".to_string()));
    assert!(names.contains(&"b".to_string()));

    // //node(): all node descendants including text nodes
    let nodes = evaluate_expr::<N>("//node()", &ctx).unwrap();
    // Expect at least these node kinds: document root excluded (relative to context), but
    // element nodes (<root>,<a>,<b>) and text nodes (pre,one,two,post)
    let count_text = nodes
        .iter()
        .filter(|i| matches!(i, I::Node(n) if matches!(n.kind(), platynui_xpath::model::NodeKind::Text)))
        .count();
    assert!(count_text >= 4, "should include text nodes: pre, one, two, post");

    // Sanity: //* should not include text nodes
    let has_text_in_star =
        elements.iter().any(|i| matches!(i, I::Node(n) if matches!(n.kind(), platynui_xpath::model::NodeKind::Text)));
    assert!(!has_text_in_star, "//* must not include text nodes");
}
