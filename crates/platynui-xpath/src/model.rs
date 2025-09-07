use core::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Document,
    Element,
    Attribute,
    Text,
    Comment,
    ProcessingInstruction,
    Namespace,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QName {
    pub prefix: Option<String>,
    pub local: String,
    pub ns_uri: Option<String>,
}

pub trait XdmNode: Clone + Eq + core::fmt::Debug + Send + Sync {
    fn kind(&self) -> NodeKind;
    fn name(&self) -> Option<QName>;
    fn string_value(&self) -> String;
    fn base_uri(&self) -> Option<String> { None }

    fn parent(&self) -> Option<Self>;
    fn children(&self) -> Vec<Self>;
    fn attributes(&self) -> Vec<Self>;
    fn namespaces(&self) -> Vec<Self> { Vec::new() }

    fn compare_document_order(&self, other: &Self) -> Ordering;
}

