<!-- Builds on java-agent-core (agent in the JVM, transport, delivery) and
     unify-java-provider (the single Java provider with a backend trait).
     Scope: the Swing/AWT tree reader in the agent + the mapping layer and
     backend integration on the Rust side. -->

## 1. Agent: Swing/AWT adapter

- [ ] 1.1 Tree read: instance-tree spine + per-node accessibility enrichment + virtual `AccessibleJTableCell` subtrees (design 1); **full** attribute/model surface incl. `Component.getName()` and `TableModel` bulk reads
- [ ] 1.2 Pin the exact attribute/pattern payload against the Swing fixture and record it in design.md (the scope is "full"; the field list is to enumerate)
- [ ] 1.3 AWT coordinate conversion onto the physical-pixel wire contract via `GraphicsConfiguration` (design 3); verify on a HiDPI-scaled monitor across JDK 8/17/21
- [ ] 1.4 Actions/patterns: focus (`requestFocus`), point hit-test + highlight for the picker; **no** text write — TextEditable stays a capability marker (`text-input-policy`), text is typed
- [ ] 1.5 Native window handle: in-JVM internals first (`sun.awt` per JDK, incl. the `--add-opens` matrix on 9+), exposed for the provider's WindowManager delegation (design 5)

## 2. Provider: client and mapping

- [ ] 2.1 Agent backend in `crates/provider-java` on top of the wire client from `crates/java-agent` (`java-agent-core`)
- [ ] 2.2 Map agent elements → `UiNode`: normalized role/namespace, `native:*` attributes, patterns, identity-stable `RuntimeId`s from the agent registry (design 2)
- [ ] 2.3 Implement `UiNode::is_valid` over the registry's liveness endpoint, answering `false` when the agent is degraded or unreachable (design 2); degraded-agent handling mirroring JAB's `DegradedTracker`
- [ ] 2.4 Provider-side PID+geometry fallback for the native window handle when the in-JVM internals do not yield one (design 5)

## 3. Routing

- [ ] 3.1 Backend routing (design 4): the router prefers the agent backend when the agent-present signal holds, else JAB; a mid-session agent appearance switches the serving backend on the next enumeration pass; `window_claims` untouched
    - **Requires reshaping the backend surface.** `unify-java-provider` left the router claiming the *union* of what the backends serve and concatenating their nodes — correct while JAB is alone, since one backend cannot overlap itself, but it cannot express selection: `Enumeration { served_windows, nodes, unserved }` returns a flat node list with no mapping back to a window, so the router cannot drop JAB's nodes for a window the agent serves. Two backends over the same window would surface it twice. The shape was deliberately not guessed then, because it depends on answers this change has: whether the agent backend serves whole windows or subtrees, and how the two backends' `app:Application` grouping interacts — JAB's app nodes enumerate *their own* windows by PID filter, so a per-window node mapping alone does not settle it (an agent-served window could still reappear under JAB's app node). Settle both, then make 3.1 fall out of the shape rather than out of a filter bolted onto it.
- [ ] 3.2 Routing is automatic — no attach/connect keyword anywhere in the user-facing surface
- [ ] 3.3 `providers.java.agent.auto_attach` (**default on**, design 4): an enumerated Java window whose JVM carries no agent gets one injected via the `java-agent-core` transport; `false` limits the agent to `-javaagent`-launched targets

## 4. Acceptance & verification

- [ ] 4.1 Swing fixture: a `JTable` cell resolves with correct name/bounds/selection and a stable RuntimeId through the agent (the JAB gap closed)
- [ ] 4.2 A Swing application started by its own script, with no PlatynUI arguments, is served through the agent backend without being restarted
- [ ] 4.3 Routing: a JVM with the agent is served via the agent backend (single representation — one Java claim, one tree); a no-agent JVM is still served by the JAB backend
- [ ] 4.4 Node lifetime: a scoped root pinned to an agent-served element is re-resolved after its window closes and reopens (the agent-backend counterpart of the JAB proof in the Swing window lane), and a killed JVM does not leave a root reported valid
- [ ] 4.5 **Linux lane** — the second half of the Why, unverified today: the existing Swing suites are tagged `platform:windows` because JAB is Windows-only, so an agent-served run of the same fixture on X11 is the first proof that Swing on Linux is reachable *without* `java-atk-wrapper`. Needs its own suite/tagging (`platform:x11`) and the fixture launched there
- [ ] 4.6 Mapping-layer unit tests against recorded agent payloads (role normalization, attribute mapping, RuntimeId shape) so CI covers the mapping without a live JVM; the tree behavior itself stays real-provider-only
- [ ] 4.7 Robustness: an unresponsive agent stays bounded and does not block other providers; `just check`/`test`/`build-native` + the relevant acceptance lanes green
