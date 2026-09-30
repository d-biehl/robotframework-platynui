//! Node identity and document order for the evaluator: the set that removes duplicate nodes, and
//! the total order that sorts nodes into document order.
//!
//! The order never compares two nodes by asking the model. It gives each node a [`Position`]: the
//! rank of its root, then one [`Slot`] for every step down from that root. Positions compare as
//! plain values, so the order is total, and sorting cannot fail or panic on a model whose answers
//! do not fit together, such as a node missing from its parent's list of children. Comparing two
//! nodes gives the same answer from the slots below their deepest shared ancestor alone, and reads
//! each list of children only as far as it has to.

use std::collections::HashMap;

use smallvec::SmallVec;

use crate::model::{NodeKind, XdmNode};

/// A map from nodes, by identity, to values.
///
/// A node with a document-order key is found by its key. A node with an identity hint is found in
/// the bucket of its hint, compared with `==` there. Any other node is found by a linear scan, the
/// fallback for models that offer neither.
#[derive(Clone)]
pub(crate) struct NodeMap<N, V> {
    keyed: HashMap<u64, V>,
    hinted: HashMap<u64, SmallVec<[(N, V); 1]>>,
    others: Vec<(N, V)>,
}

impl<N, V> Default for NodeMap<N, V> {
    fn default() -> Self {
        Self { keyed: HashMap::new(), hinted: HashMap::new(), others: Vec::new() }
    }
}

impl<N: XdmNode, V> NodeMap<N, V> {
    pub(crate) fn get(&self, node: &N) -> Option<&V> {
        if let Some(key) = node.doc_order_key() {
            return self.keyed.get(&key);
        }
        if let Some(hint) = node.identity_hint() {
            return self.hinted.get(&hint)?.iter().find(|(seen, _)| seen == node).map(|(_, value)| value);
        }
        self.others.iter().find(|(seen, _)| seen == node).map(|(_, value)| value)
    }

    pub(crate) fn get_mut(&mut self, node: &N) -> Option<&mut V> {
        if let Some(key) = node.doc_order_key() {
            return self.keyed.get_mut(&key);
        }
        if let Some(hint) = node.identity_hint() {
            return self.hinted.get_mut(&hint)?.iter_mut().find(|(seen, _)| seen == node).map(|(_, value)| value);
        }
        self.others.iter_mut().find(|(seen, _)| seen == node).map(|(_, value)| value)
    }

    /// Stores `value` for `node` unless the node has one already; returns whether it was stored.
    pub(crate) fn insert(&mut self, node: &N, value: V) -> bool {
        if let Some(key) = node.doc_order_key() {
            if self.keyed.contains_key(&key) {
                return false;
            }
            self.keyed.insert(key, value);
            return true;
        }
        if let Some(hint) = node.identity_hint() {
            let bucket = self.hinted.entry(hint).or_default();
            if bucket.iter().any(|(seen, _)| seen == node) {
                return false;
            }
            bucket.push((node.clone(), value));
            return true;
        }
        if self.others.iter().any(|(seen, _)| seen == node) {
            return false;
        }
        self.others.push((node.clone(), value));
        true
    }
}

/// A set of nodes by identity, for removing duplicates in one pass.
#[derive(Clone)]
pub(crate) struct NodeSet<N>(NodeMap<N, ()>);

impl<N> Default for NodeSet<N> {
    fn default() -> Self {
        Self(NodeMap::default())
    }
}

impl<N: XdmNode> NodeSet<N> {
    /// Adds `node` and returns whether it was not in the set yet.
    pub(crate) fn insert(&mut self, node: &N) -> bool {
        self.0.insert(node, ())
    }

    /// Whether `node` is in the set.
    pub(crate) fn contains(&self, node: &N) -> bool {
        self.0.get(node).is_some()
    }
}

/// The nodes of `nodes`, each once, in the order in which they first appear.
pub(crate) fn distinct_nodes<N: XdmNode>(nodes: Vec<N>) -> Vec<N> {
    let mut seen = NodeSet::default();
    nodes.into_iter().filter(|node| seen.insert(node)).collect()
}

