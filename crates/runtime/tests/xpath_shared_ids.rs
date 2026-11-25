//! Queries over a provider whose siblings share one runtime id (spec `xpath-evaluation`, "Walks
//! end when siblings share an identity" and "Cancellation stops every walk"), against a lazy fake
//! provider that uses only the public API.
//!
//! The runtime tells two nodes apart by their runtime ids, so siblings with one id are one node to
//! it, as the tab headers of a `WinForms` `TabControl` were before the UIA provider derived ids for
//! them. The fake counts the lists of children it is asked for. In the specs' notation the tree is
//! `W:[A,B,E]`: `W` is a `Window`, `A` and `B` are `Pane`s that share one id, and `E` is the
//! `Button` after them.

use platynui_core::ui::{Namespace, PatternName, RuntimeId, UiAttribute, UiNode, UiValue};
use platynui_runtime::{EvaluateOptions, EvaluationItem, EvaluationStream};
use rstest::rstest;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak, mpsc};
use std::time::Duration;

// Crate dependencies of the library that this integration-test target does
// not use directly (`unused_crate_dependencies` is target-scoped).
use inventory as _;
use pest as _;
use pest_derive as _;
use platynui_platform_mock as _;
use platynui_provider_mock as _;
use platynui_xpath as _;
use rust_decimal as _;
#[cfg(unix)]
use rustix as _;
use serde_json as _;
use serial_test as _;
use thiserror as _;
use tracing as _;
use tracing_subscriber as _;

/// How long a query may take before it counts as one that never ends. The trees are tiny, so this
/// only catches a loop.
const DEADLINE: Duration = Duration::from_secs(5);

/// What one fake tree counts, and when it cancels.
#[derive(Default)]
struct Counters {
    /// The lists of children the provider was asked for, in all.
    lists: AtomicUsize,
    /// A cancel flag to set once this many lists have been read.
    cancel: Option<(Arc<AtomicBool>, usize)>,
}

/// A node of the fake tree, which counts how often its list of children was read.
struct FakeNode {
    role: &'static str,
    runtime_id: RuntimeId,
    counters: Arc<Counters>,
    parent: Mutex<Option<Weak<dyn UiNode>>>,
    children: Mutex<Vec<Arc<FakeNode>>>,
    reads: AtomicUsize,
}

impl FakeNode {
    fn new(role: &'static str, runtime_id: &str, counters: &Arc<Counters>, children: Vec<Arc<FakeNode>>) -> Arc<Self> {
        let node = Arc::new(Self {
            role,
            runtime_id: RuntimeId::from(runtime_id),
            counters: Arc::clone(counters),
            parent: Mutex::new(None),
            children: Mutex::new(Vec::new()),
            reads: AtomicUsize::new(0),
        });
        for child in &children {
            *child.parent.lock().unwrap() = Some(Arc::downgrade(&(Arc::clone(&node) as Arc<dyn UiNode>)));
        }
        *node.children.lock().unwrap() = children;
        node
    }

    /// Every node whose list of children was read more than once, by role and runtime id.
    fn lists_read_twice(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.reads.load(Ordering::SeqCst) > 1 {
            out.push(format!("{} {}", self.role, self.runtime_id));
        }
        for child in self.children.lock().unwrap().iter() {
            out.extend(child.lists_read_twice());
        }
        out
    }
}

impl UiNode for FakeNode {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn role(&self) -> &str {
        self.role
    }
    fn name(&self) -> String {
        String::new()
    }
    fn runtime_id(&self) -> &RuntimeId {
        &self.runtime_id
    }
    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        self.parent.lock().unwrap().clone()
    }
    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        let read = self.counters.lists.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some((flag, after)) = &self.counters.cancel
            && read >= *after
        {
            flag.store(true, Ordering::SeqCst);
        }
        let children: Vec<Arc<dyn UiNode>> =
            self.children.lock().unwrap().iter().map(|child| Arc::clone(child) as Arc<dyn UiNode>).collect();
        Box::new(children.into_iter())
    }
    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        Box::new(std::iter::empty())
    }
    fn supported_patterns(&self) -> Vec<PatternName> {
        Vec::new()
    }
    fn invalidate(&self) {}
}

