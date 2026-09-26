use std::sync::Arc;

use platynui_core::platform::{
    KeyboardDevice, KeyboardError, KeyboardOverrides, PointerButton, PointerDevice, ScrollDelta,
};
use platynui_core::provider::ProviderError;
use platynui_core::types::Point;
use platynui_core::ui::UiNode;

use crate::keyboard::{KeyboardEngine, KeyboardMode, resolve_profile as resolve_keyboard_profile};
use crate::keyboard_sequence::KeyboardSequence;
use crate::pointer::{PointerError, PointerOverrides, PointerProfile, PointerSettings};

use super::error::KeyboardActionError;
use super::{Runtime, default_sleep};

impl Runtime {
    /// Returns a copy of the current pointer settings.
    ///
    /// # Panics
    ///
    /// Panics if the pointer settings lock is poisoned (another thread panicked while holding it).
    pub fn pointer_settings(&self) -> PointerSettings {
        self.pointer_settings.lock().expect("pointer_settings lock poisoned").clone()
    }

    /// Replaces the pointer settings and applies them to the pointer engine.
    ///
    /// # Panics
    ///
    /// Panics if the pointer settings lock or the pointer engine lock is poisoned.
    pub fn set_pointer_settings(&self, settings: PointerSettings) {
        {
            *self.pointer_settings.lock().expect("pointer_settings lock poisoned") = settings.clone();
        }
        if let Some(engine) = self.pointer_engine.lock().expect("pointer_engine lock poisoned").as_mut() {
            engine.set_settings(settings);
        }
    }

    /// Returns a copy of the current pointer profile.
    ///
    /// # Panics
    ///
    /// Panics if the pointer profile lock is poisoned (another thread panicked while holding it).
    pub fn pointer_profile(&self) -> PointerProfile {
        self.pointer_profile.lock().expect("pointer_profile lock poisoned").clone()
    }

    /// Replaces the pointer profile and applies it to the pointer engine.
    ///
    /// # Panics
    ///
    /// Panics if the pointer profile lock or the pointer engine lock is poisoned.
    pub fn set_pointer_profile(&self, profile: PointerProfile) {
        {
            *self.pointer_profile.lock().expect("pointer_profile lock poisoned") = profile.clone();
        }
        if let Some(engine) = self.pointer_engine.lock().expect("pointer_engine lock poisoned").as_mut() {
            engine.set_profile(profile);
        }
    }

    /// Returns the current pointer position reported by the pointer device.
    ///
    /// # Errors
    ///
    /// Returns [`PointerError::MissingDevice`] if the runtime has no pointer device and
    /// [`PointerError::Platform`] if the device cannot report its position.
    pub fn pointer_position(&self) -> Result<Point, PointerError> {
        let device = self.pointer_device()?;
        Ok(device.position()?)
    }

    /// Resolves the deepest, topmost UI node at the given desktop point by
    /// asking each provider to hit-test it.
    ///
    /// Returns the first provider hit. `Ok(None)` means at least one provider
    /// supports hit-testing but nothing is at the point;
    /// `Err(ProviderError::UnsupportedOperation { .. })` means no provider can
    /// hit-test at all, so callers can disable point-based features.
    ///
    /// # Errors
    ///
    /// Returns the first provider error other than [`ProviderError::UnsupportedOperation`], or
    /// [`ProviderError::UnsupportedOperation`] if no provider supports hit-testing.
    pub fn element_at_point(&self, point: Point) -> Result<Option<Arc<dyn UiNode>>, ProviderError> {
        let mut any_supported = false;
        for provider in self.providers() {
            match provider.element_at_point(point) {
                Ok(Some(node)) => return Ok(Some(node)),
                Ok(None) => any_supported = true,
                Err(ProviderError::UnsupportedOperation { .. }) => {}
                Err(err) => return Err(err),
            }
        }
        if any_supported {
            Ok(None)
        } else {
            Err(ProviderError::UnsupportedOperation {
                operation: "element_at_point",
                details: Some("no provider supports hit-testing".into()),
            })
        }
    }

