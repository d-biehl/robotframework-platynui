//! `wp-cursor-shape-v1` handler — server-side cursor shape management.
//!
//! Allows clients to set the cursor shape by name (e.g., `pointer`, `text`,
//! `grab`) instead of providing a cursor surface. The compositor resolves
//! the shape via the cursor theme loaded from `$XCURSOR_THEME`/`$XCURSOR_SIZE`.
//!
//! No handler trait is required — `delegate_cursor_shape!()` in `state.rs`
//! handles all protocol requests automatically. Requires `SeatHandler` and
//! `TabletSeatHandler`.

use smithay::wayland::tablet_manager::TabletSeatHandler;

use crate::state::State;

impl TabletSeatHandler for State {}
