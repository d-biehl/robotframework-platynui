//! Deterministic mock UiTree provider for testing the runtime and CLI wiring.
//!
//! The provider exposes a deterministic, pattern-rich tree that mirrors common
//! desktop application structures. Consumers can install custom trees, emit
//! synthetic events, or query the current state without touching native APIs.

mod events;
mod factory;
mod node;
mod provider;
pub mod tree;

pub use events::{emit_event, emit_node_updated, node_by_runtime_id};
pub use factory::{PROVIDER_ID, PROVIDER_NAME, TECHNOLOGY};
pub use tree::{
    AttributeSpec, NodeSpec, StaticMockTree, TreeGuard, install_mock_tree, reset_mock_tree,
};

#[cfg(test)]
pub use factory::{APP_RUNTIME_ID, BUTTON_RUNTIME_ID, WINDOW_RUNTIME_ID};

#[cfg(test)]
mod tests;

// Register the provider factory with the global inventory so the runtime can use it.
use factory::MOCK_PROVIDER_FACTORY;
use platynui_core::register_provider;

register_provider!(&MOCK_PROVIDER_FACTORY);
