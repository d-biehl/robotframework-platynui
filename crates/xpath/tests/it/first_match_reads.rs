//! Finding the first result reads no more of the tree than it needs (spec `xpath-evaluation`, "The
//! first result does not require reading the rest of the tree"). The wide tree is `W` with 50 `P`
//! of 20 `B` each, seen through a keyless model that records whose list of children was read.

use rstest::rstest;

use crate::common::{Keyless, eval_labels, find, first_label, preorder_labels, tree, wide_tree};

#[rstest]
#[case::attribute_filter("//B[@id='B0_3']", None, 6)]
#[case::descendant_search("//B", None, 3)]
#[case::first_match_overall("(//B)[1]", None, 3)]
#[case::first_match_of_every_parent("//B[1]", None, 3)]
#[case::first_child_of_every_panel("//P/B[1]", None, 3)]
#[case::child_step_with_a_filter("//P/B[@id='B0_3']", None, 6)]
#[case::child_of_the_window("//W/P[@id='P0']", None, 2)]
#[case::second_match_of_every_parent_below_the_window(".//B[2]", Some("W"), 3)]
#[case::union_step_with_a_filter("//(P|B)[@id='B0_3']", None, 6)]
fn the_first_match_reads_only_what_precedes_it(#[case] xpath: &str, #[case] from: Option<&str>, #[case] bound: usize) {
    let document = wide_tree();
    let order = preorder_labels(&document);
    let model = Keyless::recording(&document);
    let context = from.map_or_else(|| model.clone(), |label| model.view(&find(&document, label)));

    let full = eval_labels(xpath, &context);
    model.clear_reads();
    let first = first_label(xpath, &context).unwrap_or_else(|| panic!("`{xpath}` has a first match"));
    assert_eq!(Some(&first), full.first(), "`{xpath}`: the first item must be the first of the full result");

    let position =
        |label: &str| order.iter().position(|l| l == label).unwrap_or_else(|| panic!("{label} is in the tree"));
    let reads = model.distinct_reads();
    let shown = &reads[..reads.len().min(12)];
    for read in &reads {
        assert!(
            position(read) <= position(&first),
            "`{xpath}` read the list of {read}, which comes after its first item {first}; {} lists read, first {shown:?}",
            reads.len()
        );
    }
    assert!(
        reads.len() <= bound,
        "`{xpath}` read {} lists for its first item {first}, more than {bound}: {shown:?}",
        reads.len()
    );
}

#[test]
fn a_full_descendant_search_reads_every_list() {
    let document = wide_tree();
    let model = Keyless::recording(&document);
    assert_eq!(eval_labels("//B", &model).len(), 1000);
    assert_eq!(model.distinct_reads().len(), 1052, "the document, W, 50 P and 1,000 B");
}

/// A container known to be unique, taken with `(…)[1]`, is the only one whose children are counted:
/// the whole result reads the lists up to it and its own, and nothing after it.
#[test]
fn a_child_by_position_of_a_unique_container_reads_only_that_container() {
    let document = wide_tree();
    let model = Keyless::recording(&document);
    assert_eq!(eval_labels("(//P[@id='P0'])[1]/*[20]", &model), ["B0_19"]);
    assert_eq!(model.distinct_reads(), ["#doc", "W", "P0"]);
}

#[rstest]
#[case::descendant_search("//B", "B1")]
#[case::shape_that_has_to_sort("//B/..", "W")]
fn the_first_match_is_the_first_in_document_order(#[case] xpath: &str, #[case] expected: &str) {
    let document = tree("W:[P1:[B1,B2],B3,P2:[B4,B5]]");
    assert_eq!(first_label(xpath, &document).as_deref(), Some(expected), "keyed model: `{xpath}`");
    assert_eq!(first_label(xpath, &Keyless::new(&document)).as_deref(), Some(expected), "keyless model: `{xpath}`");
}