/// Sorts `nodes` into document order, a total order.
///
/// When every node has a document-order key, the keys decide. Otherwise every node gets a
/// [`Position`], built from its parent's list of children, which is read once per parent and never
/// from an element's attributes. Equal nodes keep their relative order.
pub(crate) fn sort_nodes<N: XdmNode>(nodes: &mut Vec<N>) {
    if nodes.len() < 2 {
        return;
    }
    if nodes.iter().all(|node| node.doc_order_key().is_some()) {
        nodes.sort_by_key(XdmNode::doc_order_key);
        return;
    }
    let mut order = DocOrder::default();
    let mut positioned: Vec<(Position, N)> = nodes.drain(..).map(|node| (order.position(&node), node)).collect();
    positioned.sort_by(|a, b| a.0.cmp(&b.0));
    nodes.extend(positioned.into_iter().map(|(_, node)| node));
}

/// One step down from a root to a node, in document order among its siblings.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Slot {
    /// An attribute, among its owner's attributes by namespace URI and local name. Attributes come
    /// right after their owner, before its namespaces and children, as the keys of the models put
    /// them; XDM's own order (namespaces first) is not kept.
    Attribute(String, String),
    /// A namespace node, among its owner's namespace nodes by prefix.
    Namespace(String),
    /// A node at this index of its parent's list of children.
    Child(usize),
    /// A node missing from its parent's list of children: after every node on the list, in the
    /// order in which such nodes were first placed.
    Missing(usize),
}

/// Where a node sits in document order: the rank of its root among the roots seen so far, then its
/// slots from that root down. Nodes of different roots keep the order in which their roots first
/// appeared, which `XPath` leaves to the implementation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Position {
    root: usize,
    path: SmallVec<[Slot; 8]>,
}

/// Gives nodes their positions in document order, with the lists of children it needs indexed as
/// far as they were read.
pub(crate) struct DocOrder<N> {
    roots: Vec<N>,
    lists: NodeMap<N, ChildIndex<N>>,
    missing: NodeMap<N, usize>,
    next_missing: usize,
}

impl<N> Default for DocOrder<N> {
    fn default() -> Self {
        Self { roots: Vec::new(), lists: NodeMap::default(), missing: NodeMap::default(), next_missing: 0 }
    }
}

/// A parent's list of children, indexed for lookup as far as it was read.
struct ChildIndex<N> {
    by_node: NodeMap<N, usize>,
    read: usize,
    complete: bool,
}

/// How far a lookup reads a list of children: up to the node it looks for, or to the end.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Reading {
    UpToTheNode,
    WholeList,
}

/// `node` and its ancestors, from its root down to the node.
fn ancestry<N: XdmNode>(node: &N) -> SmallVec<[N; 16]> {
    let mut chain: SmallVec<[N; 16]> = SmallVec::new();
    chain.push(node.clone());
    while let Some(parent) = chain.last().and_then(XdmNode::parent) {
        chain.push(parent);
    }
    chain.reverse();
    chain
}

impl<N: XdmNode> DocOrder<N> {
    /// Whether `a` comes before `b` in document order: their positions compared, but only from
    /// their deepest shared ancestor down, so no list of children above it is read.
    pub(crate) fn precedes(&mut self, a: &N, b: &N) -> bool {
        if let (Some(a), Some(b)) = (a.doc_order_key(), b.doc_order_key()) {
            return a < b;
        }
        let (chain_a, chain_b) = (ancestry(a), ancestry(b));
        let (root_a, root_b) = (self.root_rank(&chain_a[0]), self.root_rank(&chain_b[0]));
        if root_a != root_b {
            return root_a < root_b;
        }
        let shared = chain_a.iter().zip(&chain_b).take_while(|(x, y)| x == y).count();
        for depth in shared..chain_a.len().min(chain_b.len()) {
            let slot_a = self.slot(&chain_a[depth - 1], &chain_a[depth], Reading::UpToTheNode);
            let slot_b = self.slot(&chain_b[depth - 1], &chain_b[depth], Reading::UpToTheNode);
            if slot_a != slot_b {
                return slot_a < slot_b;
            }
        }
        chain_a.len() < chain_b.len()
    }

