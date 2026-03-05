//! CLI entry point, tracing setup, and EI protocol interaction.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::LazyLock;
use std::time::{Duration, Instant};

use anyhow::{Context, anyhow};
use clap::{Parser, Subcommand, ValueEnum};
use enumflags2::BitFlags;
use reis::PendingRequestResult;
use reis::ei;
use reis::event::{DeviceCapability, EiEvent, EiEventConverter};
use rustix::event::{PollFd, PollFlags, poll};
use rustix::time::Timespec;

use crate::portal;

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

/// Standalone EIS test client for validating EI protocol against compositors.
#[derive(Parser)]
#[command(name = "platynui-eis-test-client", version)]
struct Cli {
    /// Connect via XDG Desktop Portal (for GNOME/Mutter, KDE/KWin).
    #[arg(long, group = "connection")]
    portal: bool,

    /// Connect to a specific EIS socket path.
    #[arg(long, short, group = "connection")]
    socket: Option<PathBuf>,

    /// Log level (overridden by `RUST_LOG` if set).
    #[arg(long = "log-level", value_enum, global = true)]
    log_level: Option<LogLevel>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, ValueEnum)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Subcommand)]
enum Command {
    /// Probe the EIS server: show seats, capabilities, devices, regions, and keymap info.
    Probe,

    /// Move pointer to an absolute position.
    MoveTo {
        /// X coordinate (logical pixels).
        x: f32,
        /// Y coordinate (logical pixels).
        y: f32,
    },

    /// Move pointer by a relative delta.
    MoveBy {
        /// Delta X (logical pixels).
        dx: f32,
        /// Delta Y (logical pixels).
        dy: f32,
    },

    /// Click a mouse button (press + release).
    Click {
        /// Button name: left, right, or middle (default: left).
        #[arg(default_value = "left")]
        button: String,
    },

    /// Scroll by delta.
    Scroll {
        /// Horizontal scroll delta.
        dx: f32,
        /// Vertical scroll delta.
        dy: f32,
    },

    /// Press and release a key (or shortcut like ctrl+a, alt+f4).
    Key {
        /// Key name (e.g. `a`, `enter`, `f4`), shortcut (`ctrl+a`, `alt+f4`,
        /// `ctrl+shift+delete`), or raw evdev keycode (`30`).
        key: String,
    },

    /// Tap at a position (touch down + up).
    Tap {
        /// X coordinate (logical pixels).
        x: f32,
        /// Y coordinate (logical pixels).
        y: f32,
    },

    /// Touch down at a position (use with touch-move/touch-up for gestures).
    TouchDown {
        /// Touch point ID (default: 0).
        #[arg(long, default_value_t = 0)]
        id: u32,
        /// X coordinate (logical pixels).
        x: f32,
        /// Y coordinate (logical pixels).
        y: f32,
    },

    /// Move an active touch point.
    TouchMove {
        /// Touch point ID (default: 0).
        #[arg(long, default_value_t = 0)]
        id: u32,
        /// X coordinate (logical pixels).
        x: f32,
        /// Y coordinate (logical pixels).
        y: f32,
    },

    /// Lift a touch point.
    TouchUp {
        /// Touch point ID (default: 0).
        #[arg(long, default_value_t = 0)]
        id: u32,
    },

    /// Interactive mode: connect once and run multiple commands from stdin.
    Interactive,

