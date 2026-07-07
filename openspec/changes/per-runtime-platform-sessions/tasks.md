## 1. Regression & acceptance scenarios (write first, from spec scenarios)

- [ ] 1.1 Record the current baseline: `just test-acceptance-x11` fails multi-suite with `x11 connection: not available after shutdown or failed connect` (the `per-runtime-platform-lifecycle` "sequential runtimes reconnect" scenario, currently red).
- [ ] 1.2 Add an acceptance scenario (real AT-SPI/X11) that a runtime built in a later suite draws a highlight successfully — `per-runtime-platform-lifecycle` "Highlight works in a later suite's runtime". Currently red (silent highlight death).
- [ ] 1.3 Add an acceptance scenario (real X11) that a `config`-bound runtime connects to an explicitly named `platform.x11.display` regardless of ambient `DISPLAY` — `runtime-session-config` "Explicit X11 display overrides the environment" + `per-runtime-platform-lifecycle` "config-bound runtime connects to the named session". Written against the not-yet-existing `config=`; red until §7.
- [ ] 1.4 Add an acceptance scenario (real X11) that two `BareMetal` instances (two runtimes) coexist on the same display and both query the app successfully — `per-runtime-platform-lifecycle` "runtimes share no platform state". Red until per-runtime ownership lands.

## 2. Rust core: PlatformFactory + RuntimeConfig (test-first)

- [ ] 2.1 Write core unit tests for `RuntimeConfig` parsing and resolution: `platform`/`providers` grouped by component id; precedence config value → env → auto-detect; unclaimed bucket/id/key ignored with a `tracing::debug!` (the `runtime-session-config` scenarios). Red.
- [ ] 2.2 Define the `RuntimeConfig { platform, providers }` type, the loosely-typed leaf value, and typed accessors (`get_str`, …) in `crates/core`.
- [ ] 2.3 Define the `PlatformFactory` trait (`id` / `can_serve(&RuntimeConfig)` / `create(&RuntimeConfig) -> Result<PlatformBundle, PlatformError>`), the `PlatformBundle` owned-device struct, and `inventory` registration — mirroring `crates/core/src/platform/` device registration against `crates/core/src/provider/` factories.
- [ ] 2.4 Make 2.1 green.

## 3. Rust X11 backend: per-instance ownership

- [ ] 3.1 Write a backend-level test (via the mock platform factory) that `create → drop → create` re-establishes a working bundle, and that a failed `create` does not poison a later `create` — `per-runtime-platform-lifecycle` "built after teardown connects freshly" / "failed connect does not poison". Red.
- [ ] 3.2 Replace `static X11: OnceLock<…>` (`platform-linux-x11/src/x11util.rs:16`) with an owned `X11Connection` (connection + root + keymap + atoms) shared via `Arc`; delete `shutdown_connection()`; devices hold `Arc` clones. `connect_raw` takes an explicit display (config value → env fallback).
- [ ] 3.3 Make the X11 highlight controller per-instance: owned `OverlayController` on the highlight device, overlay thread signalled + joined on `Drop`; remove `static CTRL` / `shutdown_highlight` (`platform-linux-x11/src/highlight.rs:33,48`).
- [ ] 3.4 Convert X11 pointer/keyboard/screenshot/window-manager/desktop devices to instances over the shared `Arc<X11Connection>` (move the `keyboard.rs` `STATE` keymap and `window_manager.rs` `ATOMS` onto the connection).

## 4. Rust platform mediator, mock, and Windows highlight

- [ ] 4.1 Convert the `platform-linux` mediator (`platform-linux/src/lib.rs`) from a `PlatformModule` + `static RESOLVED` into a `PlatformFactory` that selects X11 vs Wayland from `config.platform.backend` or env auto-detect; remove `static RESOLVED`.
- [ ] 4.2 Wayland branch (Phase-1 scope): its `create()` drives the existing `connection::set_global_and_start` and the bundle's `Drop` calls `clear_global` / `input::shutdown` / `desktop::clear_outputs` — correct for one Wayland runtime at a time; leave the Wayland globals internalization to the follow-up phase (non-goal).
- [ ] 4.3 Add a mock `PlatformFactory` selected by `platform.backend="mock"` (so `use_mock` / `Runtime::new_with_mock` route through the same path).
- [ ] 4.4 Make the Windows highlight controller per-instance; remove `static CTRL` (`platform-windows/src/highlight.rs:66`) — fixes the suite-2+ silent highlight death. Keep the crate green from Linux via `just check-windows` / `just clippy-windows`.

