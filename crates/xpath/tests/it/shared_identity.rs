//! Walks over siblings that the model reports as one node (spec `xpath-evaluation`, "Walks end
//! when siblings share an identity", and "Cancellation stops every walk"). The model is the
//! keyless view that reports the chosen labels as one node and tells every listed node its
//! position, as the runtime does for a provider whose siblings share one runtime id.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, mpsc};
use std::time::Duration;

use platynui_xpath::engine::runtime::{Error, ErrorCode};
use platynui_xpath::xdm::XdmItem;
use platynui_xpath::{DynamicContextBuilder, XdmNode, evaluate_expr, evaluate_first_expr};
use rstest::rstest;

use crate::common::{Keyless, eval_labels, find, tree, wide_tree};

/// The sharing view itself: `A` and `B` are one node to it, and each knows where it is listed.
#[test]
fn the_sharing_view_reports_equal_siblings_with_their_positions() {
    let document = tree("W:[A,B,E]");
    let model = Keyless::sharing(&document, &["A", "B"]);
    let window = model.view(&find(&document, "W"));
    let children: Vec<Keyless> = window.children().collect();

    assert_eq!(children[0], children[1], "the shared siblings compare equal");
    assert_ne!(children[1], children[2], "an unshared sibling stays distinct");
    assert_eq!(children[0].identity_hint(), children[1].identity_hint(), "equal nodes share a hint");
    let positions: Vec<Option<usize>> = children.iter().map(Keyless::position).collect();
    assert_eq!(positions, [Some(0), Some(1), Some(2)]);
    assert_eq!(model.view(&find(&document, "E")).position(), None, "a view made directly has no position");
}

/// How long a walk may take before it counts as one that never ends. The trees are tiny, so this
/// only catches a loop.
const DEADLINE: Duration = Duration::from_secs(5);

/// Runs `work` on its own thread and returns its result, or fails the test once [`DEADLINE`] has
/// passed, so that a walk that never ends fails instead of hanging the run.
fn within_deadline<T: Send + 'static>(what: &str, work: impl FnOnce() -> T + Send + 'static) -> T {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(work());
    });
    receiver.recv_timeout(DEADLINE).unwrap_or_else(|_| panic!("`{what}` did not end within {DEADLINE:?}"))
}

/// The labels of `xpath` on the tree `W:[A,B,E]`, in which `A` and `B` are one node to the model.
fn labels_with_a_and_b_shared(xpath: &'static str) -> Vec<String> {
    within_deadline(xpath, move || {
        let document = tree("W:[A,B,E]");
        eval_labels(xpath, &Keyless::sharing(&document, &["A", "B"]))
    })
}

#[test]
fn a_descendant_search_past_equal_siblings_finds_the_node_after_them() {
    assert_eq!(labels_with_a_and_b_shared("//E"), ["E"]);
    let count: usize = labels_with_a_and_b_shared("count(//*)")[0].parse().expect("an integer");
    assert!(count <= 4, "`count(//*)` counts no more nodes than the tree has, got {count}");
}

#[test]
fn a_child_step_past_equal_siblings_ends() {
    let children = labels_with_a_and_b_shared("/W/*");
    assert!(children.contains(&"E".to_string()), "`/W/*` reaches E, got {children:?}");
    assert!(children.len() <= 3, "`/W/*` holds at most the three children, got {children:?}");
    assert_eq!(labels_with_a_and_b_shared("/W/E"), ["E"]);
    assert_eq!(labels_with_a_and_b_shared("/W/*[3]"), ["E"], "positions count over the listed children");
}

#[test]
fn sibling_axes_over_equal_siblings_end() {
    let following = labels_with_a_and_b_shared("/W/*[1]/following-sibling::*");
    assert!(following.contains(&"E".to_string()), "the following siblings reach E, got {following:?}");
    assert!(following.len() <= 2, "at most two following siblings, got {following:?}");
    let preceding = labels_with_a_and_b_shared("/W/E/preceding-sibling::*");
    assert!(preceding.len() <= 2, "at most two preceding siblings, got {preceding:?}");
}

#[rstest]
#[case::descendant_search("//Edit")]
#[case::child_step_with_a_filter("/W/*[@id='missing']")]
fn a_search_for_a_missing_node_over_many_equal_siblings_finds_nothing(#[case] xpath: &'static str) {
    let result = within_deadline(xpath, move || {
        let labels: Vec<String> = (0..50).map(|i| format!("S{i}")).collect();
        let document = tree(&format!("W:[{}]", labels.join(",")));
        let shared: Vec<&str> = labels.iter().map(String::as_str).collect();
        eval_labels(xpath, &Keyless::sharing(&document, &shared))
    });
    assert!(result.is_empty(), "`{xpath}` finds nothing, got {result:?}");
}

/// Evaluates `xpath` over the wide tree through a view that sets the cancel flag once it has read
/// its fifth list of children, and returns the evaluation's error and how many lists it read.
fn cancelled_at_the_fifth_list(xpath: &'static str, first_only: bool) -> (Option<Error>, usize) {
    within_deadline(xpath, move || {
        let document = wide_tree();
        let flag = Arc::new(AtomicBool::new(false));
        let model = Keyless::cancelling(&document, &flag, 5);
        let ctx = DynamicContextBuilder::default()
            .with_context_item(XdmItem::Node(model.clone()))
            .with_cancel_flag(flag)
            .build();
        let error = if first_only {
            evaluate_first_expr::<Keyless>(xpath, &ctx).err()
        } else {
            evaluate_expr::<Keyless>(xpath, &ctx).err()
        };
        (error, model.lists_read())
    })
}

fn assert_cancelled(xpath: &str, error: Option<Error>) {
    let error = error.unwrap_or_else(|| panic!("`{xpath}` must stop with the cancellation error"));
    assert_eq!(error.code_enum(), ErrorCode::FOER0000, "`{xpath}`: {error}");
    assert!(error.to_string().contains("cancelled"), "`{xpath}`: the cancellation error, got {error}");
}

#[test]
fn count_over_a_wildcard_walk_stops() {
    let (error, lists) = cancelled_at_the_fifth_list("count(//*)", false);
    assert_cancelled("count(//*)", error);
    assert!(lists <= 6, "`count(//*)` read {lists} lists, at most one after the fifth");
}

#[test]
fn a_walk_whose_node_test_rejects_every_node_stops() {
    let (error, lists) = cancelled_at_the_fifth_list("//Edit", true);
    assert_cancelled("//Edit", error);
    assert!(lists <= 6, "`//Edit` read {lists} lists, at most one after the fifth");
}

#[test]
fn a_walk_over_equal_siblings_stops() {
    let error = within_deadline("count(//*)", || {
        let document = tree("W:[A,B,E]");
        let ctx = DynamicContextBuilder::default()
            .with_context_item(XdmItem::Node(Keyless::sharing(&document, &["A", "B"])))
            .with_cancel_flag(Arc::new(AtomicBool::new(true)))
            .build();
        evaluate_expr::<Keyless>("count(//*)", &ctx).err()
    });
    assert_cancelled("count(//*)", error);
}