    /// The position of `node`. A sort places most of a list, so this reads every list it needs
    /// to its end, once.
    pub(crate) fn position(&mut self, node: &N) -> Position {
        let chain = ancestry(node);
        let root = self.root_rank(&chain[0]);
        let path = chain.windows(2).map(|pair| self.slot(&pair[0], &pair[1], Reading::WholeList)).collect();
        Position { root, path }
    }

    fn root_rank(&mut self, root: &N) -> usize {
        if let Some(rank) = self.roots.iter().position(|seen| seen == root) {
            return rank;
        }
        self.roots.push(root.clone());
        self.roots.len() - 1
    }

    fn slot(&mut self, parent: &N, child: &N, reading: Reading) -> Slot {
        match child.kind() {
            NodeKind::Attribute => {
                let name = child.name().unwrap_or_else(|| crate::model::QName {
                    prefix: None,
                    local: String::new(),
                    ns_uri: None,
                });
                Slot::Attribute(name.ns_uri.unwrap_or_default(), name.local)
            }
            NodeKind::Namespace => Slot::Namespace(child.name().and_then(|name| name.prefix).unwrap_or_default()),
            _ => {
                if let Some(index) = self.child_index(parent, child, reading) {
                    return Slot::Child(index);
                }
                if let Some(arrival) = self.missing.get(child) {
                    return Slot::Missing(*arrival);
                }
                let arrival = self.next_missing;
                self.next_missing += 1;
                self.missing.insert(child, arrival);
                Slot::Missing(arrival)
            }
        }
    }

