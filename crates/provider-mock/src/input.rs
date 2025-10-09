use crate::events;
use platynui_core::ui::attribute_names::text_content;
use platynui_core::ui::{Namespace, RuntimeId, UiAttribute, UiValue};
use std::collections::HashMap;
// no additional std imports required
use std::sync::{Arc, LazyLock, RwLock};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyboardInputEvent {
    Press(String),
    Release(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TextInputError {
    #[error("no text buffer registered for runtime id '{0}'")]
    MissingTextBuffer(String),
}

struct TextAttribute {
    namespace: Namespace,
    buffer: Arc<RwLock<String>>,
}

impl UiAttribute for TextAttribute {
    fn namespace(&self) -> Namespace {
        self.namespace
    }

    fn name(&self) -> &str {
        text_content::TEXT
    }

    fn value(&self) -> UiValue {
        let value = self.buffer.read().expect("text buffer poisoned").clone();
        UiValue::from(value)
    }
}

static TEXT_BUFFERS: LazyLock<RwLock<HashMap<String, Arc<RwLock<String>>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub(crate) fn reset_text_buffers() {
    TEXT_BUFFERS.write().expect("text buffer registry poisoned").clear();
}

pub(crate) fn register_text_attribute(
    namespace: Namespace,
    runtime_id: &RuntimeId,
    initial: &str,
) -> Arc<dyn UiAttribute> {
    let buffer = Arc::new(RwLock::new(initial.to_owned()));
    TEXT_BUFFERS
        .write()
        .expect("text buffer registry poisoned")
        .insert(runtime_id.as_str().to_owned(), Arc::clone(&buffer));
    Arc::new(TextAttribute { namespace, buffer })
}

pub fn replace_text(runtime_id: &str, value: impl Into<String>) -> Result<String, TextInputError> {
    let buffer = text_buffer(runtime_id)?;
    let mut guard = buffer.write().expect("text buffer poisoned");
    *guard = value.into();
    let updated = guard.clone();
    drop(guard);
    events::emit_node_updated(runtime_id);
    Ok(updated)
}

pub fn append_text(runtime_id: &str, value: &str) -> Result<String, TextInputError> {
    let buffer = text_buffer(runtime_id)?;
    let mut guard = buffer.write().expect("text buffer poisoned");
    guard.push_str(value);
    let updated = guard.clone();
    drop(guard);
    events::emit_node_updated(runtime_id);
    Ok(updated)
}

pub fn text_snapshot(runtime_id: &str) -> Option<String> {
    text_buffer(runtime_id).ok().map(|buffer| buffer.read().expect("text buffer poisoned").clone())
}

pub fn apply_keyboard_events(runtime_id: &str, events: &[KeyboardInputEvent]) -> Result<String, TextInputError> {
    let buffer = text_buffer(runtime_id)?;
    let mut guard = buffer.write().expect("text buffer poisoned");
    let mut modifiers = ModifierState::default();

    for event in events {
        match event {
            KeyboardInputEvent::Press(name) => {
                if modifiers.register_press(name) {
                    continue;
                }
                if handle_special_press(&mut guard, name, &modifiers) {
                    continue;
                }
                if modifiers.non_shift_active() {
                    continue;
                }
                guard.push_str(resolve_text_token(name));
            }
            KeyboardInputEvent::Release(name) => {
                modifiers.register_release(name);
            }
        }
    }

    let updated = guard.clone();
    drop(guard);
    events::emit_node_updated(runtime_id);
    Ok(updated)
}

fn text_buffer(runtime_id: &str) -> Result<Arc<RwLock<String>>, TextInputError> {
    TEXT_BUFFERS
        .read()
        .expect("text buffer registry poisoned")
        .get(runtime_id)
        .cloned()
        .ok_or_else(|| TextInputError::MissingTextBuffer(runtime_id.to_owned()))
}

fn resolve_text_token(name: &str) -> &str {
    name
}

fn handle_special_press(buffer: &mut String, name: &str, modifiers: &ModifierState) -> bool {
    let lowered = name.to_ascii_lowercase();
    match lowered.as_str() {
        "space" => {
            if !modifiers.non_shift_active() {
                buffer.push(' ');
            }
            true
        }
        "enter" | "return" => {
            if !modifiers.non_shift_active() {
                buffer.push('\n');
            }
            true
        }
        "tab" => {
            if !modifiers.non_shift_active() {
                buffer.push('\t');
            }
            true
        }
        "backspace" => {
            buffer.pop();
            true
        }
        "delete" => {
            buffer.pop();
            true
        }
        _ => {
            if name.chars().count() == 1
                && !modifiers.non_shift_active()
                && let Some(ch) = name.chars().next()
            {
                buffer.push(ch);
                return true;
            }
            false
        }
    }
}

#[derive(Default)]
struct ModifierState {
    control: usize,
    alt: usize,
    option: usize,
    command: usize,
    windows: usize,
    super_key: usize,
    meta: usize,
    shift: usize,
}

impl ModifierState {
    fn register_press(&mut self, name: &str) -> bool {
        match canonical_modifier(name) {
            Some(Modifier::Control) => {
                self.control += 1;
                true
            }
            Some(Modifier::Alt) => {
                self.alt += 1;
                true
            }
            Some(Modifier::Option) => {
                self.option += 1;
                true
            }
            Some(Modifier::Command) => {
                self.command += 1;
                true
            }
            Some(Modifier::Windows) => {
                self.windows += 1;
                true
            }
            Some(Modifier::Super) => {
                self.super_key += 1;
                true
            }
            Some(Modifier::Meta) => {
                self.meta += 1;
                true
            }
            Some(Modifier::Shift) => {
                self.shift += 1;
                true
            }
            None => false,
        }
    }

    fn register_release(&mut self, name: &str) {
        match canonical_modifier(name) {
            Some(Modifier::Control) => self.control = self.control.saturating_sub(1),
            Some(Modifier::Alt) => self.alt = self.alt.saturating_sub(1),
            Some(Modifier::Option) => self.option = self.option.saturating_sub(1),
            Some(Modifier::Command) => self.command = self.command.saturating_sub(1),
            Some(Modifier::Windows) => self.windows = self.windows.saturating_sub(1),
            Some(Modifier::Super) => self.super_key = self.super_key.saturating_sub(1),
            Some(Modifier::Meta) => self.meta = self.meta.saturating_sub(1),
            Some(Modifier::Shift) => self.shift = self.shift.saturating_sub(1),
            None => {}
        }
    }

    fn non_shift_active(&self) -> bool {
        self.control > 0
            || self.alt > 0
            || self.option > 0
            || self.command > 0
            || self.windows > 0
            || self.super_key > 0
            || self.meta > 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Modifier {
    Control,
    Alt,
    Option,
    Command,
    Windows,
    Super,
    Meta,
    Shift,
}

fn canonical_modifier(name: &str) -> Option<Modifier> {
    match name.to_ascii_lowercase().as_str() {
        "control" | "ctrl" => Some(Modifier::Control),
        "alt" | "menu" => Some(Modifier::Alt),
        "option" => Some(Modifier::Option),
        "command" | "cmd" => Some(Modifier::Command),
        "windows" | "win" => Some(Modifier::Windows),
        "super" => Some(Modifier::Super),
        "meta" => Some(Modifier::Meta),
        "shift" => Some(Modifier::Shift),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider;
    use crate::tree::reset_mock_tree;
    use platynui_core::ui::{Namespace, UiValue};
    use rstest::rstest;
    use serial_test::serial;

    const STATUS_ID: &str = "mock://text/status";

    #[rstest]
    #[serial]
    fn replace_and_append_text_updates_attribute() {
        reset_mock_tree();
        let _provider = provider::instantiate_test_provider();

        replace_text(STATUS_ID, "Hallo").expect("replace succeeds");
        append_text(STATUS_ID, " Welt").expect("append succeeds");

        let snapshot = text_snapshot(STATUS_ID).expect("text snapshot available");
        assert_eq!(snapshot, "Hallo Welt");
    }

    #[rstest]
    #[serial]
    fn keyboard_events_append_characters() {
        reset_mock_tree();
        let _provider = provider::instantiate_test_provider();

        replace_text(STATUS_ID, "").expect("clear succeeds");
        let events = [
            KeyboardInputEvent::Press("H".into()),
            KeyboardInputEvent::Release("H".into()),
            KeyboardInputEvent::Press("i".into()),
            KeyboardInputEvent::Release("i".into()),
            KeyboardInputEvent::Press("😀".into()),
            KeyboardInputEvent::Release("😀".into()),
        ];
        apply_keyboard_events(STATUS_ID, &events).expect("events applied");

        assert_eq!(text_snapshot(STATUS_ID).unwrap(), "Hi😀");
    }

    #[rstest]
    #[serial]
    fn keyboard_events_honor_backspace_and_modifiers() {
        reset_mock_tree();
        let _provider = provider::instantiate_test_provider();

        replace_text(STATUS_ID, "Ready").expect("reset text");
        let events = [
            KeyboardInputEvent::Press("Backspace".into()),
            KeyboardInputEvent::Release("Backspace".into()),
            KeyboardInputEvent::Press("Control".into()),
            KeyboardInputEvent::Press("A".into()),
            KeyboardInputEvent::Release("A".into()),
            KeyboardInputEvent::Release("Control".into()),
            KeyboardInputEvent::Press(" ".into()),
            KeyboardInputEvent::Release(" ".into()),
            KeyboardInputEvent::Press("B".into()),
            KeyboardInputEvent::Release("B".into()),
        ];
        apply_keyboard_events(STATUS_ID, &events).expect("events applied");

        assert_eq!(text_snapshot(STATUS_ID).unwrap(), "Read B");
    }

    #[rstest]
    fn register_text_attribute_registers_buffer() {
        reset_text_buffers();
        let runtime_id = RuntimeId::from("mock://proof");
        let attribute = register_text_attribute(Namespace::Control, &runtime_id, "Start");
        assert_eq!(attribute.name(), text_content::TEXT);
        assert_eq!(attribute.value(), UiValue::from(String::from("Start")));
        assert_eq!(text_snapshot(runtime_id.as_str()).unwrap(), "Start");
    }
}
