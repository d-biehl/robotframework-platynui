//! Portal backend — XDG Desktop Portal `RemoteDesktop` → `ConnectToEIS`.
//!
//! Uses the portal's `CreateSession` → `SelectDevices` → `Start` →
//! `ConnectToEIS` flow to negotiate a remote desktop session. The portal
//! returns an EIS FD which is then handled identically to the direct EIS
//! backend.
//!
//! Token persistence (`persist_mode=2`) avoids repeated consent dialogs.
//! Restore tokens are single-use — after each session the compositor
//! returns a new token that must be saved immediately.
//!
//! Primary path for Mutter and `KWin`.

use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use platynui_core::platform::{
    KeyCode, KeyboardError, KeyboardEvent, PlatformError, PlatformErrorKind, PointerButton, ScrollDelta,
};
use platynui_core::types::Point;
use tracing::{debug, info, warn};
use zbus::blocking::Connection;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

use super::InputBackend;
use super::eis::EisBackend;
use crate::capabilities::CompositorType;

/// Device type bitmask for `SelectDevices`: keyboard + pointer + touchscreen.
const DEVICE_TYPES_ALL: u32 = 0x7;

// ---------------------------------------------------------------------------
//  Portal D-Bus proxy traits
// ---------------------------------------------------------------------------

#[zbus::proxy(
    interface = "org.freedesktop.portal.RemoteDesktop",
    default_service = "org.freedesktop.portal.Desktop",
    default_path = "/org/freedesktop/portal/desktop"
)]
trait RemoteDesktop {
    fn create_session(&self, options: &HashMap<&str, Value<'_>>) -> zbus::Result<OwnedObjectPath>;

    fn select_devices(
        &self,
        session_handle: &ObjectPath<'_>,
        options: &HashMap<&str, Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;

    fn start(
        &self,
        session_handle: &ObjectPath<'_>,
        parent_window: &str,
        options: &HashMap<&str, Value<'_>>,
    ) -> zbus::Result<OwnedObjectPath>;

    #[zbus(name = "ConnectToEIS")]
    fn connect_to_eis(
        &self,
        session_handle: &ObjectPath<'_>,
        options: &HashMap<&str, Value<'_>>,
    ) -> zbus::Result<zbus::zvariant::OwnedFd>;
}

#[zbus::proxy(interface = "org.freedesktop.portal.Request", default_service = "org.freedesktop.portal.Desktop")]
trait PortalRequest {
    #[zbus(signal)]
    fn response(&self, response: u32, results: HashMap<String, OwnedValue>);
}

// ---------------------------------------------------------------------------
//  Portal backend — wraps an EIS backend obtained via the portal
// ---------------------------------------------------------------------------

/// Portal input backend that obtains an EIS connection via XDG Desktop Portal.
///
/// Internally delegates all input operations to an [`EisBackend`] created
/// from the portal-provided FD. The D-Bus connection is kept alive to
/// maintain the portal session.
pub(crate) struct PortalBackend {
    /// The underlying EIS backend handling actual input.
    eis: EisBackend,
    /// D-Bus connection — must stay alive for the portal session.
    _dbus_connection: Connection,
}

impl PortalBackend {
    /// Connect via the XDG Desktop Portal.
    pub(crate) fn connect(compositor_type: CompositorType) -> Result<Self, PlatformError> {
        let (stream, dbus_connection) = connect_via_portal()?;

        // Wrap the portal-provided stream in an EIS backend. We re-use the
        // EIS backend's handshake + device discovery logic, but construct it
        // from the already-connected stream.
        let eis = EisBackend::from_stream(stream, compositor_type)?;

        Ok(Self { eis, _dbus_connection: dbus_connection })
    }
}

impl InputBackend for PortalBackend {
    fn name(&self) -> &'static str {
        "Portal"
    }

    fn key_to_code(&self, name: &str) -> Result<KeyCode, KeyboardError> {
        self.eis.key_to_code(name)
    }

    fn start_input(&self) -> Result<(), KeyboardError> {
        self.eis.start_input()
    }

    fn send_key_event(&self, event: KeyboardEvent) -> Result<(), KeyboardError> {
        self.eis.send_key_event(event)
    }

    fn end_input(&self) -> Result<(), KeyboardError> {
        self.eis.end_input()
    }

    fn known_key_names(&self) -> Vec<String> {
        self.eis.known_key_names()
    }

    fn pointer_position(&self) -> Result<Point, PlatformError> {
        self.eis.pointer_position()
    }

    fn pointer_move_to(&self, point: Point) -> Result<(), PlatformError> {
        self.eis.pointer_move_to(point)
    }

    fn pointer_press(&self, button: PointerButton) -> Result<(), PlatformError> {
        self.eis.pointer_press(button)
    }

    fn pointer_release(&self, button: PointerButton) -> Result<(), PlatformError> {
        self.eis.pointer_release(button)
    }

    fn pointer_scroll(&self, delta: ScrollDelta) -> Result<(), PlatformError> {
        self.eis.pointer_scroll(delta)
    }
}

// ---------------------------------------------------------------------------
//  Portal connection flow
// ---------------------------------------------------------------------------

/// Execute the full portal flow and return the EIS stream + D-Bus connection.
fn connect_via_portal() -> Result<(UnixStream, Connection), PlatformError> {
    let connection = Connection::session().map_err(|e| {
        PlatformError::new(PlatformErrorKind::InitializationFailed, format!("D-Bus session connection failed: {e}"))
    })?;

    let portal = RemoteDesktopProxyBlocking::new(&connection).map_err(|e| {
        PlatformError::new(PlatformErrorKind::CapabilityUnavailable, format!("RemoteDesktop portal not available: {e}"))
    })?;

    let restore_token = load_restore_token();

    // Step 1: CreateSession
    debug!("portal: CreateSession");
    let session_handle = {
        let token = "platynui_create";
        let mut signals = subscribe_response(&connection, token)?;

        let options = HashMap::from([
            ("handle_token", Value::from(token)),
            ("session_handle_token", Value::from("platynui_session")),
        ]);

        portal.create_session(&options).map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("portal CreateSession failed: {e}"))
        })?;

        let results = wait_for_response(&mut signals)?;
        let session_handle_val = results.get("session_handle").ok_or_else(|| {
            PlatformError::new(PlatformErrorKind::OperationFailed, "portal did not return session_handle")
        })?;

        let session_path: String = session_handle_val.clone().try_into().or_else(|_| {
            let path: OwnedObjectPath = session_handle_val.clone().try_into().map_err(|e| {
                PlatformError::new(PlatformErrorKind::OperationFailed, format!("invalid session_handle type: {e}"))
            })?;
            Ok(path.to_string())
        })?;

        OwnedObjectPath::try_from(session_path).map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("invalid session_handle path: {e}"))
        })?
    };

    // Step 2: SelectDevices
    debug!("portal: SelectDevices");
    {
        let token = "platynui_select";
        let mut signals = subscribe_response(&connection, token)?;

        let mut options: HashMap<&str, Value<'_>> = HashMap::from([
            ("handle_token", Value::from(token)),
            ("types", Value::from(DEVICE_TYPES_ALL)),
            ("persist_mode", Value::from(2u32)),
        ]);

        if let Some(ref rt) = restore_token {
            debug!("portal: using restore token from previous session");
            options.insert("restore_token", Value::from(rt.as_str()));
        }

        portal.select_devices(&session_handle, &options).map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("portal SelectDevices failed: {e}"))
        })?;

        wait_for_response(&mut signals)?;
    }

    // Step 3: Start (may show permission dialog)
    debug!("portal: Start");
    {
        let token = "platynui_start";
        let mut signals = subscribe_response(&connection, token)?;

        let options = HashMap::from([("handle_token", Value::from(token))]);

        portal
            .start(&session_handle, "", &options)
            .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("portal Start failed: {e}")))?;

        let results = wait_for_response(&mut signals)?;

        // Save restore token for future sessions (single-use tokens).
        if let Some(new_token) = results.get("restore_token").and_then(|v| {
            let s: Result<String, _> = v.clone().try_into();
            s.ok().filter(|s| !s.is_empty())
        }) {
            if let Err(e) = save_restore_token(&new_token) {
                warn!(error = %e, "failed to save portal restore token");
            } else {
                info!("portal: saved new restore token");
            }
        }
    }

    // Step 4: ConnectToEIS
    debug!("portal: ConnectToEIS");
    let fd = portal.connect_to_eis(&session_handle, &HashMap::new()).map_err(|e| {
        PlatformError::new(PlatformErrorKind::OperationFailed, format!("portal ConnectToEIS failed: {e}"))
    })?;

    let std_fd: std::os::fd::OwnedFd = fd.into();
    let stream = UnixStream::from(std_fd);
    info!("portal: EIS connection established");

    Ok((stream, connection))
}

