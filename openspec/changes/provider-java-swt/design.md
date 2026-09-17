## Context

Third toolkit adapter on the foundation from [`java-agent-core`](../java-agent-core/design.md) (wire, handshake, injection, threading, registry ids, coordinate contract) and the mapping layer from [`provider-java-swing`](../provider-java-swing/design.md); both carry over unchanged. **Forward design** with a spike up front. The SWT-specific substance is twofold: the widget model is the only in-process level (no `javax.accessibility`), and — unlike Swing/FX — every `Control` is a native window of its own, which forces a claims-semantics extension. The `SWT_AWT` bridge (Swing embedded in SWT, common in Eclipse RCP) is a mixed-toolkit case and stays deferred (swing design, Non-Goals).

## Goals / Non-Goals

**Goals:**

- SWT widget trees as ordinary PlatynUI nodes, with the model-level surface native access cannot see (`setData` test ids, `Table`/`Tree` model reads, stable identity).
- Claims that cleanly hand a shell **and its native descendants** from the native provider to the agent — one representation, no half-trees.

**Non-Goals:**

- Mixed-toolkit trees (`SWT_AWT`) — deferred to their own proposal.
- Replacing UIA/AT-SPI for agent-less SWT apps — they remain the zero-consent floor.
- Eclipse-workbench semantics above plain SWT widgets.

## Decisions (proposed) and Open Questions

1. **Spine = the widget tree** (`Display`→`Shell`→`Control`), read on the SWT UI thread via `Display.syncExec` under the agent-side deadline (swing decision 7, thread substituted). SWT is strictly one-Display-per-thread and almost always single-Display; the adapter handles the multi-Display case by enumerating `Display`s but treats it as exotic. Startup detection mirrors FX: the agent may load before the app creates its `Display`.

2. **Enrichment = SWT's `Accessible` API where present** (`org.eclipse.swt.accessibility`) — apps that annotated accessibility stay addressable by those names; `getData()`/`setData` string keys are surfaced as `native:*` attributes (the SWTBot-convention key prominently), giving locators the automation-id channel SWT never had natively. Items that are not `Control`s (`TableItem`, `TreeItem`, `MenuItem`, `ToolItem`) become virtual children of their parent control with model-backed name/bounds/selection — the SWT analogue of Swing's virtual accessible subtrees.

3. **Claims: subtree-scoped abstention (the SWT-specific extension).** In Swing/FX one native top-level window maps to one toolkit tree; in SWT *every* `Control` is a native window (win32 HWND, GTK widget window). The Java provider claims the **shell**, and boolean `window_claims` resolution extends to: a provider that skips a claimed window also skips that window's native descendants. Native providers already enumerate top-down, so the abstention check runs at the shell and prunes the subtree — no per-HWND bookkeeping. This is a semantics extension of the existing boolean claims, not a new mechanism.

4. **Window handles are the easy case**: `Control.handle` is a public field on win32 (per-platform equivalents via reflection on GTK/Cocoa) — the agent reports exact handles; the PID+geometry fallback from swing decision 8 should never trigger for SWT. This also gives the shell↔claim mapping exactness for decision 3. "Public field" is about SWT's API, not about reachability — see decision 6.

6. **Every SWT type is reached reflectively, through the application's loader.** The agent's classes are bootstrap-defined since [`java-agent-web-start`](../java-agent-web-start/design.md) decision 1 put the agent JAR on `Boot-Class-Path`, which is what makes it survive a target with a restrictive security policy. A bootstrap-defined class resolves only what the boot loader defines, and SWT is a third-party library on the application class path — so `Display`, `Shell`, `Control` and `org.eclipse.swt.accessibility` are all unreachable by direct reference or by a bare `Class.forName` from adapter code. The loader has to come from the application side (from a live SWT object, or via `Instrumentation.getAllLoadedClasses()`, which is already how the toolkit is detected without touching it). This applies to decision 1's `Display.syncExec` marshaling as much as to decision 4's handle read, and it is more consequential here than for Swing: SWT is not merely in another module, it is not in the JDK at all, so there is no version of this that works without the loader boundary. Concentrating that boundary in one place is a design constraint, not an implementation detail.

5. **Coordinates**: `Display.map`/`Control.toDisplay` yield display coordinates; per-platform DPI behavior differs (win32 per-monitor-DPI vs. GTK scale factors) — normalization to physical pixels in-JVM per swing decision 2b; the exact per-platform conversion is spike work.

## Risks / Trade-offs

- [Subtree abstention touches native-provider enumeration paths] → change is in the generic `window_claims` consumers, behind the same boolean claimed-check; Swing/FX behavior is unaffected (their subtrees have no native children).
- [SWT internals differ per window system (win32/GTK/Cocoa)] → reflection guarded per platform; the fixture lane initially covers win32 (the fixture is Windows-scoped), Linux/GTK follows the fixture's Linux variant.
- [No SWT type is directly referenceable from the bootstrap-defined agent (decision 6)] → the adapter loses compile-time checking against the SWT API and gains a loader boundary it must not leak past. Mitigation: one reflective handle onto the application's SWT loader, covered by fixture tests. A spike written against direct SWT references would prove nothing about the real adapter.
- [Value on Windows is incremental (UIA already decent)] → the payoff is the model surface (`setData` ids, custom-drawn controls) and Linux later; scenarios are pinned to exactly that, not to re-proving what UIA already does.

## Migration Plan

Additive adapter inside existing artifacts; same `providers.java-agent.*` config. Rollback: disable the provider — SWT apps return to UIA/AT-SPI everywhere.

## Spike verification items

- Subtree-scoped abstention: UIA stops descending at an agent-claimed shell; one representation for the shell's whole control tree.
- `setData` surfacing incl. the SWTBot key against the fixture; virtual items (`TableItem`/`TreeItem`/`MenuItem`) with correct name/bounds/selection.
- SWT-thread marshaling + deadline, incl. agent-loads-before-Display-exists.
- Physical-pixel conversion on win32 per-monitor DPI (GTK deferred with the Linux fixture).
