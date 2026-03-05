//! XDG Desktop Portal `RemoteDesktop` integration for obtaining an EI file descriptor.
//!
//! Uses the portal's `CreateSession` → `SelectDevices` → `Start` → `ConnectToEIS` flow
//! to negotiate a remote desktop session and receive an EI socket FD from the compositor.

use std::collections::HashMap;
use std::os::unix::net::UnixStream;

use anyhow::{Context, anyhow};
use zbus::blocking::Connection;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

/// Opaque handle that keeps the portal D-Bus session alive.
/// Drop it to close the portal session (which also tears down the EIS socket).
pub type PortalGuard = Connection;

/// Device type bitmask for `SelectDevices`: keyboard + pointer + touchscreen.
const DEVICE_TYPES_ALL: u32 = 0x7;

// ---------------------------------------------------------------------------
// Portal D-Bus proxy traits (zbus generates blocking variants automatically)
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
// Public API
// ---------------------------------------------------------------------------

/// Connects to an EIS server via the XDG Desktop Portal `RemoteDesktop` interface.
///
/// This goes through the portal's `CreateSession` → `SelectDevices` → `Start` →
/// `ConnectToEIS` flow. The user may be prompted by the desktop environment to
/// grant permission (unless a valid `restore_token` is provided from a previous session).
///
/// Returns the EIS stream, the D-Bus connection (must stay alive), and an optional
/// new restore token for future sessions.
pub fn connect_via_portal(restore_token: Option<&str>) -> anyhow::Result<(UnixStream, Connection, Option<String>)> {
    let connection = Connection::session().context("failed to connect to session D-Bus")?;

    let portal = RemoteDesktopProxyBlocking::new(&connection).context("failed to create RemoteDesktop proxy")?;

    // Step 1: CreateSession
    tracing::debug!("portal: CreateSession");
    let session_handle = {
        let token = "eis_test_create";
        let mut signals = subscribe_response(&connection, token)?;

        let options = HashMap::from([
            ("handle_token", Value::from(token)),
            ("session_handle_token", Value::from("eis_test_session")),
        ]);

        portal.create_session(&options)?;
        let results = wait_for_response(&mut signals)?;

        let session_handle_val =
            results.get("session_handle").ok_or_else(|| anyhow!("portal did not return session_handle"))?;

        // The portal may return the session handle as either a string (s) or object path (o).
        let session_path: String = session_handle_val.clone().try_into().or_else(|_| -> anyhow::Result<String> {
            let path: OwnedObjectPath = session_handle_val
                .clone()
                .try_into()
                .map_err(|e| anyhow!("session_handle is neither string nor object path: {e}"))?;
            Ok(path.to_string())
        })?;

        let session_handle =
            OwnedObjectPath::try_from(session_path).map_err(|e| anyhow!("invalid session_handle path: {e}"))?;

        tracing::debug!(session = %session_handle, "portal: session created");
        session_handle
    };

    // Step 2: SelectDevices
    tracing::debug!("portal: SelectDevices");
    {
        let token = "eis_test_select";
        let mut signals = subscribe_response(&connection, token)?;

        let mut options: HashMap<&str, Value<'_>> = HashMap::from([
            ("handle_token", Value::from(token)),
            ("types", Value::from(DEVICE_TYPES_ALL)),
            ("persist_mode", Value::from(2u32)),
        ]);

        if let Some(rt) = restore_token {
            tracing::debug!("portal: using restore token from previous session");
            options.insert("restore_token", Value::from(rt));
        }

        portal.select_devices(&session_handle.as_ref(), &options)?;
        wait_for_response(&mut signals)?;
    }

    // Step 3: Start
    tracing::debug!("portal: Start (may show permission dialog)");
    let new_restore_token: Option<String>;
    {
        let token = "eis_test_start";
        let mut signals = subscribe_response(&connection, token)?;

        let options = HashMap::from([("handle_token", Value::from(token))]);

        portal.start(&session_handle.as_ref(), "", &options)?;
        let results = wait_for_response(&mut signals)?;

        if let Some(devices) = results.get("devices") {
            tracing::debug!(devices = ?devices, "portal: session started");
        }

        new_restore_token = results.get("restore_token").and_then(|v| {
            let s: Result<String, _> = v.clone().try_into();
            s.ok().filter(|s| !s.is_empty())
        });
        if new_restore_token.is_some() {
            tracing::info!("portal: received restore token for future sessions");
        }
    }

    // Step 4: ConnectToEIS (direct FD return — no Request/Response pattern)
    tracing::debug!("portal: ConnectToEIS");
    let fd = portal.connect_to_eis(&session_handle.as_ref(), &HashMap::new()).context("ConnectToEIS failed")?;

    let std_fd: std::os::fd::OwnedFd = fd.into();
    let stream = UnixStream::from(std_fd);
    tracing::info!("portal: EIS connection established");

    // Return: the stream, the D-Bus connection (keep alive!), and the restore token.
    Ok((stream, connection, new_restore_token))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Derives the predicted Request object path for a given `handle_token`.
fn request_object_path(connection: &Connection, handle_token: &str) -> anyhow::Result<String> {
    let unique_name = connection.unique_name().ok_or_else(|| anyhow!("D-Bus connection has no unique name"))?;
    let sender = unique_name.as_str().trim_start_matches(':').replace('.', "_");
    Ok(format!("/org/freedesktop/portal/desktop/request/{sender}/{handle_token}"))
}

/// Subscribes to the `Response` signal on the predicted Request object path.
///
/// **Must** be called _before_ the portal method that triggers the Request, otherwise the
/// Response signal might be missed due to a race condition.
fn subscribe_response(connection: &Connection, handle_token: &str) -> anyhow::Result<ResponseIterator> {
    let path = request_object_path(connection, handle_token)?;

    let request_proxy = PortalRequestProxyBlocking::builder(connection)
        .path(path.as_str())?
        .build()
        .context("failed to create Request proxy")?;

    request_proxy.receive_response().context("failed to subscribe to Request.Response signal")
}

/// Waits for the portal `Response` signal and returns the results dictionary.
fn wait_for_response(signals: &mut ResponseIterator) -> anyhow::Result<HashMap<String, OwnedValue>> {
    let signal = signals.next().ok_or_else(|| anyhow!("Response signal stream ended unexpectedly"))?;

    let args = signal.args().context("failed to parse Response args")?;

    if args.response != 0 {
        return Err(anyhow!("portal request denied (response code {})", args.response));
    }

    Ok(args.results.clone())
}
