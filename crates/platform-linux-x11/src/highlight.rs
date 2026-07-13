use crate::x11util::connect_raw;
use platynui_core::platform::{HighlightProvider, HighlightRequest, PlatformError};
use platynui_core::types::Rect;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::shape::{self, ConnectionExt as _};
use x11rb::protocol::xproto::{self as x, ConnectionExt as XProtoExt, Rectangle, Window};

/// Per-runtime X11 highlight overlay.
///
/// Owns a background thread (through [`OverlayController`]) that draws the
/// overlay windows on its own dedicated X11 connection. The thread's display
/// is supplied at construction (rather than read from `$DISPLAY`) so the
/// overlay binds to the same session as the runtime that created it. Dropping
/// the provider stops and joins the thread.
pub struct LinuxHighlightProvider {
    ctrl: OverlayController,
}

impl LinuxHighlightProvider {
    /// Spawn the overlay thread bound to `display`.
    pub fn new(display: String) -> Self {
        Self { ctrl: OverlayThread::spawn(display) }
    }
}

impl HighlightProvider for LinuxHighlightProvider {
    fn highlight(&self, request: &HighlightRequest) -> Result<(), PlatformError> {
        if request.rects.is_empty() {
            return self.clear();
        }
        self.ctrl.show(&request.rects, request.duration)
    }

    fn clear(&self) -> Result<(), PlatformError> {
        self.ctrl.clear()
    }
}

struct OverlayController {
    tx: Sender<Command>,
    handle: Option<JoinHandle<()>>,
}

impl OverlayController {
    fn show(&self, rects: &[Rect], duration: Option<Duration>) -> Result<(), PlatformError> {
        self.tx.send(Command::Show { rects: rects.to_vec(), duration }).map_err(|_| PlatformError::OperationFailed {
            operation: "x11 highlight thread",
            details: Some("stopped".into()),
        })
    }

    fn clear(&self) -> Result<(), PlatformError> {
        self.tx.send(Command::Clear).map_err(|_| PlatformError::OperationFailed {
            operation: "x11 highlight thread",
            details: Some("stopped".into()),
        })
    }
}

impl Drop for OverlayController {
    fn drop(&mut self) {
        // Close the command channel so the overlay thread's `recv` returns
        // `Disconnected`, destroys its windows, and exits. `self.tx` is a
        // struct field we cannot move out of here, so swap it for a throwaway
        // (already-disconnected) sender and drop the real one before joining.
        let (dead_tx, _) = std::sync::mpsc::channel::<Command>();
        drop(std::mem::replace(&mut self.tx, dead_tx));
        if let Some(handle) = self.handle.take() {
            tracing::debug!("highlight overlay controller dropped \u{2014} joining thread");
            let _ = handle.join();
        }
    }
}

enum Command {
    Show { rects: Vec<Rect>, duration: Option<Duration> },
    Clear,
}

struct OverlayThread;

impl OverlayThread {
    fn spawn(display: String) -> OverlayController {
        let (tx, rx) = std::sync::mpsc::channel::<Command>();
        let handle = thread::spawn(move || Self::run(rx, display));
        OverlayController { tx, handle: Some(handle) }
    }

