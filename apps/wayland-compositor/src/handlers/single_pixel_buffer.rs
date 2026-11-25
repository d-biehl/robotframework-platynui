//! `wp-single-pixel-buffer-v1` handler — efficient solid-color surfaces.
//!
//! Allows clients to create surfaces backed by a single pixel, avoiding
//! the overhead of allocating a full buffer for solid-color backgrounds.
//!
//! No handler trait is required — smithay's module handles all protocol
//! requests, routed through `delegate_dispatch2!` in `state.rs`. Requires
//! `BufferHandler`.