## 5. Rust runtime: own the bundle, delete the lease

- [ ] 5.1 `Runtime` holds an owned `PlatformBundle` + `RuntimeConfig` instead of `Option<&'static dyn Device>` + `platform_guard` (`runtime/mod.rs:43-54`); resolve the active factory by `can_serve` / `platform.backend`.
- [ ] 5.2 `create()` rolls back partially-built devices on failure (moving the lease's rollback from `platform_modules.rs:39-42`).
- [ ] 5.3 Delete `PlatformModulesLease`, `PLATFORM_MODULES_STATE`, and `platform_overrides_require_global_modules`; update `Runtime::new` / `new_with_provider_ids` / `new_with_factories*` / `shutdown` / `Drop`.
- [ ] 5.4 Make the §3.1 mock reconnect/isolation tests green.
- [ ] 5.5 Keep `Runtime::new()` no-arg (empty config) so `crates/cli` (`lib.rs:144`), `crates/playground` (`main.rs:10`), and `apps/inspector` (`lib.rs:834`) need no change; rework `new_with_factories_and_platforms` + `PlatformOverrides` for the lease-free model and update `crates/cli/src/test_support.rs`.

## 6. Rust provider factory: AT-SPI explicit bus

- [ ] 6.1 Write a test that `AtspiFactory::create` honors `providers.atspi.bus_address`, falling back to env discovery when absent — `runtime-session-config` "Explicit AT-SPI bus address overrides discovery". Red.
- [ ] 6.2 Give `UiTreeProviderFactory::create` access to its `providers.<id>` sub-config and use `bus_address` in `provider-atspi/src/connection.rs` (config value → env). Make 6.1 green.

## 7. Python / PyO3 binding (after native rebuild)

- [ ] 7.1 Rebuild the native module (`just build-native`) so the new `Runtime(config)` surface is available to Python — sequence gate before the RF work.
- [ ] 7.2 `packages/native/src/runtime.rs`: `Runtime(config)` accepts and parses the nested config `dict` into `RuntimeConfig`, and `new_with_mock()` routes through the `platform.backend="mock"` path; update `packages/native/python/platynui_native/_native.pyi`.
- [ ] 7.3 Write the RF **mock** suite for `BareMetal(config=…)`: empty/absent config = current behavior, an override reaches the mock backend, an unclaimed key is ignored (`runtime-session-config` scenarios that the mock can observe). Red.
- [ ] 7.4 `src/PlatynUI/BareMetal/__init__.py`: add the `config=` import argument, forward it through `_create_runtime()` (`:787-818`); reconcile `use_mock` as sugar for `platform.backend="mock"`. Make 7.3 and 1.3 green.

## 8. Documentation

- [ ] 8.1 `dev-docs/architecture.md` — platform/runtime lifecycle: per-runtime ownership + the `PlatformFactory`/bundle model; remove the reference-counted-lease description.
- [ ] 8.2 `dev-docs/platform-linux.md` — X11 connection/keymap/atoms/highlight are owned per runtime; the mediator is a factory; no process-global connection.
- [ ] 8.3 `dev-docs/python-bindings.md` — the `Runtime(config)` surface and the config → factory fan-out.
- [ ] 8.4 `dev-docs/python-library-design.md` — `BareMetal config=`, session binding, and the relationship to the scoped behavioral dicts.
- [ ] 8.5 `dev-docs/testing-strategy.md` — why the multi-suite X11 acceptance lane is now green; note the `x11-connection-cached-across-runtimes` memory / narrow reconnect fix is superseded.
- [ ] 8.6 `BareMetal` library docstring + any `docs/` user documentation — document `config=`, its `platform`/`providers` shape, the empty-config default, and a portable-across-OS example.

## 9. Verification

- [ ] 9.1 `just check` (fmt, clippy, ruff, mypy) and `just check-windows` / `just clippy-windows` for the Windows highlight change.
- [ ] 9.2 `just test` (Rust) — `RuntimeConfig` parsing/precedence/tolerance and the mock create→drop→create reconnect/isolation tests pass.
- [ ] 9.3 `just test-python` — the `BareMetal config=` mock suite passes.
- [ ] 9.4 `just test-acceptance-x11` multi-suite (real AT-SPI/X11) — every suite green; the reconnect, later-suite-highlight, and config-bound-display scenarios (§1) pass. This is the regression guard for the original bug.
- [ ] 9.5 Windows highlight runtime-verified on a real Windows desktop via a dedicated task (this dev machine is Linux; the acceptance run happens on a Windows host).
