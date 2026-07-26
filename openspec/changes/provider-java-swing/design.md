## Context

Where [`java-agent-core`](../java-agent-core/design.md) ends — the agent is in the JVM, reachable, bounded, delivered — this change begins: turning its answers into PlatynUI nodes, for **Swing/AWT**. It owns two things the foundation deliberately left out: the **mapping layer** (agent elements → `UiNode`) and the **Swing tree reader** inside the agent.

Structurally the agent lands as a **backend of the single Java provider** established by `unify-java-provider` (JAB is the other backend) — which is why this change needs no `window_claims` semantics change at all. The JavaFX and SWT adapters (`provider-java-javafx`, `provider-java-swt`) reuse the same client, mapping layer and routing; only their tree readers and role normalization differ.

Still **forward design**: the data model is decided below, and the remaining unknowns (coordinate conversion per JDK, window-handle internals, the exact attribute payload) are consolidated at the end.

## Goals / Non-Goals

**Goals:**

- Full-fidelity Swing trees (real identity, correct bounds, working table cells) as ordinary PlatynUI nodes — same XPath, Inspector, picker.
- Keyword-free: routing is automatic via the classifier + the Java provider's backend selection; the user only writes locators.
- A mapping layer the later toolkit adapters inherit rather than duplicate.

**Non-Goals:**

- Injection, transport, delivery, agent lifecycle — all `java-agent-core`.
- Replacing the JAB backend / native providers — they remain the zero/low-consent floor.
- **This change covers Swing/AWT only** (decided). The JavaFX-on-Linux gap stays open until the FX adapter ships. This holds scope tight against the one toolkit where the fidelity pain is proven (JTable) and lets the mapping layer harden before a second toolkit is added.
- **Mixed-toolkit trees** (`JFXPanel` hosting FX inside Swing, `SwingNode` hosting Swing inside FX, the `SWT_AWT` bridge) are deferred to a proposal of their own, written once at least two adapters exist. The single-toolkit-per-JVM assumption is **deliberately provisional, not an invariant** — the wire is shaped so lifting it later needs no format break (`toolkits` is a list).
- Programmatic text writes — see decision 1.

## Decisions (proposed) and Open Questions

1. **Data model: component-tree spine + accessibility enrichment + virtual accessible subtrees — full surface from the start (decided).** The wire carries everything both levels offer (not a minimal set): direct properties incl. `Component.getName()`, client properties, model data (`TableModel` bulk reads), the complete state set, and the accessible view — the exact field list is pinned against the fixture, but the scope decision is "all of it". The agent reads **both** levels the JVM offers, in a fixed relationship:

    - **Spine = the toolkit instance tree** (`Window`→`Container`→`JComponent`): complete, deterministic, carries geometry — and it is the only level that has `Component.getName()` (the classic automation id JAB could never see), direct model access (`TableModel` bulk reads instead of per-cell calls), and semantic actions.
    - **Enrichment = the accessible view per node** (`AccessibleContext` where present: accessibleName/role/description/states) — so locators written against JAB's `accessibleName` keep working unchanged, and apps that only annotated accessibility stay addressable.
    - **Virtual accessible subtrees where components end:** a `JTable`'s cells are not components but `AccessibleJTableCell` wrappers — *correct* in-process (name, `getCellRect` bounds, selection) — and custom-drawn components may expose structure only through accessibility; the agent grafts these as child nodes of the spine.

    Actions keep PlatynUI's philosophy: real input (pointer/keyboard) via correct bounds stays platform-level; the agent backs tree/attributes/bounds/hit-test plus selected patterns (Focusable via `requestFocus`), mirroring the JAB provider's pattern surface. In-JVM hit-testing is trivial (`SwingUtilities.getDeepestComponentAt`) and closes the table-picker gap JAB cannot. Per the `text-input-policy` capability the agent exposes **no** text write: TextEditable is a capability marker derived from the toolkit's editable state (plus `IsReadOnly`), and text is typed via synthesized keyboard input like everywhere else.

