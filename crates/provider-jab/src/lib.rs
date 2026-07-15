//! Java Access Bridge (JAB) UiTree provider for Windows.
//!
//! Java Swing/AWT applications implement no UIA provider, so their windows are
//! opaque to the Windows UIA provider. This crate reads their accessibility
//! trees through the JDK's own out-of-process channel instead: the target JVM
//! loads `JavaAccessBridge-64.dll` when accessibility is enabled, and this
//! provider loads the client side (`WindowsAccessBridge-64.dll`) and talks to
//! the JVM over hidden-window messages plus shared memory — the same channel
//! screen readers use. Nothing is ever injected into the target process, and
//! the provider never mutates target-side configuration (no user-profile
//! accessibility-properties file, no registry writes) — see the
//! `no_configuration_mutation_code_paths_exist` test pinning this.
//!
//! Threading model: one dedicated pump thread owns everything JAB — it loads
//! the DLL, calls `Windows_run()`, runs the Win32 message pump the bridge
//! rendezvous requires, and services all API calls with a per-call deadline
//! (`providers.jab.call_timeout_ms`). See `dev-docs/platform-windows.md` and
//! the `add-jab-provider` OpenSpec change for the full design.

// The crate opts into the workspace lints (notably `unsafe_code = deny`, with
// item-level allows at each FFI boundary). Backtick-pedantry on prose that is
// full of product and API names adds noise without catching bugs:
#![allow(clippy::doc_markdown)]

#[cfg(windows)]
mod client;
#[cfg(windows)]
mod dll;
#[cfg(windows)]
mod error;
#[cfg(windows)]
mod ffi;
#[cfg(windows)]
mod handle;
#[cfg(windows)]
mod map;
#[cfg(windows)]
mod node;
#[cfg(windows)]
mod process;
#[cfg(windows)]
mod provider;
#[cfg(windows)]
mod pump;

#[cfg(windows)]
pub use provider::{JabFactory, PROVIDER_ID, PROVIDER_NAME};

// Dev-dependencies consumed only by the integration tests
// (tests/live_fixture.rs); referenced here so the lib-test target does not
// trip `unused_crate_dependencies`.
#[cfg(all(windows, test))]
mod dev_dependency_links {
    use platynui_platform_windows as _;
    use platynui_provider_windows_uia as _;
}
