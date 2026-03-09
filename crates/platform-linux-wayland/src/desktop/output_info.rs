//! Per-output information collected from Wayland protocols and compositor D-Bus APIs.

use wayland_client::protocol::wl_output;

/// Information collected from `wl_output` and optionally `zxdg_output_v1` events,
/// enriched with compositor-specific data (fractional scale, primary flag) via D-Bus.
#[derive(Debug, Clone, Default)]
pub struct OutputInfo {
    /// Physical position in compositor-global coordinates.
    pub x: i32,
    pub y: i32,
    /// Current mode dimensions (hardware pixels).
    pub width: i32,
    pub height: i32,
    /// Scale factor advertised by the compositor.
    pub scale: i32,
    /// Output transform (rotation/flip) from `wl_output.geometry`.
    pub transform: Option<wl_output::Transform>,
    /// Human-readable name (from `wl_output.name` since v4 or `zxdg_output_v1.name`).
    pub name: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Logical position from `xdg-output` (preferred over geometry x/y).
    pub logical_x: Option<i32>,
    pub logical_y: Option<i32>,
    /// Logical size from `xdg-output`.
    pub logical_width: Option<i32>,
    pub logical_height: Option<i32>,
    /// Whether this is the primary monitor (set by compositor-specific query).
    pub is_primary: bool,
    /// Fractional scale from compositor D-Bus API (e.g. Mutter `GetCurrentState()`).
    /// When set, `effective_scale()` returns this value directly instead of
    /// computing the ratio from physical/logical dimensions.
    pub fractional_scale: Option<f64>,
}

impl OutputInfo {
    /// Whether the output transform involves a 90° or 270° rotation,
    /// which swaps width and height.
    fn is_rotated(&self) -> bool {
        matches!(
            self.transform,
            Some(
                wl_output::Transform::_90
                    | wl_output::Transform::_270
                    | wl_output::Transform::Flipped90
                    | wl_output::Transform::Flipped270
            )
        )
    }

    /// Effective position — prefers xdg-output logical, falls back to `wl_output` geometry.
    #[must_use]
    pub fn effective_x(&self) -> i32 {
        self.logical_x.unwrap_or(self.x)
    }

    #[must_use]
    pub fn effective_y(&self) -> i32 {
        self.logical_y.unwrap_or(self.y)
    }

    /// Physical pixel width as visible to the user (accounts for rotation).
    #[must_use]
    pub fn physical_width(&self) -> i32 {
        if self.is_rotated() { self.height } else { self.width }
    }

    /// Physical pixel height as visible to the user (accounts for rotation).
    #[must_use]
    pub fn physical_height(&self) -> i32 {
        if self.is_rotated() { self.width } else { self.height }
    }

    /// Effective size — prefers xdg-output logical, falls back to mode / scale
    /// (accounting for output transform).
    #[must_use]
    pub fn effective_width(&self) -> i32 {
        self.logical_width.unwrap_or_else(|| {
            let hw = if self.is_rotated() { self.height } else { self.width };
            if self.scale > 0 { hw / self.scale } else { hw }
        })
    }

    #[must_use]
    pub fn effective_height(&self) -> i32 {
        self.logical_height.unwrap_or_else(|| {
            let hw = if self.is_rotated() { self.width } else { self.height };
            if self.scale > 0 { hw / self.scale } else { hw }
        })
    }

    /// Effective scale factor.
    ///
    /// Prefers the exact fractional scale obtained from the compositor's
    /// D-Bus API (e.g. Mutter `GetCurrentState()`). Falls back to computing
    /// the ratio from physical mode dimensions to xdg-output logical
    /// dimensions. Last resort is the integer `wl_output.scale`.
    #[must_use]
    pub fn effective_scale(&self) -> f64 {
        // 1. Exact fractional scale from compositor D-Bus API.
        if let Some(s) = self.fractional_scale {
            return s;
        }
        // 2. Derive from physical / logical dimensions.
        if let (Some(lw), Some(lh)) = (self.logical_width, self.logical_height)
            && lw > 0
            && lh > 0
        {
            let (phys_w, phys_h) =
                if self.is_rotated() { (self.height, self.width) } else { (self.width, self.height) };
            return if phys_w >= phys_h {
                f64::from(phys_w) / f64::from(lw)
            } else {
                f64::from(phys_h) / f64::from(lh)
            };
        }
        // 3. Integer fallback.
        f64::from(self.scale.max(1))
    }
}