// ---------------------------------------------------------------------------
//  Portal helpers
// ---------------------------------------------------------------------------

fn subscribe_response(connection: &Connection, handle_token: &str) -> Result<ResponseIterator, PlatformError> {
    let unique_name = connection
        .unique_name()
        .ok_or_else(|| PlatformError::new(PlatformErrorKind::OperationFailed, "D-Bus connection has no unique name"))?;
    let sender = unique_name.as_str().trim_start_matches(':').replace('.', "_");
    let path = format!("/org/freedesktop/portal/desktop/request/{sender}/{handle_token}");

    let request_proxy = PortalRequestProxyBlocking::builder(connection)
        .path(path.as_str())
        .map_err(|e| PlatformError::new(PlatformErrorKind::OperationFailed, format!("invalid portal path: {e}")))?
        .build()
        .map_err(|e| {
            PlatformError::new(PlatformErrorKind::OperationFailed, format!("portal request proxy failed: {e}"))
        })?;

    request_proxy.receive_response().map_err(|e| {
        PlatformError::new(PlatformErrorKind::OperationFailed, format!("failed to subscribe to portal response: {e}"))
    })
}

fn wait_for_response(signals: &mut ResponseIterator) -> Result<HashMap<String, OwnedValue>, PlatformError> {
    let signal = signals
        .next()
        .ok_or_else(|| PlatformError::new(PlatformErrorKind::OperationFailed, "portal response signal stream ended"))?;

    let args = signal.args().map_err(|e| {
        PlatformError::new(PlatformErrorKind::OperationFailed, format!("failed to parse portal response: {e}"))
    })?;

    if args.response != 0 {
        return Err(PlatformError::new(
            PlatformErrorKind::OperationFailed,
            format!("portal request denied (response code {})", args.response),
        ));
    }

    Ok(args.results.clone())
}

// ---------------------------------------------------------------------------
//  Token persistence
// ---------------------------------------------------------------------------

fn token_path() -> PathBuf {
    let base = std::env::var("XDG_DATA_HOME").map_or_else(
        |_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".local/share")
        },
        PathBuf::from,
    );
    base.join("platynui/portal-restore-token")
}

fn load_restore_token() -> Option<String> {
    let path = token_path();
    match std::fs::read_to_string(&path) {
        Ok(token) => {
            let token = token.trim().to_string();
            if token.is_empty() {
                None
            } else {
                debug!(path = %path.display(), "loaded portal restore token");
                Some(token)
            }
        }
        Err(_) => None,
    }
}

fn save_restore_token(token: &str) -> Result<(), std::io::Error> {
    let path = token_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, token)
}
