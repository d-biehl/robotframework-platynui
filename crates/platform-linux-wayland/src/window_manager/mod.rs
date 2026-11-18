//! Wayland window manager backend selection.

mod platynui_ipc;

use platynui_core::platform::{
    PlatformError, PopupGeometry, WindowGeometry, WindowHit, WindowId, WindowManager, WindowState,
};
use platynui_core::types::{Point, Size};
use platynui_core::ui::UiNode;

use crate::capabilities::{CompositorType, unsupported_compositor};

trait CompositorBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError>;
    fn geometry(&self, id: WindowId) -> Result<WindowGeometry, PlatformError>;
    fn window_at_point(&self, _point: Point) -> Result<Option<WindowHit>, PlatformError> {
        Err(PlatformError::CapabilityUnavailable { capability: "window_at_point", details: None })
    }
    fn popups(&self, _pid: u32) -> Result<Vec<PopupGeometry>, PlatformError> {
        Err(PlatformError::CapabilityUnavailable { capability: "popups", details: None })
    }
    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError>;
    fn state(&self, _id: WindowId) -> Result<WindowState, PlatformError> {
        Err(PlatformError::CapabilityUnavailable { capability: "window_state", details: None })
    }
    fn activate(&self, id: WindowId) -> Result<(), PlatformError>;
    fn close(&self, id: WindowId) -> Result<(), PlatformError>;
    fn minimize(&self, id: WindowId) -> Result<(), PlatformError>;
    fn maximize(&self, id: WindowId) -> Result<(), PlatformError>;
    fn restore(&self, id: WindowId) -> Result<(), PlatformError>;
    fn move_to(&self, id: WindowId, position: Point) -> Result<(), PlatformError>;
    fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError>;
}

static PLATYNUI_IPC_BACKEND: platynui_ipc::PlatynUiIpcBackend = platynui_ipc::PlatynUiIpcBackend;

pub struct WaylandWindowManager;

impl WindowManager for WaylandWindowManager {
    fn name(&self) -> &'static str {
        match crate::connection::compositor_type() {
            Some(CompositorType::PlatynUi) => PLATYNUI_IPC_BACKEND.name(),
            _ => "Wayland",
        }
    }

    fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError> {
        backend()?.resolve_window(node)
    }

    fn geometry(&self, id: WindowId) -> Result<WindowGeometry, PlatformError> {
        backend()?.geometry(id)
    }

    /// A Wayland client does not know where its window is, so its toolkit
    /// reports screen positions relative to a point of its own choosing.
    fn toolkit_screen_coordinates_are_desktop(&self) -> bool {
        false
    }

    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError> {
        backend()?.is_active(id)
    }

    fn state(&self, id: WindowId) -> Result<WindowState, PlatformError> {
        backend()?.state(id)
    }

    fn activate(&self, id: WindowId) -> Result<(), PlatformError> {
        backend()?.activate(id)
    }

    fn close(&self, id: WindowId) -> Result<(), PlatformError> {
        backend()?.close(id)
    }

    fn minimize(&self, id: WindowId) -> Result<(), PlatformError> {
        backend()?.minimize(id)
    }

    fn maximize(&self, id: WindowId) -> Result<(), PlatformError> {
        backend()?.maximize(id)
    }

    fn restore(&self, id: WindowId) -> Result<(), PlatformError> {
        backend()?.restore(id)
    }

    fn move_to(&self, id: WindowId, position: Point) -> Result<(), PlatformError> {
        backend()?.move_to(id, position)
    }

    fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError> {
        backend()?.resize(id, size)
    }

    fn window_at_point(&self, point: Point) -> Result<Option<WindowHit>, PlatformError> {
        backend()?.window_at_point(point)
    }

    fn popups(&self, pid: u32) -> Result<Vec<PopupGeometry>, PlatformError> {
        backend()?.popups(pid)
    }
}

fn backend() -> Result<&'static dyn CompositorBackend, PlatformError> {
    match crate::connection::compositor_type() {
        Some(CompositorType::PlatynUi) => Ok(&PLATYNUI_IPC_BACKEND),
        other @ Some(_) => Err(unsupported_compositor("Wayland window manager", other)),
        None => Err(PlatformError::InitializationFailed {
            component: "Wayland window manager",
            details: Some("platform-linux-wayland is not initialized".into()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression anchor: without a detected compositor, every operation is an
    /// error, never a value.
    #[test]
    fn every_operation_fails_without_a_detected_compositor() {
        let wm = WaylandWindowManager;
        let id = WindowId::new(1);
        assert!(wm.geometry(id).is_err());
        assert!(wm.is_active(id).is_err());
        assert!(wm.state(id).is_err());
        assert!(wm.activate(id).is_err());
        assert!(wm.close(id).is_err());
        assert!(wm.minimize(id).is_err());
        assert!(wm.maximize(id).is_err());
        assert!(wm.restore(id).is_err());
        assert!(wm.move_to(id, Point::new(0.0, 0.0)).is_err());
        assert!(wm.resize(id, Size::new(10.0, 10.0)).is_err());
        assert!(wm.window_at_point(Point::new(0.0, 0.0)).is_err());
        assert!(wm.popups(1).is_err());
    }

    /// A Wayland client does not know where its window is, so its toolkit's
    /// screen coordinates are never desktop coordinates.
    #[test]
    fn toolkit_screen_coordinates_are_not_desktop_coordinates() {
        assert!(!WaylandWindowManager.toolkit_screen_coordinates_are_desktop());
    }
}
