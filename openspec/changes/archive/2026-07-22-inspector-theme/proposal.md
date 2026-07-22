## Why

The Inspector never follows the system light/dark theme on Linux: it forces dark visuals at startup (a leftover from the Slint→egui migration), and even without that line the toolkit stack gives Linux nothing to follow — winit's `theme()` is hard-wired `None` on X11 and only echoes app-set values on Wayland, so egui's default `ThemePreference::System` silently falls back to dark. On Windows and macOS winit does report the system theme, so the platform-consistent behavior users expect from a desktop tool already half-exists. The freshly redesigned toolbar and status bar are built theme-correct (panels + text-color-tinted icons), but the light-theme scenarios of the `inspector-toolbar` spec are currently unverifiable because no code path can ever produce a light Inspector on Linux.

## What Changes

- **Theme preference setting**: a persisted `theme` setting — System (default), Light, Dark — in `inspector.ron`, editable in the existing Settings dialog, mapping 1:1 to egui's `ThemePreference`.
- **Linux system-theme detection** via the XDG Desktop Portal (`org.freedesktop.portal.Settings`, key `org.freedesktop.appearance` / `color-scheme`), including live updates through the `SettingChanged` signal, implemented with `zbus` (already in the dependency tree via the AT-SPI provider). Desktop-environment-agnostic and identical on X11 and Wayland.
- **Deterministic fallback**: when no portal answers (e.g. the PlatynUI compositor test session) or the portal reports "no preference", System resolves to dark — today's look, keeping test lanes deterministic.
- **Remove the forced `set_visuals(Visuals::dark())`** in `run()`; set egui's `fallback_theme = Dark` explicitly instead. On Windows/macOS the System preference then follows winit's reported theme (which effectively already happens today).
- **Ephemeral theme override for testing**: a `--theme system|light|dark` CLI flag plus a `PLATYNUI_INSPECTOR_THEME` environment variable, following the existing `--renderer`/`PLATYNUI_INSPECTOR_RENDERER` pattern. Precedence: CLI > environment > persisted setting. The override is never written back to `inspector.ron`.
- Behavior change (not BREAKING): on a light-themed Windows/macOS/Linux-with-portal desktop, a fresh Inspector now starts light instead of dark. Users who prefer the old look set the preference to Dark once.

## Capabilities

### New Capabilities
- `inspector-theme`: the Inspector's theme contract — the System/Light/Dark preference with its persistence and System default, Linux color-scheme detection via the XDG Desktop Portal with live updates and the dark fallback, and the ephemeral CLI/environment override for test runs.

### Modified Capabilities

None. `inspector-toolbar`'s "no unthemed strip in light theme" and "icons legible in both themes" requirements are unchanged — this change is what finally makes them exercisable on Linux.

## Impact

- **Layer:** Rust only, entirely within `apps/inspector` (`lib.rs` startup/CLI/settings, `settings_dialog.rs`, a new Linux-only theme-detection module). No native (PyO3) rebuild, no Python/Robot Framework surface change.
- **Dependencies:** `zbus` becomes a direct Linux-only dependency of the inspector crate (version already pinned in the workspace lockfile via the AT-SPI provider; no new transitive weight).
- **Platforms:** Linux gains detection via the portal (GNOME, KDE, wlroots setups with a portal); Windows/macOS System preference rides on winit's existing theme reporting; environments without a portal (including the PlatynUI compositor lane and bare X sessions) fall back to dark.
- **Tests:** unit tests for the preference/override resolution (precedence, portal-value mapping); settings round-trip extended for the new field. Acceptance lanes stay green unchanged — the a11y contracts are color-independent and the test sessions have no portal, so runs remain dark and deterministic; the `--theme` override enables scripted light-theme visual passes.
- **Docs:** `dev-docs/inspector.md` (theme behavior, setting, override).
