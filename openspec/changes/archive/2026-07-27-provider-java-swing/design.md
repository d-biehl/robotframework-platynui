## Context

Where [`java-agent-core`](../java-agent-core/design.md) ends — the agent is in the JVM, reachable, bounded, delivered — this change begins: turning its answers into PlatynUI nodes, for **Swing/AWT**. It owns two things the foundation deliberately left out: the **mapping layer** (agent elements → `UiNode`) and the **Swing tree reader** inside the agent.

Structurally the agent lands as a **backend of the single Java provider** established by `unify-java-provider` (JAB is the other backend) — which is why this change needs no `window_claims` semantics change at all. The JavaFX and SWT adapters (`provider-java-javafx`, `provider-java-swt`) reuse the same client, mapping layer and routing; only their tree readers and role normalization differ.

Still **forward design**: the data model is decided below, and the remaining unknowns (coordinate conversion per JDK, window-handle internals, the exact attribute payload) are consolidated at the end.

## Goals / Non-Goals

**Goals:**

- Full-fidelity Swing trees (real identity, correct bounds, working table cells) as ordinary PlatynUI nodes — same XPath, Inspector, picker.
- Keyword-free: routing is automatic via the agent's own handshake rendezvous + the Java provider's backend selection; the user only writes locators.
- A mapping layer the later toolkit adapters inherit rather than duplicate.

**Non-Goals:**

- Injection, transport, delivery, agent lifecycle — all `java-agent-core`.
- Replacing the JAB backend / native providers — they remain the zero/low-consent floor.
- **This change covers Swing/AWT only** (decided). The JavaFX-on-Linux gap stays open until the FX adapter ships. This holds scope tight against the one toolkit where the fidelity pain is proven (JTable) and lets the mapping layer harden before a second toolkit is added.
- **Mixed-toolkit trees** (`JFXPanel` hosting FX inside Swing, `SwingNode` hosting Swing inside FX, the `SWT_AWT` bridge) are deferred to a proposal of their own, written once at least two adapters exist. The single-toolkit-per-JVM assumption is **deliberately provisional, not an invariant** — the wire is shaped so lifting it later needs no format break (`toolkits` is a list).
- **Linux** — `java-provider-linux`. The provider is `cfg(windows)` in its entirety today (JAB was its only backend), and nothing on Linux turns a JVM into a top-level node: AT-SPI enumerates the accessibility registry rather than windows, so a Swing process that never registered is invisible to it. That bring-up is not Swing-specific — JavaFX needs it more urgently — so it is its own change rather than a second half of this one. The agent-side Swing reader here is platform-neutral by construction; only the native window handle (decision 5) is not.
- Programmatic text writes — see decision 1.

## Decisions (proposed) and Open Questions

1. **Data model: component-tree spine + accessibility enrichment + virtual accessible subtrees — full surface from the start (decided).** The wire carries everything both levels offer (not a minimal set): direct properties incl. `Component.getName()`, client properties, model data (`TableModel` bulk reads), the complete state set, and the accessible view — the exact field list is pinned against the fixture, but the scope decision is "all of it". The agent reads **both** levels the JVM offers, in a fixed relationship:

    - **Spine = the toolkit instance tree** (`Window`→`Container`→`JComponent`): complete, deterministic, carries geometry — and it is the only level that has `Component.getName()` (the classic automation id JAB could never see), direct model access (`TableModel` bulk reads instead of per-cell calls), and semantic actions.
    - **Enrichment = the accessible view per node** (`AccessibleContext` where present: accessibleName/role/description/states) — so locators written against JAB's `accessibleName` keep working unchanged, and apps that only annotated accessibility stay addressable.
    - **Virtual accessible subtrees where components end:** a `JTable`'s cells are not components but `AccessibleJTableCell` wrappers — *correct* in-process (name, `getCellRect` bounds, selection) — and custom-drawn components may expose structure only through accessibility; the agent grafts these as child nodes of the spine.

    Actions keep PlatynUI's philosophy: real input (pointer/keyboard) via correct bounds stays platform-level; the agent backs tree/attributes/bounds/hit-test plus selected patterns (Focusable via `requestFocus`), mirroring the JAB provider's pattern surface. In-JVM hit-testing is trivial (`SwingUtilities.getDeepestComponentAt`) and closes the table-picker gap JAB cannot. **Highlighting is not the agent's** and stays with the platform's `HighlightProvider`: JAB cannot highlight a table cell because it has no *bounds* for one, not because it cannot draw — so correct bounds fix the picker for every consumer at once, while an agent painting into the target would change the application it observes and could leave it visibly changed if a run died mid-pick. Per the `text-input-policy` capability the agent exposes **no** text write: TextEditable is a capability marker derived from the toolkit's editable state (plus `IsReadOnly`), and text is typed via synthesized keyboard input like everywhere else.

