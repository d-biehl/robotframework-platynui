use platynui_xpath::runtime::DynamicContextBuilder;
use platynui_xpath::{XdmItem, XdmNode, evaluate_expr};

type N = platynui_xpath::simple_node::SimpleNode;

#[test]
fn namespace_uri_on_elements_and_attributes() {
    use platynui_xpath::simple_node::{attr, doc, elem, ns};
    // <p:root xmlns:p="urn:one" id="x" p:aid="y"/>
    let d = doc()
        .child(
            elem("p:root")
                .namespace(ns("p", "urn:one"))
                .attr(attr("id", "x"))
                .attr(attr("p:aid", "y")),
        )
        .build();
    let ctx = DynamicContextBuilder::<N>::default()
        .with_context_item(d.clone())
        .build();

    // Element namespace
    let r = evaluate_expr::<N>("namespace-uri(/*)", &ctx).unwrap();
    match &r.first() {
        Some(XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::AnyUri(u))) => {
            assert_eq!(u, "urn:one")
        }
        other => panic!("expected anyURI, got {:?}", other),
    }

    // Unprefixed attribute has no namespace
    let a = evaluate_expr::<N>("namespace-uri(/*/@id)", &ctx).unwrap();
    assert!(a.is_empty());

    // Prefixed attribute inherits prefix namespace
    let pa = evaluate_expr::<N>("namespace-uri(/*/@*[local-name()='aid'])", &ctx).unwrap();
    match &pa.first() {
        Some(XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::AnyUri(u))) => {
            assert_eq!(u, "urn:one")
        }
        other => panic!("expected anyURI, got {:?}", other),
    }
}

#[test]
fn namespace_uri_on_pi_and_namespace_nodes() {
    use platynui_xpath::simple_node::{SimpleNode, doc, elem};
    // <root><?target x?></root>
    let d = doc()
        .child(elem("root").child(SimpleNode::pi("target", "x")))
        .build();
    let ctx = DynamicContextBuilder::<N>::default()
        .with_context_item(d.clone())
        .build();
    // Processing-instruction has no QName -> empty sequence
    let pi_ns = evaluate_expr::<N>("namespace-uri(//processing-instruction())", &ctx).unwrap();
    assert!(pi_ns.is_empty());
    // Text node has no QName -> empty sequence
    let empty = evaluate_expr::<N>("namespace-uri(/*/text())", &ctx).unwrap();
    assert!(empty.is_empty());
}

#[test]
fn namespace_uri_type_error_on_non_node() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let err = evaluate_expr::<N>("namespace-uri('x')", &ctx).unwrap_err();
    assert!(err.code.contains("XPTY0004"));
}

#[test]
fn namespace_uri_uses_context_item_when_omitted() {
    use platynui_xpath::simple_node::{doc, elem, ns};
    let d = doc().child(elem("p:r").namespace(ns("p", "urn:x"))).build();
    let root = d.children()[0].clone();
    let ctx = DynamicContextBuilder::<N>::default()
        .with_context_item(root)
        .build();
    let r = evaluate_expr::<N>("namespace-uri()", &ctx).unwrap();
    match &r.first() {
        Some(XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::AnyUri(u))) => {
            assert_eq!(u, "urn:x")
        }
        other => panic!("expected anyURI, got {:?}", other),
    }
}
