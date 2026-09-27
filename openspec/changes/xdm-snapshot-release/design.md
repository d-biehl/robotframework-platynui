# Design

## Context

See proposal.md for the motivation and `specs/xpath-snapshot/spec.md` for the required behavior. *Verified* marks what was read in the working tree at `18353c3` or run; *inferred* marks conclusions.

The facts come from:

- a research pass over the XPath layer and the providers;
- a prototype of the fix, in a copy of `crates/runtime` outside the repository;
- an adversarial review of that prototype.

The prototype passes all 120 unit tests of `platynui-runtime`. The probes below ran against it and against the unchanged code, with the same lazy fake provider.

**The XPath wrappers (verified).**

- `RuntimeXdmNode` wraps a provider node as `Document(DocumentData)`, `Element(ElementData)` or `Attribute(AttributeData)` (`crates/runtime/src/xpath.rs:380-385`). Their mutable state sits in separate `Arc` cells: the provider's child iterator, the cached children, the "finished" and "validated" flags, the attribute cells, and for elements `parent_cache` (`:25-31`, `:742-813`). `derive(Clone)` shares these cells between clones.
- `NodeChildrenIter::next` wraps each new child and stores a strong clone of the parent wrapper in the child's `parent_cache` (`:1153-1161`), then pushes the child into the parent's cached children (`:1162`). Document and Element both build the iterator with `.with_parent_node(self.clone())` (`:576`, `:607`). That makes every parent–child edge that a query materializes a strong cycle. `xpath.rs` has no `Weak` and no `Drop`. Measured with the fake:
  - `/*` leaks 3 of 3 nodes, and `count(//*)` leaks 39 of 39;
  - an `EvaluationStream` dropped after two items leaks 9;
  - a retained cache whose containers turn invalid grows by 117, 234 and 351 live nodes over three queries.
- Attribute wrappers hold only the provider node (`:815-826`) and are in no cycle. `prepare_for_evaluation` clears them on every query (`:422-446`).
- The pre-link exists for performance, not correctness. The child axis walks siblings with `next_sibling_in_doc`, which calls `parent().children()` for each child (`crates/xpath/src/engine/evaluator/cursors.rs:187-199`, `:719-731`). Only the same parent wrapper, with its shared list of children, keeps that at one provider enumeration per list. Equality compares `runtime_id` and `order_key` (`xpath.rs:449-462`), so a fresh parent gives correct results but enumerates again.
- When no parent is pre-linked, `parent()` builds a fresh wrapper from the provider's parent and stores it (`:524-540`). If the provider's parent is gone, it returns a document that wraps the element itself (`:531-537`). `..` then yields the element, and an absolute path turns into a search below it (probe).
- A stream keeps its root alive through the `Rc<DynamicContext>` that every cursor of the engine holds (`crates/xpath/src/engine/evaluator/mod.rs:140-148`, `:335-374`). Results carry only provider nodes (`xpath.rs:359-378`).
- `prepare_for_evaluation` walks only the cached children (`:422-446`). The wrappers that `parent()` builds upward from a held element are never checked again. Probe: from a held button, `following-sibling::*` still returns a sibling after it reports invalid, on the unchanged code as well as on the prototype before this was addressed.
- Snapshots are dropped while locks are held:
  - `XdmCache::clear` (`:52-54`);
  - the slot replacement in `get_or_create_xdm_root` (`:339-342`);
  - `Runtime::clear_cache`, which calls `clear` while holding the runtime's own mutex (`crates/runtime/src/runtime/evaluation.rs:121-123`);
  - revalidation, which clears a parent's list under that list's lock (`xpath.rs:556`, `:587`) and replaces the provider iterator there (`:558`, `:589`).
- `children()` calls the providers' `is_valid` while it holds the lock of the cached children (`:549-554`, `:580-585`), so a provider that panics there poisons that lock.
- The runtime's cache field is dropped after `provider.shutdown()` (`crates/runtime/src/runtime/mod.rs:58-71`, `:338-362`).

**The providers (verified).**

