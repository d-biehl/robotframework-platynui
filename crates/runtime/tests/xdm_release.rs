//! Lifetime tests for the snapshots the runtime answers `XPath` queries from
//! (`OpenSpec` capability `xpath-snapshot`), against a lazy fake provider that
//! uses only the public API.
//!
//! The mock provider owns its whole tree, so it can show neither that a
//! snapshot is released nor that a held node loses its ancestors. The fake
//! creates fresh nodes on every `children()` call, as real providers do, and
//! counts the nodes it creates and drops and the lists of children it is asked
//! for.

use platynui_core::ui::{
    DESKTOP_RUNTIME_ID, Namespace, PatternName, RuntimeId, UiAttribute, UiNode, UiNodeExt, UiValue, attribute_names,
};
use platynui_runtime::{EvaluateOptions, EvaluationItem, EvaluationStream, XdmCache, evaluate};
use rstest::rstest;
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

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

/// How a listed node treats its parent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parents {
    /// Only a `Weak`, as every real provider did before this change.
    Weak,
    /// A strong reference, below the top level (the provider rule).
    Kept,
}

/// The roles of the children at each level below the root.
type Shape = Vec<Vec<&'static str>>;

/// Two windows; under each, a pane, a button and a pane; under each of those,
/// a button, a text and a button; and a button under each of those: 45 nodes.
fn shape() -> Shape {
    vec![vec!["Window", "Window"], vec!["Pane", "Button", "Pane"], vec!["Button", "Text", "Button"], vec!["Button"]]
}

/// Paths deeper than this are hashed, so that a deep chain's ids stay short.
const READABLE_DEPTH: usize = 12;

/// The shared state of one fake tree, with its counters.
struct Tree {
    shape: Shape,
    parents: Parents,
    created: AtomicUsize,
    dropped: AtomicUsize,
    listings: Mutex<HashMap<String, usize>>,
    generations: Mutex<Vec<u64>>,
    removed: Mutex<HashSet<String>>,
    added: Mutex<HashMap<String, Vec<(String, &'static str)>>>,
    panicking: AtomicBool,
    listing_panics: AtomicBool,
}

impl Tree {
    fn new(shape: Shape, parents: Parents) -> Arc<Self> {
        let depth = shape.len() + 1;
        Arc::new(Self {
            shape,
            parents,
            created: AtomicUsize::new(0),
            dropped: AtomicUsize::new(0),
            listings: Mutex::new(HashMap::new()),
            generations: Mutex::new(vec![0; depth]),
            removed: Mutex::new(HashSet::new()),
            added: Mutex::new(HashMap::new()),
            panicking: AtomicBool::new(false),
            listing_panics: AtomicBool::new(false),
        })
    }

    /// The root, which stands in for the runtime's desktop.
    fn root(self: &Arc<Self>) -> Arc<dyn UiNode> {
        FakeNode::create(self, DESKTOP_RUNTIME_ID.to_owned(), "Desktop", 0, None)
    }

    /// Provider nodes alive right now.
    fn live(&self) -> usize {
        self.created.load(Ordering::SeqCst) - self.dropped.load(Ordering::SeqCst)
    }

    fn created(&self) -> usize {
        self.created.load(Ordering::SeqCst)
    }

    /// Lists of children read so far.
    fn listings(&self) -> usize {
        self.listings.lock().unwrap().values().sum()
    }

    /// The nodes whose children were read more than once since the last call.
    fn take_listed_more_than_once(&self) -> Vec<String> {
        let mut listings = self.listings.lock().unwrap();
        let mut repeated: Vec<String> =
            listings.iter().filter(|(_, count)| **count > 1).map(|(id, _)| id.clone()).collect();
        repeated.sort();
        listings.clear();
        repeated
    }

    /// The nodes whose children were read since the last call, sorted.
    fn take_listed(&self) -> Vec<String> {
        let mut listings = self.listings.lock().unwrap();
        let mut listed: Vec<String> = listings.keys().cloned().collect();
        listed.sort();
        listings.clear();
        listed
    }

