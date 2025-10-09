use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::model::simple::{doc, elem, ns};
use platynui_xpath::{XdmNode, evaluate_expr, xdm::XdmItem as I};

type N = platynui_xpath::model::simple::SimpleNode;

fn ctx_with(root: N) -> DynamicContext<N> {
    DynamicContextBuilder::default().with_context_item(I::Node(root)).build()
}

#[test]
fn namespace_axis_includes_declared_and_xml() {
    // <root xmlns:p="urn:one" xmlns:q="urn:two"><child/></root>
    let d = doc()
        .child(elem("root").namespace(ns("p", "urn:one")).namespace(ns("q", "urn:two")).child(elem("child")))
        .build();
    let root = d.children().next().unwrap();
    let ctx = ctx_with(root.clone());

    let out = evaluate_expr::<N>("namespace::node()", &ctx).unwrap();
    // Collect prefixes and URIs
    let mut prefixes = Vec::new();
    let mut uris = Vec::new();
    for it in out {
        if let I::Node(n) = it {
            if let Some(q) = n.name()
                && let Some(p) = q.prefix
            {
                prefixes.push(p);
            }
            uris.push(n.string_value());
        }
    }
    // Must include p and q (declared) and xml (implicit)
    assert!(prefixes.contains(&"p".to_string()));
    assert!(prefixes.contains(&"q".to_string()));
    assert!(prefixes.contains(&"xml".to_string()));

    // xml URI must be canonical; others as declared
    assert!(uris.contains(&"urn:one".to_string()));
    assert!(uris.contains(&"urn:two".to_string()));
    assert!(uris.contains(&"http://www.w3.org/XML/1998/namespace".to_string()));
}
