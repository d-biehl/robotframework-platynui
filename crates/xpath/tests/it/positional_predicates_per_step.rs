//! Where a positional predicate counts (spec `xpath-evaluation`): inside a step it counts per
//! context node, on a parenthesized expression over the whole sequence. Every row runs on the keyed
//! `SimpleNode` and on the keyless view of the same tree.

use platynui_xpath::engine::runtime::ErrorCode;
use platynui_xpath::xdm::XdmItem;
use platynui_xpath::{DynamicContextBuilder, SimpleNode, evaluate_expr};
use rstest::rstest;

use crate::common::{Keyless, eval_labels, tree};

const W: &str = "W:[P1:[B1,B2],B3,P2:[B4,B5]]";

fn assert_both_models(notation: &str, xpath: &str, expected: &[&str]) {
    let document = tree(notation);
    assert_eq!(eval_labels(xpath, &document), expected, "keyed model: `{xpath}` on {notation}");
    assert_eq!(eval_labels(xpath, &Keyless::new(&document)), expected, "keyless model: `{xpath}` on {notation}");
}

#[rstest]
#[case::first_match_of_every_parent(W, "//B[1]", &["B1", "B3", "B4"])]
#[case::count_of_the_first_matches(W, "count(//B[1])", &["3"])]
#[case::last_match_of_every_parent(W, "//B[last()]", &["B2", "B3", "B5"])]
#[case::positional_child_step_after_a_child_step(W, "//P/B[2]", &["B2", "B5"])]
#[case::positional_child_step_after_a_wildcard_step(W, "/W/*/B[1]", &["B1", "B4"])]
#[case::position_after_a_filter(W, "//B[@id!='B1'][1]", &["B2", "B3", "B4"])]
#[case::first_of_a_union_step_per_parent(W, "//(P|B)[1]", &["P1", "B1", "B4"])]
#[case::last_of_a_union_step_per_parent(W, "//(P|B)[last()]", &["B2", "P2", "B5"])]
#[case::nearest_ancestor(W, "//B/ancestor::*[1]", &["W", "P1", "P2"])]
#[case::nearest_preceding_sibling(W, "//B/preceding-sibling::*[1]", &["P1", "B1", "B4"])]
#[case::nearest_preceding(W, "//B/preceding::B[1]", &["B1", "B2", "B3", "B4"])]
#[case::next_following(W, "//B/following::B[1]", &["B2", "B3", "B4", "B5"])]
#[case::next_following_sibling("r:[x1,x2,y1,x3,y2]", "//x/following-sibling::y[1]", &["y1", "y2"])]
#[case::first_descendant("r:[X1:[B1,X2:[B2]]]", "//X/descendant::B[1]", &["B1", "B2"])]
#[case::position_that_no_parent_reaches(W, "//B[3]", &[])]
#[case::position_zero(W, "//B[0]", &[])]
#[case::fractional_position(W, "//B[1.5]", &[])]
#[case::non_positional_attribute_filter(W, "//B[@id='B4']", &["B4"])]
#[case::non_positional_path_filter(W, "//P[B]", &["P1", "P2"])]
fn a_positional_predicate_in_a_step_counts_per_context_node(
    #[case] notation: &str,
    #[case] xpath: &str,
    #[case] expected: &[&str],
) {
    assert_both_models(notation, xpath, expected);
}

#[rstest]
#[case::first_match_overall(W, "(//B)[1]", &["B1"])]
#[case::second_match_overall(W, "(//B)[2]", &["B2"])]
#[case::last_match_overall(W, "(//B)[last()]", &["B5"])]
#[case::third_match_of_a_child_step(W, "(//P/B)[3]", &["B4"])]
#[case::first_match_at_different_depths("r:[A:[X1],X2]", "(//X)[1]", &["X1"])]
#[case::position_past_the_end(W, "(//B)[6]", &[])]
#[case::predicates_over_an_atomic_sequence(W, "(1 to 100)[position() > 3][position() <= 5][2]", &["5"])]
fn a_predicate_on_a_parenthesized_expression_counts_over_the_whole_sequence(
    #[case] notation: &str,
    #[case] xpath: &str,
    #[case] expected: &[&str],
) {
    assert_both_models(notation, xpath, expected);
}

#[test]
fn a_predicate_error_is_still_raised() {
    let document = tree(W);
    let keyed =
        DynamicContextBuilder::<SimpleNode>::default().with_context_item(XdmItem::Node(document.clone())).build();
    let err = evaluate_expr::<SimpleNode>("//B[(1, 2)]", &keyed).expect_err("keyed model");
    assert_eq!(err.code_enum(), ErrorCode::FORG0006);
    let keyless =
        DynamicContextBuilder::<Keyless>::default().with_context_item(XdmItem::Node(Keyless::new(&document))).build();
    let err = evaluate_expr::<Keyless>("//B[(1, 2)]", &keyless).expect_err("keyless model");
    assert_eq!(err.code_enum(), ErrorCode::FORG0006);
}
