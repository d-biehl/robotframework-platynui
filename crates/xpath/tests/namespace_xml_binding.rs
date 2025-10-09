use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::model::XdmNode;
use platynui_xpath::{
    evaluate_expr,
    simple_node::{doc, elem, ns, text},
};
use rstest::rstest;

fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    let document = doc().child(elem("r").child(text("x"))).build();
    let root = document.children().next().unwrap();
    DynamicContextBuilder::default().with_context_item(root).build()
}

#[rstest]
fn xml_namespace_binding_always_present() {
    let c = ctx();
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("namespace::xml", &c).unwrap();
    assert_eq!(out.len(), 1);
    let uri = match &out[0] {
        platynui_xpath::xdm::XdmItem::Node(n) => n.string_value(),
        _ => panic!("node"),
    };
    assert_eq!(uri, "http://www.w3.org/XML/1998/namespace");
}

#[rstest]
fn xml_namespace_not_overridden() {
    // If document defines xml prefix with different URI, it must be ignored; synthesized binding remains
    let docu = doc().child(elem("r").namespace(ns("xml", "http://example.com/not-xml")).child(text("x"))).build();
    let root = docu.children().next().unwrap();
    let c = DynamicContextBuilder::default().with_context_item(root).build();
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("namespace::xml", &c).unwrap();
    assert_eq!(out.len(), 1);
    let uri = match &out[0] {
        platynui_xpath::xdm::XdmItem::Node(n) => n.string_value(),
        _ => panic!("node"),
    };
    assert_eq!(uri, "http://www.w3.org/XML/1998/namespace");
}

#[rstest]
fn xml_namespace_no_duplicate_when_declared() {
    let docu =
        doc().child(elem("r").namespace(ns("xml", "http://www.w3.org/XML/1998/namespace")).child(text("x"))).build();
    let root = docu.children().next().unwrap();
    let c = DynamicContextBuilder::default().with_context_item(root).build();
    let out = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("namespace::xml", &c).unwrap();
    assert_eq!(out.len(), 1);
}
