//! `wp-viewporter` handler — surface scaling/cropping.
//!
//! Viewporter allows clients to specify source rectangles and destination sizes
//! for surfaces. Used by GTK4, Qt6, and Chromium for efficient scaling.
//!
//! No handler trait is required — `delegate_viewporter!()` in `state.rs` handles
//! all protocol requests automatically via `ViewporterState`.
