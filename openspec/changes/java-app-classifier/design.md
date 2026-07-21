## Context

Java-window detection lives entirely inside `provider-jab` today and is Windows-specific: `java_windows()` runs `EnumWindows`, tests `isJavaWindow`, and collects `SunAwtSuspect`s (class `SunAwt*` but the bridge answers no) for a warn-once diagnostic. Nothing else in the system can ask "is this a JVM window, and which toolkit?" — the `window_claims` registry only records *that* a provider claimed a HWND, not *why*.

The cross-platform facts (which toolkit is reachable through which native stack, and the hard JavaFX-on-Linux gap) are recorded in [`dev-docs/java-toolkits.md`](../../../dev-docs/java-toolkits.md). This change lifts the *detection* half of that map into a shared API; the *agent* half is `provider-java-agent`.

## Goals / Non-Goals

**Goals:**

- One provider-independent answer to "is-JVM? / toolkit? / visible in native a11y?" for a top-level window.
- Make that answer observable (Inspector, selectors, logs) instead of a buried warning.
- A cross-platform-shaped API even though only the Windows backend ships now.
- Be the plug point the agent provider's routing will consult.

**Non-Goals:**

- **No window-ownership change.** Who serves a window is unchanged; this only observes and diagnoses.
- **No claims priority.** That belongs to `provider-java-agent`, where a second claimant (the agent) actually competes with JAB. Building priority now would be unused machinery.
- **No de-duplication of JAB's own enumeration.** JAB keeps `java_windows()`; it only delegates the *diagnostic message*. (Refactoring JAB onto the classifier is a possible later cleanup, not this change.)
- **No injection, consent, or agent** — nothing that touches the target JVM.
- **No Linux/macOS backends** yet (API is ready for them).

## Decisions

1. **Classifier is a platform-bundle capability, not a free function.** `is-JVM` needs platform-specific process/window introspection (Toolhelp module scan, `/proc/maps`, attach artifacts) — the same reason `WindowManager` is injected. So the classifier is a trait obtained from the platform bundle, with the Windows backend implemented in `platform-windows`. Callers with no backend get `None`/"unknown" and degrade silently. *Alternative considered:* a `#[cfg]`-branched free function in core — rejected, it would pull OS APIs into `core` and break the platform-abstraction boundary.

2. **Two independent signals, deliberately separable.** `is-JVM` (robust: `jvm.dll` module / `libjvm.so` / attach artifacts) and `toolkit` (best-effort: window class on Windows) are distinct fields, because their reliability differs per platform (window class is precise on Windows, fuzzy elsewhere — see the doc). `native_a11y_visible` is the third, answered by "does the platform provider surface this window/PID" (on Windows: `isJavaWindow` for Swing; the window already resolving under UIA for SWT/JavaFX). Keeping them separate lets a consumer say "JVM yes, toolkit Swing, a11y no" — the actionable state.

3. **Observability via native attributes on the owning node.** The classification is exposed as `native:*` attributes (e.g. `native:JvmToolkit = "Swing"`, `native:JvmAccessibilityReachable = false`) on the `app:Application` (or the top-level window) node the relevant provider already emits. This reuses the attribute channel selectors and the Inspector already consume — no new node type, no new surface. *Alternative:* a synthetic diagnostic node — rejected as heavier and redundant with attributes.

4. **Generalize the diagnostic, keep JAB's enumeration.** The "JVM window absent from native a11y" warn-once moves to a shared helper (message names the enablement path, and later the agent). JAB calls it instead of building its own `SunAwtSuspect` string. JAB's window enumeration itself is untouched (non-goal above).

## Risks / Trade-offs

- [Stage-1 value without the agent is modest — mostly observability + a plug point] → accepted: diagnosability of "why is my Swing/JavaFX app empty" is real user value today, and decoupling detection from the JEP-451-laden agent keeps each change clean.
- [Window class is an imperfect toolkit signal (embedded content: JFXPanel, FXCanvas, SWT_AWT)] → the field is "best-effort host toolkit"; the authoritative per-subtree answer comes later from the agent. Documented.
- [A new platform-bundle trait widens the platform API] → small, additive, mirrors the existing `WindowManager` pattern.

## Migration Plan

Additive: new core trait + Windows backend + observability attributes + generalized diagnostic. No behavior change to ownership or existing attributes. Requires `just build-native` for the Inspector/Python to see the new attributes. Rollback: the attributes and diagnostic are inert; removing them changes nothing else.

## Open Questions

- Exact attribute names (`native:JvmToolkit`, `native:JvmAccessibilityReachable`?) — pin during implementation against the Inspector's display.
- Whether `native_a11y_visible` is worth computing eagerly on Windows for SWT/JavaFX (needs a UIA probe) or only for the Swing/JAB case where it is free (`isJavaWindow`). Lean: only where free; leave `None` otherwise.
