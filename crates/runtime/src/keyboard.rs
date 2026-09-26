use std::time::Duration;

use platynui_core::platform::{
    KeyCode, KeyState, KeyboardDevice, KeyboardError, KeyboardEvent, KeyboardOverrides, KeyboardProfile,
};

use crate::keyboard_sequence::{ResolvedKeyboardSequence, ResolvedSegment};

#[derive(Debug, Clone, Copy)]
pub enum KeyboardMode {
    Press,
    Release,
    Type,
}

pub struct KeyboardEngine<'a> {
    device: &'a dyn KeyboardDevice,
    profile: KeyboardProfile,
    sleep: &'a dyn Fn(Duration),
    pressed: Vec<KeyCode>,
    started: bool,
}

impl<'a> KeyboardEngine<'a> {
    pub fn new(
        device: &'a dyn KeyboardDevice,
        profile: KeyboardProfile,
        sleep: &'a dyn Fn(Duration),
    ) -> Result<Self, KeyboardError> {
        device.start_input()?;
        Ok(Self { device, profile, sleep, pressed: Vec::new(), started: true })
    }

    pub fn execute(mut self, sequence: &ResolvedKeyboardSequence, mode: KeyboardMode) -> Result<(), KeyboardError> {
        tracing::debug!(mode = ?mode, segments = sequence.segments().len(), "keyboard execute");
        let mut result = match mode {
            KeyboardMode::Press => self.press_sequence(sequence),
            KeyboardMode::Release => self.release_sequence(sequence),
            KeyboardMode::Type => self.type_sequence(sequence),
        };

        // The failure itself is returned; only keys left held are this layer's
        // to report. Key codes are opaque here and may belong to a secret, so
        // the records give the number of keys only.
        if result.is_err() && !self.pressed.is_empty() {
            tracing::debug!(keys = self.pressed.len(), "keyboard input failed; releasing the keys it pressed");
            if self.release_all_pressed().is_err() {
                tracing::error!(
                    keys = self.pressed.len(),
                    "releasing keys after a keyboard error failed; these keys may remain held down"
                );
            }
        }

        if self.started {
            let end_result = self.device.end_input();
            self.started = false;
            if result.is_ok()
                && let Err(end_err) = end_result
            {
                result = Err(end_err);
            }
        }

        result
    }

    fn press_sequence(&mut self, sequence: &ResolvedKeyboardSequence) -> Result<(), KeyboardError> {
        for (segment_index, segment) in sequence.segments().iter().enumerate() {
            match segment {
                ResolvedSegment::Text(codes) => {
                    for (idx, code) in codes.iter().enumerate() {
                        self.press_code(code)?;
                        if idx + 1 < codes.len() {
                            self.sleep_between_keys();
                        }
                    }
                    if !codes.is_empty() {
                        self.sleep_after_text();
                    }
                }
                ResolvedSegment::Shortcut(groups) => {
                    for (group_idx, group) in groups.iter().enumerate() {
                        for (idx, code) in group.iter().enumerate() {
                            self.press_code(code)?;
                            if idx + 1 < group.len() {
                                self.sleep_chord_press();
                            }
                        }
                        if group_idx + 1 < groups.len() {
                            self.sleep_between_keys();
                        }
                    }
                }
            }
            if segment_index + 1 < sequence.segments().len() {
                self.sleep_between_keys();
            }
        }
        self.sleep_after_sequence();
        Ok(())
    }

