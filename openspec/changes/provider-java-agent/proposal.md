## Why

Native accessibility leaves hard gaps for Java UI toolkits (see [`dev-docs/java-toolkits.md`](../../../dev-docs/java-toolkits.md)):

- **Swing/AWT on Windows** through JAB has low fidelity — the JDK bridge aliases every JTable cell to one shared renderer, so cells carry volatile names, no bounds, no stable identity, and the picker cannot reveal them (`fix-jab-hit-test-virtual-children`).
- **JavaFX on Linux** has **no native accessibility at all** (Oracle: "no plan to make FX accessible on Linux") — it is unreachable by any current provider.
- Linux Swing/AWT is only reachable via the fragile, launch-modifying `java-atk-wrapper`, which PlatynUI has decided not to rely on.

An **in-JVM agent** reads the toolkit's own in-process model directly and bypasses all of the above — full fidelity, real object identity, and (once the JavaFX adapter ships) the *only* path for JavaFX-on-Linux. Commercial Java UI-test tools (QF-Test, Squish, Jubula) work exactly this way. This change adds a PlatynUI agent-backed provider. **v1 targets Swing/AWT** — the toolkit with the proven fidelity gap (JTable) — and hardens the transport/lifecycle before JavaFX and SWT adapters follow as later stages (JavaFX closing the Linux gap then).

## What Changes

- A new **`provider-java-agent`**: a UiTree provider that talks over RPC to an agent running inside the target JVM and maps the agent's element model onto PlatynUI's `UiNode`/role/attribute/pattern/`RuntimeId` model — so Java apps appear in the same tree, XPath, Inspector, and picker as every other provider, with **no explicit "attach" keyword** (detection + routing are automatic, via `java-app-classifier`).
- **Automatic routing**: the provider claims a Java window only when an agent is present in its JVM; `window_claims` becomes **rank-based** so the agent outranks JAB (agent > JAB > native-for-SWT/JavaFX). JAB stays as the zero-config Windows fallback for JVMs with no agent.
- **Two injection paths** for getting the agent in, loading the *same* agent: `-javaagent:` at launch (durable) and the **Attach API** into a running JVM (no launch change — good for Java Web Start), the latter under the JEP 451 sunset caveat. The attach protocol is **implemented natively in Rust** (no bundled binary, no JDK/`jattach` required on the test host).
- An **opt-in auto-attach policy** (`providers.java-agent.auto_attach`, default **off**): when on, detected agent-less Swing/JavaFX JVMs are attached automatically — still no keyword, consent given once at config level. Off ⇒ the agent is used only where the operator launched it with `-javaagent`.

## Capabilities

### New Capabilities

- `java-agent-provider`: an in-JVM-agent-backed UiTree provider for Java toolkits (Swing/AWT, JavaFX, SWT) that surfaces the target's in-process accessibility/scene model as PlatynUI nodes, cross-platform, with automatic (keyword-free) detection-driven routing and a claims priority over JAB.

### Modified Capabilities

- `java-app-classification`: the classifier gains an "agent present in this JVM?" signal feeding the routing decision.

## Impact

- **New crate** `crates/provider-java-agent` (platform-neutral core; the RPC client). **New agent artifact**: the in-JVM JAR, built and shipped as part of PlatynUI.
- **Modified**: `platynui_core::platform::window_claims` gains provider priority; `java-app-classifier` gains the agent-presence signal; the Java classifier routing consults it.
- **New platform crate** for the native attach transport (Rust; Unix socket-file+signal, Windows `CreateRemoteThread`→`JVM_EnqueueOperation`) + `-javaagent` docs. Discovery/auth via the agent's per-user **handshake file** (OS-chosen loopback port + random token, published PID-keyed in a `0700` directory) — one mechanism for port allocation, agent detection, and multi-user safety.
- **Modified**: `platynui_core::platform::window_claims` moves from a boolean "claimed by other" check to **rank-based** ownership; existing consumers (the UIA/JAB abstain checks) update accordingly.
- **v1 scope**: Swing/AWT only. JavaFX and SWT adapters (and mixed-toolkit trees) are explicitly deferred to later stages.
- **Config**: `providers.java-agent.enabled`, `providers.java-agent.auto_attach` (default off), attach/port settings.
- **Depends on**: `java-app-classifier` (routing plug point). **Platform scope**: cross-platform (one provider for Windows/Linux/macOS — the agent + RPC are platform-neutral; `jattach` covers the attach transport). **JEP 451**: the Attach injection path is on a deprecation trajectory; `-javaagent` is the durable path (documented, not blocking). No BREAKING changes; JAB and native providers stay.
