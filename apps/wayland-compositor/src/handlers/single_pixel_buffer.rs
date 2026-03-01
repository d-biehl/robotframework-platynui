//! `wp-single-pixel-buffer-v1` handler — efficient solid-color surfaces.
//!
//! Allows clients to create surfaces backed by a single pixel, avoiding
//! the overhead of allocating a full buffer for solid-color backgrounds.
//!
//! No handler trait is required — `delegate_single_pixel_buffer!()` in `state.rs`
//! handles all protocol requests automatically. Requires `BufferHandler`.
