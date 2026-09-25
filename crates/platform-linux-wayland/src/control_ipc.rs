use std::fmt;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use platynui_core::platform::PlatformError;
use serde_json::Value;
use tracing::debug;

/// How long the initialization handshake waits for an answer. It runs every
/// time a runtime initializes, so a socket that accepts a connection and never
/// answers must not stall that for the 5 s capability calls get.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(1);

/// The `compositor` value in a `PlatynUI` compositor's `status` response.
const COMPOSITOR_IDENTITY: &str = "platynui";

/// What the control socket said when asked whether a `PlatynUI` compositor
/// answers on it. Only [`Identified`](Self::Identified) decides anything; every
/// other outcome leaves the decision to the remaining evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HandshakeOutcome {
    /// A `PlatynUI` compositor answered, naming its Wayland socket if it did.
    Identified { socket_name: Option<String> },
    /// Something answered, but not as a `PlatynUI` compositor.
    NoMarker,
    /// No control-socket path is configured.
    NoSocketPath,
    /// Connecting to the socket failed.
    ConnectFailed(String),
    /// The request or its answer failed, or no answer came in time.
    ExchangeFailed(String),
}

impl fmt::Display for HandshakeOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Identified { socket_name: Some(name) } => write!(f, "identified (Wayland socket {name})"),
            Self::Identified { socket_name: None } => f.write_str("identified"),
            Self::NoMarker => f.write_str("answered without the PlatynUI identity marker"),
            Self::NoSocketPath => f.write_str(
                "no control socket configured (PLATYNUI_CONTROL_SOCKET, or XDG_RUNTIME_DIR and WAYLAND_DISPLAY)",
            ),
            Self::ConnectFailed(error) => write!(f, "connect failed: {error}"),
            Self::ExchangeFailed(error) => write!(f, "no usable answer: {error}"),
        }
    }
}

/// Ask the control socket at `socket_path` whether a `PlatynUI` compositor
/// answers on it: one `status` request, bounded by a short timeout of its own.
pub(crate) fn handshake(socket_path: Option<&Path>) -> HandshakeOutcome {
    match socket_path {
        Some(path) => handshake_at(path, HANDSHAKE_TIMEOUT),
        None => HandshakeOutcome::NoSocketPath,
    }
}

fn handshake_at(path: &Path, timeout: Duration) -> HandshakeOutcome {
    let stream = match UnixStream::connect(path) {
        Ok(stream) => stream,
        Err(error) => return HandshakeOutcome::ConnectFailed(error.to_string()),
    };
    let exchange = || -> std::io::Result<String> {
        stream.set_read_timeout(Some(timeout))?;
        stream.set_write_timeout(Some(timeout))?;
        let mut writer = &stream;
        writeln!(writer, "{}", serde_json::json!({ "command": "status" }))?;
        writer.flush()?;
        let mut line = String::new();
        BufReader::new(&stream).read_line(&mut line)?;
        Ok(line)
    };
    match exchange() {
        Ok(line) if line.is_empty() => HandshakeOutcome::ExchangeFailed("closed without answering".into()),
        Ok(line) => classify_status(&line),
        Err(error) if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {
            HandshakeOutcome::ExchangeFailed(format!("no answer within {} ms", timeout.as_millis()))
        }
        Err(error) => HandshakeOutcome::ExchangeFailed(error.to_string()),
    }
}

/// Read a `status` answer: identified only with the `PlatynUI` identity marker.
fn classify_status(line: &str) -> HandshakeOutcome {
    let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
        return HandshakeOutcome::NoMarker;
    };
    let field = |name: &str| value.get(name).and_then(Value::as_str);
    if field("status") == Some("ok") && field("compositor") == Some(COMPOSITOR_IDENTITY) {
        HandshakeOutcome::Identified { socket_name: field("socket").map(str::to_owned) }
    } else {
        HandshakeOutcome::NoMarker
    }
}

