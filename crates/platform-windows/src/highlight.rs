
use std::mem::size_of;
use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;

use platynui_core::platform::{
    HighlightProvider, HighlightRequest, PlatformError, PlatformErrorKind, desktop_info_providers,
};
use platynui_core::register_highlight_provider;
use platynui_core::types::Rect;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, BLENDFUNCTION, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS,
    DeleteDC, DeleteObject, GetDC, HBITMAP, HDC, ReleaseDC, SelectObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow, MA_NOACTIVATE,
    RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE, ShowWindow, ULW_ALPHA, UpdateLayeredWindow,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_MOUSEACTIVATE, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::PCWSTR;

// Public registration ----------------------------------------------------------------------------

static WINDOWS_HIGHLIGHT: WindowsHighlightProvider = WindowsHighlightProvider;

register_highlight_provider!(&WINDOWS_HIGHLIGHT);

pub struct WindowsHighlightProvider;

impl HighlightProvider for WindowsHighlightProvider {
    fn highlight(&self, requests: &[HighlightRequest]) -> Result<(), PlatformError> {
        if requests.is_empty() {
            return self.clear();
        }

        let duration = min_duration(requests);
        let rects: Vec<Rect> = requests.iter().map(|r| r.bounds).collect();
        OverlayController::global().show(&rects, duration)
    }

    fn clear(&self) -> Result<(), PlatformError> {
        OverlayController::global().clear()
    }
}

fn min_duration(requests: &[HighlightRequest]) -> Option<Duration> {
    requests.iter().filter_map(|r| r.duration).min()
}

// Overlay controller -----------------------------------------------------------------------------

struct OverlayController {
    tx: Sender<Command>,
}

impl OverlayController {
    fn global() -> &'static Self {
        static CTRL: OnceLock<OverlayController> = OnceLock::new();
        CTRL.get_or_init(|| OverlayThread::spawn())
    }

    fn show(&self, rects: &[Rect], duration: Option<Duration>) -> Result<(), PlatformError> {
        self.tx.send(Command::Show { rects: rects.to_vec(), duration }).map_err(|_| {
            PlatformError::new(PlatformErrorKind::InitializationFailed, "highlight thread stopped")
        })
    }

    fn clear(&self) -> Result<(), PlatformError> {
        self.tx.send(Command::Clear).map_err(|_| {
            PlatformError::new(PlatformErrorKind::InitializationFailed, "highlight thread stopped")
        })
    }
}

struct OverlayThread;

impl OverlayThread {
    fn spawn() -> OverlayController {
        let (tx, rx) = std::sync::mpsc::channel::<Command>();
        let tx_clone = tx.clone();
        thread::spawn(move || Self::run(rx, tx_clone));
        OverlayController { tx }
    }

    fn run(rx: Receiver<Command>, tx: Sender<Command>) {
        // Ensure a basic window class exists
        let class_name: Vec<u16> = "PlatynUI_Highlight\0".encode_utf16().collect();
        unsafe {
            extern "system" fn wndproc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
                if msg == WM_MOUSEACTIVATE {
                    return LRESULT(MA_NOACTIVATE as isize);
                }
                unsafe { DefWindowProcW(hwnd, msg, w, l) }
            }
            let wc = WNDCLASSW {
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(wndproc),
                hInstance: HINSTANCE(std::ptr::null_mut()),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            let _ = RegisterClassW(&wc);
        }

        let mut overlay = Overlay::new();
        let mut generation: u64 = 0;

        while let Ok(cmd) = rx.recv() {
            match cmd {
                Command::Show { rects, duration } => {
                    generation = generation.wrapping_add(1);
                    overlay.show(&rects);
                    if let Some(d) = duration {
                        let gen_id = generation;
                        let tx2 = tx.clone();
                        thread::spawn(move || {
                            thread::sleep(d);
                            let _ = tx2.send(Command::ClearIfGeneration(gen_id));
                        });
                    }
                }
                Command::Clear => overlay.clear(),
                Command::ClearIfGeneration(expected) => {
                    if generation == expected {
                        overlay.clear();
                    }
                }
            }
        }
    }
}

// Extend Command with a generation-aware clear
enum Command {
    Show { rects: Vec<Rect>, duration: Option<Duration> },
    Clear,
    ClearIfGeneration(u64),
}

// Overlay window + drawing -----------------------------------------------------------------------

struct Overlay {
    hwnd: Option<HWND>,
}

impl Overlay {
    fn new() -> Self {
        Self { hwnd: None }
    }

