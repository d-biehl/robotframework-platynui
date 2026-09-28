# Proposal

## Why

The runtime answers a query from a snapshot of the UI, and a caller discards the snapshot when it did not find what it looked for (`dev-docs/architecture.md` §9.3). A discarded snapshot is never freed. The XPath layer wraps each provider node, and a child's wrapper holds a strong reference to its parent's wrapper, whose list of children holds the child (`crates/runtime/src/xpath.rs:1153-1162`). Nothing breaks that cycle. Measured on Windows from Python, in private bytes per evaluation:

| Query | Snapshot reused | Snapshot discarded before each query |
|---|---|---|
| `count(/*/*)`, 300 times | 0 | 52.5 KiB |
| `.//*[@Name='x']` under a VS Code window of about 3,500 elements | 0.31 MiB | 14.9 MiB |

The same applies to every snapshot that ends:

- **Discarded snapshots.** BareMetal discards the snapshot after every lookup that found nothing, so a wait that polls every 0.1 s leaks a snapshot per attempt. It also discards before every `Query`.
- **Evaluations without a cache.** `platynui-cli` commands and the Inspector's search build a tree that is released after the call.
- **Replaced snapshots.** An evaluation from another context node replaces the cached one.
- **Revalidation.** A list of children that is read again leaves the old wrappers behind, so even a cache that nobody discards grows, for example in `platynui-cli watch`.

The leak reaches beyond PlatynUI's process. Every leaked Java Access Bridge node pins a JVM global reference, so the application under test leaks memory too. The JAB pump thread never winds down after shutdown, because it waits for the last node to release its client (`crates/provider-java-jab/src/provider.rs:301-303`).

Freeing the snapshot alone is not enough. Every real provider keeps a node's parent only as a `Weak` (UI Automation `crates/provider-windows-uia/src/node.rs:95`, the Java Access Bridge `crates/provider-java-jab/src/node.rs:200`, the Java agent `crates/provider-java/src/agent/node.rs:61`, AT-SPI `crates/provider-atspi/src/node.rs:59`). Today the ancestors of a node that a query returned stay alive only because the leaked snapshot holds them.

A prototype that breaks the cycle and nothing else leaves a returned node without ancestors (`ancestors = []`). That would silently break:

- activating an element's window before an action (BareMetal's auto-activation, `platynui-cli pointer`);
- `UiNode.parent()`, `ancestors()` and `top_level_or_self()`;
- the Inspector's reveal of a search result;
- AT-SPI bounds, which walk the parent chain.

## What Changes

- **The snapshot model becomes a spec.** One snapshot per query; reuse by the next query; revalidation with `UiNode::is_valid`; the caller discards it. This is the model as designed (maintainer decision), with its accepted limits: nodes added under a parent whose cached children are all still valid, and attribute values that a provider keeps from an earlier read.
- **Snapshots are released.** A snapshot's memory and the provider nodes it holds are returned when it is discarded, when another context replaces it, when an evaluation without a cache or a stream ends, and when the runtime shuts down, before its providers do. Revalidation no longer makes a reused snapshot grow. A deep snapshot is released without recursion, so releasing it cannot overflow the stack, and releasing never happens while a lock is held.
- **Revalidation also covers the ancestors of a held context node.** A query from a held element reaches that element's ancestors through wrappers built upward from it. Those are not revalidated today, so `following-sibling::*` from a held element still returns a sibling that has gone. They are now checked like the rest of the snapshot.
- **A node that is handed out keeps its ancestors.** Every provider keeps the parent of each node it lists as a child alive, with a strong reference from child to parent. It never holds a node's children, and it keeps the desktop, which the runtime owns, only as a `Weak`. This is the mechanism the providers already use for the parent chains of hit tests. A held node therefore keeps exactly its chain of ancestors, whether it came from a query with or without a cache, from `children()`, or from a hit test. The mock provider owns its whole tree and is exempt.
- **The contract is written down.** The documentation of `UiNode::parent` states the rule. The core contract testkit gains two checks. One lists a node's children, drops everything else, and asserts that each child keeps its parent reachable. The other asserts that no node holds its children.
- **UI Automation stops leaking runtime ids.** Measuring the fix turned up a second leak: every UI Automation node that computes its runtime id leaks the array `GetRuntimeId` returns, about 90 bytes, whether or not a snapshot is involved. With a snapshot of thousands of elements discarded on every lookup, that alone grows memory by about 0.6 MiB per lookup. The array is now freed.
- **Event-driven invalidation is not pursued (maintainer decision).** Invalidating on every change, or finding the node a change concerns, costs more than it saves, because many events concern parts of the UI that no snapshot has read. `dev-docs/planning.md` §3.1 is marked accordingly.
- **Deliberately unchanged:** the snapshot's semantics, the existing public APIs, the XPath engine (`crates/xpath`), and when BareMetal and the other callers discard the snapshot. The public testkit gains the two checks and four `ContractIssue` variants; these are additions for the release notes.
- **Not a breaking change.** A node held by a caller now keeps its chain of ancestors (O(depth) provider handles) instead of the whole leaked snapshot. Discarding a large snapshot now does real work: it releases COM elements, JAB references and D-Bus proxies. For JAB each release is a blocking call on the pump thread, which runs before the next JAB request, so its cost is measured in the Windows lane.