/// The tree `W:[A,B,E]` below the desktop, in which `A` and `B` share one runtime id.
fn shared_tree(counters: &Arc<Counters>) -> Arc<FakeNode> {
    let a = FakeNode::new("Pane", "shared", counters, vec![]);
    let b = FakeNode::new("Pane", "shared", counters, vec![]);
    let e = FakeNode::new("Button", "e", counters, vec![]);
    let window = FakeNode::new("Window", "w", counters, vec![a, b, e]);
    FakeNode::new("Desktop", "desktop", counters, vec![window])
}

/// A window with 50 children that all share one runtime id.
fn fifty_shared_tree(counters: &Arc<Counters>) -> Arc<FakeNode> {
    let children = (0..50).map(|_| FakeNode::new("Pane", "shared", counters, vec![])).collect();
    FakeNode::new("Desktop", "desktop", counters, vec![FakeNode::new("Window", "w", counters, children)])
}

/// A window with 20 panes of 10 buttons each, all ids distinct, and no `Edit`.
fn wide_tree(counters: &Arc<Counters>) -> Arc<FakeNode> {
    let panes = (0..20)
        .map(|p| {
            let buttons = (0..10).map(|b| FakeNode::new("Button", &format!("b{p}_{b}"), counters, vec![])).collect();
            FakeNode::new("Pane", &format!("p{p}"), counters, buttons)
        })
        .collect();
    FakeNode::new("Desktop", "desktop", counters, vec![FakeNode::new("Window", "w", counters, panes)])
}

/// What a query returned: the roles of its nodes or its atomic values as text, or its error.
#[derive(Debug)]
struct Outcome {
    items: Vec<String>,
    error: Option<String>,
}

/// Builds a tree for the given counters.
type Tree = fn(&Arc<Counters>) -> Arc<FakeNode>;

/// Runs `xpath` over the tree `tree` builds, on its own thread and taking at most `limit` items,
/// and fails the test once [`DEADLINE`] has passed, so that a query that never ends fails instead
/// of hanging the run. Returns the outcome and the lists of children read more than once.
fn run(tree: Tree, counters: Arc<Counters>, xpath: &'static str, limit: usize) -> (Outcome, Vec<String>) {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let root = tree(&counters);
        let desktop: Arc<dyn UiNode> = Arc::clone(&root) as Arc<dyn UiNode>;
        let mut options = EvaluateOptions::new(desktop);
        if let Some((flag, _)) = &counters.cancel {
            options = options.with_cancel_flag(Arc::clone(flag));
        }
        let mut outcome = Outcome { items: Vec::new(), error: None };
        match EvaluationStream::new(None, xpath.to_string(), options) {
            Err(error) => outcome.error = Some(error.to_string()),
            Ok(stream) => {
                for item in stream.take(limit) {
                    match item {
                        Ok(EvaluationItem::Node(node)) => outcome.items.push(node.role().to_string()),
                        Ok(EvaluationItem::Value(UiValue::Integer(value))) => outcome.items.push(value.to_string()),
                        Ok(EvaluationItem::Value(value)) => outcome.items.push(format!("{value:?}")),
                        Ok(other) => outcome.items.push(format!("{other:?}")),
                        Err(error) => {
                            outcome.error = Some(error.to_string());
                            break;
                        }
                    }
                }
            }
        }
        let _ = sender.send((outcome, root.lists_read_twice()));
    });
    receiver.recv_timeout(DEADLINE).unwrap_or_else(|_| panic!("`{xpath}` did not end within {DEADLINE:?}"))
}