    /// The node is gone: it reports itself invalid, and no listing has it.
    fn remove(&self, id: &str) {
        self.removed.lock().unwrap().insert(id.to_owned());
    }

    /// Adds a child with `role` under `parent` and returns its id.
    fn add(&self, parent: &str, role: &'static str) -> String {
        let mut added = self.added.lock().unwrap();
        let children = added.entry(parent.to_owned()).or_default();
        let id = format!("{parent}/+{}", children.len());
        children.push((id.clone(), role));
        id
    }

    /// Every node created so far at `depth` reports itself invalid; nodes
    /// listed afterwards are valid.
    fn invalidate_level(&self, depth: usize) {
        self.generations.lock().unwrap()[depth] += 1;
    }

    /// From now on, `is_valid` panics below the root.
    fn set_panicking(&self, panicking: bool) {
        self.panicking.store(panicking, Ordering::SeqCst);
    }

    /// From now on, a list of children panics when its next child is read,
    /// which happens while the snapshot holds the lock of that list.
    fn set_listing_panics(&self, panics: bool) {
        self.listing_panics.store(panics, Ordering::SeqCst);
    }

    fn generation(&self, depth: usize) -> u64 {
        self.generations.lock().unwrap()[depth]
    }

    /// The children `parent` has right now.
    fn children_of(&self, parent: &str, depth: usize) -> Vec<(String, &'static str)> {
        *self.listings.lock().unwrap().entry(parent.to_owned()).or_default() += 1;
        let mut children: Vec<(String, &'static str)> = self
            .shape
            .get(depth)
            .map(|roles| {
                roles.iter().enumerate().map(|(index, role)| (child_id(parent, depth, index), *role)).collect()
            })
            .unwrap_or_default();
        if let Some(added) = self.added.lock().unwrap().get(parent) {
            children.extend(added.iter().cloned());
        }
        let removed = self.removed.lock().unwrap();
        children.retain(|(id, _)| !removed.contains(id));
        children
    }
}

fn child_id(parent: &str, depth: usize, index: usize) -> String {
    let parent = if depth == 0 { "root" } else { parent };
    if depth < READABLE_DEPTH {
        format!("{parent}/{index}")
    } else {
        let mut hasher = DefaultHasher::new();
        parent.hash(&mut hasher);
        index.hash(&mut hasher);
        format!("deep{depth}-{:016x}", hasher.finish())
    }
}

struct FakeNode {
    tree: Arc<Tree>,
    runtime_id: RuntimeId,
    role: &'static str,
    depth: usize,
    generation: u64,
    parent: Option<Weak<dyn UiNode>>,
    /// Held only to keep the parent alive (the provider rule).
    _kept_parent: Option<Arc<dyn UiNode>>,
    self_weak: OnceLock<Weak<dyn UiNode>>,
}

impl FakeNode {
    fn create(
        tree: &Arc<Tree>,
        id: String,
        role: &'static str,
        depth: usize,
        parent: Option<&Arc<dyn UiNode>>,
    ) -> Arc<dyn UiNode> {
        // Top-level nodes reach the desktop only weakly, as the rule requires.
        let keep = tree.parents == Parents::Kept && depth > 1;
        let node = Arc::new(Self {
            tree: Arc::clone(tree),
            runtime_id: RuntimeId::from(id),
            role,
            depth,
            generation: tree.generation(depth),
            parent: parent.map(Arc::downgrade),
            _kept_parent: parent.filter(|_| keep).cloned(),
            self_weak: OnceLock::new(),
        });
        tree.created.fetch_add(1, Ordering::SeqCst);
        let erased: Arc<dyn UiNode> = node.clone();
        let _ = node.self_weak.set(Arc::downgrade(&erased));
        erased
    }
}

impl Drop for FakeNode {
    fn drop(&mut self) {
        self.tree.dropped.fetch_add(1, Ordering::SeqCst);
    }
}

/// A provider's lazy list of children, which holds its parent while it runs.
struct Listing {
    tree: Arc<Tree>,
    parent: Arc<dyn UiNode>,
    depth: usize,
    entries: std::vec::IntoIter<(String, &'static str)>,
}

impl Iterator for Listing {
    type Item = Arc<dyn UiNode>;