## Capabilities

### New Capabilities

- `xpath-snapshot`: how the runtime answers queries from a snapshot of the UI. It covers the reuse of a snapshot by the next query and its revalidation, the release of a snapshot that ends, and that a node handed out keeps its ancestors while it is held.

### Modified Capabilities

None. The existing promises that a hit-test result has a walkable parent chain (`element-at-point`, `jab-hit-test`) are unchanged; this change extends the same property to nodes from queries and `children()`.

## Impact

- **Rust crates:**
  - `crates/runtime`: `src/xpath.rs` (the XPath wrappers: a weak link to the parent, release without recursion, dropping outside the locks); `src/runtime/evaluation.rs` and `src/runtime/mod.rs` (`clear_cache`, and `shutdown` releasing the snapshot while the providers still run); a lazy fake provider in `src/runtime/test_fixtures.rs` and new release tests in `tests/xdm_release.rs`.
  - `crates/core`: the documentation of `UiNode::parent` and two testkit checks in `src/ui/contract/testkit.rs`.
  - `crates/provider-windows-uia`, `crates/provider-java-jab`, `crates/provider-java` (agent) and `crates/provider-atspi`: their child iterators keep the parent alive. `crates/provider-windows-uia` also frees the runtime-id array (`src/map.rs`). `crates/provider-mock` is unchanged.
- **Test app:** `apps/test-app-egui` exposes its button row as an accessible group, so that an acceptance scenario can pin a root inside a window.
- **Python/RF:** no code change. New acceptance scenarios cover what the prototype showed would break: an element captured before the snapshot was discarded, and a root inside a window.
- **Tests:**
  - Rust tests with a lazy fake provider that counts created and dropped nodes and `children()` calls.
  - Runtime tests for window activation of nodes from queries with and without a cache.
  - A testkit test and a live Java test for the parent chain.
  - Acceptance scenarios for egui (every lane) and Swing (Windows).
  - A memory measurement on Windows, recorded in the tasks.
  - The mock cannot show release or lost ancestors, because it owns its tree.
- **Native rebuild:** yes. The extension links the runtime and the providers.
- **Platforms:** Windows (UI Automation, JAB, agent) and Linux (AT-SPI on X11 and Wayland) are verified in their lanes. macOS has no provider nodes yet.
- **Docs:**
  - `dev-docs/architecture.md`: §9.3 on a snapshot's lifetime, and the provider checklist on parent references;
  - `dev-docs/testing-strategy.md`: runtime lifetime tests use a lazy fake, because the mock owns its tree;
  - `dev-docs/python-bindings.md`: `clear_cache()` frees memory, and a held `UiNode` keeps its own ancestors alive;
  - `dev-docs/planning.md`: §3.1 and §10.9.
- **Coordination:**
  - `snapshot-validity` adds its own capabilities and does not depend on this change. It makes `PlatynUI.core` discard snapshots more often, so this change should land first.
  - `window-activation-state` has a scenario that depends on an element's ancestors; its runtime tests run as a regression guard here.
