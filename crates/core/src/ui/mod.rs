pub mod attributes;
pub mod contract;
pub mod identifiers;
pub mod namespace;
pub mod node;
pub mod pattern;
pub mod value;

pub const DESKTOP_RUNTIME_ID: &str = "platynui:Desktop";

pub use attributes::pattern as attribute_names;
pub use contract::{ContractViolation, testkit, validate_control_or_item};
pub use identifiers::{PatternId, RuntimeId, TechnologyId};
pub use namespace::{Namespace, all_namespaces, resolve_namespace};
pub use node::{UiAttribute, UiNode, UiNodeAncestorIter, UiNodeExt};
pub use pattern::{
    FocusableAction, FocusablePattern, PatternError, PatternRegistry, UiPattern, WindowSurfaceActions,
    WindowSurfacePattern, downcast_pattern_arc, downcast_pattern_ref, supported_patterns_value,
};
pub use value::UiValue;
