//! Shared EIS (libei) session management for input injection.
//!
//! Provides two connection paths:
//! 1. **Portal** (primary): D-Bus `RemoteDesktop` portal → `ConnectToEIS` → FD.
//!    Used by Mutter (GNOME) and `KWin` (KDE).
//! 2. **Direct socket** (fallback): `LIBEI_SOCKET` environment variable.
//!    Used by the `PlatynUI` compositor.

use enumflags2::BitFlags;
use reis::PendingRequestResult;
use reis::ei;
use reis::event::{DeviceCapability, EiEvent, EiEventConverter};
use std::collections::HashMap;
use std::io;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use zbus::blocking::Connection;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

// ---------------------------------------------------------------------------
// Portal D-Bus proxy traits
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

/// Device type bitmask for `SelectDevices`: keyboard + pointer + touchscreen.
const DEVICE_TYPES_ALL: u32 = 0x7;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// An established EIS session, ready for input injection.
pub struct EiSession {
    pub context: ei::Context,
    pub converter: EiEventConverter,
    pub connection: reis::event::Connection,
    /// D-Bus connection that must stay alive for portal-based sessions.
    _portal_guard: Option<Connection>,
}

// ---------------------------------------------------------------------------
// Session establishment
// ---------------------------------------------------------------------------

/// Establish an EIS session, trying the portal first, then falling back to
/// a direct `LIBEI_SOCKET` connection.
pub fn establish_session(client_name: &str) -> Result<EiSession, String> {
    // Try portal first (works for KWin, Mutter).
    match connect_via_portal(client_name) {
        Ok(session) => return Ok(session),
        Err(e) => {
            tracing::debug!(error = %e, "portal EIS connection not available, trying LIBEI_SOCKET");
        }
    }

    // Fallback: direct socket (PlatynUI compositor, custom setups).
    connect_via_socket(client_name)
}

/// Connect to EIS via the XDG Desktop Portal `RemoteDesktop` interface.
fn connect_via_portal(client_name: &str) -> Result<EiSession, String> {
    let dbus = Connection::session().map_err(|e| format!("D-Bus session connect: {e}"))?;
    let portal = RemoteDesktopProxyBlocking::new(&dbus).map_err(|e| format!("RemoteDesktop proxy: {e}"))?;

    // Step 1: CreateSession
    let session_handle = {
        let token = "platynui_create";
        let mut signals = subscribe_response(&dbus, token)?;

        let options = HashMap::from([
            ("handle_token", Value::from(token)),
            ("session_handle_token", Value::from("platynui_session")),
        ]);

        portal.create_session(&options).map_err(|e| format!("CreateSession: {e}"))?;
        let results = wait_for_response(&mut signals)?;

        let handle_val = results
            .get("session_handle")
            .ok_or_else(|| "portal did not return session_handle".to_string())?;

        let session_path: String = handle_val.clone().try_into().or_else(|_| -> Result<String, String> {
            let path: OwnedObjectPath = handle_val
                .clone()
                .try_into()
                .map_err(|e| format!("session_handle is neither string nor object path: {e}"))?;
            Ok(path.to_string())
        })?;

        OwnedObjectPath::try_from(session_path).map_err(|e| format!("invalid session_handle path: {e}"))?
    };

    tracing::debug!(session = %session_handle, "portal: session created");

    // Step 2: SelectDevices
    {
        let token = "platynui_select";
        let mut signals = subscribe_response(&dbus, token)?;

        let options: HashMap<&str, Value<'_>> = HashMap::from([
            ("handle_token", Value::from(token)),
            ("types", Value::from(DEVICE_TYPES_ALL)),
            ("persist_mode", Value::from(2u32)),
        ]);

        portal
            .select_devices(&session_handle.as_ref(), &options)
            .map_err(|e| format!("SelectDevices: {e}"))?;
        wait_for_response(&mut signals)?;
    }

    // Step 3: Start (may show permission dialog)
    {
        let token = "platynui_start";
        let mut signals = subscribe_response(&dbus, token)?;

        let options = HashMap::from([("handle_token", Value::from(token))]);

        portal
            .start(&session_handle.as_ref(), "", &options)
            .map_err(|e| format!("Start: {e}"))?;
        wait_for_response(&mut signals)?;
    }

    // Step 4: ConnectToEIS
    let fd = portal
        .connect_to_eis(&session_handle.as_ref(), &HashMap::new())
        .map_err(|e| format!("ConnectToEIS: {e}"))?;

    let std_fd: std::os::fd::OwnedFd = fd.into();
    let stream = UnixStream::from(std_fd);

    let session = handshake(stream, client_name, Some(dbus))?;

    tracing::info!("EIS session established via portal");
    Ok(session)
}

