use clap::{Args, Subcommand};
use platynui_core::platform::KeyboardOverrides;
use platynui_runtime::Runtime;
use std::time::Duration;

use crate::util::CliResult;

#[derive(Args)]
pub struct KeyboardArgs {
    #[command(subcommand)]
    pub command: KeyboardCommand,
}

#[derive(Subcommand)]
pub enum KeyboardCommand {
    Type(KeyboardTypeArgs),
    Press(KeyboardSequenceArgs),
    Release(KeyboardSequenceArgs),
}

#[derive(Args)]
pub struct KeyboardTypeArgs {
    #[arg(value_name = "SEQUENCE", help = "Keyboard sequenz inkl. Text, z. B. <Ctrl+A>Hallo")]
    pub sequence: String,

    #[command(flatten)]
    overrides: KeyboardOverrideArgs,
}

#[derive(Args)]
pub struct KeyboardSequenceArgs {
    #[arg(value_name = "SEQUENCE", help = "Keyboard sequence im <Ctrl+Alt+T>-Format.")]
    pub sequence: String,

    #[command(flatten)]
    overrides: KeyboardOverrideArgs,
}

#[derive(Args, Default, Clone)]
struct KeyboardOverrideArgs {
    #[arg(long = "delay-ms", value_parser = parse_millis)]
    uniform_delay: Option<Duration>,
    #[arg(long = "press-delay", value_parser = parse_millis)]
    press_delay: Option<Duration>,
    #[arg(long = "release-delay", value_parser = parse_millis)]
    release_delay: Option<Duration>,
    #[arg(long = "between-keys-delay", value_parser = parse_millis)]
    between_keys_delay: Option<Duration>,
    #[arg(long = "chord-press-delay", value_parser = parse_millis)]
    chord_press_delay: Option<Duration>,
    #[arg(long = "chord-release-delay", value_parser = parse_millis)]
    chord_release_delay: Option<Duration>,
    #[arg(long = "after-sequence-delay", value_parser = parse_millis)]
    after_sequence_delay: Option<Duration>,
    #[arg(long = "after-text-delay", value_parser = parse_millis)]
    after_text_delay: Option<Duration>,
}

pub fn run(runtime: &Runtime, args: &KeyboardArgs) -> CliResult<String> {
    match &args.command {
        KeyboardCommand::Type(type_args) => run_type(runtime, type_args),
        KeyboardCommand::Press(sequence_args) => run_press(runtime, sequence_args),
        KeyboardCommand::Release(sequence_args) => run_release(runtime, sequence_args),
    }
}

fn run_type(runtime: &Runtime, args: &KeyboardTypeArgs) -> CliResult<String> {
    let sequence = args.sequence.trim();
    if sequence.is_empty() {
        anyhow::bail!("Please provide a keyboard sequence.");
    }
    let overrides = build_overrides(&args.overrides);
    runtime.keyboard_type(sequence, overrides)?;
    Ok(String::new())
}

fn run_press(runtime: &Runtime, args: &KeyboardSequenceArgs) -> CliResult<String> {
    let overrides = build_overrides(&args.overrides);
    runtime.keyboard_press(&args.sequence, overrides)?;
    Ok(String::new())
}

fn run_release(runtime: &Runtime, args: &KeyboardSequenceArgs) -> CliResult<String> {
    let overrides = build_overrides(&args.overrides);
    runtime.keyboard_release(&args.sequence, overrides)?;
    Ok(String::new())
}

