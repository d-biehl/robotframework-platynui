//! A path returns its nodes in document order, each once (spec `xpath-evaluation`, "A path returns
//! its nodes in document order without duplicates"). Every row runs on the keyed `SimpleNode` and on
//! the keyless view of the same tree; the atomic scenario lives in `evaluator_path_filter_expr.rs`.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use platynui_xpath::engine::runtime::ErrorCode;
use platynui_xpath::xdm::XdmItem;
use platynui_xpath::{DynamicContextBuilder, evaluate_expr};
use rstest::rstest;

use crate::common::{Keyless, eval_labels, eval_labels_with_other, keyless_tree_missing_b, tree, two_documents};

const W: &str = "W:[P1:[B1,B2],B3,P2:[B4,B5]]";

#[rstest]
#[case::matches_at_different_depths("r:[A:[X1],X2]", "//X", &["X1", "X2"])]
#[case::descendant_search_over_nested_parents(W, "//B", &["B1", "B2", "B3", "B4", "B5"])]
#[case::child_step_over_nested_contexts("r:[X1:[X2:[Y1],Y2]]", "//X/Y", &["Y1", "Y2"])]
#[case::following_sibling_from_several_contexts("r:[a:[b1,b2],c]", "//*/following-sibling::*", &["b2", "c"])]
#[case::child_step_from_several_contexts("r:[a:[b1,b2],c]", "//*/child::*", &["a", "b1", "b2", "c"])]
#[case::following_from_nested_contexts("r:[a1:[a2,c1]]", "//a/following::c", &["c1"])]
#[case::following_from_deeply_nested_contexts("r:[a1:[a2:[a3],c1],c2]", "//a/following::c", &["c1", "c2"])]
#[case::path_from_an_arbitrary_sequence("r:[a,b,c]", "(//b, //c, //a)/.", &["a", "b", "c"])]
#[case::path_from_a_sequence_with_duplicates("r:[a,b,c]", "(//c, //a, //c)/self::*", &["a", "c"])]
#[case::parent_step_from_nodes_out_of_order(W, "(//B[@id='B1'], //B[@id='B4'], //B[@id='B3'])/..", &["W", "P1", "P2"])]
#[case::union_step_after_descendant_search(W, "//(P|B)", &["P1", "B1", "B2", "B3", "P2", "B4", "B5"])]
#[case::union_step_with_a_predicate(W, "//(P|B)[@id!='B1']", &["P1", "B2", "B3", "P2", "B4", "B5"])]
#[case::control_last_match_overall(W, "(//B)[last()]", &["B5"])]
#[case::control_parents_of_all_matches(W, "//B/..", &["W", "P1", "P2"])]
fn a_path_returns_its_nodes_in_document_order(#[case] notation: &str, #[case] xpath: &str, #[case] expected: &[&str]) {
    let document = tree(notation);
    assert_eq!(eval_labels(xpath, &document), expected, "keyed model: `{xpath}` on {notation}");
    assert_eq!(eval_labels(xpath, &Keyless::new(&document)), expected, "keyless model: `{xpath}` on {notation}");
}

#[rstest]
#[case::path_from_a_sequence("(., ../*)/self::*", &["a", "b", "c"])]
#[case::union("(. | //*)", &["a", "b", "c", "r"])]
fn a_node_missing_from_its_parents_list_keeps_a_stable_place(#[case] xpath: &str, #[case] members: &[&str]) {
    // The document view keeps the tree alive: a node holds its ancestors only weakly.
    let (_document, b) = keyless_tree_missing_b();
    let first = eval_labels(xpath, &b);
    assert_eq!(eval_labels(xpath, &b), first, "`{xpath}` must give the same order on every run");
    let mut sorted = first.clone();
    sorted.sort();
    assert_eq!(sorted, members, "`{xpath}` must return each node once, got {first:?}");
}

#[rstest]
#[case::path_from_a_sequence("(//x, $other//x)/.")]
#[case::union("$other//x | //x")]
fn nodes_of_two_documents_keep_a_stable_order(#[case] xpath: &str) {
    let (first_document, second_document) = two_documents();
    let keyed = eval_labels_with_other(xpath, &first_document, &second_document);
    assert_eq!(eval_labels_with_other(xpath, &first_document, &second_document), keyed, "keyed model: `{xpath}`");
    let (first_view, second_view) = (Keyless::new(&first_document), Keyless::new(&second_document));
    let keyless = eval_labels_with_other(xpath, &first_view, &second_view);
    assert_eq!(eval_labels_with_other(xpath, &first_view, &second_view), keyless, "keyless model: `{xpath}`");
    for result in [&keyed, &keyless] {
        let mut sorted = result.clone();
        sorted.sort();
        assert_eq!(sorted, ["x1", "x2"], "`{xpath}` must return each node once, got {result:?}");
    }
}

/// Spec `xpath-evaluation`, "Cancellation stops a sort": the flag is set while `//*/..` collects
/// the parents it has to sort. Only the sort itself can notice, because the walk below it tests
/// every element with `*` and never asks for the flag.
#[test]
fn cancellation_stops_a_sort() {
    let document = tree("W:[P1:[B1,B2],B3,P2:[B4,B5]]");
    let flag = Arc::new(AtomicBool::new(false));
    let model = Keyless::cancelling(&document, &flag, 3);
    let ctx = DynamicContextBuilder::default()
        .with_context_item(XdmItem::Node(model))
        .with_cancel_flag(Arc::clone(&flag))
        .build();
    let err = evaluate_expr::<Keyless>("//*/..", &ctx).expect_err("the evaluation is cancelled");
    assert_eq!(err.code_enum(), ErrorCode::FOER0000);
    assert!(err.to_string().contains("cancelled"), "the cancellation error, got {err}");
}
