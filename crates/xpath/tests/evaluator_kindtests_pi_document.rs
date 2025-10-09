use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::model::simple::{doc, elem};
use platynui_xpath::{engine::evaluator::evaluate_expr, model::XdmNode, xdm::XdmItem};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;

#[rstest]
fn processing_instruction_target_filter() {
    // <root><?go data?></root>
    let root = elem("root").child(platynui_xpath::model::simple::SimpleNode::pi("go", "data")).build();
    let ctx = DynamicContextBuilder::default().with_context_item(root.clone()).build();
    let seq = evaluate_expr::<N>("child::processing-instruction('go')", &ctx).unwrap();
    assert_eq!(seq.len(), 1);
    match &seq[0] {
        XdmItem::Node(n) => {
            assert!(matches!(n.kind(), platynui_xpath::model::NodeKind::ProcessingInstruction));
            let nm = n.name().unwrap();
            assert_eq!(nm.local, "go");
        }
        _ => panic!("expected node"),
    }
    // Non-matching target should yield empty
    let empty = evaluate_expr::<N>("child::processing-instruction('nope')", &ctx).unwrap();
    assert!(empty.is_empty());
}

#[rstest]
fn document_node_inner_test_matches_document_element() {
    // doc(root(child))
    let document = doc().child(elem("root").child(elem("child"))).build();
    let ctx = DynamicContextBuilder::default().with_context_item(document.clone()).build();
    // self::document-node(element(root)) should match the document node
    let out = evaluate_expr::<N>("self::document-node(element(root))", &ctx).unwrap();
    assert_eq!(out.len(), 1);
    match &out[0] {
        XdmItem::Node(n) => {
            assert!(matches!(n.kind(), platynui_xpath::model::NodeKind::Document));
            // Ensure its first child is <root>
            let ch: Vec<_> = n.children().collect();
            assert!(!ch.is_empty());
            assert_eq!(ch[0].name().unwrap().local, "root");
        }
        _ => panic!("expected node"),
    }
}