/// Runs `xpath` in full over the tree `W:[A,B,E]` with shared ids.
fn over_shared(xpath: &'static str) -> Vec<String> {
    let (outcome, _) = run(shared_tree, Arc::default(), xpath, usize::MAX);
    assert_eq!(outcome.error, None, "`{xpath}` must not fail");
    outcome.items
}

#[test]
fn a_descendant_search_past_equal_siblings_finds_the_node_after_them() {
    let (outcome, _) = run(shared_tree, Arc::default(), "//Button", 1);
    assert_eq!(outcome.items, ["Button"], "the first result of `//Button`");
    let count: usize = over_shared("count(//*)")[0].parse().expect("an integer");
    assert!(count <= 4, "`count(//*)` counts no more nodes than the tree has, got {count}");
}

#[test]
fn a_child_step_past_equal_siblings_ends() {
    let children = over_shared("/Window/*");
    assert!(children.contains(&"Button".to_string()), "`/Window/*` reaches the button, got {children:?}");
    assert!(children.len() <= 3, "`/Window/*` holds at most the three children, got {children:?}");
    assert_eq!(over_shared("/Window/Button"), ["Button"]);
    assert_eq!(over_shared("/Window/*[3]"), ["Button"], "positions count over the listed children");
}

#[test]
fn sibling_axes_over_equal_siblings_end() {
    let following = over_shared("/Window/*[1]/following-sibling::*");
    assert!(following.contains(&"Button".to_string()), "the following siblings reach the button, got {following:?}");
    assert!(following.len() <= 2, "at most two following siblings, got {following:?}");
    let preceding = over_shared("/Window/Button/preceding-sibling::*");
    assert!(preceding.len() <= 2, "at most two preceding siblings, got {preceding:?}");
}

#[rstest]
#[case::descendant_search("//Edit")]
#[case::child_step_with_a_filter("/Window/*[@Name='missing']")]
fn a_search_for_a_missing_node_over_many_equal_siblings_finds_nothing(#[case] xpath: &'static str) {
    let (outcome, read_twice) = run(fifty_shared_tree, Arc::default(), xpath, usize::MAX);
    assert_eq!(outcome.error, None, "`{xpath}` must not fail");
    assert!(outcome.items.is_empty(), "`{xpath}` finds nothing, got {:?}", outcome.items);
    assert!(read_twice.is_empty(), "`{xpath}` read lists twice: {read_twice:?}");
}

/// Counters that set a fresh cancel flag when the fifth list of children is read.
fn cancelling_at_the_fifth_list() -> Arc<Counters> {
    Arc::new(Counters { lists: AtomicUsize::new(0), cancel: Some((Arc::new(AtomicBool::new(false)), 5)) })
}

fn assert_cancelled(xpath: &str, outcome: &Outcome) {
    let error = outcome.error.as_deref().unwrap_or_else(|| panic!("`{xpath}` must stop with the cancellation error"));
    assert!(error.contains("cancelled"), "`{xpath}`: the cancellation error, got {error}");
}

#[rstest]
#[case::count_over_a_wildcard_walk("count(//*)", usize::MAX)]
#[case::a_walk_whose_node_test_rejects_every_node("//Edit", 1)]
fn cancellation_stops_the_walk_after_at_most_one_further_list(#[case] xpath: &'static str, #[case] limit: usize) {
    let counters = cancelling_at_the_fifth_list();
    let (outcome, _) = run(wide_tree, Arc::clone(&counters), xpath, limit);
    assert_cancelled(xpath, &outcome);
    let lists = counters.lists.load(Ordering::SeqCst);
    assert!(lists <= 6, "`{xpath}` read {lists} lists, at most one after the fifth");
}

#[test]
fn a_walk_over_equal_siblings_stops() {
    let counters =
        Arc::new(Counters { lists: AtomicUsize::new(0), cancel: Some((Arc::new(AtomicBool::new(true)), 0)) });
    let (outcome, _) = run(shared_tree, counters, "count(//*)", usize::MAX);
    assert_cancelled("count(//*)", &outcome);
}
