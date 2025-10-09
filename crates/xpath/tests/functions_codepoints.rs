use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::runtime::ErrorCode;
use platynui_xpath::{engine::evaluator::evaluate_expr, xdm::XdmAtomicValue as A, xdm::XdmItem as I};
use rstest::rstest;

type N = platynui_xpath::model::simple::SimpleNode;
fn ctx() -> platynui_xpath::engine::runtime::DynamicContext<N> {
    DynamicContextBuilder::default().build()
}

#[rstest]
fn string_to_codepoints_basic() {
    let r = evaluate_expr::<N>("fn:string-to-codepoints('ABC')", &ctx()).unwrap();
    let ints: Vec<i64> =
        r.into_iter().map(|i| if let I::Atomic(A::Integer(v)) = i { v } else { panic!("expected int") }).collect();
    assert_eq!(ints, vec!['A' as i64, 'B' as i64, 'C' as i64]);
}

#[rstest]
fn codepoints_to_string_basic() {
    let r = evaluate_expr::<N>("fn:codepoints-to-string((65,66,67))", &ctx()).unwrap();
    if let I::Atomic(A::String(s)) = &r[0] {
        assert_eq!(s, "ABC");
    } else {
        panic!("expected string")
    }
}

#[rstest]
fn roundtrip_property_ascii() {
    let r = evaluate_expr::<N>("fn:codepoints-to-string(fn:string-to-codepoints('Hello'))", &ctx()).unwrap();
    if let I::Atomic(A::String(s)) = &r[0] {
        assert_eq!(s, "Hello");
    } else {
        panic!("expected string")
    }
}

#[rstest]
fn unicode_roundtrip() {
    let src = "Grüß 😊";
    let expr = format!("fn:codepoints-to-string(fn:string-to-codepoints('{}'))", src);
    let r = evaluate_expr::<N>(&expr, &ctx()).unwrap();
    if let I::Atomic(A::String(s)) = &r[0] {
        assert_eq!(s, src);
    } else {
        panic!("expected string")
    }
}

#[rstest]
fn codepoints_to_string_invalid_error() {
    // 0x110000 is above the valid Unicode range; XPath integer literals are decimal-only.
    // Use decimal 1114112 (0x110000) to trigger FORG0001 from codepoints-to-string.
    let err = evaluate_expr::<N>("fn:codepoints-to-string((1114112))", &ctx()).expect_err("should error");
    assert_eq!(err.code_enum(), ErrorCode::FORG0001);
}

#[rstest]
fn codepoints_to_string_invalid_surrogate_error() {
    // Surrogate range U+D800..U+DFFF is invalid as Unicode scalar values.
    // Pick a representative: 0xD800 (55296)
    let err = evaluate_expr::<N>("fn:codepoints-to-string((55296))", &ctx()).expect_err("should error");
    assert_eq!(err.code_enum(), ErrorCode::FORG0001);
}
