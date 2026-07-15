## Why

PlatynUI cannot automate Java Swing/AWT applications on Windows: Swing implements no UIA provider, so a Swing window appears in the UIA tree as an empty shell. The planned Java Access Bridge (JAB) provider (follow-up change `add-jab-provider`) needs a controlled, growable Swing fixture application to develop and test against — none exists in the repo. A small FFI spike against the real JAB client DLL is also needed to validate the core technical assumptions (threading model, struct layouts, handle lifecycle) before the provider is designed in detail.

## What Changes

- New Java Swing test/fixture application at `apps/test-app-swing`, mirroring the `apps/test-app-qt` precedent: plain Java 8 sources compiled with `javac` via `just` recipes (no Maven/Gradle), excluded from the Cargo workspace, with a README.
- CLI conventions mirroring the existing Qt/egui test apps: `--title`, `--auto-close N`, `--dialogs N`, `--open-modal`.
- Every control carries an explicit `accessibleName` (JAB has no AutomationId equivalent, so the accessible name is the locator anchor) and the app provides click-observable state changes for interaction tests.
- Initial control coverage in two stages — stage 1: frame, menu bar, button, text field, label; stage 2: checkbox, radio group, combo box, slider, spinner, progress bar. The app is structured so later stages (tabs/table/tree, dialogs/popups, dynamic content) can be added incrementally by follow-up changes.
- Per-process JAB enablement at launch via `-Djavax.accessibility.assistive_technologies=com.sun.java.accessibility.AccessBridge` — no `jabswitch`, no persistent machine/user configuration.
- Platform-neutral app design: the same sources must run unmodified on Linux (later acceptance lane via `java-atk-wrapper` against the existing AT-SPI2 provider); only the launch flag is OS-specific.
- Throwaway JAB FFI spike in `crates/playground` (dev sandbox): load `WindowsAccessBridge-64.dll`, run the message pump on a dedicated thread, discover Java windows, and dump the test app's accessibility tree. Findings are recorded for `add-jab-provider`; the spike code itself is not a lasting deliverable.

## Capabilities

### New Capabilities

- `swing-test-app`: the Swing fixture application — build/run recipes, CLI surface, accessible-name discipline, staged control coverage, and per-process accessibility enablement.

### Modified Capabilities

<!-- none — no existing spec's requirements change; the spike is exploratory and produces findings, not spec-level behavior -->

## Impact

- **New code**: `apps/test-app-swing/` (Java 8 sources, README); spike code in `crates/playground` (temporary, replaced by the provider work).
- **Build system**: root `Cargo.toml` gains an `exclude` entry for `apps/test-app-swing` (same mechanism as `apps/test-app-qt`); `justfile` gains build/run recipes for the Swing app. No native rebuild of `packages/native` is involved.
- **Toolchain prerequisite**: a JDK 8+ on `PATH` (`javac`, `java`) for developers working on this area; the local reference environment is Eclipse Temurin 8 (64-bit), which ships the JAB client DLL and the official C headers used by the spike.
- **Platforms**: Windows is the target of the spike and the upcoming provider; the app itself is platform-neutral. No shipped provider/platform behavior changes in this change.
- **Tests/CI**: no acceptance lane yet (follows with the provider change); `just check` / `just test` are unaffected apart from the workspace exclude.
