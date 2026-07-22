## 1. Preference model and override resolution (test-first, pure logic)

- [x] 1.1 Add `ThemeChoice { System, Light, Dark }` (clap `ValueEnum` + serde, default `System`) with a `resolve` following the `RendererChoice` pattern (`--theme` CLI flag beats `PLATYNUI_INSPECTOR_THEME`, invalid env values warn and are ignored) and an effective-preference helper (`override.unwrap_or(persisted)` → `egui::ThemePreference`); unit tests first: CLI-over-env precedence, invalid env ignored, mapping of all three values
- [x] 1.2 Add `theme: ThemeChoice` to `PersistedSettings` with `#[serde(default)]`; extend the existing round-trip unit tests: new field round-trips, pre-existing RON without the field loads as `System`, `save()` writes the persisted value even while an override is active

## 2. Startup wiring and Settings dialog

- [x] 2.1 Remove `cc.egui_ctx.set_visuals(egui::Visuals::dark())` from `run()`; at app construction apply the effective preference via `ctx.set_theme(...)` and set `Options::fallback_theme = Theme::Dark` explicitly
- [x] 2.2 Add a "Theme" radio row (System / Light / Dark) to the Settings dialog editing the persisted value; apply changes to `ctx.set_theme` immediately; when a CLI/env override is active, show a short note that this run is pinned by the override
- [x] 2.3 Keep the toolbar/status-bar rendering unchanged and confirm nothing else hard-codes dark (grep for `Visuals::dark`/`set_visuals`)

## 3. Linux portal watcher

- [x] 3.1 Add `zbus` as a Linux-only dependency of `apps/inspector` (workspace-locked version) and a cfg-gated `theme_watch` module: blocking session-bus connection, initial `ReadOne("org.freedesktop.appearance", "color-scheme")` with fallback to the legacy `Read` (nested variant), then follow `SettingChanged` filtered on namespace+key; publish values through a shared atomic and `ctx.request_repaint()`; exit silently on any connect/read failure
- [x] 3.2 Map portal values in a pure, unit-tested helper: `2` → Light, `1`/`0`/unknown → Dark
- [x] 3.3 Apply watcher updates in `logic()`: when the shared value changed since last applied, write it into `Options::fallback_theme` (System preference resolves through it; forced Light/Dark are unaffected)

## 4. Verification and docs

- [x] 4.1 `just check` and `just test` (workspace gates)
- [ ] 4.2 Run the egui acceptance lane unchanged and confirm it stays green and dark (no portal in the compositor session → fallback path proven); judge via `robotcode results`
- [x] 4.3 Scripted visual pass in a real session using the override: `--theme light` and `--theme dark` screenshots (toolbar icons tinted correctly in both, no unthemed regions — exercises the `inspector-toolbar` light-theme scenarios), plus a run on a portal desktop with System to see detection and, if possible, a live switch
- [x] 4.4 Update `dev-docs/inspector.md` (theme behavior and precedence, the setting, the `--theme`/`PLATYNUI_INSPECTOR_THEME` override, portal detection and dark fallback)