    /// Delete the stored portal restore token (forces re-authentication on next use).
    ResetToken,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run() -> ExitCode {
    let cli = Cli::parse();
    init_tracing(cli.log_level);

    // Handle reset-token before trying to connect
    if matches!(cli.command, Command::ResetToken) {
        return match cmd_reset_token() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e:#}");
                ExitCode::FAILURE
            }
        };
    }

    if let Err(err) = execute(&cli) {
        eprintln!("error: {err:#}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn execute(cli: &Cli) -> anyhow::Result<()> {
    // Establish connection (keep _dbus_guard alive for portal sessions)
    let (stream, _dbus_guard) = connect(cli)?;

    // EI handshake — use the low-level API so we get an EiEventConverter
    // instead of EiConvertEventIterator (which has a buffering bug: it calls
    // poll_readable() before draining already-buffered protocol events).
    tracing::debug!("starting EI handshake");
    let context = ei::Context::new(stream).context("failed to create EI context")?;
    let resp = reis::handshake::ei_handshake_blocking(&context, "eis-test-client", ei::handshake::ContextType::Sender)
        .context("EI handshake failed")?;
    let negotiated_interfaces = resp.negotiated_interfaces.clone();
    let handshake_serial = resp.serial;
    let mut converter = EiEventConverter::new(&context, resp);
    let connection = converter.connection().clone();

    // Drain any events already buffered during the handshake read
    dispatch_buffered(&context, &mut converter)?;

    tracing::info!("EI handshake completed");

    match &cli.command {
        Command::Probe => cmd_probe(&connection, &context, &mut converter, &negotiated_interfaces, handshake_serial),
        Command::MoveTo { x, y } => {
            cmd_input(&connection, &context, &mut converter, DeviceCapability::PointerAbsolute, |device| {
                let abs = device.interface::<ei::PointerAbsolute>().expect("capability checked");
                abs.motion_absolute(*x, *y);
                Ok(())
            })
        }
        Command::MoveBy { dx, dy } => {
            cmd_input(&connection, &context, &mut converter, DeviceCapability::Pointer, |device| {
                let ptr = device.interface::<ei::Pointer>().expect("capability checked");
                ptr.motion_relative(*dx, *dy);
                Ok(())
            })
        }
        Command::Click { button } => {
            let code = button_code(button)?;
            let device = find_device(&connection, &context, &mut converter, DeviceCapability::Button)?;
            send_press_release(
                &connection,
                &device,
                |d| d.interface::<ei::Button>().expect("checked").button(code, ei::button::ButtonState::Press),
                |d| d.interface::<ei::Button>().expect("checked").button(code, ei::button::ButtonState::Released),
            )
        }
        Command::Scroll { dx, dy } => {
            cmd_input(&connection, &context, &mut converter, DeviceCapability::Scroll, |device| {
                let scroll = device.interface::<ei::Scroll>().expect("capability checked");
                scroll.scroll(*dx, *dy);
                Ok(())
            })
        }
        Command::Key { key } => {
            let (modifiers, keycode) = parse_key_spec(key)?;
            let device = find_device(&connection, &context, &mut converter, DeviceCapability::Keyboard)?;
            send_key_combo(&connection, &device, &modifiers, keycode)
        }
        Command::Tap { x, y } => {
            let device = find_device(&connection, &context, &mut converter, DeviceCapability::Touch)?;
            send_touch_tap(&connection, &device, 0, *x, *y)
        }
        Command::TouchDown { id, x, y } => {
            cmd_input(&connection, &context, &mut converter, DeviceCapability::Touch, |device| {
                let ts = device.interface::<ei::Touchscreen>().expect("capability checked");
                ts.down(*id, *x, *y);
                Ok(())
            })
        }
        Command::TouchMove { id, x, y } => {
            cmd_input(&connection, &context, &mut converter, DeviceCapability::Touch, |device| {
                let ts = device.interface::<ei::Touchscreen>().expect("capability checked");
                ts.motion(*id, *x, *y);
                Ok(())
            })
        }
        Command::TouchUp { id } => {
            cmd_input(&connection, &context, &mut converter, DeviceCapability::Touch, |device| {
                let ts = device.interface::<ei::Touchscreen>().expect("capability checked");
                ts.up(*id);
                Ok(())
            })
        }
        Command::Interactive => cmd_interactive(&connection, &context, &mut converter),
        Command::ResetToken => unreachable!("handled before connect"),
    }
}

// ---------------------------------------------------------------------------
// Connection
// ---------------------------------------------------------------------------

fn connect(cli: &Cli) -> anyhow::Result<(UnixStream, Option<portal::PortalGuard>)> {
    if cli.portal {
        tracing::info!("connecting via XDG Desktop Portal");
        let restore_token = load_restore_token();
        let (stream, dbus_conn, new_token) = portal::connect_via_portal(restore_token.as_deref())?;
        if let Some(token) = new_token
            && let Err(e) = save_restore_token(&token)
        {
            tracing::warn!(error = %e, "failed to save restore token");
        }
        return Ok((stream, Some(dbus_conn)));
    }

    if let Some(path) = &cli.socket {
        tracing::info!(path = %path.display(), "connecting to EIS socket");
        let stream = UnixStream::connect(path).with_context(|| format!("failed to connect to {}", path.display()));
        return stream.map(|s| (s, None));
    }

    // Fall back to LIBEI_SOCKET environment variable
    tracing::info!("connecting via LIBEI_SOCKET environment variable");
    let path = std::env::var("LIBEI_SOCKET").map_err(|_| anyhow!("LIBEI_SOCKET not set; use --portal or --socket"))?;
    let full_path = if std::path::Path::new(&path).is_relative() {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR not set")?;
        PathBuf::from(runtime_dir).join(&path)
    } else {
        PathBuf::from(&path)
    };

    UnixStream::connect(&full_path)
        .with_context(|| format!("failed to connect to {}", full_path.display()))
        .map(|s| (s, None))
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Probe: connect, print all seats/devices, then exit.
///
/// After the first `DeviceResumed`, keeps polling for a short grace period
/// to catch additional devices the compositor may send.
fn cmd_probe(
    connection: &reis::event::Connection,
    context: &ei::Context,
    converter: &mut EiEventConverter,
    negotiated_interfaces: &HashMap<String, u32>,
    handshake_serial: u32,
) -> anyhow::Result<()> {
    /// Grace period after last new event before we stop waiting.
    const PROBE_GRACE: Duration = Duration::from_millis(500);

    // Print connection / handshake info
    println!("Connection:");
    println!("  Context type: Sender");
    println!("  Handshake serial: {handshake_serial}");
    if negotiated_interfaces.is_empty() {
        println!("  Negotiated interfaces: (none)");
    } else {
        println!("  Negotiated interfaces:");
        let mut ifaces: Vec<_> = negotiated_interfaces.iter().collect();
        ifaces.sort_by_key(|(name, _)| name.as_str());
        for (name, version) in ifaces {
            println!("    {name} v{version}");
        }
    }

    let mut saw_device = false;
    let mut deadline: Option<Instant> = None;

    tracing::debug!("probe: waiting for events");
    loop {
        // Drain already-queued high-level events
        while let Some(event) = converter.next_event() {
            tracing::debug!(?event, "probe: received event");
            match event {
                EiEvent::SeatAdded(ref seat) => {
                    println!("Seat: {:?}", seat.seat.name().unwrap_or("<unnamed>"));
                    seat.seat.bind_capabilities(BitFlags::all());
                    connection.flush().context("flush failed")?;
                    deadline = Some(Instant::now() + PROBE_GRACE);
                }
                EiEvent::DeviceAdded(ref dev) => {
                    print_device_info(dev);
                    saw_device = true;
                    deadline = Some(Instant::now() + PROBE_GRACE);
                }
                EiEvent::DeviceResumed(ref dev) => {
                    println!("  Device resumed: {:?}", dev.device.name().unwrap_or("<unnamed>"));
                    deadline = Some(Instant::now() + PROBE_GRACE);
                }
                EiEvent::Disconnected(ref disc) => {
                    println!("Disconnected: reason={:?}, explanation={:?}", disc.reason, disc.explanation);
                    if !saw_device {
                        println!("(no devices discovered)");
                    }
                    return Ok(());
                }
                _ => {
                    deadline = Some(Instant::now() + PROBE_GRACE);
                }
            }
        }

        // If we have a deadline and it has passed, we're done
        if let Some(dl) = deadline
            && Instant::now() >= dl
        {
            break;
        }

        // Wait for new data (with timeout if we already saw events)
        match try_read_and_dispatch(context, converter, deadline) {
            Ok(true) => {}      // got data, keep processing
            Ok(false) => break, // timeout expired
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e).context("EI read error"),
        }
    }

    if !saw_device {
        println!("(no devices discovered)");
    }
    Ok(())
}

fn print_device_info(dev: &reis::event::DeviceAdded) {
    println!("  Device: {:?}", dev.device.name().unwrap_or("<unnamed>"));
    println!("    Type: {:?}", dev.device.device_type());
    println!("    Seat: {:?}", dev.device.seat().name().unwrap_or("<unnamed>"));

    if let Some((w, h)) = dev.device.dimensions() {
        println!("    Dimensions: {w}×{h}");
    }

    for region in dev.device.regions() {
        println!(
            "    Region: {}×{} @ ({}, {}), scale={:.1}{}",
            region.width,
            region.height,
            region.x,
            region.y,
            region.scale,
            region.mapping_id.as_deref().map_or(String::new(), |id| format!(", mapping_id={id}")),
        );
    }

    if dev.device.has_capability(DeviceCapability::Keyboard)
        && let Some(keymap) = dev.device.keymap()
    {
        println!("    Keymap: type={:?}, size={} bytes", keymap.type_, keymap.size);
    }

    let caps: Vec<&str> = [
        (DeviceCapability::Pointer, "pointer"),
        (DeviceCapability::PointerAbsolute, "pointer-absolute"),
        (DeviceCapability::Keyboard, "keyboard"),
        (DeviceCapability::Touch, "touch"),
        (DeviceCapability::Scroll, "scroll"),
        (DeviceCapability::Button, "button"),
    ]
    .iter()
    .filter(|(cap, _)| dev.device.has_capability(*cap))
    .map(|(_, name)| *name)
    .collect();

    println!("    Capabilities: {}", caps.join(", "));
}

/// Generic input command: wait for a resumed device with the required capability,
/// then execute the action.  If multiple devices arrive, picks the first match.
fn cmd_input(
    connection: &reis::event::Connection,
    context: &ei::Context,
    converter: &mut EiEventConverter,
    required: DeviceCapability,
    action: impl FnOnce(&reis::event::Device) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let device = find_device(connection, context, converter, required)?;
    send_input(connection, &device, action)
}

/// Wait for a resumed device with the given capability.
fn find_device(
    connection: &reis::event::Connection,
    context: &ei::Context,
    converter: &mut EiEventConverter,
    required: DeviceCapability,
) -> anyhow::Result<reis::event::Device> {
    /// How long to wait for a matching device after binding capabilities.
    const TIMEOUT: Duration = Duration::from_secs(5);

    let mut resumed: Vec<reis::event::Device> = Vec::new();
    let deadline = Instant::now() + TIMEOUT;

    loop {
        while let Some(event) = converter.next_event() {
            match event {
                EiEvent::SeatAdded(ref seat) => {
                    // Bind all capabilities so the compositor creates a
                    // full virtual device (e.g. pointer + scroll + button).
                    seat.seat.bind_capabilities(BitFlags::all());
                    connection.flush().context("flush failed")?;
                }
                EiEvent::DeviceResumed(ref dev) => {
                    tracing::debug!(device = ?dev.device.name(), "device resumed");
                    if dev.device.has_capability(required) {
                        return Ok(dev.device.clone());
                    }
                    resumed.push(dev.device.clone());
                }
                EiEvent::Disconnected(ref disc) => {
                    return Err(no_capability_error(required, &resumed, Some(disc.reason)));
                }
                _ => {}
            }
        }

        if Instant::now() >= deadline {
            return Err(no_capability_error(required, &resumed, None));
        }

        match try_read_and_dispatch(context, converter, Some(deadline)) {
            Ok(true) => {}
            Ok(false) => return Err(no_capability_error(required, &resumed, None)),
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
                return Err(no_capability_error(required, &resumed, None));
            }
            Err(e) => return Err(e).context("EI read error"),
        }
    }
}

