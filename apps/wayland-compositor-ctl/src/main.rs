//! CLI tool for controlling a running `PlatynUI` Wayland compositor.
//!
//! Connects to the compositor's test-control Unix socket and sends
//! JSON commands. Analogous to `swaymsg` or `hyprctl`.
//!
//! # Usage
//!
//! ```text
//! platynui-wayland-compositor-ctl status
//! platynui-wayland-compositor-ctl list-windows
//! platynui-wayland-compositor-ctl get-window firefox
//! platynui-wayland-compositor-ctl focus 0
//! platynui-wayland-compositor-ctl close firefox
//! platynui-wayland-compositor-ctl screenshot
//! platynui-wayland-compositor-ctl screenshot -o my-screenshot.png
//! platynui-wayland-compositor-ctl --json list-windows
//! platynui-wayland-compositor-ctl shutdown
//! ```
//!
//! # Window Identifiers
//!
//! Window commands accept flexible identifiers:
//! - A **number** (e.g. `0`, `2`) refers to the window index from `list-windows`
//! - A **string** (e.g. `firefox`, `foot`) matches first by `app_id` (exact),
//!   then by window title (case-insensitive substring)
//!
//! # Socket Discovery
//!
//! 1. `--socket <path>` — explicit socket path
//! 2. `$PLATYNUI_CONTROL_SOCKET` — set by the compositor
//! 3. `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY.control` — derived from environment

