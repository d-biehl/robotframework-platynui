use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder, NodeResolver};
use platynui_xpath::model::simple::{doc as sdoc, elem, text};
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmItem};
use rstest::{fixture, rstest};
use std::sync::Arc;

type N = platynui_xpath::model::simple::SimpleNode;

struct TestNodeResolver;
impl NodeResolver<N> for TestNodeResolver {
    fn doc_node(&self, uri: &str) -> Result<Option<N>, platynui_xpath::engine::runtime::Error> {
        Ok(match uri {
            "urn:x" => Some(sdoc().child(elem("root").child(text("ok"))).build()),
            _ => None,
        })
    }
    fn collection_nodes(&self, uri: Option<&str>) -> Result<Vec<N>, platynui_xpath::engine::runtime::Error> {
        let mut v = Vec::new();
        if uri == Some("urn:col") || uri.is_none() {
            v.push(sdoc().child(elem("a")).build());
            v.push(sdoc().child(elem("b")).build());
        }
        Ok(v)
    }
}

#[fixture]
fn ctx_with_resolver() -> DynamicContext<N> {
    let nr = Arc::new(TestNodeResolver);
    DynamicContextBuilder::<N>::default().with_node_resolver(nr).build()
}

#[rstest]
fn doc_returns_document_node_from_resolver(ctx_with_resolver: DynamicContext<N>) {
    let ctx = ctx_with_resolver;
    let out = evaluate_expr::<N>("string(doc('urn:x')/element(root)/text())", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert_eq!(s, "ok")
        }
        _ => panic!("expected string"),
    }
}

#[rstest]
fn collection_returns_nodes_from_resolver(ctx_with_resolver: DynamicContext<N>) {
    let ctx = ctx_with_resolver;
    let out = evaluate_expr::<N>("count(collection('urn:col'))", &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(i)) => assert_eq!(*i, 2),
        _ => panic!("expected integer"),
    }
}

#[rstest]
fn doc_errors_when_unavailable_or_no_resolver() {
    // With resolver: unknown uri triggers FODC0005
    let nr = Arc::new(TestNodeResolver);
    let ctx = DynamicContextBuilder::<N>::default().with_node_resolver(nr).build();
    let err = evaluate_expr::<N>("doc('urn:nope')", &ctx).expect_err("expected error");
    assert_eq!(err.code_enum(), platynui_xpath::engine::runtime::ErrorCode::FODC0005);
    // Without resolver: any uri triggers FODC0005
    let ctx2 = DynamicContextBuilder::<N>::default().build();
    let err2 = evaluate_expr::<N>("doc('urn:any')", &ctx2).expect_err("expected error");
    assert_eq!(err2.code_enum(), platynui_xpath::engine::runtime::ErrorCode::FODC0005);
}