    /// Returns a copy of the current keyboard profile.
    ///
    /// # Panics
    ///
    /// Panics if the keyboard profile lock is poisoned (another thread panicked while holding it).
    pub fn keyboard_profile(&self) -> platynui_core::platform::KeyboardProfile {
        self.keyboard_profile.lock().expect("keyboard_profile lock poisoned").clone()
    }

    /// Replaces the keyboard profile.
    ///
    /// # Panics
    ///
    /// Panics if the keyboard profile lock is poisoned (another thread panicked while holding it).
    pub fn set_keyboard_profile(&self, profile: platynui_core::platform::KeyboardProfile) {
        *self.keyboard_profile.lock().expect("keyboard_profile lock poisoned") = profile;
    }

    /// Moves the pointer to `point`.
    ///
    /// # Errors
    ///
    /// - [`PointerError::Poisoned`] if the pointer engine lock is poisoned.
    /// - [`PointerError::MissingDevice`] if the runtime has no pointer device (no platform
    ///   backend, or after shutdown).
    /// - [`PointerError::Platform`] if the device rejects an action.
    /// - [`PointerError::EnsureMove`] if the pointer does not reach a target position in time.
    // Public API taking the overrides by value; changing the signature would break callers
    // (CLI, Python bindings).
    #[allow(clippy::needless_pass_by_value)]
    pub fn pointer_move_to(&self, point: Point, overrides: Option<PointerOverrides>) -> Result<Point, PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().map_err(|_| PointerError::Poisoned)?;
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        engine.move_to(point, overrides_ref)
    }

    /// Clicks `button` (or the default button) once, after moving to `target` if one is given.
    ///
    /// # Errors
    ///
    /// - [`PointerError::Poisoned`] if the pointer engine lock is poisoned.
    /// - [`PointerError::MissingDevice`] if the runtime has no pointer device (no platform
    ///   backend, or after shutdown).
    /// - [`PointerError::Platform`] if the device rejects an action.
    /// - [`PointerError::EnsureMove`] if the pointer does not reach a target position in time.
    // Public API taking the overrides by value; changing the signature would break callers
    // (CLI, Python bindings).
    #[allow(clippy::needless_pass_by_value)]
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

    /// Clicks `button` (or the default button) `clicks` times, after moving to `target` if one is given.
    ///
    /// # Errors
    ///
    /// - [`PointerError::Poisoned`] if the pointer engine lock is poisoned.
    /// - [`PointerError::MissingDevice`] if the runtime has no pointer device (no platform
    ///   backend, or after shutdown).
    /// - [`PointerError::Platform`] if the device rejects an action.
    /// - [`PointerError::EnsureMove`] if the pointer does not reach a target position in time.
    /// - [`PointerError::InvalidClickCount`] if `clicks` is zero.
    // Public API taking the overrides by value; changing the signature would break callers
    // (CLI, Python bindings).
    #[allow(clippy::needless_pass_by_value)]
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

    /// Presses `button` (or the default button), after moving to `target` if one is given.
    ///
    /// # Errors
    ///
    /// - [`PointerError::Poisoned`] if the pointer engine lock is poisoned.
    /// - [`PointerError::MissingDevice`] if the runtime has no pointer device (no platform
    ///   backend, or after shutdown).
    /// - [`PointerError::Platform`] if the device rejects an action.
    /// - [`PointerError::EnsureMove`] if the pointer does not reach a target position in time.
    // Public API taking the overrides by value; changing the signature would break callers
    // (CLI, Python bindings).
    #[allow(clippy::needless_pass_by_value)]
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

    /// Releases `button` (or the default button), after moving to `target` if one is given.
    ///
    /// # Errors
    ///
    /// - [`PointerError::Poisoned`] if the pointer engine lock is poisoned.
    /// - [`PointerError::MissingDevice`] if the runtime has no pointer device (no platform
    ///   backend, or after shutdown).
    /// - [`PointerError::Platform`] if the device rejects an action.
    /// - [`PointerError::EnsureMove`] if the pointer does not reach a target position in time.
    // Public API taking the overrides by value; changing the signature would break callers
    // (CLI, Python bindings).
    #[allow(clippy::needless_pass_by_value)]
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

    /// Scrolls by `delta`.
    ///
    /// # Errors
    ///
    /// - [`PointerError::Poisoned`] if the pointer engine lock is poisoned.
    /// - [`PointerError::MissingDevice`] if the runtime has no pointer device (no platform
    ///   backend, or after shutdown).
    /// - [`PointerError::Platform`] if the device rejects an action.
    // Public API taking the overrides by value; changing the signature would break callers
    // (CLI, Python bindings).
    #[allow(clippy::needless_pass_by_value)]
    pub fn pointer_scroll(&self, delta: ScrollDelta, overrides: Option<PointerOverrides>) -> Result<(), PointerError> {
        let bounds = self.desktop.info().bounds;
        let mut guard = self.pointer_engine.lock().map_err(|_| PointerError::Poisoned)?;
        let engine = guard.as_mut().ok_or(PointerError::MissingDevice)?;
        engine.set_desktop_bounds(bounds);
        let overrides_ref = overrides.as_ref();
        engine.scroll(delta, overrides_ref)
    }

    /// Drags from `start` to `end` with `button` (or the default button) held down.
    ///
    /// # Errors
    ///
    /// - [`PointerError::Poisoned`] if the pointer engine lock is poisoned.
    /// - [`PointerError::MissingDevice`] if the runtime has no pointer device (no platform
    ///   backend, or after shutdown).
    /// - [`PointerError::Platform`] if the device rejects an action.
    /// - [`PointerError::EnsureMove`] if the pointer does not reach a target position in time.
    // Public API taking the overrides by value; changing the signature would break callers
    // (CLI, Python bindings).
    #[allow(clippy::needless_pass_by_value)]
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

    /// Presses (and keeps pressed) the keys of `sequence`.
    ///
    /// # Errors
    ///
    /// See [`keyboard_type`](Self::keyboard_type).
    pub fn keyboard_press(
        &self,
        sequence: &str,
        overrides: Option<KeyboardOverrides>,
    ) -> Result<(), KeyboardActionError> {
        self.run_keyboard(sequence, overrides, KeyboardMode::Press)
    }

    /// Releases the keys of `sequence`.
    ///
    /// # Errors
    ///
    /// See [`keyboard_type`](Self::keyboard_type).
    pub fn keyboard_release(
        &self,
        sequence: &str,
        overrides: Option<KeyboardOverrides>,
    ) -> Result<(), KeyboardActionError> {
        self.run_keyboard(sequence, overrides, KeyboardMode::Release)
    }

    /// Types `sequence`: presses and releases each key in turn.
    ///
    /// # Errors
    ///
    /// - [`KeyboardActionError::Keyboard`] if the runtime has no keyboard device.
    /// - [`KeyboardActionError::Sequence`] if `sequence` does not parse.
    /// - [`KeyboardActionError::Key`] if the device cannot map a character or key name.
    /// - [`KeyboardActionError::Start`] if the device cannot start the input.
    /// - [`KeyboardActionError::Send`] if the device reports an error while sending the input.
    pub fn keyboard_type(
        &self,
        sequence: &str,
        overrides: Option<KeyboardOverrides>,
    ) -> Result<(), KeyboardActionError> {
        self.run_keyboard(sequence, overrides, KeyboardMode::Type)
    }

    fn run_keyboard(
        &self,
        sequence: &str,
        overrides: Option<KeyboardOverrides>,
        mode: KeyboardMode,
    ) -> Result<(), KeyboardActionError> {
        let device = self.keyboard_device()?;
        let parsed = KeyboardSequence::parse(sequence)?;
        let resolved = parsed.resolve(device.as_ref())?;
        let overrides = overrides.unwrap_or_default();
        let profile = resolve_keyboard_profile(&self.keyboard_profile(), &overrides);
        KeyboardEngine::new(device.as_ref(), profile, &default_sleep)
            .map_err(KeyboardActionError::Start)?
            .execute(&resolved, mode)
            .map_err(KeyboardActionError::Send)
    }

    /// Returns the list of known key names exposed by the active keyboard device.
    ///
    /// # Errors
    ///
    /// Returns [`KeyboardError::NotReady`] if the runtime has no keyboard device.
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

