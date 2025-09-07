use pest::Parser;
use platynui_xpath::compile_xpath;
use platynui_xpath::model::{NodeKind, QName, XdmNode};
use platynui_xpath::parser::{Rule, XPathParser};
use platynui_xpath::runtime::StaticContext;
use platynui_xpath::xdm::XdmItem;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct Dom {
    nodes: Vec<NodeRec>,
}

#[derive(Debug, Clone)]
struct NodeRec {
    kind: NodeKind,
    name: Option<QName>,
    value: String,
    parent: Option<usize>,
    children: Vec<usize>,
    attrs: Vec<usize>,
}

#[derive(Debug, Clone)]
struct Node {
    dom: Arc<Dom>,
    idx: usize,
}

impl PartialEq for Node {
    fn eq(&self, o: &Self) -> bool {
        Arc::ptr_eq(&self.dom, &o.dom) && self.idx == o.idx
    }
}
impl Eq for Node {}

impl XdmNode for Node {
    fn kind(&self) -> NodeKind {
        self.dom.nodes[self.idx].kind.clone()
    }
    fn name(&self) -> Option<QName> {
        self.dom.nodes[self.idx].name.clone()
    }
    fn string_value(&self) -> String {
        self.dom.nodes[self.idx].value.clone()
    }
    fn parent(&self) -> Option<Self> {
        self.dom.nodes[self.idx].parent.map(|i| Node {
            dom: self.dom.clone(),
            idx: i,
        })
    }
    fn children(&self) -> Vec<Self> {
        self.dom.nodes[self.idx]
            .children
            .iter()
            .map(|&i| Node {
                dom: self.dom.clone(),
                idx: i,
            })
            .collect()
    }
    fn attributes(&self) -> Vec<Self> {
        self.dom.nodes[self.idx]
            .attrs
            .iter()
            .map(|&i| Node {
                dom: self.dom.clone(),
                idx: i,
            })
            .collect()
    }
    fn compare_document_order(&self, other: &Self) -> std::cmp::Ordering {
        self.idx.cmp(&other.idx)
    }
}

fn el(dom: &mut Dom, p: Option<usize>, local: &str) -> usize {
    let i = dom.nodes.len();
    dom.nodes.push(NodeRec {
        kind: NodeKind::Element,
        name: Some(QName {
            prefix: None,
            local: local.into(),
            ns_uri: None,
        }),
        value: String::new(),
        parent: p,
        children: vec![],
        attrs: vec![],
    });
    if let Some(pp) = p {
        dom.nodes[pp].children.push(i);
    }
    i
}
fn at(dom: &mut Dom, p: usize, local: &str, v: &str) -> usize {
    let i = dom.nodes.len();
    dom.nodes.push(NodeRec {
        kind: NodeKind::Attribute,
        name: Some(QName {
            prefix: None,
            local: local.into(),
            ns_uri: None,
        }),
        value: v.into(),
        parent: Some(p),
        children: vec![],
        attrs: vec![],
    });
    dom.nodes[p].attrs.push(i);
    i
}
fn tx(dom: &mut Dom, p: usize, v: &str) -> usize {
    let i = dom.nodes.len();
    dom.nodes.push(NodeRec {
        kind: NodeKind::Text,
        name: None,
        value: v.into(),
        parent: Some(p),
        children: vec![],
        attrs: vec![],
    });
    dom.nodes[p].children.push(i);
    i
}

fn sample() -> Node {
    let mut d = Dom { nodes: vec![] };
    let root = el(&mut d, None, "root");
    let a1 = el(&mut d, Some(root), "a");
    at(&mut d, a1, "id", "x");
    let _b = el(&mut d, Some(a1), "b");
    let a2 = el(&mut d, Some(root), "a");
    at(&mut d, a2, "id", "y");
    let c = el(&mut d, Some(a2), "c");
    tx(&mut d, c, "hello");
    Node {
        dom: Arc::new(d),
        idx: root,
    }
}

fn names<T: XdmNode>(items: &Vec<XdmItem<T>>) -> Vec<String> {
    let mut v = vec![];
    for it in items {
        if let XdmItem::Node(n) = it
            && let Some(q) = n.name()
        {
            v.push(q.local);
        }
    }
    v
}

fn main() {
    let root = sample();
    let sc = StaticContext::default();
    for expr in ["a", "//c", "a | //c", "a/descendant::a", "a/descendant::c"] {
        if false {
            println!("\nParse tree for '1 to 3':");
            let pairs = XPathParser::parse(platynui_xpath::parser::Rule::xpath, expr).unwrap();
            fn walk(pair: pest::iterators::Pair<Rule>, depth: usize) {
                println!(
                    "{:indent$}{:?} -> '{}'",
                    "",
                    pair.as_rule(),
                    pair.as_str(),
                    indent = depth * 2
                );
                for p in pair.clone().into_inner() {
                    walk(p, depth + 1);
                }
            }
            for p in pairs {
                walk(p, 0);
            }
        }
        match platynui_xpath::parser::XPathParser::parse_to_ast(expr) {
            Ok(ast) => println!("AST built: {:?}", ast),
            Err(e) => println!("parse_to_ast error: {:?}", e),
        }
        let exec = compile_xpath(expr, &sc).unwrap();
        println!("expr: {}\nIR:\n{}", expr, exec.debug_dump_ir());
        if expr.contains('a') || expr.contains('c') {
            let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root.clone())).unwrap();
            println!("result names: {:?}\n", names(&out));
        } else {
            let out: Vec<XdmItem<Node>> = exec.evaluate_on(Some(root.clone())).unwrap();
            println!("result len: {}\n", out.len());
        }
    }
}
