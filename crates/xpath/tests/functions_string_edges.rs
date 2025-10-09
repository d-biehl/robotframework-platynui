use platynui_xpath::engine::evaluator::evaluate_expr;
use platynui_xpath::engine::runtime::DynamicContextBuilder;
use platynui_xpath::xdm::{XdmAtomicValue, XdmItem};
use rstest::rstest;

fn dc() -> platynui_xpath::engine::runtime::DynamicContext<platynui_xpath::model::simple::SimpleNode> {
    DynamicContextBuilder::new().build()
}

// substring-before edge cases
#[rstest]
fn substring_before_empty_needle() {
    let d = dc();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:substring-before('abc','')", &d).unwrap();
    match &r[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert!(s.is_empty()),
        other => panic!("expected empty String, got {other:?}"),
    }
}

#[rstest]
fn substring_before_not_found() {
    let d = dc();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:substring-before('abc','z')", &d).unwrap();
    match &r[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert!(s.is_empty()),
        other => panic!("expected empty String, got {other:?}"),
    }
}

#[rstest]
fn substring_before_unicode_multibyte() {
    let d = dc();
    // needle is multi-byte snowman
    let r =
        evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:substring-before('hi☃there','☃')", &d).unwrap();
    match &r[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert!(s.contains("hi")),
        other => panic!("expected String containing 'hi', got {other:?}"),
    }
}

// substring-after edge cases
#[rstest]
fn substring_after_empty_needle_returns_original() {
    let d = dc();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:substring-after('abc','')", &d).unwrap();
    match &r[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert!(s.contains("abc")),
        other => panic!("expected String containing 'abc', got {other:?}"),
    }
}

#[rstest]
fn substring_after_not_found() {
    let d = dc();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:substring-after('abc','z')", &d).unwrap();
    match &r[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert!(s.is_empty()),
        other => panic!("expected empty String, got {other:?}"),
    }
}

#[rstest]
fn substring_after_unicode_multibyte() {
    let d = dc();
    let r =
        evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:substring-after('hi☃there','☃')", &d).unwrap();
    match &r[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert!(s.contains("there")),
        other => panic!("expected String containing 'there', got {other:?}"),
    }
}

// translate edge cases
#[rstest]
fn translate_basic_mapping() {
    let d = dc();
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:translate('abracadabra','abc','xyz')", &d)
        .unwrap();
    // mapping: a->x, b->y, c->z ; other chars unchanged
    match &r[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => {
            assert!(s.contains("xyrxzxdxyrx"), "unexpected translate result: {s}")
        }
        other => panic!("expected String, got {other:?}"),
    }
}

#[rstest]
fn translate_removal() {
    let d = dc();
    // map 'abc', but only 'a' and 'b' get replacements; 'c' removed
    let r =
        evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:translate('abcabc','abc','XY')", &d).unwrap();
    match &r[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert!(s.contains("XYXY")),
        other => panic!("expected String containing 'XYXY', got {other:?}"),
    }
}

#[rstest]
fn translate_duplicate_map_chars_only_first_counts() {
    let d = dc();
    // second 'a' in map ignored; ensures stability
    let r = evaluate_expr::<platynui_xpath::model::simple::SimpleNode>("fn:translate('aa','aa','ZQ')", &d).unwrap();
    match &r[0] {
        XdmItem::Atomic(XdmAtomicValue::String(s)) => assert!(s.contains("ZZ")),
        other => panic!("expected String containing 'ZZ', got {other:?}"),
    }
}
