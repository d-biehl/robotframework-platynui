# Design

## Context

See proposal.md (Why) for the symptom. Three facts in the code combine into the hang. All three were verified against the code on `main` (0d8997db), and the first also live on the Windows VM.

1. **UIA sometimes reports no runtime id.** For MSAA-proxied elements without a window of their own (the tab headers of a .NET Framework WinForms `TabControl`: `providerId:0x0`, `NativeWindowHandle` 0, MSAA child ids 1–5), `GetRuntimeId` returns an empty array, and the RuntimeId property is "not supported". This held on every retrieval path: the raw-view walker, the walker with the provider's cache request, and `FindAll` on the control view. `CompareElements` still tells the headers apart. It recognizes one header across separate walks, `FindAll` and `ElementFromPoint`, although every path hands out a new COM wrapper. Measured with a comtypes probe against KeePass 2.62 and against a WinForms `TabControl` hosted by Windows PowerShell 5.1.
2. **The provider turns "no id" into a shared id.** `runtime_id_array_body` (`crates/provider-windows-uia/src/map.rs:205`) formats an empty array as an empty body, so `format_scoped_runtime_id` (`map.rs:243`) yields `uia://app/<pid>/` for every such element. A failed read falls back to the constant `uia://temp` (`crates/provider-windows-uia/src/node.rs:376`).
3. **The engine finds siblings by identity.** `next_sibling_in_doc` (`crates/xpath/src/engine/evaluator/cursors.rs:813`) scans the parent's list for the first entry equal to the node and returns the entry after it. `prev_sibling_in_doc` (`cursors.rs:885`) works the same way. Runtime elements are equal when their runtime ids are equal (`crates/runtime/src/xpath.rs:522`), and UIA nodes carry no document-order key. With siblings `[A, B]` that share one id, `next_sibling(B)` finds `A` first and returns `B` again. The child walk (`cursors.rs:262`), the descendant walk (`cursors.rs:302`, through `doc_successor_within` at `cursors.rs:857`), and the following and sibling axes all go through these helpers.

A fourth fact explains why Cancel left the Inspector busy:

4. **Cancellation is checked only where a candidate reaches the VM.** `NodeAxisCursor::next_item` (`cursors.rs:714`) and its bounded variant (`cursors.rs:732`) loop over candidates without checking the flag. A named node test reaches the VM, whose `with_vm` (`crates/xpath/src/engine/evaluator/mod.rs:387`) checks the flag. Wildcard and kind tests take the fast path in `matches_test` (`cursors.rs:439`) and never do. A fake-provider run on `main` and on dev301 confirmed this: `count(//*)` ignored the flag, while `//Edit` stopped.

The runtime adapter already shares one list of children per parent wrapper: a child links weakly to the wrapper whose list produced it (`ParentLink::Linked`, `crates/runtime/src/xpath.rs:435`), set where the list grows (`xpath.rs:1383`). The index each child gets there is known at that moment (`self.pos`), but it is not kept.

## Goals / Non-Goals

**Goals:**

- Remove the hang at both of its roots: give the UIA elements without a reported id distinct ids, and make sibling walks independent of identity wherever the model can say where a node sits.
- Keep the guarantees of `xpath-evaluation` ("The first result does not require reading the rest of the tree") and `xpath-snapshot` (each list read at most once per query) unchanged.
- Make a running walk respond to the cancel flag, whatever its node test is.

**Non-Goals:**

- Telling apart elements whose non-empty runtime ids UIA reports twice. Asking the provider (`CompareElements`) to decide when two runtime ids collide stays a later change.
- Interrupting a native evaluation from a Robot Framework keyword timeout. With the loop gone, queries end on their own.
- Guaranteeing termination for engine models that report equal siblings and give no positions (see Decision 2).
- The observation that a cancelled stream yields the cancellation error on every further `next()` instead of ending. The Inspector and Python stop at the first error. It is not changed here.

## Decisions

### 1. The UIA provider derives an id from the parent when UIA reports none

`UiaNode::runtime_id` (`node.rs:373`) keeps today's path for a non-empty array. For an empty array or a failed read, it composes `<parent id>/child:<n>` from the element's MSAA child id `n` (`UIA_LegacyIAccessibleChildIdPropertyId`, when greater than 0), or otherwise `<parent id>/index:<n>` from its 0-based position among its raw-view siblings. `uia://temp` goes away.

- **Child id first.** It is what UIA's own identity rests on for these elements: the `CompareElements` matrix coincided with "same MSAA child id under the same parent" on every path. It is also known without the sibling list, so a hit-tested element computes the same id as a listed one without walking its siblings.
- **Parent id from the node's own parent.** Listed children and hit-test results already have a parent: `ElementChildrenIter` sets it before it hands out the node (`node.rs:821`), and `attach_ancestor_chain` builds it for hit-test results (`node.rs:196`). That parent's runtime id is cached in its `rid_cell`, so the derivation costs one property read. A node without a parent (a hit test whose process is unknown, `provider.rs:543`) asks UI Automation for its raw-view parent and forms that parent's id by the same rules.
- **Position.** `ElementChildrenIter` counts every raw sibling it walks, including the own-process elements it skips (`node.rs:811`), and gives the node that count as a hint. A node without the hint computes its position from its raw-view parent with `CompareElements`. That only happens for an element that has neither an id nor a child id and was not listed.
- **Syntax `/child:` and `/index:`.** A UIA body consists of hex digits and dots, so a derived id can never equal a reported one. It nests (`…/child:1/child:2`), inherits the `desktop`/`app` scope of its parent, so the one-id-per-view rule (architecture.md §5.4) holds, and it reads well in the Inspector.
  - *Alternative considered:* mimic Microsoft's managed MSAA proxy, which appends `[UiaAppendRuntimeId, index]` to the parent's array. That looks like a real UIA id and could collide with a fragment child that UIA does report under the same window.

