## 1. Regression & acceptance scenarios (write first, from spec scenarios)

- [ ] 1.1 Record the current baseline: `just test-acceptance-x11` fails multi-suite with `x11 connection: not available after shutdown or failed connect` (the `per-runtime-platform-lifecycle` "sequential runtimes reconnect" scenario, currently red).
- [ ] 1.2 Add an acceptance scenario (real AT-SPI/X11) that a runtime built in a later suite draws a highlight successfully — `per-runtime-platform-lifecycle` "Highlight works in a later suite's runtime". Currently red (silent highlight death).
- [ ] 1.3 Add an acceptance scenario (real X11) that a `config`-bound runtime connects to an explicitly named `platform.x11.display` regardless of ambient `DISPLAY` — `runtime-session-config` "Explicit X11 display overrides the environment" + `per-runtime-platform-lifecycle` "config-bound runtime connects to the named session". Written against the not-yet-existing `config=`; red until §7.
- [ ] 1.4 Add an acceptance scenario (real X11) that two `BareMetal` instances (two runtimes) coexist on the same display and both query the app successfully — `per-runtime-platform-lifecycle` "runtimes share no platform state". Red until per-runtime ownership lands.

## 2. Rust core: PlatformFactory + RuntimeConfig (test-first)

- [x] 2.1 Write core unit tests for `RuntimeConfig` parsing and resolution: `platform`/`providers` grouped by component id; unclaimed bucket/id/key retained-but-unclaimed and type-mismatch degrades to `None` (the core-testable `runtime-session-config` scenarios). The config-value → env → auto-detect precedence is verified at the backend level (3.1, 6.1), where env is actually read.
- [x] 2.2 Define the `RuntimeConfig { platform, providers }` type, the loosely-typed leaf value (`ConfigValue`/`ConfigMap`), and typed accessors (`get_str`, `get_bool`, …) in `crates/core/src/config.rs`.
- [x] 2.3 Define the `PlatformFactory` trait (`id` / `can_serve(&RuntimeConfig)` / `create(&RuntimeConfig) -> Result<PlatformBundle, PlatformError>`), the `PlatformBundle` owned-device struct (`Arc<dyn …>` fields — see design D1/D2), and `inventory` registration + `register_platform_factory!` in `crates/core/src/platform/factory.rs`.
- [x] 2.4 Make 2.1 green (`cargo test -p platynui-core` — config + factory tests pass; clippy/fmt clean).

## 3. Rust X11 backend: per-instance ownership

- [x] 3.1 Reconnect/isolation guarantee is proven by the multi-suite `just test-acceptance-x11` lane (§9.4), now **26/26** where it was 16/26 — each suite builds a fresh runtime after the previous is dropped and reconnects. (A mock unit test is not the right vehicle: the mock shares process-global state and has no connection to re-establish; the real lane is the meaningful guard.)
- [x] 3.2 Replaced `static X11` with an owned `X11Connection` (`X11Connection::connect(display)` → `Arc`); deleted `shutdown_connection()`/`X11Guard`; `connect_raw`/`resolve_display` take the config display → env fallback. (Keymap `STATE` + atoms `ATOMS` kept as server-stable global caches, not the bug — loaders now take `&X11Connection`; relocation deferred.)
- [x] 3.3 X11 highlight is per-instance: `LinuxHighlightProvider` owns an `OverlayController` (own `tx` + `JoinHandle`), thread spawned with the session display, dropped+joined on `Drop`; `static CTRL`/`shutdown_highlight`/`global()` removed.
- [x] 3.4 X11 pointer/keyboard/screenshot/window-manager/desktop devices hold `Arc<X11Connection>` (constructors `::new(Arc<X11Connection>)`); crate builds + 9 tests pass standalone.

## 4. Rust platform mediator, mock, and Windows highlight

- [x] 4.1 `platform-linux` mediator is now two `PlatformFactory`s (`X11Factory`, `WaylandFactory`) selecting by `config.platform.backend` or `session_type()` auto-detect; `static RESOLVED`, the `PlatformModule`, and all wrapper-device registrations removed. Builds clean.
- [x] 4.2 `WaylandFactory::create` calls `create_wayland_bundle` (wraps the existing `set_global_and_start`/`input::initialize` globals); teardown-on-drop deferred (non-goal, `TODO(wayland-internalization)`). `WaylandModule` removed.
- [x] 4.3 `MockPlatformFactory` (id `"mock"`, `can_serve` only when `platform.backend=="mock"`) + `create_mock_bundle` in `platform-mock`; `use_mock`/`new_with_mock` route through it.
- [ ] 4.4 Windows: give `platform-windows` a `PlatformFactory` (`create_windows_bundle`) so Windows stays functional under the factory-only runtime, and make the highlight controller per-instance (remove `static CTRL`, fixes suite-2+ highlight). Compile-checked via `just check-windows`; runtime-verified on a Windows host (follow-up task, this dev box is Linux). **Required to avoid a Windows runtime regression.**

## 5. Rust runtime: own the bundle, delete the lease

- [x] 5.1 `Runtime` holds `platform: Option<PlatformBundle>` + `config: RuntimeConfig` instead of `&'static` device fields + `platform_guard`; `select_platform` resolves the factory by `platform.backend` (hard error if a named backend is missing/can't-serve) else the first `can_serve`, else `None`. The bundle's wm is injected into every provider (D9); desktop-info/pointer-engine come from the bundle. `PointerEngine` now owns `Arc<dyn PointerDevice>`.
- [x] 5.2 `create()` rollback lives in each factory (`create_x11_bundle` releases on failure); the runtime maps create errors to `InitializationFailed`.
- [x] 5.3 Deleted `platform_modules.rs` (`PlatformModulesLease`, `PLATFORM_MODULES_STATE`, `platform_overrides_require_global_modules`). `shutdown` clears the pointer engine then drops the bundle (closes connection, joins threads). Lease-ordering tests removed.
- [x] 5.4 Reconnect/isolation covered by the acceptance lane (see 3.1); runtime unit tests reworked to the mock-backend config path (84 pass).
- [x] 5.5 `Runtime::new()` stays no-arg (cli/playground/inspector untouched); `new_with_factories_and_platforms`/`PlatformOverrides` replaced by `new_with_config` / `new_with_factories_and_config`; `crates/cli/src/test_support.rs` + native mock path updated.

## 6. Rust provider factory: AT-SPI explicit bus

- [x] 6.1 Wrote `AtspiFactory` tests (`factory_reads_configured_bus_address`, `factory_defaults_to_env_discovery_without_config`) via a testable `AtspiFactory::build` — the config → `bus_address` wiring is unit-covered; the live override is acceptance-verified. Green.
- [x] 6.2 `UiTreeProviderFactory::create(&self, config: &RuntimeConfig)` (symmetric with `PlatformFactory`); AT-SPI reads `providers.atspi.bus_address` and routes it through new `connection::connect_a11y_bus_with` (config → env). All four providers + registry + runtime bridge updated; clippy clean (windows-uia cross-checked in §9).

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
