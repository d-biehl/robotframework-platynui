use platynui_xpath::runtime::{DynamicContextBuilder, NodeResolver};
use platynui_xpath::simple_node::{doc as sdoc, elem, text};
use platynui_xpath::{XdmItem, evaluate_expr};
use std::sync::Arc;

type N = platynui_xpath::simple_node::SimpleNode;

struct TestNodeResolver;
impl NodeResolver<N> for TestNodeResolver {
    fn doc_node(&self, uri: &str) -> Result<Option<N>, platynui_xpath::runtime::Error> {
        Ok(match uri {
            "urn:x" => Some(sdoc().child(elem("root").child(text("ok"))).build()),
            _ => None,
        })
    }
    fn collection_nodes(
        &self,
        uri: Option<&str>,
    ) -> Result<Vec<N>, platynui_xpath::runtime::Error> {
        let mut v = Vec::new();
        if uri == Some("urn:col") || uri.is_none() {
            v.push(sdoc().child(elem("a")).build());
            v.push(sdoc().child(elem("b")).build());
        }
        Ok(v)
    }
}

#[test]
fn doc_returns_document_node_from_resolver() {
    let nr = Arc::new(TestNodeResolver);
    let ctx = DynamicContextBuilder::<N>::default()
        .with_node_resolver(nr)
        .build();
    let out = evaluate_expr::<N>("string(doc('urn:x')/element(root)/text())", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert_eq!(s, "ok")
        }
        _ => panic!("expected string"),
    }
}

#[test]
fn collection_returns_nodes_from_resolver() {
    let nr = Arc::new(TestNodeResolver);
    let ctx = DynamicContextBuilder::<N>::default()
        .with_node_resolver(nr)
        .build();
    let out = evaluate_expr::<N>("count(collection('urn:col'))", &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, 2),
        _ => panic!("expected integer"),
    }
}

#[test]
fn doc_errors_when_unavailable_or_no_resolver() {
    // With resolver: unknown uri triggers FODC0005
    let nr = Arc::new(TestNodeResolver);
    let ctx = DynamicContextBuilder::<N>::default()
        .with_node_resolver(nr)
        .build();
    let err = evaluate_expr::<N>("doc('urn:nope')", &ctx).expect_err("expected error");
    assert_eq!(
        err.code_enum(),
        platynui_xpath::runtime::ErrorCode::FODC0005
    );
    // Without resolver: any uri triggers FODC0005
    let ctx2 = DynamicContextBuilder::<N>::default().build();
    let err2 = evaluate_expr::<N>("doc('urn:any')", &ctx2).expect_err("expected error");
    assert_eq!(
        err2.code_enum(),
        platynui_xpath::runtime::ErrorCode::FODC0005
    );
}
