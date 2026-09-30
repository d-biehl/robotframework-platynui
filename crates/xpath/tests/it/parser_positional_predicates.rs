//! Which predicates may count positions (`Expr::is_non_positional_predicate`). Only a predicate
//! that is provably a boolean or nodes, and reads neither `position()` nor `last()` in its own
//! focus, selects the same items per context node as over a whole sequence.

use platynui_xpath::parser::parse;
use rstest::rstest;

#[rstest]
#[case::attribute("@a")]
#[case::path_ending_in_an_axis_step("x/y")]
#[case::relative_descendant_path(".//x")]
#[case::general_comparison("@a = 'x'")]
#[case::value_comparison("@a eq 'x'")]
#[case::node_comparison(". is $n")]
#[case::and("@a and @b")]
#[case::or("@a or @b")]
#[case::not("not(@a)")]
#[case::contains("contains(@a, 'x')")]
#[case::starts_with("starts-with(@a, 'x')")]
#[case::ends_with("ends-with(@a, 'x')")]
#[case::matches("matches(@a, 'x')")]
#[case::exists("exists(x)")]
#[case::empty("empty(x)")]
#[case::boolean("boolean(x)")]
#[case::true_function("true()")]
#[case::false_function("false()")]
#[case::string_literal("'x'")]
#[case::position_in_a_nested_predicate("x[position() = 1]")]
#[case::last_in_a_nested_predicate("count(x[last()]) > 0")]
#[case::position_in_a_later_step("x/y[position() = 2]")]
fn a_non_positional_predicate(#[case] predicate: &str) {
    let parsed = parse(predicate).expect("the predicate parses");
    assert!(parsed.is_non_positional_predicate(), "[{predicate}] should be non-positional");
}

#[rstest]
#[case::integer("1")]
#[case::decimal("1.5")]
#[case::variable("$n")]
#[case::arithmetic("@a + 1")]
#[case::function_that_is_not_boolean("count(x)")]
#[case::string_function("string(@a)")]
#[case::path_ending_in_a_filter_step("x/string()")]
#[case::sequence("(1, 2)")]
#[case::context_item(".")]
#[case::position("position()")]
#[case::last("last()")]
#[case::position_in_a_comparison("position() < 3")]
#[case::last_in_a_comparison("position() = last()")]
#[case::position_in_an_operand("@a and position() = 1")]
#[case::position_in_a_function_argument("exists((position(), 1))")]
#[case::position_in_a_for_body("exists(for $i in x return position())")]
#[case::position_in_some("some $i in x satisfies position() = 1")]
#[case::last_in_every("every $i in x satisfies last() > 1")]
fn a_possibly_positional_predicate(#[case] predicate: &str) {
    let parsed = parse(predicate).expect("the predicate parses");
    assert!(!parsed.is_non_positional_predicate(), "[{predicate}] should be possibly positional");
}
