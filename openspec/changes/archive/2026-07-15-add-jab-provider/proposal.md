## Why

Java Swing/AWT applications are opaque to PlatynUI on Windows: Swing implements no UIA provider (OpenJDK tracks that as a "Future Project", JDK-5079680), so a Swing window appears in the UIA tree as an empty shell. The Java Access Bridge (JAB) is the JDK's sanctioned, out-of-process accessibility channel — the same one JAWS and NVDA use — and the only option compatible with the project constraint that nothing (no agent, no vendor code) may be loaded into the target JVM beyond the JDK's own bridge. On Linux, Swing already reaches PlatynUI through `java-atk-wrapper` and the existing AT-SPI2 provider; Windows is the missing piece.

## What Changes

- New Windows-only provider crate `crates/provider-jab` (package `platynui-provider-jab`), registered via the inventory mechanism like the other OS providers — a sibling of `provider-windows-uia`, structured after `provider-atspi`.
- Internal JAB client layer inside the crate (FFI over `WindowsAccessBridge-64.dll` via `libloading`, a dedicated message-pump thread owning all JAB calls, RAII handle management with `releaseJavaObject`, per-call timeouts, DLL discovery). Kept as private modules so a future UIA-proxy experiment (variant A, explicitly parked) could extract them into a shared crate.
- Java top-level windows (`EnumWindows` + `isJavaWindow`) appear as `control:Window` nodes with full lazy subtrees, flat view plus `app:Application` grouping, `Technology` = "JAB".
- Roles normalized to PascalCase from JAB's `role_en_US` against the PlatynUI role vocabulary, aligning with the AT-SPI2 mapping where the vocabularies coincide (soft goal: the same Swing app tends to answer the same selectors on Windows and Linux); originals preserved under `native:*`.
- Patterns: Element, Focusable (`requestFocus`), ActivationTarget, TextContent/TextEditable (AccessibleText / `setTextContents`), Toggleable, StatefulValue, Selectable/SelectionProvider, Expandable; window capability patterns delegate to the injected `WindowManager` via `native:NativeWindowHandle` (the AT-SPI2 blueprint).
- A JAB-specific `RuntimeId` scheme (JAB has no stable native IDs; `control:Id` is never emitted because JAB exposes no AutomationId equivalent).
- Enablement diagnostics instead of configuration mutation: a `SunAwt*` top-level window that fails `isJavaWindow` produces an actionable "bridge not enabled" diagnostic; the provider never writes `.accessibility.properties` or any target-side config.
- Duplicate-window suppression: `provider-windows-uia` learns to skip Java top-level windows claimed by the JAB provider (config-gated), so each Java window appears exactly once in the merged tree.
- Windows acceptance lane `tests/acceptance/swing` driving the fixture app from `add-swing-test-app` (which this change depends on, including its spike findings).
- Event support is explicitly deferred: the provider ships with `event_capabilities: None` (runtime polls); JAB callbacks are a follow-up change.

## Capabilities

### New Capabilities

- `jab-provider`: reading Java Swing/AWT UI trees on Windows through the Java Access Bridge — discovery, tree/attribute/pattern exposure, handle hygiene, robustness, diagnostics, and single-appearance of Java windows in the merged desktop tree.

### Modified Capabilities

<!-- none — no existing openspec/specs capability covers the UIA provider's window enumeration; the suppression behavior is specified as part of jab-provider -->

## Impact

- **New crate**: `crates/provider-jab` (Windows-only, `cfg(target_os = "windows")`), new dependency `libloading` (plus existing `windows` crate features for `EnumWindows` and the message pump).
- **Wiring**: `crates/link` (`platynui_link_os_providers!` gains the JAB provider on Windows); root `Cargo.toml` workspace member via glob; `justfile` `windows_rust_packages` list for the Windows wheel builds.
- **Modified crate**: `crates/provider-windows-uia` — config-gated skip of claimed Java top-levels during root streaming.
- **Native rebuild required**: `packages/native` links providers via `platynui-link`, so Python/Robot consumers need `just build-native` to see the provider.
- **Tests**: unit tests in the new crate (role map, state parsing, FFI struct-layout assertions); new Windows real-provider acceptance suite `tests/acceptance/swing` (needs a JDK on the runner; skips with a clear message otherwise); mock lane unaffected.
- **Docs**: `dev-docs/platform-windows.md` (new JAB section), `dev-docs/architecture.md` (crate landscape, RuntimeId table), `AGENTS.md` crate list, README platform-support table (Swing/JAB row, experimental).
- **Platform scope**: Windows only. Linux Swing support continues to ride `provider-atspi` + `java-atk-wrapper` (untouched); macOS unaffected; JavaFX apps already work via UIA and are out of scope.
