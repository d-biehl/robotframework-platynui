//! `wp-viewporter` handler — surface scaling/cropping.
//!
//! Viewporter allows clients to specify source rectangles and destination sizes
//! for surfaces. Used by GTK4, Qt6, and Chromium for efficient scaling.
//!
//! No handler trait is required — `ViewporterState` handles all protocol
//! requests, routed through `delegate_dispatch2!` in `state.rs`.
