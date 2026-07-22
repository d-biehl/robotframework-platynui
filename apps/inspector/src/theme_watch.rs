//! System color-scheme detection for the "System" theme preference.
//!
//! Substance is Linux-only: the watcher reads `org.freedesktop.appearance` /
//! `color-scheme` from the XDG Desktop Portal (`org.freedesktop.portal.Settings`
//! on the session bus) and follows the `SettingChanged` signal. It publishes
//! the raw portal value (0 = no preference, 1 = prefer dark, 2 = prefer light)
//! into a shared atomic and requests a repaint; the app maps the value into
//! egui's *fallback* theme — which egui only consults while the preference is
//! System and the windowing system reports no theme itself. That is exactly
//! the Linux situation (winit's `theme()` is `None` on X11 and app-echo-only
//! on Wayland); on Windows/macOS winit reports the theme and the fallback is
//! never consulted. Without a portal the cell stays at [`NO_SIGNAL`] and the
//! Inspector stays dark — deterministic for the acceptance lanes' portal-less
//! sessions.

use eframe::egui;
use std::sync::Arc;
use std::sync::atomic::AtomicU8;

/// Sentinel for "no portal value received": distinct from every real portal
/// answer so "no preference" (0) and "no signal at all" map differently.
pub const NO_SIGNAL: u8 = u8::MAX;

/// Map a portal `color-scheme` value onto the egui theme used as the
/// System-preference fallback. "Prefer light" (2) and "no preference" (0)
/// yield the light theme — no preference means the default appearance, which
/// is light by freedesktop/GTK convention (GNOME and DankMaterialShell report
/// their light mode as `'default'`/0 and rarely set `prefer-light`). "Prefer
/// dark" (1), [`NO_SIGNAL`], and unknown values stay dark — identical to a
/// session without a portal.
pub fn theme_from_color_scheme(scheme: u8) -> egui::Theme {
    match scheme {
        0 | 2 => egui::Theme::Light,
        _ => egui::Theme::Dark,
    }
}

/// Spawn the watcher as a daemon-style background thread (it lives until
/// process exit, like the picker's modifier readers). Returns the shared
/// cell holding the latest portal value. On non-Linux platforms, and
/// whenever no portal answers, the cell simply stays at [`NO_SIGNAL`].
pub fn spawn(ctx: egui::Context) -> Arc<AtomicU8> {
    let shared = Arc::new(AtomicU8::new(NO_SIGNAL));
    #[cfg(target_os = "linux")]
    {
        let shared_for_thread = Arc::clone(&shared);
        let spawned = std::thread::Builder::new().name("theme-watch".into()).spawn(move || {
            // Failure is normal (no bus, no portal, no key): the dark
            // fallback stands and startup is never delayed or disturbed.
            if let Err(err) = linux::watch(&ctx, &shared_for_thread) {
                tracing::debug!(%err, "system color-scheme watcher stopped");
            }
        });
        if let Err(err) = spawned {
            tracing::warn!(%err, "failed to spawn the system color-scheme watcher");
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = ctx;
    shared
}

#[cfg(target_os = "linux")]
mod linux {
    use eframe::egui;
    use std::sync::atomic::{AtomicU8, Ordering};
    use zbus::zvariant::Value;

    const NAMESPACE: &str = "org.freedesktop.appearance";
    const KEY: &str = "color-scheme";

    pub(super) fn watch(ctx: &egui::Context, shared: &AtomicU8) -> Result<(), Box<dyn std::error::Error>> {
        let connection = zbus::blocking::Connection::session()?;
        let proxy = zbus::blocking::Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
        )?;

        if let Some(scheme) = read_initial(&proxy)? {
            publish(scheme, shared, ctx);
        }

        let signals = proxy.receive_signal("SettingChanged")?;
        for message in signals {
            let body = message.body();
            let (namespace, key, value): (&str, &str, Value<'_>) = body.deserialize()?;
            if namespace == NAMESPACE
                && key == KEY
                && let Some(scheme) = scheme_from_value(&value)
            {
                publish(scheme, shared, ctx);
            }
        }
        Ok(())
    }

    /// Initial read: `ReadOne` on current portals, falling back to the
    /// deprecated `Read`, whose reply nests the value in a second variant.
    /// `None` for an unreadable value — the no-signal sentinel then stands.
    fn read_initial(proxy: &zbus::blocking::Proxy<'_>) -> Result<Option<u32>, zbus::Error> {
        let value: zbus::zvariant::OwnedValue = match proxy.call("ReadOne", &(NAMESPACE, KEY)) {
            Ok(value) => value,
            Err(_) => proxy.call("Read", &(NAMESPACE, KEY))?,
        };
        Ok(scheme_from_value(&value))
    }

    fn scheme_from_value(value: &Value<'_>) -> Option<u32> {
        match value {
            Value::U32(scheme) => Some(*scheme),
            Value::Value(inner) => scheme_from_value(inner),
            _ => None,
        }
    }

    fn publish(scheme: u32, shared: &AtomicU8, ctx: &egui::Context) {
        shared.store(u8::try_from(scheme).unwrap_or(super::NO_SIGNAL), Ordering::Relaxed);
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::{NO_SIGNAL, theme_from_color_scheme};
    use eframe::egui;

    #[test]
    fn prefer_light_yields_light() {
        assert_eq!(theme_from_color_scheme(2), egui::Theme::Light);
    }

    #[test]
    fn no_preference_is_the_light_default_appearance() {
        // GNOME and DankMaterialShell report their light mode as 0/'default'.
        assert_eq!(theme_from_color_scheme(0), egui::Theme::Light);
    }

    #[test]
    fn prefer_dark_yields_dark() {
        assert_eq!(theme_from_color_scheme(1), egui::Theme::Dark);
    }

    #[test]
    fn no_signal_and_unknown_values_yield_dark() {
        assert_eq!(theme_from_color_scheme(NO_SIGNAL), egui::Theme::Dark);
        assert_eq!(theme_from_color_scheme(7), egui::Theme::Dark);
    }
}
