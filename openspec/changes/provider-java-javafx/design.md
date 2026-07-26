## Context

Second toolkit adapter on the foundation from [`java-agent-core`](../java-agent-core/design.md) (wire, handshake, injection, threading, registry ids, coordinate contract) and the mapping layer from [`provider-java-swing`](../provider-java-swing/design.md) (element mapping, `is_valid`, backend routing) — all carry over unchanged. This document only records what is *FX-specific*; like the swing change it is **forward design** with a spike up front. Mixed-toolkit trees remain out of scope (swing design, Non-Goals) — a JVM where both Swing and FX are active is detected (both entries in `toolkits`) but only its FX windows are served by this adapter; grafting across toolkits is the deferred proposal.

## Goals / Non-Goals

**Goals:**

- FX scene graphs as ordinary PlatynUI nodes — same XPath, Inspector, picker; on Linux, the first access path to FX at all.
- Zero protocol change: the adapter slots into the existing agent JAR, handshake `toolkits` list, and backend routing.

**Non-Goals:**

- Mixed-toolkit trees (`JFXPanel`, `SwingNode`) — deferred to their own proposal.
- Replacing the UIA provider for agent-less FX apps on Windows — it remains the zero-consent floor there.
- FX event push — the notification frame stays generation-counter-only, as in the swing change.

## Decisions (proposed) and Open Questions

1. **Spine = the scene graph** (`Stage`→`Scene`→`Node`, plus `PopupWindow`s as additional roots): complete, deterministic, carries geometry, and holds the FX automation id (`Node.getId()` — the analogue of Swing's `Component.getName()`) plus direct model access (e.g. `TableView.getItems()` bulk reads). FX implements no `javax.accessibility`; **enrichment** comes from FX's own accessibility API — `queryAccessibleAttribute` (name/role/description/state via `AccessibleAttribute`) and `AccessibleRole` mapped onto PlatynUI roles. Virtual subtrees where nodes end (e.g. `TableView` cells that are virtualized off-screen) are read from the *model*, mirroring the swing decision's virtual-subtree shape.

2. **Threading: FX Application Thread via `Platform.runLater`, deadline-bounded** — the exact EDT discipline from swing decision 7 (multi-client server, per-call agent-side deadline, abandon-on-timeout, generation counter) with the FX thread substituted. Startup detection: the adapter must notice when the FX runtime starts *after* the agent loaded (`premain` runs before `Application.launch`) — poll/hook the toolkit-initialized state rather than assuming launch order.

3. **Reflection across the module system.** FX 9+ lives on the module path (`javafx.*` modules); the agent sits in the unnamed module. The agent holds `Instrumentation`, so `redefineModule` can open what reflection needs — no `--add-opens` burden on the user's launch command. FX 8 (bundled in JDK 8) needs none of this. Which internals actually need opening (glass `Window` for native handles, virtualized-cell access) is spike work.

4. **Coordinates: physical desktop pixels in-JVM** (swing decision 2b), FX flavor: `Node.localToScreen` yields FX user space; conversion to physical via `Screen`/glass output scale. Spike verifies on a HiDPI monitor across JDK 8 / 17 / 21.

5. **Window handle: hybrid** (swing decision 8), FX flavor: glass internals (`com.sun.glass.ui.Window.getNativeHandle`/`getRawHandle`) first, PID+geometry fallback second.

6. **Claims**: no new mechanism — the Java provider (single claimant, boolean `window_claims`) simply also claims FX windows when the agent backend serves them. On Windows the UIA provider then skips the window exactly as for JAB-served ones (one representation); on Linux nothing else ever claims FX windows.

## Risks / Trade-offs

- [FX internals (glass, virtualization) shift across 8/11/17/21] → reflection guarded per version; spike pins the matrix; PID+geometry fallback for handles.
- [Headless/Monocle FX in CI] → acceptance runs against the real windowed fixture, as for Swing; Monocle is out of scope.
- [Linux lane has no native cross-check (FX invisible to AT-SPI)] → verify against the fixture's own observable state (the blueprint's last-action observables) instead of a second provider.

## Migration Plan

Additive adapter inside existing artifacts; enabled with the same `providers.java-agent.*` config. Rollback: disable the provider — Windows falls back to UIA, Linux FX returns to unreachable (status quo ante).

## Spike verification items

- FX-thread marshaling + deadline against the FX fixture, including agent-loaded-before-FX-starts.
- `redefineModule`-based access on FX 11+/17/21 vs. plain reflection on FX 8: what needs opening for glass handles and virtualized cells.
- Physical-pixel conversion on a HiDPI monitor (JDK 8 / 17 / 21).
- Windows claims handover: UIA tree vs. agent tree for the same Stage — single representation once the Java provider claims.
