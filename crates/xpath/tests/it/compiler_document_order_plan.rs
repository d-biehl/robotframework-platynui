//! Where the compiler normalizes a node stream: only where it cannot prove that the stream is in
//! document order, and then with one normalization op that sorts and removes duplicates.

use platynui_xpath::compiler::compile;
use platynui_xpath::compiler::ir::OpCode;
use rstest::rstest;

fn normalizations(xpath: &str) -> usize {
    let plan = compile(xpath).unwrap_or_else(|e| panic!("compile `{xpath}`: {e:?}")).instrs.0;
    plan.iter().filter(|op| matches!(op, OpCode::Normalize)).count()
}

#[rstest]
#[case::descendant_search_with_a_filter("//B[@id='x']")]
#[case::descendant_search_below_a_child_step("Window[@Name='x']//Button[@Name='y']")]
#[case::child_step_after_a_descendant_search("//Window/Button[@Name='OK']")]
#[case::positional_child_step_after_a_descendant_search(".//B[2]")]
#[case::first_match_overall("(//B)[1]")]
#[case::chain_of_positional_child_steps(".//*[@Name='t']/*[3]/*[2]")]
#[case::union_step_rewritten_into_a_descendant_step(".//(A|B)[@a]")]
fn a_path_whose_order_is_proven_is_not_normalized(#[case] xpath: &str) {
    assert_eq!(normalizations(xpath), 0, "`{xpath}`");
}

#[rstest]
#[case::parent_of_several_nodes("//B/..")]
#[case::ancestors_of_several_nodes("//B/ancestor::*")]
#[case::path_from_a_sequence("(//c, //a)/self::*")]
#[case::following_siblings_of_several_nodes("//*/following-sibling::*")]
#[case::union_step_with_a_positional_predicate(".//(A|B)[1]")]
fn a_path_whose_order_is_not_proven_is_normalized_once(#[case] xpath: &str) {
    assert_eq!(normalizations(xpath), 1, "`{xpath}`");
}