2. **Where PlatynUI adds value = the mapping layer.** The provider maps agent elements → normalized role/namespace, `native:*` attributes, patterns, and — crucially — **stable identity-based `RuntimeId`s**: the agent holds real Java object references, so unlike JAB's enumeration-index scheme, ids can be identity-stable. The identities themselves come from the agent's weak-ref registry (`java-agent-core` decision 2); this change decides how they become `RuntimeId`s.

    **`UiNode::is_valid` is load-bearing and the agent backend owns it** (the trait states that any provider handing out nodes with a real lifetime must implement it; the Robot Framework library keeps the element a scoped root resolved to for exactly as long as it answers `true`, so the default `true` would pin a dead root forever). The registry's liveness endpoint answers this cheaply: a node is valid while its id still resolves to a live object still attached to a showing window — cleared weak ref, detached component, or closed window ⇒ `false`, and the root re-resolves. **In the degraded/unreachable case the answer is `false`**, not an error and not an optimistic `true`: a JVM that died takes its nodes with it, and for one that is merely wedged, forcing a re-resolve is the recoverable direction. The per-node check is deliberately independent of the UI-generation counter, which stays a coarse invalidation *hint*.

3. **Coordinates: the AWT conversion (decided).** The wire contract is physical desktop pixels (`java-agent-core` decision 3); for Swing/AWT the conversion happens via `GraphicsConfiguration` transforms, where the toolkit's own scaling knowledge lives — Java 8 (already physical) and Java 9+ (per-monitor user-space) are normalized at the source, and the provider stays dumb. No provider-side calibration heuristic (the inverse of what JAB forced on us). To verify on a HiDPI-scaled monitor across JDK 8/17/21.

4. **Routing: internal backend selection in `provider-java` (decided).** With `unify-java-provider` in place there is exactly one Java claimant, so `window_claims` stays **boolean and untouched**. The classifier's "agent present?" signal (from `java-agent-core`) drives the router: a JVM window is served via the **agent backend when an agent is present**, else via the JAB backend. Mid-session robustness comes for free: when an agent appears in an already-running JVM, the router switches the serving backend on the next enumeration pass; the *claim* never changes, so there is no registry protocol and no consumer update anywhere. (This supersedes an earlier rank-based-claims design, which existed only because agent and JAB were two competing providers.)

5. **Native window handle: hybrid (decided).** For the WindowManager delegation of the window patterns, the agent first tries to read the native handle in-process (JDK internals — `sun.awt` peers on 8, their moved/jigsawed forms on 9+; what `--add-opens` that needs per version is open work); when that fails, the provider falls back to PID + geometry/title matching against the native window list. Exactness when possible, portability always.

## Risks / Trade-offs

- [JDK internals for the window handle shift across versions] → hybrid with a PID+geometry fallback; the exact matrix is verification work.
- [Full attribute surface is more wire traffic than a minimal set] → the wire is coarse-grained by design (a node's attributes arrive in one message, `TableModel` in bulk), which is what makes the full surface affordable.
- [Mapping layer shaped only around Swing] → the FX and SWT adapters restate their spine but reuse role normalization and identity handling; the second adapter is where this gets tested, deliberately after the first has hardened.

## Decisions summary

| # | Topic | Decision |
|---|---|---|
| 1 | Data model | spine (instance tree) + accessibility enrichment + virtual `AccessibleJTableCell` subtrees; **full field surface**; no text write |
| 2 | Mapping | provider maps to `UiNode`; identity-stable RuntimeIds from the agent registry; **`is_valid` owned** (`false` when degraded) |
| 3 | Coordinates | AWT conversion via `GraphicsConfiguration` onto the physical-pixel wire contract |
| 4 | Routing | **internal backend selection** (agent preferred over JAB per JVM window); `window_claims` untouched |
| 5 | Window handle | **hybrid**: in-JVM internals first, PID+geometry fallback |

## Verification items (approach decided; mechanics to confirm)

- **Coordinate conversion** (3): the in-JVM physical-pixel conversion on a HiDPI-scaled monitor across JDK 8 / 17 / 21.
- **Window-handle internals** (5): which `sun.awt` internals expose the native handle per JDK, and the `--add-opens` each needs on 9+ — before the PID+geometry fallback kicks in.
- **Element-model field list** (1): pin the exact attribute/pattern payload against the Swing fixture (the *scope* is "full", the field list is to enumerate).
