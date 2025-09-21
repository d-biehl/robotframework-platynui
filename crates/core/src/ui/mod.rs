pub mod attributes;
pub mod identifiers;
pub mod namespace;
pub mod node;
pub mod snapshot;
pub mod value;

pub use attributes::names as attribute_names;
pub use identifiers::{PatternId, RuntimeId, TechnologyId};
pub use namespace::{Namespace, all_namespaces, resolve_namespace};
pub use node::{AttributeKey, DesktopInfo, MonitorInfo, UiNode, UiNodeBuilder};
pub use snapshot::UiSnapshot;
pub use value::UiValue;
