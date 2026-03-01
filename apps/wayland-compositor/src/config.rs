//! Configuration file support — TOML-based persistent compositor settings.
//!
//! Path discovery (highest priority first):
//! 1. `--config <path>` CLI flag
//! 2. `$XDG_CONFIG_HOME/platynui/compositor.toml`
//! 3. `~/.config/platynui/compositor.toml` (if `XDG_CONFIG_HOME` unset)
//! 4. Built-in defaults (no file needed)
//!
//! CLI flags always override config file values.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Top-level compositor configuration, deserialized from TOML.
///
/// All fields are optional — missing values use built-in defaults.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct CompositorConfig {
    /// Font settings for compositor-rendered UI (titlebars, panel).
    pub font: FontConfig,

    /// Theme colors for window decorations and panel.
    pub theme: ThemeConfig,

    /// Keyboard layout and options.
    pub keyboard: KeyboardConfig,

    /// Output (monitor) definitions — overrides `--outputs`/`--width`/`--height`.
    #[serde(default)]
    pub output: Vec<OutputConfig>,
}

/// Font configuration for compositor-rendered text.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct FontConfig {
    /// Font family name. Resolved via fontconfig at runtime.
    /// Falls back to egui's built-in font if not found.
    pub family: String,

    /// Font size in logical pixels.
    pub size: f32,
}

impl Default for FontConfig {
    fn default() -> Self {
        Self { family: "Noto Sans".to_string(), size: 13.0 }
    }
}

/// Theme colors for window decorations.
///
/// Colors are specified as CSS-style hex strings (`#rrggbb` or `#rrggbbaa`).
/// Invalid values fall back to the built-in defaults.
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct ThemeConfig {
    /// Title bar background for inactive windows.
    pub titlebar_background: String,

    /// Title bar background for the focused window.
    pub titlebar_background_focused: String,

    /// Title bar text color.
    pub titlebar_text: String,

    /// Close button color.
    pub button_close: String,

    /// Maximize button color.
    pub button_maximize: String,

    /// Minimize button color.
    pub button_minimize: String,

    /// Border color for the focused window.
    pub active_border: String,

    /// Border color for unfocused windows.
    pub inactive_border: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            titlebar_background: "#33333f".to_string(),
            titlebar_background_focused: "#404d73".to_string(),
            titlebar_text: "#ffffff".to_string(),
            button_close: "#6b4444".to_string(),
            button_maximize: "#44634a".to_string(),
            button_minimize: "#635c3a".to_string(),
            active_border: "#7380b3".to_string(),
            inactive_border: "#595966".to_string(),
        }
    }
}

impl ThemeConfig {
    /// Parse a hex color string (`#rrggbb` or `#rrggbbaa`) into `[f32; 4]` (RGBA, 0.0–1.0).
    ///
    /// Returns `None` if the string is not a valid hex color.
    #[must_use]
    pub fn parse_color(hex: &str) -> Option<[f32; 4]> {
        let hex = hex.strip_prefix('#')?;
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, 1.0])
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some([f32::from(r) / 255.0, f32::from(g) / 255.0, f32::from(b) / 255.0, f32::from(a) / 255.0])
            }
            _ => None,
        }
    }

    /// Get the titlebar background color as `[f32; 4]` (RGBA).
    #[must_use]
    pub fn titlebar_background_rgba(&self) -> [f32; 4] {
        Self::parse_color(&self.titlebar_background).unwrap_or([0.20, 0.20, 0.25, 1.0])
    }

    /// Get the focused titlebar background color as `[f32; 4]` (RGBA).
    #[must_use]
    pub fn titlebar_background_focused_rgba(&self) -> [f32; 4] {
        Self::parse_color(&self.titlebar_background_focused).unwrap_or([0.25, 0.30, 0.45, 1.0])
    }

    /// Get the titlebar text color as `[f32; 4]` (RGBA).
    #[must_use]
    pub fn titlebar_text_rgba(&self) -> [f32; 4] {
        Self::parse_color(&self.titlebar_text).unwrap_or([1.0, 1.0, 1.0, 1.0])
    }

    /// Get the close button color as `[f32; 4]` (RGBA).
    #[must_use]
    pub fn button_close_rgba(&self) -> [f32; 4] {
        Self::parse_color(&self.button_close).unwrap_or([0.85, 0.25, 0.25, 1.0])
    }

    /// Get the maximize button color as `[f32; 4]` (RGBA).
    #[must_use]
    pub fn button_maximize_rgba(&self) -> [f32; 4] {
        Self::parse_color(&self.button_maximize).unwrap_or([0.25, 0.75, 0.35, 1.0])
    }

    /// Get the minimize button color as `[f32; 4]` (RGBA).
    #[must_use]
    pub fn button_minimize_rgba(&self) -> [f32; 4] {
        Self::parse_color(&self.button_minimize).unwrap_or([0.90, 0.75, 0.20, 1.0])
    }

    /// Get the active (focused) border color as `[f32; 4]` (RGBA).
    #[must_use]
    pub fn active_border_rgba(&self) -> [f32; 4] {
        Self::parse_color(&self.active_border).unwrap_or([0.45, 0.50, 0.70, 1.0])
    }

    /// Get the inactive border color as `[f32; 4]` (RGBA).
    #[must_use]
    pub fn inactive_border_rgba(&self) -> [f32; 4] {
        Self::parse_color(&self.inactive_border).unwrap_or([0.35, 0.35, 0.40, 1.0])
    }
}

