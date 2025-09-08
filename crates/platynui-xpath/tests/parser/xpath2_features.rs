use super::*;

#[rstest]
#[case::for_simple("for $i in 1 to 10 return $i")]
#[case::if_simple("if ($x > 0) then $x else 0")]
#[case::some_quantified("some $x in //item satisfies $x > 10")]
#[case::every_quantified("every $x in //item satisfies $x > 0")]
#[case::sequence_const("(1, 2, 3)")]
#[case::union("$a union $b")]
#[case::intersect("$a intersect $b")]
#[case::except("$a except $b")]
#[case::if_conditional("if (@condition) then 'yes' else 'no'")]
#[case::for_return("for $i in //item return $i/@value")]
#[case::some_book_price("some $x in //book satisfies $x/@price < 20")]
#[case::every_book_price("every $x in //book satisfies $x/@price > 0")]
#[case::range("1 to 10")]
#[case::except_path("//book except //book[@id='1']")]
#[case::intersect_path("//book intersect //bestseller")]
#[case::castable_date("@value castable as xs:date")]
#[case::cast_boolean("@value cast as xs:boolean")]
fn test_xpath2_features(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}",
        xpath,
        result.err()
    );
}

#[rstest]
#[case::sequence("(1, 2, 3)")]
#[case::set_union("$a union $b")]
#[case::set_intersect("$a intersect $b")]
#[case::set_except("$a except $b")]
fn test_sequences_and_sets(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}",
        xpath,
        result.err()
    );
}

#[rstest]
#[case::for_multi_bind(
    "for $i in 1 to 10, $j in 1 to $i return $i * $j",
)]
#[case::nested_if(
    "if (count(//error) > 0) then 'Has errors' else if (count(//warning) > 0) then 'Has warnings' else 'OK'",
)]
#[case::nested_quantified(
    "some $item in //product satisfies every $attr in $item/@* satisfies string-length($attr) > 0",
)]
#[case::sequence_comparison("//book[author = ('Smith', 'Jones', 'Brown')]")]
#[case::tokenized_sequence(
    "//product[@tags = tokenize('electronics,computer,laptop', ',')]",
)]
#[case::sequence_filtering("(//book/@price)[position() > 5]")]
#[case::type_checking("//item[price castable as xs:decimal]")]
#[case::date_comparison(
    "//element[@date castable as xs:date and xs:date(@date) > current-date()]",
)]
fn test_xpath2_advanced_features(#[case] xpath: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse '{}': {:?}",
        xpath,
        result.err()
    );
}

// Test XPath 2.0 A.4 Precedence Order compliance
#[rstest]
#[case::prec_mul_over_add(
    "5 + 3 * 2",
)]
#[case::prec_mul_over_sub(
    "10 - 2 * 3",
)]
#[case::prec_div_over_add("8 div 2 + 1")]
#[case::prec_mul_div_same_left_to_right(
    "3 * 4 div 2",
)]
#[case::prec_arith_over_cmp(
    "2 + 3 > 1 * 4",
)]
#[case::prec_cmp_over_and(
    "1 < 2 and 3 > 2",
)]
#[case::prec_and_over_or("true or false and false")]
#[case::prec_intersect_over_union(
    "$a union $b intersect $c",
)]
#[case::prec_fn_call_high("count(//book) + 1")]
#[case::prec_path_over_add(
    "child::* + 1",
)]
fn test_xpath2_precedence_order_compliance(#[case] expression: &str) {
    let result = parse_xpath(expression);
    assert!(result.is_ok(), "Precedence parse failed: {}", result.unwrap_err());
}

// Test complex precedence scenarios with multiple operators
#[rstest]
#[case::complex_arith(
    "$x + $y * $z div $w - $v",
)]
#[case::path_with_predicate("//book[@price < 10 + 5]")]
#[case::logical_with_functions("position() = 1 or position() = last()")]
#[case::multiple_set_ops(
    "$a union $b except $c intersect $d",
)]
#[case::for_with_arith(
    "for $i in 1 to 10 return $i * 2 + 1",
)]
#[case::if_with_arith("if ($x > 0) then $x + $y else $x - $y")]
#[case::quantified_with_logical(
    "some $x in $seq satisfies $x > $threshold and $x < $max",
)]
#[case::complex_path("/books/book[position() = 1]/title")]
#[case::var_arith(
    "$price * $quantity + $tax",
)]
#[case::fn_call_with_logical("not($flag) and $value > 10")]
fn test_xpath2_complex_precedence_scenarios(#[case] expression: &str) {
    let result = parse_xpath(expression);
    assert!(result.is_ok(), "Complex precedence parse failed: {}", result.unwrap_err());
}