/// Typed text: errors say why and where, without repeating the text, and
/// their sensitive rendering names no part of it.
#[cfg(test)]
mod typed_text_tests {
    use rstest::rstest;

    use super::Runtime;
    use crate::runtime::test_fixtures::*;
    use crate::{KeyboardActionError, SyntaxHint};

    fn typing_error(sequence: &str) -> KeyboardActionError {
        runtime_with_stub_keyboard(false).keyboard_type(sequence, None).expect_err("typing fails")
    }

    #[track_caller]
    fn assert_hidden(rendering: &str, fragments: &[&str]) {
        for fragment in fragments {
            assert!(!rendering.contains(fragment), "{rendering:?} contains {fragment:?}");
        }
    }

    #[track_caller]
    fn assert_shows(rendering: &str, parts: &[&str]) {
        for part in parts {
            assert!(rendering.contains(part), "{rendering:?} lacks {part:?}");
        }
    }

    #[test]
    fn a_character_that_cannot_be_typed_is_named_with_its_position_and_the_reason() {
        let err = typing_error(&format!("Qz7{REJECTED_CHAR}"));

        let message = err.to_string();
        assert_shows(&message, &["position 4", "'é'", "cannot be typed", "unsupported key: é"]);
        assert_hidden(&message, &["Qz7"]);
        let sensitive = err.sensitive().to_string();
        assert_shows(&sensitive, &["position 4", "cannot be typed"]);
        assert_hidden(&sensitive, &["Qz7", "é", "unsupported"]);
        assert_eq!(err.syntax_hint(), None);
    }