/// Connect to EIS via the `LIBEI_SOCKET` environment variable.
fn connect_via_socket(client_name: &str) -> Result<EiSession, String> {
    let socket_path = resolve_ei_socket()?;
    tracing::debug!(path = %socket_path.display(), "connecting to EIS via socket");

    let stream = UnixStream::connect(&socket_path).map_err(|e| format!("EIS connect to {}: {e}", socket_path.display()))?;

    let session = handshake(stream, client_name, None)?;

    tracing::info!(path = %socket_path.display(), "EIS session established via socket");
    Ok(session)
}

/// Resolve the EIS socket path from the `LIBEI_SOCKET` environment variable.
fn resolve_ei_socket() -> Result<PathBuf, String> {
    let path = std::env::var("LIBEI_SOCKET").map_err(|_| {
        "LIBEI_SOCKET not set and portal connection failed — no EIS input injection available".to_string()
    })?;

    if std::path::Path::new(&path).is_relative() {
        let runtime_dir =
            std::env::var("XDG_RUNTIME_DIR").map_err(|_| "XDG_RUNTIME_DIR not set".to_string())?;
        Ok(PathBuf::from(runtime_dir).join(&path))
    } else {
        Ok(PathBuf::from(path))
    }
}

/// Perform the EIS handshake on an already-connected stream.
fn handshake(stream: UnixStream, client_name: &str, portal_guard: Option<Connection>) -> Result<EiSession, String> {
    let context = ei::Context::new(stream).map_err(|e| format!("EI context: {e}"))?;

    let resp = reis::handshake::ei_handshake_blocking(&context, client_name, ei::handshake::ContextType::Sender)
        .map_err(|e| format!("EI handshake: {e}"))?;

    let mut converter = EiEventConverter::new(&context, resp);
    let connection = converter.connection().clone();

    // Drain events buffered during the handshake.
    dispatch_buffered(&context, &mut converter)?;

    Ok(EiSession {
        context,
        converter,
        connection,
        _portal_guard: portal_guard,
    })
}

// ---------------------------------------------------------------------------
// Device discovery
// ---------------------------------------------------------------------------