fn send_input(
    connection: &reis::event::Connection,
    device: &reis::event::Device,
    action: impl FnOnce(&reis::event::Device) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let last_serial = connection.serial();
    let device_proxy = device.device();

    device_proxy.start_emulating(last_serial, 1);
    action(device)?;
    device_proxy.frame(last_serial, timestamp_us());
    device_proxy.stop_emulating(last_serial);
    connection.flush().context("flush failed")?;

    // Give the compositor time to process before we tear down
    // the portal session (dropping the D-Bus connection).
    std::thread::sleep(Duration::from_millis(50));

    tracing::info!("input sent successfully");
    Ok(())
}

/// Minimum delay between a press and release event so the compositor
/// registers the pair as a distinct press-then-release.
const PRESS_RELEASE_GAP: Duration = Duration::from_millis(20);

/// Send a press/release pair: press → frame → flush → pause → release → frame.
fn send_press_release(
    connection: &reis::event::Connection,
    device: &reis::event::Device,
    press: impl FnOnce(&reis::event::Device),
    release: impl FnOnce(&reis::event::Device),
) -> anyhow::Result<()> {
    let last_serial = connection.serial();
    let device_proxy = device.device();

    device_proxy.start_emulating(last_serial, 1);
    press(device);
    device_proxy.frame(last_serial, timestamp_us());
    connection.flush().context("flush failed")?;

    std::thread::sleep(PRESS_RELEASE_GAP);

    release(device);
    device_proxy.frame(last_serial, timestamp_us());
    device_proxy.stop_emulating(last_serial);
    connection.flush().context("flush failed")?;

    std::thread::sleep(Duration::from_millis(50));

    tracing::info!("press/release sent successfully");
    Ok(())
}

