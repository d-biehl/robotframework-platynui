//! Compositor process harness for the protocol integration tests.
//!
//! Starts `platynui-wayland-compositor` headless with a unique socket name,
//! waits for its control socket, and offers typed wrappers for the control
//! commands the tests observe through. Unlike the older `ipc_tests` helpers it
//! never skips: a compositor that cannot start is a failed prerequisite, so the
//! test fails with the compositor's own output (dev-docs/testing-strategy.md).
//!
//! The compositor's stdout and stderr go to files under `CARGO_TARGET_TMPDIR`
//! instead of pipes, so a chatty compositor can never block on a full pipe, and
//! every failure message can include the log.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// Default output size of the test compositor (logical = physical at scale 1).
pub const OUTPUT_SIZE: (i32, i32) = (1280, 720);
/// [`OUTPUT_SIZE`] as the unsigned values protocols use for extents and sizes.
pub const OUTPUT_SIZE_U32: (u32, u32) = (1280, 720);

/// How a test wants the compositor started.
#[derive(Debug, Clone)]
pub struct Options {
    /// Serve EIS (`false` passes `--no-eis`).
    pub eis: bool,
    /// Pass `--restrict-protocols` with this whitelist.
    pub restrict_protocols: Option<String>,
    /// Pass `--scale`.
    pub scale: Option<f64>,
    /// Pass `--outputs`.
    pub outputs: Option<u32>,
    /// Pass `--print-env`.
    pub print_env: bool,
    /// Pass `--keyboard-layout`.
    pub keyboard_layout: Option<String>,
    /// Start Xwayland (`--xwayland`; implies `--print-env` to learn `DISPLAY`).
    pub xwayland: bool,
    /// Output size (`--width`/`--height`).
    pub size: (i32, i32),
    /// Child command after `--`.
    pub child: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            eis: false,
            restrict_protocols: None,
            scale: None,
            outputs: None,
            print_env: false,
            keyboard_layout: None,
            xwayland: false,
            size: OUTPUT_SIZE,
            child: Vec::new(),
        }
    }
}

/// A running compositor; shut down (and killed if needed) on drop.
///
/// Each compositor gets a private, freshly created `XDG_RUNTIME_DIR`, so the
/// fixed-name EIS socket (`eis-platynui`) of parallel tests cannot collide,
/// and nothing leaks into the developer's real runtime directory.
pub struct Compositor {
    child: Option<Child>,
    runtime: tempfile::TempDir,
    pub socket_name: String,
    pub runtime_dir: PathBuf,
    pub control_socket: PathBuf,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

impl Compositor {
    /// Start the compositor and wait for its control socket.
    ///
    /// # Panics
    /// When the binary cannot be started or the control socket does not appear
    /// within 15 s — with the compositor's output in the message.
    pub fn start(test_name: &str, options: &Options) -> Self {
        let socket_name = format!("pui-{test_name}-{}", std::process::id());
        let log_dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
        let stdout_path = log_dir.join(format!("{socket_name}.stdout"));
        let stderr_path = log_dir.join(format!("{socket_name}.stderr"));
        let stdout = std::fs::File::create(&stdout_path).expect("create stdout log");
        let stderr = std::fs::File::create(&stderr_path).expect("create stderr log");

        let mut cmd = Command::new(env!("CARGO_BIN_EXE_platynui-wayland-compositor"));
        cmd.args(["--backend", "headless", "--socket-name", &socket_name, "--timeout", "60"]);
        cmd.args(["--width", &options.size.0.to_string(), "--height", &options.size.1.to_string()]);
        if !options.eis {
            cmd.arg("--no-eis");
        }
        if let Some(list) = &options.restrict_protocols {
            cmd.args(["--restrict-protocols", list]);
        }
        if let Some(scale) = options.scale {
            cmd.args(["--scale", &scale.to_string()]);
        }
        if let Some(outputs) = options.outputs {
            cmd.args(["--outputs", &outputs.to_string()]);
        }
        if options.xwayland {
            cmd.arg("--xwayland");
        }
        // Debugging aid: `PLATYNUI_TEST_COMPOSITOR_LOG=debug` raises the compositor's log level.
        if let Ok(level) = std::env::var("PLATYNUI_TEST_COMPOSITOR_LOG") {
            cmd.args(["--log-level", &level]);
        }
        if options.print_env || options.xwayland {
            cmd.arg("--print-env");
        }
        if let Some(layout) = &options.keyboard_layout {
            cmd.args(["--keyboard-layout", layout]);
        }
        if !options.child.is_empty() {
            cmd.arg("--").args(&options.child);
        }
        cmd.env("LIBGL_ALWAYS_SOFTWARE", "1")
            .env_remove("LIBEI_SOCKET")
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr);
        // The keymap comes from the CLI only, never from the developer's session.
        for var in [
            "XKB_DEFAULT_RULES",
            "XKB_DEFAULT_MODEL",
            "XKB_DEFAULT_LAYOUT",
            "XKB_DEFAULT_VARIANT",
            "XKB_DEFAULT_OPTIONS",
        ] {
            cmd.env_remove(var);
        }

