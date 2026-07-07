use crate::platform::PlatformError;
use crate::types::Rect;
use crate::ui::RuntimeId;
use crate::ui::identifiers::TechnologyId;

/// Describes a single monitor attached to the current desktop session.
#[must_use]
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorInfo {
    /// Stable identifier provided by the platform (may equal the OS display name).
    pub id: String,
    /// Human readable name shown in tooling.
    pub name: Option<String>,
    /// Bounding box of the monitor in desktop coordinates.
    pub bounds: Rect,
    /// Whether the monitor is the primary display.
    pub is_primary: bool,
    /// Optional scale factor (1.0 = 100%).
    pub scale_factor: Option<f64>,
}

impl MonitorInfo {
    pub fn new(id: impl Into<String>, bounds: Rect) -> Self {
        Self { id: id.into(), name: None, bounds, is_primary: false, scale_factor: None }
    }
}

/// Aggregate metadata about the desktop environment used to build the
/// `control:Desktop` node.
#[derive(Clone, Debug, PartialEq)]
pub struct DesktopInfo {
    pub runtime_id: RuntimeId,
    pub name: String,
    pub technology: TechnologyId,
    pub bounds: Rect,
    pub os_name: String,
    pub os_version: String,
    pub monitors: Vec<MonitorInfo>,
}

impl DesktopInfo {
    pub fn display_count(&self) -> usize {
        self.monitors.len()
    }

    pub fn primary_monitor(&self) -> Option<&MonitorInfo> {
        self.monitors.iter().find(|monitor| monitor.is_primary)
    }
}

/// Provides desktop metadata on demand.
pub trait DesktopInfoProvider: Send + Sync {
    fn desktop_info(&self) -> Result<DesktopInfo, PlatformError>;
}