fn no_capability_error(
    required: DeviceCapability,
    resumed: &[reis::event::Device],
    reason: Option<ei::connection::DisconnectReason>,
) -> anyhow::Error {
    let mut msg = format!("no device with {required:?} capability");
    if let Some(r) = reason {
        let _ = write!(msg, " (disconnected: {r:?})");
    }
    if !resumed.is_empty() {
        msg.push_str("\navailable devices:");
        for dev in resumed {
            let name = dev.name().unwrap_or("<unnamed>");
            let caps: Vec<&str> = [
                (DeviceCapability::Pointer, "pointer"),
                (DeviceCapability::PointerAbsolute, "pointer-absolute"),
                (DeviceCapability::Keyboard, "keyboard"),
                (DeviceCapability::Touch, "touch"),
                (DeviceCapability::Scroll, "scroll"),
                (DeviceCapability::Button, "button"),
            ]
            .iter()
            .filter(|(cap, _)| dev.has_capability(*cap))
            .map(|(_, n)| *n)
            .collect();
            let _ = write!(msg, "\n  {name}: {}", caps.join(", "));
        }
    }
    anyhow!(msg)
}

// ---------------------------------------------------------------------------
// Interactive mode
// ---------------------------------------------------------------------------

/// Interactive REPL: connect once, then process commands from stdin.
fn cmd_interactive(
    connection: &reis::event::Connection,
    context: &ei::Context,
    converter: &mut EiEventConverter,
) -> anyhow::Result<()> {
    let devices = wait_for_devices(connection, context, converter)?;
    if devices.is_empty() {
        return Err(anyhow!("no devices available"));
    }

    println!("Interactive mode. Available devices:");
    print_device_summary(&devices);
    println!();
    print_interactive_help();
    println!();

    let mut line_editor = reedline::Reedline::create();
    let prompt = reedline::DefaultPrompt::new(
        reedline::DefaultPromptSegment::Basic("ei".to_string()),
        reedline::DefaultPromptSegment::Empty,
    );

    'repl: loop {
        match line_editor.read_line(&prompt) {
            Ok(reedline::Signal::Success(line)) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                // Support multiple commands separated by semicolons
                let commands: Vec<&str> = line.split(';').map(str::trim).filter(|s| !s.is_empty()).collect();
                for cmd in commands {
                    let parts: Vec<&str> = cmd.split_whitespace().collect();
                    if matches!(parts[0], "quit" | "exit" | "q") {
                        break 'repl;
                    }
                    if let Err(e) = dispatch_interactive(&parts, &devices, connection) {
                        eprintln!("error: {e:#}");
                    }
                }
            }
            Ok(reedline::Signal::CtrlC | reedline::Signal::CtrlD) => break,
            Err(e) => return Err(anyhow::Error::msg(e.to_string()).context("readline error")),
        }
    }

    Ok(())
}

fn print_device_summary(devices: &[reis::event::Device]) {
    for dev in devices {
        let name = dev.name().unwrap_or("<unnamed>");
        println!("  {name}: {}", capability_list(dev));
    }
}

fn capability_list(dev: &reis::event::Device) -> String {
    [
        (DeviceCapability::Pointer, "pointer"),
        (DeviceCapability::PointerAbsolute, "pointer-absolute"),
        (DeviceCapability::Keyboard, "keyboard"),
        (DeviceCapability::Touch, "touch"),
        (DeviceCapability::Scroll, "scroll"),
        (DeviceCapability::Button, "button"),
    ]
    .iter()
    .filter(|(cap, _)| dev.has_capability(*cap))
    .map(|(_, n)| *n)
    .collect::<Vec<_>>()
    .join(", ")
}

fn print_interactive_help() {
    println!("Commands: move-by <dx> <dy> | move-to <x> <y> | click [left|right|middle]");
    println!("          scroll <dx> <dy> | key <name|code> | tap <x> <y>");
    println!("          touch-down [id] <x> <y> | touch-move [id] <x> <y> | touch-up [id]");
    println!("          probe | keys | quit");
    println!("Key examples: key a | key enter | key ctrl+a | key alt+f4 | key 30");
}