### 2. The engine moves between siblings by position when the model knows it

`XdmNode` gains an optional `sibling_position()`, the node's 0-based index in `parent().children()`, defaulting to `None`. When a node has a position, `next_sibling_in_doc` and `prev_sibling_in_doc` take the entry right after or right before it, once a single equality check confirms that the entry at that position is the node. When there is no position, or the check fails, they scan as today.

- **Runtime adapter.** `ElementData` records the index under which `NodeChildrenIter` appended it, next to the `Linked` parent (`xpath.rs:1383`). `NodeChildrenIter` overrides `nth` to index the shared cache directly (`cached_child`, `xpath.rs:1340`), so a sibling step is O(1) instead of a scan, which also removes an O(n²) cost per list. A node built upward with an `Owned` parent (the context node of a query) has no position and scans. Every node a step hands out after it comes from a list and has one, so a walk only ever moves forward.
- **Why termination needs positions.** Without a position, no stateless rule can walk siblings that the model calls equal. "First equal" loops on `[A, B]`. "Skip equal entries" cycles on `[A, C, B]` with `A = B`: `C → B → C`. The requirement therefore says "a model that tells positions". The runtime does for every provider.
- *Alternative considered:* cursors that keep each level's list and index in their own state, so they work on any model. That rewrites every axis cursor and the bounded advance introduced with document order (8f1f6329), for a guarantee that only engine test models would add.
- *Alternative considered:* making runtime equality unique, for example by pointer. The spec defines identity by runtime id (architecture.md §9.2, "Each node once, by identity"), and duplicate removal across lists depends on it.
- **Engine tests.** The engine's `Keyless` test model (`crates/xpath/tests/it/common/mod.rs`) gains a view in which chosen nodes compare equal and every listed node reports its position. That gives the engine-level scenarios a model without the runtime. `SimpleNode` does not implement the hook: it has unique identities, and the scan stays correct for it.

### 3. Every visited candidate checks the cancel flag

`NodeAxisCursor::next_item` and `next_item_before` check `vm.is_cancelled()` (`mod.rs:406`, one relaxed atomic load) for each candidate before they test it, and return the cancellation error. The cost is negligible next to the provider calls a candidate already costs. One check per candidate bounds the reads after the flag is set to the one list the current candidate read, which is what the spec's "at most one further list" states.

### 4. Real-provider proof with a WinForms helper window

`apps/winforms-test-window` holds one PowerShell script: a form with a `TabControl` (`tabMain`) of five pages named like KeePass's (General, Advanced, Properties, Auto-Type, History), each with a `TextBox`. The suite runs it with `powershell.exe -NoProfile -ExecutionPolicy Bypass -File …`, which is Windows PowerShell 5.1 on .NET Framework 4.8 on every Windows 10/11 machine, so it needs no build and no SDK. The spike on the VM reproduced the empty ids and child ids 1–5 with exactly this setup. It is a helper like `apps/win32-test-window` (testing-strategy.md §5): no blueprint, no catalog, one suite (`tests/acceptance/winforms/`), tagged `acceptance real platform:windows`. Like the other non-Rust apps it is excluded from the Cargo workspace.

## Risks / Trade-offs

- [A child id or position changes when tabs are added or removed, so the derived id of a header can change while the element lives.] → Accepted: UIA's own MSAA-proxy identity changes the same way, and nothing better is reported. It stays reproducible for an unchanged tree, which is what the spec requires.
- [The parent's id must be final before a child derives from it.] → Parents are created and linked first on every path (listing, hit-test chain), and a derived id is computed lazily on first read, never at construction.
- [A stale position after a list was read again could pick a wrong sibling.] → The equality check at the position catches it and falls back to the scan. Revalidation replaces the wrappers of a list that was read again, and they carry fresh positions.
- [`/W/*`-style results may contain both equal siblings, while a sorted step removes one of them.] → Unspecified by design for models that break identity. For UIA, Decision 1 makes the ids distinct, so it does not arise there.
- [The WinForms helper depends on Windows PowerShell 5.1 and .NET Framework, which ship with Windows but could be removed by policy.] → The suite fails with an actionable message when `powershell.exe` is missing. It never skips (testing-strategy.md).

## Migration Plan

- **Kind:** behavioral and additive. UIA elements without a reported id get new ids. Every other id, and every result on a model with unique ids, stays the same. The `XdmNode` hook is additive with a default.
- **Rebuild:** a native rebuild is needed (`just build-native`), and the wheels pick it up with the next dev build.
- **Rollback:** revert the change. No persisted data depends on the ids.
