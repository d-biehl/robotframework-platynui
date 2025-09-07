use std::{
    any::Any,
    sync::{Arc, Weak},
};

pub trait Attribute: Send + Sync {
    fn name(&self) -> &str;
    fn value(&self) -> Option<Arc<dyn Any + Send + Sync>>;
    fn namespace_uri(&self) -> &str;
}

pub trait Node: Send + Sync {
    fn parent(&self) -> Option<Weak<dyn Node>>;
    fn children(&self) -> Vec<Arc<dyn Node>>;

    fn attributes(&self) -> Vec<Arc<dyn Attribute>>;

    fn invalidate(&self);

    fn local_name(&self) -> &str;
    fn namespace_uri(&self) -> &str;
}