/// Parse and execute a single interactive command line.
fn dispatch_interactive(
    parts: &[&str],
    devices: &[reis::event::Device],
    connection: &reis::event::Connection,
) -> anyhow::Result<()> {
    match parts[0] {
        "move-by" => {
            let (dx, dy) = parse_f32_pair(parts, "move-by <dx> <dy>")?;
            interactive_action(devices, DeviceCapability::Pointer, connection, |dev| {
                dev.interface::<ei::Pointer>().expect("checked").motion_relative(dx, dy);
                Ok(())
            })
        }
        "move-to" => {
            let (x, y) = parse_f32_pair(parts, "move-to <x> <y>")?;
            interactive_action(devices, DeviceCapability::PointerAbsolute, connection, |dev| {
                dev.interface::<ei::PointerAbsolute>().expect("checked").motion_absolute(x, y);
                Ok(())
            })
        }
        "click" => {
            let button_name = parts.get(1).copied().unwrap_or("left");
            let code = button_code(button_name)?;
            let dev = devices
                .iter()
                .find(|d| d.has_capability(DeviceCapability::Button))
                .ok_or_else(|| anyhow!("no device with Button capability"))?;
            send_press_release(
                connection,
                dev,
                |d| d.interface::<ei::Button>().expect("checked").button(code, ei::button::ButtonState::Press),
                |d| d.interface::<ei::Button>().expect("checked").button(code, ei::button::ButtonState::Released),
            )
        }
        "scroll" => {
            let (dx, dy) = parse_f32_pair(parts, "scroll <dx> <dy>")?;
            interactive_action(devices, DeviceCapability::Scroll, connection, |dev| {
                dev.interface::<ei::Scroll>().expect("checked").scroll(dx, dy);
                Ok(())
            })
        }
        "key" => {
            if parts.len() != 2 {
                return Err(anyhow!("usage: key <name|keycode>  (e.g. key a, key ctrl+a, key 30)"));
            }
            let (modifiers, keycode) = parse_key_spec(parts[1])?;
            let dev = devices
                .iter()
                .find(|d| d.has_capability(DeviceCapability::Keyboard))
                .ok_or_else(|| anyhow!("no device with Keyboard capability"))?;
            send_key_combo(connection, dev, &modifiers, keycode)
        }
        "tap" => {
            let (x, y) = parse_f32_pair(parts, "tap <x> <y>")?;
            let dev = devices
                .iter()
                .find(|d| d.has_capability(DeviceCapability::Touch))
                .ok_or_else(|| anyhow!("no device with Touch capability"))?;
            send_touch_tap(connection, dev, 0, x, y)
        }
        "touch-down" => {
            let (id, x, y) = parse_touch_args(parts, "touch-down [id] <x> <y>")?;
            interactive_action(devices, DeviceCapability::Touch, connection, |dev| {
                dev.interface::<ei::Touchscreen>().expect("checked").down(id, x, y);
                Ok(())
            })
        }
        "touch-move" => {
            let (id, x, y) = parse_touch_args(parts, "touch-move [id] <x> <y>")?;
            interactive_action(devices, DeviceCapability::Touch, connection, |dev| {
                dev.interface::<ei::Touchscreen>().expect("checked").motion(id, x, y);
                Ok(())
            })
        }
        "touch-up" => {
            let id = match parts.len() {
                1 => 0,
                2 => parts[1].parse::<u32>().map_err(|_| anyhow!("invalid touch id"))?,
                _ => return Err(anyhow!("usage: touch-up [id]")),
            };
            interactive_action(devices, DeviceCapability::Touch, connection, |dev| {
                dev.interface::<ei::Touchscreen>().expect("checked").up(id);
                Ok(())
            })
        }
        "keys" => {
            println!("Available key names:");
            print_key_names();
            Ok(())
        }
        "probe" => {
            println!("Devices:");
            print_device_summary(devices);
            Ok(())
        }
        "help" | "?" => {
            print_interactive_help();
            Ok(())
        }
        other => Err(anyhow!("unknown command: {other} (type 'help' for usage)")),
    }
}

/// Parse touch command args: `["cmd", x, y]` (id defaults to 0) or `["cmd", id, x, y]`.
fn parse_touch_args(parts: &[&str], usage: &str) -> anyhow::Result<(u32, f32, f32)> {
    match parts.len() {
        3 => {
            let x = parts[1].parse::<f32>().map_err(|_| anyhow!("invalid number: {}", parts[1]))?;
            let y = parts[2].parse::<f32>().map_err(|_| anyhow!("invalid number: {}", parts[2]))?;
            Ok((0, x, y))
        }
        4 => {
            let id = parts[1].parse::<u32>().map_err(|_| anyhow!("invalid touch id: {}", parts[1]))?;
            let x = parts[2].parse::<f32>().map_err(|_| anyhow!("invalid number: {}", parts[2]))?;
            let y = parts[3].parse::<f32>().map_err(|_| anyhow!("invalid number: {}", parts[3]))?;
            Ok((id, x, y))
        }
        _ => Err(anyhow!("usage: {usage}")),
    }
}

/// Parse two f32 arguments from a command line like `["cmd", "1.0", "2.0"]`.
fn parse_f32_pair(parts: &[&str], usage: &str) -> anyhow::Result<(f32, f32)> {
    if parts.len() != 3 {
        return Err(anyhow!("usage: {usage}"));
    }
    let a = parts[1].parse::<f32>().map_err(|_| anyhow!("invalid number: {}", parts[1]))?;
    let b = parts[2].parse::<f32>().map_err(|_| anyhow!("invalid number: {}", parts[2]))?;
    Ok((a, b))
}

