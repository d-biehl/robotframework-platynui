use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

fn run(expr: &str) -> Result<Vec<I<N>>, platynui_xpath::engine::runtime::Error> {
    evaluate_expr::<N>(expr, &ctx())
}
fn expect_err(expr: &str, frag: &str) {
    let e = run(expr).unwrap_err();
    assert_eq!(e.code_enum(), ErrorCode::XPTY0004);
    assert!(e.message.contains(frag), "expected fragment {frag} in '{}'; got '{}'", expr, e.message);
}

#[rstest]
#[case("(1) treat as xs:integer", "1")] // simple numeric
#[case("('a') treat as xs:string?", "a")] // optional ok
fn treat_success_scalar_int(#[case] expr: &str, #[case] expect: &str) {
    let r = run(expr).unwrap();
    assert_eq!(r.len(), 1);
    // Fallback Debug formatting since Display not implemented; rely on Debug string containing the literal
    let dbg = format!("{:?}", &r[0]);
    assert!(dbg.contains(expect), "debug repr {dbg} did not contain {expect}");
}

#[rstest]
fn treat_empty_optional_ok() {
    run("() treat as xs:string?").unwrap();
}

#[rstest]
fn treat_empty_required_error() {
    expect_err("() treat as xs:string", "cardinality mismatch");
}

#[rstest]
fn treat_too_many_error() {
    expect_err("(1,2) treat as xs:integer", "cardinality mismatch");
}

#[rstest]
fn treat_type_mismatch_error() {
    expect_err("('a') treat as xs:integer", "type mismatch");
}

#[rstest]
fn treat_type_mismatch_after_cardinality_ok() {
    expect_err("(1) treat as xs:string", "type mismatch");
}
