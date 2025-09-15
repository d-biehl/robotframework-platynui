use platynui_xpath::engine::runtime::{DynamicContextBuilder, ErrorCode};
use platynui_xpath::{xdm::XdmItem, model::XdmNode, engine::evaluator::evaluate_expr};

type N = platynui_xpath::model::simple::SimpleNode;

#[test]
fn element_name_and_wildcard() {
    use platynui_xpath::model::simple::{doc, elem, text};
    let d = doc()
        .child(elem("root").child(elem("child").child(text("t"))))
        .build();
    let ctx = DynamicContextBuilder::<N>::default()
        .with_context_item(d)
        .build();
    // element(root)
    let a = evaluate_expr::<N>("/element(root)", &ctx).unwrap();
    assert_eq!(a.len(), 1);
    match &a[0] {
        XdmItem::Node(n) => assert_eq!(n.name().unwrap().local, "root"),
        _ => panic!(),
    }
    // element(*)
    let b = evaluate_expr::<N>("/element(*)", &ctx).unwrap();
    assert_eq!(b.len(), 1);
    // element(child)
    let c = evaluate_expr::<N>("/element(root)/element(child)", &ctx).unwrap();
    assert_eq!(c.len(), 1);
    match &c[0] {
        XdmItem::Node(n) => assert_eq!(n.name().unwrap().local, "child"),
        _ => panic!(),
    }
}

#[test]
fn attribute_name_and_wildcard() {
    use platynui_xpath::model::simple::{doc, elem};
    let d = doc()
        .child(elem("root").attr(platynui_xpath::model::simple::attr("id", "1")))
        .build();
    let ctx = DynamicContextBuilder::<N>::default()
        .with_context_item(d)
        .build();
    // attribute axis via abbreviation
    let a = evaluate_expr::<N>("/element(root)/@id", &ctx).unwrap();
    assert_eq!(a.len(), 1);
    match &a[0] {
        XdmItem::Node(n) => assert_eq!(n.name().unwrap().local, "id"),
        _ => panic!(),
    }
    // attribute wildcard
    let b = evaluate_expr::<N>("/element(root)/@*", &ctx).unwrap();
    assert_eq!(b.len(), 1);
}

#[test]
fn element_type_arg_rejected_without_schema_awareness() {
    let expr = "element(root, xs:string)";
    let err = platynui_xpath::compile_xpath(expr).expect_err("expected static error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}

#[test]
fn schema_element_rejected_without_schema_awareness() {
    let expr = "schema-element(root)";
    let err = platynui_xpath::compile_xpath(expr).expect_err("expected static error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}
