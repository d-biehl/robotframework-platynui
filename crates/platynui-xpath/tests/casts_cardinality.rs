use platynui_xpath::engine::evaluator::evaluate_expr;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::runtime::ErrorCode;
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

fn expect_err(expr: &str, frag: &str) {
    let c = ctx();
    let err = evaluate_expr::<N>(expr, &c).unwrap_err();
    assert!(
        match frag {
            "FORG0004" => err.code_enum() == ErrorCode::FORG0004,
            "FORG0005" => err.code_enum() == ErrorCode::FORG0005,
            _ => err.code_qname().unwrap().local.contains(frag),
        },
        "expected fragment {frag} in {:?}",
        err.code_qname()
    );
}

// () cast as xs:integer?  ==> empty sequence (allowed because optional)
#[rstest]
fn cast_empty_optional_success() {
    let c = ctx();
    let r = evaluate_expr::<N>("() cast as xs:integer?", &c).unwrap();
    assert!(r.is_empty(), "expected empty sequence, got {:?}", r);
}

// () cast as xs:integer  ==> static error XPST0003 (empty not allowed without '?')
#[rstest]
fn cast_empty_required_error() {
    expect_err("() cast as xs:integer", "XPST0003");
}

// (1,2) cast as xs:integer?  ==> dynamic error XPTY0004 (multi-item sequence cannot be cast)
#[rstest]
fn cast_multi_item_error() {
    expect_err("(1,2) cast as xs:integer?", "XPTY0004");
}

// (() ) with extra parens still optional allowed
#[rstest]
fn cast_empty_extra_parens_optional() {
    let c = ctx();
    let r = evaluate_expr::<N>("(()) cast as xs:boolean?", &c).unwrap();
    assert!(r.is_empty());
}

// empty sequence castable as optional vs required
#[rstest]
fn castable_empty_optional_true_required_false() {
    let c = ctx();
    let t1 = evaluate_expr::<N>("() castable as xs:decimal?", &c).unwrap();
    assert_eq!(t1.len(), 1);
    let t1v = format!("{:?}", t1[0]);
    assert!(t1v.contains("Boolean(true)"), "expected true got {t1v}");
    let t2 = evaluate_expr::<N>("() castable as xs:decimal", &c).unwrap();
    let t2v = format!("{:?}", t2[0]);
    assert!(t2v.contains("Boolean(false)"), "expected false got {t2v}");
}

// multi-item sequence is not castable regardless of optionality
#[rstest]
fn castable_multi_item_false() {
    let c = ctx();
    let r = evaluate_expr::<N>("(1,2) castable as xs:integer?", &c).unwrap();
    let v = format!("{:?}", r[0]);
    assert!(v.contains("Boolean(false)"));
}

// successful cast then treat to enforce cardinality stays empty
#[rstest]
fn cast_then_treat_optional_empty() {
    let c = ctx();
    let r = evaluate_expr::<N>("(() cast as xs:string?) treat as xs:string?", &c).unwrap();
    assert!(r.is_empty());
}

// cast required then treat as optional (should error at cast step before treat)
#[rstest]
fn cast_required_then_treat_optional_error() {
    expect_err("(() cast as xs:string) treat as xs:string?", "XPST0003");
}

// double optional markers are not syntax but we can check nested optional logic via two casts
#[rstest]
fn cast_optional_chain_remains_empty() {
    let c = ctx();
    let r = evaluate_expr::<N>("(() cast as xs:integer?) cast as xs:integer?", &c).unwrap();
    assert!(r.is_empty());
}
