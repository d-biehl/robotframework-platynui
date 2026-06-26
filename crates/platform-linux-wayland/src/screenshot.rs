//! Wayland screenshot provider.
//!
//! There is no single unprivileged "capture the screen" API across Wayland
//! compositors, so this provider routes on the *detected* compositor — the same
//! gate `highlight` and `window_manager` use (see [`crate::capabilities`]),
//! never the mere presence of the control socket (which could be a forwarded or
//! orphaned socket from a foreign compositor):
//!
//! - **Our own compositor** (`CompositorType::PlatynUi`): it exposes a
//!   `{"command": "screenshot"}` request on the control socket that returns the
//!   whole scene as a base64-encoded PNG (physical pixels, plus the output
//!   `scale`). We ask for that frame, decode it, and crop it to the requested
//!   region — so `Take Screenshot` works under our compositor like the X11 backend.
//! - **Any other compositor**: not implemented yet, so we return
//!   `CapabilityUnavailable`. Phase 1b will add `ext-image-copy-capture-v1`
//!   (primary), `wlr-screencopy` (older wlroots) and the `xdg-desktop-portal`
//!   Screenshot (Mutter/GNOME) as fallbacks — see `dev-docs/platform-linux-wayland.md` §5.

use platynui_core::platform::{PixelFormat, PlatformError, Screenshot, ScreenshotProvider, ScreenshotRequest};
use platynui_core::types::Rect;

use crate::capabilities::CompositorType;
use crate::control_ipc;

pub struct WaylandScreenshot;

impl ScreenshotProvider for WaylandScreenshot {
    fn capture(&self, request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
        // Only our own compositor serves screenshots over the control socket. Gate on the
        // detected compositor rather than the socket's presence (a foreign compositor never
        // creates it; a forwarded/orphaned one would be a false positive) — same check as
        // highlight and window_manager.
        let compositor = crate::connection::compositor_type();
        if compositor != Some(CompositorType::PlatynUi) {
            return Err(unsupported_compositor(compositor));
        }

        let response =
            control_ipc::send_command(&serde_json::json!({ "command": "screenshot" }), "Wayland screenshot")?;

        let data =
            response.get("data").and_then(serde_json::Value::as_str).ok_or_else(|| PlatformError::OperationFailed {
                operation: "Wayland screenshot",
                details: Some("compositor response is missing the base64 'data' field".into()),
            })?;
        // Physical-pixel frame; the region is in logical coordinates, so scale it.
        let scale = response.get("scale").and_then(serde_json::Value::as_f64).unwrap_or(1.0);

        let png_bytes = base64_decode(data).map_err(|details| PlatformError::OperationFailed {
            operation: "Wayland screenshot",
            details: Some(format!("base64 decode: {details}")),
        })?;

        let (full_w, full_h, pixels) = decode_png_rgba8(&png_bytes)?;

        match request.region {
            Some(region) => {
                let (w, h, cropped) = crop_rgba8(&pixels, full_w, full_h, region, scale)?;
                Ok(Screenshot::new(w, h, PixelFormat::Rgba8, cropped))
            }
            None => Ok(Screenshot::new(full_w, full_h, PixelFormat::Rgba8, pixels)),
        }
    }
}

/// The error returned when asked to screenshot a compositor we don't support yet. Phase 1b (see
/// `dev-docs/platform-linux-wayland.md` §5) will replace this with `ext-image-copy-capture-v1`,
/// `wlr-screencopy` or the `xdg-desktop-portal` Screenshot, selected per compositor.
fn unsupported_compositor(compositor: Option<CompositorType>) -> PlatformError {
    let which = compositor.map_or_else(|| "an undetected Wayland compositor".to_string(), |c| c.to_string());
    PlatformError::CapabilityUnavailable {
        capability: "Wayland screenshot",
        details: Some(format!(
            "not implemented for {which}; only the PlatynUI compositor (control socket) is supported so far"
        )),
    }
}

/// Decode the compositor's PNG frame into tightly-packed RGBA8 pixels.
fn decode_png_rgba8(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), PlatformError> {
    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(png_err)?;
    let buffer_size = reader.output_buffer_size().ok_or_else(|| PlatformError::OperationFailed {
        operation: "Wayland screenshot",
        details: Some("PNG output buffer size is unavailable (image too large?)".into()),
    })?;
    let mut buf = vec![0u8; buffer_size];
    let info = reader.next_frame(&mut buf).map_err(png_err)?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(PlatformError::OperationFailed {
            operation: "Wayland screenshot",
            details: Some(format!("unexpected PNG format from compositor: {:?}/{:?}", info.color_type, info.bit_depth)),
        });
    }
    buf.truncate(info.buffer_size());
    Ok((info.width, info.height, buf))
}