use std::io::{BufRead, BufReader, IsTerminal, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

/// CLI tool for controlling a running `PlatynUI` Wayland compositor.
#[derive(Parser)]
#[command(name = "platynui-wayland-compositor-ctl", about = "Control a running PlatynUI Wayland compositor", version)]
struct Cli {
    /// Path to the compositor control socket.
    #[arg(long, short)]
    socket: Option<PathBuf>,

    /// Output raw JSON instead of human-readable format.
    #[arg(long, short)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show compositor status (version, uptime, backend, windows, outputs).
    Status,

    /// Health check (alias for `status`).
    Ping,

    /// List all mapped windows.
    ListWindows,

    /// Get details of a specific window.
    GetWindow {
        /// Window identifier: index number, `app_id`, or title substring.
        window: String,
    },

    /// Focus a window.
    Focus {
        /// Window identifier: index number, `app_id`, or title substring.
        window: String,
    },

    /// Close a window.
    Close {
        /// Window identifier: index number, `app_id`, or title substring.
        window: String,
    },

    /// Take a screenshot (PNG).
    Screenshot {
        /// Output file path.
        /// Default: `screenshot-<timestamp>.png` in current directory.
        #[arg(long, short)]
        output: Option<PathBuf>,
    },

    /// Request a graceful compositor shutdown.
    Shutdown,
}

// ---------------------------------------------------------------------------
// Terminal colors
// ---------------------------------------------------------------------------

/// ANSI color helper with TTY detection.
struct Style {
    enabled: bool,
}

impl Style {
    fn new() -> Self {
        Self { enabled: std::io::stdout().is_terminal() }
    }

    fn bold(&self, s: &str) -> String {
        if self.enabled { format!("\x1b[1m{s}\x1b[0m") } else { s.to_string() }
    }

    fn green(&self, s: &str) -> String {
        if self.enabled { format!("\x1b[32m{s}\x1b[0m") } else { s.to_string() }
    }

    fn red(&self, s: &str) -> String {
        if self.enabled { format!("\x1b[31m{s}\x1b[0m") } else { s.to_string() }
    }

    fn cyan(&self, s: &str) -> String {
        if self.enabled { format!("\x1b[36m{s}\x1b[0m") } else { s.to_string() }
    }

    fn dim(&self, s: &str) -> String {
        if self.enabled { format!("\x1b[2m{s}\x1b[0m") } else { s.to_string() }
    }

    fn green_bold(&self, s: &str) -> String {
        if self.enabled { format!("\x1b[1;32m{s}\x1b[0m") } else { s.to_string() }
    }

    fn yellow_bold(&self, s: &str) -> String {
        if self.enabled { format!("\x1b[1;33m{s}\x1b[0m") } else { s.to_string() }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> ExitCode {
    let cli = Cli::parse();

    let Some(socket_path) = cli.socket.clone().or_else(discover_socket_path) else {
        let s = Style::new();
        eprintln!("{} cannot determine control socket path", s.red("error:"));
        eprintln!("  {} set WAYLAND_DISPLAY or use --socket <path>", s.dim("hint:"));
        return ExitCode::FAILURE;
    };

    let stream = match UnixStream::connect(&socket_path) {
        Ok(s) => s,
        Err(err) => {
            let s = Style::new();
            eprintln!("{} cannot connect to {}: {err}", s.red("error:"), socket_path.display());
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = stream.set_read_timeout(Some(std::time::Duration::from_secs(10))) {
        eprintln!("warning: failed to set read timeout: {err}");
    }
    if let Err(err) = stream.set_write_timeout(Some(std::time::Duration::from_secs(5))) {
        eprintln!("warning: failed to set write timeout: {err}");
    }

    match execute_command(&cli, &stream) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let s = Style::new();
            eprintln!("{} {err}", s.red("error:"));
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// Socket discovery
// ---------------------------------------------------------------------------

/// Discover the control socket path from environment variables.
fn discover_socket_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PLATYNUI_CONTROL_SOCKET") {
        return Some(PathBuf::from(path));
    }
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok()?;
    Some(PathBuf::from(runtime_dir).join(format!("{wayland_display}.control")))
}

// ---------------------------------------------------------------------------
// Command execution
// ---------------------------------------------------------------------------

/// Send a command to the compositor and handle the response.
fn execute_command(cli: &Cli, stream: &UnixStream) -> Result<(), Box<dyn std::error::Error>> {
    let json_command = build_command_json(&cli.command);

    let mut writer = stream;
    writeln!(writer, "{json_command}")?;
    writer.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;

    let response: serde_json::Value = serde_json::from_str(response_line.trim())?;

    let status = response.get("status").and_then(serde_json::Value::as_str).unwrap_or("unknown");

    if status == "error" {
        let msg = response.get("message").and_then(serde_json::Value::as_str).unwrap_or("unknown error");
        return Err(msg.into());
    }

    handle_response(cli, &response)
}

/// Build the JSON command string for a given CLI command.
fn build_command_json(command: &Command) -> String {
    let value = match command {
        Command::Status | Command::Ping => serde_json::json!({"command": "status"}),
        Command::ListWindows => serde_json::json!({"command": "list_windows"}),
        Command::GetWindow { window } => window_command_json("get_window", window),
        Command::Focus { window } => window_command_json("focus_window", window),
        Command::Close { window } => window_command_json("close_window", window),
        Command::Screenshot { .. } => serde_json::json!({"command": "screenshot"}),
        Command::Shutdown => serde_json::json!({"command": "shutdown"}),
    };
    value.to_string()
}

/// Build a JSON command with a window selector (id, `app_id`, or title).
///
/// Numeric values → `"id":<n>`, strings → `"app_id":"...","title":"..."`.
fn window_command_json(command: &str, window: &str) -> serde_json::Value {
    if let Ok(id) = window.parse::<u64>() {
        serde_json::json!({"command": command, "id": id})
    } else {
        serde_json::json!({"command": command, "app_id": window, "title": window})
    }
}

// ---------------------------------------------------------------------------
// Response formatting
// ---------------------------------------------------------------------------

/// Handle the response based on command type.
fn handle_response(cli: &Cli, response: &serde_json::Value) -> Result<(), Box<dyn std::error::Error>> {
    let s = Style::new();

    match &cli.command {
        Command::Status | Command::Ping => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(response)?);
            } else {
                print_status(response, &s);
            }
        }

        Command::ListWindows => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(response)?);
            } else {
                print_window_list(response, &s);
            }
        }

        Command::GetWindow { .. } => {
            if cli.json {
                if let Some(window) = response.get("window") {
                    println!("{}", serde_json::to_string_pretty(window)?);
                }
            } else {
                print_window_detail(response, &s);
            }
        }

        Command::Screenshot { output } => {
            let data = response.get("data").and_then(serde_json::Value::as_str).ok_or("missing screenshot data")?;
            let width = response.get("width").and_then(serde_json::Value::as_i64).unwrap_or(0);
            let height = response.get("height").and_then(serde_json::Value::as_i64).unwrap_or(0);
            let scale = response.get("scale").and_then(serde_json::Value::as_f64).unwrap_or(1.0);

            if cli.json {
                // In JSON mode, print the full response (with base64 data)
                println!("{}", serde_json::to_string_pretty(response)?);
            } else {
                let path = output.clone().unwrap_or_else(generate_screenshot_filename);
                let png_bytes = base64_decode(data)?;
                std::fs::write(&path, &png_bytes)?;
                println!(
                    "{} Saved screenshot to {} ({width}\u{00d7}{height} @ {scale}x, {} bytes)",
                    s.green_bold("\u{2713}"),
                    s.bold(&path.display().to_string()),
                    png_bytes.len(),
                );
            }
        }

        Command::Focus { window } => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(response)?);
            } else {
                let title = response.get("title").and_then(serde_json::Value::as_str).unwrap_or("");
                let app_id = response.get("app_id").and_then(serde_json::Value::as_str).unwrap_or("");
                println!(
                    "{} Focused window {} {}",
                    s.green_bold("\u{2713}"),
                    format_window_identifier(window, title, app_id, &s),
                    s.dim(&format!("({app_id})")),
                );
            }
        }

        Command::Close { window } => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(response)?);
            } else {
                let title = response.get("title").and_then(serde_json::Value::as_str).unwrap_or("");
                let app_id = response.get("app_id").and_then(serde_json::Value::as_str).unwrap_or("");
                println!(
                    "{} Closed window {} {}",
                    s.green_bold("\u{2713}"),
                    format_window_identifier(window, title, app_id, &s),
                    s.dim(&format!("({app_id})")),
                );
            }
        }

        Command::Shutdown => {
            if cli.json {
                println!("{}", serde_json::to_string_pretty(response)?);
            } else {
                println!("{} Compositor is shutting down", s.yellow_bold("\u{26a0}"));
            }
        }
    }
    Ok(())
}