    #[test]
    fn an_escaped_character_is_reported_at_its_backslash() {
        let err = typing_error("Qz7\\u00E9");

        assert_shows(&err.to_string(), &["position 4", "'é'"]);
        assert_hidden(&err.to_string(), &["Qz7"]);
        assert_hidden(&err.sensitive().to_string(), &["Qz7", "é", "00E9"]);
    }

    #[test]
    fn a_key_name_that_is_not_known_hints_at_the_escape() {
        let runtime = runtime_with_stub_keyboard(false);
        for err in [
            runtime.keyboard_type("Qz7<Kq9>w", None).expect_err("typing fails"),
            runtime.keyboard_press("Qz7<Kq9>w", None).expect_err("pressing fails"),
        ] {
            let message = err.to_string();
            assert_shows(&message, &["'Kq9'", "position 5", "cannot be typed", "unsupported key: Kq9", "\\<"]);
            assert_hidden(&message, &["Qz7"]);
            let sensitive = err.sensitive().to_string();
            assert_shows(&sensitive, &["position 5", "cannot be typed", "\\<"]);
            assert_hidden(&sensitive, &["Qz7", "Kq9", "unsupported"]);
            assert_eq!(err.syntax_hint(), Some(SyntaxHint::LiteralLessThan));
        }
    }