pub(crate) fn send_command(command: &Value, operation: &'static str) -> Result<Value, PlatformError> {
    let socket_path = discover_control_socket_path().ok_or_else(|| PlatformError::CapabilityUnavailable {
        capability: "control socket path discovery",
        details: Some("set PLATYNUI_CONTROL_SOCKET or ensure WAYLAND_DISPLAY is set".into()),
    })?;

    let mut stream = UnixStream::connect(&socket_path).map_err(|error| PlatformError::InitializationFailed {
        component: "control socket connection",
        details: Some(format!("{}: {error}", socket_path.display())),
    })?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(5))).ok();

    writeln!(stream, "{command}").map_err(|error| PlatformError::OperationFailed {
        operation: "control socket write",
        details: Some(error.to_string()),
    })?;
    stream.flush().map_err(|error| PlatformError::OperationFailed {
        operation: "control socket flush",
        details: Some(error.to_string()),
    })?;

    let mut response = String::new();
    let mut reader = BufReader::new(stream);
    reader.read_line(&mut response).map_err(|error| PlatformError::OperationFailed {
        operation: "control socket read",
        details: Some(error.to_string()),
    })?;

    let value: Value = serde_json::from_str(response.trim()).map_err(|error| PlatformError::OperationFailed {
        operation: "control socket decode JSON",
        details: Some(error.to_string()),
    })?;

    if value.get("status").and_then(Value::as_str) != Some("ok") {
        let message = value.get("message").and_then(Value::as_str).unwrap_or("unknown error");
        return Err(PlatformError::OperationFailed { operation, details: Some(message.into()) });
    }

    debug!(command = %command, operation, "control socket request succeeded");
    Ok(value)
}

pub(crate) fn discover_control_socket_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PLATYNUI_CONTROL_SOCKET") {
        return Some(PathBuf::from(path));
    }
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok()?;
    Some(PathBuf::from(runtime_dir).join(format!("{wayland_display}.control")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::thread;

    /// A socket path of its own for each test, removed when dropped.
    struct SocketPath(PathBuf);

    impl SocketPath {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("platynui-handshake-{}-{name}.sock", std::process::id()));
            let _ = std::fs::remove_file(&path);
            Self(path)
        }
    }

    impl Drop for SocketPath {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    /// Serve one connection: read the request line, then answer with `answer`
    /// (`None`: never answer, holding the connection open until `hold` ends).
    fn serve_once(path: &Path, answer: Option<&'static str>, hold: Duration) -> thread::JoinHandle<String> {
        let listener = UnixListener::bind(path).expect("bind test socket");
        thread::spawn(move || {
            let (stream, _) = listener.accept().expect("accept");
            let mut request = String::new();
            BufReader::new(&stream).read_line(&mut request).expect("read request");
            match answer {
                Some(answer) => writeln!(&stream, "{answer}").expect("answer"),
                None => thread::sleep(hold),
            }
            request
        })
    }

    #[test]
    fn a_platynui_compositor_is_identified_with_its_socket_name() {
        let path = SocketPath::new("identified");
        let server =
            serve_once(&path.0, Some(r#"{"status":"ok","compositor":"platynui","socket":"wl-0"}"#), Duration::ZERO);
        let outcome = handshake(Some(&path.0));
        assert_eq!(outcome, HandshakeOutcome::Identified { socket_name: Some("wl-0".into()) });
        let request: Value = serde_json::from_str(&server.join().unwrap()).unwrap();
        assert_eq!(request["command"], "status", "the handshake asks for the status");
    }

    #[test]
    fn an_answer_without_the_marker_identifies_nothing() {
        for answer in [r#"{"status":"ok","version":"1.0"}"#, r#"{"status":"ok","compositor":"other"}"#, "hello"] {
            let path = SocketPath::new("no-marker");
            let server = serve_once(&path.0, Some(answer), Duration::ZERO);
            assert_eq!(handshake(Some(&path.0)), HandshakeOutcome::NoMarker, "{answer}");
            server.join().unwrap();
        }
    }

    #[test]
    fn a_peer_that_never_answers_times_out() {
        let path = SocketPath::new("silent");
        let server = serve_once(&path.0, None, Duration::from_millis(500));
        let outcome = handshake_at(&path.0, Duration::from_millis(100));
        assert!(
            matches!(&outcome, HandshakeOutcome::ExchangeFailed(reason) if reason.contains("100 ms")),
            "{outcome:?}"
        );
        server.join().unwrap();
    }

    #[test]
    fn a_path_nothing_listens_on_fails_to_connect() {
        let path = SocketPath::new("nobody");
        assert!(matches!(handshake(Some(&path.0)), HandshakeOutcome::ConnectFailed(_)));
    }

    #[test]
    fn no_configured_path_is_its_own_outcome() {
        assert_eq!(handshake(None), HandshakeOutcome::NoSocketPath);
    }
}
