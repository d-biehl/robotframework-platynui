## Context

The follow-up change `add-jab-provider` will add a Windows provider that reads Java Swing/AWT accessibility trees through the Java Access Bridge (JAB). Two prerequisites are missing today:

1. A controlled Swing fixture application. The repo has fixture apps for the AccessKit tier (`apps/test-app-egui`) and the native-widget tier (`apps/test-app-qt`, Python/PySide6, excluded from the Cargo workspace — root `Cargo.toml:3-4`), but nothing Java.
2. Validated ground truth for the JAB client API from Rust: threading/message-pump model, struct layouts, handle lifecycle, and the real role vocabulary Swing emits. These determine the provider design and are cheap to verify with a spike, expensive to get wrong in a provider.

Constraints from prior exploration (research verified against Oracle docs, OpenJDK sources, and prior-art clients — NVDA, Google access-bridge-explorer, Robocorp):

- The automation client must stay **out-of-process**: no agents or vendor code loaded into the target JVM (customer security requirement). JAB is the sanctioned path — enabling it activates only JDK-own code in the target process.
- `Windows_run()` creates a hidden window on the calling thread; discovery and callbacks arrive as window messages there. **All JAB calls belong on one dedicated thread that runs a Win32 message pump.**
- Every `AccessibleContext` handle returned by the API must be released via `releaseJavaObject`, or the **target JVM** leaks.
- The local reference JDK is Eclipse Temurin 8 (64-bit) at `C:\Program Files\Eclipse Adoptium\jdk-8.0.492.9-hotspot`, which ships `WindowsAccessBridge-64.dll` (`jre\bin`) and the official C headers (`include\win32\bridge\AccessBridgeCalls.h`, `AccessBridgePackages.h`, `AccessBridgeCallbacks.h`, plus `AccessBridgeCalls.c` as the reference loader).

## Goals / Non-Goals

**Goals:**

- A Swing fixture app that mirrors the conventions of the existing test apps (CLI flags, observable interactions, stable accessible names) and can grow stage by stage.
- Zero persistent machine configuration: JAB enabled per process at launch.
- A throwaway Rust spike that de-risks the JAB provider: proves the FFI surface, the pump-thread model, and harvests the role vocabulary of every stage-1/2 control.
- Platform-neutral Java sources so the same app can later serve a Linux acceptance lane via `java-atk-wrapper` against `provider-atspi`.

**Non-Goals:**

- The JAB provider itself, RuntimeId design, UIA-window deduplication (all in `add-jab-provider`).
- Acceptance lanes (Windows or Linux) — they need the provider first.
- Later control stages (tabs/table/tree, dialogs/popups, dynamic content) — the app is structured for them, follow-up changes add them.
- Bundling or redistributing `WindowsAccessBridge-64.dll` (GPLv2+CPE licensing; discovery from an installed JDK is sufficient here).
- An in-process Java agent (QF-Test style) — explicitly rejected: it is exactly the instrumentation the security constraint forbids.

## Decisions

1. **Plain `javac` via `just` recipes, no Maven/Gradle.** The app is a fixture, not a product; a build system adds nothing but a toolchain dependency. `just build-test-app-swing` compiles `src/**/*.java` with `-encoding UTF-8 -source 8 -target 8` into `build/classes`; `just run-test-app-swing` runs it with the enablement flag. Mirrors how `test-app-qt` leans on the existing `uv` environment instead of its own tooling. *Alternative considered:* Gradle wrapper — rejected per explicit requirement and because it would download toolchains in CI.

2. **Location `apps/test-app-swing`, excluded from the Cargo workspace.** Follows the `apps/test-app-qt` precedent exactly (root `Cargo.toml` `exclude`); `apps/*` glob members without a `Cargo.toml` would otherwise break the workspace.

3. **Java 8 source/target.** Matches the installed reference JDK and the oldest runtime realistically found in enterprise Swing estates; everything the app needs (Swing, `javax.accessibility`) is ancient API. Newer JDKs can still compile and run it.

4. **Per-process enablement via `-Djavax.accessibility.assistive_technologies=com.sun.java.accessibility.AccessBridge`.** Verified: JDK 8 `Toolkit` consults the system property before `%USERPROFILE%\.accessibility.properties`, and JDK 9+ resolves the same class name through `AccessibilityProvider` for backward compatibility. This keeps CI and dev machines free of persistent accessibility config and matches the "diagnose, don't mutate" stance planned for the provider. *Alternative:* `jabswitch -enable` — rejected as default (per-user persistent state), documented in the README as the manual fallback.