/// Format a window identifier for display in action messages.
fn format_window_identifier(input: &str, title: &str, app_id: &str, s: &Style) -> String {
    if title.is_empty() && app_id.is_empty() {
        return s.bold(input);
    }
    if title.is_empty() {
        return s.bold(app_id);
    }
    s.bold(&format!("\"{title}\""))
}

// ---------------------------------------------------------------------------
// Human-readable output
// ---------------------------------------------------------------------------

/// Print compositor status.
fn print_status(response: &serde_json::Value, style: &Style) {
    let version = response.get("version").and_then(serde_json::Value::as_str).unwrap_or("unknown");
    let backend = response.get("backend").and_then(serde_json::Value::as_str).unwrap_or("unknown");
    let uptime = response.get("uptime_secs").and_then(serde_json::Value::as_u64).unwrap_or(0);
    let socket = response.get("socket").and_then(serde_json::Value::as_str).unwrap_or("unknown");
    let xwayland = response.get("xwayland").and_then(serde_json::Value::as_bool).unwrap_or(false);
    let windows = response.get("windows").and_then(serde_json::Value::as_u64).unwrap_or(0);
    let minimized = response.get("minimized").and_then(serde_json::Value::as_u64).unwrap_or(0);

    println!("{}", style.bold(&format!("PlatynUI Compositor v{version}")));
    println!("  {:<12} {backend}", style.dim("Backend:"));
    println!("  {:<12} {}", style.dim("Uptime:"), format_uptime(uptime));
    println!("  {:<12} {socket}", style.dim("Socket:"));
    println!("  {:<12} {}", style.dim("XWayland:"), if xwayland { style.green("yes") } else { style.dim("no") });
    if minimized > 0 {
        println!("  {:<12} {windows} ({minimized} minimized)", style.dim("Windows:"));
    } else {
        println!("  {:<12} {windows}", style.dim("Windows:"));
    }

    if let Some(outputs) = response.get("outputs").and_then(serde_json::Value::as_array) {
        println!("  {:<12} {}", style.dim("Outputs:"), outputs.len());
        for output in outputs {
            let name = output.get("name").and_then(serde_json::Value::as_str).unwrap_or("?");
            let out_w = output.get("width").and_then(serde_json::Value::as_i64).unwrap_or(0);
            let out_h = output.get("height").and_then(serde_json::Value::as_i64).unwrap_or(0);
            let out_x = output.get("x").and_then(serde_json::Value::as_i64).unwrap_or(0);
            let out_y = output.get("y").and_then(serde_json::Value::as_i64).unwrap_or(0);
            let scale = output.get("scale").and_then(serde_json::Value::as_f64).unwrap_or(1.0);
            println!(
                "    {} {:<10} {out_w}\u{00d7}{out_h}+{out_x}+{out_y}  scale {scale}",
                style.cyan(&format!("#{}", output.get("index").and_then(serde_json::Value::as_u64).unwrap_or(0))),
                name,
            );
        }
    }
}

