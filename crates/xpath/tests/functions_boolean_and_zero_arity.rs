use platynui_xpath::engine::runtime::{DynamicContextBuilder, ErrorCode};
use platynui_xpath::{
    evaluate_expr,
    simple_node::{doc, elem, ns, text},
};
use rstest::rstest;

fn ctx_with_text(
    t: &str,
) -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    let root = doc().child(elem("r").child(text(t))).build();
    DynamicContextBuilder::default().with_context_item(root).build()
}

fn ctx_without_item() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::default().build()
}

fn ctx_with_element() -> (
    platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode>,
    platynui_xpath::model::simple::SimpleNode,
) {
    let elem_node = elem("p:root").namespace(ns("p", "urn:one")).child(text("abc")).build();
    let ctx = DynamicContextBuilder::default().with_context_item(elem_node.clone()).build();
    (ctx, elem_node)
}

#[rstest]
fn boolean_ebv() {
    let c = ctx_with_text("");
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("boolean(())", &c).unwrap();
    assert_eq!(out.len(), 1);
    // EBV of empty is false
    if let platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) = &out[0] {
        assert!(!b);
    } else {
        panic!("bool");
    }

    let out2 = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("boolean((1))", &c).unwrap();
    if let platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Boolean(b)) = &out2[0] {
        assert!(*b);
    } else {
        panic!("bool");
    }
}

#[rstest]
fn string_zero_arity_uses_context() {
    let c = ctx_with_text("Hello");
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("string()", &c).unwrap();
    let s = match &out[0] {
        platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => s.clone(),
        _ => panic!("str"),
    };
    assert_eq!(s, "Hello");
}

#[rstest]
fn normalize_space_zero_arity() {
    let c = ctx_with_text("  A  B   C  ");
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("normalize-space()", &c).unwrap();
    let s = match &out[0] {
        platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => s.clone(),
        _ => panic!("str"),
    };
    assert_eq!(s, "A B C");
}

#[rstest]
#[case("string()")]
#[case("data()")]
#[case("number()")]
#[case("normalize-space()")]
#[case("string-length()")]
#[case("name()")]
#[case("local-name()")]
#[case("namespace-uri()")]
#[case("root()")]
#[case("base-uri()")]
#[case("document-uri()")]
fn zero_arity_without_context_item_errors(#[case] expr: &str) {
    let ctx = ctx_without_item();
    let err = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>(expr, &ctx).unwrap_err();
    assert_eq!(err.code_enum(), ErrorCode::XPDY0002);
}

#[rstest]
fn data_zero_arity_uses_context() {
    let c = ctx_with_text("42");
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("data()", &c).unwrap();
    match &out[0] {
        platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::UntypedAtomic(s)) => {
            assert_eq!(s, "42")
        }
        _ => panic!("expected untypedAtomic"),
    }
}

#[rstest]
fn number_zero_arity_uses_context() {
    let c = ctx_with_text("41");
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("number()", &c).unwrap();
    match &out[0] {
        platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Double(v)) => {
            assert_eq!(*v, 41.0)
        }
        _ => panic!("expected double"),
    }
}

#[rstest]
fn string_length_zero_arity_uses_context() {
    let c = ctx_with_text("Hello");
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("string-length()", &c).unwrap();
    match &out[0] {
        platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::Integer(len)) => {
            assert_eq!(*len, 5)
        }
        _ => panic!("expected integer"),
    }
}

#[rstest]
fn name_local_namespace_zero_arity() {
    let (ctx, elem_node) = ctx_with_element();
    let name = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("name()", &ctx).unwrap();
    match &name[0] {
        platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert_eq!(s, "p:root")
        }
        _ => panic!("expected string"),
    }

    let local = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("local-name()", &ctx).unwrap();
    match &local[0] {
        platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String(s)) => {
            assert_eq!(s, "root")
        }
        _ => panic!("expected string"),
    }

    let ns = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("namespace-uri()", &ctx).unwrap();
    match &ns[0] {
        platynui_xpath::xdm::XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::AnyUri(uri)) => {
            assert_eq!(uri, "urn:one")
        }
        _ => panic!("expected anyURI"),
    }

    let root = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("root()", &ctx).unwrap();
    match &root[0] {
        platynui_xpath::xdm::XdmItem::Node(n) => assert_eq!(n, &elem_node),
        _ => panic!("expected node"),
    }

    let base = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("base-uri()", &ctx).unwrap();
    assert!(base.is_empty());

    let doc_uri = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("document-uri()", &ctx).unwrap();
    assert!(doc_uri.is_empty());
}