5. **`accessibleName` on every control; component names are not enough.** JAB exposes only `AccessibleContext` data; `java.awt.Component#setName` has no field in any JAB struct and is invisible out-of-process. Without an AutomationId equivalent, the accessible name is the locator anchor — the app models the discipline real target apps need. Click feedback follows the `test-app-qt` pattern: clicking the stage-1 button updates an observable label (`click-count` in text and accessible name), giving tests a name-based observable.

6. **CLI surface mirrors the Qt/egui apps** (`--title`, `--auto-close N`, `--dialogs N`, `--open-modal`): the acceptance-lane launchers and docs already speak this dialect; `--auto-close` keeps CI hangs impossible. Argument parsing is hand-rolled (a dozen lines) — no libraries.

7. **App structure: one `Main` class plus one panel class per stage** (package `platynui.testapp`). Stages live side by side in a vertical layout (stage 2 in its own titled panel), so adding stage N later is additive and does not reshuffle existing accessible names — selector stability across growth is a design property, not luck.

8. **Spike lives in `crates/playground`** (the dev sandbox and default workspace member) as a `jab_spike` binary. It loads `WindowsAccessBridge-64.dll` with `libloading` (discovery order: `PLATYNUI_JAB_DLL` env var → `%JAVA_HOME%\jre\bin` / `%JAVA_HOME%\bin` → `PATH`), binds the lowercase cdecl exports (`Windows_run`, `isJavaWindow`, `getAccessibleContextFromHWND`, `getAccessibleContextInfo`, `getAccessibleChildFromContext`, `releaseJavaObject`, `getVersionInfo`, …) manually — replicating `AccessBridgeCalls.c` rather than compiling it. One dedicated thread initializes, calls `Windows_run()`, and pumps messages; tree walking happens on that thread; results print to stdout. FFI structs are `#[repr(C)]` with `i64` handles (`JOBJECT64` = `jlong` in the `-64` API), fixed UTF-16 arrays, no packing pragmas (verified in the headers). `#[allow(unsafe_code)]` is applied at item level, matching the precedent in the Wayland input crates (workspace denies `unsafe_code` globally).

9. **Spike validation checklist is the deliverable, not the code.** It must answer, against the running fixture app: (a) rendezvous timing — how long after `Windows_run` until `isJavaWindow` sees the frame (retry loop, not sleep); (b) struct layout correctness — name/role/states/bounds round-trip plausibly; (c) the exact `role_en_US` strings for every stage-1/2 control (feeds the provider role map); (d) whether JAB-reported bounds are desktop-pixel-correct under Per-Monitor-V2 DPI when the Java 8 app itself is not per-monitor aware (Windows may DPI-virtualize the app — a known coordinate risk for the provider); (e) handle-release discipline works (walk twice, no growth in JVM heap of live references — observed via repeated walks staying consistent). Findings are written into `add-jab-provider`'s `design.md`.

## Risks / Trade-offs

- [JAB rendezvous is asynchronous — right after init no JVM is visible] → spike and later provider use a bounded retry-with-pump loop instead of fixed sleeps; the README documents that apps must be started *after* their JVM has the bridge enabled (the flag handles this).
- [Java 8 is not per-monitor DPI aware; JAB coordinates may be virtualized while PlatynUI runs Per-Monitor-V2] → explicit spike checklist item (d); if coordinates are off on scaled monitors, the provider design must compensate (scale via window DPI) — better to learn this now than in the provider.
- [Silent truncation: JAB strings live in fixed 256/1024-wchar buffers] → fixture app keeps names short; spike notes truncation behavior for the provider's attribute layer.
- [A stage-2 control could render with an unexpected accessible role across JDK vendors] → the spike records `role` and `role_en_US` verbatim; the provider maps from `role_en_US` only.
- [Spike code rots in the playground] → it is declared throwaway in the proposal; the durable artifacts are the findings in `add-jab-provider`'s design and the fixture app itself.
- [Developers without a JDK cannot build the app] → recipes fail fast with a clear message when `javac` is missing; nothing else in `just check`/`just test` depends on Java.

## Migration Plan

Purely additive: new app directory, new `just` recipes, one `exclude` line in the root `Cargo.toml`, spike binary in the playground. No native rebuild (`packages/native` untouched), no shipped behavior changes, no data. Rollback = delete the directory, recipes, and exclude line.

## Open Questions

- None blocking. (Whether the spike's DPI findings force a provider-side coordinate correction is deliberately deferred to `add-jab-provider`.)
