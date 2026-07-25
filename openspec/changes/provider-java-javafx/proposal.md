## Why

JavaFX is the one Java toolkit with a *total* platform gap: on Linux it has **no native accessibility at all** (Oracle: "no plan to make FX accessible on Linux") — no current provider can reach an FX app there by any means (see [`dev-docs/java-toolkits.md`](../../../dev-docs/java-toolkits.md)). On Windows, FX implements UIA natively and is served by the UIA provider, but only at the fidelity FX chooses to expose — the scene graph's `Node` ids (`Node.getId()`, the CSS/automation id), model-level data, and identity-stable references stay invisible. The agent infrastructure built by `provider-java-swing` (agent JAR, loopback NDJSON-RPC + handshake file, native Rust attach, rank-based `window_claims`) is toolkit-neutral by design; this change adds the **JavaFX adapter** — for Linux, the only path there is.

## What Changes

- **JavaFX adapter in the existing agent JAR**: scene-graph spine (`Stage`→`Scene`→`Node`, plus `PopupWindow`s) read on the **FX Application Thread** under the same per-call deadline discipline as the Swing EDT. FX implements no `javax.accessibility`, so enrichment comes from FX's own accessibility surface (`queryAccessibleAttribute` / `AccessibleRole`) grafted onto the spine — same spine+enrichment shape as Swing, different source.
- **Toolkit self-detection extended**: a started FX runtime adds `"javafx"` to the handshake file's `toolkits` list; the agent-side registry, coordinate conversion (physical pixels in-JVM), actions/patterns (focus, text edit, point hit-test + highlight for the picker), and window-handle resolution (glass internals first, PID+geometry fallback) all follow the swing change's decisions.
- **Claims**: the Java provider (single claimant, boolean `window_claims` — per `unify-java-provider`) now also claims FX windows when the agent backend can serve them — on Windows the UIA provider then skips the window as it does for JAB-served ones (single representation); on Linux it is the first claim ever made for an FX window.
- **Acceptance** against the `apps/test-app-javafx` fixture (`add-javafx-test-app`), including the Linux lane where FX is otherwise invisible.

## Capabilities

### Modified Capabilities

- `java-provider`: the agent backend gains the JavaFX toolkit adapter (new requirement; the toolkit-neutral core, injection, and routing requirements from `provider-java-swing` are unchanged).

## Impact

- **Modified**: the agent JAR (FX adapter classes — reflection against the app's FX runtime, FX is not a dependency of the agent artifact); `crates/provider-java` (FX role normalization in the mapping layer; the router's FX window claim).
- **No new crates, no wire/protocol change** — the handshake `toolkits` list and the reserved notification frame absorb the addition by design. No BREAKING changes; the UIA provider keeps serving agent-less FX apps on Windows.
- **Depends on**: `provider-java-swing` (all agent infrastructure: agent artifact, transport, attach, backend routing, config; transitively `unify-java-provider`) and `add-javafx-test-app` (the fixture + catalog suite).
- **Non-goals**: mixed-toolkit trees (`JFXPanel` hosting FX inside Swing, `SwingNode` hosting Swing inside FX) stay deferred to a proposal of their own, per the swing change's design; FX 8's bundled runtime vs. FX 11+ modules is a compatibility matter inside the adapter, not a packaging change.