/// Keyboard layout configuration.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct KeyboardConfig {
    /// XKB keyboard model (e.g. `pc105`).
    pub model: Option<String>,

    /// XKB rules file (e.g. `evdev`).
    pub rules: Option<String>,

    /// XKB options, comma-separated (e.g. `grp:alt_shift_toggle,compose:ralt`).
    pub options: Option<String>,

    /// Keyboard layouts — each entry specifies a layout name and optional variant.
    #[serde(default)]
    pub layout: Vec<KeyboardLayoutEntry>,
}

/// A single keyboard layout entry in the config.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct KeyboardLayoutEntry {
    /// XKB layout name (e.g. `de`, `us`, `fr`).
    pub name: String,

    /// XKB variant (e.g. `nodeadkeys`, `neo`). Empty = default variant.
    #[serde(default)]
    pub variant: Option<String>,
}

/// Configuration for a single virtual output (monitor).
#[derive(Debug, Clone, Deserialize)]
#[serde(default, rename_all = "kebab-case")]
pub struct OutputConfig {
    /// Width in pixels.
    pub width: u32,

    /// Height in pixels.
    pub height: u32,

    /// X position in the combined output space.
    pub x: i32,

    /// Y position in the combined output space.
    pub y: i32,

    /// Output scale factor (e.g. `1.0`, `1.5`, `2.0`).
    pub scale: f64,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self { width: 1920, height: 1080, x: 0, y: 0, scale: 1.0 }
    }
}

/// Discover the config file path.
///
/// Returns `None` if no config file exists (which is fine — defaults are used).
fn discover_config_path(cli_path: Option<&Path>) -> Option<PathBuf> {
    // 1. Explicit CLI path
    if let Some(path) = cli_path {
        if path.exists() {
            return Some(path.to_path_buf());
        }
        tracing::warn!(path = %path.display(), "config file specified via --config does not exist");
        return None;
    }

    // 2. $XDG_CONFIG_HOME/platynui/compositor.toml
    let config_dir = std::env::var("XDG_CONFIG_HOME")
        .map_or_else(
            |_| {
                // 3. ~/.config/platynui/compositor.toml
                let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
                PathBuf::from(home).join(".config")
            },
            PathBuf::from,
        )
        .join("platynui")
        .join("compositor.toml");

    if config_dir.exists() { Some(config_dir) } else { None }
}

/// Load the compositor configuration.
///
/// Reads the TOML file (if found), otherwise returns built-in defaults.
///
/// # Errors
///
/// Returns an error if the config file exists but cannot be read or parsed.
pub fn load_config(cli_path: Option<&Path>) -> Result<CompositorConfig, Box<dyn std::error::Error>> {
    let Some(path) = discover_config_path(cli_path) else {
        tracing::debug!("no config file found, using built-in defaults");
        return Ok(CompositorConfig::default());
    };

    tracing::info!(path = %path.display(), "loading config file");
    let content = std::fs::read_to_string(&path)
        .map_err(|err| format!("failed to read config file {}: {err}", path.display()))?;

    let config: CompositorConfig =
        toml::from_str(&content).map_err(|err| format!("failed to parse config file {}: {err}", path.display()))?;

    tracing::debug!(?config, "config loaded");
    Ok(config)
}