2. **Where PlatynUI adds value = the mapping layer.** The provider maps agent elements → normalized role/namespace, `native:*` attributes, patterns, and — crucially — **stable identity-based `RuntimeId`s**: the agent holds real Java object references, so unlike JAB's enumeration-index scheme, ids can be identity-stable. The identities themselves come from the agent's weak-ref registry (`java-agent-core` decision 2); this change decides how they become `RuntimeId`s.

    **`UiNode::is_valid` is load-bearing and the agent backend owns it** (the trait states that any provider handing out nodes with a real lifetime must implement it; the Robot Framework library keeps the element a scoped root resolved to for exactly as long as it answers `true`, so the default `true` would pin a dead root forever). The registry's liveness endpoint answers this cheaply: a node is valid while its id still resolves to a live object still attached to a showing window — cleared weak ref, detached component, or closed window ⇒ `false`, and the root re-resolves. **In the degraded/unreachable case the answer is `false`**, not an error and not an optimistic `true`: a JVM that died takes its nodes with it, and for one that is merely wedged, forcing a re-resolve is the recoverable direction. The per-node check is deliberately independent of the UI-generation counter, which stays a coarse invalidation *hint*.

3. **Coordinates: the AWT conversion (decided).** The wire contract is physical desktop pixels (`java-agent-core` decision 3); for Swing/AWT the conversion happens via `GraphicsConfiguration` transforms, where the toolkit's own scaling knowledge lives — Java 8 (already physical) and Java 9+ (per-monitor user-space) are normalized at the source, and the provider stays dumb. No provider-side calibration heuristic (the inverse of what JAB forced on us). To verify on a HiDPI-scaled monitor across JDK 8/17/21.

4. **Routing: internal backend selection in `provider-java` (decided).** With `unify-java-provider` in place there is exactly one Java claimant, so `window_claims` stays **boolean and untouched**. The "agent present?" signal drives the router — taken from the agent's own handshake file (`handshake::agent_present`) rather than from the platform's Java classifier, which is what keeps the criterion available on every platform: a JVM window is served via the **agent backend when an agent is present**, else via the JAB backend. Mid-session robustness comes for free: when an agent appears in an already-running JVM, the router switches the serving backend on the next enumeration pass; the *claim* never changes, so there is no registry protocol and no consumer update anywhere. (This supersedes an earlier rank-based-claims design, which existed only because agent and JAB were two competing providers.)

5. **Native window handle: hybrid (decided).** For the WindowManager delegation of the window patterns, the agent first tries to read the native handle in-process (JDK internals — `sun.awt` peers on 8, their moved/jigsawed forms on 9+; what `--add-opens` that needs per version is open work); when that fails, the provider falls back to PID + geometry/title matching against the native window list (`EnumWindows`). Exactness when possible, portability always. The fallback is Windows-shaped on purpose: it presumes a native window list, which `java-provider-linux` has to decide separately — on X11 there is none to match against.

### The attach takes effect in the pass that triggered it (added during implementation)

A first implementation attached at the end of an enumeration pass, which made the *next* pass the
first one to serve through the agent. Correct, and visibly poor: in the Inspector a Swing application
showed `JAB`, and only a second refresh showed `JavaAgent`. Worse, it read as "the agent does not
work".

