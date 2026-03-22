//! Wayland window manager backend selection.

mod platynui_ipc;

use platynui_core::platform::{PlatformError, WindowId, WindowManager};
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::UiNode;

use crate::capabilities::CompositorType;

trait CompositorBackend: Send + Sync {
    fn name(&self) -> &'static str;
    fn resolve_window(&self, node: &dyn UiNode) -> Result<WindowId, PlatformError>;
    fn bounds(&self, id: WindowId, toolkit_hint: Option<&str>) -> Result<Rect, PlatformError>;
    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError>;
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

    fn bounds(&self, id: WindowId, toolkit_hint: Option<&str>) -> Result<Rect, PlatformError> {
        backend()?.bounds(id, toolkit_hint)
    }

    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError> {
        backend()?.is_active(id)
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
}

fn backend() -> Result<&'static dyn CompositorBackend, PlatformError> {
    match crate::connection::compositor_type() {
        Some(CompositorType::PlatynUi) => Ok(&PLATYNUI_IPC_BACKEND),
        Some(other) => Err(PlatformError::CapabilityUnavailable {
            capability: "Wayland window manager",
            details: Some(format!("no backend implemented yet for compositor {other}")),
        }),
        None => Err(PlatformError::InitializationFailed {
            component: "Wayland window manager",
            details: Some("platform-linux-wayland is not initialized".into()),
        }),
    }
}
