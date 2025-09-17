use platynui_xpath::compiler::compile_xpath_with_context;
use platynui_xpath::engine::runtime::ErrorCode;
use platynui_xpath::engine::runtime::{FunctionSignatures, StaticContext, StaticContextBuilder};
use platynui_xpath::xdm::ExpandedName;

#[test]
fn default_context_contains_builtin_function() {
    let ctx = StaticContext::default();
    let name = ExpandedName {
        ns_uri: StaticContext::default().default_function_namespace.clone(),
        local: "true".to_string(),
    };
    assert!(ctx.function_signatures.supports(&name, 0));
}

#[test]
fn missing_signature_raises_static_error() {
    let ctx = StaticContext {
        function_signatures: FunctionSignatures::default(),
        ..StaticContext::default()
    };
    let err = compile_xpath_with_context("fn:true()", &ctx).expect_err("expected static error");
    assert_eq!(err.code_enum(), ErrorCode::XPST0017);
}

#[test]
fn custom_signature_allows_compilation() {
    let mut ctx = StaticContext {
        function_signatures: FunctionSignatures::default(),
        ..StaticContext::default()
    };
    let default_ns = StaticContext::default()
        .default_function_namespace
        .clone()
        .unwrap();
    ctx.function_signatures
        .register_ns(&default_ns, "true", 0, Some(0));
    assert!(compile_xpath_with_context("fn:true()", &ctx).is_ok());
}

#[test]
fn default_context_has_codepoint_collation() {
    let ctx = StaticContext::default();
    assert!(
        ctx.statically_known_collations
            .contains(platynui_xpath::collation::CODEPOINT_URI)
    );
}

#[test]
fn builder_can_add_collation() {
    let ctx = StaticContextBuilder::new()
        .with_collation("urn:example:collation")
        .build();
    assert!(
        ctx.statically_known_collations
            .contains("urn:example:collation")
    );
}
