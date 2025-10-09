use platynui_xpath::compiler::compile_with_context;
use platynui_xpath::engine::runtime::{DynamicContextBuilder, StaticContextBuilder};
use platynui_xpath::evaluate;
use platynui_xpath::model::simple::{SimpleNode, doc, elem, text};
use platynui_xpath::xdm::{ExpandedName, XdmAtomicValue, XdmItem};

fn build_document() -> SimpleNode {
    doc()
        .child(
            elem("root")
                .child(
                    elem("section")
                        .child(elem("item").child(elem("value").child(text("5"))))
                        .child(elem("item").child(elem("value").child(text("9"))))
                        .child(elem("item").child(elem("value").child(text("1")))),
                )
                .child(elem("section").child(elem("item").child(elem("value").child(text("7"))))),
        )
        .build()
}

#[test]
fn filter_step_in_path_evaluates() {
    let expr = "sum(for $s in /root/section return sum($s/item[position() <= $top]/xs:integer(value)))";
    let doc = build_document();
    let var_name = ExpandedName { ns_uri: None, local: "top".to_string() };
    let static_ctx = StaticContextBuilder::new().with_variable(var_name.clone()).build();
    let compiled = compile_with_context(expr, &static_ctx).expect("compile ok");
    let dynamic_ctx = DynamicContextBuilder::default()
        .with_context_item(XdmItem::Node(doc.clone()))
        .with_variable(var_name.clone(), vec![XdmItem::Atomic(XdmAtomicValue::Integer(2))])
        .build();

    let result = evaluate::<SimpleNode>(&compiled, &dynamic_ctx).expect("eval ok");
    assert_eq!(result, vec![XdmItem::Atomic(XdmAtomicValue::Integer(21))]);
}

#[test]
fn filter_step_with_descendant_insertion() {
    let expr = "/root//item/xs:integer(value)";
    let doc = build_document();
    let dynamic_ctx = DynamicContextBuilder::default().with_context_item(XdmItem::Node(doc)).build();
    let compiled = compile_with_context(expr, &StaticContextBuilder::new().build()).expect("compile ok");
    let result = evaluate::<SimpleNode>(&compiled, &dynamic_ctx).expect("eval ok");
    let ints: Vec<i64> = result
        .into_iter()
        .filter_map(|item| match item {
            XdmItem::Atomic(XdmAtomicValue::Integer(i)) => Some(i),
            _ => None,
        })
        .collect();
    assert_eq!(ints, vec![5, 9, 1, 7]);
}

#[test]
fn path_from_with_filter_step() {
    let expr = "(1, 2, 3)/xs:integer(.)";
    let compiled = compile_with_context(expr, &StaticContextBuilder::new().build()).expect("compile ok");
    let dyn_ctx = DynamicContextBuilder::<SimpleNode>::default().build();
    let result = evaluate::<SimpleNode>(&compiled, &dyn_ctx).expect("eval ok");
    let ints: Vec<i64> = result
        .into_iter()
        .filter_map(|item| match item {
            XdmItem::Atomic(XdmAtomicValue::Integer(i)) => Some(i),
            _ => None,
        })
        .collect();
    assert_eq!(ints, vec![1, 2, 3]);
}