/// Wait for a resumed EIS device with the given capability.
pub fn find_device(
    session: &mut EiSession,
    required: DeviceCapability,
) -> Result<reis::event::Device, String> {
    const TIMEOUT: Duration = Duration::from_secs(5);
    let deadline = Instant::now() + TIMEOUT;

    loop {
        while let Some(event) = session.converter.next_event() {
            match event {
                EiEvent::SeatAdded(ref seat) => {
                    seat.seat.bind_capabilities(BitFlags::all());
                    session.connection.flush().map_err(|e| format!("flush: {e}"))?;
                }
                EiEvent::DeviceResumed(ref dev) => {
                    if dev.device.has_capability(required) {
                        return Ok(dev.device.clone());
                    }
                }
                EiEvent::Disconnected(ref disc) => {
                    return Err(format!("EIS disconnected waiting for {required:?}: {:?}", disc.reason));
                }
                _ => {}
            }
        }

        if Instant::now() >= deadline {
            return Err(format!("timeout waiting for EIS device with {required:?} capability"));
        }

        match try_read_and_dispatch(&session.context, &mut session.converter, Some(deadline)) {
            Ok(true) => {}
            Ok(false) => {
                return Err(format!("timeout waiting for EIS device with {required:?} capability"));
            }
            Err(e) => return Err(format!("EIS read error: {e}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Protocol helpers
// ---------------------------------------------------------------------------

/// Drain already-queued events from the `EiEventConverter` without blocking.
fn dispatch_buffered(context: &ei::Context, converter: &mut EiEventConverter) -> Result<(), String> {
    let _ = context.read();
    while let Some(result) = context.pending_event() {
        match result {
            PendingRequestResult::Request(event) => {
                converter.handle_event(event).map_err(|e| format!("EI protocol error: {e}"))?;
            }
            PendingRequestResult::ParseError(e) => {
                return Err(format!("EI parse error: {e}"));
            }
            PendingRequestResult::InvalidObject(_) => {}
        }
    }
    Ok(())
}

/// Poll the EIS socket with an optional deadline, then read and dispatch events.
fn try_read_and_dispatch(
    context: &ei::Context,
    converter: &mut EiEventConverter,
    deadline: Option<Instant>,
) -> Result<bool, io::Error> {
    use rustix::event::{PollFd, PollFlags, poll};
    use rustix::time::Timespec;

    let mut poll_fds = [PollFd::new(context, PollFlags::IN)];

    loop {
        let timeout = deadline.map(|dl| {
            let remaining = dl.saturating_duration_since(Instant::now());
            Timespec {
                tv_sec: remaining.as_secs().cast_signed(),
                tv_nsec: i64::from(remaining.subsec_nanos()),
            }
        });

        if let Some(ref ts) = timeout
            && ts.tv_sec == 0
            && ts.tv_nsec == 0
        {
            return Ok(false);
        }

        match poll(&mut poll_fds, timeout.as_ref()) {
            Ok(0) => return Ok(false),
            Ok(_) => break,
            Err(rustix::io::Errno::INTR) => {}
            Err(e) => return Err(e.into()),
        }
    }

    context.read()?;
    dispatch_buffered(context, converter).map_err(io::Error::other)?;
    Ok(true)
}

/// Current timestamp in microseconds for EI frame events.
#[expect(clippy::cast_possible_truncation)]
pub fn timestamp_us() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

// ---------------------------------------------------------------------------
// Portal helpers
// ---------------------------------------------------------------------------

/// Derives the predicted Request object path for a given `handle_token`.
fn request_object_path(connection: &Connection, handle_token: &str) -> Result<String, String> {
    let unique_name = connection
        .unique_name()
        .ok_or_else(|| "D-Bus connection has no unique name".to_string())?;
    let sender = unique_name.as_str().trim_start_matches(':').replace('.', "_");
    Ok(format!("/org/freedesktop/portal/desktop/request/{sender}/{handle_token}"))
}

/// Subscribe to the `Response` signal on the predicted Request object path.
/// Must be called _before_ the portal method that triggers the Request.
fn subscribe_response(connection: &Connection, handle_token: &str) -> Result<ResponseIterator, String> {
    let path = request_object_path(connection, handle_token)?;

    let request_proxy = PortalRequestProxyBlocking::builder(connection)
        .path(path.as_str())
        .map_err(|e| format!("Request proxy path: {e}"))?
        .build()
        .map_err(|e| format!("Request proxy build: {e}"))?;

    request_proxy
        .receive_response()
        .map_err(|e| format!("subscribe to Response signal: {e}"))
}

/// Wait for the portal `Response` signal and return the results dictionary.
fn wait_for_response(signals: &mut ResponseIterator) -> Result<HashMap<String, OwnedValue>, String> {
    let signal = signals
        .next()
        .ok_or_else(|| "Response signal stream ended unexpectedly".to_string())?;

    let args = signal.args().map_err(|e| format!("parse Response args: {e}"))?;

    if args.response != 0 {
        return Err(format!("portal request denied (response code {})", args.response));
    }

    Ok(args.results.clone())
}
