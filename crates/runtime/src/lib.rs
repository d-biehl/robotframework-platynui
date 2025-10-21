mod keyboard;
mod keyboard_sequence;
mod pointer;
pub mod provider;
pub mod runtime;
#[cfg(test)]
pub mod test_support;
mod xpath;

// Optional mock feature: explicitly link mock providers when feature is enabled
#[cfg(feature = "mock-provider")]
const _: () = {
    use platynui_platform_mock as _;
    use platynui_provider_mock as _;
};

// Runtime no longer auto-links platform providers; application crates should
// link their desired providers (Windows/Linux/macOS) explicitly. This keeps
// unit tests simple and predictable.

pub use keyboard_sequence::{KeyboardSequence, KeyboardSequenceError};
pub use pointer::{PointerError, PointerOverrides, PointerProfile, PointerSettings};
pub use runtime::{FocusError, Runtime};
pub use xpath::{
    EvaluateError, EvaluateOptions, EvaluatedAttribute, EvaluationItem, EvaluationStream, NodeResolver, evaluate,
};
