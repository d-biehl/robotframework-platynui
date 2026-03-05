//! `xdg-toplevel-icon-v1` handler — per-toplevel icon assignment.
//!
//! Clients use this staging protocol to set a custom icon (via an XDG
//! icon-theme name or pixel buffer) for individual toplevel windows.
//! Pixel-buffer icons are stored and rendered in the SSD titlebar.
//! Named icons (`set_name`) are logged but not resolved (we have no
//! icon-theme loader).
//!
//! Smithay 0.7 does not provide a high-level abstraction for this protocol.
//! We implement `GlobalDispatch` / `Dispatch` manually using the generated
//! bindings from `wayland-protocols 0.32`.

use std::sync::Mutex;

use smithay::{
    reexports::{
        wayland_protocols::xdg::toplevel_icon::v1::server::{
            xdg_toplevel_icon_manager_v1::{self, XdgToplevelIconManagerV1},
            xdg_toplevel_icon_v1::{self, XdgToplevelIconV1},
        },
        wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, backend::GlobalId},
    },
    wayland::shm::{BufferData, with_buffer_contents},
};

use crate::state::State;

// ---------------------------------------------------------------------------
// Icon pixel data — owned RGBA copy of the best-available buffer
// ---------------------------------------------------------------------------

/// RGBA pixel data for a toplevel icon, ready for egui rendering.
#[derive(Debug, Clone)]
pub struct ToplevelIconPixels {
    /// RGBA8 pixel data (row-major, no padding).
    pub rgba: Vec<u8>,
    /// Icon width in pixels.
    pub width: u32,
    /// Icon height in pixels.
    pub height: u32,
}

/// Per-icon-object state accumulated via `set_name` / `add_buffer` before
/// the icon is applied to a toplevel via `set_icon`.
#[derive(Debug, Default)]
struct IconBuilder {
    /// Named icon (from XDG icon theme) — stored but not resolved.
    #[allow(dead_code)]
    name: Option<String>,
    /// Best pixel buffer seen so far (highest resolution).
    best_pixels: Option<ToplevelIconPixels>,
}

// ---------------------------------------------------------------------------
// GlobalDispatch — advertise the xdg_toplevel_icon_manager_v1 global
// ---------------------------------------------------------------------------

impl GlobalDispatch<XdgToplevelIconManagerV1, ()> for State {
    fn bind(
        _state: &mut Self,
        _handle: &DisplayHandle,
        _client: &Client,
        resource: New<XdgToplevelIconManagerV1>,
        _global_data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        // Send `done` immediately (no preferred icon sizes).
        let manager = data_init.init(resource, ());
        manager.done();
    }
}

// ---------------------------------------------------------------------------
// Dispatch — handle manager requests (destroy, create_icon, set_icon)
// ---------------------------------------------------------------------------

