use std::{
    any::Any,
    sync::{Arc, Weak},
};

pub trait Attribute: Send + Sync {
    fn name(&self) -> String;
    fn value(&self) -> Option<Arc<dyn Any + Send + Sync>>;
    fn namespace(&self) -> String;
}

pub trait UiNode: Send + Sync {
    fn parent(&self) -> Option<Weak<dyn UiNode>>;
    fn children(&self) -> Vec<Arc<dyn UiNode>>;

    fn attributes(&self) -> Vec<Arc<dyn Attribute>>;

    fn invalidate(&self);

    fn role(&self) -> String;
    fn namespace(&self) -> String;

    fn runtime_id(&self) -> String;
}