/// Wait for devices to appear after handshake (bind all capabilities, collect
/// all resumed devices until a grace period expires).
fn wait_for_devices(
    connection: &reis::event::Connection,
    context: &ei::Context,
    converter: &mut EiEventConverter,
) -> anyhow::Result<Vec<reis::event::Device>> {
    const TIMEOUT: Duration = Duration::from_secs(5);
    const GRACE: Duration = Duration::from_millis(500);

    let hard_deadline = Instant::now() + TIMEOUT;
    let mut grace_deadline: Option<Instant> = None;
    let mut devices: Vec<reis::event::Device> = Vec::new();

    loop {
        while let Some(event) = converter.next_event() {
            match event {
                EiEvent::SeatAdded(ref seat) => {
                    seat.seat.bind_capabilities(BitFlags::all());
                    connection.flush().context("flush failed")?;
                }
                EiEvent::DeviceResumed(ref dev) => {
                    tracing::debug!(device = ?dev.device.name(), "device resumed");
                    devices.push(dev.device.clone());
                    grace_deadline = Some(Instant::now() + GRACE);
                }
                EiEvent::Disconnected(ref disc) => {
                    if devices.is_empty() {
                        return Err(anyhow!("disconnected before devices arrived: {:?}", disc.reason));
                    }
                    return Ok(devices);
                }
                _ => {}
            }
        }

        let effective_deadline = match grace_deadline {
            Some(gd) => gd.min(hard_deadline),
            None => hard_deadline,
        };

        if Instant::now() >= effective_deadline {
            break;
        }

        match try_read_and_dispatch(context, converter, Some(effective_deadline)) {
            Ok(true) => {}
            Ok(false) => break,
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(e).context("EI read error"),
        }
    }

    Ok(devices)
}

/// Send an input action on the first device that has the required capability.
fn interactive_action(
    devices: &[reis::event::Device],
    required: DeviceCapability,
    connection: &reis::event::Connection,
    action: impl FnOnce(&reis::event::Device) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let device = devices
        .iter()
        .find(|d| d.has_capability(required))
        .ok_or_else(|| anyhow!("no device with {required:?} capability"))?;
    send_input(connection, device, action)
}

// ---------------------------------------------------------------------------
// EI event-loop helpers
// ---------------------------------------------------------------------------

/// Parse any events already sitting in the context's internal read buffer and
/// feed them into the converter.  This is critical after the handshake because
/// `context.read()` may have pulled in post-handshake events (Seat/Device)
/// together with the handshake data.
fn dispatch_buffered(context: &ei::Context, converter: &mut EiEventConverter) -> anyhow::Result<()> {
    while let Some(result) = context.pending_event() {
        match result {
            PendingRequestResult::Request(event) => {
                converter.handle_event(event).context("EI protocol error")?;
            }
            PendingRequestResult::ParseError(e) => {
                return Err(anyhow!("EI parse error: {e}"));
            }
            PendingRequestResult::InvalidObject(_) => {}
        }
    }
    Ok(())
}

