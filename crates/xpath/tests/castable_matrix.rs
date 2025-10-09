use platynui_xpath::engine::runtime::{DynamicContextBuilder, StaticContextBuilder};
use platynui_xpath::{
    compiler::compile_with_context, evaluate, evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I,
};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

fn bool(expr: &str) -> bool {
    let c = ctx();
    let r = evaluate_expr::<N>(expr, &c).unwrap();
    if let I::Atomic(A::Boolean(b)) = &r[0] { *b } else { panic!("expected boolean") }
}

#[rstest]
#[case("'42' castable as xs:integer", true)]
#[case("'3.14' castable as xs:integer", false)] // fractional -> FOCA0001 becomes false
#[case("'abc' castable as xs:integer", false)] // lexical invalid
#[case("'' castable as xs:anyURI", true)] // empty anyURI after collapse now valid
#[case("'http://x' castable as xs:anyURI", true)]
// untypedAtomic function-style constructor not implemented; skip direct tests
#[case("() castable as xs:integer?", true)]
#[case("() castable as xs:integer", false)]
#[case("'YWJj' castable as xs:base64Binary", true)]
#[case("'0G' castable as xs:hexBinary", false)]
#[case("'name' castable as xs:NCName", true)]
#[case("'prefix:name' castable as xs:NCName", false)]
#[case("'2024' castable as xs:gYear", true)]
fn castable_basic(#[case] expr: &str, #[case] expected: bool) {
    assert_eq!(bool(expr), expected, "expr={expr}");
}

// QName with prefix requires static namespace; provide one context for prefixed case
#[rstest]
fn castable_qname_with_prefix() {
    let static_ctx = StaticContextBuilder::new().with_namespace("p", "urn:ex").build();
    let dyn_ctx = ctx();
    let compiled_ok = compile_with_context("'p:l' castable as xs:QName", &static_ctx).unwrap();
    let compiled_bad = compile_with_context("'zzz:l' castable as xs:QName", &static_ctx).unwrap();
    let r = evaluate(&compiled_ok, &dyn_ctx).unwrap();
    if let I::Atomic(A::Boolean(b)) = &r[0] {
        assert!(*b);
    } else {
        panic!("expected boolean");
    }
    let r2 = evaluate(&compiled_bad, &dyn_ctx).unwrap();
    if let I::Atomic(A::Boolean(b)) = &r2[0] {
        assert!(!*b, "unknown prefix should not be castable");
    } else {
        panic!("expected boolean");
    }
}