The fix is to close the loop inside the pass: after injecting, wait — bounded, ~hundreds of ms in
practice — for the new agent to publish its handshake file, then **sweep the backends again** and
discard the first sweep. Discarding rather than editing is the point: which node in a flat
`Enumeration.nodes` stands for which window is exactly the question that list cannot answer, so
surgically removing the bridge's nodes for the taken-over windows would need the mapping this design
deliberately does not have. The second sweep needs no special case at all — the agent now records
those windows at its own rank first, so the Access Bridge skips them the same way it does in the
steady state.

Cost: one extra sweep, once per process, on the pass where an attach happened. There is precedent for
paying a bounded wait here — the JAB backend already blocks up to 1500 ms on its first pass for the
asynchronous bridge rendezvous.

Attachment is also retried up to `MAX_ATTACH_ATTEMPTS` times per JVM rather than exactly once. An
attach can fail transiently (the target's attach listener may not exist yet the moment its first
window becomes visible), and with a single attempt that application would lose the agent for the whole
session — a worse outcome than the double refresh this change removes. Unbounded retries are equally
wrong: a JVM that structurally refuses must not be attacked once per enumeration pass for as long as
it runs.

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

## Measured results

Everything below was measured against the Swing fixture (`apps/test-app-swing`) with the agent
injected, not derived from documentation.

### Window-handle internals and `--add-opens` (decision 5, task 1.5)

The expected answer was a per-JDK matrix of `--add-opens` flags. The measured answer is better and
changes the shape of the problem: **no flags are needed on any JDK.**

| JDK | Launch flags | Handle |
|---|---|---|
| 8 | none | ✅ `sun.awt.windows.WComponentPeer#getHWnd` via the public, deprecated `Component.getPeer()` |
| 21 | none, agent does *not* open modules | ❌ none |
| 21 | `--add-opens java.desktop/java.awt=ALL-UNNAMED` | ❌ none — the field read is opened, the peer method is not |
| 21 | that **plus** `--add-opens java.desktop/sun.awt.windows=ALL-UNNAMED` | ✅ `WComponentPeer#getHWnd` |
| 21 | none, agent opens the modules itself | ✅ `WComponentPeer#getHWnd` |

Two findings worth keeping:

- **Two packages, not one.** `java.awt` alone is not enough: reading `Component.peer` needs
  `java.awt` opened and calling `WComponentPeer#getHWnd` needs `sun.awt.windows` opened. A matrix
  that had stopped at the first flag would have concluded "impossible".
- **`Instrumentation.redefineModule` removes the flag requirement entirely.** The command-line remedy
  was never available to this design — attaching to a running JVM means PlatynUI does not own the
  launch line, and a JVM already up cannot be given the flag retroactively. An instrumentation agent
  may open the packages to itself, which is what JEP 261 gave agents the power for. So the agent does
  that at adapter-install time (`ModuleAccess`), and the in-JVM path is the normal path on every JDK
  rather than a Java-8-only luxury.

This does **not** make decision 5's fallback dead code: a JVM that refuses the module redefinition, or
a future peer layout, still lands on the provider's PID+geometry match. It moves the fallback back to
where it belongs — an actual fallback, not the everyday path.

### Element-model field list (decision 1, task 1.2)

One frame per element, every block below optional except the first group. Roles and states are the
`Locale.ENGLISH` display strings, which *are* the Access Bridge's `role_en_US` / `states_en_US`
vocabularies — verified against the fixture: `frame`, `root pane`, `layered pane`, `menu bar`, `menu`,
`menu item`, `panel`, `push button`, `text`, `label`, `check box`, `radio button`, `combo box`,
`slider`, `spinbox`, `progress bar`, `scroll pane`, `viewport`, `table`. So the provider maps one
vocabulary and a JAB-era locator keeps matching.

