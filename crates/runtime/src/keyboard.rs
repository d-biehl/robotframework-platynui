use std::time::Duration;

use platynui_core::platform::{
    KeyCode, KeyState, KeyboardDevice, KeyboardError, KeyboardEvent, KeyboardOverrides, KeyboardSettings,
};

use crate::keyboard_sequence::{ResolvedKeyboardSequence, ResolvedSegment};

pub enum KeyboardMode {
    Press,
    Release,
    Type,
}

pub struct KeyboardEngine<'a> {
    device: &'a dyn KeyboardDevice,
    settings: KeyboardSettings,
    sleep: &'a dyn Fn(Duration),
    pressed: Vec<KeyCode>,
    started: bool,
}

impl<'a> KeyboardEngine<'a> {
    pub fn new(
        device: &'a dyn KeyboardDevice,
        settings: KeyboardSettings,
        sleep: &'a dyn Fn(Duration),
    ) -> Result<Self, KeyboardError> {
        device.start_input()?;
        Ok(Self { device, settings, sleep, pressed: Vec::new(), started: true })
    }

    pub fn execute(mut self, sequence: &ResolvedKeyboardSequence, mode: KeyboardMode) -> Result<(), KeyboardError> {
        let mut result = match mode {
            KeyboardMode::Press => self.press_sequence(sequence),
            KeyboardMode::Release => self.release_sequence(sequence),
            KeyboardMode::Type => self.type_sequence(sequence),
        };

        if result.is_err() {
            let _ = self.release_all_pressed();
        }

        if self.started {
            let end_result = self.device.end_input();
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
        self.sleep(self.settings.press_delay);
        Ok(())
    }

    fn release_code(&mut self, code: &KeyCode) -> Result<(), KeyboardError> {
        self.device.send_key_event(KeyboardEvent { code: code.clone(), state: KeyState::Release })?;
        if let Some(pos) = self.pressed.iter().rposition(|stored| stored == code) {
            self.pressed.remove(pos);
        }
        self.sleep(self.settings.release_delay);
        Ok(())
    }

    fn release_all_pressed(&mut self) -> Result<(), KeyboardError> {
        while let Some(code) = self.pressed.pop() {
            self.device.send_key_event(KeyboardEvent { code: code.clone(), state: KeyState::Release })?;
            self.sleep(self.settings.release_delay);
        }
        Ok(())
    }

    fn sleep(&self, duration: Duration) {
        if !duration.is_zero() {
            (self.sleep)(duration);
        }
    }

    fn sleep_between_keys(&self) {
        self.sleep(self.settings.between_keys_delay);
    }

    fn sleep_chord_press(&self) {
        self.sleep(self.settings.chord_press_delay);
    }

    fn sleep_chord_release(&self) {
        self.sleep(self.settings.chord_release_delay);
    }

    fn sleep_after_text(&self) {
        self.sleep(self.settings.after_text_delay);
    }

    fn sleep_after_sequence(&self) {
        self.sleep(self.settings.after_sequence_delay);
    }
}

pub fn apply_overrides(base: &KeyboardSettings, overrides: &KeyboardOverrides) -> KeyboardSettings {
    let mut settings = base.clone();
    if let Some(value) = overrides.press_delay {
        settings.press_delay = value;
    }
    if let Some(value) = overrides.release_delay {
        settings.release_delay = value;
    }
    if let Some(value) = overrides.between_keys_delay {
        settings.between_keys_delay = value;
    }
    if let Some(value) = overrides.chord_press_delay {
        settings.chord_press_delay = value;
    }
    if let Some(value) = overrides.chord_release_delay {
        settings.chord_release_delay = value;
    }
    if let Some(value) = overrides.after_sequence_delay {
        settings.after_sequence_delay = value;
    }
    if let Some(value) = overrides.after_text_delay {
        settings.after_text_delay = value;
    }
    settings
}