    #[rstest]
    #[case::unclosed(
        "Qz7<Kq9",
        4,
        "the < at position 4 opens a key block that is not closed with >",
        SyntaxHint::LiteralLessThan
    )]
    #[case::unclosed_at_start("<Ctrl A", 1, "opens a key block that is not closed with >", SyntaxHint::LiteralLessThan)]
    #[case::unclosed_at_end(
        "Qz7<Kq9 ",
        9,
        "a key name is expected at position 9, and the key block is not closed with >",
        SyntaxHint::LiteralLessThan
    )]
    #[case::trailing_backslash("Qz7\\", 4, "the \\ at position 4 escapes nothing", SyntaxHint::LiteralBackslash)]
    #[case::backslash_before_line_break("ab\\\ncd", 3, "escapes nothing", SyntaxHint::LiteralBackslash)]
    #[case::empty_key_block("Qz7<>", 5, "a key name is expected at position 5", SyntaxHint::LiteralLessThan)]
    #[case::lt_in_key_block("a<<b>", 3, "a key name is expected at position 3", SyntaxHint::LiteralLessThan)]
    #[case::non_ascii("äöü<Kq9", 4, "opens a key block that is not closed with >", SyntaxHint::LiteralLessThan)]
    #[case::second_line("ab\ncd<Kq9", 6, "opens a key block that is not closed with >", SyntaxHint::LiteralLessThan)]
    fn a_sequence_that_does_not_parse_gives_the_position_and_why(
        #[case] input: &str,
        #[case] position: usize,
        #[case] explanation: &str,
        #[case] hint: SyntaxHint,
    ) {
        let err = typing_error(input);
        let position = format!("position {position}");
        let fragments = [input, "Qz7", "Kq9", "Ctrl", "äöü", "\n", "EOI", "segment", "sequence"];

        let message = err.to_string();
        assert_shows(&message, &[&position, explanation]);
        assert_hidden(&message, &fragments);
        assert_hidden(&format!("{err:?}"), &fragments[..6]);
        let sensitive = err.sensitive().to_string();
        assert_shows(&sensitive, &[&position, explanation]);
        assert_hidden(&sensitive, &fragments);
        assert_eq!(err.syntax_hint(), Some(hint));
        let written = if hint == SyntaxHint::LiteralLessThan { "\\<" } else { "\\\\" };
        assert_shows(&message, &[written]);
    }

    #[rstest]
    #[case::hex("Kw\\xQz", "position 3", "\\xQz", &["Kw", "Qz"])]
    #[case::unicode("C:\\users", "position 3", "\\users", &["C:", "sers"])]
    fn an_invalid_escape_is_named_with_its_position(
        #[case] input: &str,
        #[case] position: &str,
        #[case] escape: &str,
        #[case] hidden_when_sensitive: &[&str],
    ) {
        let err = typing_error(input);

        let message = err.to_string();
        assert_shows(&message, &[position, escape]);
        assert_hidden(&message, &hidden_when_sensitive[..1]);
        let sensitive = err.sensitive().to_string();
        assert_shows(&sensitive, &[position, "invalid"]);
        assert_hidden(&sensitive, hidden_when_sensitive);
        assert_eq!(err.syntax_hint(), None);
    }

    #[test]
    fn a_send_failure_keeps_the_reason_and_its_sensitive_rendering_only_says_sending_failed() {
        let err = typing_error(&format!("Qz7{UNSENDABLE_CHAR}"));

        assert_shows(&err.to_string(), &[SEND_FAILURE_TEXT]);
        assert_hidden(&err.to_string(), &["Qz7", "position"]);
        assert_eq!(err.sensitive().to_string(), "sending the keyboard input failed");
    }

    #[test]
    fn a_device_that_cannot_start_the_input_says_so() {
        let err = runtime_with_stub_keyboard(true).keyboard_type("Qz7", None).expect_err("starting fails");

        assert_shows(&err.to_string(), &["starting the keyboard input failed", SEND_FAILURE_TEXT]);
        assert_eq!(err.sensitive().to_string(), "starting the keyboard input failed");
    }

    #[test]
    fn a_runtime_without_platform_has_no_keyboard_device_ready() {
        let runtime = Runtime::new_with_factories(&[&RUNTIME_FACTORY]).expect("runtime");
        let err = runtime.keyboard_type("Qz7", None).expect_err("no keyboard");

        assert_eq!(err.sensitive().to_string(), "no keyboard device is ready");
        assert_hidden(&err.to_string(), &["Qz7"]);
    }
}
