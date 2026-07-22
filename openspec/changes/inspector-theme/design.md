## Context

The Inspector today is always dark, for two stacked reasons — both verified in source:

- The app forces dark visuals at startup (`cc.egui_ctx.set_visuals(egui::Visuals::dark())`, `apps/inspector/src/lib.rs:1007`), unchanged since the Slint→egui migration (commit f8b8b70), with no documented reason.
- Even without that line, Linux has nothing to follow: winit's `theme()` is hard-wired `None` on X11 (winit-0.30.13 `src/platform_impl/linux/x11/window.rs:1905`) and on Wayland only echoes what the app itself set (`src/platform_impl/linux/wayland/window/mod.rs:706`). egui 0.34 defaults to `ThemePreference::System` and resolves it from `RawInput::system_theme`; when that is `None` it uses `Options::fallback_theme`, whose default is `Theme::Dark` (egui-0.34.3 `src/memory/mod.rs:199-206,313`).

On Windows and macOS winit does report the system theme, so egui's System preference already works there — the forced `set_visuals` at creation time writes dark visuals into whichever theme slot is current at startup, which is what actually pins the look.

The standard Linux mechanism for the user's color-scheme preference is the XDG Desktop Portal: `org.freedesktop.portal.Settings` on the session bus, namespace `org.freedesktop.appearance`, key `color-scheme` (`0` = no preference, `1` = prefer dark, `2` = prefer light), with the `SettingChanged` signal for live updates. It is desktop-environment-agnostic and identical under X11 and Wayland. `zbus` 5.14 is already pinned in the workspace lockfile (via the AT-SPI provider stack), so a hand-rolled portal client adds no new dependency weight — full portal crates (`ashpd`) or detector crates (`dark-light`) would.

The Inspector already has an established CLI-flag + environment-variable resolution pattern to imitate: `RendererChoice::resolve` / `GlowHardwareAccelerationChoice::resolve` / `SearchResultLimitChoice::resolve` (`apps/inspector/src/lib.rs:158,280,356`). Persisted settings live in `PersistedSettings` (`lib.rs:53`, `#[serde(default)]`, RON via eframe storage) and are edited in `settings_dialog.rs` (picker combo checkboxes, toolbar-style radios).

Acceptance context: the PlatynUI compositor test session runs no portal, and the a11y contracts the egui lane asserts are color-independent — so System-with-no-portal must resolve to dark to keep runs looking like today.

## Goals / Non-Goals

**Goals:**

- A persisted theme preference — System (default), Light, Dark — editable in the Settings dialog.
- System follows the OS on all platforms: winit-reported theme on Windows/macOS, XDG-portal color-scheme (with live updates) on Linux, dark where neither answers.
- An ephemeral `--theme` / `PLATYNUI_INSPECTOR_THEME` override for scripted test runs, never persisted.
- Remove the startup `set_visuals(dark)`; the light-theme scenarios of `inspector-toolbar` become exercisable.

**Non-Goals:**

- No custom color schemes or per-widget theming — only egui's stock light/dark visuals.
- No portal integration beyond `color-scheme` (no accent color, no contrast).
- No theme detection in other PlatynUI components (CLI, compositor); this is Inspector-only.
- No attempt to theme native window decorations (winit CSD theming stays untouched).

## Decisions

### D1: Preference is a three-value enum persisted like the toolbar style

`ThemeChoice { System, Light, Dark }`, default `System`, stored as a new `#[serde(default)]` field in `PersistedSettings` and edited as a radio row in the Settings dialog (same pattern as `ToolbarStyle`). Old RON files load unchanged. Light/Dark map 1:1 onto `egui::ThemePreference::{Light, Dark}`; System maps onto `ThemePreference::System`.

### D2: Linux detection feeds egui's `fallback_theme`, not its preference

egui resolves `ThemePreference::System` from `RawInput::system_theme`, falling back to `Options::fallback_theme` — and on Linux `system_theme` is always `None` (see Context). Instead of fighting that resolution (mutating raw input, or rewriting the preference to a concrete Light/Dark and thereby lying to the Settings dialog), the Linux portal watcher writes the detected scheme into `fallback_theme` via `ctx.options_mut`:

- portal answers `1` (prefer dark) → `fallback_theme = Dark`
- portal answers `2` (prefer light) or `0` (no preference) → `fallback_theme = Light` — "no preference" means the default appearance, which is light by freedesktop/GTK convention; GNOME and DankMaterialShell report their light mode as `'default'` (0) and rarely set `prefer-light` (verified live under niri/DMS: the light toggle writes `color-scheme = 'default'`)
- no portal / connect or read error / unreadable value → the shared state stays at a no-signal sentinel and `fallback_theme` stays `Dark`