    fn next(&mut self) -> Option<Self::Item> {
        assert!(!self.tree.listing_panics.load(Ordering::SeqCst), "a listing panics on purpose");
        let (id, role) = self.entries.next()?;
        Some(FakeNode::create(&self.tree, id, role, self.depth, Some(&self.parent)))
    }
}

struct Attribute {
    name: &'static str,
    value: UiValue,
}

impl UiAttribute for Attribute {
    fn namespace(&self) -> Namespace {
        Namespace::Control
    }
    fn name(&self) -> &str {
        self.name
    }
    fn value(&self) -> UiValue {
        self.value.clone()
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
        self.runtime_id.as_str().to_owned()
    }
    fn runtime_id(&self) -> &RuntimeId {
        &self.runtime_id
    }
    fn parent(&self) -> Option<Weak<dyn UiNode>> {
        self.parent.clone()
    }
    fn has_children(&self) -> bool {
        self.depth < self.tree.shape.len()
    }
    fn children(&self) -> Box<dyn Iterator<Item = Arc<dyn UiNode>> + Send + 'static> {
        let parent = self.self_weak.get().and_then(Weak::upgrade).expect("a node is alive while it lists");
        let entries = self.tree.children_of(self.runtime_id.as_str(), self.depth);
        Box::new(Listing { tree: Arc::clone(&self.tree), parent, depth: self.depth + 1, entries: entries.into_iter() })
    }
    fn attributes(&self) -> Box<dyn Iterator<Item = Arc<dyn UiAttribute>> + Send + 'static> {
        let attributes: Vec<Arc<dyn UiAttribute>> = vec![
            Arc::new(Attribute { name: attribute_names::common::ROLE, value: UiValue::from(self.role) }),
            Arc::new(Attribute { name: attribute_names::common::NAME, value: UiValue::from(self.name()) }),
            Arc::new(Attribute {
                name: attribute_names::common::RUNTIME_ID,
                value: UiValue::from(self.runtime_id.as_str().to_owned()),
            }),
        ];
        Box::new(attributes.into_iter())
    }
    fn supported_patterns(&self) -> Vec<PatternName> {
        Vec::new()
    }
    fn is_valid(&self) -> bool {
        assert!(!(self.depth > 0 && self.tree.panicking.load(Ordering::SeqCst)), "is_valid panics on purpose");
        !self.tree.removed.lock().unwrap().contains(self.runtime_id.as_str())
            && self.tree.generation(self.depth) == self.generation
    }
    fn invalidate(&self) {}
}

fn options(root: &Arc<dyn UiNode>) -> EvaluateOptions {
    EvaluateOptions::new(Arc::clone(root))
}

fn cached(root: &Arc<dyn UiNode>, cache: &XdmCache) -> EvaluateOptions {
    options(root).with_cache(cache.clone())
}

fn nodes(items: Vec<EvaluationItem>) -> Vec<Arc<dyn UiNode>> {
    items
        .into_iter()
        .map(|item| match item {
            EvaluationItem::Node(node) => node,
            other => panic!("expected a node, got {other:?}"),
        })
        .collect()
}

fn ids(items: Vec<EvaluationItem>) -> Vec<String> {
    nodes(items).iter().map(|node| node.runtime_id().as_str().to_owned()).collect()
}

fn integer(items: &[EvaluationItem]) -> i64 {
    match items {
        [EvaluationItem::Value(UiValue::Integer(value))] => *value,
        other => panic!("expected one integer, got {other:?}"),
    }
}

// --- A query reads one snapshot that does not change while it runs ---

#[rstest]
#[case::descendants("//Button")]
#[case::count("count(//*)")]
#[case::following_siblings("//Pane/following-sibling::*")]
#[case::last("(//Button)[last()]")]
#[case::preceding("//Button/preceding::Pane")]
#[case::union_of_attributes_and_elements("//Pane/@Name | //Button")]
fn each_list_is_read_once_per_query(#[case] query: &str) {
    let tree = Tree::new(shape(), Parents::Weak);
    let root = tree.root();

    let items = evaluate(None, query, options(&root)).expect("query");

    assert!(!items.is_empty(), "{query} should have a result");
    assert_eq!(tree.take_listed_more_than_once(), Vec::<String>::new(), "lists read more than once by {query}");
}

// --- The first result does not require reading the rest of the tree (xpath-evaluation) ---

#[rstest]
fn the_first_match_reads_only_the_lists_up_to_it() {
    let tree = Tree::new(shape(), Parents::Weak);
    let root = tree.root();

    // What `Runtime::evaluate_single` does: the first item of a stream.
    let first = EvaluationStream::new(None, "//Button".to_owned(), options(&root)).expect("query").next();
    let Some(Ok(EvaluationItem::Node(button))) = first else { panic!("expected a button, got {first:?}") };

    assert_eq!(button.runtime_id().as_str(), "root/0/0/0", "the first button in document order");
    let mut expected = vec![DESKTOP_RUNTIME_ID.to_owned(), "root/0".to_owned(), "root/0/0".to_owned()];
    expected.sort();
    assert_eq!(tree.take_listed(), expected, "only the lists of the button's ancestors");
}

#[rstest]
fn the_second_match_overall_is_the_child_of_the_first() {
    let tree = Tree::new(shape(), Parents::Weak);
    let root = tree.root();

    assert_eq!(ids(evaluate(None, "(//Button)[2]", options(&root)).expect("query")), ["root/0/0/0/0"]);
}

// --- The next query may reuse the snapshot ---

#[rstest]
fn repeating_a_query_on_a_retained_snapshot_reads_nothing_new() {
    let tree = Tree::new(shape(), Parents::Weak);
    let root = tree.root();
    let cache = XdmCache::new();

    let first = ids(evaluate(None, "//Button", cached(&root, &cache)).expect("query"));
    let (listings, created) = (tree.listings(), tree.created());
    let second = ids(evaluate(None, "//Button", cached(&root, &cache)).expect("query"));

    assert_eq!(second, first);
    assert_eq!(tree.listings(), listings, "no list of children may be read again");
    assert_eq!(tree.created(), created, "no node may be created again");
}

#[rstest]
fn a_node_that_is_no_longer_valid_is_not_returned() {
    let tree = Tree::new(shape(), Parents::Weak);
    let root = tree.root();
    let cache = XdmCache::new();

    let first = ids(evaluate(None, "//Button", cached(&root, &cache)).expect("query"));
    assert!(first.contains(&"root/0/1".to_owned()));
    let listings = tree.listings();

    tree.remove("root/0/1");
    let second = ids(evaluate(None, "//Button", cached(&root, &cache)).expect("query"));

    assert!(!second.contains(&"root/0/1".to_owned()), "a gone button was returned: {second:?}");
    assert!(tree.listings() > listings, "the list holding the gone button must be read again");
}

#[rstest]
fn a_sibling_that_has_gone_is_not_returned_from_a_held_element() {
    let tree = Tree::new(shape(), Parents::Kept);
    let root = tree.root();
    let pane = nodes(evaluate(None, "(//Pane)[1]", options(&root)).expect("query")).remove(0);
    assert_eq!(pane.runtime_id().as_str(), "root/0/0");
    let cache = XdmCache::new();

    let first = ids(evaluate(Some(Arc::clone(&pane)), "following-sibling::*", cached(&root, &cache)).expect("query"));
    assert_eq!(first, ["root/0/1", "root/0/2"]);

    tree.remove("root/0/2");
    let second = ids(evaluate(Some(Arc::clone(&pane)), "following-sibling::*", cached(&root, &cache)).expect("query"));

    assert_eq!(second, ["root/0/1"]);
}

#[rstest]
fn an_added_node_appears_only_after_the_snapshot_was_discarded() {
    let tree = Tree::new(shape(), Parents::Weak);
    let root = tree.root();
    let cache = XdmCache::new();
    ids(evaluate(None, "//Button", cached(&root, &cache)).expect("query"));

    let added = tree.add("root/1", "Button");
    let before_discard = ids(evaluate(None, "//Button", cached(&root, &cache)).expect("query"));
    cache.clear();
    let after_discard = ids(evaluate(None, "//Button", cached(&root, &cache)).expect("query"));

    assert!(!before_discard.contains(&added), "the retained snapshot must not see the added node");
    assert!(after_discard.contains(&added), "after the discard the query must read the current UI");
}

// --- A snapshot that ends is released ---

#[rstest]
fn a_discarded_snapshot_is_released(#[values(Parents::Weak, Parents::Kept)] parents: Parents) {
    let tree = Tree::new(shape(), parents);
    let root = tree.root();
    let before = tree.live();
    let cache = XdmCache::new();

    drop(evaluate(None, "//Button", cached(&root, &cache)).expect("query"));
    assert!(tree.live() > before, "the retained snapshot holds its nodes");
    cache.clear();

    assert_eq!(tree.live(), before);
}

#[rstest]
fn a_query_without_a_retained_snapshot_leaves_nothing_behind(
    #[values("count(//*)", "/*", "//Button")] query: &str,
    #[values(Parents::Weak, Parents::Kept)] parents: Parents,
) {
    let tree = Tree::new(shape(), parents);
    let root = tree.root();
    let before = tree.live();

    drop(evaluate(None, query, options(&root)).expect("query"));

    assert_eq!(tree.live(), before, "{query} left nodes behind");
}

#[rstest]
fn a_single_result_leaves_nothing_behind_once_dropped(#[values(Parents::Weak, Parents::Kept)] parents: Parents) {
    let tree = Tree::new(shape(), parents);
    let root = tree.root();
    let before = tree.live();

    // What `Runtime::evaluate_single` does: the first item of a stream.
    let first = EvaluationStream::new(None, "//Button".to_owned(), options(&root)).expect("query").next();
    assert!(matches!(first, Some(Ok(EvaluationItem::Node(_)))));
    drop(first);

    assert_eq!(tree.live(), before);
}

#[rstest]
fn a_snapshot_replaced_by_another_context_is_released(#[values(Parents::Weak, Parents::Kept)] parents: Parents) {
    let tree = Tree::new(shape(), parents);
    let root = tree.root();
    let windows = nodes(evaluate(None, "/*", options(&root)).expect("query"));
    let before = tree.live();
    let cache = XdmCache::new();

    drop(evaluate(Some(Arc::clone(&windows[0])), ".//*", cached(&root, &cache)).expect("query"));
    let one_window = tree.live() - before;
    assert!(one_window > 0, "the retained snapshot holds the first window's nodes");
    drop(evaluate(Some(Arc::clone(&windows[1])), ".//*", cached(&root, &cache)).expect("query"));

    assert_eq!(tree.live() - before, one_window, "only the second window's snapshot may remain");
    cache.clear();
    assert_eq!(tree.live(), before);
}

#[rstest]
fn a_stream_dropped_early_is_released(#[values(Parents::Weak, Parents::Kept)] parents: Parents) {
    let tree = Tree::new(shape(), parents);
    let root = tree.root();
    let before = tree.live();

    let mut stream = EvaluationStream::new(None, "//*".to_owned(), options(&root)).expect("query");
    let first = stream.next();
    let second = stream.next();
    assert!(matches!((&first, &second), (Some(Ok(_)), Some(Ok(_)))));
    drop((first, second, stream));

    assert_eq!(tree.live(), before);
}

#[rstest]
#[case::does_not_compile_without_a_snapshot("//[", false)]
#[case::does_not_compile_with_a_snapshot("//[", true)]
#[case::fails_while_it_runs_without_a_snapshot("for $b in //Button return fn:error()", false)]
#[case::fails_while_it_runs_with_a_snapshot("for $b in //Button return fn:error()", true)]
fn a_query_that_fails_is_released(
    #[case] query: &str,
    #[case] retained: bool,
    #[values(Parents::Weak, Parents::Kept)] parents: Parents,
) {
    let tree = Tree::new(shape(), parents);
    let root = tree.root();
    let window = nodes(evaluate(None, "/*[1]", options(&root)).expect("query")).remove(0);
    let before = tree.live();
    let cache = XdmCache::new();
    let query_options = if retained {
        // A snapshot of another context, which the failing query replaces.
        drop(evaluate(None, "//*", cached(&root, &cache)).expect("query"));
        cached(&root, &cache)
    } else {
        options(&root)
    };

    evaluate(Some(Arc::clone(&window)), query, query_options).expect_err("the query fails");

    cache.clear();
    assert_eq!(tree.live(), before);
}

#[rstest]
fn revalidation_does_not_grow_a_retained_snapshot(#[values(Parents::Weak, Parents::Kept)] parents: Parents) {
    let tree = Tree::new(shape(), parents);
    let root = tree.root();
    let before = tree.live();
    let cache = XdmCache::new();
    let expected = ids(evaluate(None, "//Button", cached(&root, &cache)).expect("query"));

    let mut live = Vec::new();
    for _ in 0..3 {
        // The panes and buttons below the windows go stale.
        tree.invalidate_level(2);
        let result = ids(evaluate(None, "//Button", cached(&root, &cache)).expect("query"));
        assert_eq!(result, expected);
        live.push(tree.live());
    }

    assert!(live.windows(2).all(|pair| pair[0] == pair[1]), "the snapshot grew: {live:?}");
    cache.clear();
    assert_eq!(tree.live(), before);
}

#[rstest]
fn a_deep_snapshot_is_released_without_overflowing_the_stack() {
    const DEPTH: usize = 10_000;
    let tree = Tree::new(vec![vec!["Pane"]; DEPTH], Parents::Kept);
    let root = tree.root();
    let cache = XdmCache::new();

    // Evaluation itself recurses per level, so the snapshot is built on a large stack.
    let count = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn({
            let (root, cache) = (Arc::clone(&root), cache.clone());
            move || integer(&evaluate(None, "count(//*)", cached(&root, &cache)).expect("query"))
        })
        .expect("thread")
        .join()
        .expect("the snapshot is built");
    assert_eq!(count, i64::try_from(DEPTH).unwrap());
    assert_eq!(tree.live(), DEPTH + 1);

    std::thread::Builder::new()
        .stack_size(256 << 10)
        .spawn(move || cache.clear())
        .expect("thread")
        .join()
        .expect("the snapshot is released on a small stack");

    assert_eq!(tree.live(), 1, "only the root may remain");
}

/// A query from a held deep element builds that element's ancestors upward,
/// and the snapshot then owns them. Released from a small stack, neither those
/// wrappers nor the provider's chain of parents may recurse per level.
#[rstest]
fn a_deep_snapshot_of_a_held_element_is_released_without_overflowing_the_stack() {
    const DEPTH: usize = 10_000;
    let tree = Tree::new(vec![vec!["Pane"]; DEPTH], Parents::Kept);
    let root = tree.root();
    let cache = XdmCache::new();
    let mut deepest = Arc::clone(&root);
    while let Some(child) = deepest.children().next() {
        deepest = child;
    }

    let count = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn({
            let (root, cache) = (Arc::clone(&root), cache.clone());
            move || integer(&evaluate(Some(deepest), "count(ancestor::*)", cached(&root, &cache)).expect("query"))
        })
        .expect("thread")
        .join()
        .expect("the snapshot is built");
    assert_eq!(count, i64::try_from(DEPTH - 1).unwrap());
    assert_eq!(tree.live(), DEPTH + 1, "only the snapshot holds the deepest element and its chain");

    std::thread::Builder::new()
        .stack_size(256 << 10)
        .spawn(move || cache.clear())
        .expect("thread")
        .join()
        .expect("the snapshot is released on a small stack");

    assert_eq!(tree.live(), 1, "only the root may remain");
}

#[rstest]
fn a_provider_that_panics_does_not_abort_the_process() {
    let tree = Tree::new(shape(), Parents::Kept);
    let root = tree.root();
    let before = tree.live();
    let cache = XdmCache::new();
    let mut stream = EvaluationStream::new(None, "//*".to_owned(), cached(&root, &cache)).expect("query");
    let partly = (stream.next(), stream.next());
    assert_eq!(integer(&evaluate(None, "count(/*)", cached(&root, &cache)).expect("query")), 2);

    tree.set_panicking(true);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = evaluate(None, "//Button", cached(&root, &cache));
    }));
    tree.set_panicking(false);

    assert!(outcome.is_err(), "the query fails with the provider's panic");
    drop((partly, stream));
    cache.clear();
    assert_eq!(tree.live(), before);
}

