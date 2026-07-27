//! The single Java UiTree provider: a router over toolkit backends.
//!
//! Java applications reach PlatynUI through more than one channel — the Java
//! Access Bridge for Swing/AWT on Windows today, an in-JVM agent next — and
//! those channels do not cover the same windows. Registering one provider per
//! channel would make two of them compete for the same top-level windows and
//! force the process-wide window-claims registry from its boolean "claimed by
//! someone else" question into rank-based ownership. So there is exactly one
//! registered Java provider, and a channel is a *backend* of it: the claim
//! stays boolean because there is only ever one Java claimant, and gaining a
//! backend changes which backend *serves* a window, never who *claims* it.
//!
//! What the router owns: registration, configuration
//! (`providers.java.*` — see [`provider`]), the window claims, backend
//! selection per top-level window, and the shared "JVM window absent from
//! native accessibility" diagnostic for Java-looking windows no backend can
//! serve. What a backend owns: everything about its channel — the tree, roles,
//! patterns, `@Technology`, node validity, and its own robustness. Backend
//! nodes are handed through unwrapped, so those answers stay the backend's.
//!
//! See the `unify-java-provider` OpenSpec change for the design and
//! `dev-docs/platform-windows.md` for the JAB backend's specifics.

// The crate opts into the workspace lints. Backtick-pedantry on prose that is
// full of product and API names adds noise without catching bugs:
#![allow(clippy::doc_markdown)]

#[cfg(windows)]
mod agent;
#[cfg(windows)]
mod backend;
#[cfg(windows)]
mod jab;
#[cfg(windows)]
mod provider;

#[cfg(windows)]
pub use backend::{Enumeration, JavaBackend, UnservedJavaWindow};
#[cfg(windows)]
pub use provider::{JAVA_FACTORY, JavaFactory, PROVIDER_ID, PROVIDER_NAME, TECHNOLOGY};

// Dev-dependencies consumed only by the integration tests
// (tests/live_fixture.rs); referenced here so the lib-test target does not
// trip `unused_crate_dependencies`.
#[cfg(all(windows, test))]
mod dev_dependency_links {
    use platynui_platform_windows as _;
    use platynui_provider_windows_uia as _;
    use windows as _;
}