This keeps one uniform rule on every platform — the user preference is always mapped verbatim to `ThemePreference`, and System resolves through egui's own machinery: winit's report where it exists, the portal-fed fallback on Linux, dark otherwise. Windows/macOS are untouched by the watcher (their `system_theme` is `Some`, so `fallback_theme` is never consulted).

Alternative rejected: computing the effective theme in the app and always setting a concrete `ThemePreference::{Light,Dark}` — duplicates egui's resolution, breaks live OS switching on Windows/macOS unless reimplemented, and makes the Settings dialog state ambiguous.

### D3: Portal client is a hand-rolled zbus blocking watcher on one background thread

A small Linux-only module (`theme_watch.rs`, cfg-gated like `modifiers.rs`) using `zbus::blocking`: connect to the session bus, call `ReadOne("org.freedesktop.appearance", "color-scheme")` (falling back to the older `Read` method whose reply is a nested variant), then block on the `SettingChanged` signal stream. Each value is stored in an `Arc<AtomicU8>` shared with the app and followed by `ctx.request_repaint()`; the app's `logic()` applies changes to `fallback_theme` when the stored value differs from the last applied one. The thread is spawned once at startup and never joined (daemon-style; the process exit tears it down) — same lifecycle the picker's modifier reader uses. If the initial connect or read fails (no bus, no portal, no key), the watcher exits silently and the dark fallback stands.

Alternatives rejected: `ashpd` (full portal client, large async surface for one key) and `dark-light` (detection-only crates pull their own portal stack; live-update support varies) — `zbus` is already in the tree at the exact version the lockfile pins.

### D4: Override resolution copies the renderer pattern

`--theme system|light|dark` (clap `ValueEnum`) plus `PLATYNUI_INSPECTOR_THEME`, resolved exactly like `RendererChoice::resolve` (`lib.rs:158`): CLI wins over environment; invalid environment values log a warning and are ignored. The resolved override is `Option<ThemeChoice>` — `None` means "use the persisted setting". The override is applied at startup and pins the theme for the session:

- effective preference = `override.unwrap_or(persisted)`
- `save()` always writes the *persisted* value, never the override
- the Settings dialog keeps editing the persisted value; while an override is active it shows a short note that the running instance is pinned by `--theme`/environment (edits still persist for the next run)

### D5: Startup wiring replaces the forced dark visuals

`cc.egui_ctx.set_visuals(egui::Visuals::dark())` (`lib.rs:1007`) is removed. At app construction the effective preference is applied via `ctx.set_theme(...)`, and `fallback_theme` is set to `Theme::Dark` explicitly (documenting the no-signal default rather than relying on egui's default staying dark). The Linux watcher then adjusts `fallback_theme` as portal values arrive.

### D6: Testing posture

The resolution logic (CLI/env precedence, portal-value → theme mapping, effective-preference computation) is pure and unit-tested; the settings round-trip test is extended for the new field. Portal presence, live switching, and actual rendered colors are only verifiable against a real desktop — covered by a scripted visual pass using `--theme light`/`--theme dark` under the compositor session (which itself has no portal, proving the fallback path). The egui acceptance lane needs no changes: its assertions are color-independent and its sessions resolve to dark exactly as before.

## Risks / Trade-offs

- [Portal "no preference" (0) initially mapped to dark] → revised after live testing under niri/DankMaterialShell: light mode there (and on GNOME) reports `0` (`'default'`), so a 0→dark mapping would keep the Inspector dark on light desktops. Now `0` → light per freedesktop convention; only a truly absent/unreadable signal (tracked by a sentinel distinct from `0`) stays dark, which keeps the portal-less test lanes deterministic.
- [Fresh behavior change on light desktops: Inspector starts light after the upgrade] → intended per proposal; one-time Settings change (Dark) restores the old look permanently.
- [A second session-bus connection from the Inspector (besides AccessKit's own)] → connections are independent and cheap; the watcher holds one blocking connection and one thread, nothing shared with AT-SPI.
- [`SettingChanged` floods (some DEs emit many unrelated settings)] → filter on namespace+key before storing; storing an u8 and requesting a repaint is cheap even unfiltered.
- [zbus blocking API spins an internal executor thread] → accepted; same cost class as the existing X11/compositor modifier readers.
- [eframe may cache visuals set at creation] → verified: `set_visuals` writes the current theme's style only (egui-0.34.3 `src/context.rs:2234`); removing it and steering `theme_preference`/`fallback_theme` is the supported path.

## Migration Plan

Behavioral change confined to the `platynui-inspector` binary; no native (PyO3) rebuild, no Python/RF surface change. `inspector.ron` gains one `#[serde(default)]` field — old files load unchanged (System default), files written by the new version load in the old binary (struct-level `#[serde(default)]`, no `deny_unknown_fields`). Rollback = revert the commits; settings files stay compatible both ways.

## Open Questions

- Whether the Settings dialog should offer a live preview hint ("current effective theme: dark (portal)") — nice for debugging portal issues, but can land later; not spec-relevant.
