use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, model::XdmNode, xdm::XdmItem};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

#[fixture]
fn empty_ctx() -> DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

#[rstest]
fn namespace_uri_on_elements_and_attributes() {
    use platynui_xpath::model::simple::{attr, doc, elem, ns};
    // <p:root xmlns:p="urn:one" id="x" p:aid="y"/>
    let d = doc()
        .child(elem("p:root").namespace(ns("p", "urn:one")).attr(attr("id", "x")).attr(attr("p:aid", "y")))
        .build();
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(d.clone()).build();

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

#[rstest]
fn namespace_uri_on_pi_and_namespace_nodes() {
    use platynui_xpath::model::simple::{SimpleNode, doc, elem};
    // <root><?target x?></root>
    let d = doc().child(elem("root").child(SimpleNode::pi("target", "x"))).build();
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(d.clone()).build();
    // Processing-instruction has no QName -> empty sequence
    let pi_ns = evaluate_expr::<N>("namespace-uri(//processing-instruction())", &ctx).unwrap();
    assert!(pi_ns.is_empty());
    // Text node has no QName -> empty sequence
    let empty = evaluate_expr::<N>("namespace-uri(/*/text())", &ctx).unwrap();
    assert!(empty.is_empty());
}

#[rstest]
fn namespace_uri_type_error_on_non_node(empty_ctx: DynamicContext<N>) {
    let ctx = empty_ctx;
    let err = evaluate_expr::<N>("namespace-uri('x')", &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::XPTY0004);
}

#[rstest]
fn namespace_uri_uses_context_item_when_omitted() {
    use platynui_xpath::model::simple::{doc, elem, ns};
    let d = doc().child(elem("p:r").namespace(ns("p", "urn:x"))).build();
    let root = d.children().next().unwrap();
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(root).build();
    let r = evaluate_expr::<N>("namespace-uri()", &ctx).unwrap();
    match &r.first() {
        Some(XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::AnyUri(u))) => {
            assert_eq!(u, "urn:x")
        }
        other => panic!("expected anyURI, got {:?}", other),
    }
}
