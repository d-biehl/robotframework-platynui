## Why

Native accessibility leaves hard gaps for Java UI toolkits (see [`dev-docs/java-toolkits.md`](../../../dev-docs/java-toolkits.md)): Swing/AWT through JAB aliases every JTable cell to one shared renderer, JavaFX has no accessibility at all on Linux, and Linux Swing needs the fragile `java-atk-wrapper`. An **in-JVM agent** reads the toolkit's own in-process model and bypasses all of it — the approach every commercial Java UI-test tool takes.

Every toolkit adapter needs the same foundation before any of them can read a single node: an agent artifact that can be **injected into a running JVM**, a transport to talk to it, and a way to **ship and find** it. That foundation is toolkit-neutral, and it is what this change delivers. It is deliberately separated from the toolkit adapters (`provider-java-swing` and its `-javafx`/`-swt` follow-ups) so each is one coherent unit: this change ends with "the agent is in the JVM and answers", the adapters begin with "map what it answers onto PlatynUI nodes".

**Injection is the primary path, not a convenience**: Java applications are launched by scripts, installers or Web Start — the launch line is typically not PlatynUI's to change — and the Inspector's core use is looking into an application that is *already running*, where `-javaagent` is impossible by definition.

## What Changes

- **A new agent artifact**: one PlatynUI-owned JAR with `premain` (launch) and `agentmain` (attach) entry points and toolkit self-detection, built by a Gradle product project (`java/agent`), compiled `-target 8`. This change delivers the toolkit-neutral skeleton — element registry, threading discipline, RPC server; the per-toolkit tree readers land with the adapter changes.
- **A control/data plane split**: injection happens once (attach or `-javaagent`); the running traffic flows over the agent's own loopback NDJSON-RPC connection, discovered and authenticated through a per-user **handshake file**.
- **A native attach transport in Rust** — the JVM attach protocol spoken directly (Unix: signal + socket file; Windows: `CreateRemoteThread` → `JVM_EnqueueOperation`), so no JDK, no `jattach` and no bundled foreign binary is required on the test host.
- **A new crate `crates/java-agent`** (`platynui-java-agent`): attach transport, handshake discovery, and the wire-level RPC client — provider-neutral, so it needs neither `unify-java-provider` nor any toolkit adapter to be built and verified.
- **Delivery**: the JAR ships in its own pure-data wheel `platynui-provider-java`, exact-pinned via `[java]` extras — **opt-in-by-install**, which is also the consent for instrumentation. Discovery via the new `platynui.providers` entry-point group.
- **Injection policy**: attaching automatically is the intended default, since attach is the primary path; the flag that governs it (`auto_attach`) lands with the router in `provider-java-swing`, because deciding "this window's JVM has no agent" needs window detection. This change owns the mechanism and the availability gate (`providers.java.agent.enabled`, `providers.java.agent.jar`).

## Capabilities

### New Capabilities

- `java-agent`: the in-JVM agent artifact, its injection paths, its transport and lifecycle, and how it is delivered and discovered — everything a toolkit adapter needs before it can read a node.

### Modified Capabilities

- `java-app-classification`: gains an "agent present in this JVM?" signal, derived from the handshake file.

## Impact

- **New**: `java/agent` (Gradle product project; `just build-java-agent` — `just build-native` stays JDK-free), `crates/java-agent` (attach transport + RPC client), the Python package `platynui-provider-java` (`py3-none-any` wheel carrying the JAR + an `agent-path` CLI), the entry-point group `platynui.providers`.
- **Modified**: `java-app-classifier` (agent-presence signal); release/wheel recipes; docs.
- **No provider or tree behavior changes** — nothing in this change surfaces a UI node. `window_claims` is untouched.
- **Depends on**: nothing open (`java-app-classifier` is archived). **Unblocks**: `provider-java-swing` and, through it, the `-javafx`/`-swt` adapters.
- **JEP 451**: the attach path warns on JDK 21 and will be default-disallowed in a future release (opt-in `-XX:+EnableDynamicAgentLoading`, settable via `JAVA_TOOL_OPTIONS` without editing the command line) — documented, not blocking; `-javaagent` is the durable fallback. **Platform scope**: cross-platform.
