use platynui_core::platform::{PlatformError, WindowId, WindowManager};
use platynui_core::types::{Point, Rect, Size};
use platynui_core::ui::UiNode;
use std::sync::Mutex;

pub static MOCK_WINDOW_MANAGER: MockWindowManager = MockWindowManager::new();

/// Log entry for window manager operations.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowManagerLogEntry {
    ResolveWindow,
    Bounds(WindowId),
    IsActive(WindowId),
    Activate(WindowId),
    Close(WindowId),
    Minimize(WindowId),
    Maximize(WindowId),
    Restore(WindowId),
    MoveTo(WindowId, Point),
    Resize(WindowId, Size),
}

#[derive(Debug)]
pub struct MockWindowManager {
    log: Mutex<Vec<WindowManagerLogEntry>>,
}

impl MockWindowManager {
    const fn new() -> Self {
        Self { log: Mutex::new(Vec::new()) }
    }

    fn record(&self, entry: WindowManagerLogEntry) {
        let mut log = self.log.lock().expect("window manager log poisoned");
        log.push(entry);
    }
}

impl WindowManager for MockWindowManager {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn resolve_window(&self, _node: &dyn UiNode) -> Result<WindowId, PlatformError> {
        self.record(WindowManagerLogEntry::ResolveWindow);
        Ok(WindowId::new(1))
    }

    fn bounds(&self, id: WindowId) -> Result<Rect, PlatformError> {
        self.record(WindowManagerLogEntry::Bounds(id));
        Ok(Rect::new(0.0, 0.0, 800.0, 600.0))
    }

    fn is_active(&self, id: WindowId) -> Result<bool, PlatformError> {
        self.record(WindowManagerLogEntry::IsActive(id));
        Ok(true)
    }

    fn activate(&self, id: WindowId) -> Result<(), PlatformError> {
        self.record(WindowManagerLogEntry::Activate(id));
        Ok(())
    }

    fn close(&self, id: WindowId) -> Result<(), PlatformError> {
        self.record(WindowManagerLogEntry::Close(id));
        Ok(())
    }

    fn minimize(&self, id: WindowId) -> Result<(), PlatformError> {
        self.record(WindowManagerLogEntry::Minimize(id));
        Ok(())
    }

    fn maximize(&self, id: WindowId) -> Result<(), PlatformError> {
        self.record(WindowManagerLogEntry::Maximize(id));
        Ok(())
    }

    fn restore(&self, id: WindowId) -> Result<(), PlatformError> {
        self.record(WindowManagerLogEntry::Restore(id));
        Ok(())
    }

    fn move_to(&self, id: WindowId, position: Point) -> Result<(), PlatformError> {
        self.record(WindowManagerLogEntry::MoveTo(id, position));
        Ok(())
    }

    fn resize(&self, id: WindowId, size: Size) -> Result<(), PlatformError> {
        self.record(WindowManagerLogEntry::Resize(id, size));
        Ok(())
    }
}

pub fn take_window_manager_log() -> Vec<WindowManagerLogEntry> {
    let mut log = MOCK_WINDOW_MANAGER.log.lock().expect("window manager log poisoned");
    log.drain(..).collect()
}

pub fn reset_window_manager_state() {
    MOCK_WINDOW_MANAGER.log.lock().expect("window manager log poisoned").clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use platynui_core::platform::window_managers;
    use rstest::rstest;
    use serial_test::serial;

    #[rstest]
    #[serial]
    fn window_manager_not_auto_registered() {
        reset_window_manager_state();
        let registered: Vec<_> = window_managers().collect();
        let mock_in_registry =
            registered.iter().any(|wm| std::ptr::eq(*wm, &MOCK_WINDOW_MANAGER as &dyn WindowManager));
        assert!(!mock_in_registry, "Mock window manager should not be auto-registered");
    }

    #[rstest]
    #[serial]
    fn mock_activate_records_entry() {
        reset_window_manager_state();
        let id = WindowId::new(42);
        MOCK_WINDOW_MANAGER.activate(id).unwrap();
        let log = take_window_manager_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0], WindowManagerLogEntry::Activate(id));
    }
}
