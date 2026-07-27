## Why

Native accessibility leaves hard gaps for Java UI toolkits (see [`dev-docs/java-toolkits.md`](../../../dev-docs/java-toolkits.md)). For **Swing/AWT** — the toolkit this change targets — the gap is proven twice over:

- **On Windows**, JAB has low fidelity — the JDK bridge aliases every JTable cell to one shared renderer, so cells carry volatile names, no bounds, no stable identity, and the picker cannot reveal them (`fix-jab-hit-test-virtual-children`).
- **On Linux**, Swing/AWT is only reachable via the fragile, launch-modifying `java-atk-wrapper`, which PlatynUI has decided not to rely on. The agent closes that too — but making the provider run on Linux at all is `java-provider-linux`, because nothing there turns a JVM into a top-level node and that gap is not Swing-specific.

An **in-JVM agent** reads the toolkit's own in-process model directly and bypasses all of the above — full fidelity, real object identity. Commercial Java UI-test tools (QF-Test, Squish, Jubula) work exactly this way.

`java-agent-core` puts that agent into the JVM and gives it a transport; this change is where its answers become **PlatynUI nodes**. It adds the Rust client and mapping layer plus the **Swing/AWT adapter** — the toolkit whose fidelity gap (JTable) is proven — as a backend of the single Java provider from `unify-java-provider`. The JavaFX adapter (`provider-java-javafx` — the only path to JavaFX on Linux, which has no native accessibility at all) and the SWT adapter (`provider-java-swt`) are separate follow-up changes reusing the same client and mapping layer; mixed-toolkit trees (`JFXPanel`/`SwingNode`) are deferred to a proposal of their own once two adapters exist.

## What Changes

- A new **agent backend** in `crates/provider-java` (the single Java provider established by `unify-java-provider`; its crate name `platynui-provider-java` deliberately matches the Python delivery package): it consumes the wire-level client from `java-agent-core` and maps the agent's element model onto PlatynUI's `UiNode`/role/attribute/pattern/`RuntimeId` model — so Java apps appear in the same tree, XPath, Inspector, and picker as every other provider, with **no explicit "attach" keyword**.
- **The Swing/AWT adapter in the agent**: the instance-tree spine with per-node accessibility enrichment and virtual `AccessibleJTableCell` subtrees, AWT coordinate conversion to the physical-pixel wire contract, the Swing pattern surface, and the picker's hit-test endpoint. Not a highlight endpoint — drawing stays `HighlightProvider`'s (platform) job; what the Access Bridge lacks for a table cell is *bounds*, not a way to draw, so the bounds are the fix.
- **Automatic routing, no claims change**: the Java provider claims Java windows exactly as today (boolean `window_claims`); its router serves a window via the **agent backend when an agent is present** in that JVM, else via the JAB backend — JAB stays the zero-config Windows fallback. A mid-session agent appearance just switches the serving backend on the next enumeration pass.
- **Honest node validity**: nodes report liveness over the agent's registry, so a scoped root pinned to an element is re-resolved instead of pinning a dead one.

## Capabilities

### Modified Capabilities

- `java-provider`: gains the **agent backend** — in-JVM-agent-backed trees surfaced as PlatynUI nodes, with automatic (keyword-free) backend selection preferring the agent over JAB, and node validity answered rather than assumed. This change delivers the client, the mapping layer and the **Swing/AWT adapter**; the JavaFX and SWT adapters are added by `provider-java-javafx` / `provider-java-swt`.

## Impact

- **Modified crate** `crates/provider-java` (from `unify-java-provider`): gains the agent backend — element mapping on top of the wire client from `crates/java-agent`. **Modified agent** (`java/agent` from `java-agent-core`): gains the Swing/AWT tree reader.
- **Modified**: the Java provider's backend router consults the agent-presence signal (added by `java-agent-core`). `window_claims` is untouched (boolean, single Java claimant — per `unify-java-provider`).
- **Scope**: Swing/AWT only. The JavaFX and SWT adapters are separate follow-up changes; mixed-toolkit trees get a proposal of their own once two adapters exist. The wire keeps them cheap: the handshake file's `toolkits` field is a list from day one.
- **Config**: `providers.java.agent.auto_attach` (default **on**) — the routing policy lives here because deciding "this window's JVM has no agent, attach to it" needs window detection; the mechanism and its availability gate (`enabled`, `jar`) come with `java-agent-core`.
- **Depends on**: `java-agent-core` (the agent, its transport, delivery and the agent-presence signal) and `unify-java-provider` (the single Java provider + backend trait this backend slots into). **Unblocks**: `java-provider-linux` (which makes this backend available there), and the JavaFX/SWT adapters that reuse this client and mapping layer.
- **Platform scope**: verified on **Windows** — the fixture, the JAB backend to compare against, and the JTable gap that motivates the change are all there. The agent-side work is platform-neutral by construction (it is Java code in the JVM); only the native window handle is per-platform. No BREAKING changes; JAB and native providers stay.
