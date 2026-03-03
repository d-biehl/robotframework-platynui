//! Window management — stacking policy, window placement, and space operations.

use smithay::{
    desktop::{Space, Window},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
    wayland::shell::xdg::XdgShellState,
};

/// Cascade offset between successive windows in logical pixels.
const CASCADE_OFFSET: i32 = 30;
/// Initial margin from the usable area origin (logical pixels).
const CASCADE_INITIAL_MARGIN: i32 = 10;
/// Horizontal wrap distance for cascade positioning.
const CASCADE_WRAP_HORIZONTAL: i32 = 300;
/// Vertical wrap distance for cascade positioning.
const CASCADE_WRAP_VERTICAL: i32 = 200;

/// Next cascade position tracker.
static NEXT_CASCADE: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

/// Map a new window into the space with cascading placement.
///
/// `usable_origin` is the top-left corner of the usable area (after layer
/// surface exclusive zones have been removed).  Cascade offsets are applied
/// relative to this point so new windows don't appear behind panels.
///
/// The SSD title-bar offset is **not** applied here because the decoration
/// mode is not yet negotiated at this point (`new_toplevel` fires before
/// `new_decoration`).  The offset is applied later in
/// [`XdgDecorationHandler::new_decoration`](crate::handlers::decoration).
pub fn map_window(space: &mut Space<Window>, window: Window, usable_origin: Point<i32, Logical>) {
    let offset = NEXT_CASCADE.fetch_add(CASCADE_OFFSET, std::sync::atomic::Ordering::Relaxed);
    let position: Point<i32, Logical> = (
        usable_origin.x + CASCADE_INITIAL_MARGIN + offset % CASCADE_WRAP_HORIZONTAL,
        usable_origin.y + CASCADE_INITIAL_MARGIN + offset % CASCADE_WRAP_VERTICAL,
    )
        .into();

    // Send an initial configure to suggest window dimensions.
    // Note: this configure does not include the decoration mode — it will
    // be sent again from `new_decoration` once the mode is negotiated.
    if let Some(toplevel) = window.toplevel() {
        toplevel.send_configure();
    }

    space.map_element(window, position, false);

    tracing::debug!(x = position.x, y = position.y, "mapped new window");
}

/// Handle surface commits that may require initial configure for unmapped toplevels.
pub fn handle_commit(xdg_shell_state: &XdgShellState, surface: &WlSurface) {
    for toplevel in xdg_shell_state.toplevel_surfaces() {
        if toplevel.wl_surface() == surface && !toplevel.is_initial_configure_sent() {
            toplevel.send_configure();
            break;
        }
    }
}
