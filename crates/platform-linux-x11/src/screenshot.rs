use crate::x11util::{connection, root_window_from};
use platynui_core::platform::{
    PixelFormat, PlatformError, PlatformErrorKind, Screenshot, ScreenshotProvider, ScreenshotRequest,
};
use platynui_core::register_screenshot_provider;
use x11rb::protocol::xproto::{ConnectionExt as _, ImageFormat};

pub struct LinuxScreenshot;

impl ScreenshotProvider for LinuxScreenshot {
    fn capture(&self, request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
        let guard = connection()?;
        let root = root_window_from(&guard);

        // Determine region: entire root if none specified
        let (x, y, w, h) = if let Some(r) = request.region {
            (r.x().round() as i16, r.y().round() as i16, r.width().round() as u16, r.height().round() as u16)
        } else {
            let geom = guard.conn.get_geometry(root).map_err(to_pf)?.reply().map_err(to_pf)?;
            (0, 0, geom.width, geom.height)
        };

        let reply = guard
            .conn
            .get_image(ImageFormat::Z_PIXMAP, root, x, y, w, h, !0)
            .map_err(to_pf)?
            .reply()
            .map_err(to_pf)?;

        // Heuristic: Use BGRA8 (many X11 servers deliver BGRX/BGRA in ZPixmap 32bpp)
        let depth = reply.depth;
        if depth != 24 && depth != 32 {
            return Err(PlatformError::new(
                PlatformErrorKind::UnsupportedPlatform,
                format!("unsupported image depth {depth}"),
            ));
        }
        let mut pixels = reply.data;
        // Ensure alpha is fully opaque to avoid artifacts from undefined padding/alpha bits in 24/32bpp.
        for chunk in pixels.chunks_exact_mut(4) {
            chunk[3] = 0xFF;
        }
        let format = PixelFormat::Bgra8;
        Ok(Screenshot::new(u32::from(w), u32::from(h), format, pixels))
    }
}

fn to_pf<E: std::fmt::Display>(e: E) -> PlatformError {
    // Screenshot failures after connect are operational.
    PlatformError::new(PlatformErrorKind::OperationFailed, format!("x11: {e}"))
}

static SHOT: LinuxScreenshot = LinuxScreenshot;
register_screenshot_provider!(&SHOT);
