//! `wp-presentation-time` handler — frame timing for video and animation.
//!
//! Provides accurate presentation timestamps to clients, enabling them to
//! synchronize rendering with display refresh.
//!
//! No handler trait is required — `delegate_presentation!()` in `state.rs`
//! handles all protocol requests automatically via `PresentationState`.
