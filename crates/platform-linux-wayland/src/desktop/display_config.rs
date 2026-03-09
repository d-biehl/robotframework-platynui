//! Display configuration enrichment via compositor-specific D-Bus APIs.
//!
//! Wayland protocols provide only integer scale factors and no "primary
//! monitor" concept. This module queries compositor-specific D-Bus
//! interfaces (Mutter `DisplayConfig`, `KWin`) to enrich [`OutputInfo`] with:
//!
//! - Exact fractional scale (e.g. 1.25 instead of integer 2)
//! - Primary monitor flag
//!
//! The public entry point is [`enrich_outputs()`].

use std::collections::HashMap;

use tracing::{debug, warn};
use zbus::zvariant::OwnedValue;

use super::OutputInfo;
use crate::capabilities::CompositorType;

// ---------------------------------------------------------------------------
//  Mutter D-Bus types — `GetCurrentState()` signature:
//  (u, a((ssss)a(siiddada{sv})a{sv}), a(iiduba(ssss)a{sv}), a{sv})
// ---------------------------------------------------------------------------

/// Physical monitor mode: `(siiddada{sv})`.
type MutterMode = (String, i32, i32, f64, f64, Vec<f64>, HashMap<String, OwnedValue>);

/// Physical monitor ID: `(ssss)` — connector, vendor, product, serial.
type MutterMonitorId = (String, String, String, String);

/// Physical monitor: `((ssss)a(siiddada{sv})a{sv})`.
type MutterPhysicalMonitor = (MutterMonitorId, Vec<MutterMode>, HashMap<String, OwnedValue>);

/// Logical monitor: `(iiduba(ssss)a{sv})`.
type MutterLogicalMonitor = (i32, i32, f64, u32, bool, Vec<MutterMonitorId>, HashMap<String, OwnedValue>);

/// Full `GetCurrentState()` reply body.
type MutterCurrentState = (u32, Vec<MutterPhysicalMonitor>, Vec<MutterLogicalMonitor>, HashMap<String, OwnedValue>);

// ---------------------------------------------------------------------------
//  Public API
// ---------------------------------------------------------------------------

/// Enrich output information with compositor-specific data from D-Bus.
///
/// Queries the running compositor's display configuration API and updates
/// each [`OutputInfo`] with fractional scale and primary monitor flag.
/// Falls back gracefully when the D-Bus call fails or returns unexpected data.
pub fn enrich_outputs(compositor: CompositorType, outputs: &mut [OutputInfo]) {
    // Reset primary flags — will be set by compositor query or fallback.
    for o in outputs.iter_mut() {
        o.is_primary = false;
    }

    let enriched = match compositor {
        CompositorType::Mutter => enrich_from_mutter(outputs),
        CompositorType::KWin => enrich_from_kwin(outputs),
        _ => false,
    };

    if !enriched {
        debug!("display config: no compositor enrichment available, using fallbacks");
    }

    // Ensure at least one monitor is marked primary.
    if !outputs.iter().any(|o| o.is_primary) {
        if let Some(o) = outputs.iter_mut().find(|o| o.effective_x() == 0 && o.effective_y() == 0) {
            o.is_primary = true;
            debug!("primary monitor fallback: output at (0, 0)");
        } else if let Some(o) = outputs.first_mut() {
            o.is_primary = true;
            debug!("primary monitor fallback: first output");
        }
    }
}

// ---------------------------------------------------------------------------
//  Mutter / GNOME
// ---------------------------------------------------------------------------

/// Enrich outputs from `org.gnome.Mutter.DisplayConfig.GetCurrentState()`.
///
/// Each logical monitor provides exact fractional scale and `is_primary`.
/// Physical monitors in the response are matched to our outputs by
/// connector name (e.g. "DP-6", "HDMI-1").
fn enrich_from_mutter(outputs: &mut [OutputInfo]) -> bool {
    let Ok(conn) = zbus::blocking::Connection::session() else {
        debug!("Mutter: cannot connect to session bus");
        return false;
    };

    let result = conn.call_method(
        Some("org.gnome.Mutter.DisplayConfig"),
        "/org/gnome/Mutter/DisplayConfig",
        Some("org.gnome.Mutter.DisplayConfig"),
        "GetCurrentState",
        &(),
    );

    let Ok(reply) = result else {
        debug!("Mutter: GetCurrentState() call failed");
        return false;
    };

    let body = reply.body();
    let Ok((_, _, logical_monitors, _)) = body.deserialize::<MutterCurrentState>() else {
        warn!("Mutter: could not deserialize GetCurrentState() response");
        return false;
    };

    let mut matched = false;

    for (_, _, scale, _, is_primary, monitors, _) in &logical_monitors {
        for (connector, _, _, _) in monitors {
            if let Some(o) = outputs.iter_mut().find(|o| o.name.as_deref() == Some(connector.as_str())) {
                o.fractional_scale = Some(*scale);
                o.is_primary = *is_primary;
                debug!(connector = connector.as_str(), scale, is_primary, "Mutter: enriched output");
                matched = true;
            }
        }
    }

    if !matched {
        debug!("Mutter: no connector names matched any output");
    }
    matched
}

// ---------------------------------------------------------------------------
//  KWin / KDE Plasma
// ---------------------------------------------------------------------------

/// Enrich outputs from `KWin` D-Bus interfaces.
///
/// `KWin` exposes `primaryOutputName` via `org.kde.KWin`. Fractional scale
/// and further properties can be obtained from `org.kde.KScreen2.GetConfig()`
/// in future iterations.
fn enrich_from_kwin(outputs: &mut [OutputInfo]) -> bool {
    let Ok(conn) = zbus::blocking::Connection::session() else {
        debug!("KWin: cannot connect to session bus");
        return false;
    };

    let mut enriched = false;

    // Primary monitor via org.kde.KWin.primaryOutputName.
    let result = conn.call_method(
        Some("org.kde.KWin"),
        "/KWin",
        Some("org.freedesktop.DBus.Properties"),
        "Get",
        &("org.kde.KWin", "primaryOutputName"),
    );

    if let Ok(reply) = result {
        let body = reply.body();
        if let Ok(variant) = body.deserialize::<zbus::zvariant::Value<'_>>()
            && let Ok(name) = variant.downcast_ref::<&str>()
            && let Some(o) = outputs.iter_mut().find(|o| o.name.as_deref() == Some(name))
        {
            o.is_primary = true;
            debug!(name, "KWin: primary monitor identified");
            enriched = true;
        }
    }

    if !enriched {
        debug!("KWin: could not determine primary output");
    }
    enriched
}