/// Poll the context fd (optionally with a deadline), read if data arrives,
/// and dispatch buffered events.  Returns `Ok(true)` if data was read, or
/// `Ok(false)` if the deadline expired without new data.
fn try_read_and_dispatch(
    context: &ei::Context,
    converter: &mut EiEventConverter,
    deadline: Option<Instant>,
) -> io::Result<bool> {
    let mut pfd = [PollFd::new(context, PollFlags::IN)];
    loop {
        let timeout = deadline.map(|dl| {
            let remaining = dl.saturating_duration_since(Instant::now());
            Timespec { tv_sec: remaining.as_secs().cast_signed(), tv_nsec: i64::from(remaining.subsec_nanos()) }
        });

        // If deadline already passed, return immediately
        if let Some(ref ts) = timeout
            && ts.tv_sec == 0
            && ts.tv_nsec == 0
        {
            return Ok(false);
        }

        match poll(&mut pfd, timeout.as_ref()) {
            Ok(0) => return Ok(false), // timeout
            Ok(_) => break,
            Err(rustix::io::Errno::INTR) => {}
            Err(e) => return Err(e.into()),
        }
    }

    // Read new data into the internal buffer
    context.read()?;

    // Parse protocol events and feed them into the converter
    dispatch_buffered(context, converter).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(true)
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Key name → evdev keycode mapping
// ---------------------------------------------------------------------------

/// All recognized key names and their evdev keycodes.
const KEY_MAP: &[(&str, u32)] = &[
    // Letters
    ("a", 30),
    ("b", 48),
    ("c", 46),
    ("d", 32),
    ("e", 18),
    ("f", 33),
    ("g", 34),
    ("h", 35),
    ("i", 23),
    ("j", 36),
    ("k", 37),
    ("l", 38),
    ("m", 50),
    ("n", 49),
    ("o", 24),
    ("p", 25),
    ("q", 16),
    ("r", 19),
    ("s", 31),
    ("t", 20),
    ("u", 22),
    ("v", 47),
    ("w", 17),
    ("x", 45),
    ("y", 21),
    ("z", 44),
    // Number row
    ("1", 2),
    ("2", 3),
    ("3", 4),
    ("4", 5),
    ("5", 6),
    ("6", 7),
    ("7", 8),
    ("8", 9),
    ("9", 10),
    ("0", 11),
    // F-keys
    ("f1", 59),
    ("f2", 60),
    ("f3", 61),
    ("f4", 62),
    ("f5", 63),
    ("f6", 64),
    ("f7", 65),
    ("f8", 66),
    ("f9", 67),
    ("f10", 68),
    ("f11", 87),
    ("f12", 88),
    // Special keys
    ("esc", 1),
    ("escape", 1),
    ("enter", 28),
    ("return", 28),
    ("tab", 15),
    ("space", 57),
    ("backspace", 14),
    ("bs", 14),
    ("delete", 111),
    ("del", 111),
    ("insert", 110),
    ("ins", 110),
    ("home", 102),
    ("end", 107),
    ("pageup", 104),
    ("pgup", 104),
    ("pagedown", 109),
    ("pgdn", 109),
    // Arrow keys
    ("up", 103),
    ("down", 108),
    ("left", 105),
    ("right", 106),
    // Modifiers
    ("shift", 42),
    ("leftshift", 42),
    ("lshift", 42),
    ("rightshift", 54),
    ("rshift", 54),
    ("ctrl", 29),
    ("control", 29),
    ("leftctrl", 29),
    ("lctrl", 29),
    ("rightctrl", 97),
    ("rctrl", 97),
    ("alt", 56),
    ("leftalt", 56),
    ("lalt", 56),
    ("rightalt", 100),
    ("ralt", 100),
    ("altgr", 100),
    ("super", 125),
    ("meta", 125),
    ("win", 125),
    ("leftmeta", 125),
    ("lmeta", 125),
    ("rightmeta", 126),
    ("rmeta", 126),
    // Punctuation & symbols
    ("minus", 12),
    ("-", 12),
    ("equal", 13),
    ("=", 13),
    ("leftbrace", 26),
    ("[", 26),
    ("rightbrace", 27),
    ("]", 27),
    ("semicolon", 39),
    (";", 39),
    ("apostrophe", 40),
    ("'", 40),
    ("grave", 41),
    ("`", 41),
    ("backslash", 43),
    ("\\", 43),
    ("comma", 51),
    (",", 51),
    ("dot", 52),
    (".", 52),
    ("period", 52),
    ("slash", 53),
    ("/", 53),
    // Lock keys
    ("capslock", 58),
    ("caps", 58),
    ("numlock", 69),
    ("scrolllock", 70),
    // Misc
    ("print", 99),
    ("printscreen", 99),
    ("prtsc", 99),
    ("pause", 119),
    ("menu", 127),
    ("compose", 127),
];

/// Modifier aliases that map to their left-side evdev keycode.
const MODIFIER_NAMES: &[(&str, u32)] = &[
    ("ctrl", 29),
    ("control", 29),
    ("lctrl", 29),
    ("leftctrl", 29),
    ("rctrl", 97),
    ("rightctrl", 97),
    ("shift", 42),
    ("lshift", 42),
    ("leftshift", 42),
    ("rshift", 54),
    ("rightshift", 54),
    ("alt", 56),
    ("lalt", 56),
    ("leftalt", 56),
    ("ralt", 100),
    ("rightalt", 100),
    ("altgr", 100),
    ("super", 125),
    ("meta", 125),
    ("win", 125),
    ("lmeta", 125),
    ("leftmeta", 125),
    ("rmeta", 126),
    ("rightmeta", 126),
];

/// Look up a key name → evdev keycode.  Falls back to parsing as a raw number.
fn key_name_to_code(name: &str) -> anyhow::Result<u32> {
    let lower = name.to_lowercase();
    for &(alias, code) in KEY_MAP {
        if alias == lower {
            return Ok(code);
        }
    }
    // Try raw numeric code
    lower
        .parse::<u32>()
        .map_err(|_| anyhow!("unknown key: {name}  (type 'keys' in interactive mode to list available names)"))
}

/// Parse a key specification like `a`, `enter`, `ctrl+a`, `alt+f4`, `ctrl+shift+delete`, or `30`.
/// Returns (`modifier_codes`, `final_key_code`).
fn parse_key_spec(spec: &str) -> anyhow::Result<(Vec<u32>, u32)> {
    let parts: Vec<&str> = spec.split('+').collect();
    if parts.is_empty() || parts.iter().any(|p| p.is_empty()) {
        return Err(anyhow!("invalid key specification: {spec}"));
    }

    // Last part is the actual key, everything before is a modifier
    let key_code = key_name_to_code(parts.last().expect("non-empty"))?;

    let mut modifiers = Vec::new();
    for &m in &parts[..parts.len() - 1] {
        let lower = m.to_lowercase();
        let code = MODIFIER_NAMES
            .iter()
            .find(|&&(alias, _)| alias == lower)
            .map(|&(_, code)| code)
            .ok_or_else(|| anyhow!("{m} is not a known modifier (use ctrl, shift, alt, super)"))?;
        modifiers.push(code);
    }

    Ok((modifiers, key_code))
}

/// Send a key combo: press modifiers in order, press key, release key, release modifiers in reverse.
fn send_key_combo(
    connection: &reis::event::Connection,
    device: &reis::event::Device,
    modifiers: &[u32],
    keycode: u32,
) -> anyhow::Result<()> {
    if modifiers.is_empty() {
        // Simple single key — use existing press/release
        return send_press_release(
            connection,
            device,
            |d| d.interface::<ei::Keyboard>().expect("checked").key(keycode, ei::keyboard::KeyState::Press),
            |d| d.interface::<ei::Keyboard>().expect("checked").key(keycode, ei::keyboard::KeyState::Released),
        );
    }

    // Shortcut: modifiers down → key down → key up → modifiers up (reverse)
    let last_serial = connection.serial();
    let device_proxy = device.device();
    let kbd = device.interface::<ei::Keyboard>().expect("checked");

    device_proxy.start_emulating(last_serial, 1);

    // Press modifiers
    for &m in modifiers {
        kbd.key(m, ei::keyboard::KeyState::Press);
        device_proxy.frame(last_serial, timestamp_us());
        connection.flush().context("flush failed")?;
        std::thread::sleep(PRESS_RELEASE_GAP);
    }

    // Press + release the main key
    kbd.key(keycode, ei::keyboard::KeyState::Press);
    device_proxy.frame(last_serial, timestamp_us());
    connection.flush().context("flush failed")?;
    std::thread::sleep(PRESS_RELEASE_GAP);

    kbd.key(keycode, ei::keyboard::KeyState::Released);
    device_proxy.frame(last_serial, timestamp_us());
    connection.flush().context("flush failed")?;
    std::thread::sleep(PRESS_RELEASE_GAP);

    // Release modifiers in reverse order
    for &m in modifiers.iter().rev() {
        kbd.key(m, ei::keyboard::KeyState::Released);
        device_proxy.frame(last_serial, timestamp_us());
        connection.flush().context("flush failed")?;
        std::thread::sleep(PRESS_RELEASE_GAP);
    }

    device_proxy.stop_emulating(last_serial);
    connection.flush().context("flush failed")?;
    std::thread::sleep(Duration::from_millis(50));

    tracing::info!("key combo sent successfully");
    Ok(())
}

/// Print all available key names, grouped by category.
fn print_key_names() {
    println!("  Letters:    a-z");
    println!("  Numbers:    0-9");
    println!("  F-keys:     f1-f12");
    println!("  Navigation: up, down, left, right, home, end, pageup/pgup, pagedown/pgdn");
    println!("  Editing:    enter/return, tab, space, backspace/bs, delete/del, insert/ins");
    println!("  Modifiers:  ctrl, shift, alt, altgr, super/meta/win");
    println!("  Other:      esc/escape, capslock/caps, numlock, scrolllock, print/prtsc, pause, menu");
    println!("  Symbols:    minus/-  equal/=  [  ]  ;  '  `  \\  ,  .  /");
    println!("  Shortcuts:  ctrl+a  alt+f4  ctrl+shift+delete  super+l");
    println!("  Raw codes:  any number (e.g. 30 = KEY_A)");
}

/// Send a touch tap: down → frame → pause → up → frame (separate frames required by protocol).
fn send_touch_tap(
    connection: &reis::event::Connection,
    device: &reis::event::Device,
    touchid: u32,
    x: f32,
    y: f32,
) -> anyhow::Result<()> {
    let last_serial = connection.serial();
    let device_proxy = device.device();
    let ts = device.interface::<ei::Touchscreen>().expect("capability checked");

    device_proxy.start_emulating(last_serial, 1);

    ts.down(touchid, x, y);
    device_proxy.frame(last_serial, timestamp_us());
    connection.flush().context("flush failed")?;

    std::thread::sleep(PRESS_RELEASE_GAP);

    ts.up(touchid);
    device_proxy.frame(last_serial, timestamp_us());
    device_proxy.stop_emulating(last_serial);
    connection.flush().context("flush failed")?;

    std::thread::sleep(Duration::from_millis(50));

    tracing::info!("touch tap sent successfully");
    Ok(())
}

/// Maps a button name to a Linux evdev button code.
fn button_code(name: &str) -> anyhow::Result<u32> {
    match name.to_lowercase().as_str() {
        "left" => Ok(0x110),   // BTN_LEFT
        "right" => Ok(0x111),  // BTN_RIGHT
        "middle" => Ok(0x112), // BTN_MIDDLE
        other => other
            .parse::<u32>()
            .map_err(|_| anyhow!("unknown button: {other} (use left/right/middle or a numeric code)")),
    }
}

/// Returns a monotonic timestamp in microseconds.
#[expect(clippy::cast_possible_truncation, reason = "timestamp won't overflow u64 in practice")]
fn timestamp_us() -> u64 {
    static EPOCH: LazyLock<Instant> = LazyLock::new(Instant::now);
    EPOCH.elapsed().as_micros() as u64
}

// ---------------------------------------------------------------------------
// Portal restore token persistence
// ---------------------------------------------------------------------------

/// Path to the stored portal restore token.
fn token_path() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME").map_or_else(
        |_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".local/share")
        },
        PathBuf::from,
    );
    base.join("platynui/eis-restore-token")
}

