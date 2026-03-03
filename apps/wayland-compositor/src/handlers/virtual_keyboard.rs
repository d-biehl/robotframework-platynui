//! Virtual keyboard protocol handler (`zwp-virtual-keyboard-v1`).
//!
//! Allows external tools to inject keyboard events into the compositor's
//! input stack. Smithay handles the heavy lifting — this module just
//! wires the delegation and registers the global with a security filter.