/// A provider that panics while one of its lists is read poisons the lock of
/// that list. The query fails with the panic; the snapshot still answers the
/// next query, and it is still released.
#[rstest]
fn a_listing_that_panics_does_not_abort_the_process(#[values(false, true)] retained: bool) {
    let tree = Tree::new(shape(), Parents::Kept);
    let root = tree.root();
    let before = tree.live();
    let cache = XdmCache::new();
    let query_options = || if retained { cached(&root, &cache) } else { options(&root) };
    if retained {
        assert_eq!(integer(&evaluate(None, "count(/*)", query_options()).expect("query")), 2);
    }

    tree.set_listing_panics(true);
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = evaluate(None, "//Button", query_options());
    }));
    tree.set_listing_panics(false);

    assert!(outcome.is_err(), "the query fails with the provider's panic");
    assert_eq!(integer(&evaluate(None, "count(//*)", query_options()).expect("query")), 44);
    cache.clear();
    assert_eq!(tree.live(), before);
}

/// Two threads that query one retained snapshot see every node, and leave no
/// provider iterator behind.
#[rstest]
fn concurrent_queries_on_one_snapshot_see_every_node() {
    for _ in 0..50 {
        let tree = Tree::new(shape(), Parents::Kept);
        let root = tree.root();
        let before = tree.live();
        let cache = XdmCache::new();
        assert_eq!(integer(&evaluate(None, "count(.)", cached(&root, &cache)).expect("query")), 1);
        let barrier = std::sync::Barrier::new(2);
        let counts: Vec<i64> = std::thread::scope(|scope| {
            let workers: Vec<_> = (0..2)
                .map(|_| {
                    scope.spawn(|| {
                        barrier.wait();
                        integer(&evaluate(None, "count(//*)", cached(&root, &cache)).expect("query"))
                    })
                })
                .collect();
            workers.into_iter().map(|worker| worker.join().expect("worker")).collect()
        });
        assert_eq!(counts, [44, 44]);
        cache.clear();
        assert_eq!(tree.live(), before, "no node nor provider iterator may stay behind");
    }
}

// --- A node that is handed out keeps its ancestors while it is held ---

#[rstest]
fn a_held_result_keeps_its_ancestors() {
    let tree = Tree::new(shape(), Parents::Kept);
    let root = tree.root();
    let before = tree.live();
    let cache = XdmCache::new();

    let button = nodes(evaluate(None, "/Window[1]/Pane[1]/Button[1]", cached(&root, &cache)).expect("query")).remove(0);
    assert_eq!(button.runtime_id().as_str(), "root/0/0/0");
    cache.clear();

    // `ancestors()` also yields the desktop below the top level, a known quirk
    // outside this test; what matters is that the chain reaches the window.
    let ancestors: Vec<String> = button.ancestors().map(|node| node.runtime_id().as_str().to_owned()).collect();
    assert_eq!(ancestors[..2], ["root/0/0", "root/0"], "the pane and the window");
    let in_query = integer(&evaluate(Some(Arc::clone(&button)), "count(ancestor::*)", options(&root)).expect("query"));
    assert_eq!(in_query, 2, "the pane and the window, below the desktop");
    assert_eq!(tree.live() - before, 3, "only the button and its chain may remain");

    drop(button);
    assert_eq!(tree.live(), before);
}
