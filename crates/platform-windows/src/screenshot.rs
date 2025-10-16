use platynui_core::platform::{
    PixelFormat, PlatformError, PlatformErrorKind, Screenshot, ScreenshotProvider, ScreenshotRequest,
    desktop_info_providers,
};
use platynui_core::register_screenshot_provider;
use platynui_core::types::Rect;
use std::mem::size_of;
// no HWND import needed with Option<HWND> calls
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BitBlt, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDC, HBITMAP, HDC, ReleaseDC, SRCCOPY, SelectObject,
};

static WINDOWS_SCREENSHOT: WindowsScreenshotProvider = WindowsScreenshotProvider;

register_screenshot_provider!(&WINDOWS_SCREENSHOT);

pub struct WindowsScreenshotProvider;

impl ScreenshotProvider for WindowsScreenshotProvider {
    fn capture(&self, request: &ScreenshotRequest) -> Result<Screenshot, PlatformError> {
        let desktop = desktop_bounds().ok_or_else(|| {
            PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "desktop bounds unavailable")
        })?;

        // Determine requested capture region and clamp to desktop
        let requested = request.region.unwrap_or(desktop);
        let region = intersect_rect(&requested, &desktop).ok_or_else(|| {
            PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "capture region outside desktop")
        })?;

        let left = region.x().floor() as i32;
        let top = region.y().floor() as i32;
        let width = region.width().ceil().max(1.0) as i32;
        let height = region.height().ceil().max(1.0) as i32;

        unsafe {
            let screen_dc: HDC = GetDC(None);
            if screen_dc.0.is_null() {
                return Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "GetDC(NULL) failed"));
            }
            let mem_dc: HDC = CreateCompatibleDC(Some(screen_dc));
            if mem_dc.0.is_null() {
                let _ = ReleaseDC(None, screen_dc);
                return Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "CreateCompatibleDC failed"));
            }

            let info: BITMAPINFO = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height, // top-down DIB
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                ..Default::default()
            };

            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let bitmap: HBITMAP = match CreateDIBSection(Some(mem_dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
                Ok(bmp) => bmp,
                Err(e) => {
                    let _ = DeleteDC(mem_dc);
                    let _ = ReleaseDC(None, screen_dc);
                    return Err(PlatformError::new(
                        PlatformErrorKind::CapabilityUnavailable,
                        format!("CreateDIBSection failed: {e:?}"),
                    ));
                }
            };
            let old = SelectObject(mem_dc, bitmap.into());

            // Copy from screen DC into memory DC
            let res = BitBlt(mem_dc, 0, 0, width, height, Some(screen_dc), left, top, SRCCOPY);
            if res.is_err() {
                let _ = SelectObject(mem_dc, old);
                let _ = DeleteObject(bitmap.into());
                let _ = DeleteDC(mem_dc);
                let _ = ReleaseDC(None, screen_dc);
                return Err(PlatformError::new(PlatformErrorKind::CapabilityUnavailable, "BitBlt failed"));
            }

            // Copy pixels from DIBSection memory
            let byte_count = (width as usize) * (height as usize) * 4;
            let mut pixels = vec![0u8; byte_count];
            std::ptr::copy_nonoverlapping(bits as *const u8, pixels.as_mut_ptr(), byte_count);

            let _ = SelectObject(mem_dc, old);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(None, screen_dc);

            Ok(Screenshot::new(width as u32, height as u32, PixelFormat::Bgra8, pixels))
        }
    }
}

fn intersect_rect(a: &Rect, b: &Rect) -> Option<Rect> {
    let left = a.x().max(b.x());
    let top = a.y().max(b.y());
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    let w = right - left;
    let h = bottom - top;
    if w > 0.0 && h > 0.0 { Some(Rect::new(left, top, w, h)) } else { None }
}

fn desktop_bounds() -> Option<Rect> {
    desktop_info_providers().next().and_then(|p| p.desktop_info().ok()).map(|info| info.bounds)
}
