use platynui_core::platform::{PlatformError, PointerButton, PointerDevice, ScrollDelta};
use platynui_core::types::{Point, Size};
use std::sync::Mutex;
use std::time::Duration;
use tracing::debug;

#[derive(Clone, Debug, PartialEq)]
pub enum PointerLogEntry {
    Move(Point),
    Press(PointerButton),
    Release(PointerButton),
    Scroll(ScrollDelta),
}

struct PointerState {
    position: (f64, f64),
    log: Vec<PointerLogEntry>,
}

impl PointerState {
    pub(crate) const fn new() -> Self {
        Self { position: (0.0, 0.0), log: Vec::new() }
    }

    fn push(&mut self, entry: PointerLogEntry) {
        self.log.push(entry);
    }

    fn point(&self) -> Point {
        Point::new(self.position.0, self.position.1)
    }
}

pub struct MockPointerDevice;

/// Shared in-memory pointer state. Every `MockPointerDevice` handle — the
/// `MOCK_POINTER` static and any built by `create_mock_bundle` — observes this
/// one state, so the `take_*`/`reset_*` helpers see calls routed through a
/// per-runtime bundle. (The mock deliberately shares state for observability;
/// true per-runtime isolation is a property of the real backends, not the mock.)
static POINTER_STATE: Mutex<PointerState> = Mutex::new(PointerState::new());

impl MockPointerDevice {
    pub(crate) const fn new() -> Self {
        Self
    }
}

impl PointerDevice for MockPointerDevice {
    fn position(&self) -> Result<Point, PlatformError> {
        Ok(POINTER_STATE.lock().unwrap().point())
    }

    fn move_to(&self, point: Point) -> Result<(), PlatformError> {
        let mut state = POINTER_STATE.lock().unwrap();
        state.position = (point.x(), point.y());
        state.push(PointerLogEntry::Move(point));
        debug!(x = point.x(), y = point.y(), "mock-pointer: move");
        Ok(())
    }

    fn press(&self, button: PointerButton) -> Result<(), PlatformError> {
        POINTER_STATE.lock().unwrap().push(PointerLogEntry::Press(button));
        debug!(?button, "mock-pointer: press");
        Ok(())
    }

    fn release(&self, button: PointerButton) -> Result<(), PlatformError> {
        POINTER_STATE.lock().unwrap().push(PointerLogEntry::Release(button));
        debug!(?button, "mock-pointer: release");
        Ok(())
    }

    fn scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
        POINTER_STATE.lock().unwrap().push(PointerLogEntry::Scroll(delta));
        debug!(h = delta.horizontal, v = delta.vertical, "mock-pointer: scroll");
        Ok(())
    }

    fn double_click_time(&self) -> Result<Option<Duration>, PlatformError> {
        Ok(Some(Duration::from_millis(400)))
    }

    fn double_click_size(&self) -> Result<Option<Size>, PlatformError> {
        Ok(Some(Size::new(4.0, 4.0)))
    }
}

pub static MOCK_POINTER: MockPointerDevice = MockPointerDevice::new();

// Mock pointer device does NOT auto-register - only available via explicit handles

/// Clears the recorded pointer log and resets the cursor position to the origin.
pub fn reset_pointer_state() {
    *POINTER_STATE.lock().unwrap() = PointerState::new();
}

/// Returns the recorded pointer log since the last reset and clears the buffer.
pub fn take_pointer_log() -> Vec<PointerLogEntry> {
    let mut state = POINTER_STATE.lock().unwrap();
    let entries = state.log.clone();
    state.log.clear();
    entries
}

// Expose device reference for explicit injection in tests/integration code.
// Test helpers for exposing internal state

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_log_records_events() {
        reset_pointer_state();
        // Use direct reference to mock pointer
        let device = &MOCK_POINTER;

        device.move_to(Point::new(10.0, 20.0)).unwrap();
        device.press(PointerButton::Left).unwrap();
        device.release(PointerButton::Left).unwrap();
        device.scroll(ScrollDelta::new(0.0, -120.0)).unwrap();

        let log = take_pointer_log();
        assert!(matches!(log[0], PointerLogEntry::Move(point) if point == Point::new(10.0, 20.0)));
        assert!(matches!(log[1], PointerLogEntry::Press(PointerButton::Left)));
        assert!(matches!(log[2], PointerLogEntry::Release(PointerButton::Left)));
        assert!(matches!(log[3], PointerLogEntry::Scroll(delta) if delta.vertical == -120.0));
    }
}
