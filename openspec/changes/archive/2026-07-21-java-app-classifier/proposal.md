## Why

PlatynUI recognizes Java windows only *inside* the JAB provider, and only on Windows: `provider-jab` enumerates top-level windows, calls `isJavaWindow`, and warns once about `SunAwt*` windows the bridge does not answer for ([crates/provider-jab/src/provider.rs](../../../crates/provider-jab/src/provider.rs)). There is no provider-independent, cross-platform way to answer the question that actually governs routing and diagnosability:

> Is this window backed by a JVM, and which toolkit (Swing/AWT, SWT, JavaFX) — and is it reachable through the platform's native accessibility at all?

That knowledge decides which provider should serve the window and whether accessibility is even possible (a Swing app without the bridge is invisible on Windows; **JavaFX has no native accessibility on Linux at all** — see [`dev-docs/java-toolkits.md`](../../../dev-docs/java-toolkits.md)). Today it is a log line at best: the Inspector and selectors cannot see "this is a Swing app with accessibility disabled", and the planned in-JVM agent provider (separate change `provider-java-agent`) has no shared place to plug its routing decision.

## What Changes

- A **shared Java-app classifier** — a core API (`platynui_core::platform::java`) with platform backends — that, for a top-level window, determines: **is-JVM** (a robust signal independent of the accessibility bridge), the **toolkit** where the platform allows, and whether the window is **visible in the native accessibility tree**.
- The classification is surfaced as **observable facts** (native attributes on the owning `app:`/window node) so the Inspector, selectors, and logs can distinguish "JVM: Swing, accessibility enabled" from "JVM: Swing, accessibility not reachable" or "JVM: JavaFX (no native a11y on this platform)".
- The Windows-only `SunAwtSuspect` warn-once is **generalized** into a cross-platform "JVM window absent from native accessibility" diagnostic that points at the enablement path (or, once it exists, the agent).
- **No change to window ownership**: JAB still claims Swing on Windows; UIA/AT-SPI/AX still handle the rest. **No injection, no consent, no JEP-451 surface** — this is pure detection and observability.

## Capabilities

### New Capabilities

- `java-app-classification`: provider-independent detection of JVM-backed top-level windows and their UI toolkit, plus the native-accessibility-reachability signal, surfaced as observable facts and an actionable diagnostic.

### Modified Capabilities

<!-- none — jab-provider keeps its own enumeration; de-duplicating it onto the shared classifier is a non-goal here (kept out to hold scope and risk down). -->

## Impact

- **New core API**: `platynui_core::platform::java` (classifier trait + result types: `is_jvm`, `toolkit: Option<JavaToolkit>`, `native_a11y_visible`). Platform-specific detection stays behind the platform bundle, mirroring `WindowManager`.
- **Windows backend** (`platform-windows`): top-level window class via `GetClassNameW` (`SunAwt*`/`SWT_Window*`/`Glass*`) + `jvm.dll` module presence via Toolhelp. This is the full, primary implementation.
- **Linux/macOS backends**: **out of scope here** — the API is designed cross-platform (the signal table in `java-toolkits.md`), backends land in follow-ups. Callers degrade to "unknown" gracefully.
- **Observability**: classification facts on the relevant node; the generalized diagnostic replaces the JAB-local `SunAwtSuspect` warning (JAB delegates the message, keeps its own enumeration).
- **Docs**: `dev-docs/java-toolkits.md` (facts already recorded) gains a pointer to the classifier API.
- **Depends on**: nothing. **Unblocks**: `provider-java-agent` (its routing consults this classifier). **Platform scope**: Windows. No BREAKING changes.
