mod keyboard;
mod keyboard_sequence;
mod pointer;
pub mod provider;
pub mod runtime;
mod xpath;

#[cfg(all(target_os = "windows", not(feature = "mock-provider")))]
const _: () = {
    use platynui_platform_windows as _;
    use platynui_provider_windows_uia as _;
};

#[cfg(all(target_os = "linux", not(feature = "mock-provider")))]
const _: () = {
    use platynui_platform_linux_x11 as _;
    use platynui_provider_atspi as _;
};

#[cfg(all(target_os = "macos", not(feature = "mock-provider")))]
const _: () = {
    use platynui_platform_macos as _;
    use platynui_provider_macos_ax as _;
};

pub use keyboard_sequence::{KeyboardSequence, KeyboardSequenceError};
pub use pointer::PointerError;
pub use runtime::{FocusError, Runtime};
pub use xpath::{
    EvaluateError, EvaluateOptions, EvaluatedAttribute, EvaluationItem, NodeResolver, evaluate,
};
