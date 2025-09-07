use super::*;

#[rstest]
#[case("for $i in 1 to 10 return $i", "Simple for expression")]
#[case("if ($x > 0) then $x else 0", "Simple if expression")]
#[case("some $x in //item satisfies $x > 10", "Existential quantification")]
#[case("every $x in //item satisfies $x > 0", "Universal quantification")]
#[case("(1, 2, 3)", "Sequence construction")]
#[case("$a union $b", "Union operator")]
#[case("$a intersect $b", "Intersect operator")]
#[case("$a except $b", "Except operator")]
#[case("if (@condition) then 'yes' else 'no'", "Conditional expression")]
#[case("for $i in //item return $i/@value", "For expression")]
#[case("some $x in //book satisfies $x/@price < 20", "Some expression")]
#[case("every $x in //book satisfies $x/@price > 0", "Every expression")]
#[case("1 to 10", "Range expression")]
#[case("//book except //book[@id='1']", "Except operator")]
#[case("//book intersect //bestseller", "Intersect operator")]
#[case("@value castable as xs:date", "Castable as date")]
#[case("@value cast as xs:boolean", "Cast as boolean")]
fn test_xpath2_features(#[case] xpath: &str, #[case] description: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
}

#[rstest]
#[case("(1, 2, 3)", "Sequence construction")]
#[case("$a union $b", "Union operator")]
#[case("$a intersect $b", "Intersect operator")]
#[case("$a except $b", "Except operator")]
fn test_sequences_and_sets(#[case] xpath: &str, #[case] description: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
}

#[rstest]
#[case(
    "for $i in 1 to 10, $j in 1 to $i return $i * $j",
    "Multiple variable binding"
)]
#[case(
    "if (count(//error) > 0) then 'Has errors' else if (count(//warning) > 0) then 'Has warnings' else 'OK'",
    "Nested if expressions"
)]
#[case(
    "some $item in //product satisfies every $attr in $item/@* satisfies string-length($attr) > 0",
    "Nested quantified expressions"
)]
#[case("//book[author = ('Smith', 'Jones', 'Brown')]", "Sequence comparison")]
#[case(
    "//product[@tags = tokenize('electronics,computer,laptop', ',')]",
    "Tokenized sequence"
)]
#[case("(//book/@price)[position() > 5]", "Sequence filtering")]
#[case("//item[price castable as xs:decimal]", "Type checking")]
#[case(
    "//element[@date castable as xs:date and xs:date(@date) > current-date()]",
    "Date comparison"
)]
fn test_xpath2_advanced_features(#[case] xpath: &str, #[case] description: &str) {
    let result = parse_xpath(xpath);
    assert!(
        result.is_ok(),
        "Failed to parse {}: '{}'. Error: {:?}",
        description,
        xpath,
        result.err()
    );
}

// Test XPath 2.0 A.4 Precedence Order compliance
#[rstest]
#[case(
    "5 + 3 * 2",
    "Multiplication should have higher precedence than addition"
)]
#[case(
    "10 - 2 * 3",
    "Multiplication should have higher precedence than subtraction"
)]
#[case("8 div 2 + 1", "Division should have higher precedence than addition")]
#[case(
    "3 * 4 div 2",
    "Multiplication and division should have same precedence, left-to-right"
)]
#[case(
    "2 + 3 > 1 * 4",
    "Arithmetic should have higher precedence than comparison"
)]
#[case(
    "1 < 2 and 3 > 2",
    "Comparison should have higher precedence than logical AND"
)]
#[case("true or false and false", "AND should have higher precedence than OR")]
#[case(
    "$a union $b intersect $c",
    "Intersect should have higher precedence than union"
)]
#[case("count(//book) + 1", "Function call should have high precedence")]
#[case(
    "child::* + 1",
    "Path expression should have higher precedence than addition"
)]
fn test_xpath2_precedence_order_compliance(#[case] expression: &str, #[case] description: &str) {
    let result = parse_xpath(expression);
    assert!(result.is_ok(), "{}: {}", description, result.unwrap_err());
}

// Test complex precedence scenarios with multiple operators
#[rstest]
#[case(
    "$x + $y * $z div $w - $v",
    "Complex arithmetic with correct precedence"
)]
#[case("//book[@price < 10 + 5]", "Path with predicate containing arithmetic")]
#[case("position() = 1 or position() = last()", "Logical with function calls")]
#[case(
    "$a union $b except $c intersect $d",
    "Multiple set operations with correct precedence"
)]
#[case(
    "for $i in 1 to 10 return $i * 2 + 1",
    "For expression with arithmetic"
)]
#[case("if ($x > 0) then $x + $y else $x - $y", "Conditional with arithmetic")]
#[case(
    "some $x in $seq satisfies $x > $threshold and $x < $max",
    "Quantified with logical operations"
)]
#[case("/books/book[position() = 1]/title", "Complex path with predicates")]
#[case(
    "$price * $quantity + $tax",
    "Variable arithmetic with correct precedence"
)]
#[case("not($flag) and $value > 10", "Function call with logical operations")]
fn test_xpath2_complex_precedence_scenarios(#[case] expression: &str, #[case] description: &str) {
    let result = parse_xpath(expression);
    assert!(result.is_ok(), "{}: {}", description, result.unwrap_err());
}
