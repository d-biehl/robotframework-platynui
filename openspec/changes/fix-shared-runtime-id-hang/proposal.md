# Proposal

## Why

A query against a WinForms application on .NET Framework never ends once it has to walk past the tab headers of a `TabControl`. The test process then has to be killed, and the Inspector keeps a core busy after a cancelled search. The cause is identity, not the query: UI Automation reports an empty runtime id for MSAA-proxied elements that have no window of their own. The provider turns each of them into the same `uia://app/<pid>/`, and the XPath engine finds a node's next sibling by looking the node up by identity, so the walk returns to the same sibling forever.

This was found and measured against KeePass 2.62 on the Windows VM (2026-11-25), with the published 0.13.0.dev301 and with `main` (0d8997db) alike. A plain WinForms `TabControl` hosted by Windows PowerShell 5.1 shows the same empty ids. Every WinForms application on .NET Framework that uses tabs is affected, and so is every provider that reports two siblings with one id.

## What Changes

- **UIA runtime ids that UIA does not supply.** When `GetRuntimeId` returns an empty array or fails, the UIA provider derives the id from the parent's runtime id plus the element's MSAA child id, or, without one, its position among its parent's children. Such an id is unique among the siblings and reproducible: the same element gets the same id whether it was listed or hit-tested. The shared fallback `uia://temp` goes away. Ids that UIA does report stay exactly as they are.
- **Sibling walks that end.** The XPath engine moves from a node to its next or previous sibling by the node's position in its parent's list of children when the model knows that position, instead of searching the list for an equal node. Child, descendant, following and sibling steps therefore end on every model, also on one that reports two siblings as the same node.
- **Cancellation stops every walk.** The engine checks the cancel flag for every candidate node a walk visits, not only for candidates that reach a predicate or a named test. `count(//*)`, and any walk whose candidates the node test rejects, stop once the flag is set.
- **A WinForms test window for the Windows lane.** A small helper window (a WinForms `TabControl` with five tabs, run by `powershell.exe` on .NET Framework) and an acceptance suite that proves the tab headers have distinct runtime ids, that every header is reachable, and that a query for a missing element ends with "not found" instead of hanging.
- **Behavior change (not breaking for valid models):** A UIA element whose runtime id UIA did not supply now has a runtime id of the form `<parent runtime id>/child:<n>` or `<parent runtime id>/index:<n>`. Nobody could rely on the old `uia://app/<pid>/` or `uia://temp`, since every such element shared it.

## Capabilities

### New Capabilities

- `uia-runtime-id`: how the UIA provider forms runtime ids, including elements for which UI Automation reports none, so that every UIA node has an id that is unique among its siblings and stable across the paths that reach it.

### Modified Capabilities

- `xpath-evaluation`: adds that walks over children and siblings end, and keep reaching later nodes, when a model reports two siblings as one node, and that cancellation stops every walk, not only sorts.

## Impact

- **Rust crates:**
  - `crates/provider-windows-uia`: runtime id formation (`map.rs` `format_scoped_runtime_id`, `node.rs` `UiaNode::runtime_id` and `ElementChildrenIter`). Hit-tested nodes (`provider.rs`) take the same fallback via their attached ancestor chain.
  - `crates/xpath`: sibling navigation in the evaluator cursors (`next_sibling_in_doc`, `prev_sibling_in_doc` and their callers), a new optional position hook on `XdmNode`, and the per-candidate cancellation check.
  - `crates/runtime`: the XDM adapter (`xpath.rs`) records each element's position in the list of children that produced it and answers the new hook.
- **Python / Robot Framework:** no API change. BareMetal and `PlatynUI.core` gain the fix through the native module; queries that hung now return or time out normally.
- **Tests:**
  - Unit tests in `crates/xpath` and integration tests in `crates/runtime/tests` with a fake provider whose siblings share runtime ids.
  - Windows-only unit tests for the id composition in `crates/provider-windows-uia`.
  - A new acceptance suite under `tests/acceptance/winforms/` in the `real-windows` lane.
- **New helper:** `apps/winforms-test-window` (a PowerShell script, excluded from the Cargo workspace like the other non-Rust apps). Documented as a helper in `dev-docs/testing-strategy.md`, not a blueprint fixture.
- **Docs:** `dev-docs/architecture.md` §5.4 (the UIA row of the runtime id schema) and `dev-docs/platform-windows.md` (the RuntimeId attribute).
- **Rebuild:** needs a native rebuild (`just build-native`). Published wheels pick the fix up with the next dev build.
- **Platforms and providers:** the id fallback is Windows/UIA only. The engine fix applies to every provider (UIA, AT-SPI, JAB, Java agent, macOS AX, mock), because it guards against a model that breaks the identity contract, whichever provider it is.
- **Out of scope:** elements whose non-empty runtime ids UIA reports twice, like the Dynamics 365 case WinAppDriver users hit, and asking the provider (`CompareElements`) to tell colliding nodes apart. Those are planned for a later change.
