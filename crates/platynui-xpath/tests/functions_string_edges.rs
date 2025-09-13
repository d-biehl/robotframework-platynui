use platynui_xpath::evaluator::evaluate_expr;
use platynui_xpath::runtime::DynamicContextBuilder;
use platynui_xpath::simple_node::SimpleNode;
use rstest::rstest;

fn dc() -> platynui_xpath::runtime::DynamicContext<SimpleNode> {
    DynamicContextBuilder::new().build()
}

// substring-before edge cases
#[rstest]
fn substring_before_empty_needle() {
    let d = dc();
    let r = evaluate_expr::<SimpleNode>("fn:substring-before('abc','')", &d).unwrap();
    assert_eq!(r[0].to_string(), "String(\"\")");
}

#[rstest]
fn substring_before_not_found() {
    let d = dc();
    let r = evaluate_expr::<SimpleNode>("fn:substring-before('abc','z')", &d).unwrap();
    assert_eq!(r[0].to_string(), "String(\"\")");
}

#[rstest]
fn substring_before_unicode_multibyte() {
    let d = dc();
    // needle is multi-byte snowman
    let r = evaluate_expr::<SimpleNode>("fn:substring-before('hi☃there','☃')", &d).unwrap();
    assert!(r[0].to_string().contains("hi"));
}

// substring-after edge cases
#[rstest]
fn substring_after_empty_needle_returns_original() {
    let d = dc();
    let r = evaluate_expr::<SimpleNode>("fn:substring-after('abc','')", &d).unwrap();
    assert!(r[0].to_string().contains("abc"));
}

#[rstest]
fn substring_after_not_found() {
    let d = dc();
    let r = evaluate_expr::<SimpleNode>("fn:substring-after('abc','z')", &d).unwrap();
    assert_eq!(r[0].to_string(), "String(\"\")");
}

#[rstest]
fn substring_after_unicode_multibyte() {
    let d = dc();
    let r = evaluate_expr::<SimpleNode>("fn:substring-after('hi☃there','☃')", &d).unwrap();
    assert!(r[0].to_string().contains("there"));
}

// translate edge cases
#[rstest]
fn translate_basic_mapping() {
    let d = dc();
    let r = evaluate_expr::<SimpleNode>("fn:translate('abracadabra','abc','xyz')", &d).unwrap();
    // mapping: a->x, b->y, c->z ; other chars unchanged
    let s = r[0].to_string();
    assert!(
        s.contains("xyrxzxdxyrx"),
        "unexpected translate result: {s}"
    );
}

#[rstest]
fn translate_removal() {
    let d = dc();
    // map 'abc', but only 'a' and 'b' get replacements; 'c' removed
    let r = evaluate_expr::<SimpleNode>("fn:translate('abcabc','abc','XY')", &d).unwrap();
    assert!(r[0].to_string().contains("XYXY"));
}

#[rstest]
fn translate_duplicate_map_chars_only_first_counts() {
    let d = dc();
    // second 'a' in map ignored; ensures stability
    let r = evaluate_expr::<SimpleNode>("fn:translate('aa','aa','ZQ')", &d).unwrap();
    assert!(r[0].to_string().contains("ZZ"));
}
