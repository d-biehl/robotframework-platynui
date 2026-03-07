#![allow(dead_code)]

use platynui_core::types::{Point, Rect};

/// Convert window-relative coordinates to absolute screen coordinates by
/// combining the element's position within its window (from AT-SPI
/// `GetExtents(WINDOW)`) with the window's screen position (from compositor
/// IPC or best-effort fallback).
///
/// Under Wayland, AT-SPI `GetExtents(SCREEN)` returns `(0, 0)` for the
/// window origin — Wayland clients do not know their absolute screen position.
/// This module provides the translation layer.
pub fn window_relative_to_screen(element_in_window: Point, window_origin: Point) -> Point {
    Point::new(element_in_window.x() + window_origin.x(), element_in_window.y() + window_origin.y())
}

/// Convert a window-relative bounding rect to absolute screen coordinates.
pub fn rect_window_to_screen(element_rect: Rect, window_origin: Point) -> Rect {
    Rect::new(
        element_rect.x() + window_origin.x(),
        element_rect.y() + window_origin.y(),
        element_rect.width(),
        element_rect.height(),
    )
}

/// Zero-origin fallback for when window screen position is unknown.
///
/// Used on Mutter/GNOME where no compositor IPC exposes window geometry.
/// The resulting coordinates are window-relative (the best we can do).
pub const UNKNOWN_WINDOW_ORIGIN: Point = Point::new(0.0, 0.0);