    fn release_sequence(&mut self, sequence: &ResolvedKeyboardSequence) -> Result<(), KeyboardError> {
        for (segment_index, segment) in sequence.segments().iter().enumerate() {
            match segment {
                ResolvedSegment::Text(codes) => {
                    for (idx, code) in codes.iter().enumerate().rev() {
                        self.release_code(code)?;
                        if idx > 0 {
                            self.sleep_between_keys();
                        }
                    }
                    if !codes.is_empty() {
                        self.sleep_after_text();
                    }
                }
                ResolvedSegment::Shortcut(groups) => {
                    for (group_idx, group) in groups.iter().enumerate() {
                        for (idx, code) in group.iter().enumerate().rev() {
                            self.release_code(code)?;
                            if idx > 0 {
                                self.sleep_chord_release();
                            }
                        }
                        if group_idx + 1 < groups.len() {
                            self.sleep_between_keys();
                        }
                    }
                }
            }
            if segment_index + 1 < sequence.segments().len() {
                self.sleep_between_keys();
            }
        }
        self.sleep_after_sequence();
        Ok(())
    }

    fn type_sequence(&mut self, sequence: &ResolvedKeyboardSequence) -> Result<(), KeyboardError> {
        for (segment_index, segment) in sequence.segments().iter().enumerate() {
            match segment {
                ResolvedSegment::Text(codes) => {
                    for (idx, code) in codes.iter().enumerate() {
                        self.press_code(code)?;
                        self.release_code(code)?;
                        if idx + 1 < codes.len() {
                            self.sleep_between_keys();
                        }
                    }
                    if !codes.is_empty() {
                        self.sleep_after_text();
                    }
                }
                ResolvedSegment::Shortcut(groups) => {
                    for (group_idx, group) in groups.iter().enumerate() {
                        for (idx, code) in group.iter().enumerate() {
                            self.press_code(code)?;
                            if idx + 1 < group.len() {
                                self.sleep_chord_press();
                            }
                        }
                        for (idx, code) in group.iter().enumerate().rev() {
                            self.release_code(code)?;
                            if idx > 0 {
                                self.sleep_chord_release();
                            }
                        }
                        if group_idx + 1 < groups.len() {
                            self.sleep_between_keys();
                        }
                    }
                }
            }
            if segment_index + 1 < sequence.segments().len() {
                self.sleep_between_keys();
            }
        }
        self.sleep_after_sequence();
        Ok(())
    }

    fn press_code(&mut self, code: &KeyCode) -> Result<(), KeyboardError> {
        self.device.send_key_event(KeyboardEvent { code: code.clone(), state: KeyState::Press })?;
        self.pressed.push(code.clone());
        self.sleep(self.profile.press_delay);
        Ok(())
    }

    fn release_code(&mut self, code: &KeyCode) -> Result<(), KeyboardError> {
        self.device.send_key_event(KeyboardEvent { code: code.clone(), state: KeyState::Release })?;
        if let Some(pos) = self.pressed.iter().rposition(|stored| stored == code) {
            self.pressed.remove(pos);
        }
        self.sleep(self.profile.release_delay);
        Ok(())
    }

    /// Releases the pressed keys, most recent first. A key whose release fails
    /// stays in `pressed`, so the count left afterwards is the keys still held.
    fn release_all_pressed(&mut self) -> Result<(), KeyboardError> {
        while let Some(code) = self.pressed.last().cloned() {
            self.device.send_key_event(KeyboardEvent { code, state: KeyState::Release })?;
            self.pressed.pop();
            self.sleep(self.profile.release_delay);
        }
        Ok(())
    }

    fn sleep(&self, duration: Duration) {
        if !duration.is_zero() {
            (self.sleep)(duration);
        }
    }

    fn sleep_between_keys(&self) {
        self.sleep(self.profile.between_keys_delay);
    }

    fn sleep_chord_press(&self) {
        self.sleep(self.profile.chord_press_delay);
    }

    fn sleep_chord_release(&self) {
        self.sleep(self.profile.chord_release_delay);
    }

    fn sleep_after_text(&self) {
        self.sleep(self.profile.after_text_delay);
    }

    fn sleep_after_sequence(&self) {
        self.sleep(self.profile.after_sequence_delay);
    }
}

impl Drop for KeyboardEngine<'_> {
    fn drop(&mut self) {
        if self.started {
            tracing::warn!("KeyboardEngine dropped without calling execute — ending input session");
            let _ = self.release_all_pressed();
            let _ = self.device.end_input();
            self.started = false;
        }
    }
}