- Every real provider keeps a node's parent as a `Weak`: UI Automation `crates/provider-windows-uia/src/node.rs:95`, JAB `crates/provider-java-jab/src/node.rs:200`, the agent `crates/provider-java/src/agent/node.rs:61`, AT-SPI `crates/provider-atspi/src/node.rs:59`. Their field comments say that the parent of a tree node "is kept alive by the tree/consumer".
- Each of them already has a strong slot for the parent chains of hit tests: UI Automation `node.rs:165-169`, `:190-221`; JAB `node.rs:772-773`, `:852`; the agent `backend.rs:505-512`; AT-SPI `node.rs:129-131`, `lib.rs:405-424`.
- No provider node holds its children or a wrapper. The strong references that provider nodes hold point to objects that hold no nodes: the foreign-window map, the popup registry, the window managers and the Java classifier.
- The mock provider owns its whole tree, and `MockNode` holds its children (`crates/provider-mock/src/node.rs:17-18`, `provider.rs:15-28`).
- Constructors also serve `get_nodes`, where the parent is the desktop: UI Automation `provider.rs:321`, JAB `provider.rs:339-347`, the agent `backend.rs:399`, `:408`, AT-SPI `lib.rs:324`.

**What depends on ancestors (verified).**

- `Runtime::bring_to_front` and `top_level_window_for` (`crates/runtime/src/runtime/window.rs:43-79`).
- `PyNode.parent`, `ancestors` and `top_level_or_self` (`packages/native/src/runtime.rs:117-150`).
- BareMetal's automatic activation and its pinned roots (`src/PlatynUI/BareMetal/__init__.py:262-324`).
- `platynui-cli pointer` after an evaluation without a cache (`crates/cli/src/commands/pointer.rs:588-604`).
- The Inspector's reveal of a search result (`apps/inspector/src/viewmodel/async_tasks.rs:192-211`).
- AT-SPI's bounds (`crates/provider-atspi/src/extents.rs:177-237`).
- The platforms' `extract_pid` (`crates/platform-linux-x11/src/window_manager.rs:377-392`, `crates/platform-linux-wayland/src/window_manager/platynui_ipc.rs:388-429`).

With the cycle broken and nothing else changed, a held button has no ancestors and `count(ancestor::*)` is 0 (prototype).

**Release costs (verified).** Dropping a JAB node is a non-blocking send (`crates/provider-java-jab/src/handle.rs:39-45`). The pump thread, however, drains every queued release before it runs the next job (`pump.rs:161-163`, `:198-223`), and `releaseJavaObject` is a blocking call into the Access Bridge (`:155-158`). UI Automation releases its COM elements on whatever thread drops the last reference; it runs in a per-thread MTA (`crates/provider-windows-uia/src/com.rs:32-45`).

## Goals / Non-Goals

**Goals:**

- Every snapshot that ends is released, with the provider nodes that only it held.
- A node that is handed out keeps its chain of ancestors, and only that chain.
- Revalidation also covers the ancestors reached upward from a held element.
- The public API, the XPath engine and the number of provider enumerations per query stay as they are.

**Non-Goals:**

- The snapshot's semantics: reuse, `is_valid`, and the caller discarding it (maintainer decision).
- Event-driven invalidation (maintainer decision; `dev-docs/planning.md` §3.1 is marked as not pursued).
- A runtime-side safety net for providers that break the rule, and turning the fallback for a dead parent (`xpath.rs:531-537`) into an error (Open Questions).
- Splitting JAB releases between the pump's jobs (Open Questions).
- The recursion limit of evaluation itself: queries over chains deeper than about 1,500 to 2,000 levels overflow today, before any release.
- `UiNodeExt::ancestors()` including the desktop for deeper nodes, contrary to `PyNode.ancestors`' documentation (`crates/core/src/ui/node.rs:134-137`, `packages/native/src/runtime.rs:125-127`). A follow-up.

## Decisions

### 1. Providers keep the parent of every node they list

A provider node keeps its parent alive with a strong reference when the node was listed as someone's child. Top-level nodes reach the runtime's desktop node only through a `Weak`. No node holds its children. Ownership downward belongs only to the XPath snapshot and to consumers.

This is the mechanism the providers already use for hit-test chains, so it reuses their keepalive slots (`hold_parent` and its equivalents). The strong link is set in the providers' child iterators, never in constructors, because the constructors also serve `get_nodes` with the desktop as parent:

