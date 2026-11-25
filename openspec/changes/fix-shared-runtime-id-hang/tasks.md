# Tasks

## 1. XPath engine: sibling positions and cancellation (`crates/xpath`)

- [x] 1.1 Extend the `Keyless` test model (`crates/xpath/tests/it/common/mod.rs`) with a view in which chosen labels compare equal and every listed node reports its position among its siblings. Verify that its own small test shows two nodes `A`, `B` compare equal and report positions 0 and 1.
- [x] 1.2 Write `crates/xpath/tests/it/shared_identity.rs` from the scenarios of "Walks end when siblings share an identity" on that model: `W:[A,B,E]` with `A = B` (`//E`, `count(//*)`, `/W/*`, `/W/E`, `/W/*[3]`, the following- and preceding-sibling cases), and the 50 equal siblings with list-read counts from the recording view. Run every evaluation on a worker thread with a deadline, so a regression fails instead of hanging the run. Verify they fail (deadline exceeded) on the current engine.
- [x] 1.3 Write the cancellation scenarios of "Cancellation stops every walk" with the existing `Keyless::cancelling` view (flag set at the fifth list: `count(//*)`, and `//Edit` over a tree without `Edit`, each at most six lists). Verify that `count(//*)` fails on the current engine.
- [x] 1.4 Add `XdmNode::sibling_position()` with a `None` default and its contract docs (`crates/xpath/src/model/mod.rs`). Use it in `next_sibling_in_doc` and `prev_sibling_in_doc` (`cursors.rs`), with the equality check at the position and the scan as fallback (design Decision 2). Verify that 1.2 passes and `just test-crate platynui-xpath` stays green, document-order and first-result tests included.
- [x] 1.5 Check the cancel flag for every candidate in `NodeAxisCursor::next_item` and `next_item_before` (design Decision 3). Verify that 1.3 passes and `just test-crate platynui-xpath` stays green.
- [x] 1.6 Update `dev-docs/architecture.md` §9.2 (siblings are found by their position in the list, identity only as a fallback) and §9.4 (the cancel flag is checked for every node a walk visits, not "at each axis step"). Verify that the wording matches 1.4 and 1.5.

## 2. Runtime adapter: positions for every listed element (`crates/runtime`)

- [x] 2.1 Write `crates/runtime/tests/xpath_shared_ids.rs` against the public API with a lazy fake provider whose siblings `A` and `B` share one runtime id and which counts its `children()` calls. Cover the runtime-level scenarios of both new `xpath-evaluation` requirements, including the pre-set flag over equal siblings, each behind a deadline as in 1.2. Verify that the sibling scenarios fail on the current adapter even with group 1 done.
- [x] 2.2 Record each element's index where `NodeChildrenIter` appends it, answer `sibling_position()` from it, and let `NodeChildrenIter::nth` index the shared list directly (`crates/runtime/src/xpath.rs`). Verify that 2.1 passes and `just test-crate platynui-runtime` stays green, the snapshot read-count and release tests (`xdm_release.rs`) included.

## 3. WinForms helper and acceptance suite (Windows)

- [x] 3.1 Add `apps/winforms-test-window/winforms-test-window.ps1` (design Decision 4: form `probeForm`, `TabControl` `tabMain` with the pages General, Advanced, Properties, Auto-Type and History, each with a `TextBox`, plus `-Title` and `-AutoClose <seconds>` defaulting to 60) and a README that calls it a helper, not a blueprint fixture. Add the directory to the `exclude` list in the root `Cargo.toml`. Verify on the Windows VM that `powershell.exe -NoProfile -ExecutionPolicy Bypass -File …` shows the window, that UI Automation lists its five `TabItem`s with empty runtime ids, and that the window closes after `-AutoClose`.
- [x] 3.2 Add the paragraph on the helper to `dev-docs/testing-strategy.md` §5, next to `apps/win32-test-window`. Verify that the description matches 3.1.
- [x] 3.3 Write `tests/acceptance/winforms/__init__.robot` and `runtime_ids.robot`, following the `robot-test-style` skill. The suite starts the helper with `Process`, fails with an actionable message when `powershell.exe` is missing, pins `app:Application[@ProcessId=…]` as its root, and is tagged `acceptance real platform:windows`. Cover every scenario of capability `uia-runtime-id` that names the WinForms test window: the kept id from `native:NativeWindowHandle`, `child:1`–`child:5`, the hit test at the third header's centre via `Get Element At Point`, equal ids after `Query` re-reads, the count of 5 with History found, and the 3-second `Wait Until Exists` that fails with `ElementNotFoundError` within 10 seconds. Verify with `robotcode analyze code` that the suite is clean. Then run it on the VM with groups 1 and 2 built (`just build-native`): it SHALL end, and only the derived-id scenarios SHALL fail.

## 4. UIA provider: ids that UIA does not report (`crates/provider-windows-uia`)

- [x] 4.1 Write Windows unit tests for the id composition first, as a pure function of the parent id, the child id and the position: `/child:<n>`, `/index:<n>`, nesting, a non-empty body kept unchanged, and a failed read handled like an empty array. Verify that they compile and fail with `just clippy-windows` on the host and `just test-crate platynui-provider-windows-uia` on the VM.
- [x] 4.2 Implement design Decision 1 in `UiaNode::runtime_id` and `ElementChildrenIter`:
  - derive the id for an empty or failed read, child id first, then position;
  - take the parent from the node, or else UI Automation's raw-view parent;
  - give each listed node a position hint that counts the skipped own-process siblings;
  - compute the position with `CompareElements` when there is no hint;
  - remove `uia://temp`.
  Verify that 4.1 passes on the VM, that `just clippy-windows` is clean, and that the suite from 3.3 is fully green on the VM.
- [x] 4.3 Update the UIA row and add a paragraph on derived ids in `dev-docs/architecture.md` §5.4, and update the RuntimeId bullet in `dev-docs/platform-windows.md`. Verify that both name the `/child:` and `/index:` forms exactly as implemented.
- [x] 4.4 Bound the walk up UI Automation's raw view for a node without a parent node: a loop of at most `MAX_ANCESTORS` (256, shared with `attach_ancestor_chain`) to the nearest ancestor with a reported id, and `…/unlisted:<n>` when it is not reached (spec scenario "A parent chain that does not end is cut off"). Verify with Windows unit tests of `resolve_parent_id` (circle, chain past the limit, nesting) on the VM, a clean `just clippy-windows`, and the green WinForms suite.

## 5. Verification

- [x] 5.1 Run `just check`, `just test` and `just test-python` on the host, and `just cross-target-checks` for the Windows and macOS builds of the touched crates. All SHALL be green.
- [x] 5.2 Run `just test-acceptance-windows` on the VM. The full lane SHALL be green, the WinForms, win32 and egui suites included. Run nothing on the host at the same time.
- [x] 5.3 On the VM, repeat the KeePass measurements from the diagnosis with the repo build against the "Add Entry" dialog: every query that hung (all matches under `m_tabEntry`, a wrong Id such as `m_tbUsername`, `/Tab[@Id='m_tabMain']/*`, `TabItem[@Name='Properties']`, `count(…//*)`) SHALL end, and the five headers SHALL carry `…/child:1` to `…/child:5`. Record the results in the change's notes for the user.

## Workflow follow-up

- Archive the change once the user has reviewed the Windows results.