impl Dispatch<XdgToplevelIconManagerV1, ()> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        _resource: &XdgToplevelIconManagerV1,
        request: xdg_toplevel_icon_manager_v1::Request,
        _data: &(),
        _dhandle: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            xdg_toplevel_icon_manager_v1::Request::CreateIcon { id } => {
                data_init.init(id, Mutex::new(IconBuilder::default()));
            }
            xdg_toplevel_icon_manager_v1::Request::SetIcon { toplevel, icon } => {
                let pixels = icon.as_ref().and_then(|icon_obj| {
                    icon_obj
                        .data::<Mutex<IconBuilder>>()
                        .and_then(|m| m.lock().ok())
                        .and_then(|builder| builder.best_pixels.clone())
                });

                if let Some(toplevel_surface) = state.xdg_shell_state.get_toplevel(&toplevel) {
                    let surface_id = toplevel_surface.wl_surface().id();
                    if let Some(px) = pixels {
                        tracing::debug!(
                            toplevel = %toplevel.id(),
                            width = px.width,
                            height = px.height,
                            "toplevel icon set (pixel buffer)"
                        );
                        state.toplevel_icons.insert(surface_id, px);
                    } else {
                        tracing::debug!(toplevel = %toplevel.id(), "toplevel icon cleared");
                        state.toplevel_icons.remove(&surface_id);
                    }
                }
            }
            xdg_toplevel_icon_manager_v1::Request::Destroy => {}
            _ => {
                tracing::debug!("unhandled toplevel icon manager request");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Dispatch — handle per-icon requests (destroy, set_name, add_buffer)
// ---------------------------------------------------------------------------

impl Dispatch<XdgToplevelIconV1, Mutex<IconBuilder>> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        resource: &XdgToplevelIconV1,
        request: xdg_toplevel_icon_v1::Request,
        data: &Mutex<IconBuilder>,
        _dhandle: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            xdg_toplevel_icon_v1::Request::SetName { icon_name } => {
                tracing::debug!(icon_name, "toplevel icon name set (not resolved — no icon theme loader)");
                if let Ok(mut builder) = data.lock() {
                    builder.name = Some(icon_name);
                }
            }
            xdg_toplevel_icon_v1::Request::AddBuffer { buffer, scale } => {
                if let Some(pixels) = read_icon_buffer(&buffer) {
                    tracing::debug!(width = pixels.width, height = pixels.height, scale, "toplevel icon buffer added");
                    if let Ok(mut builder) = data.lock() {
                        // Keep the largest buffer (best resolution).
                        let dominated =
                            builder.best_pixels.as_ref().is_none_or(|existing| pixels.width >= existing.width);
                        if dominated {
                            builder.best_pixels = Some(pixels);
                        }
                    }
                } else {
                    tracing::debug!("toplevel icon buffer unreadable (not shm or unsupported format)");
                    resource.post_error(
                        xdg_toplevel_icon_v1::Error::InvalidBuffer,
                        "buffer must be a square wl_shm buffer",
                    );
                }
            }
            xdg_toplevel_icon_v1::Request::Destroy => {}
            _ => {
                tracing::debug!("unhandled toplevel icon request");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Buffer reading helper
// ---------------------------------------------------------------------------

/// Read RGBA pixel data from a `wl_buffer` backed by `wl_shm`.
///
/// Returns `None` if the buffer is not shm-backed, has unsupported format,
/// or is not square (protocol requirement).
#[allow(clippy::cast_sign_loss)]
fn read_icon_buffer(
    buffer: &smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer,
) -> Option<ToplevelIconPixels> {
    // Read buffer metadata + pixel data in one call.
    let result: Result<(BufferData, Vec<u8>), _> = with_buffer_contents(buffer, |ptr, len, data| {
        // Validate dimensions before casting.
        if data.width <= 0 || data.height <= 0 || data.stride <= 0 || data.offset < 0 {
            return (data, Vec::new());
        }

        let width = data.width.cast_unsigned() as usize;
        let height = data.height.cast_unsigned() as usize;
        let stride = data.stride.cast_unsigned() as usize;
        let offset = data.offset.cast_unsigned() as usize;

        let row_bytes = width * 4;
        let mut rgba = Vec::with_capacity(row_bytes * height);

        for y in 0..height {
            let src_start = offset + y * stride;
            let src_end = src_start + width * 4;
            if src_end > len {
                return (data, Vec::new()); // truncated pool
            }

            // SAFETY: `ptr` is valid for `len` bytes (guaranteed by smithay's
            // `with_buffer_contents`).
            #[allow(unsafe_code)]
            let row = unsafe { std::slice::from_raw_parts(ptr.add(src_start), width * 4) };

            // Convert from Wayland ARGB8888 (B, G, R, A bytes on LE) to RGBA.
            for pixel in row.chunks_exact(4) {
                rgba.push(pixel[2]); // R
                rgba.push(pixel[1]); // G
                rgba.push(pixel[0]); // B
                rgba.push(pixel[3]); // A
            }
        }

        (data, rgba)
    });

    let (info, rgba) = result.ok()?;

    // Protocol requires square buffers.
    if info.width != info.height || info.width <= 0 {
        return None;
    }

    // Reject empty / truncated reads.
    let expected_len = info.width.cast_unsigned() as usize * info.height.cast_unsigned() as usize * 4;
    if rgba.len() != expected_len {
        return None;
    }

    Some(ToplevelIconPixels { rgba, width: info.width.cast_unsigned(), height: info.height.cast_unsigned() })
}

// ---------------------------------------------------------------------------
// Registration helper
// ---------------------------------------------------------------------------

/// Register the `xdg_toplevel_icon_manager_v1` global.
pub fn init_toplevel_icon(dh: &DisplayHandle) -> GlobalId {
    dh.create_global::<State, XdgToplevelIconManagerV1, _>(1, ())
}