| Provider | Where |
|---|---|
| UI Automation | `ElementChildrenIter::next` (`node.rs:816-818`); `AppWindowIter::next` (`:1859-1861`), so a window keeps its `ApplicationNode` |
| JAB | `ChildIter::next` (`node.rs:654-667`); `JabAppNode::children` (`:1412-1422`) |
| Agent | the closure in `AgentNode::children` (`node.rs:264-272`); `AgentAppNode::children` (`app.rs:130-133`) |
| AT-SPI | the closure in `AtspiNode::children` (`node.rs:366-375`), grafted popups included |
| Mock | unchanged: it owns its tree, and a strong link back would form a cycle |

A held node therefore keeps O(depth) provider handles alive, shared among siblings. That holds for every way a node is obtained: queries with and without a cache, `children()`, and hit tests.

*Alternatives rejected:*

- **The runtime pins the chain when it hands a node out** (a handle in `EvaluationItem`, or a delegating wrapper node). It covers only XPath results, not `children()` or walks inside the providers (AT-SPI bounds, `extract_pid`), and it duplicates the providers' mechanism.
- **A strong parent in the `UiNode` trait.** About 72 implementations and every consumer change, the mock needs restructuring against cycles, and the desktop needs a special case.
- **Results pin their whole snapshot.** One held node would keep every enumerated sibling alive, which is the 14.9 MiB case, and `clear_cache` would free nothing while a result is held.
- **Dead parents are resolved again on demand.** That costs round trips per ancestor and has to reproduce the scopes of the runtime ids. A resolved parent still needs an owner, which leads back to this rule.
- **Holding the parent in constructors, or holding the desktop strongly.** Both would pin the desktop and every provider. With the mock they would form desktop → provider → roots → desktop.

### 2. The wrappers link to their parent weakly

- The enum carries `Arc<DocumentData>` and `Arc<ElementData>`. A clone of a wrapper becomes a single reference-count increment. The per-field cells stay, so the iterators do not change.
- `parent_cache` becomes `Mutex<ParentLink>`, with `ParentLink = Unresolved | Linked(Weak<…>) | Owned(Option<RuntimeXdmNode>)`.
- `NodeChildrenIter::next` stores `Linked` with a `Weak` to the parent wrapper.
- `parent()` returns an `Owned` parent. For `Linked` it returns the parent if the `Weak` still upgrades. If it does not, `parent()` falls back to building a fresh parent from the provider, as for `Unresolved`, and stores it as `Owned`.

**Why this is acyclic.** Every strong edge between wrappers points from an older wrapper to a newer one:

- a child is created inside its parent's iterator;
- an `Owned` parent is created by the `parent()` call that stores it;
- attributes hold only provider nodes.

This rests on one invariant: `Owned` holds only a wrapper that the same `parent()` call has just created, never an existing one. It is written as a comment at `ParentLink`, and the drop-count tests guard it.

**Why the O(N²) guard holds.** During a query, the root is pinned by the stream, and a pre-linked parent is reachable downward from the root. So `parent()` returns the same wrapper with the shared list of children, as today. Measured on a tree three to six levels deep, for `//Button`, `count(//*)`, `//Pane/following-sibling::*`, `(//Button)[last()]` and `//Button/preceding::Pane`: 436 provider `children()` calls before and after, which is one per node.

A `Weak` goes dead only off the root: for an attribute's owner, or when another query on the same snapshot has meanwhile read a list again while a stream was still running. The fallback then costs one enumeration and reads the current UI. A stream that is still running may therefore end a sibling walk early when a sibling has gone meanwhile. That is the same kind of behavior as today's revalidation of a parent list, and it is documented in `dev-docs/architecture.md` §9.3.

*Alternatives rejected:*

