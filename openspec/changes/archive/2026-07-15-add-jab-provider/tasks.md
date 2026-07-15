## 1. Prerequisites

- [x] 1.1 Confirm `add-swing-test-app` is implemented and fill the *Findings* section of this change's `design.md` from the spike (role table, rendezvous timing, DPI result, release observations); resolve the DPI open question for `Bounds`

## 2. Crate scaffold & wiring

- [x] 2.1 Create `crates/provider-jab` (package `platynui-provider-jab`, Windows-only) with descriptor/factory skeleton (`PROVIDER_ID "jab"`, technology "JAB", `event_capabilities: None`), config reading (`enabled`, `dll_path`, `call_timeout_ms`), and inventory registration under `cfg(target_os = "windows")`
- [x] 2.2 Wire the crate: `platynui_link_os_providers!` in `crates/link` (Windows arm), `justfile` `windows_rust_packages` list; verify `just check` passes on the empty skeleton
- [x] 2.3 *(added during implementation)* Consumer manifests: `platynui-provider-jab` as Windows target dependency of `crates/cli`, `packages/native`, `apps/inspector` (the link macro only references crates the caller declares)

## 3. JAB client core (private modules)

- [x] 3.1 `ffi.rs`: `#[repr(C)]` structs (`AccessibleContextInfo`, `AccessBridgeVersionInfo`, …) with compile-time size/offset assertions derived from the headers; function-pointer types for the lowercase cdecl exports; item-level `#[allow(unsafe_code)]` per workspace lint policy (unit-test the layout assertions)
- [x] 3.2 `dll.rs`: discovery (`providers.jab.dll_path` → `%JAVA_HOME%\jre\bin` → `%JAVA_HOME%\bin` → `PATH`) + `libloading` binding of all needed exports; unit-test discovery order with fake directory layouts *(export-name correction found live: the selection count export is `getAccessibleSelectionCountFromContext`, not the Java-API-style `getAccessibleSelectedChildrenCount…`)*
- [x] 3.3 `pump.rs`: dedicated thread — bind, `Windows_run()`, Win32 message pump, mpsc request servicing with per-call deadline, degraded-vmID tracking, release-queue draining, clean shutdown (no callbacks registered in MVP; free the library last); apply `ChangeWindowMessageFilter` when running elevated
- [x] 3.4 `handle.rs`: `JabObject` RAII wrapper (Drop → release queue), `isSameObject`-based equality helper (surfaced as the cheap `is_valid()` liveness probe)

## 4. Provider: discovery & nodes

- [x] 4.1 Top-level discovery: `EnumWindows` + `isJavaWindow` + `GetAccessibleContextFromHWND` with bounded pump-wait on first access; claimed-HWND bookkeeping; root streaming (windows, then `app:Application` per PID with process metadata)
- [x] 4.2 `node.rs`: lazy `JabNode` with cached `getAccessibleContextInfo`, `invalidate()` clearing the cache, lazy children via `getAccessibleChildFromContext`, parent references, RuntimeId scheme `jab://<vmID>/<hwnd>[/<index-path>]` (app view scoped as `jab://app/<pid>/…`, mirroring UIA)
- [x] 4.3 `map.rs`: role normalization from `role_en_US` (PascalCase fallback) — unit tests pinning the spike-harvested role table to the PlatynUI role vocabulary (AT-SPI2 column as reference where the vocabularies align); JList open question resolved: `label` children of a `list` with the `selectable` state are promoted to `item:ListItem` (verified live against the combo-box popup)
- [x] 4.4 Attributes: `Name`, `Role`, `Bounds` (self-calibrating per-window DPI transform, decision 13), `IsEnabled`/`IsVisible`/`IsFocused` from `states_en_US` parsing (unit-tested parser), `native:*` passthrough set, `Technology`
- [x] 4.5 Run the core contract testkit (`platynui_core::ui::contract::testkit`) against a live fixture-app node set (`tests/live_fixture.rs`, `#[ignore]`-gated, run by the acceptance recipe) — includes pattern-honesty both ways and 4× stable-walk checks

