use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder, NodeResolver};
use platynui_xpath::{engine::evaluator::evaluate_expr, model::XdmNode, xdm::XdmItem};
use rstest::{fixture, rstest};
use std::sync::Arc;

type N = platynui_xpath::model::simple::SimpleNode;

struct TestNodeResolver;
impl NodeResolver<N> for TestNodeResolver {
    fn doc_node(&self, uri: &str) -> Result<Option<N>, platynui_xpath::engine::runtime::Error> {
        Ok(match uri {
            "urn:ok" => {
                Some(platynui_xpath::model::simple::doc().child(platynui_xpath::model::simple::elem("root")).build())
            }
            _ => None,
        })
    }
}

#[fixture]
fn ctx() -> DynamicContext<N> {
    DynamicContextBuilder::<N>::default().build()
}

#[rstest]
fn default_collation_reports_uri(ctx: DynamicContext<N>) {
    let out = evaluate_expr::<N>("default-collation()", &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert_eq!(s, platynui_xpath::engine::collation::CODEPOINT_URI)
        }
        _ => panic!("expected string"),
    }
}

#[rstest]
fn doc_available_uses_node_resolver() {
    let resolver = Arc::new(TestNodeResolver);
    let ctx = DynamicContextBuilder::<N>::default().with_node_resolver(resolver).build();
    let t1 = evaluate_expr::<N>("doc-available('urn:ok')", &ctx).unwrap();
    match &t1[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(*b),
        _ => panic!("expected boolean"),
    }
    let t2 = evaluate_expr::<N>("doc-available('urn:missing')", &ctx).unwrap();
    match &t2[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) => assert!(!*b),
        _ => panic!("expected boolean"),
    }
}

#[rstest]
fn root_function_returns_document_root() {
    use platynui_xpath::model::simple::{doc, elem};
    let d = doc().child(elem("root").child(elem("c"))).build();
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(d.clone()).build();
    // root(/root/c)
    let out = evaluate_expr::<N>("root(/root/c)", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    match &out[0] {
        XdmItem::Node(n) => {
            assert!(matches!(n.kind(), platynui_xpath::model::NodeKind::Document));
            let ch: Vec<_> = n.children().collect();
            assert_eq!(ch[0].name().unwrap().local, "root");
        }
        _ => panic!("expected node"),
    }
}

#[rstest]
fn base_uri_document_uri_empty_without_adapter_support() {
    use platynui_xpath::model::simple::{doc, elem};
    let d = doc().child(elem("root")).build();
    let ctx = DynamicContextBuilder::<N>::default().with_context_item(d.clone()).build();
    // base-uri(/) -> empty for SimpleNode
    let b = evaluate_expr::<N>("base-uri(/)", &ctx).unwrap();
    assert!(b.is_empty());
    // document-uri(/) -> empty for SimpleNode
    let u = evaluate_expr::<N>("document-uri(/)", &ctx).unwrap();
    assert!(u.is_empty());
}