pub fn resolve_profile(base: &KeyboardProfile, overrides: &KeyboardOverrides) -> KeyboardProfile {
    let mut profile = base.clone();
    if let Some(value) = overrides.press_delay {
        profile.press_delay = value;
    }
    if let Some(value) = overrides.release_delay {
        profile.release_delay = value;
    }
    if let Some(value) = overrides.between_keys_delay {
        profile.between_keys_delay = value;
    }
    if let Some(value) = overrides.chord_press_delay {
        profile.chord_press_delay = value;
    }
    if let Some(value) = overrides.chord_release_delay {
        profile.chord_release_delay = value;
    }
    if let Some(value) = overrides.after_sequence_delay {
        profile.after_sequence_delay = value;
    }
    if let Some(value) = overrides.after_text_delay {
        profile.after_text_delay = value;
    }
    profile
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard_sequence::KeyboardSequence;
    use crate::test_support::{logged, records};
    use platynui_core::platform::PlatformError;

    /// A device that fails to press `X`, and optionally fails every release.
    struct FailingKeyboard {
        fail_release: bool,
    }

    impl KeyboardDevice for FailingKeyboard {
        fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
            Ok(KeyCode::new(name.to_owned()))
        }

        fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError> {
            let name = event.code.downcast_ref::<String>().map(String::as_str);
            let fails = match event.state {
                KeyState::Press => name == Some("X"),
                KeyState::Release => self.fail_release,
            };
            if fails {
                Err(KeyboardError::Platform(PlatformError::OperationFailed {
                    operation: "stub send",
                    details: Some("Jv5 wire cut".into()),
                }))
            } else {
                Ok(())
            }
        }
    }

    fn run(device: &FailingKeyboard, sequence: &str, mode: KeyboardMode) -> (Result<(), KeyboardError>, String) {
        let resolved = KeyboardSequence::parse(sequence).expect("parses").resolve(device).expect("resolves");
        logged(|| {
            KeyboardEngine::new(device, KeyboardProfile::default(), &|_| {}).expect("starts").execute(&resolved, mode)
        })
    }

    #[test]
    fn a_failure_with_keys_held_releases_them_and_records_the_count_at_debug() {
        let (result, log) = run(&FailingKeyboard { fail_release: false }, "<Ctrl+Shift+X>", KeyboardMode::Press);

        assert!(result.is_err());
        assert!(records(&log, "WARN").is_empty() && records(&log, "ERROR").is_empty(), "{log}");
        let releasing: Vec<_> = records(&log, "DEBUG").into_iter().filter(|line| line.contains("releasing")).collect();
        assert_eq!(releasing.len(), 1, "{log}");
        assert!(releasing[0].contains("keys=2"), "{log}");
        assert!(!log.contains("Jv5"), "the device's error text is not recorded below trace: {log}");
    }

    #[test]
    fn a_failure_without_keys_held_records_nothing_about_releasing() {
        let (result, log) = run(&FailingKeyboard { fail_release: false }, "X", KeyboardMode::Type);

        assert!(result.is_err());
        assert!(!log.contains("releasing"), "{log}");
        assert!(records(&log, "WARN").is_empty() && records(&log, "ERROR").is_empty(), "{log}");
    }

    #[test]
    fn keys_that_cannot_be_released_are_an_error_with_their_count() {
        let (result, log) = run(&FailingKeyboard { fail_release: true }, "<Ctrl+Shift+X>", KeyboardMode::Press);

        assert!(result.is_err(), "the keyboard error is returned");
        let errors = records(&log, "ERROR");
        assert_eq!(errors.len(), 1, "{log}");
        assert!(errors[0].contains("keys=2") && errors[0].contains("held down"), "{log}");
        assert!(!log.contains("Jv5"), "{log}");
    }
}