    fn run(rx: Receiver<Command>, display: String) {
        // Separate X11 connection in this thread, bound to the runtime's display.
        let (conn, screen_num) = match connect_raw(&display) {
            Ok(v) => v,
            Err(err) => {
                tracing::warn!(%err, "highlight thread: X11 connect failed");
                return;
            }
        };
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;

        // Allocate red pixel in the default colormap (works on 16/24/32-bit visuals)
        let red_pixel = conn
            .alloc_color(screen.default_colormap, u16::MAX, 0, 0)
            .ok()
            .and_then(|c| c.reply().ok())
            .map(|r| r.pixel)
            .unwrap_or(0x00FF_0000); // fallback to 0xRRGGBB

        let mut deadline: Option<Instant> = None;
        let mut segments: Vec<Window> = Vec::new();
        loop {
            match rx.recv_timeout(Duration::from_millis(16)) {
                Ok(Command::Show { rects, duration }) => {
                    if rects.is_empty() {
                        // Treat as clear
                        for w in &segments {
                            let _ = conn.unmap_window(*w);
                        }
                        let _ = conn.flush();
                        deadline = None;
                        continue;
                    }
                    // Match Windows style: expand by 1px gap with 3px frame and clamp to desktop bounds if present
                    let expanded: Vec<Rect> = rects.iter().map(|r| expand_rect(r, 3, 1)).collect();
                    let bounds = root_bounds(&conn, root);
                    let mut clamped_pairs: Vec<(Rect, Rect)> = Vec::new();
                    for r in &expanded {
                        if let Some(b) = bounds.as_ref() {
                            if let Some(i) = intersect_rect(r, b) {
                                clamped_pairs.push((*r, i));
                            }
                        } else {
                            clamped_pairs.push((*r, *r));
                        }
                    }
                    if clamped_pairs.is_empty() {
                        for w in &segments {
                            let _ = conn.unmap_window(*w);
                        }
                        let _ = conn.flush();
                        deadline = None;
                        continue;
                    }

                    let frame_rects = frame_segments(&clamped_pairs, 3);

                    // Safety cap: avoid flooding the X server with hundreds
                    // of tiny override_redirect windows (e.g. dashed edges
                    // around full-screen bounds).  Fall back to solid edges
                    // when the segment count exceeds a reasonable limit.
                    const MAX_OVERLAY_WINDOWS: usize = 64;
                    let frame_rects = if frame_rects.len() > MAX_OVERLAY_WINDOWS {
                        // Re-generate with solid edges only (4 windows).
                        let solid_pairs: Vec<(Rect, Rect)> = clamped_pairs.iter().map(|(_, c)| (*c, *c)).collect();
                        frame_segments(&solid_pairs, 3)
                    } else {
                        frame_rects
                    };

                    for (idx, r) in frame_rects.iter().enumerate() {
                        let rect = Rectangle {
                            x: r.x().round() as i16,
                            y: r.y().round() as i16,
                            width: r.width().round().max(1.0) as u16,
                            height: r.height().round().max(1.0) as u16,
                        };

                        if idx >= segments.len()
                            && let Ok(win) = conn.generate_id()
                            && conn
                                .create_window(
                                    screen.root_depth,
                                    win,
                                    root,
                                    rect.x,
                                    rect.y,
                                    rect.width,
                                    rect.height,
                                    0,
                                    x::WindowClass::INPUT_OUTPUT,
                                    screen.root_visual,
                                    &x::CreateWindowAux::new()
                                        .background_pixel(red_pixel)
                                        .border_pixel(0)
                                        .override_redirect(1),
                                )
                                .is_ok()
                        {
                            // Make the overlay input-transparent: an empty SHAPE
                            // input region lets real clicks pass through to the UI
                            // beneath instead of hitting the transient highlight
                            // (parity with the Windows overlay's WS_EX_TRANSPARENT,
                            // and it keeps the live picker from ever resolving its
                            // own overlay). Best-effort — skip if SHAPE is absent.
                            let _ = conn.shape_rectangles(
                                shape::SO::SET,
                                shape::SK::INPUT,
                                x::ClipOrdering::UNSORTED,
                                win,
                                0,
                                0,
                                &[],
                            );
                            segments.push(win);
                        }

                        if let Some(&win) = segments.get(idx) {
                            let _ = conn.change_window_attributes(
                                win,
                                &x::ChangeWindowAttributesAux::new()
                                    .background_pixel(red_pixel)
                                    .border_pixel(0)
                                    .override_redirect(1),
                            );
                            let _ = conn.configure_window(
                                win,
                                &x::ConfigureWindowAux::new()
                                    .x(i32::from(rect.x))
                                    .y(i32::from(rect.y))
                                    .width(u32::from(rect.width))
                                    .height(u32::from(rect.height))
                                    .stack_mode(x::StackMode::ABOVE),
                            );
                            let _ = conn.map_window(win);
                        }
                    }

                    for w in segments.iter().skip(frame_rects.len()) {
                        let _ = conn.unmap_window(*w);
                    }
                    let _ = conn.flush();

                    deadline = duration.map(|d| Instant::now() + d);
                }
                Ok(Command::Clear) => {
                    for w in &segments {
                        let _ = conn.unmap_window(*w);
                    }
                    let _ = conn.flush();
                    deadline = None;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    if let Some(t) = deadline
                        && Instant::now() >= t
                    {
                        for w in &segments {
                            let _ = conn.unmap_window(*w);
                        }
                        let _ = conn.flush();
                        deadline = None;
                    }
                    // no messages; continue pumping
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        for w in segments {
            let _ = conn.destroy_window(w);
        }
        let _ = conn.flush();
    }
}

// Geometry helpers ------------------------------------------------------------------------------

/// Root-window bounds of the overlay thread's own connection, used to clamp
/// highlight rectangles. Returns `None` when the geometry cannot be read, in
/// which case rectangles are drawn unclamped.
fn root_bounds<C: Connection>(conn: &C, root: Window) -> Option<Rect> {
    let geom = conn.get_geometry(root).ok()?.reply().ok()?;
    Some(Rect::new(0.0, 0.0, f64::from(geom.width), f64::from(geom.height)))
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

fn expand_rect(r: &Rect, thickness: i32, gap: i32) -> Rect {
    let t = thickness as f64;
    let g = gap as f64;
    Rect::new(r.x() - (t + g), r.y() - (t + g), r.width() + 2.0 * (t + g), r.height() + 2.0 * (t + g))
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

fn frame_segments(pairs: &[(Rect, Rect)], thickness: i32) -> Vec<Rect> {
    const DASH_LEN: f64 = 8.0;
    const GAP_LEN: f64 = 4.0;

    let mut result = Vec::new();
    fn push_hline(result: &mut Vec<Rect>, x_start: f64, x_end: f64, y: f64, t: f64, style: LineStyle) {
        if style == LineStyle::Solid {
            result.push(Rect::new(x_start, y, x_end - x_start, t));
            return;
        }
        let mut x = x_start;
        while x < x_end {
            let len = (x_end - x).min(DASH_LEN);
            result.push(Rect::new(x, y, len, t));
            x += DASH_LEN + GAP_LEN;
        }
    }

    fn push_vline(result: &mut Vec<Rect>, y_start: f64, y_end: f64, x: f64, t: f64, style: LineStyle) {
        if style == LineStyle::Solid {
            result.push(Rect::new(x, y_start, t, y_end - y_start));
            return;
        }
        let mut y = y_start;
        while y < y_end {
            let len = (y_end - y).min(DASH_LEN);
            result.push(Rect::new(x, y, t, len));
            y += DASH_LEN + GAP_LEN;
        }
    }
    for (expanded, clamped) in pairs {
        let t = thickness as f64;
        let x0 = clamped.x();
        let y0 = clamped.y();
        let w = clamped.width();
        let h = clamped.height();
        if w <= 0.0 || h <= 0.0 {
            continue;
        }

        let styles = edge_styles(expanded, clamped);

        // Top
        push_hline(&mut result, x0, x0 + w, y0, t, styles.top);
        // Bottom
        push_hline(&mut result, x0, x0 + w, y0 + h - t, t, styles.bottom);
        // Left
        push_vline(&mut result, y0, y0 + h, x0, t, styles.left);
        // Right
        push_vline(&mut result, y0, y0 + h, x0 + w - t, t, styles.right);
    }
    result
}
