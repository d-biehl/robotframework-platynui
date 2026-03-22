use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

use platynui_core::platform::PlatformError;
use serde_json::Value;
use tracing::debug;

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

fn discover_control_socket_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("PLATYNUI_CONTROL_SOCKET") {
        return Some(PathBuf::from(path));
    }
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let wayland_display = std::env::var("WAYLAND_DISPLAY").ok()?;
    Some(PathBuf::from(runtime_dir).join(format!("{wayland_display}.control")))
}
