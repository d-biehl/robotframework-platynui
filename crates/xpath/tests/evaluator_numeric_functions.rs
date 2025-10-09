use platynui_xpath::compiler::compile;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::{attr, doc as simple_doc, elem};
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use platynui_xpath::{SimpleNode, evaluate};

fn build_numeric_attr_doc() -> SimpleNode {
    simple_doc()
        .child(
            elem("root")
                .child(elem("number").attr(attr("value", "10")))
                .child(elem("number").attr(attr("value", "20")))
                .child(elem("number").attr(attr("value", "30"))),
        )
        .build()
}

#[test]
fn sum_atomizes_attribute_nodes() {
    let doc = build_numeric_attr_doc();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc.clone())).build();

    let compiled = compile("sum(//number/@value)").expect("compile ok");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval ok");
    assert_eq!(result.len(), 1, "sum should return single item");
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(v)) => assert_eq!(*v, 60),
        XdmItem::Atomic(XdmAtomicValue::Decimal(v)) => assert_eq!(*v, 60.0),
        XdmItem::Atomic(XdmAtomicValue::Double(v)) => assert!((*v - 60.0).abs() < f64::EPSILON),
        other => panic!("unexpected sum result: {other:?}"),
    }
}

#[test]
fn avg_atomizes_attribute_nodes() {
    let doc = build_numeric_attr_doc();
    let ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc.clone())).build();

    let compiled = compile("avg(//number/@value)").expect("compile ok");
    let result = evaluate::<SimpleNode>(&compiled, &ctx).expect("eval ok");
    assert_eq!(result.len(), 1, "avg should return single item");
    match &result[0] {
        XdmItem::Atomic(XdmAtomicValue::Integer(v)) => assert_eq!(*v, 20),
        XdmItem::Atomic(XdmAtomicValue::Decimal(v)) => assert_eq!(*v, 20.0),
        XdmItem::Atomic(XdmAtomicValue::Double(v)) => assert!((*v - 20.0).abs() < f64::EPSILON),
        other => panic!("unexpected avg result: {other:?}"),
    }
}