- **Breaking the links procedurally when a snapshot ends** (at `clear`, at replacement, on stream drop, on errors). Freedom from leaks would then depend on every exit path, panics included. It needs a locked walk over the whole snapshot and damages streams that still share it.
- **An arena per snapshot.** It rewrites about 600 to 800 lines of `xpath.rs` and needs a snapshot-wide lock. Revalidation would orphan slots until the whole snapshot drops, so a cache that nobody clears would grow again (`PlatynUI.core`, `platynui-cli watch`).
- **Sibling hooks in the engine.** They are no fix for the cycle and change the public trait of `crates/xpath`.
- **A `Weak` that returns `None` when dead.** The child axis would stop after the first child, ancestors would end early, and `ToRoot` would raise `XPDY0050` (`crates/xpath/src/engine/evaluator/mod.rs:637-651`).
- **A `Weak` per cell.** An element's plain fields (role, name, order key, node) cannot be rebuilt from `Weak` cells.

### 3. Release without recursion and without aborting

- `Drop` for `DocumentData` and `ElementData` moves the cached children, and an `Owned` parent, onto a stack and drains it iteratively with `Arc::into_inner`.
- While it drains, it collects the provider node of each released wrapper with its depth relative to where the release started: a child one level below its parent's wrapper, an owned parent one level above its element. It finally drops them deepest first. A provider's chain of parents therefore never recurses during release either, in either direction. A plain reverse pre-order would not be enough, because an owned parent is visited after the element that owns it.
- Every lock in `Drop` and in the drain tolerates poisoning (`PoisonError::into_inner`). Measured on the prototype before this was added: when a provider's `is_valid` panicked during revalidation, the following release panicked a second time inside a destructor and aborted the process (exit `0xC0000409`). From Python that would end the Robot Framework run instead of raising.

### 4. Nothing is released while a lock is held

Each place that drops a snapshot or part of one takes the value out with `std::mem::take` or `replace`, releases the lock, and drops the value afterwards:

- `XdmCache::clear`, and the slot replacement in `get_or_create_xdm_root`;
- `Runtime::clear_cache`, which becomes `self.shared_xpath_cache().clear()`, so the runtime's own mutex is not held either;
- revalidation's clearing of a list of children, and its replacement of the provider iterator (`xpath.rs:556`, `:558`, `:587`, `:589`);
- replacing an `Owned` parent in `parent()`.

A child never links back strongly to the parent whose lock is held, so no deadlock was possible. The point is that COM, JAB and D-Bus releases run outside the query's locks.

An exhausted provider iterator is also dropped when its list is finished (`xpath.rs:1165-1167`), which frees UI Automation's tree walker and cache request (`crates/provider-windows-uia/src/node.rs:749-757`).

### 5. Revalidation includes the ancestors of a held element

`prepare_for_evaluation` on an element also follows an `Owned` parent and prepares it. This ends, because an `Owned` parent's own children link back with `Linked`, never `Owned`. The ancestors reached upward from a held element are then checked like every other part of the snapshot. That satisfies the documented promise that a snapshot does not return an element that has gone (`dev-docs/architecture.md` §9.3). It matters most to `PlatynUI.core`, which queries from held contexts.

### 6. Shutdown releases the snapshot first

`Runtime::shutdown` clears the shared cache before it shuts down the event dispatcher and the providers. The providers then release their nodes while they still run. A runtime that is still alive when the Python interpreter exits no longer releases its snapshot after a third-party `atexit` hook, such as a `CoUninitialize`.

### 7. The rule is written down and checked

- The documentation of `UiNode::parent` (`crates/core/src/ui/node.rs:30`) states the rule: the `Weak` upgrades for as long as the node lives, except for the desktop, and nodes never hold their children. The providers' keepalive field comments and the provider checklist in `dev-docs/architecture.md` (§7.3, "Set parent references correctly in children iterators") say the same.
- The core contract testkit (`crates/core/src/ui/contract/testkit.rs`) gains two checks:
  - `verify_children_keep_parent(parent, max_children)` lists the children, drops the listing and the parent, and asserts that each child's parent still upgrades to the same runtime id. If the parent outlives its children, something besides them holds it (the caller, or a provider that owns its tree), and the check reports that it cannot prove the rule instead of passing. It reports the same for a node without children.
  - `verify_subtree_released(root, max_nodes)` records a `Weak` to every listed node, drops all strong references, and asserts that every `Weak` is dead. It fails for a provider that caches its children, which together with decision 1 would bring back a cycle. The mock is exempt.

### 8. Tests where the behavior can be shown