## 5. Patterns

- [x] 5.1 Focusable (`requestFocus` + `focused` state) and ActivationTarget (bounds center)
- [x] 5.2 TextContent (chunked `getAccessibleTextRange`) and TextEditable (new core `TextEditableAction` pattern backed by `setTextContents`, `editable` state, enforced 1023-UTF-16-unit write limit; live-verified round-trip)
- [x] 5.3 Toggleable (`ToggleState` from `checked`), StatefulValue (parsed `Value`/`MinValue`/`MaxValue`), Selectable/SelectionProvider (`IsSelected`, `SelectedItems`, `CanSelectMultiple` from `multiselectable`), Expandable (`IsExpanded`/`CanExpand`) — attribute contracts per architecture §6.3/§6.4
- [x] 5.4 Window capability patterns on top-level nodes delegating to the injected `WindowManager` (`set_window_manager` seam, `native:NativeWindowHandle`; activate + close live-verified)

## 6. Single appearance (claims)

- [x] 6.1 Add the window-claims seam to `platynui-core` (`platform::window_claims`: refcounted claim/release/query) with unit tests
- [x] 6.2 JAB provider syncs claims per discovery pass (claims newcomers, releases closed/died windows) and releases all on shutdown
- [x] 6.3 `provider-windows-uia`: skip claimed top-levels during root streaming behind `providers.windows-uia.honor_window_claims` (default true; key follows the provider-id config convention — the proposal's `providers.uia.*` spelling was corrected); unit-tested config gate

## 7. Diagnostics

- [x] 7.1 Warn-once-per-HWND "bridge not enabled" diagnostic (`SunAwt*` class + `isJavaWindow` false) naming both enablement paths; info-log DLL discovery result and `getVersionInfo` on connect; `no_configuration_mutation_code_paths_exist` test pins that no registry/file/process mutation paths exist in the crate

## 8. Acceptance lane (Windows, real profile)

- [x] 8.1 Create `tests/acceptance/swing` suite (smoke: discovery/single-appearance/roles/list-item promotion/late-poll; interaction: click-counter, text, toggle, radio, value; window: activate/move/close; dedup: kill-switch; hygiene) — 16 tests, all green. Interaction results read back on the same long-lived runtime (provider reads JAB state live per access; `Wait Until Keyword Succeeds` covers Swing's EDT latency)
- [x] 8.2 Extend the `test-acceptance-windows` recipe: build the Swing app (soft-fail without a JDK), run the JAB live Rust lane, hand over `PLATYNUI_TEST_APP_SWING_CLASSES`; suites self-skip (non-Windows / no java / unbuilt fixture)
- [x] 8.3 Robustness scenario: frozen-JVM test (`tests/live_fixture.rs::live_frozen_jvm_stays_contained` — debugger-freeze the fixture, assert bounded timeout + degraded fast-fail + a concurrent UIA query stays responsive + recovery on thaw); nextest `jab-live` test-group serializes it
- [x] 8.4 Repeated-walk handle-hygiene scenario (10× full walk, stable signature; Rust live lane also asserts 4× stable walks + healthy JVM afterwards)

## 9. Docs & verification

- [x] 9.1 `dev-docs/platform-windows.md`: new JAB provider section §2a (threading model, handle rules, live-read model, DPI calibration, claims, diagnostics, config keys); `dev-docs/architecture.md`: crate landscape + RuntimeId table row; `AGENTS.md` crate list; README platform-support table (new Java Swing/AWT row + provider bullet)
- [x] 9.2 `just check` green (fmt/clippy/ruff/mypy); `just test` green (2007 passed, 2 JAB live tests skipped by default); `just build-native` green; `just test-acceptance-windows --profile real run tests/acceptance/swing` green (JAB live Rust lane 2/2 + Swing Robot lane 16/16). **Call-timeout default kept at 2000 ms**: across all real runs no legitimate call approached it (Temurin 8 answers in low-ms); the deadline only ever fired in the deliberate frozen-JVM test, where 750 ms was comfortable — 2000 ms leaves generous headroom without making a real hang feel unresponsive
