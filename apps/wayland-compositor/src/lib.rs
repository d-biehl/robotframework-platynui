//! `PlatynUI` Wayland Compositor — a controlled display server for automated UI testing on Linux.
//!
//! Provides a Wayland compositor environment where GTK, Qt, and X11 applications can run
//! under reproducible conditions. Supports headless (CI), nested (development), and
//! bare-metal (DRM) operation modes.
//!
//! This crate only builds on Linux.

#![cfg(target_os = "linux")]

mod backend;
pub mod child;
mod client;
pub mod config;
mod control;
mod cursor;
pub mod decorations;
mod environment;
mod focus;
mod grabs;
mod handlers;
mod input;
pub mod multi_output;
mod ready;
mod render;
pub mod security;
mod signals;
pub mod state;
pub mod ui;
mod workspace;

pub mod xwayland;

use clap::{Parser, ValueEnum};
use smithay::input::keyboard::XkbConfig;

/// CLI arguments for the Wayland compositor.
#[allow(clippy::struct_excessive_bools)] // CLI struct — bools are natural for flags
#[derive(Parser, Debug)]
#[command(
    name = "platynui-wayland-compositor",
    author,
    version,
    about = "Smithay-based Wayland compositor for PlatynUI",
    long_about = None
)]
pub struct CompositorArgs {
    /// Backend to use for rendering.
    #[arg(long, default_value = "headless")]
    pub backend: Backend,

    /// Width of the virtual output in pixels.
    #[arg(long, default_value_t = 1920)]
    pub width: u32,

    /// Height of the virtual output in pixels.
    #[arg(long, default_value_t = 1080)]
    pub height: u32,

    /// Wayland socket name (auto-selected if not specified).
    #[arg(long)]
    pub socket_name: Option<String>,

    /// Path to TOML configuration file.
    /// Default: `$XDG_CONFIG_HOME/platynui/compositor.toml`.
    #[arg(long)]
    pub config: Option<std::path::PathBuf>,

    /// Set the log level for diagnostic output (written to stderr).
    /// Overrides the `PLATYNUI_LOG_LEVEL` environment variable.
    /// Use `RUST_LOG` for fine-grained per-crate filtering.
    #[arg(long = "log-level", value_enum)]
    pub log_level: Option<LogLevel>,

    /// File descriptor to write `READY\n` to when the compositor is ready.
    /// Useful for systemd-notify style readiness signaling.
    #[arg(long)]
    pub ready_fd: Option<i32>,

    /// Print environment variables (`WAYLAND_DISPLAY`, etc.) on stdout when ready.
    #[arg(long)]
    pub print_env: bool,

    /// Automatically shut down after this many seconds (0 = no timeout).
    /// Useful in CI to prevent indefinite hangs.
    #[arg(long, default_value_t = 0)]
    pub timeout: u64,

    /// XKB keyboard layout(s), comma-separated (e.g. `de,us,fr`).
    /// Overrides `XKB_DEFAULT_LAYOUT`. Positionally paired with `--keyboard-variant`.
    #[arg(long)]
    pub keyboard_layout: Option<String>,

    /// XKB keyboard variant(s), comma-separated, positionally paired with layouts.
    /// Empty entries use the default variant (e.g. `nodeadkeys,,neo`).
    /// Overrides `XKB_DEFAULT_VARIANT`.
    #[arg(long)]
    pub keyboard_variant: Option<String>,

    /// XKB keyboard model (e.g. `pc105`). Overrides `XKB_DEFAULT_MODEL`.
    #[arg(long)]
    pub keyboard_model: Option<String>,

    /// XKB rules file (e.g. `evdev`). Overrides `XKB_DEFAULT_RULES`.
    #[arg(long)]
    pub keyboard_rules: Option<String>,

    /// XKB options, comma-separated (e.g. `grp:alt_shift_toggle,compose:ralt`).
    /// Overrides `XKB_DEFAULT_OPTIONS`.
    #[arg(long)]
    pub keyboard_options: Option<String>,

    /// Enable `XWayland` for running X11 applications.
    /// Requires the `xwayland` feature and the `Xwayland` binary in `$PATH`.
    #[arg(long)]
    pub xwayland: bool,

    /// Disable the test-control IPC socket.
    /// By default, a control socket is created at `$XDG_RUNTIME_DIR/<socket-name>.control`
    /// and its path is exported as `PLATYNUI_CONTROL_SOCKET` for child processes and tools.
    #[arg(long)]
    pub no_control_socket: bool,

    /// Number of virtual outputs (monitors) to create. Default: 1.
    #[arg(long, default_value_t = 1)]
    pub outputs: u32,

    /// Arrangement of multiple outputs.
    #[arg(long, default_value = "horizontal")]
    pub output_layout: multi_output::OutputLayout,

    /// Scale factor for virtual outputs (e.g. `1.0`, `1.5`, `2.0`).
    /// Applied to all outputs created via `--outputs`. For per-output
    /// scale, use the TOML configuration file.
    #[arg(long, default_value_t = 1.0)]
    pub scale: f64,

    /// Scale factor for the winit preview window (e.g. `0.5` to halve the
    /// window size). Only affects the winit backend window dimensions and
    /// rendering resolution — Wayland clients still see the real output
    /// scale/mode. Useful to fit large multi-output setups on screen.
    #[arg(long, default_value_t = 1.0)]
    pub window_scale: f64,

