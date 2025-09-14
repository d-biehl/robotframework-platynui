use platynui_xpath::engine::runtime::{DynamicContextBuilder, NodeResolver};
use platynui_xpath::{xdm::XdmItem, model::XdmNode, engine::evaluator::evaluate_expr};
use std::sync::Arc;

type N = platynui_xpath::model::simple::SimpleNode;

struct TestNodeResolver;
impl NodeResolver<N> for TestNodeResolver {
    fn doc_node(&self, uri: &str) -> Result<Option<N>, platynui_xpath::engine::runtime::Error> {
        Ok(match uri {
            "urn:ok" => Some(
                platynui_xpath::model::simple::doc()
                    .child(platynui_xpath::model::simple::elem("root"))
                    .build(),
            ),
            _ => None,
        })
    }
}

#[test]
fn default_collation_reports_uri() {
    let ctx = DynamicContextBuilder::<N>::default().build();
    let out = evaluate_expr::<N>("default-collation()", &ctx).unwrap();
    match &out[0] {
        XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert_eq!(s, platynui_xpath::engine::collation::CODEPOINT_URI)
        }
        _ => panic!("expected string"),
    }
}

#[test]
fn doc_available_uses_node_resolver() {
    let resolver = Arc::new(TestNodeResolver);
    let ctx = DynamicContextBuilder::<N>::default()
        .with_node_resolver(resolver)
        .build();
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

#[test]
fn root_function_returns_document_root() {
    use platynui_xpath::model::simple::{doc, elem};
    let d = doc().child(elem("root").child(elem("c"))).build();
    let ctx = DynamicContextBuilder::<N>::default()
        .with_context_item(d.clone())
        .build();
    // root(/root/c)
    let out = evaluate_expr::<N>("root(/root/c)", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    match &out[0] {
        XdmItem::Node(n) => {
            assert!(matches!(
                n.kind(),
                platynui_xpath::model::NodeKind::Document
            ));
            let ch = n.children();
            assert_eq!(ch[0].name().unwrap().local, "root");
        }
        _ => panic!("expected node"),
    }
}

#[test]
fn base_uri_document_uri_empty_without_adapter_support() {
    use platynui_xpath::model::simple::{doc, elem};
    let d = doc().child(elem("root")).build();
    let ctx = DynamicContextBuilder::<N>::default()
        .with_context_item(d.clone())
        .build();
    // base-uri(/) -> empty for SimpleNode
    let b = evaluate_expr::<N>("base-uri(/)", &ctx).unwrap();
    assert!(b.is_empty());
    // document-uri(/) -> empty for SimpleNode
    let u = evaluate_expr::<N>("document-uri(/)", &ctx).unwrap();
    assert!(u.is_empty());
}