/// Apply config-file keyboard settings as defaults for CLI args that weren't set.
///
/// This implements the priority: CLI flag > config file > environment variable > default.
pub fn apply_keyboard_config_defaults(args: &mut super::CompositorArgs, keyboard: &KeyboardConfig) {
    fn config_fallback(cli: &mut Option<String>, config_val: Option<&String>) {
        if cli.is_none()
            && let Some(val) = config_val
        {
            *cli = Some(val.clone());
        }
    }

    config_fallback(&mut args.keyboard_model, keyboard.model.as_ref());
    config_fallback(&mut args.keyboard_rules, keyboard.rules.as_ref());
    config_fallback(&mut args.keyboard_options, keyboard.options.as_ref());

    // Build layout/variant strings from [[keyboard.layout]] entries
    if !keyboard.layout.is_empty() {
        if args.keyboard_layout.is_none() {
            let layouts: Vec<&str> = keyboard.layout.iter().map(|l| l.name.as_str()).collect();
            args.keyboard_layout = Some(layouts.join(","));
        }
        if args.keyboard_variant.is_none() {
            let variants: Vec<&str> = keyboard.layout.iter().map(|l| l.variant.as_deref().unwrap_or("")).collect();
            args.keyboard_variant = Some(variants.join(","));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_color_rgb() {
        let c = ThemeConfig::parse_color("#ff8040").unwrap();
        assert!((c[0] - 1.0).abs() < 0.01);
        assert!((c[1] - 0.502).abs() < 0.01);
        assert!((c[2] - 0.251).abs() < 0.01);
        assert!((c[3] - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_parse_color_rgba() {
        let c = ThemeConfig::parse_color("#ff804080").unwrap();
        assert!((c[3] - 0.502).abs() < 0.01);
    }

    #[test]
    fn test_parse_color_invalid() {
        assert!(ThemeConfig::parse_color("invalid").is_none());
        assert!(ThemeConfig::parse_color("#xyz").is_none());
        assert!(ThemeConfig::parse_color("#12345").is_none());
    }

    #[test]
    fn test_default_config_roundtrip() {
        let config = CompositorConfig::default();
        // All default theme colors should parse successfully
        assert!(ThemeConfig::parse_color(&config.theme.titlebar_background).is_some());
        assert!(ThemeConfig::parse_color(&config.theme.button_close).is_some());
        assert!(ThemeConfig::parse_color(&config.theme.active_border).is_some());
    }

    #[test]
    fn test_deserialize_minimal_toml() {
        let toml_str = r#"
            [font]
            family = "DejaVu Sans"
        "#;
        let config: CompositorConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.font.family, "DejaVu Sans");
        assert!((config.font.size - 13.0).abs() < f32::EPSILON); // default
    }

    #[test]
    fn test_deserialize_full_toml() {
        let toml_str = r##"
            [font]
            family = "Noto Sans"
            size = 14.0

            [theme]
            titlebar-background = "#3c3c3c"
            button-close = "#e06c75"

            [keyboard]
            model = "pc105"
            options = "grp:alt_shift_toggle"

            [[keyboard.layout]]
            name = "de"
            variant = "nodeadkeys"

            [[keyboard.layout]]
            name = "us"

            [[output]]
            width = 1920
            height = 1080
            x = 0
            y = 0
            scale = 1.0

            [[output]]
            width = 2560
            height = 1440
            x = 1920
            y = 0
            scale = 1.5
        "##;
        let config: CompositorConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.font.family, "Noto Sans");
        assert!((config.font.size - 14.0).abs() < f32::EPSILON);
        assert_eq!(config.keyboard.layout.len(), 2);
        assert_eq!(config.keyboard.layout[0].name, "de");
        assert_eq!(config.keyboard.layout[0].variant.as_deref(), Some("nodeadkeys"));
        assert_eq!(config.keyboard.layout[1].name, "us");
        assert!(config.keyboard.layout[1].variant.is_none());
        assert_eq!(config.output.len(), 2);
        assert_eq!(config.output[1].width, 2560);
        assert_eq!(config.output[1].x, 1920);
    }
}
