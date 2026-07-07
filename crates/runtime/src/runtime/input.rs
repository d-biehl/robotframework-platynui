use std::sync::Arc;

use platynui_core::platform::{
    KeyboardDevice, KeyboardError, KeyboardOverrides, PointerButton, PointerDevice, ScrollDelta,
};
use platynui_core::types::Point;

use crate::keyboard::{KeyboardEngine, KeyboardMode, resolve_profile as resolve_keyboard_profile};
use crate::keyboard_sequence::KeyboardSequence;
use crate::pointer::{PointerError, PointerOverrides, PointerProfile, PointerSettings};

use super::error::KeyboardActionError;
use super::{Runtime, default_sleep};

impl Runtime {
    pub fn pointer_settings(&self) -> PointerSettings {
        self.pointer_settings.lock().expect("pointer_settings lock poisoned").clone()
    }

    pub fn set_pointer_settings(&self, settings: PointerSettings) {
        {
            *self.pointer_settings.lock().expect("pointer_settings lock poisoned") = settings.clone();
        }
        if let Some(engine) = self.pointer_engine.lock().expect("pointer_engine lock poisoned").as_mut() {
            engine.set_settings(settings);
        }
    }

    pub fn pointer_profile(&self) -> PointerProfile {
        self.pointer_profile.lock().expect("pointer_profile lock poisoned").clone()
    }

    pub fn set_pointer_profile(&self, profile: PointerProfile) {
        {
            *self.pointer_profile.lock().expect("pointer_profile lock poisoned") = profile.clone();
        }
        if let Some(engine) = self.pointer_engine.lock().expect("pointer_engine lock poisoned").as_mut() {
            engine.set_profile(profile);
        }
    }

    pub fn pointer_position(&self) -> Result<Point, PointerError> {
        let device = self.pointer_device()?;
        Ok(device.position()?)
    }

    pub fn keyboard_profile(&self) -> platynui_core::platform::KeyboardProfile {
        self.keyboard_profile.lock().expect("keyboard_profile lock poisoned").clone()
    }

    pub fn set_keyboard_profile(&self, profile: platynui_core::platform::KeyboardProfile) {
        *self.keyboard_profile.lock().expect("keyboard_profile lock poisoned") = profile;
    }