    /// Restrict privileged protocol access to a whitelist of app IDs.
    /// Comma-separated list (e.g. `org.kde.kate,org.gnome.Calculator`).
    /// When not set, all clients have full access (test compositor default).
    #[arg(long)]
    pub restrict_protocols: Option<String>,

    /// Shut down the compositor when the child program (specified after `--`) exits.
    /// Essential for CI pipelines: compositor starts → app starts → tests run → compositor exits.
    #[arg(long)]
    pub exit_with_child: bool,

    /// Render the mouse cursor as a software element in the frame buffer.
    ///
    /// By default (winit backend), the host window cursor is used and is not
    /// part of the rendered frame. With `--software-cursor`, the cursor is
    /// composited into every frame — making it visible in screencopy tools
    /// (wayvnc, grim) and screenshots.
    #[arg(long)]
    pub software_cursor: bool,

    /// Child program and arguments to launch after compositor readiness.
    /// Specify after `--` (e.g. `-- gtk4-demo` or `-- python -m pytest tests/`).
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    pub child_command: Vec<String>,
}

/// Supported rendering backends.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Backend {
    /// Off-screen rendering (no window, for CI).
    /// Uses `GlowRenderer` on a DRI render node; set `LIBGL_ALWAYS_SOFTWARE=1`
    /// for environments without a hardware GPU.
    Headless,
    /// Nested compositor in a winit window (for development).
    Winit,
    /// Direct hardware rendering on a TTY (DRM/KMS + libinput).
    /// Requires the `backend-drm` feature.
    Drm,
}

/// Supported log level values for the `--log-level` CLI flag.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Initialize the tracing subscriber.
///
/// Priority (highest wins):
/// 1. `RUST_LOG` environment variable (fine-grained per-crate filtering)
/// 2. `--log-level` CLI argument
/// 3. `PLATYNUI_LOG_LEVEL` environment variable
/// 4. Default: `warn`
fn init_tracing(cli_level: Option<LogLevel>) {
    use tracing_subscriber::EnvFilter;

    let filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else {
        let directive = if let Some(level) = cli_level {
            match level {
                LogLevel::Error => "error",
                LogLevel::Warn => "warn",
                LogLevel::Info => "info",
                LogLevel::Debug => "debug",
                LogLevel::Trace => "trace",
            }
            .to_string()
        } else if let Ok(val) = std::env::var("PLATYNUI_LOG_LEVEL") {
            val
        } else {
            "warn".to_string()
        };
        EnvFilter::new(directive)
    };

    tracing_subscriber::fmt().with_env_filter(filter).with_target(true).with_writer(std::io::stderr).init();
}

/// Build an `XkbConfig` from CLI flags and environment variables.
///
/// Priority: CLI flag > environment variable > XKB default.
#[must_use]
pub fn resolve_xkb_config(args: &CompositorArgs) -> XkbConfig<'_> {
    XkbConfig {
        rules: args.keyboard_rules.as_deref().unwrap_or(""),
        model: args.keyboard_model.as_deref().unwrap_or(""),
        layout: args.keyboard_layout.as_deref().unwrap_or(""),
        variant: args.keyboard_variant.as_deref().unwrap_or(""),
        options: args.keyboard_options.clone(),
    }
}

/// Read XKB environment variables into the `CompositorArgs` fields (as fallback).
///
/// Only fills fields that were not set via CLI flags.
fn apply_xkb_env_defaults(args: &mut CompositorArgs) {
    fn env_fallback(cli: &mut Option<String>, env_var: &str) {
        if cli.is_none()
            && let Ok(val) = std::env::var(env_var)
            && !val.is_empty()
        {
            *cli = Some(val);
        }
    }

    env_fallback(&mut args.keyboard_layout, "XKB_DEFAULT_LAYOUT");
    env_fallback(&mut args.keyboard_variant, "XKB_DEFAULT_VARIANT");
    env_fallback(&mut args.keyboard_model, "XKB_DEFAULT_MODEL");
    env_fallback(&mut args.keyboard_rules, "XKB_DEFAULT_RULES");
    env_fallback(&mut args.keyboard_options, "XKB_DEFAULT_OPTIONS");
}

/// Run the compositor with the given CLI arguments.
///
/// # Errors
///
/// Returns an error if the compositor fails to start or encounters a fatal runtime error.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = CompositorArgs::parse();
    init_tracing(args.log_level);
    tracing::info!("PlatynUI Wayland Compositor starting");

    // Load configuration file (CLI flag > XDG path > defaults)
    let compositor_config = config::load_config(args.config.as_deref())?;

    // Apply config-file keyboard defaults (priority: CLI > config > env > default)
    config::apply_keyboard_config_defaults(&mut args, &compositor_config.keyboard);

    // Apply XKB environment variable defaults for fields not set via CLI or config
    apply_xkb_env_defaults(&mut args);

    if args.keyboard_layout.is_some() || args.keyboard_variant.is_some() {
        tracing::info!(
            layout = args.keyboard_layout.as_deref().unwrap_or("(default)"),
            variant = args.keyboard_variant.as_deref().unwrap_or("(default)"),
            model = args.keyboard_model.as_deref().unwrap_or("(default)"),
            options = args.keyboard_options.as_deref().unwrap_or("(none)"),
            "keyboard layout configured",
        );
    }

    // Ensure XDG_RUNTIME_DIR exists
    environment::ensure_xdg_runtime_dir()?;

    match args.backend {
        Backend::Headless => backend::headless::run(&args, compositor_config),
        Backend::Winit => backend::winit::run(&args, compositor_config),
        Backend::Drm => backend::drm::run(&args, compositor_config),
    }
}
