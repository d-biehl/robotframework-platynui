use platynui_xpath::engine::runtime::ErrorCode;
use platynui_xpath::engine::runtime::{DynamicContext, DynamicContextBuilder};
use platynui_xpath::model::simple::{doc, elem};
use platynui_xpath::{XdmNode, evaluate_expr, xdm::XdmItem as I};
use rstest::{fixture, rstest};

type N = platynui_xpath::model::simple::SimpleNode;

fn ctx_with_document(document: N) -> DynamicContext<N> {
    DynamicContextBuilder::default().with_context_item(I::Node(document)).build()
}

// Fixture: context for document(root(child)) with context item set to the document node
#[fixture]
fn ctx_doc_root() -> DynamicContext<N> {
    let document = doc().child(elem("root").child(elem("child"))).build();
    ctx_with_document(document)
}

// Aspect: count
#[rstest]
fn document_node_with_element_inner_returns_one(ctx_doc_root: DynamicContext<N>) {
    let out = evaluate_expr::<N>("self::document-node(element(root))", &ctx_doc_root).unwrap();
    assert_eq!(out.len(), 1);
}

// Aspect: node kind
#[rstest]
fn document_node_with_element_inner_yields_document_node(ctx_doc_root: DynamicContext<N>) {
    let out = evaluate_expr::<N>("self::document-node(element(root))", &ctx_doc_root).unwrap();
    match &out[0] {
        I::Node(n) => assert!(matches!(n.kind(), platynui_xpath::model::NodeKind::Document)),
        _ => panic!("expected node"),
    }
}

// Aspect: static error for text() inner
#[rstest]
fn document_node_with_text_inner_is_static_error(ctx_doc_root: DynamicContext<N>) {
    let err = evaluate_expr::<N>("self::document-node(text())", &ctx_doc_root).expect_err("expected static error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}

// Aspect: static error for comment() inner
#[rstest]
fn document_node_with_comment_inner_is_static_error(ctx_doc_root: DynamicContext<N>) {
    let err = evaluate_expr::<N>("self::document-node(comment())", &ctx_doc_root).expect_err("expected static error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}
