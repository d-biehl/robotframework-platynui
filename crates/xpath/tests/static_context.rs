use platynui_xpath::SimpleNode;
use platynui_xpath::compiler::compile_with_context;
use platynui_xpath::compiler::ir::{ItemTypeIR, NodeTestIR, OccurrenceIR, SeqTypeIR};
use platynui_xpath::engine::runtime::{
    DynamicContextBuilder, ErrorCode, FunctionSignatures, StaticContext, StaticContextBuilder,
};
use platynui_xpath::evaluate;
use platynui_xpath::model::simple::{attr, doc as simple_doc, elem, text};
use platynui_xpath::xdm::{ExpandedName, XdmItem};
use platynui_xpath::{NodeKind, XdmNode};

#[test]
fn default_context_contains_builtin_function() {
    let ctx = StaticContext::default();
    let name =
        ExpandedName { ns_uri: StaticContext::default().default_function_namespace.clone(), local: "true".to_string() };
    assert!(ctx.function_signatures.supports(&name, 0));
}

#[test]
fn missing_signature_raises_static_error() {
    let mut ctx = StaticContext::default();
    ctx.function_signatures = FunctionSignatures::default();
    let err = compile_with_context("fn:true()", &ctx).expect_err("expected static error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0017);
}

#[test]
fn custom_signature_allows_compilation() {
    let mut ctx = StaticContext::default();
    ctx.function_signatures = FunctionSignatures::default();
    let default_ns = StaticContext::default().default_function_namespace.clone().unwrap();
    ctx.function_signatures.register_ns(&default_ns, "true", 0, Some(0));
    assert!(compile_with_context("fn:true()", &ctx).is_ok());
}

#[test]
fn default_context_has_codepoint_collation() {
    let ctx = StaticContext::default();
    assert!(ctx.statically_known_collations.contains(platynui_xpath::collation::CODEPOINT_URI));
}

#[test]
fn builder_can_add_collation() {
    let ctx = StaticContextBuilder::new().with_collation("urn:example:collation").build();
    assert!(ctx.statically_known_collations.contains("urn:example:collation"));
}

#[test]
fn builder_can_clear_context_item_type() {
    let ctx =
        StaticContextBuilder::new().with_context_item_type(SeqTypeIR::EmptySequence).clear_context_item_type().build();
    assert!(ctx.context_item_type.is_none());
}

#[test]
fn context_item_type_empty_sequence_is_static_error() {
    let ctx = StaticContextBuilder::new().with_context_item_type(SeqTypeIR::EmptySequence).build();
    let err = compile_with_context(".", &ctx).expect_err("expected static error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}

#[test]
fn context_item_type_empty_sequence_rejects_root_path() {
    let ctx = StaticContextBuilder::new().with_context_item_type(SeqTypeIR::EmptySequence).build();
    let err = compile_with_context("/foo", &ctx).expect_err("expected static error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}

#[test]
fn context_item_type_empty_sequence_rejects_root_descendant_path() {
    let ctx = StaticContextBuilder::new().with_context_item_type(SeqTypeIR::EmptySequence).build();
    let err = compile_with_context("//foo", &ctx).expect_err("expected static error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0003);
}

#[test]
fn context_item_type_allows_node_usage() {
    let ctx = StaticContextBuilder::new()
        .with_context_item_type(SeqTypeIR::Typed { item: ItemTypeIR::AnyNode, occ: OccurrenceIR::One })
        .build();
    let compiled = compile_with_context(".", &ctx).expect("compile context item");
    let doc = simple_doc().child(elem("root").child(text("v"))).build();
    let dyn_ctx = DynamicContextBuilder::<SimpleNode>::new().with_context_item(XdmItem::Node(doc.clone())).build();
    let result = evaluate(&compiled, &dyn_ctx).expect("evaluate");
    assert_eq!(result.len(), 1);
    assert!(matches!(&result[0], XdmItem::Node(n) if n == &doc));
}

#[test]
fn context_item_type_rejects_mismatched_runtime_item() {
    let ctx = StaticContextBuilder::new()
        .with_context_item_type(SeqTypeIR::Typed {
            item: ItemTypeIR::Atomic(ExpandedName {
                ns_uri: Some("http://www.w3.org/2001/XMLSchema".to_string()),
                local: "string".to_string(),
            }),
            occ: OccurrenceIR::One,
        })
        .build();
    let compiled = compile_with_context(".", &ctx).expect("compile context item");
    let doc = simple_doc().child(elem("root")).build();
    let dyn_ctx = DynamicContextBuilder::<SimpleNode>::new().with_context_item(XdmItem::Node(doc)).build();
    let err = evaluate(&compiled, &dyn_ctx).expect_err("expected runtime type error");
    assert_eq!(err.code_enum(), ErrorCode::XPTY0004);
}

#[test]
fn context_item_type_element_allows_attribute_access() {
    let ctx = StaticContextBuilder::new()
        .with_context_item_type(SeqTypeIR::Typed {
            item: ItemTypeIR::Kind(NodeTestIR::KindElement { name: None, ty: None, nillable: false }),
            occ: OccurrenceIR::One,
        })
        .build();
    let compiled = compile_with_context("@class", &ctx).expect("compile attribute access");
    let doc = simple_doc().child(elem("root").attr(attr("class", "foo"))).build();
    let element = doc.children().next().expect("element child");
    let dyn_ctx = DynamicContextBuilder::<SimpleNode>::new().with_context_item(XdmItem::Node(element)).build();
    let result = evaluate(&compiled, &dyn_ctx).expect("evaluate attribute");
    assert_eq!(result.len(), 1);
    assert!(matches!(&result[0], XdmItem::Node(n) if matches!(n.kind(), NodeKind::Attribute)));
}

#[test]
fn context_item_type_element_rejects_atomic_runtime_item() {
    let ctx = StaticContextBuilder::new()
        .with_context_item_type(SeqTypeIR::Typed {
            item: ItemTypeIR::Kind(NodeTestIR::KindElement { name: None, ty: None, nillable: false }),
            occ: OccurrenceIR::One,
        })
        .build();
    let compiled = compile_with_context(".//child", &ctx).expect("compile descendant path");
    let dyn_ctx = DynamicContextBuilder::<SimpleNode>::new()
        .with_context_item(XdmItem::Atomic(platynui_xpath::xdm::XdmAtomicValue::String("text".to_string())))
        .build();
    let err = evaluate(&compiled, &dyn_ctx).expect_err("expected type error");
    assert_eq!(err.code_enum(), ErrorCode::XPTY0004);
}