/// Crop a tightly-packed RGBA8 buffer to `region` (logical coords) scaled to the
/// frame's physical pixels, clamped to the frame bounds.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn crop_rgba8(
    pixels: &[u8],
    full_w: u32,
    full_h: u32,
    region: Rect,
    scale: f64,
) -> Result<(u32, u32, Vec<u8>), PlatformError> {
    let x = (region.x() * scale).round().max(0.0) as u32;
    let y = (region.y() * scale).round().max(0.0) as u32;
    let req_w = (region.width() * scale).round().max(0.0) as u32;
    let req_h = (region.height() * scale).round().max(0.0) as u32;

    if x >= full_w || y >= full_h || req_w == 0 || req_h == 0 {
        return Err(PlatformError::OperationFailed {
            operation: "Wayland screenshot",
            details: Some(format!(
                "requested region {region:?} (scale {scale}) is outside the {full_w}x{full_h} frame"
            )),
        });
    }

    let w = req_w.min(full_w - x);
    let h = req_h.min(full_h - y);
    let row_bytes = full_w as usize * 4;
    let mut out = Vec::with_capacity(w as usize * h as usize * 4);
    for row in 0..h as usize {
        let start = (y as usize + row) * row_bytes + x as usize * 4;
        out.extend_from_slice(&pixels[start..start + w as usize * 4]);
    }
    Ok((w, h, out))
}

#[allow(clippy::needless_pass_by_value)] // by value so it works point-free in `.map_err(png_err)`
fn png_err(error: png::DecodingError) -> PlatformError {
    PlatformError::OperationFailed { operation: "Wayland screenshot", details: Some(format!("PNG decode: {error}")) }
}

/// Decode standard base64 (RFC 4648). Mirrors the compositor's self-contained
/// encoder so neither side needs a base64 crate; tolerant of padding/whitespace.
fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    fn sextet(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some(u32::from(c - b'A')),
            b'a'..=b'z' => Some(u32::from(c - b'a') + 26),
            b'0'..=b'9' => Some(u32::from(c - b'0') + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    for &c in input.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let value = sextet(c).ok_or_else(|| format!("invalid base64 byte {c:#x}"))?;
        acc = (acc << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            #[allow(clippy::cast_possible_truncation)]
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_decode_round_trips_known_values() {
        assert_eq!(base64_decode("aGVsbG8=").unwrap(), b"hello");
        assert_eq!(base64_decode("").unwrap(), b"");
        assert_eq!(base64_decode("Zm9vYmFy").unwrap(), b"foobar");
        // whitespace is ignored
        assert_eq!(base64_decode("aGVs\nbG8=").unwrap(), b"hello");
    }

    #[test]
    fn base64_decode_rejects_invalid() {
        assert!(base64_decode("****").is_err());
    }

    #[test]
    fn crop_extracts_the_scaled_region() {
        // 4x2 RGBA frame, scale 1.0; crop the right 2x1 starting at (2,0).
        let mut pixels = vec![0u8; 4 * 2 * 4];
        // mark pixel (2,0) red and (3,0) green so we can recognise the crop
        let px = |x: usize, y: usize| (y * 4 + x) * 4;
        pixels[px(2, 0)] = 255;
        pixels[px(3, 0) + 1] = 255;
        let (w, h, out) = crop_rgba8(&pixels, 4, 2, Rect::new(2.0, 0.0, 2.0, 1.0), 1.0).unwrap();
        assert_eq!((w, h), (2, 1));
        assert_eq!(out.len(), 2 * 4); // 2 px wide, 1 px high, 4 bytes/px
        assert_eq!(out[0], 255); // first cropped pixel is the red one
        assert_eq!(out[4 + 1], 255); // second cropped pixel is the green one
    }

    #[test]
    fn capture_reports_unavailable_without_our_compositor() {
        // No Wayland compositor is detected in a unit test (compositor_type() == None), so the
        // provider must report the capability as unavailable instead of poking a control socket.
        let request = ScreenshotRequest::with_region(Rect::new(0.0, 0.0, 8.0, 8.0));
        let err = WaylandScreenshot.capture(&request).unwrap_err();
        assert!(matches!(err, PlatformError::CapabilityUnavailable { capability: "Wayland screenshot", .. }));
    }
}