| Field | Type | Present on |
|---|---|---|
| `id` | int | always — the registry id, identity-stable |
| `kind` | `window`/`component`/`cell`/`accessible` | always |
| `role`, `className` | string | always |
| `childCount` | int | always |
| `states` | [string] | always (empty when the element has no accessible context) |
| `enabled`, `visible`, `showing`, `focusable`, `focused` | bool | always |
| `name` | string | when `Component.getName()` is set, or the model value of a cell |
| `accessibleName`, `accessibleDescription` | string | when set |
| `bounds` | `{x,y,width,height}` | when on screen — **absent**, never zeroed, when not |
| `text`, `editable` | string, bool | text-bearing elements |
| `toolTipText` | string | `JComponent` with a tooltip |
| `value` | `{current,minimum,maximum}` | `AccessibleValue` (slider, spinner, progress bar) |
| `selection` | `{count,indices}` | `AccessibleSelection` |
| `table` | `{rows,columns,selectedRows,selectedColumns}` | `JTable` |
| `cell` | `{row,column,rowExtent,columnExtent,selected,editable}` | virtual cells — the `native:TableCell.*` source |
| `clientProperties` | `{key: scalar}` | `JComponent` with scalar client properties |
| `window` | `{handle,handleSource,title,active,focused,resizable,extendedState,alwaysOnTop}` | top-level windows |

`Component.getName()` is the spine's exclusive contribution and the fixture proves it: `frame0`,
`null.layeredPane`, `Spinner.nextButton` are all invisible to JAB.

### The JTable gap, closed (task 4.1's substance)

Measured on the fixture's 4×3 table with row 2 preselected:

- Each cell reports **its own** name (`r0c0` … `r3c2`) — through JAB every cell reads whatever the one
  shared renderer was configured with last.
- Each cell reports **real bounds**, distinct per row and column. JAB reports none, which is why the
  platform highlighter had nothing to draw around.
- `cell.selected` is `true` exactly for row 2's cells.
- **Ids are stable across enumerations** (`32 == 32` over two passes) — the property JAB's
  enumeration-index scheme cannot have.
- A hit-test at a cell's centre returns a 9-deep chain ending **at the cell**, not at the table.

### Known limitation: one frame per tree level does not scale to huge tables

The wire is coarse on purpose — a node's whole surface in one message, a level's children in one
message — and that is what makes the full attribute surface affordable. It has one pathological case,
worth writing down rather than discovering in the field: a `JTable`'s children are its cells, so
`ui/children` on a large table produces one very large frame. Measured against a real cell payload
(441 bytes compact):

| Table | Cells | One `ui/children` frame |
|---|---|---|
| 100 × 3 | 300 | ~0.1 MB |
| 1 000 × 3 | 3 000 | ~1.3 MB |
| 5 000 × 3 | 15 000 | ~6.3 MB |

Nothing breaks — the frame is read in one go and the deadline is generous — but a tree walk that
descends into a five-thousand-row table pays megabytes for it, and the Access Bridge's per-child
laziness would not. The fix is pagination on `ui/children` (offset/limit), which is a wire addition
rather than a change, so it can land when a real application needs it. Until then the honest summary
is: excellent for ordinary trees, and a table with thousands of rows should be addressed by cell
coordinates rather than walked.

## Verification items

- **Window-handle internals** (5) — ✅ measured, see above. JDK 17 not on this host; 8 and 21 bracket
  the two mechanisms (public accessor vs. opened module), and 17 shares 21's.
- **Element-model field list** (1) — ✅ measured, see above.
- **Coordinate conversion** (3) — implemented and verified at scale 1.0 across JDK 8 and 21
  (`GraphicsConfiguration` transform, one multiplication; see `SwingGeometry` for why the naive
  per-monitor translate cancels). Geometry agreement against the platform's own `GetWindowRect` is
  verified in the provider's live fixture, where the Win32 side is already a dependency.
  **Verification under display scaling is `verify-display-scaling`**, not an open item here: the
  blocker is a scaled display rather than anything Java, and the same display is what the UIA and JAB
  paths need too — JAB's own DPI transform being *identity at 100 %* means it has never run at all.
