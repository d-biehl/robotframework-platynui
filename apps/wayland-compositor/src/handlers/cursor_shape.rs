//! `wp-cursor-shape-v1` handler — server-side cursor shape management.
//!
//! Allows clients to set the cursor shape by name (e.g., `pointer`, `text`,
//! `grab`) instead of providing a cursor surface. The compositor resolves
//! the shape via the cursor theme loaded from `$XCURSOR_THEME`/`$XCURSOR_SIZE`.
//!
//! No handler trait is required — smithay's cursor-shape module handles all
//! protocol requests (routed through `delegate_dispatch2!` in `state.rs`). It
//! requires `SeatHandler` and `TabletSeatHandler`; tablet tools share the
//! pointer's focus targets.

use smithay::input::tablet::TabletSeatHandler;

use crate::{focus::PointerFocusTarget, state::State};

impl TabletSeatHandler for State {
    type ToolFocus = PointerFocusTarget;
}