    pub fn pointer_move_to(&self, point: Point, overrides: Option<PointerOverrides>) -> Result<Point, PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().map_err(|_| PointerError::Poisoned)?;
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        engine.move_to(point, overrides_ref)
    }

    pub fn pointer_click(
        &self,
        target: Option<Point>,
        button: Option<PointerButton>,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().map_err(|_| PointerError::Poisoned)?;
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        engine.click(target, button, overrides_ref)
    }

    pub fn pointer_multi_click(
        &self,
        target: Option<Point>,
        button: Option<PointerButton>,
        clicks: u32,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().map_err(|_| PointerError::Poisoned)?;
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        engine.multi_click(target, button, clicks, overrides_ref)
    }

    pub fn pointer_press(
        &self,
        target: Option<Point>,
        button: Option<PointerButton>,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().map_err(|_| PointerError::Poisoned)?;
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        let resolved_button = button.unwrap_or_else(|| engine.default_button());
        if let Some(point) = target {
            engine.move_to(point, overrides_ref)?;
        }
        engine.press(resolved_button, overrides_ref)
    }

    pub fn pointer_release(
        &self,
        target: Option<Point>,
        button: Option<PointerButton>,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().map_err(|_| PointerError::Poisoned)?;
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        let resolved_button = button.unwrap_or_else(|| engine.default_button());
        if let Some(point) = target {
            engine.move_to(point, overrides_ref)?;
        }
        engine.release(resolved_button, overrides_ref)
    }

    pub fn pointer_scroll(&self, delta: ScrollDelta, overrides: Option<PointerOverrides>) -> Result<(), PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().map_err(|_| PointerError::Poisoned)?;
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        engine.scroll(delta, overrides_ref)
    }

    pub fn pointer_drag(
        &self,
        start: Point,
        end: Point,
        button: Option<PointerButton>,
        overrides: Option<PointerOverrides>,
    ) -> Result<(), PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().map_err(|_| PointerError::Poisoned)?;
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        engine.drag(start, end, button, overrides_ref)
    }

    pub fn keyboard_press(
        &self,
        sequence: &str,
        overrides: Option<KeyboardOverrides>,
    ) -> Result<(), KeyboardActionError> {
        let device = self.keyboard_device()?;
        let parsed = KeyboardSequence::parse(sequence)?;
        let resolved = parsed.resolve(device.as_ref())?;
        let overrides = overrides.unwrap_or_default();
        let profile = resolve_keyboard_profile(&self.keyboard_profile(), &overrides);
        KeyboardEngine::new(device.as_ref(), profile, &default_sleep)?.execute(&resolved, KeyboardMode::Press)?;
        Ok(())
    }

    pub fn keyboard_release(
        &self,
        sequence: &str,
        overrides: Option<KeyboardOverrides>,
    ) -> Result<(), KeyboardActionError> {
        let device = self.keyboard_device()?;
        let parsed = KeyboardSequence::parse(sequence)?;
        let resolved = parsed.resolve(device.as_ref())?;
        let overrides = overrides.unwrap_or_default();
        let profile = resolve_keyboard_profile(&self.keyboard_profile(), &overrides);
        KeyboardEngine::new(device.as_ref(), profile, &default_sleep)?.execute(&resolved, KeyboardMode::Release)?;
        Ok(())
    }

    pub fn keyboard_type(
        &self,
        sequence: &str,
        overrides: Option<KeyboardOverrides>,
    ) -> Result<(), KeyboardActionError> {
        let device = self.keyboard_device()?;
        let parsed = KeyboardSequence::parse(sequence)?;
        let resolved = parsed.resolve(device.as_ref())?;
        let overrides = overrides.unwrap_or_default();
        let profile = resolve_keyboard_profile(&self.keyboard_profile(), &overrides);
        KeyboardEngine::new(device.as_ref(), profile, &default_sleep)?.execute(&resolved, KeyboardMode::Type)?;
        Ok(())
    }

    /// Returns the list of known key names exposed by the active keyboard device.
    pub fn keyboard_known_key_names(&self) -> Result<Vec<String>, KeyboardError> {
        let device = self.keyboard_device()?;
        Ok(device.known_key_names())
    }

    fn pointer_device(&self) -> Result<Arc<dyn PointerDevice>, PointerError> {
        self.platform.as_ref().map(|bundle| bundle.pointer.clone()).ok_or(PointerError::MissingDevice)
    }

    fn keyboard_device(&self) -> Result<Arc<dyn KeyboardDevice>, KeyboardError> {
        self.platform.as_ref().map(|bundle| bundle.keyboard.clone()).ok_or(KeyboardError::NotReady)
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime::test_fixtures::*;
    use platynui_core::platform::{PointerButton, ScrollDelta};
    use platynui_core::types::Point;
    use platynui_platform_mock::{
        KeyboardLogEntry, PointerLogEntry, reset_keyboard_state, reset_pointer_state, take_keyboard_log,
        take_pointer_log,
    };
    use rstest::rstest;
    use serial_test::serial;

    use super::Runtime;

    #[rstest]
    #[serial]
    fn keyboard_press_logs_events(rt_runtime_platform: Runtime) {
        reset_keyboard_state();
        let mut runtime = rt_runtime_platform;
        configure_keyboard_for_tests(&runtime);
        let overrides = zero_keyboard_overrides();

        runtime.keyboard_press("<Ctrl+Alt+T>", Some(overrides.clone())).expect("press succeeds");

        let log = take_keyboard_log();
        assert_eq!(
            log,
            vec![
                KeyboardLogEntry::StartInput,
                KeyboardLogEntry::Press("Control".into()),
                KeyboardLogEntry::Press("Alt".into()),
                KeyboardLogEntry::Press("T".into()),
                KeyboardLogEntry::EndInput,
            ]
        );

        runtime.keyboard_release("<Ctrl+Alt+T>", Some(overrides)).expect("cleanup release succeeds");
        runtime.shutdown();
    }

    #[rstest]
    #[serial]
    fn keyboard_release_logs_events(rt_runtime_platform: Runtime) {
        reset_keyboard_state();
        let mut runtime = rt_runtime_platform;
        configure_keyboard_for_tests(&runtime);
        let overrides = zero_keyboard_overrides();

        runtime.keyboard_press("<Ctrl+Alt+T>", Some(overrides.clone())).expect("press succeeds");
        reset_keyboard_state();

        runtime.keyboard_release("<Ctrl+Alt+T>", Some(overrides.clone())).expect("release succeeds");

        let log = take_keyboard_log();
        assert_eq!(
            log,
            vec![
                KeyboardLogEntry::StartInput,
                KeyboardLogEntry::Release("T".into()),
                KeyboardLogEntry::Release("Alt".into()),
                KeyboardLogEntry::Release("Control".into()),
                KeyboardLogEntry::EndInput,
            ]
        );

        runtime.shutdown();
    }

    #[rstest]
    #[serial]
    fn keyboard_type_emits_press_and_release(rt_runtime_platform: Runtime) {
        reset_keyboard_state();
        let mut runtime = rt_runtime_platform;
        configure_keyboard_for_tests(&runtime);
        let overrides = zero_keyboard_overrides();

        runtime.keyboard_type("Ab", Some(overrides)).expect("type succeeds");

        let log = take_keyboard_log();
        assert_eq!(
            log,
            vec![
                KeyboardLogEntry::StartInput,
                KeyboardLogEntry::Press("A".into()),
                KeyboardLogEntry::Release("A".into()),
                KeyboardLogEntry::Press("b".into()),
                KeyboardLogEntry::Release("b".into()),
                KeyboardLogEntry::EndInput,
            ]
        );

        runtime.shutdown();
    }

    #[rstest]
    #[serial]
    fn pointer_move_uses_device_log(rt_runtime_platform: Runtime) {
        reset_pointer_state();
        let runtime = rt_runtime_platform;
        configure_pointer_for_tests(&runtime);

        runtime.pointer_move_to(Point::new(50.0, 25.0), Some(zero_overrides())).expect("move succeeds");

        let log = take_pointer_log();
        assert!(log.iter().any(|event| matches!(event, PointerLogEntry::Move(p) if *p == Point::new(50.0, 25.0))));
    }

    #[rstest]
    #[serial]
    fn pointer_click_emits_press_and_release(rt_runtime_platform: Runtime) {
        reset_pointer_state();
        let runtime = rt_runtime_platform;
        configure_pointer_for_tests(&runtime);

        runtime.pointer_click(Some(Point::new(10.0, 10.0)), None, Some(zero_overrides())).expect("click succeeds");

        let log = take_pointer_log();
        assert!(log.iter().any(|event| matches!(event, PointerLogEntry::Press(PointerButton::Left))));
        assert!(log.iter().any(|event| matches!(event, PointerLogEntry::Release(PointerButton::Left))));
    }

    #[rstest]
    #[serial]
    fn pointer_multi_click_emits_multiple_events(rt_runtime_platform: Runtime) {
        reset_pointer_state();
        let runtime = rt_runtime_platform;
        configure_pointer_for_tests(&runtime);

        runtime
            .pointer_multi_click(Some(Point::new(20.0, 20.0)), Some(PointerButton::Right), 3, Some(zero_overrides()))
            .expect("multi-click succeeds");

        let log = take_pointer_log();
        let presses = log.iter().filter(|event| matches!(event, PointerLogEntry::Press(PointerButton::Right))).count();
        let releases =
            log.iter().filter(|event| matches!(event, PointerLogEntry::Release(PointerButton::Right))).count();
        assert_eq!(presses, 3);
        assert_eq!(releases, 3);
    }

    #[rstest]
    #[serial]
    fn pointer_multi_click_rejects_zero(rt_runtime_platform: Runtime) {
        reset_pointer_state();
        let runtime = rt_runtime_platform;
        configure_pointer_for_tests(&runtime);

        let error =
            runtime.pointer_multi_click(Some(Point::new(5.0, 5.0)), None, 0, Some(zero_overrides())).unwrap_err();
        match error {
            crate::PointerError::InvalidClickCount { provided } => assert_eq!(provided, 0),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[rstest]
    #[serial]
    fn pointer_scroll_chunks_delta(rt_runtime_platform: Runtime) {
        reset_pointer_state();
        let runtime = rt_runtime_platform;
        configure_pointer_for_tests(&runtime);

        let overrides = zero_overrides().scroll_step(ScrollDelta::new(0.0, -10.0));
        runtime.pointer_scroll(ScrollDelta::new(0.0, -25.0), Some(overrides)).expect("scroll succeeds");

        let scrolls: Vec<_> = take_pointer_log()
            .into_iter()
            .filter_map(|event| match event {
                PointerLogEntry::Scroll(delta) => Some(delta),
                _ => None,
            })
            .collect();
        assert_eq!(scrolls.len(), 3);
        let total: f64 = scrolls.iter().map(|delta| delta.vertical).sum();
        assert!((total + 25.0).abs() < f64::EPSILON);
    }
}
