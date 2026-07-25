## Why

SWT renders through native platform widgets, so it is the one Java toolkit the native providers already serve — decently via UIA on Windows, weakly via AT-SPI on Linux (see [`dev-docs/java-toolkits.md`](../../../dev-docs/java-toolkits.md)). What native access cannot see is the **SWT widget model itself**: the `Widget.setData` keys real-world apps and tools use as test ids (the SWTBot convention), direct model data on `Table`/`Tree`, stable object identity across re-enumeration, and any structure a custom-drawn control keeps in-process instead of exposing natively. The agent infrastructure from `provider-java-swing` is toolkit-neutral; this change adds the **SWT adapter** — the instance-tree spine is the only in-process level SWT has (it implements no `javax.accessibility`), and it is exactly the level native access misses.

## What Changes

- **SWT adapter in the existing agent JAR**: widget-tree spine (`Display`→`Shell`→`Control`) read on the SWT UI thread (`Display.syncExec` under the agent's per-call deadline); enrichment from SWT's own `org.eclipse.swt.accessibility.Accessible` where apps provide it; `setData` entries surfaced as addressable `native:*` attributes.
- **Toolkit self-detection extended**: an active `Display` adds `"swt"` to the handshake file's `toolkits` list.
- **Claims across a native subtree**: unlike Swing/FX, every SWT `Control` is its own native window — the Java provider claims the *shell* and the native provider (UIA/AT-SPI) must abstain for the shell's **entire native window subtree**, not just the top-level. This extends the boolean `window_claims` semantics from "a window" to "a window and its native descendants".
- **Acceptance** against the `apps/test-app-swt` fixture (`add-swt-test-app`).

## Capabilities

### Modified Capabilities

- `java-provider`: the agent backend gains the SWT toolkit adapter, and claims gain the subtree-scoped abstention semantics (new requirements; the toolkit-neutral core, injection, and routing requirements from `provider-java-swing` are unchanged).

## Impact

- **Modified**: the agent JAR (SWT adapter classes — reflection against the app's SWT, SWT is not a dependency of the agent artifact); `crates/provider-java` (SWT role normalization); `platynui_core::platform::window_claims` (subtree-scoped abstention) and its native-provider consumers.
- **No new crates, no wire/protocol change** — the `toolkits` list absorbs the addition by design. No BREAKING changes; agent-less SWT apps stay with UIA/AT-SPI exactly as today.
- **Depends on**: `provider-java-swing` (all infrastructure) and `add-swt-test-app` (the fixture + catalog suite).
- **Non-goals**: the `SWT_AWT` bridge (mixed-toolkit trees — deferred proposal); Eclipse-workbench-specific semantics (views/editors/perspectives) beyond plain SWT widgets; a native window-handle fallback question does not arise (SWT exposes `Control.handle` directly on win32; per-platform equivalents are design work).
