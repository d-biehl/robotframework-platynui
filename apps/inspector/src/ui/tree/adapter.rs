use slint::{ModelRc, SharedString};

/// Port/Adapter interface the Slint UI talks to. Implementations may pull real data or demo data.
pub trait TreeViewAdapter {
    fn visible_model(&self) -> ModelRc<crate::TreeNodeVM>;
    fn toggle(&mut self, id: &str, expand: bool);
    fn request_children(&mut self, id: &str);
    fn parent_of(&self, id: &str) -> Option<SharedString>;
}