fn build_overrides(args: &KeyboardOverrideArgs) -> Option<KeyboardOverrides> {
    let mut overrides = KeyboardOverrides::new();

    if let Some(delay) = args.uniform_delay {
        overrides = overrides
            .press_delay(delay)
            .release_delay(delay)
            .between_keys_delay(delay)
            .chord_press_delay(delay)
            .chord_release_delay(delay)
            .after_sequence_delay(delay)
            .after_text_delay(delay);
    }

    if let Some(delay) = args.press_delay {
        overrides = overrides.press_delay(delay);
    }
    if let Some(delay) = args.release_delay {
        overrides = overrides.release_delay(delay);
    }
    if let Some(delay) = args.between_keys_delay {
        overrides = overrides.between_keys_delay(delay);
    }
    if let Some(delay) = args.chord_press_delay {
        overrides = overrides.chord_press_delay(delay);
    }
    if let Some(delay) = args.chord_release_delay {
        overrides = overrides.chord_release_delay(delay);
    }
    if let Some(delay) = args.after_sequence_delay {
        overrides = overrides.after_sequence_delay(delay);
    }
    if let Some(delay) = args.after_text_delay {
        overrides = overrides.after_text_delay(delay);
    }

    if overrides.is_empty() { None } else { Some(overrides) }
}

fn parse_millis(value: &str) -> Result<Duration, String> {
    value.parse::<u64>().map(Duration::from_millis).map_err(|err| format!("invalid milliseconds '{value}': {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::runtime;
    use platynui_platform_mock::{KeyboardLogEntry, reset_keyboard_state, take_keyboard_log};
    use rstest::rstest;
    use serial_test::serial;

    fn entries() -> Vec<KeyboardLogEntry> {
        take_keyboard_log()
    }

    fn presses(log: &[KeyboardLogEntry]) -> Vec<String> {
        log.iter()
            .filter_map(|entry| match entry {
                KeyboardLogEntry::Press(name) => Some(name.clone()),
                _ => None,
            })
            .collect()
    }

    #[rstest]
    #[serial]
    fn type_command_executes_sequence() {
        reset_keyboard_state();
        let runtime = runtime();
        let args = KeyboardTypeArgs {
            sequence: "<Ctrl+A>Hallo".into(),
            overrides: KeyboardOverrideArgs { uniform_delay: Some(Duration::ZERO), ..Default::default() },
        };
        let output = run_type(&runtime, &args).expect("type");
        assert!(output.is_empty());
        let log = entries();
        assert!(presses(&log).contains(&"Control".to_string()));
        assert!(presses(&log).contains(&"A".to_string()));
        assert!(presses(&log).contains(&"H".to_string()));
    }

    #[rstest]
    #[serial]
    fn press_command_executes_sequence() {
        reset_keyboard_state();
        let runtime = runtime();
        let args = KeyboardSequenceArgs {
            sequence: "<Shift+Ctrl+S>".into(),
            overrides: KeyboardOverrideArgs { press_delay: Some(Duration::ZERO), ..Default::default() },
        };
        let output = run_press(&runtime, &args).expect("press");
        assert!(output.is_empty());
        let log = entries();
        let names = presses(&log);
        assert!(names.contains(&"Shift".to_string()));
        assert!(names.contains(&"Control".to_string()));
        assert!(names.contains(&"S".to_string()));
    }

    #[rstest]
    #[serial]
    fn release_command_executes_sequence() {
        reset_keyboard_state();
        let runtime = runtime();
        // keep modifiers in pressed state via explicit press first
        runtime.keyboard_press("<Ctrl+K>", None).expect("press for release test");
        let _ = entries();

        let args = KeyboardSequenceArgs { sequence: "<Ctrl+K>".into(), overrides: KeyboardOverrideArgs::default() };
        let output = run_release(&runtime, &args).expect("release");
        assert!(output.is_empty());
        let log = entries();
        assert!(log.iter().any(|entry| matches!(entry, KeyboardLogEntry::Release(name) if name == "Control")));
    }

    #[rstest]
    fn empty_sequence_is_rejected() {
        reset_keyboard_state();
        let runtime = runtime();
        let args = KeyboardTypeArgs { sequence: "   ".into(), overrides: KeyboardOverrideArgs::default() };
        let err = run_type(&runtime, &args).expect_err("empty sequence");
        assert!(err.to_string().contains("keyboard sequence"));
    }
}