The mock owns its tree, so it can show neither a release nor lost ancestors. The fixtures of the runtime's existing tests own theirs too (`xpath.rs:1386-1540`, `crates/runtime/src/runtime/test_fixtures.rs:118-139`).

- **A lazy fake provider** in `crates/runtime/tests/xdm_release.rs`, which uses only the public API:
  - it creates fresh children on every `children()` call and keeps parents as `Weak`, with the strong link of decision 1 as an option;
  - validity can be switched per level;
  - counters per tree record created and dropped nodes and `children()` calls.

  It covers every scenario of *A query reads one snapshot*, *The next query may reuse the snapshot* and *A snapshot that ends is released*. The deep-release test builds a chain of 10,000 levels on a thread with a 64 MiB stack and clears it on a thread with a 256 KiB stack, which does not depend on the build profile.
- **Runtime tests** with a lazy tree factory in `test_fixtures.rs`, modeled on `RejectingWindowFactory` (`:345-434`), with a window that counts activations (tests next to `runtime/window.rs:247-295`). They cover:
  - `bring_to_front` for a button from `evaluate_single` without a cache (the CLI's path);
  - the same after a cached query and `clear_cache()` (BareMetal's path);
  - a button found below a pane used as the context (a root inside a window);
  - shutdown releasing the snapshot while the providers still run.
- **Testkit tests** for both checks: a lazy node without the strong link fails, one with it passes, and an owned tree reports no child but cannot prove the rule.
- **UI Automation:** a unit test that is not ignored and runs in `just test` on Windows, next to `desktop_root_satisfies_the_common_attribute_contract` (`crates/provider-windows-uia/src/node.rs:2132`). It runs both checks on the taskbar's element (`Shell_TrayWnd`), which avoids the `WM_GETOBJECT` stalls of arbitrary windows.
- **JAB and the agent:** the live Java tests (`crates/provider-java/tests/live_fixture.rs`, `#[ignore]`, run by `just test-acceptance-windows`) reach a table cell through the application node and run both checks, with only the check holding the nodes. They also assert that the cell's ancestors reach the window and the application node.
- **AT-SPI** has no ignored tests that a lane runs. The acceptance scenarios cover it on X11 and Wayland, including the bounds of a captured element, which AT-SPI computes along the parent chain.
- **Acceptance** (`robot-test-style`): the two egui scenarios of *A node that is handed out keeps its ancestors*, with the windows stacked (`Stack Windows`, `tests/acceptance/egui/auto_activate.robot:120-124`) so that a click cannot reach a covered window without activation. There is also a Swing variant under the root pinned to the application node, for JAB and the agent. Before they are relied on, each is run once on a build that releases snapshots but does not keep parents, and must fail there.
- **Measurements on Windows**, recorded in the tasks:
  - private memory per evaluation with `clear_cache()` before each run, for `count(/*/*)` and for a search under a large editor window, 20 runs each;
  - the same search on a retained snapshot, against a static window (Notepad) and against the editor;
  - the time of `clear_cache()` after the large snapshot;
  - the latency of the first JAB query after `clear_cache()` on the Swing table, and the total time of the Swing lane;
  - the process's exit code at the end of the lane, and the Application event log for a crash at interpreter exit;
  - a few minutes of `platynui-cli watch --expression` against a busy application, with flat memory.

### 9. UI Automation frees the runtime-id array

Measured after decisions 1 to 6, discarding a snapshot of about 6,000 elements still grew memory by about 0.6 MiB per lookup, linearly. A walk through `children()` did not grow; one that read each node's runtime id did. `runtime_id_hex_body` (`crates/provider-windows-uia/src/map.rs`) never frees the `SAFEARRAY` that `IUIAutomationElement::GetRuntimeId` hands to its caller. It now calls `SafeArrayDestroy` on every path. This is a defect of its own, older than the snapshot leak, but without the fix a discarded snapshot keeps costing memory on Windows, and the spec's memory scenario fails.

### 10. Shared snapshots stay consistent under concurrent use

`XdmCache` is `Send + Sync`, so two threads can query one snapshot. The review of the fix found two effects of that, one of them older than this change: an iterator could skip a child that another iterator appended to the same list meanwhile, and a provider iterator could be installed again after another thread had finished the list and dropped it. Every check of a shared list is therefore repeated under the lock that guards appending to it, and the list is marked finished only after its last child was appended. No caller in this repository queries one snapshot from two threads today; the Python binding holds the GIL.

## Risks / Trade-offs

- **[The provider rule is a convention; the type system does not enforce it]** → The trait documentation, the provider checklist and the two testkit checks, which run against UI Automation on every Windows `just test` and against the Java providers in the Windows lane. A provider outside this repository that keeps only `Weak` parents loses its ancestors: `..` then yields the node itself (`xpath.rs:531-537`). See Open Questions.
- **[Held nodes now keep their chains of ancestors]** → That is intended and costs O(depth) provider handles per held node. BareMetal's pinned root and the nodes Python holds now keep exactly their chains, instead of the whole leaked snapshot.
- **[Releasing now actually happens, on the thread that drops the last reference]** → COM `Release`, JAB releases and zbus proxy drops run on a Python thread, a tokio worker or the Inspector's UI thread. UI Automation's per-thread MTA and the implicit MTA should cover threads that never initialized COM (*inferred*). The Windows lane and the Inspector smoke test check this.
- **[JAB releases delay the next JAB request]** → The pump drains releases, which are blocking calls, before its next job. Clearing a large Swing snapshot, which BareMetal does on every `Query` and every miss, therefore delays the next JAB query. The Windows lane measures it. If it matters, a follow-up caps the releases per pump iteration (`crates/provider-java-jab/src/pump.rs:161-163`, `:198-223`). In exchange, the application under test no longer leaks JVM global references, and the pump winds down after shutdown.
- **[A dead `Weak` during interleaved queries on one snapshot]** → The fallback is correct but reads the current UI, and a running stream may end a sibling walk early. This is documented, and it is the same kind of behavior as revalidation of a parent list today.
- **[The `Owned` invariant could be broken by a later optimization]** → It is documented at `ParentLink`, and the drop-count tests fail if a cycle returns.
- **[The mock and the existing fixtures cannot see any of this]** → The lazy fake is the runtime's guard, and the testkit checks guard the providers.
- **[The warm growth of 0.31 MiB per evaluation may have another cause]** → The mechanism that the prototype removed (growth through revalidation) is one candidate. The Windows measurement compares a static window with the editor. If warm growth remains, it becomes a follow-up (allocator, UI Automation's client caches).
- **[Coordination]** → `window-activation-state` has a scenario that depends on ancestors; its runtime tests are part of `just test` and run as a regression guard. `snapshot-validity` does not depend on this change, but it makes `PlatynUI.core` discard snapshots more often, so this change should land first.

## Migration Plan

- **Behavioral where stated:**
  - memory is released;
  - a held node keeps its ancestors instead of the whole snapshot;
  - releasing happens when a snapshot ends and at shutdown;
  - the ancestors of a held element are revalidated;
  - UI Automation frees the runtime-id arrays it used to leak.

  No existing public API changes. The public core testkit gains `verify_children_keep_parent`, `verify_subtree_released` and four `ContractIssue` variants.
- **Native rebuild:** yes. The extension links the runtime and the providers.
- **Sequence:**
  1. The failing tests: the lazy fake, the drop counts and the testkit.
  2. The provider rule and its documentation. Behavior does not change yet, because the leak still keeps everything.
  3. The wrapper fix in `xpath.rs`.
  4. The acceptance scenarios, lanes and measurements.
  5. The documentation.

  The rule comes before the fix, so that no intermediate state loses ancestors.
- **Rollback:** revert in reverse order. Reverting the wrapper fix alone brings the leak back and keeps the ancestors. Reverting the provider rule while the wrapper fix stays would lose ancestors, so it is never reverted alone.

## Open Questions

- A safety net for providers outside this repository that keep only `Weak` parents: a runtime-side pin, or a DEBUG record when a provider's parent no longer upgrades. Deferrable: no provider in the repository needs it after decision 1.
- Turning the fallback for a dead provider parent (`xpath.rs:531-537`) into an error. Deferrable: after decision 1 only providers that break the rule reach it, and it is a behavior change of its own.
- Splitting JAB releases between the pump's jobs. Deferrable: it depends on the Windows measurement.
