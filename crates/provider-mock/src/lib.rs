//! Deterministic mock UiTree provider for testing the runtime and CLI wiring.
//!
//! The provider exposes a deterministic, pattern-rich tree that mirrors common
//! desktop application structures. Consumers can install custom trees, emit
//! synthetic events, or query the current state without touching native APIs.

mod events;
pub mod factory;
mod focus;
mod input;
mod node;
mod provider;
pub mod tree;
mod window;

pub use events::{emit_event, emit_node_updated, node_by_runtime_id};
pub use factory::{MOCK_PROVIDER_FACTORY, MockProviderFactory, PROVIDER_ID, PROVIDER_NAME, TECHNOLOGY};
pub use input::{KeyboardInputEvent, TextInputError, append_text, apply_keyboard_events, replace_text, text_snapshot};
pub use tree::{AttributeSpec, NodeSpec, StaticMockTree, TreeGuard, install_mock_tree, reset_mock_tree};

#[cfg(test)]
pub use factory::{APP_RUNTIME_ID, BUTTON_RUNTIME_ID, WINDOW_RUNTIME_ID};

#[cfg(test)]
mod tests;

// Mock providers are NOT auto-registered.
// They are ONLY available via Python handles (Runtime.new_with_providers([MOCK_PROVIDER])).
// This ensures Runtime() uses only OS providers by default.
