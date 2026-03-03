//! Multi-Monitor support — create and manage multiple virtual outputs.
//!
//! CLI flag `--outputs <N>` creates N monitors arranged according to
//! `--output-layout`. Each output is a separate `wl_output` global with its
//! own mode and scale. Headless outputs are off-screen; winit creates one
//! combined view; DRM maps to physical connectors.

use smithay::{
    desktop::{Space, Window},
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::wayland_server::DisplayHandle,
    utils::{Physical, Size},
};

use crate::state::State;

/// Layout strategy for arranging multiple outputs.
#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
pub enum OutputLayout {
    /// Outputs side by side, left to right.
    #[default]
    Horizontal,
    /// Outputs stacked, top to bottom.
    Vertical,
}

/// Configuration for a single output.
#[derive(Debug, Clone)]
pub struct OutputConfig {
    /// Name of the output (e.g. "PLATYNUI-1").
    pub name: String,
    /// Resolution in pixels (physical).
    pub size: Size<i32, Physical>,
    /// Refresh rate in millihertz.
    pub refresh: i32,
    /// Position in the global logical coordinate space.
    pub position: (i32, i32),
    /// Scale factor (e.g. `1.0`, `1.5`, `2.0`). Default: `1.0`.
    pub scale: f64,
}

/// Create output configurations for `count` monitors with the given scale.
///
/// All outputs share the same resolution (from the CLI `--width`/`--height`) and
/// scale factor (`--scale`).  Positions use **logical** coordinates: each output
/// occupies `width/scale × height/scale` logical pixels, so adjacent outputs tile
/// without gaps regardless of the scale factor.
///
/// # Panics
///
/// Panics if `width` or `height` exceed `i32::MAX`, or if `count` exceeds `i32::MAX`.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn create_output_configs(
    count: u32,
    width: u32,
    height: u32,
    layout: OutputLayout,
    scale: f64,
) -> Vec<OutputConfig> {
    let w = i32::try_from(width).expect("width exceeds i32::MAX");
    let h = i32::try_from(height).expect("height exceeds i32::MAX");
    let effective_scale = if scale > 0.0 { scale } else { 1.0 };

    // Logical size per output — the space each output occupies in the
    // compositor's logical coordinate system.
    let logical_w = (f64::from(w) / effective_scale).round() as i32;
    let logical_h = (f64::from(h) / effective_scale).round() as i32;

    (0..count)
        .map(|i| {
            let idx = i32::try_from(i).expect("output count exceeds i32::MAX");
            let position = match layout {
                OutputLayout::Horizontal => (idx * logical_w, 0),
                OutputLayout::Vertical => (0, idx * logical_h),
            };
            OutputConfig {
                name: format!("PLATYNUI-{}", i + 1),
                size: (w, h).into(),
                refresh: crate::state::DEFAULT_REFRESH_MHTZ,
                position,
                scale: effective_scale,
            }
        })
        .collect()
}

/// Create Smithay `Output` objects from configurations and register them as
/// Wayland globals.
///
/// Returns the created outputs in order. Each output is also mapped into the
/// provided `Space` at its configured position.
pub fn create_outputs(configs: &[OutputConfig], dh: &DisplayHandle, space: &mut Space<Window>) -> Vec<Output> {
    configs
        .iter()
        .map(|cfg| {
            let output = Output::new(
                cfg.name.clone(),
                PhysicalProperties {
                    size: (0, 0).into(),
                    subpixel: Subpixel::Unknown,
                    make: "PlatynUI".to_string(),
                    model: "Wayland Compositor".to_string(),
                },
            );

            let mode = Mode { size: cfg.size, refresh: cfg.refresh };
            let scale = if cfg.scale > 0.0 { Some(smithay::output::Scale::Fractional(cfg.scale)) } else { None };
            output.change_current_state(Some(mode), None, scale, Some(cfg.position.into()));
            output.set_preferred(mode);
            output.create_global::<State>(dh);

            space.map_output(&output, cfg.position);

            tracing::info!(
                name = cfg.name,
                x = cfg.position.0,
                y = cfg.position.1,
                width = cfg.size.w,
                height = cfg.size.h,
                scale = cfg.scale,
                "output created",
            );

            output
        })
        .collect()
}
