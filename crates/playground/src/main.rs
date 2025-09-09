use platynui_core::strategies::{Attribute, Node};
use std::fmt;
use std::sync::{Arc, RwLock, Weak};

struct TestNode {
    local_name: String,
    namespace_uri: String,
    parent: RwLock<Option<Weak<Self>>>,
    children: RwLock<Vec<Arc<Self>>>,
    attributes: RwLock<Vec<Arc<dyn Attribute>>>,
}

impl TestNode {
    pub fn new(
        local: impl Into<String>,
        ns: impl Into<String>,
        parent: Option<&Arc<TestNode>>,
    ) -> Arc<Self> {
        let node = Arc::new(Self {
            local_name: local.into(),
            namespace_uri: ns.into(),
            parent: RwLock::new(None),
            children: RwLock::new(Vec::new()),
            attributes: RwLock::new(Vec::new()),
        });

        if let Some(p) = parent {
            // Avoid panicking on poisoned locks by recovering the inner guard
            node.parent
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .replace(Arc::downgrade(p));
            p.children
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .push(Arc::clone(&node));
        }

        node
    }
}

impl std::fmt::Debug for TestNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // only print fields that are Debug-friendly (avoid formatting dyn Node directly)
        let parent = self
            .parent
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .cloned();
        let children_len = self
            .children
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .len();
        let attributes_len = self
            .attributes
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .len();

        f.debug_struct("TestNode")
            .field("local_name", &self.local_name)
            .field("namespace_uri", &self.namespace_uri)
            .field("parent", &parent)
            .field("children_len", &children_len)
            .field("attributes_len", &attributes_len)
            .finish()
    }
}

impl Node for TestNode {
    fn parent(&self) -> Option<Weak<dyn Node>> {
        self.parent
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .map(|w| w.clone() as Weak<dyn Node>)
    }
    fn children(&self) -> Vec<Arc<dyn Node>> {
        self.children
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .cloned()
            .map(|c| c as Arc<dyn Node>)
            .collect()
    }
    fn attributes(&self) -> Vec<Arc<dyn Attribute>> {
        self.attributes
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    fn invalidate(&self) {}
    fn local_name(&self) -> &str {
        &self.local_name
    }
    fn namespace_uri(&self) -> &str {
        &self.namespace_uri
    }
}

fn main() {
    let root = TestNode::new("my-node", "http://example.com/ns", None);
    let a = TestNode::new("a", "ns", Some(&root));
    TestNode::new("b", "ns", Some(&root));
    TestNode::new("c", "ns", Some(&a));
    println!("Node local name: {}", root.local_name());
    println!("Node namespace URI: {}", root.namespace_uri());
}