/// Print the window list in a human-readable table.
fn print_window_list(response: &serde_json::Value, s: &Style) {
    if let Some(windows) = response.get("windows").and_then(serde_json::Value::as_array) {
        if windows.is_empty() {
            println!("{}", s.dim("No windows"));
            return;
        }

        // Determine column widths
        let mut max_app_id: usize = 6; // "App ID"
        let mut max_title: usize = 5; // "Title"
        for w in windows {
            let app_id = w.get("app_id").and_then(serde_json::Value::as_str).unwrap_or("");
            let title = w.get("title").and_then(serde_json::Value::as_str).unwrap_or("");
            max_app_id = max_app_id.max(app_id.len());
            max_title = max_title.max(title.len());
        }
        // Cap column widths for readability
        max_app_id = max_app_id.min(30);
        max_title = max_title.min(40);

        // Header
        println!(
            "  {} {:<max_app_id$}  {:<max_title$}  {:<12}  {}",
            s.bold("ID"),
            s.bold("App ID"),
            s.bold("Title"),
            s.bold("Size"),
            s.bold("State"),
        );

        // Separator
        let sep_len = 4 + max_app_id + 2 + max_title + 2 + 12 + 2 + 20;
        println!("  {}", s.dim(&"\u{2500}".repeat(sep_len)));

        for w in windows {
            let id = w.get("id").and_then(serde_json::Value::as_u64).unwrap_or(0);
            let app_id = w.get("app_id").and_then(serde_json::Value::as_str).unwrap_or("");
            let title = w.get("title").and_then(serde_json::Value::as_str).unwrap_or("");
            let width = w.get("width").and_then(serde_json::Value::as_i64).unwrap_or(0);
            let height = w.get("height").and_then(serde_json::Value::as_i64).unwrap_or(0);
            let focused = w.get("focused").and_then(serde_json::Value::as_bool).unwrap_or(false);
            let maximized = w.get("maximized").and_then(serde_json::Value::as_bool).unwrap_or(false);
            let fullscreen = w.get("fullscreen").and_then(serde_json::Value::as_bool).unwrap_or(false);

            // Truncate long fields
            let app_id_display = truncate(app_id, max_app_id);
            let title_display = truncate(title, max_title);
            let size = format!("{width}\u{00d7}{height}");

            let mut state_parts = Vec::new();
            if focused {
                state_parts.push(s.green_bold("\u{25cf} focused"));
            }
            if maximized {
                state_parts.push("maximized".to_string());
            }
            if fullscreen {
                state_parts.push("fullscreen".to_string());
            }
            let state_str = state_parts.join(", ");

            println!(
                "  {:<2} {:<max_app_id$}  {:<max_title$}  {:<12}  {}",
                s.cyan(&id.to_string()),
                app_id_display,
                title_display,
                size,
                state_str,
            );
        }
    }

    // Also show minimized windows if present
    if let Some(minimized) = response.get("minimized").and_then(serde_json::Value::as_array).filter(|a| !a.is_empty()) {
        println!();
        println!("  {} ({} minimized)", s.dim("Hidden windows"), minimized.len());
        for min_win in minimized {
            let id = min_win.get("id").and_then(serde_json::Value::as_str).unwrap_or("?");
            let app_id = min_win.get("app_id").and_then(serde_json::Value::as_str).unwrap_or("");
            let title = min_win.get("title").and_then(serde_json::Value::as_str).unwrap_or("");
            println!("    {} {:<20}  {}", s.dim(id), app_id, s.dim(title));
        }
    }
}