        // Short path under the system temp dir: socket paths are limited to 108 bytes.
        let runtime = tempfile::Builder::new().prefix("pui-rt-").tempdir().expect("create a runtime dir");
        cmd.env("XDG_RUNTIME_DIR", runtime.path());
        let child = cmd.spawn().unwrap_or_else(|err| panic!("cannot start the compositor: {err}"));
        let runtime_dir = runtime.path().to_path_buf();
        let control_socket = runtime_dir.join(format!("{socket_name}.control"));
        let mut compositor =
            Self { child: Some(child), runtime, socket_name, runtime_dir, control_socket, stdout_path, stderr_path };

        let deadline = Instant::now() + Duration::from_secs(15);
        while !compositor.control_socket.exists() || compositor.try_command(&json!({"command": "status"})).is_err() {
            assert!(
                Instant::now() <= deadline,
                "the compositor's control socket did not come up\n{}",
                compositor.logs()
            );
            if let Some(child) = compositor.child.as_mut()
                && let Ok(Some(status)) = child.try_wait()
            {
                panic!("the compositor exited during startup with {status}\n{}", compositor.logs());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        compositor
    }

    /// The Wayland socket path clients connect to.
    pub fn wayland_socket(&self) -> PathBuf {
        self.runtime_dir.join(&self.socket_name)
    }

    /// Where the compositor's EIS socket is (or would be).
    pub fn eis_socket(&self) -> PathBuf {
        self.runtime_dir.join("eis-platynui")
    }

    /// The compositor's stdout and stderr so far.
    pub fn logs(&self) -> String {
        let read = |path: &Path| std::fs::read_to_string(path).unwrap_or_default();
        format!(
            "--- compositor stdout ---\n{}\n--- compositor stderr ---\n{}",
            read(&self.stdout_path),
            read(&self.stderr_path)
        )
    }

    /// The X11 display of the compositor's Xwayland (`:N`), once it is ready.
    ///
    /// # Panics
    /// When Xwayland does not become ready within 30 s.
    pub fn x11_display(&self) -> String {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(display) = self.stdout().lines().find_map(|l| l.strip_prefix("DISPLAY=")) {
                return display.trim().to_string();
            }
            assert!(Instant::now() < deadline, "Xwayland did not become ready\n{}", self.logs());
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The compositor's stdout so far.
    pub fn stdout(&self) -> String {
        std::fs::read_to_string(&self.stdout_path).unwrap_or_default()
    }

    /// Send one control command; errors are returned, not raised.
    pub fn try_command(&self, command: &Value) -> Result<Value, Box<dyn std::error::Error>> {
        let stream = UnixStream::connect(&self.control_socket)?;
        stream.set_read_timeout(Some(Duration::from_secs(10)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        let mut writer = &stream;
        writeln!(writer, "{command}")?;
        writer.flush()?;
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line)?;
        Ok(serde_json::from_str(&line)?)
    }

    /// Send one control command and require `"status": "ok"`.
    ///
    /// # Panics
    /// When the command fails or answers with an error.
    pub fn command(&self, command: &Value) -> Value {
        let response = self
            .try_command(command)
            .unwrap_or_else(|err| panic!("control command {command} failed: {err}\n{}", self.logs()));
        assert_eq!(response["status"], "ok", "control command {command} answered {response}\n{}", self.logs());
        response
    }

    /// The pointer position the compositor reports.
    pub fn pointer_position(&self) -> (f64, f64) {
        let r = self.command(&json!({"command": "get_pointer_position"}));
        (r["x"].as_f64().expect("x"), r["y"].as_f64().expect("y"))
    }

    /// The seat's modifier state (`ctrl`, `alt`, `shift`, `logo`).
    pub fn modifiers(&self) -> Value {
        self.command(&json!({"command": "get_modifiers"}))
    }

    /// The compositor keymap as reported by `get_keymap`.
    pub fn keymap(&self) -> String {
        self.command(&json!({"command": "get_keymap"}))["keymap"].as_str().expect("keymap string").to_string()
    }

    /// The mapped windows (`list_windows` → `windows`).
    pub fn windows(&self) -> Vec<Value> {
        self.command(&json!({"command": "list_windows"}))["windows"].as_array().cloned().unwrap_or_default()
    }

    /// The `status` response.
    pub fn status(&self) -> Value {
        self.command(&json!({"command": "status"}))
    }

    /// Wait until a window with `app_id` is mapped and return its entry.
    ///
    /// # Panics
    /// When it does not appear within 10 s.
    pub fn wait_for_window(&self, app_id: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(window) = self.windows().into_iter().find(|w| w["app_id"] == app_id) {
                return window;
            }
            assert!(Instant::now() < deadline, "window {app_id} did not appear\n{}", self.logs());
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Wait until a window with `title` is mapped and return its entry.
    ///
    /// # Panics
    /// When it does not appear within 10 s.
    pub fn wait_for_window_title(&self, title: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Some(window) = self.windows().into_iter().find(|w| w["title"] == title) {
                return window;
            }
            assert!(Instant::now() < deadline, "window {title} did not appear\n{}", self.logs());
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Move a window's frame to a logical position via `move_window`.
    pub fn move_window(&self, app_id: &str, x: i32, y: i32) {
        self.command(&json!({"command": "move_window", "app_id": app_id, "x": x, "y": y}));
    }

    /// Send a fire-and-forget input command (the compositor answers none) and
    /// wait until it has been processed: a `status` request on the same
    /// connection is answered only after the lines before it.
    ///
    /// # Panics
    /// When the connection fails.
    pub fn input(&self, command: &Value) {
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let stream = UnixStream::connect(&self.control_socket)?;
            stream.set_read_timeout(Some(Duration::from_secs(10)))?;
            let mut writer = &stream;
            writeln!(writer, "{command}")?;
            writeln!(writer, "{}", json!({"command": "status"}))?;
            writer.flush()?;
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line)?;
            Ok(())
        })();
        result.unwrap_or_else(|err| panic!("input command {command} failed: {err}\n{}", self.logs()));
    }

    /// Move the pointer through the control socket.
    pub fn pointer_move_to(&self, x: f64, y: f64) {
        self.input(&json!({"command": "pointer_move_to", "x": x, "y": y}));
    }

    /// Press or release a key (evdev code) through the control socket.
    pub fn key(&self, evdev: u32, pressed: bool) {
        self.input(&json!({"command": "key_event", "key": evdev, "state": if pressed { "press" } else { "release" }}));
    }

    /// Poll `cond` on fresh control-socket state until it holds or 5 s pass.
    pub fn eventually(&self, what: &str, mut cond: impl FnMut(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !cond(self) {
            assert!(Instant::now() < deadline, "timed out waiting for {what}\n{}", self.logs());
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    /// Whether the compositor process is still running.
    pub fn is_running(&mut self) -> bool {
        self.child.as_mut().is_some_and(|c| matches!(c.try_wait(), Ok(None)))
    }
}

impl Drop for Compositor {
    fn drop(&mut self) {
        let _ = self.try_command(&json!({"command": "shutdown"}));
        if let Some(mut child) = self.child.take() {
            let deadline = Instant::now() + Duration::from_secs(5);
            while matches!(child.try_wait(), Ok(None)) && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(25));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}