    fn ensure_window(&mut self) -> HWND {
        if let Some(h) = self.hwnd {
            return h;
        }
        unsafe {
            let class_name: Vec<u16> = "PlatynUI_Highlight\0".encode_utf16().collect();
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(
                    WS_EX_LAYERED.0
                        | WS_EX_TRANSPARENT.0
                        | WS_EX_TOOLWINDOW.0
                        | WS_EX_TOPMOST.0
                        | WS_EX_NOACTIVATE.0,
                ),
                PCWSTR(class_name.as_ptr()),
                PCWSTR(class_name.as_ptr()),
                WINDOW_STYLE(WS_POPUP.0),
                0,
                0,
                0,
                0,
                None,
                None,
                None,
                None,
            )
            .expect("CreateWindowExW");
            self.hwnd = Some(hwnd);
            hwnd
        }
    }

    fn show(&mut self, rects: &[Rect]) {
        if rects.is_empty() {
            self.clear();
            return;
        }

        const FRAME_THICKNESS: i32 = 3; // pixels
        const FRAME_GAP: i32 = 1; // 1px gap between target and frame

        // Expand each requested rect so the frame is drawn AROUND the target
        // area with a 1px gap instead of covering the target itself.
        let expanded: Vec<Rect> =
            rects.iter().map(|r| expand_rect(r, FRAME_THICKNESS, FRAME_GAP)).collect();

        // Clamp to desktop bounds to avoid drawing off-screen.
        let desktop_bounds = desktop_bounds().unwrap_or_else(|| union_rect(&expanded));
        let mut clamped: Vec<Rect> = Vec::new();
        for r in &expanded {
            if let Some(i) = intersect_rect(r, &desktop_bounds) {
                clamped.push(i);
            }
        }
        if clamped.is_empty() {
            self.clear();
            return;
        }

        let union = union_rect(&clamped);
        let hwnd = self.ensure_window();
        let width = union.width().max(1.0).round() as i32;
        let height = union.height().max(1.0).round() as i32;
        unsafe {
            // Acquire screen DC and guard against failures
            let screen_dc: HDC = GetDC(None);
            if screen_dc.0.is_null() {
                return; // Nothing we can do; avoid leaking handles
            }

            // Create a compatible memory DC; on failure, release screen DC and return
            let mem_dc: HDC = CreateCompatibleDC(Some(screen_dc));
            if mem_dc.0.is_null() {
                let _ = ReleaseDC(None, screen_dc);
                return;
            }

            // Create a top-down 32bpp DIB section to draw the overlay pixels into
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let mut bmi = BITMAPINFO::new(width as i32, height as i32);
            let bitmap: HBITMAP = match CreateDIBSection(
                Some(mem_dc),
                &mut bmi.inner,
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            ) {
                Ok(bmp) => bmp,
                Err(_) => {
                    let _ = DeleteDC(mem_dc);
                    let _ = ReleaseDC(None, screen_dc);
                    return;
                }
            };

            let old = SelectObject(mem_dc, bitmap.into());

            // Fill fully transparent
            let buf_size = (width as usize) * (height as usize) * 4;
            let slice = std::slice::from_raw_parts_mut(bits as *mut u8, buf_size);
            for b in slice.iter_mut() {
                *b = 0;
            }

            // Draw frames around each clamped rect. Use red for better visibility.
            let color = Rgba { r: 255, g: 0, b: 0, a: 230 };
            for (idx, r) in clamped.iter().enumerate() {
                let expanded = &expanded[idx];
                let styles = edge_styles(expanded, r);
                draw_frame(
                    slice,
                    width as usize,
                    height as usize,
                    r,
                    &union,
                    FRAME_THICKNESS,
                    color,
                    styles,
                );
            }

            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let dst = POINT { x: union.x().round() as i32, y: union.y().round() as i32 };
            let size = SIZE { cx: width, cy: height };
            let src = POINT { x: 0, y: 0 };
            let _ = UpdateLayeredWindow(
                hwnd,
                Some(screen_dc),
                Some(&dst),
                Some(&size),
                Some(mem_dc),
                Some(&src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );

            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);

            let _ = SelectObject(mem_dc, old);
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(None, screen_dc);
        }
    }

    fn clear(&mut self) {
        if let Some(hwnd) = self.hwnd.take() {
            unsafe {
                let _ = ShowWindow(hwnd, SW_HIDE);
                let _ = DestroyWindow(hwnd);
            }
        }
    }
}

fn union_rect(rects: &[Rect]) -> Rect {
    let mut left = rects[0].x();
    let mut top = rects[0].y();
    let mut right = rects[0].right();
    let mut bottom = rects[0].bottom();
    for r in &rects[1..] {
        left = left.min(r.x());
        top = top.min(r.y());
        right = right.max(r.right());
        bottom = bottom.max(r.bottom());
    }
    Rect::new(left, top, right - left, bottom - top)
}

fn expand_rect(rect: &Rect, thickness: i32, gap: i32) -> Rect {
    let pad = (thickness + gap) as f64;
    Rect::new(rect.x() - pad, rect.y() - pad, rect.width() + 2.0 * pad, rect.height() + 2.0 * pad)
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
    // Use the first registered desktop info provider (Windows supplies one).
    desktop_info_providers().next().and_then(|p| p.desktop_info().ok()).map(|info| info.bounds)
}

#[derive(Clone, Copy)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LineStyle {
    Solid,
    Dashed,
}

struct EdgeStyles {
    top: LineStyle,
    right: LineStyle,
    bottom: LineStyle,
    left: LineStyle,
}

