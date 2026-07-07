## 1. Regression & acceptance scenarios (write first, from spec scenarios)

- [x] 1.1 Baseline recorded (16/26, `x11 connection: not available after shutdown`) and now the pass proof: the multi-suite `just test-acceptance-x11` lane is **26/26** — the "sequential runtimes reconnect" scenario.
- [x] 1.2 Covered by the existing multi-suite lane: highlight is exercised in the Interaction suite (not the first suite to run), so "highlight works in a later suite's runtime" is proven green within the 26/26 — no separate scenario needed.
- [x] 1.3 Dedicated acceptance scenario added: `tests/acceptance/egui/config_display.robot` — a `config`-bound explicit `platform.x11.display` (+ `providers.atspi.bus_address`) drives the real session, and a wrong config display (`:987`) fails though the environment `DISPLAY` is valid, proving the config value overrides the environment. X11-only (skips on Wayland).
- [x] 1.4 Dedicated acceptance scenario added: `tests/acceptance/egui/coexisting_runtimes.robot` — two `BareMetal` instances (distinct RF library instances via differing `auto_activate`, each a native runtime with its own `Arc<X11Connection>`) coexist on the same display: both resolve the window, each drives its own highlight, and one acts while the other observes. X11-only (skips on Wayland).

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
- [x] 4.4 `platform-windows` has a `WindowsPlatformFactory` + `create_windows_bundle` (DPI-awareness kept once-per-process); highlight controller made per-instance (owns `tx` + `JoinHandle`, joined on `Drop`; `static CTRL`/`global`/`shutdown_highlight` removed); all inventory registrations removed. Verified `just check-windows` + `just clippy-windows` clean (cross-compile). Windows *runtime* verification on a Windows host is §9.5 (follow-up).

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

- [x] 7.1 Native rebuilt via `just build-native-mock` (the `Runtime(config)` surface is available to Python).
- [x] 7.2 `packages/native/src/runtime.rs`: `Runtime(config=None)` parses the nested dict → `RuntimeConfig` (recursive `PyDict`→`ConfigMap`; `bool` checked before `int`; unsupported leaves/keys skipped with a debug log). `None`/empty ⇒ `RuntimeConfig::default()` (unchanged behavior). `.pyi` updated to `__init__(self, config: dict[str, Any] | None = ...)`. `new_with_mock()` unchanged.
- [x] 7.3 Test written as a **native pytest** (`packages/native/tests/test_runtime_config.py`, 5 cases) rather than an RF mock suite — the RF `use_mock` path bypasses the config dict and a config-driven real path can't run headless, so the native layer is where `Runtime(config)` + parsing live. Covers: mock backend selected, empty buckets no-op, unknown/foreign/non-dict keys + mixed leaf types tolerated, `new_with_mock` regression. Real value-overrides remain acceptance-scope (1.3/9.4).
- [x] 7.4 `src/PlatynUI/BareMetal/__init__.py`: `config: dict | None = None` kw-only arg forwarded via `_create_runtime()` → `Runtime(self._config)`, documented in the docstring (satisfies §8.6). **`use_mock` reconciliation deviates from D7:** it stays `new_with_mock()` rather than a `backend="mock"` config, because `Runtime(config)` runs provider `discover()` (real AT-SPI on Linux; the mock *provider* is not inventory-registered), so a mock-backend config would still load AT-SPI. Behaviorally identical; documented in code.

## 8. Documentation

- [x] 8.1 `dev-docs/architecture.md` — rewrote the registration model (two factory macros), the Linux mediator (two `PlatformFactory`s, no `RESOLVED`/wrappers), the init/shutdown lifecycle (per-runtime bundle, no lease), the desktop-node source (from the bundle), and test injection (mock via `config` backend).
- [x] 8.2 `dev-docs/platform-linux.md` — design decisions + selection example rewritten to the factory/per-runtime model; X11 utilities/highlight/shutdown now describe the owned `Arc<X11Connection>` and per-instance highlight thread (keymap/atoms noted as server-stable global caches).
- [x] 8.3 `dev-docs/python-bindings.md` — added a "Runtime Configuration" section: `Runtime(config=None)`, the `platform`/`providers` dict shape, the leaf-value mapping (bool-before-int), tolerant unknown keys, empty ⇒ default, construction-time immutability.
- [x] 8.4 `dev-docs/python-library-design.md` — added Rev. 49 entry documenting the construction-time session `config` (per-runtime platform sessions): the `platform`/`providers` buckets, the reserved `backend` selector, empty ⇒ current behavior, and the migration relevance (the high-level `PlatynUI` library will share the same `config` plumbing once it moves to `scope='SUITE'`). Kept as a linked migration note pointing to the authoritative docs (BareMetal docstring, `python-bindings.md`, `architecture.md`), not a duplicate.
- [x] 8.5 `dev-docs/testing-strategy.md` — added a normative "Also guards per-runtime isolation" note to the acceptance-lane section (why the lanes run several suites in one process); framed as intent, not a status count.
- [x] 8.6 `BareMetal` docstring — done in §7.4: `config=` documented with the `platform`/`providers` shape, reserved `backend`, portability, empty-default, and construction-time immutability, plus a `config` row in the args table.

## 9. Verification

- [x] 9.1 `just check` (fmt/clippy/ruff/mypy — 125 files) clean; `just check-windows` + `just clippy-windows` clean (cross-compile).
- [x] 9.2 `just test` (nextest) — **2018 passed, 0 failed** (incl. `RuntimeConfig` core tests + reworked runtime/mock tests).
- [x] 9.3 `just test-python` — **746 passed** (incl. the 5 native `Runtime(config)` tests).
- [x] 9.4 `just test-acceptance-x11` multi-suite (real AT-SPI/X11) — **26/26**, every suite green, no shutdown/init/window-manager errors. The regression guard for the original bug. (Dedicated config-bound-display / two-instance scenarios are 1.3/1.4, remaining.)
- [x] 9.5 Windows runtime verification on a real Windows desktop — **verified manually on a Windows host: the per-runtime platform-session build runs and all exercised functionality works.** (Development box is Linux; cross-compile was checked here, runtime confirmed on Windows by the maintainer.)
