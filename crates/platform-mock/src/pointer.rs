use platynui_core::platform::{PlatformError, PointerButton, PointerDevice, ScrollDelta};
use platynui_core::types::{Point, Size};
use std::sync::Mutex;
use std::time::Duration;

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
    const fn new() -> Self {
        Self { position: (0.0, 0.0), log: Vec::new() }
    }

    fn push(&mut self, entry: PointerLogEntry) {
        self.log.push(entry);
    }

    fn point(&self) -> Point {
        Point::new(self.position.0, self.position.1)
    }
}

pub struct MockPointerDevice {
    state: Mutex<PointerState>,
}

impl MockPointerDevice {
    const fn new() -> Self {
        Self { state: Mutex::new(PointerState::new()) }
    }
}

impl PointerDevice for MockPointerDevice {
    fn position(&self) -> Result<Point, PlatformError> {
        Ok(self.state.lock().unwrap().point())
    }

    fn move_to(&self, point: Point) -> Result<(), PlatformError> {
        let mut state = self.state.lock().unwrap();
        state.position = (point.x(), point.y());
        state.push(PointerLogEntry::Move(point));
        println!("mock-pointer: move to ({:.1}, {:.1})", point.x(), point.y());
        Ok(())
    }

    fn press(&self, button: PointerButton) -> Result<(), PlatformError> {
        self.state.lock().unwrap().push(PointerLogEntry::Press(button));
        println!("mock-pointer: press {button:?}");
        Ok(())
    }

    fn release(&self, button: PointerButton) -> Result<(), PlatformError> {
        self.state.lock().unwrap().push(PointerLogEntry::Release(button));
        println!("mock-pointer: release {button:?}");
        Ok(())
    }

    fn scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
        self.state.lock().unwrap().push(PointerLogEntry::Scroll(delta));
        println!("mock-pointer: scroll (h={:.1}, v={:.1})", delta.horizontal, delta.vertical);
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
    let mut state = MOCK_POINTER.state.lock().unwrap();
    *state = PointerState::new();
}

/// Returns the recorded pointer log since the last reset and clears the buffer.
pub fn take_pointer_log() -> Vec<PointerLogEntry> {
    let mut state = MOCK_POINTER.state.lock().unwrap();
    let entries = state.log.clone();
    state.log.clear();
    entries
}

// Expose device reference for explicit injection in tests/integration code.
// Test helpers for exposing internal state

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::platform::pointer_devices;

    #[test]
    fn pointer_not_auto_registered() {
        let providers: Vec<_> = pointer_devices().collect();
        // Mock pointer should NOT be in the registry
        let mock_in_registry =
            providers.iter().any(|device| std::ptr::eq(*device, &MOCK_POINTER as &dyn PointerDevice));
        assert!(!mock_in_registry, "Mock pointer should not be auto-registered");
    }

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