    /// The index of `child` in `parent`'s list of children, reading on from where the last lookup
    /// in that list stopped. `None` when the list does not hold it.
    fn child_index(&mut self, parent: &N, child: &N, reading: Reading) -> Option<usize> {
        if self.lists.get(parent).is_none() {
            self.lists.insert(parent, ChildIndex { by_node: NodeMap::default(), read: 0, complete: false });
        }
        let list = self.lists.get_mut(parent)?;
        let mut found = list.by_node.get(child).copied();
        if list.complete || (found.is_some() && reading == Reading::UpToTheNode) {
            return found;
        }
        for (index, sibling) in parent.children().enumerate().skip(list.read) {
            list.by_node.insert(&sibling, index);
            list.read = index + 1;
            if found.is_none() && sibling == *child {
                found = Some(index);
                if reading == Reading::UpToTheNode {
                    return found;
                }
            }
        }
        list.complete = true;
        found
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use super::{DocOrder, NodeSet, distinct_nodes, sort_nodes};
    use crate::model::simple::{SimpleNode, attr, doc, elem};
    use crate::model::{NodeKind, QName, XdmNode};
    use crate::xdm::XdmAtomicValue;

    /// A `SimpleNode` without its document-order key, as the real providers are: with or without
    /// its identity hint, maybe with one child left out of its parent's list, counting the calls to
    /// `attributes()`, recording whose list of children was read, and how far a list was read.
    #[derive(Clone)]
    struct Plain {
        node: SimpleNode,
        hinted: bool,
        hidden: Option<SimpleNode>,
        attribute_reads: Rc<Cell<usize>>,
        listed: Rc<RefCell<Vec<String>>>,
        furthest: Rc<Cell<usize>>,
    }

    /// A list of children that notes how many of its nodes were read.
    struct Listed {
        nodes: std::vec::IntoIter<Plain>,
        read: usize,
        furthest: Rc<Cell<usize>>,
    }

    impl Iterator for Listed {
        type Item = Plain;
        fn next(&mut self) -> Option<Plain> {
            let node = self.nodes.next()?;
            self.read += 1;
            self.furthest.set(self.furthest.get().max(self.read));
            Some(node)
        }
    }

    impl Plain {
        fn new(node: &SimpleNode) -> Self {
            Self {
                node: node.clone(),
                hinted: true,
                hidden: None,
                attribute_reads: Rc::new(Cell::new(0)),
                listed: Rc::new(RefCell::new(Vec::new())),
                furthest: Rc::new(Cell::new(0)),
            }
        }
        fn view(&self, node: SimpleNode) -> Self {
            Self { node, ..self.clone() }
        }
    }

    impl PartialEq for Plain {
        fn eq(&self, other: &Self) -> bool {
            self.node == other.node
        }
    }

    impl Eq for Plain {}

    impl std::fmt::Debug for Plain {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "{}", self.node)
        }
    }

    impl XdmNode for Plain {
        type Children<'a> = Listed;
        type Attributes<'a> = std::vec::IntoIter<Plain>;
        type Namespaces<'a> = std::vec::IntoIter<Plain>;

        fn kind(&self) -> NodeKind {
            self.node.kind()
        }
        fn name(&self) -> Option<QName> {
            self.node.name()
        }
        fn typed_value(&self) -> Vec<XdmAtomicValue> {
            self.node.typed_value()
        }
        fn parent(&self) -> Option<Self> {
            self.node.parent().map(|node| self.view(node))
        }
        fn children(&self) -> Self::Children<'_> {
            self.listed.borrow_mut().push(label(&self.node));
            let hidden = self.hidden.clone();
            let nodes = self
                .node
                .children()
                .filter(|child| hidden.as_ref() != Some(child))
                .map(|node| self.view(node))
                .collect::<Vec<_>>();
            Listed { nodes: nodes.into_iter(), read: 0, furthest: Rc::clone(&self.furthest) }
        }
        fn attributes(&self) -> Self::Attributes<'_> {
            self.attribute_reads.set(self.attribute_reads.get() + 1);
            self.node.attributes().map(|node| self.view(node)).collect::<Vec<_>>().into_iter()
        }
        fn namespaces(&self) -> Self::Namespaces<'_> {
            Vec::new().into_iter()
        }
        fn identity_hint(&self) -> Option<u64> {
            if self.hinted { self.node.identity_hint() } else { None }
        }
    }

    /// `r:[a:[b1,b2],c]` with an `id` on every element, and attributes `x` and `y` on `a`.
    fn tree() -> SimpleNode {
        let named = |id: &str| elem(&id[..1]).attr(attr("id", id));
        doc()
            .child(
                named("r")
                    .child(named("a").attr(attr("y", "2")).attr(attr("x", "1")).child(named("b1")).child(named("b2")))
                    .child(named("c")),
            )
            .build()
    }

    fn find(root: &SimpleNode, id: &str) -> SimpleNode {
        fn walk(node: &SimpleNode, id: &str) -> Option<SimpleNode> {
            if node.attributes().any(|a| a.name().is_some_and(|q| q.local == "id") && a.string_value() == id) {
                return Some(node.clone());
            }
            node.children().find_map(|child| walk(&child, id))
        }
        walk(root, id).unwrap_or_else(|| panic!("no node {id}"))
    }

    fn label(node: &SimpleNode) -> String {
        match node.kind() {
            NodeKind::Document => "#doc".to_string(),
            NodeKind::Attribute => format!("@{}", node.name().map(|q| q.local).unwrap_or_default()),
            _ => node
                .attributes()
                .find(|a| a.name().is_some_and(|q| q.local == "id"))
                .map(|a| a.string_value())
                .unwrap_or_default(),
        }
    }

    fn sorted(nodes: Vec<Plain>) -> Vec<String> {
        let mut nodes = nodes;
        sort_nodes(&mut nodes);
        nodes.iter().map(|node| label(&node.node)).collect()
    }

    #[test]
    fn a_node_set_removes_duplicates_with_and_without_identity_hints() {
        let document = tree();
        for hinted in [true, false] {
            let nodes: Vec<Plain> =
                ["a", "c", "b1"].iter().map(|id| Plain { hinted, ..Plain::new(&find(&document, id)) }).collect();
            let mut set: NodeSet<Plain> = NodeSet::default();
            let kept: Vec<&Plain> = nodes.iter().chain(nodes.iter()).filter(|node| set.insert(node)).collect();
            assert_eq!(kept.len(), 3, "each node once (hinted: {hinted})");
            assert!(nodes.iter().all(|node| set.contains(node)), "every node is in the set (hinted: {hinted})");
            assert_eq!(distinct_nodes(nodes.iter().chain(nodes.iter()).cloned().collect()), nodes);
        }
    }

    #[test]
    fn nodes_with_keys_sort_by_their_keys() {
        let document = tree();
        let mut nodes: Vec<SimpleNode> = ["c", "b2", "r", "a", "b1"].iter().map(|id| find(&document, id)).collect();
        sort_nodes(&mut nodes);
        assert_eq!(nodes.iter().map(label).collect::<Vec<_>>(), ["r", "a", "b1", "b2", "c"]);
    }

    #[test]
    fn nodes_without_keys_sort_by_their_places_below_the_root() {
        let document = tree();
        for hinted in [true, false] {
            let view = Plain { hinted, ..Plain::new(&document) };
            let nodes = ["c", "b2", "r", "a", "b1"].iter().map(|id| view.view(find(&document, id))).collect();
            assert_eq!(sorted(nodes), ["r", "a", "b1", "b2", "c"], "hinted: {hinted}");
        }
    }

    #[test]
    fn attributes_follow_their_owner_by_name_without_reading_its_attributes() {
        let document = tree();
        let view = Plain::new(&document);
        let a = find(&document, "a");
        let mut nodes: Vec<Plain> = vec![view.view(find(&document, "b1"))];
        nodes.extend(a.attributes().map(|attribute| view.view(attribute)));
        nodes.push(view.view(a));
        view.attribute_reads.set(0);
        assert_eq!(sorted(nodes), ["a", "@id", "@x", "@y", "b1"]);
        assert_eq!(view.attribute_reads.get(), 0, "the sort must not read an element's attributes");
    }

    #[test]
    fn a_node_missing_from_its_parents_list_goes_after_the_list_in_arrival_order() {
        let document = tree();
        let b1 = find(&document, "b1");
        let view = Plain { hidden: Some(b1.clone()), ..Plain::new(&document) };
        let nodes: Vec<Plain> = ["c", "b1", "b2", "a"].iter().map(|id| view.view(find(&document, id))).collect();
        assert_eq!(sorted(nodes.clone()), ["a", "b2", "b1", "c"]);
        assert_eq!(sorted(nodes), ["a", "b2", "b1", "c"], "and the same on every run");
    }

    #[test]
    fn comparing_two_nodes_reads_no_list_above_their_deepest_shared_ancestor() {
        let document = tree();
        let view = Plain { hinted: false, ..Plain::new(&document) };
        let [a, b1, b2, c] = ["a", "b1", "b2", "c"].map(|id| view.view(find(&document, id)));
        let lists_read = || {
            let mut lists = view.listed.borrow().clone();
            lists.dedup();
            lists
        };
        let mut order = DocOrder::default();
        assert!(order.precedes(&b1, &b2));
        assert!(!order.precedes(&b2, &b1));
        assert!(order.precedes(&a, &b1), "an ancestor comes first");
        assert!(!order.precedes(&b1, &b1));
        assert_eq!(lists_read(), ["a"], "only the list of b1 and b2's parent");
        assert!(order.precedes(&b2, &c));
        assert_eq!(lists_read(), ["a", "r"], "then the list of r, where b2 and c part");
    }

    #[test]
    fn comparing_two_siblings_reads_their_list_only_as_far_as_the_later_one() {
        let named = |id: &str| elem(&id[..1]).attr(attr("id", id));
        let parent = (0..10).fold(named("r"), |parent, index| parent.child(named(&format!("x{index}"))));
        let document = doc().child(parent).build();
        let view = Plain { hinted: false, ..Plain::new(&document) };
        let [x1, x2, x6] = ["x1", "x2", "x6"].map(|id| view.view(find(&document, id)));
        let mut order = DocOrder::default();
        assert!(!order.precedes(&x2, &x1));
        assert_eq!(view.furthest.get(), 3, "r's list up to x2");
        assert!(order.precedes(&x2, &x6));
        assert_eq!(view.furthest.get(), 7, "then on to x6, not to the end of the list");
    }

    #[test]
    fn nodes_of_different_roots_keep_the_order_in_which_their_roots_appeared() {
        let first = tree();
        let second = tree();
        let nodes: Vec<Plain> =
            vec![Plain::new(&find(&second, "c")), Plain::new(&find(&first, "a")), Plain::new(&find(&second, "a"))];
        let mut sorted_nodes = nodes;
        sort_nodes(&mut sorted_nodes);
        assert_eq!(sorted_nodes.iter().map(|node| label(&node.node)).collect::<Vec<_>>(), ["a", "c", "a"]);
        assert!(sorted_nodes[0].node == find(&second, "a"), "the second tree's root appeared first");
    }
}
