## 1. Prerequisites

- [ ] 1.1 Confirm `add-swing-test-app` is implemented and fill the *Findings* section of this change's `design.md` from the spike (role table, rendezvous timing, DPI result, release observations); resolve the DPI open question for `Bounds`

## 2. Crate scaffold & wiring

- [ ] 2.1 Create `crates/provider-jab` (package `platynui-provider-jab`, Windows-only) with descriptor/factory skeleton (`PROVIDER_ID "jab"`, technology "JAB", `event_capabilities: None`), config reading (`enabled`, `dll_path`, `call_timeout_ms`), and inventory registration under `cfg(target_os = "windows")`
- [ ] 2.2 Wire the crate: `platynui_link_os_providers!` in `crates/link` (Windows arm), `justfile` `windows_rust_packages` list; verify `just check` passes on the empty skeleton

## 3. JAB client core (private modules)

- [ ] 3.1 `ffi.rs`: `#[repr(C)]` structs (`AccessibleContextInfo`, `AccessBridgeVersionInfo`, …) with compile-time size/offset assertions derived from the headers; function-pointer types for the lowercase cdecl exports; item-level `#[allow(unsafe_code)]` per workspace lint policy (unit-test the layout assertions)
- [ ] 3.2 `dll.rs`: discovery (`providers.jab.dll_path` → `%JAVA_HOME%\jre\bin` → `%JAVA_HOME%\bin` → `PATH`) + `libloading` binding of all needed exports; unit-test discovery order with fake directory layouts
- [ ] 3.3 `pump.rs`: dedicated thread — bind, `Windows_run()`, Win32 message pump, mpsc request servicing with per-call deadline, degraded-vmID tracking, release-queue draining, clean shutdown (no callbacks registered in MVP; free the library last); apply `ChangeWindowMessageFilter` when running elevated
- [ ] 3.4 `handle.rs`: `JabObject` RAII wrapper (Drop → release queue), `isSameObject`-based equality helper

## 4. Provider: discovery & nodes

- [ ] 4.1 Top-level discovery: `EnumWindows` + `isJavaWindow` + `GetAccessibleContextFromHWND` with bounded pump-wait on first access; claimed-HWND bookkeeping; root streaming (windows, then `app:Application` per PID with process metadata)
- [ ] 4.2 `node.rs`: lazy `JabNode` with cached `getAccessibleContextInfo`, `invalidate()` clearing the cache, lazy children via `getAccessibleChildFromContext`, parent references, RuntimeId scheme `jab://<vmID>/<hwnd>[/<index-path>]`
- [ ] 4.3 `map.rs`: role normalization from `role_en_US` (PascalCase fallback) — unit tests pinning the spike-harvested role table to the PlatynUI role vocabulary (AT-SPI2 column as reference where the vocabularies align); resolve the design's open question on JList `label` entries (pass-through vs. `item:ListItem` promotion)
- [ ] 4.4 Attributes: `Name`, `Role`, `Bounds` (with DPI correction if findings require), `IsEnabled`/`IsVisible`/`IsFocused` from `states_en_US` parsing (unit-test the parser), `native:*` passthrough set, `Technology`
- [ ] 4.5 Run the core contract testkit (`platynui_core::ui::contract::testkit`) against a live fixture-app node set to catch attribute/pattern-list violations early

## 5. Patterns

- [ ] 5.1 Focusable (`requestFocus` + `focused` state) and ActivationTarget (bounds center)
- [ ] 5.2 TextContent (chunked `getAccessibleTextRange`) and TextEditable (`setTextContents`, `editable` state, documented 1023-char write limit)
- [ ] 5.3 Toggleable (`checked`), StatefulValue (parsed value/min/max), Selectable/SelectionProvider (`AccessibleSelection`), Expandable (`expanded`/`expandable`)
- [ ] 5.4 Window capability patterns on top-level nodes delegating to the injected `WindowManager` (`set_window_manager` seam, `native:NativeWindowHandle`)

## 6. Single appearance (claims)

- [ ] 6.1 Add the window-claims seam to `platynui-core` (register/unregister/query claimed HWNDs) with unit tests
- [ ] 6.2 JAB provider registers claims on successful window attach and releases them on shutdown/JVM death
- [ ] 6.3 `provider-windows-uia`: skip claimed top-levels during root streaming behind `providers.uia.honor_window_claims` (default true); unit-test the config gate

## 7. Diagnostics

- [ ] 7.1 Warn-once-per-HWND "bridge not enabled" diagnostic (`SunAwt*` class + `isJavaWindow` false) naming both enablement paths; info-log DLL discovery result and `getVersionInfo` on connect; assert no config mutation code paths exist

## 8. Acceptance lane (Windows, real profile)

- [ ] 8.1 Create `tests/acceptance/swing` suite skeleton first (window discovery, tree walk, click → `clicks-1`, text round-trip, toggle, window activate/move/close, no-duplicates check, XPath end-to-end scenarios from the spec) — expected red until the provider lands
- [ ] 8.2 Extend the `test-acceptance-windows` recipe: build the Swing app, hand over `PLATYNUI_TEST_APP_SWING_*` env vars (JDK check with clear skip message when `javac`/`java` missing)
- [ ] 8.3 Robustness scenario: frozen-JVM test (suspend fixture EDT, assert runtime and UIA queries stay responsive, JAB query errors within deadline) — mark as manual/optional in CI if flaky
- [ ] 8.4 Repeated-walk handle-hygiene scenario (10× full walk, stable results)

## 9. Docs & verification

- [ ] 9.1 `dev-docs/platform-windows.md`: new JAB provider section (threading model, handle rules, claims, diagnostics, config keys); `dev-docs/architecture.md`: crate landscape + RuntimeId table row; `AGENTS.md` crate list; README platform-support table (Swing/JAB, experimental)
- [ ] 9.2 `just check`, `just test`, `just build-native`, then full Windows acceptance run (`just test-acceptance-windows`) green; record the JAB call-timeout default decision from real runs