/// Load a previously saved restore token (returns `None` if absent or unreadable).
fn load_restore_token() -> Option<String> {
    let path = token_path();
    match std::fs::read_to_string(&path) {
        Ok(token) => {
            let token = token.trim().to_string();
            if token.is_empty() {
                None
            } else {
                tracing::info!(path = %path.display(), "loaded portal restore token");
                Some(token)
            }
        }
        Err(_) => None,
    }
}

/// Save a restore token for future sessions.
fn save_restore_token(token: &str) -> anyhow::Result<()> {
    let path = token_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("failed to create directory {}", parent.display()))?;
    }
    std::fs::write(&path, token).with_context(|| format!("failed to write {}", path.display()))?;
    tracing::info!(path = %path.display(), "saved portal restore token");
    Ok(())
}

/// Delete the stored restore token.
fn cmd_reset_token() -> anyhow::Result<()> {
    let path = token_path();
    match std::fs::remove_file(&path) {
        Ok(()) => {
            println!("Restore token deleted: {}", path.display());
            Ok(())
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            println!("No restore token found at {}", path.display());
            Ok(())
        }
        Err(e) => Err(anyhow::Error::from(e).context(format!("failed to delete {}", path.display()))),
    }
}

// ---------------------------------------------------------------------------
// Tracing
// ---------------------------------------------------------------------------

fn init_tracing(cli_level: Option<LogLevel>) {
    use tracing_subscriber::EnvFilter;

    let filter = if std::env::var("RUST_LOG").is_ok() {
        EnvFilter::from_default_env()
    } else {
        let directive = if let Some(level) = cli_level {
            log_level_directive(level)
        } else if let Ok(val) = std::env::var("PLATYNUI_LOG_LEVEL") {
            val
        } else {
            "warn".to_string()
        };
        EnvFilter::new(directive)
    };

    tracing_subscriber::fmt().with_env_filter(filter).with_target(true).with_writer(std::io::stderr).init();
}

fn log_level_directive(level: LogLevel) -> String {
    match level {
        LogLevel::Error => "error",
        LogLevel::Warn => "warn",
        LogLevel::Info => "info",
        LogLevel::Debug => "debug",
        LogLevel::Trace => "trace",
    }
    .to_string()
}
