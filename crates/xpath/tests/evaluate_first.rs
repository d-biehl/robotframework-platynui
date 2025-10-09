use platynui_xpath::simple_node::{attr, doc as simple_doc, elem, text};
use platynui_xpath::xdm::XdmItem;
use platynui_xpath::{DynamicContextBuilder, SimpleNode, XdmNode, compile, evaluate_first, evaluate_first_expr};

#[test]
fn test_evaluate_first_with_results() {
    let doc = simple_doc()
        .child(
            elem("root")
                .child(elem("item").child(text("first")))
                .child(elem("item").child(text("second")))
                .child(elem("item").child(text("third"))),
        )
        .build();
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();

    let compiled = compile("//item").unwrap();
    let result = evaluate_first(&compiled, &ctx).unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        XdmItem::Node(node) => {
            assert_eq!(node.string_value(), "first");
        }
        _ => panic!("Expected node"),
    }
}

#[test]
fn test_evaluate_first_empty_sequence() {
    let doc = simple_doc().child(elem("root")).build();
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();

    let compiled = compile("//item").unwrap();
    let result = evaluate_first(&compiled, &ctx).unwrap();

    assert!(result.is_none());
}

#[test]
fn test_evaluate_first_with_predicate() {
    let doc = simple_doc()
        .child(
            elem("root")
                .child(elem("item").attr(attr("id", "a")))
                .child(elem("item").attr(attr("id", "b")))
                .child(elem("item").attr(attr("id", "c"))),
        )
        .build();
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc.clone())).build();

    let compiled = compile("//item[@id='b']").unwrap();
    let result = evaluate_first(&compiled, &ctx).unwrap();

    assert!(result.is_some());

    // Verify it's the correct item by checking the attribute via XPath
    let verify_ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(result.unwrap()).build();
    let id_value = evaluate_first_expr::<SimpleNode>("@id", &verify_ctx).unwrap();
    match id_value {
        Some(XdmItem::Node(attr_node)) => {
            assert_eq!(attr_node.string_value(), "b");
        }
        _ => panic!("Expected attribute node"),
    }
}

#[test]
fn test_evaluate_first_expr_convenience() {
    let doc = simple_doc()
        .child(elem("root").child(elem("item").child(text("value1"))).child(elem("item").child(text("value2"))))
        .build();
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();

    let result = evaluate_first_expr::<SimpleNode>("//item", &ctx).unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        XdmItem::Node(node) => {
            assert_eq!(node.string_value(), "value1");
        }
        _ => panic!("Expected node"),
    }
}

#[test]
fn test_evaluate_first_position_predicate() {
    let doc = simple_doc()
        .child(
            elem("root")
                .child(elem("item").child(text("1")))
                .child(elem("item").child(text("2")))
                .child(elem("item").child(text("3"))),
        )
        .build();
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();

    // Note: Due to current limitation, [1] doesn't early-exit
    // But evaluate_first() still provides fast-path by stopping at first result
    let compiled = compile("//item[1]").unwrap();
    let result = evaluate_first(&compiled, &ctx).unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        XdmItem::Node(node) => {
            assert_eq!(node.string_value(), "1");
        }
        _ => panic!("Expected node"),
    }
}

#[test]
fn test_evaluate_first_large_tree_performance() {
    // Build a tree with many items
    let mut root = elem("root");
    for i in 0..1000 {
        root = root.child(elem("item").child(text(&i.to_string())));
    }
    let doc = simple_doc().child(root).build();
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();

    let compiled = compile("//item").unwrap();

    // This should stop after finding first item, not traverse all 1000
    let result = evaluate_first(&compiled, &ctx).unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        XdmItem::Node(node) => {
            assert_eq!(node.string_value(), "0");
        }
        _ => panic!("Expected node"),
    }
}

#[test]
fn test_evaluate_first_atomic_value() {
    let doc = simple_doc().child(elem("root")).build();
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();

    let compiled = compile("1 to 100").unwrap();
    let result = evaluate_first(&compiled, &ctx).unwrap();

    assert!(result.is_some());
    match result.unwrap() {
        XdmItem::Atomic(val) => {
            assert_eq!(val.to_string(), "1");
        }
        _ => panic!("Expected atomic value"),
    }
}

#[test]
fn test_evaluate_first_exists_pattern() {
    let doc = simple_doc()
        .child(
            elem("root")
                .child(elem("item").attr(attr("status", "error")))
                .child(elem("item").attr(attr("status", "ok"))),
        )
        .build();
    let ctx = DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(doc)).build();

    // Existence check pattern
    let has_error = evaluate_first_expr::<SimpleNode>("//item[@status='error']", &ctx).unwrap().is_some();
    assert!(has_error);

    let has_warning = evaluate_first_expr::<SimpleNode>("//item[@status='warning']", &ctx).unwrap().is_some();
    assert!(!has_warning);
}