fn edge_styles(expanded: &Rect, clamped: &Rect) -> EdgeStyles {
    let left_clipped = clamped.x() > expanded.x();
    let right_clipped = clamped.right() < expanded.right();
    let top_clipped = clamped.y() > expanded.y();
    let bottom_clipped = clamped.bottom() < expanded.bottom();
    EdgeStyles {
        top: if top_clipped { LineStyle::Dashed } else { LineStyle::Solid },
        right: if right_clipped { LineStyle::Dashed } else { LineStyle::Solid },
        bottom: if bottom_clipped { LineStyle::Dashed } else { LineStyle::Solid },
        left: if left_clipped { LineStyle::Dashed } else { LineStyle::Solid },
    }
}

fn draw_frame(
    buf: &mut [u8],
    width: usize,
    height: usize,
    rect: &Rect,
    origin: &Rect,
    thickness: i32,
    color: Rgba,
    styles: EdgeStyles,
) {
    let x0 = (rect.x() - origin.x()).round() as i32;
    let y0 = (rect.y() - origin.y()).round() as i32;
    let x1 = (x0 as f64 + rect.width().round()) as i32 - 1;
    let y1 = (y0 as f64 + rect.height().round()) as i32 - 1;
    let t = thickness.max(1);

    // Top
    draw_hline(buf, width, height, x0, x1, y0, t, color, styles.top);
    // Bottom
    draw_hline(buf, width, height, x0, x1, y1 - (t - 1), t, color, styles.bottom);
    // Left
    draw_vline(buf, width, height, x0, y0, y1, t, color, styles.left);
    // Right
    draw_vline(buf, width, height, x1 - (t - 1), y0, y1, t, color, styles.right);
}

fn draw_hline(
    buf: &mut [u8],
    width: usize,
    height: usize,
    x0: i32,
    x1: i32,
    y: i32,
    thickness: i32,
    color: Rgba,
    style: LineStyle,
) {
    if thickness <= 0 {
        return;
    }
    let minx = x0.min(x1).max(0);
    let maxx = x0.max(x1).min(width as i32 - 1);
    let starty = y.max(0);
    let endy = (y + thickness - 1).min(height as i32 - 1);
    if starty > endy || minx > maxx {
        return;
    }
    let dash_on = 6;
    let dash_off = 4;
    let cycle = dash_on + dash_off;
    for yy in starty..=endy {
        let mut x = minx;
        while x <= maxx {
            let draw_this = match style {
                LineStyle::Solid => true,
                LineStyle::Dashed => ((x - minx) % cycle) < dash_on,
            };
            if draw_this {
                let idx = (yy as usize * width + x as usize) * 4;
                blend_pixel(buf, idx, color);
            }
            x += 1;
        }
    }
}

fn draw_vline(
    buf: &mut [u8],
    width: usize,
    height: usize,
    x: i32,
    y0: i32,
    y1: i32,
    thickness: i32,
    color: Rgba,
    style: LineStyle,
) {
    if thickness <= 0 {
        return;
    }
    let miny = y0.min(y1).max(0);
    let maxy = y0.max(y1).min(height as i32 - 1);
    let startx = x.max(0);
    let endx = (x + thickness - 1).min(width as i32 - 1);
    if startx > endx || miny > maxy {
        return;
    }
    let dash_on = 6;
    let dash_off = 4;
    let cycle = dash_on + dash_off;
    for xx in startx..=endx {
        let mut y = miny;
        while y <= maxy {
            let draw_this = match style {
                LineStyle::Solid => true,
                LineStyle::Dashed => ((y - miny) % cycle) < dash_on,
            };
            if draw_this {
                let idx = (y as usize * width + xx as usize) * 4;
                blend_pixel(buf, idx, color);
            }
            y += 1;
        }
    }
}

fn blend_pixel(buf: &mut [u8], idx: usize, color: Rgba) {
    let a = color.a as u16;
    let r = (color.r as u16 * a / 255) as u8;
    let g = (color.g as u16 * a / 255) as u8;
    let b = (color.b as u16 * a / 255) as u8;
    buf[idx + 0] = b; // BGRA
    buf[idx + 1] = g;
    buf[idx + 2] = r;
    buf[idx + 3] = color.a;
}

// Minimal BITMAPINFO for CreateDIBSection --------------------------------------------------------

#[repr(C)]
struct BITMAPINFO {
    inner: windows::Win32::Graphics::Gdi::BITMAPINFO,
}

impl BITMAPINFO {
    fn new(width: i32, height: i32) -> Self {
        use windows::Win32::Graphics::Gdi::{BI_RGB, BITMAPINFO, BITMAPINFOHEADER};
        let mut info = BITMAPINFO::default();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // top-down DIB
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0 as u32,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        Self { inner: info }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::types::Rect;

    #[test]
    fn union_rect_computes_expected_bounds() {
        let r = union_rect(&[Rect::new(10.0, 10.0, 10.0, 10.0), Rect::new(15.0, 8.0, 5.0, 20.0)]);
        assert_eq!(r.x(), 10.0);
        assert_eq!(r.y(), 8.0);
        assert_eq!(r.right(), 20.0);
        assert_eq!(r.bottom(), 28.0);
    }
}