/// Print window detail in human-readable form.
fn print_window_detail(response: &serde_json::Value, s: &Style) {
    let Some(w) = response.get("window") else { return };

    let id = w.get("id").and_then(serde_json::Value::as_u64).unwrap_or(0);
    let app_id = w.get("app_id").and_then(serde_json::Value::as_str).unwrap_or("");
    let title = w.get("title").and_then(serde_json::Value::as_str).unwrap_or("");
    let x = w.get("x").and_then(serde_json::Value::as_i64).unwrap_or(0);
    let y = w.get("y").and_then(serde_json::Value::as_i64).unwrap_or(0);
    let width = w.get("width").and_then(serde_json::Value::as_i64).unwrap_or(0);
    let height = w.get("height").and_then(serde_json::Value::as_i64).unwrap_or(0);
    let focused = w.get("focused").and_then(serde_json::Value::as_bool).unwrap_or(false);
    let maximized = w.get("maximized").and_then(serde_json::Value::as_bool).unwrap_or(false);
    let fullscreen = w.get("fullscreen").and_then(serde_json::Value::as_bool).unwrap_or(false);

    println!("{}", s.bold(&format!("Window #{id}")));
    println!("  {:<12} {title}", s.dim("Title:"));
    println!("  {:<12} {app_id}", s.dim("App ID:"));
    println!("  {:<12} {x},{y}", s.dim("Position:"));
    println!("  {:<12} {width}\u{00d7}{height}", s.dim("Size:"));

    let mut states = Vec::new();
    if focused {
        states.push(s.green("focused"));
    }
    if maximized {
        states.push("maximized".to_string());
    }
    if fullscreen {
        states.push("fullscreen".to_string());
    }
    if states.is_empty() {
        states.push(s.dim("normal"));
    }
    println!("  {:<12} {}", s.dim("State:"), states.join(", "));
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Truncate a string to `max_len`, appending "\u{2026}" if truncated.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else if max_len > 1 {
        format!("{}\u{2026}", &s[..max_len - 1])
    } else {
        "\u{2026}".to_string()
    }
}

/// Format an uptime duration in human-readable form.
fn format_uptime(total_secs: u64) -> String {
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let secs = total_secs % 60;

    if hours > 0 {
        format!("{hours}h {minutes}m {secs}s")
    } else if minutes > 0 {
        format!("{minutes}m {secs}s")
    } else {
        format!("{secs}s")
    }
}

/// Generate a default screenshot filename with a timestamp.
fn generate_screenshot_filename() -> PathBuf {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();

    // Compute UTC date/time components using Howard Hinnant's civil_from_days algorithm.
    let days = secs / 86_400;
    let day_secs = secs % 86_400;
    let hh = day_secs / 3600;
    let mm = (day_secs % 3600) / 60;
    let ss = day_secs % 60;

    #[allow(clippy::cast_possible_wrap)]
    let z = days as i64 + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    #[allow(clippy::cast_sign_loss)]
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    #[allow(clippy::cast_possible_wrap)]
    let y_raw = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y_raw + 1 } else { y_raw };

    PathBuf::from(format!("screenshot-{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}.png"))
}

/// Decode a base64 string (RFC 4648) into bytes.
fn base64_decode(input: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    const DECODE_TABLE: [u8; 256] = {
        let mut table = [255u8; 256];
        let mut i = 0u8;
        while i < 26 {
            table[(b'A' + i) as usize] = i;
            table[(b'a' + i) as usize] = i + 26;
            i += 1;
        }
        let mut d = 0u8;
        while d < 10 {
            table[(b'0' + d) as usize] = d + 52;
            d += 1;
        }
        table[b'+' as usize] = 62;
        table[b'/' as usize] = 63;
        table
    };

    let input: Vec<u8> = input.bytes().filter(|&b| b != b'=' && b != b'\n' && b != b'\r').collect();
    let mut output = Vec::with_capacity(input.len() * 3 / 4);

    for chunk in input.chunks(4) {
        let mut buf = [0u32; 4];
        for (i, &byte) in chunk.iter().enumerate() {
            let val = DECODE_TABLE[byte as usize];
            if val == 255 {
                return Err(format!("invalid base64 character: {}", byte as char).into());
            }
            buf[i] = u32::from(val);
        }

        let triple = (buf[0] << 18) | (buf[1] << 12) | (buf[2] << 6) | buf[3];

        #[allow(clippy::cast_possible_truncation)]
        {
            output.push((triple >> 16) as u8);
            if chunk.len() > 2 {
                output.push((triple >> 8) as u8);
            }
            if chunk.len() > 3 {
                output.push(triple as u8);
            }
        }
    }

    Ok(output)
}
